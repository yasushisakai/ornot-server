use crate::{
    model::{PartialTopic, Settable, Topic, SimplePlan},
    redis_helper::{redis_add, redis_delete, redis_get_slice},
};
use actix::prelude::*;
use actix_redis::{Command, RedisActor};
use actix_web::{web, Error as AWError, HttpResponse};
use futures::future::join;
use redis_async::{resp::RespValue, resp_array};
use std::collections::BTreeMap;

pub async fn get(
    redis: web::Data<Addr<RedisActor>>,
    topic_id: web::Path<String>,
) -> Result<HttpResponse, AWError> {
    let topic_id = topic_id.into_inner();

    let slice = redis_get_slice(&topic_id, "topic", &redis).await;

    match slice {
        Some(x) => {
            let topic: Topic =
                serde_json::from_slice(&x).expect("this slice should be able to Deserilaze");
            Ok(HttpResponse::Ok().json(&topic))
        }
        None => Ok(HttpResponse::NoContent().finish()),
    }
}

pub async fn delete(
    redis: web::Data<Addr<RedisActor>>,
    topic_id: web::Path<String>,
) -> Result<HttpResponse, AWError> {
    let topic_id = topic_id.into_inner();

    let data = redis_get_slice(&topic_id, "topic", &redis).await;

    let topic: Topic = match data {
        Some(x) => serde_json::from_slice(&x).unwrap(),
        None => return Ok(HttpResponse::NoContent().finish()),
    };

    let did_delete = redis_delete(topic, &redis).await;

    match did_delete {
        true => Ok(HttpResponse::Ok().body("deleted topic")),
        false => Ok(HttpResponse::InternalServerError().finish()),
    }
}

pub async fn list(redis: web::Data<Addr<RedisActor>>) -> Result<HttpResponse, AWError> {
    let res = redis
        .send(Command(resp_array!["SMEMBERS", "topics",]))
        .await?; // get all

    if let Ok(RespValue::Array(ids)) = res {
        let mut list: Vec<Vec<String>> = Vec::new();
        for id in ids {
            if let RespValue::BulkString(v) = id {
                let (topic_id, topic_title) = serde_json::from_slice(&v).unwrap();
                list.push(vec![topic_id, topic_title]);
            }
        }
        Ok(HttpResponse::Ok().json(list))
    } else {
        Ok(HttpResponse::InternalServerError().finish())
    }
}

pub async fn put(
    redis: web::Data<Addr<RedisActor>>,
    topic: web::Json<PartialTopic>,
) -> Result<HttpResponse, AWError> {
    let topic: Topic = topic.into_inner().into();

    match redis_add(topic, &redis).await {
        true => Ok(HttpResponse::Ok().body("added topic")),
        false => Ok(HttpResponse::InternalServerError().body("could not put topic")),
    }
}

pub async fn add_plan(
    redis: web::Data<Addr<RedisActor>>,
    path: web::Path<(String, String)>,
) -> Result<HttpResponse, AWError> {
    let (topic_id, text) = path.into_inner();

    let data = redis_get_slice(&topic_id, "topic", &redis).await;

    let mut topic: Topic = match data {
        Some(x) => serde_json::from_slice(&x).expect("slice should be Deserilazable"),
        None => return Ok(HttpResponse::NoContent().body("could not retrieve topic")),
    };

    // add the plan
    topic.add_plan(SimplePlan(text));

    // save the new topic data
    let res = redis
        .send(Command(resp_array!["SET", &topic.domain(), &topic.json()]))
        .await?;

    // respond
    match res {
        Ok(RespValue::SimpleString(x)) if x == "OK" => Ok(HttpResponse::Ok().json(topic)),
        _ => Ok(HttpResponse::InternalServerError().finish()),
    }
}

type Vote = BTreeMap<String, f64>;

