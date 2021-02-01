pub mod topic;
pub mod user;
pub mod plan;

use crate::{
    model::{Topic, User, Plan, RawPlan},
    redis_helper::{redis_add, redis_get_list, redis_get_slices},
};
use actix::Addr;
use actix_redis::{Command, RedisActor};
use actix_web::{web, Error as AWError, HttpResponse, Responder};
use futures::future::{join, join_all};
use liq::Setting;
use redis_async::{resp::RespValue, resp_array};
use serde::{Serialize, Deserialize};

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
    let plan_cmd = redis_get_list("plan", &redis);

    let (topic_list_items, (user_list_items, plan_list_items)) = join(topics_cmd, join(users_cmd, plan_cmd)).await;

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

    let plan_ids: Vec<String> = match plan_list_items{
        Some(x) => x.iter().map(|li| li.0.to_string()).collect(),
        None => {
            return Ok(HttpResponse::InternalServerError().body("failed to get user_ids"))
    }};

    let topics_cmd = redis_get_slices(&topic_ids, "topic", &redis);
    let users_cmd = redis_get_slices(&user_ids, "user", &redis);
    let plan_cmd = redis_get_slices(&plan_ids, "plan", &redis);

    let (topic_slices, (user_slices, plan_slices)) = join(topics_cmd, join(users_cmd, plan_cmd)).await;

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

    let mut plans: Vec<RawPlanWrapper> = Vec::new();

    for (i, slice) in plan_slices.iter().enumerate() {
        let id: String = plan_ids[i].to_string();
        let raw: RawPlan = serde_json::from_slice(&slice)
            .expect("slice should be Deserialized to RawPlan");
        let wrapper = RawPlanWrapper{id, raw};
       plans.push(wrapper) 
    }

    Ok(HttpResponse::Ok().json((users, topics, plans)))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RawPlanWrapper {
    id: String,
    raw: RawPlan
}

type DumpFile = (Vec<User>, Vec<Topic>, Vec<RawPlanWrapper>);

pub async fn restore(
    redis: web::Data<Addr<RedisActor>>,
    dump: web::Json<DumpFile>,
) -> Result<HttpResponse, AWError> {
    let (users, topics, plans) = dump.into_inner();

    let user_add = join_all(users.into_iter().map(|u| redis_add(u, &redis)));
    let topic_add = join_all(topics.into_iter().map(|t| redis_add(t, &redis)));

    let plan_add = join_all(plans.into_iter().map(|w| {
        let plan: Plan = w.raw.into();
        redis_add(plan, &redis)
    }));

    let _all = join(user_add, join(topic_add, plan_add)).await;

    Ok(HttpResponse::Ok().body("success"))
}
