use crate::auth::{check_auth, generate_access_token, generate_temp_code};
use crate::send_mail::async_send_mail;
use crate::{
    auth::compose_temp_code_mail,
    model::{PartialUser, Settable, User},
};
use actix::prelude::*;
use actix_redis::{Command, RedisActor};
use actix_web::{Error as AWError, HttpRequest, HttpResponse, Responder, web};
use futures::future::{join, join_all};
use redis_async::{resp::RespValue, resp_array};

/// gets a list of users from a list of ids
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

    let mut users: Vec<User> = Vec::new();

    if !res.iter().all(|res| match res {
        Ok(RespValue::BulkString(x)) => {
            users.push(serde_json::from_slice(x).unwrap());
            true
        }
        _ => false,
    }) {
        Ok(HttpResponse::InternalServerError().finish())
    } else {
        Ok(HttpResponse::Ok().json(users))
    }
}

pub async fn get(
    redis: web::Data<Addr<RedisActor>>,
    user_id: web::Path<String>,
) -> Result<HttpResponse, AWError> {
    let user_id = user_id.into_inner();

    let domain = format!("user:{}", user_id);

    let res = redis.send(Command(resp_array!["GET", &domain])).await?;

    match res {
        Ok(RespValue::BulkString(x)) => Ok(HttpResponse::Ok().body(x)),
        _ => Ok(HttpResponse::InternalServerError().finish()),
    }
}

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

    let mut user: User = p_user.into();
    user.set_temp_code(&temp_code);

    let email = compose_temp_code_mail(&user, &email, &temp_code);
    let send = async_send_mail(email);
    let set = redis.send(Command(resp_array!["SET", &user.domain(), &user.json()]));
    let append = redis.send(Command(resp_array!["SADD", "users", &user.id]));

    // TODO: revise this... I don't think I'm understanding this
    let ((res, _), _) = join(join(set, append), send).await;

    match res? {
        Ok(RespValue::SimpleString(x)) if x == "OK" => Ok(HttpResponse::Ok().body("email was sent with temp code")),
        _ => Ok(HttpResponse::InternalServerError().finish()),
    }
}

pub async fn verify_temp_code(
    redis: web::Data<Addr<RedisActor>>,
    path: web::Path<(String, String)>, // user_id and token
) -> Result<HttpResponse, AWError> {
    let (user_id, temp_code) = path.into_inner();

    let domain = format!("user:{}", user_id);

    let get = redis.send(Command(resp_array!["GET", &domain])).await;

    let mut user: User = match get? {
        Ok(RespValue::BulkString(x)) => {
            serde_json::from_slice(&x).expect("user should be deserializable")
        }
        _ => return Ok(HttpResponse::InternalServerError().finish()),
    };

    if user.match_temp_code(&temp_code) {
        let access_token = generate_access_token(&user);
        user.set_access_token(access_token);

        let set = redis.send(Command(resp_array!["set", &domain, user.json()])).await?;

        match set {
            Ok(RespValue::SimpleString(x)) if x == "OK" => Ok(HttpResponse::Ok().json(user)),
            _=> Ok(HttpResponse::Unauthorized().body("unable to set access token"))
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
) -> Result<HttpResponse, AWError> {
    let user_id = user_id.into_inner();

    let domain = format!("user:{}", user_id);
    let del = redis.send(Command(resp_array!["DEL", &domain]));
    let pop = redis.send(Command(resp_array!["SREM", "users", &user_id]));

    let (res, _) = join(del, pop).await;

    match res? {
        Ok(RespValue::Integer(x)) if x == 1 => Ok(HttpResponse::Ok().body("deleted user")),
        _ => Ok(HttpResponse::InternalServerError().finish()),
    }
}
