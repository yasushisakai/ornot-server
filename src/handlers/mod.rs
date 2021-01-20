pub mod topic;
pub mod user;

use actix::Addr;
use actix_redis::{Command, RedisActor};
use actix_web::{web, HttpResponse, Error as AWError, Responder};
use redis_async::{resp::RespValue, resp_array};
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
