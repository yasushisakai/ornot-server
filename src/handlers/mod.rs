pub mod topic;
pub mod user;

use std::collections::HashSet;
use actix::Addr;
use actix_redis::{Command, RedisActor};
use actix_web::{web, HttpResponse, Error as AWError, Responder};
use redis_async::{resp::RespValue, resp_array};
use futures::future::join_all;
use crate::model::{Topic, User};
use liq::Setting;

pub async fn nuclear(redis: web::Data<Addr<RedisActor>>) -> Result<impl Responder, AWError> {
    let res = redis.send(Command(resp_array!["FLUSHALL",])).await?;

    match res {
        Ok(RespValue::SimpleString(x)) if x == "OK" => Ok("ok"),
        _ => Ok("nope"),
    }
}


pub async fn get_setting(
    redis: web::Data<Addr<RedisActor>>,
    setting_id: web::Path<String>
    ) -> Result<HttpResponse, AWError> {

    let setting_id = setting_id.into_inner();
    let domain = format!("setting:{}", setting_id);

    let res = redis.send(Command(resp_array![
            "get",
            &domain
    ])).await?;

    match res {
        Ok(RespValue::BulkString(x)) => 
            Ok(HttpResponse::Ok().body(x)),
        _ => Ok(HttpResponse::NoContent().finish())
    }

}

pub async fn calculate_setting(
    setting: web::Json<Setting>
    ) -> Result<HttpResponse, AWError> {

    let setting = setting.into_inner();
    let result = setting.calculate();

    Ok(HttpResponse::Ok().json(result))
}

pub async fn dump(
    redis: web::Data<Addr<RedisActor>>
    ) -> Result<HttpResponse, AWError> {

    // get all tables
    
    let res = redis.send(Command(resp_array![
        "SMEMBERS",
        "topics"
    ])).await?;

    let topic_ids: Vec<String> = match res {
        Ok(RespValue::Array(ids)) => {
            let mut temp: Vec<String> = Vec::new();
            for id in ids {
                if let RespValue::BulkString(v) = id {
                    let (topic_id, _): (String, String)  = serde_json::from_slice(&v).expect("list item should be Deserializeable");
                    temp.push(topic_id);
                } 
            }
            temp
        },
        _ => Vec::new()
    };
    // get each topics
    let res: Vec<Result<RespValue, AWError>> = join_all(topic_ids.iter().map(|id|{
        let domain = format!("topic:{}",&id);
        redis.send(Command(resp_array![
                "get",
                &domain,
        ]))
    })).await.into_iter()
    .map(|item|{
            item.map_err(AWError::from)
                .and_then(|res| res.map_err(AWError::from))
        }).collect();

    let mut topics:Vec<Topic> = Vec::new();
    let mut user_ids:HashSet<String> = HashSet::new();
    
    for r in res {
        if let Ok(RespValue::BulkString(x)) = r {
            let t:Topic = serde_json::from_slice(&x).expect("should be deserializable");
            // get all unique user ids
            user_ids = user_ids.union(&t.get_users()).map(String::from).collect();
            topics.push(t);
        }
    }
    
    // get each user from user id
    let mut users: Vec<User> = Vec::new();
    let res: Vec<Result<RespValue, AWError>> = join_all(user_ids.iter().map(|id|{
        let domain = format!("user:{}", &id);
        redis.send(Command(resp_array![
            "get",
            &domain,
        ]))
    })).await.into_iter()
    .map(|item|{
            item.map_err(AWError::from)
                .and_then(|res| res.map_err(AWError::from))
        }).collect();

    for r in res {
        if let Ok(RespValue::BulkString(x)) = r {
            let user: User = serde_json::from_slice(&x).expect("user json data should be DeSeriliazable");
            users.push(user); 
        }
    }

    Ok(HttpResponse::Ok().json((users, topics)))
}

type DumpFile = (Vec<User>, Vec<Topic>);