pub async fn update_vote_and_calculate(
    redis: web::Data<Addr<RedisActor>>,
    path: web::Path<(String, String)>,
    vote: web::Json<Vote>,
) -> Result<HttpResponse, AWError> {
    let (topic_id, user_id) = path.into_inner();

    let data = redis_get_slice(&topic_id, "topic", &redis).await;

    let mut topic: Topic = match data {
        Some(x) => serde_json::from_slice(&x).expect("slice should be Deserilazable"),
        _ => {
            return Ok(HttpResponse::InternalServerError().finish());
        }
    };

    let vote = vote.into_inner();

    let new_hash = topic.insert_vote(&user_id, vote);
    // let's just compute it each time for now.

    if new_hash != topic.setting_hash {
        topic.update_setting_hash(&new_hash);
        topic.calculate();
        let set_topic = redis.send(Command(resp_array!["SET", &topic.domain(), &topic.json()]));

        let setting_domain = format!("setting:{}", topic.setting_hash);
        let set_setting = redis.send(Command(resp_array![
            "SET",
            setting_domain,
            topic.setting_json()
        ]));

        let (res, _) = join(set_topic, set_setting).await;

        match res? {
            Ok(RespValue::SimpleString(x)) if x == "OK" => Ok(HttpResponse::Ok().json(topic)),
            _ => Ok(HttpResponse::InternalServerError().body("cannot save new topic")),
        }
    } else {
        // no change
        Ok(HttpResponse::Ok().json("no change"))
    }
}

pub async fn remove_plan(
    redis: web::Data<Addr<RedisActor>>,
    path: web::Path<(String, String)>,
) -> Result<HttpResponse, AWError> {
    let (topic_id, text) = path.into_inner();

    // get the topic data
    let data = redis_get_slice(&topic_id, "topic", &redis).await;

    let mut topic: Topic = match data {
        Some(x) => serde_json::from_slice(&x).expect("sould Deserilazable"),
        None => return Ok(HttpResponse::NoContent().finish()),
    };

    topic.remove_plan(SimplePlan(text));

    // save the new topic data
    let res = redis
        .send(Command(resp_array!["SET", &topic.domain(), &topic.json()]))
        .await?;

    // respond
    match res {
        Ok(RespValue::SimpleString(x)) if x == "OK" => Ok(HttpResponse::Ok().json(topic)),
        _ => Ok(HttpResponse::InternalServerError().finish()),
    }
}

pub async fn add_user(
    redis: web::Data<Addr<RedisActor>>,
    path: web::Path<(String, String)>,
) -> Result<HttpResponse, AWError> {
    let (topic_id, user_id) = path.into_inner();

    let data = redis_get_slice(&topic_id, "topic", &redis).await;

    let mut topic: Topic = match data {
        Some(x) => serde_json::from_slice(&x).unwrap(),
        None => return Ok(HttpResponse::InternalServerError().finish()),
    };

    // add the plan
    topic.add_user(user_id);

    // save the new topic data
    let res = redis
        .send(Command(resp_array!["SET", &topic.domain(), &topic.json()]))
        .await?;

    // respond
    match res {
        Ok(RespValue::SimpleString(x)) if x == "OK" => Ok(HttpResponse::Ok().json(topic)),
        _ => Ok(HttpResponse::InternalServerError().finish()),
    }
}

pub async fn remove_user(
    redis: web::Data<Addr<RedisActor>>,
    path: web::Path<(String, String)>,
) -> Result<HttpResponse, AWError> {
    let (topic_id, user_id) = path.into_inner();

    let data = redis_get_slice(&topic_id, "topic", &redis).await;

    let mut topic: Topic = match data {
        Some(x) => serde_json::from_slice(&x).unwrap(),
        _ => return Ok(HttpResponse::InternalServerError().finish()),
    };

    topic.remove_user(user_id);

    // save the new topic data
    let res = redis
        .send(Command(resp_array!["SET", &topic.domain(), &topic.json()]))
        .await?;

    // respond
    match res {
        Ok(RespValue::SimpleString(x)) if x == "OK" => Ok(HttpResponse::Ok().json(topic)),
        _ => Ok(HttpResponse::InternalServerError().finish()),
    }
}
