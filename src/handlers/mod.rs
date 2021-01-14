pub mod user;
pub mod topic;

use std::collections::HashMap;
use actix::prelude::*;
use actix_redis::{Command, RedisActor};
use actix_web::{web, HttpResponse, Responder, Error as AWError};
use redis_async::{resp::RespValue, resp_array};
use futures::future::join;
use crate::model::{PartialTopic, Topic, Settable};

pub async fn nuclear(
    redis: web::Data<Addr<RedisActor>>
    )-> Result<impl Responder, AWError> {

    let res = redis.send(Command(resp_array![
        "FLUSHALL",
    ])).await?;

    match res {
        Ok(RespValue::SimpleString(x)) if x == "OK" => 
            Ok("ok"),
        _=> Ok("nope")
    }
}
