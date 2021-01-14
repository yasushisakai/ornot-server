use actix::prelude::*;
use actix_redis::{Command, RedisActor};
use actix_web::{web, HttpResponse, Error as AWError};
use redis_async::{resp::RespValue, resp_array};
use futures::future::{join, join_all};
use crate::model::{PartialUser, User, Settable};

/// gets a list of users from a list of ids
pub async fn from_ids(
    redis: web::Data<Addr<RedisActor>>,
    user_ids: web::Json<Vec<String>>
) -> Result<HttpResponse, AWError> {
    let user_ids = user_ids.into_inner();

    let commands = user_ids.iter().map(|id|{
        let domain = format!("user:{}", id);
        redis.send(Command(resp_array!["GET", &domain]))
    });

        let res: Vec<Result<RespValue, AWError>> =
        join_all(commands.into_iter())
            .await
            .into_iter()
            .map(|item| {
                item.map_err(AWError::from)
                    .and_then(|res| res.map_err(AWError::from))
            })
            .collect();

    let mut users:Vec<User> = Vec::new();

    if !res.iter().all(|res| match res {
        Ok(RespValue::BulkString(x)) => {
            users.push(serde_json::from_slice(x).unwrap());
            true},
        _ => false,
    }) {
        Ok(HttpResponse::InternalServerError().finish())
    } else {
        Ok(HttpResponse::Ok().json(users))
    }
}

pub async fn get(
    redis: web::Data<Addr<RedisActor>>,
    user_id: web::Path<String>
) -> Result<HttpResponse, AWError> {
    let user_id = user_id.into_inner();

    let domain = format!("user:{}", user_id);

    let res = redis.send(Command(resp_array![
        "GET",
        &domain
    ])).await?;

    match res {
        Ok(RespValue::BulkString(x)) => {
            Ok(HttpResponse::Ok().body(x))
        },
        _ => Ok(HttpResponse::InternalServerError().finish())
    }
}

pub async fn put(
    redis: web::Data<Addr<RedisActor>>,
    user: web::Json<PartialUser>
) -> Result<HttpResponse, AWError> {
    let user: User = user.into_inner().into(); 

    let user_json = serde_json::to_string(&user).unwrap();

    let set = redis.send(Command(resp_array![
        "SET",
        &user.domain(),
        &user_json
    ]));

    let append = redis.send(Command(resp_array![
        "SADD",
        "users",
        &user.id
    ]));

    let (res, _) = join(set, append).await;

    match res? {
        Ok(RespValue::SimpleString(x)) if x == "OK" => 
            Ok(HttpResponse::Ok().json(&user)),
        _ => Ok(HttpResponse::InternalServerError().finish())
    }
}

pub async fn delete(
    redis: web::Data<Addr<RedisActor>>,
    user_id: web::Path<String>
) -> Result<HttpResponse, AWError> {
    let user_id = user_id.into_inner();

    let domain = format!("user:{}", user_id);

    let del = redis.send(Command(resp_array![
        "DEL",
        &domain
    ]));

    let pop = redis.send(Command(resp_array![
        "SREM",
        "users",
        &user_id
    ]));

    let (res, _) = join(del,pop).await;

    match res? {
        Ok(RespValue::Integer(x))  if x == 1 => {
            Ok(HttpResponse::Ok().body("deleted user"))
        },
        _ => Ok(HttpResponse::InternalServerError().finish())
    }
}

