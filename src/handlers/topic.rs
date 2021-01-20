use std::collections::BTreeMap;
use actix::prelude::*;
use actix_redis::{Command, RedisActor};
use actix_web::{web, HttpResponse, Error as AWError};
use redis_async::{resp::RespValue, resp_array};
use futures::future::join;
use crate::model::{PartialTopic, Topic, Settable};

pub async fn get(
    redis: web::Data<Addr<RedisActor>>,
    topic_id: web::Path<String>
) -> Result<HttpResponse, AWError> {
    let topic_id = topic_id.into_inner();

    let domain = format!("topic:{}", topic_id);

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

pub async fn delete(
    redis: web::Data<Addr<RedisActor>>,
    topic_id: web::Path<String>
) -> Result<HttpResponse, AWError> {
    let topic_id = topic_id.into_inner();

    let domain = format!("topic:{}", topic_id);

    // first I need to get the title
    let res = redis.send(Command(resp_array![
        "GET",
        &domain
    ])).await?;
    
    let topic: Topic = match &res {
        Ok(RespValue::BulkString(x)) => {
            serde_json::from_slice(x).unwrap()
        },
        _=> {
            return Ok(HttpResponse::InternalServerError().finish())
        }
    };

    let list_element = serde_json::to_string(&topic.list_item()).unwrap();

    let single_del = redis.send(Command(resp_array![
        "DEL",
        &domain
    ]));

    let from_list = redis.send(Command(resp_array![
        "SREM",
        "topics",
        list_element
    ]));

    let (res, _) = join(single_del, from_list).await;


    match res? {
        Ok(RespValue::Integer(x)) if x == 1 => {
            Ok(HttpResponse::Ok().body("ok"))
        },
        _ => Ok(HttpResponse::InternalServerError().finish())
    }
}

pub async fn list(
    redis: web::Data<Addr<RedisActor>>,
    ) -> Result<HttpResponse, AWError> {
    
    let res = redis.send(Command(resp_array![
        "SMEMBERS",
        "topics",
    ])).await?; // get all

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
    topic: web::Json<PartialTopic>
) -> Result<HttpResponse, AWError> {
    let topic: Topic = topic.into_inner().into(); 

    let topic_json = serde_json::to_string(&topic).unwrap();

    let set_topic = redis.send(Command(resp_array![
        "SET",
        &topic.domain(),
        &topic_json
    ]));

    let add_to_list = redis.send(Command(resp_array![
        "SADD",
        "topics",
        serde_json::to_string(&topic.list_item()).unwrap()
    ]));

    let (res, _) = join(set_topic, add_to_list).await;

    match res? {
        Ok(RespValue::SimpleString(x)) if x == "OK" => {
            Ok(HttpResponse::Ok().json(&topic))
        },
        _ => Ok(HttpResponse::InternalServerError().body("didn't work"))
    }
}


pub async fn add_plan(
    redis: web::Data<Addr<RedisActor>>,
    path: web::Path<(String, String)>
) -> Result<HttpResponse, AWError> {

    let (topic_id, text) = path.into_inner();

    // get the topic data
    let domain = format!("topic:{}", &topic_id);

    let res = redis.send(Command(resp_array![
        "GET",
        &domain
    ])).await?;

    let mut topic:Topic = match res {
        Ok(RespValue::BulkString(data)) => {
            serde_json::from_slice(&data).unwrap()
        },
        _ => {
            return Ok(HttpResponse::InternalServerError().finish())
        }
    };

    // add the plan
    topic.add_plan(text); 

    // save the new topic data
    let res = redis.send(Command(resp_array![
        "SET",
        &domain,
        &topic.json()
    ])).await?;

    // respond
    match res {
        Ok(RespValue::SimpleString(x)) if x == "OK" => 
            Ok(HttpResponse::Ok().json(topic)),
        _ => Ok(HttpResponse::InternalServerError().finish())
    }
}    

type Vote = BTreeMap<String, f64>;

pub async fn update_vote_and_calculate(
    redis: web::Data<Addr<RedisActor>>,
    path: web::Path<(String, String)>,
    vote: web::Json<Vote>
    )-> Result<HttpResponse, AWError>
{
    let (topic_id, user_id) = path.into_inner(); 
    let domain = format!("topic:{}", &topic_id);
    
    let res = redis.send(Command(resp_array![
        "GET",
        &domain
    ])).await?;

    let mut topic: Topic = match res {
        Ok(RespValue::BulkString(data)) => {
            serde_json::from_slice(&data).unwrap()
        },
        _ => {return Ok(HttpResponse::InternalServerError().finish());}
    };

    println!("{:?}", topic);

    let vote = vote.into_inner();

    let new_hash = topic.insert_vote(&user_id, vote);
    // let's just compute it each time for now.
    
    if new_hash != topic.setting_hash {
        topic.update_setting_hash(&new_hash);
        topic.calculate();
        let set_topic = redis.send(Command(resp_array![
            "SET",
            &topic.domain(),
            &topic.json()
        ])); 

        let setting_domain = format!("setting:{}", topic.setting_hash);
        let set_setting = redis.send(Command(resp_array![
            "SET",
            setting_domain, 
            topic.setting_json()
        ]));

        let (res, _) = join(set_topic, set_setting).await;
        
        match res? {
            Ok(RespValue::SimpleString(x)) if x == "OK" => {
                Ok(HttpResponse::Ok().json(topic))
            },
            _ => {
                Ok(HttpResponse::InternalServerError().body("cannot save new topic"))
            }
        }
    } else { // no change
       Ok(HttpResponse::Ok().json("no change"))
    }
}

pub async fn remove_plan(
    redis: web::Data<Addr<RedisActor>>,
    path: web::Path<(String, String)>
) -> Result<HttpResponse, AWError> {

    let (topic_id, text) = path.into_inner();

    // get the topic data
    let domain = format!("topic:{}", &topic_id);

    let res = redis.send(Command(resp_array![
        "GET",
        &domain
    ])).await?;

    let mut topic:Topic = match res {
        Ok(RespValue::BulkString(data)) => {
            serde_json::from_slice(&data).unwrap()
        },
        _ => {
            return Ok(HttpResponse::InternalServerError().finish())
        }
    };

    topic.remove_plan(text); 

    let new_topic_json = serde_json::to_string(&topic).unwrap();
    
    // save the new topic data
    let res = redis.send(Command(resp_array![
        "SET",
        &domain,
        &new_topic_json
    ])).await?;

    // respond
    match res {
        Ok(RespValue::SimpleString(x)) if x == "OK" => 
            Ok(HttpResponse::Ok().json(new_topic_json)),
        _ => Ok(HttpResponse::InternalServerError().finish())
    }
}

pub async fn add_user(
    redis: web::Data<Addr<RedisActor>>,
    path: web::Path<(String, String)>
) -> Result<HttpResponse, AWError> {

    let (topic_id, user_id) = path.into_inner();

    // get the topic data
    let domain = format!("topic:{}", &topic_id);

    let res = redis.send(Command(resp_array![
        "GET",
        &domain
    ])).await?;

    let mut topic:Topic = match res {
        Ok(RespValue::BulkString(data)) => {
            serde_json::from_slice(&data).unwrap()
        },
        _ => {
            return Ok(HttpResponse::InternalServerError().finish())
        }
    };

    // add the plan
    topic.add_user(user_id); 

    let new_topic_json = serde_json::to_string(&topic).unwrap();
    
    // save the new topic data
    let res = redis.send(Command(resp_array![
        "SET",
        &domain,
        &new_topic_json
    ])).await?;

    // respond
    match res {
        Ok(RespValue::SimpleString(x)) if x == "OK" => 
            Ok(HttpResponse::Ok().body("ok")),
        _ => Ok(HttpResponse::InternalServerError().finish())
    }
}

pub async fn remove_user(
    redis: web::Data<Addr<RedisActor>>,
    path: web::Path<(String, String)>
) -> Result<HttpResponse, AWError> {

    let (topic_id, user_id) = path.into_inner();

    // get the topic data
    let domain = format!("topic:{}", &topic_id);

    let res = redis.send(Command(resp_array![
        "GET",
        &domain
    ])).await?;

    let mut topic:Topic = match res {
        Ok(RespValue::BulkString(data)) => {
            serde_json::from_slice(&data).unwrap()
        },
        _ => {
            return Ok(HttpResponse::InternalServerError().finish())
        }
    };

    topic.remove_user(user_id); 

    let new_topic_json = serde_json::to_string(&topic).unwrap();
    
    // save the new topic data
    let res = redis.send(Command(resp_array![
        "SET",
        &domain,
        &new_topic_json
    ])).await?;

    // respond
    match res {
        Ok(RespValue::SimpleString(x)) if x == "OK" => 
            Ok(HttpResponse::Ok().json(new_topic_json)),
        _ => Ok(HttpResponse::InternalServerError().finish())
    }
}

