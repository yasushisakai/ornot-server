mod handlers;
mod model;

use std::env;
// use actix::prelude::*;
use actix_redis::RedisActor;
use actix_web::{middleware, web, App, HttpServer};
use dotenv;

use handlers::{
    post_user,
    get_user,
    post_topic,
    list_topics,
    get_topic,
    add_plan,
    delete_plan,
    add_user,
    insert_vote,
    delete_user,
    calculate_topic
};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv::dotenv().ok();
    std::env::set_var("RUST_LOG", "actix_web=trace,actix_redis=trace,ornot-server=trace");
    env_logger::init();

    HttpServer::new(|| {
        let address = format!("{}:{}", env::var("REDIS_ADDR").unwrap(), env::var("REDIS_PORT").unwrap());
        let redis_addr = RedisActor::start(&address);

        App::new()
            .data(redis_addr)
            .wrap(middleware::Logger::default())
            .service(
                web::resource("/api/v1/user")
                    .route(web::post().to(post_user)))
            .service(
                web::resource("/api/v1/user/{user_id}")
                    .route(web::get().to(get_user)))
            .service(
                web::resource("/api/v1/topic")
                    .route(web::post().to(post_topic)))
            .service(
                web::resource("/api/v1/topics")
                    .route(web::get().to(list_topics)))
            .service(
                web::resource("/api/v1/topic/{topic_id}")
                    .route(web::get().to(get_topic)))
            .service(
                web::resource("api/v1/topic/{topic_id}/add_plan/{text}")
                    .route(web::post().to(add_plan)))
            .service(
                web::resource("api/v1/topic/{topic_id}/update_vote/{user_id}")
                    .route(web::post().to(insert_vote)))
            .service(
                web::resource("api/v1/topic/{topic_id}/delete_plan/{text}")
                    .route(web::delete().to(delete_plan)))
            .service(
                web::resource("api/v1/topic/{topic_id}/add_user/{user_id}")
                    .route(web::post().to(add_user)))
            .service(
                web::resource("api/v1/topic/{topic_id}/delete_user/{user_id}")
                    .route(web::delete().to(delete_user)))
            .service(
                web::resource("api/v1/topic/{topic_id}/calculate")
                    .route(web::get().to(calculate_topic)))

    })
    .bind("0.0.0.0:8080")?
    .run()
    .await
}
