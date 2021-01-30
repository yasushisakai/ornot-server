pub mod topic;
pub mod user;

use crate::{
    model::{Topic, User},
    redis_helper::{redis_add, redis_get_list, redis_get_slices},
};
use actix::Addr;
use actix_redis::{Command, RedisActor};
use actix_web::{web, Error as AWError, HttpResponse, Responder};
use futures::future::{join, join_all};
use liq::Setting;
use redis_async::{resp::RespValue, resp_array};
use std::collections::HashSet;

pub async fn nuclear(redis: web::Data<Addr<RedisActor>>) -> Result<impl Responder, AWError> {
    let res = redis.send(Command(resp_array!["FLUSHALL",])).await?;

    match res {
        Ok(RespValue::SimpleString(x)) if x == "OK" => Ok("ok"),
        _ => Ok("nope"),
    }
}

pub async fn get_setting(
    redis: web::Data<Addr<RedisActor>>,
    setting_id: web::Path<String>,
) -> Result<HttpResponse, AWError> {
    let setting_id = setting_id.into_inner();
    let domain = format!("setting:{}", setting_id);

    let res = redis.send(Command(resp_array!["get", &domain])).await?;

    match res {
        Ok(RespValue::BulkString(x)) => Ok(HttpResponse::Ok().body(x)),
        _ => Ok(HttpResponse::NoContent().finish()),
    }
}

pub async fn calculate_setting(setting: web::Json<Setting>) -> Result<HttpResponse, AWError> {
    let setting = setting.into_inner();
    let result = setting.calculate();

    Ok(HttpResponse::Ok().json(result))
}

pub async fn dump(redis: web::Data<Addr<RedisActor>>) -> Result<HttpResponse, AWError> {
    // get all tables

    let topics_cmd = redis_get_list("topic", &redis);
    let users_cmd = redis_get_list("user", &redis);

    let (topic_list_items, user_list_items) = join(topics_cmd, users_cmd).await;

    let topic_ids: Vec<String> = match topic_list_items{
        Some(x) => x.iter().map(|li| li.0.to_string()).collect(),
        None => {
            return Ok(HttpResponse::InternalServerError().body("failed to get topic_ids"))
    }};
        
    let user_ids: Vec<String> = match user_list_items{
        Some(x) => x.iter().map(|li| li.0.to_string()).collect(),
        None => {
            return Ok(HttpResponse::InternalServerError().body("failed to get user_ids"))
    }};

    let topics_cmd = redis_get_slices(&topic_ids, "topic", &redis);
    let users_cmd = redis_get_slices(&user_ids, "user", &redis);

    let (topic_slices, user_slices) = join(topics_cmd, users_cmd).await;

    let topics: Vec<Topic> = topic_slices
        .iter()
        .map(|t| serde_json::from_slice(t)
            .expect("this slice should be Deserialized to Topic"))
        .collect();

    let users: Vec<User> = user_slices
        .iter()
        .map(|u| serde_json::from_slice(u)
            .expect("this slice should be Deserialized to User"))
        .collect();

    Ok(HttpResponse::Ok().json((users, topics)))
}

type DumpFile = (Vec<User>, Vec<Topic>);

pub async fn restore(
    redis: web::Data<Addr<RedisActor>>,
    dump: web::Json<DumpFile>,
) -> Result<HttpResponse, AWError> {
    let (users, topics) = dump.into_inner();

    let user_add = join_all(users.into_iter().map(|u| redis_add(u, &redis)));
    let topic_add = join_all(topics.into_iter().map(|t| redis_add(t, &redis)));

    let (_users, _topics) = join(user_add, topic_add).await;

    Ok(HttpResponse::Ok().body("success"))
}
