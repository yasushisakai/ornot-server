use crate::{
    model::{RawPlan, Plan, Settable},
    redis_helper::{redis_add, redis_get_slice},
};
use actix::prelude::*;
use actix_redis::RedisActor;
use actix_web::{web, Error as AWError, HttpResponse};

pub async fn get(
    redis: web::Data<Addr<RedisActor>>,
    plan_id: web::Path<String>,
) -> Result<HttpResponse, AWError> {
    let plan_id = plan_id.into_inner();

    let slice = redis_get_slice(&plan_id, "plan", &redis).await;

    match slice {
        Some(x) => {
            let plan: Plan =
                serde_json::from_slice(&x).expect("this slice should be able to Deserilaze");
            Ok(HttpResponse::Ok().json(&plan))
        }
        None => Ok(HttpResponse::NoContent().finish()),
    }
}

pub async fn put(
    redis: web::Data<Addr<RedisActor>>,
    raw_plan: web::Json<RawPlan>,
) -> Result<HttpResponse, AWError> {
    let plan: Plan = raw_plan.into_inner().into();
    let id = plan.id();
    match redis_add(plan, &redis).await {
        true => Ok(HttpResponse::Ok().json(id)),
        false => Ok(HttpResponse::InternalServerError().body("could not put plan")),
    }
}


