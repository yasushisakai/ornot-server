use std::collections::HashMap;
use actix::prelude::*;
use actix_redis::{Command, RedisActor};
use actix_web::{web, HttpResponse, Error as AWError};
use redis_async::{resp::RespValue, resp_array};
use futures::future::{join, join_all};
use crate::model::{PartialTopic, Topic, PartialUser, User, Settable};

pub async fn post_user(
    redis: web::Data<Addr<RedisActor>>,
    user: web::Json<PartialUser>
) -> Result<HttpResponse, AWError> {
    let user: User = user.into_inner().into(); 

    let user_json = serde_json::to_string(&user).unwrap();

    let res = redis.send(Command(resp_array![
        "SET",
        &user.domain(),
        &user_json
    ])).await?;

    match res {
        Ok(RespValue::SimpleString(x)) if x == "OK" => 
            Ok(HttpResponse::Ok().json(&user)),
        _ => Ok(HttpResponse::InternalServerError().finish())
    }
}

pub async fn get_user(
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

pub async fn get_users(
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

pub async fn get_topic(
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

pub async fn list_topics(
    redis: web::Data<Addr<RedisActor>>,
    ) -> Result<HttpResponse, AWError> {
    
    let res = redis.send(Command(resp_array![
        "LRANGE",
        "topics",
        "0",
        "-1"
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

pub async fn post_topic(
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

    let topic_list = (&topic.id, &topic.title);

    let add_to_list = redis.send(Command(resp_array![
        "LPUSH",
        "topics",
        serde_json::to_string(&topic_list).unwrap()
    ]));

    let (res, _) = join(set_topic, add_to_list).await;

    match res? {
        Ok(RespValue::SimpleString(x)) if x == "OK" => {
            Ok(HttpResponse::Ok().body(&topic.id))
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

type Vote = HashMap<String, f64>;

pub async fn insert_vote(
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
        Ok(RespValue::BulkString(data)) => serde_json::from_slice(&data).unwrap(),
        _ => {return Ok(HttpResponse::InternalServerError().finish());}
    };

    let vote = vote.into_inner();

    topic.insert_vote(user_id, vote);

    let res = redis.send(Command(resp_array![
        "SET",
        &domain,
        serde_json::to_string(&topic).unwrap()
    ])).await?;
    
    match res {
        Ok(RespValue::SimpleString(x)) if x == "OK" => {
            Ok(HttpResponse::Ok().body("updated topic"))},
        _ => Ok(HttpResponse::InternalServerError().finish())
    }
}

pub async fn delete_plan(
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

    topic.delete_plan(text); 

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
            Ok(HttpResponse::Ok().json(new_topic_json)),
        _ => Ok(HttpResponse::InternalServerError().finish())
    }
}


pub async fn delete_user(
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

    topic.delete_user(user_id); 

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


pub async fn calculate_topic(
    redis: web::Data<Addr<RedisActor>>,
    path: web::Path<String>,
    ) -> Result<HttpResponse, AWError> {

    let topic_id = path.into_inner();
    let domain = format!("topic:{}", &topic_id);

    let res = redis.send(Command(resp_array![
        "GET",
        &domain
    ])).await?;

    let mut topic:Topic = match res {
        Ok(RespValue::BulkString(data)) => serde_json::from_slice(&data).unwrap(),
        _ => {return Ok(HttpResponse::InternalServerError().finish());}
    };

    topic.calculate_result();

    let res = redis.send(Command(resp_array![
        "SET",
        &domain,
        serde_json::to_string(&topic).unwrap()
    ])).await?;

    match res {
        Ok(RespValue::SimpleString(x)) if x == "OK" => {},
        _ => {return Ok(HttpResponse::InternalServerError().finish());}
    }

    match topic.result {
        Some(result) => Ok(HttpResponse::Ok().json(result)),
        None => Ok(HttpResponse::InternalServerError().finish())
    }
}

