use crate::auth::{check_auth, generate_access_token, generate_temp_code};
use crate::send_mail::async_send_mail;
use crate::{
    auth::compose_temp_code_mail,
    model::{PartialUser, Settable, User},
};
use actix::prelude::*;
use actix_redis::{Command, RedisActor};
use actix_web::{Error as AWError, HttpRequest, HttpResponse, web};
use futures::future::{join_all, join};
use redis_async::{resp::RespValue, resp_array};

/// gets a list of users nickname from a list of ids...
/// it will be [[id, nickname]]
pub async fn from_ids(
    redis: web::Data<Addr<RedisActor>>,
    user_ids: web::Json<Vec<String>>,
) -> Result<HttpResponse, AWError> {
    let user_ids = user_ids.into_inner();

    let commands = user_ids.iter().map(|id| {
        let domain = format!("user:{}", id);
        redis.send(Command(resp_array!["GET", &domain]))
    });

    let res: Vec<Result<RespValue, AWError>> = join_all(commands.into_iter())
        .await
        .into_iter()
        .map(|item| {
            item.map_err(AWError::from)
                .and_then(|res| res.map_err(AWError::from))
        })
        .collect();

    let mut user_nicknames: Vec<(String, String)> = Vec::new();

    if !res.iter().all(|res| match res {
        Ok(RespValue::BulkString(x)) => {
            let user:User = serde_json::from_slice(x).expect("I should be able to deserialize");
            user_nicknames.push((user.id, user.nickname));
            true
        }
        _ => false,
    }) {
        Ok(HttpResponse::InternalServerError().finish())
    } else {
        Ok(HttpResponse::Ok().json(user_nicknames))
    }
}

pub async fn get(
    redis: web::Data<Addr<RedisActor>>,
    user_id: web::Path<String>,
    req: HttpRequest,
) -> Result<HttpResponse, AWError> {
    let user_id = user_id.into_inner();

    let domain = format!("user:{}", user_id);

    let get = redis.send(Command(resp_array!["GET", &domain]));
    let auth = check_auth(&redis,&user_id,&req.headers());

    let (res, is_auth) = join(get, auth).await; 
    
    if let Ok(auth) = is_auth {
        if !auth {
            return Ok(HttpResponse::Unauthorized().finish())
        }
    } else {
        return Ok(HttpResponse::Unauthorized().finish())
    }
    
    match res? {
        Ok(RespValue::BulkString(x)) => Ok(HttpResponse::Ok().body(x)),
        _ => Ok(HttpResponse::InternalServerError().finish()),
    }
}

// FIXME: this should go away, once the auth is working seaminglessly
pub async fn put(
    redis: web::Data<Addr<RedisActor>>,
    user: web::Json<PartialUser>,
) -> Result<HttpResponse, AWError> {
    let user: User = user.into_inner().into();

    let set = redis.send(Command(resp_array!["SET", &user.domain(), &user.json()]));

    let append = redis.send(Command(resp_array!["SADD", "users", &user.id]));

    let (res, _) = join(set, append).await;

    match res? {
        Ok(RespValue::SimpleString(x)) if x == "OK" => Ok(HttpResponse::Ok().json(&user)),
        _ => Ok(HttpResponse::InternalServerError().finish()),
    }
}

pub async fn sign_up(
    redis: web::Data<Addr<RedisActor>>,
    p_user: web::Json<PartialUser>,
) -> Result<HttpResponse, AWError> {
    let p_user = p_user.into_inner();

    let temp_code = generate_temp_code(&p_user);
    let email = &p_user.email.to_owned();

    let user: User = p_user.into();
    let temp_code_domain = format!("temp_code:{}",&temp_code);
     
    let email = compose_temp_code_mail(&user, &email, &temp_code);
    let send = async_send_mail(email);
    let temp = redis.send(Command(resp_array!["SET", &temp_code_domain, &user.id]));
    let set = redis.send(Command(resp_array!["SET", &user.domain(), &user.json()]));
    let append = redis.send(Command(resp_array!["SADD", "users", &user.id]));

    let redis = join_all(vec![set, append, temp]);

    let (res, _) = join(redis, send).await;

    match res.iter().nth(0) {
        Some(Ok(Ok(RespValue::SimpleString(x)))) if x == "OK" => Ok(HttpResponse::Ok().body("email was sent with temp code")),
        _ => Ok(HttpResponse::InternalServerError().finish()),
    }
}

pub async fn verify_temp_code(
    redis: web::Data<Addr<RedisActor>>,
    path: web::Path<(String, String)>, // user_id and temp
) -> Result<HttpResponse, AWError> {
    let (user_id, temp_code) = path.into_inner();

    let domain = format!("user:{}", user_id);
    let code_domain = format!("temp_code:{}", &temp_code);

    let get = redis.send(Command(resp_array!["GET", &domain]));
    let temp = redis.send(Command(resp_array!["GET",&code_domain]));

    let (user, uid_from_temp) = join(get, temp).await;

    let uid: String = match uid_from_temp? {
        Ok(RespValue::BulkString(x)) => String::from_utf8(x).unwrap(),
        _ => return Ok(HttpResponse::Unauthorized().finish())
    };

    let mut user: User = match user? {
        Ok(RespValue::BulkString(x)) => {
            serde_json::from_slice(&x).expect("user should be deserializable")
        }
        _ => return Ok(HttpResponse::InternalServerError().finish()),
    };

    if user.id == uid {
        let access_token = generate_access_token(&user);
        let token_domain = format!("access_token:{}",&access_token);
        let set_token = redis.send(Command(resp_array!["SET", &token_domain, &user.id]));
        user.is_verified = true;

        let set = redis.send(Command(resp_array!["SET", &domain, user.json()]));

        let (set, _set_token) = join(set, set_token).await;

        match set? {
            Ok(RespValue::SimpleString(x)) if x == "OK" => Ok(HttpResponse::Ok().json(access_token)),
            _=> Ok(HttpResponse::InternalServerError().body("unable to set access token"))
        }
    } else {
        Ok(HttpResponse::Unauthorized().body("invalid user_id, temp_code pair"))
    }
}

pub async fn verify_auth_code(
    redis: web::Data<Addr<RedisActor>>,
    user_id: web::Path<String>,
    req: HttpRequest,
    )-> Result<HttpResponse, AWError> {

    let user_id = user_id.into_inner();
    
    let is_valid = check_auth(&redis,&user_id, req.headers()).await?;
    
    match is_valid {
        true => Ok(HttpResponse::Ok().body("yes")),
        false => Ok(HttpResponse::Unauthorized().finish())
    }
}

pub async fn delete(
    redis: web::Data<Addr<RedisActor>>,
    user_id: web::Path<String>,
    req: web::HttpRequest, 
) -> Result<HttpResponse, AWError> {
    let user_id = user_id.into_inner();
    let headers = req.headers();
    let domain = format!("user:{}", user_id);

    let del = redis.send(Command(resp_array!["DEL", &domain]));
    let pop = redis.send(Command(resp_array!["SREM", "users", &user_id]));
    let auth = check_auth(&redis, &user_id, &headers);

    let ((res, _), is_auth) = join(join(del, pop), auth).await;

    if let Ok(verified) = is_auth {
        if !verified {
            return Ok(HttpResponse::Unauthorized().finish());
        }
    } else {
        return Ok(HttpResponse::Unauthorized().finish());
    }

    match res? {
        Ok(RespValue::Integer(x)) if x == 1 => Ok(HttpResponse::Ok().body("deleted user")),
        _ => Ok(HttpResponse::InternalServerError().finish()),
    }
}
