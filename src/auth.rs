use crate::model::{PartialUser, User};
use crate::send_mail::Email;
use actix::Addr;
use actix_redis::{Command, RedisActor, RespValue};
use futures::future::join;
use redis_async::resp_array;
use actix_web::{web, http, Error};
use sha2::{Digest, Sha256};
use bs58::encode;
use dotenv::dotenv;

pub fn generate_temp_code(p_user: &PartialUser) -> String {
    dotenv().ok();
    let salt = std::env::var("SALT_TEMP_CODE").expect("env var 'SALT_TEMP_CODE missing'"); //fixme: salt value is exposed
    let salted = format!("{}{}{}", salt, p_user.nickname, p_user.email);
    encode(format!("{:x}", Sha256::digest(salted.as_bytes()))).into_string()
}

pub fn generate_access_token(user: &User) -> String {
    dotenv().ok();
    let salt = std::env::var("SALT_ACCESS_TOKEN").expect("evn var 'SALT_ACCESS_TOKEN missing'");
    let salted = format!("{}{}", salt, user.id);
    encode(format!("{:x}", Sha256::digest(salted.as_bytes()))).into_string()
}

pub fn compose_temp_code_mail(user: &User, email: &str, code: &str) -> Email {
    let body = format!(
        "Hi {}, this is a tiny note to let you know your temp code. \n
use the below url to verify that you own this email address. \n
https://ornot.vote/auth/?i={}&c={}\n
bye and have a nice day :)
",
        user.nickname, user.id, code,
    );

    Email {
        to_name: user.nickname.to_string(),
        to_address: email.to_string(),
        subject: "your temp code".to_string(),
        body,
    }
}

pub async fn check_auth(
    redis: &web::Data<Addr<RedisActor>>,
    user_id: &str,
    header: &http::header::HeaderMap
    ) -> Result<bool, Error> {

    let domain = format!("user:{}", user_id);

    let get = redis.send(Command(resp_array![
    "GET", 
    &domain
    ]));

    let header = match header.get("Authorization") {
        Some(h) => h.to_str(),
        None => {return Ok(false)}
    };

    let bearer = header.unwrap().to_string();
    let token: &str = match bearer.split_whitespace().nth(1) {
        Some(t) => t,
        None => {return Ok(false)}
    }; 

    let token_domain = format!("access_token:{}", &token);
    let get_token = redis.send(Command(resp_array!["GET", &token_domain]));

    let (user, uid) = join(get, get_token).await;

    let user: User = match user? {
        Ok(RespValue::BulkString(x)) => serde_json::from_slice(&x).expect("data shoud be deserializable"),
        _=> {return Ok(false)}
    };

    let uid: String = match uid? {
        Ok(RespValue::BulkString(x)) => String::from_utf8(x).unwrap(),
        _=> return Ok(false)
    };
    
    Ok(user.id == uid)
}

