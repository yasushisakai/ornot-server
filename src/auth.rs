// 1. first an unregistered user tries to sign-up
//      this should be a POST request -> sign-up
//
// 2. the server sends an email to the user with a temp-code
//
// 3. User uses that link to authenticate
//      this should be a GET request -> temp
//
// 4. server will verify that temp-code and if true, issues an access-token
//
// 5. User client saves that in localStorage
//      authentification BEARER header?

use crate::model::{PartialUser, User};
use crate::send_mail::Email;
use actix::Addr;
use actix_redis::{Command, RedisActor, RespValue};
use redis_async::resp_array;
use actix_web::{web, http, Error};
use sha2::{Digest, Sha256};

pub fn generate_temp_code(p_user: &PartialUser) -> String {
    let salt = "temp_code"; //fixme: salt value is exposed
    let salted = format!("{}{}{}", salt, p_user.nickname, p_user.email);
    format!("{:x}", Sha256::digest(salted.as_bytes()))
}

pub fn generate_access_token(user: &User) -> String {
    let salt = "access_token"; //fixme: salt value is exposed
    let user_code = user.get_temp_code().expect("user should have a temp code by now");
    let salted = format!("{}{}", salt, user_code);
    format!("{:x}", Sha256::digest(salted.as_bytes()))
}

pub fn compose_temp_code_mail(user: &User, email: &str, code: &str) -> Email {
    let body = format!(
        "Hi {}, this is a tiny note to let you know your temp code. \n
use the below url to verify that you own this email address. \n
https://ornot.vote/auth/{}/{}\n
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
    ])).await;

    let user: User = match get {
        Ok(Ok(RespValue::BulkString(x))) => serde_json::from_slice(&x).expect("data shoud be deserializable"),
        _=> {return Ok(false)}
    };

    let header = match header.get("Authorization") {
        Some(h) => h.to_str(),
        None => {return Ok(false)}
    };

    let bearer = header.unwrap().to_string();
    let token: Option<&str> = bearer.split_whitespace().nth(1);
    let access_token: Option<&str> = user.get_access_token().map(|t|t.as_str());
    
    Ok(token == access_token)
}

