mod auth;
mod handlers;
mod model;
mod send_mail;

use actix_cors::Cors;
use actix_redis::RedisActor;
use actix_web::{middleware, web, App, HttpServer};
use dotenv;
use std::env;

use handlers::*;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv::dotenv().ok();
    std::env::set_var(
        "RUST_LOG",
        "actix_web=trace,actix_redis=trace,ornot-server=trace",
    );
    env_logger::init();

    HttpServer::new(|| {
        let address = format!(
            "{}:{}",
            env::var("REDIS_ADDR").unwrap(),
            env::var("REDIS_PORT").unwrap()
        );

        let redis_addr = RedisActor::start(&address);

        // TODO: change this
        let cors = Cors::permissive();

        App::new()
            .data(redis_addr)
            .wrap(middleware::Logger::default())
            .wrap(cors)
            // user
            .service(
                web::resource("/api/v1/users").route(web::post().to(user::from_ids)), // .route(web::get().to(list_users))
            )
            .service(web::resource("/api/v1/user/signup")
                .route(web::post().to(user::sign_up)))
            .service(web::resource("/api/v1/user/force_add")
                .route(web::post().to(user::force_add)))
            .service(web::resource("/api/v1/user/{user_id}/code/{temp_code}")
                .route(web::get().to(user::verify_temp_code)))
            .service(web::resource("/api/v1/user/{user_id}/check")
                .route(web::get().to(user::verify_auth_code)))
            .service(
                web::resource("/api/v1/user/{user_id}")
                    .route(web::get().to(user::get))
                    .route(web::delete().to(user::delete)),
            )
            // topic
            .service(web::resource("/api/v1/topics").route(web::get().to(topic::list)))
            .service(web::resource("/api/v1/topic").route(web::put().to(topic::put)))
            .service(
                web::resource("/api/v1/topic/{topic_id}")
                    .route(web::get().to(topic::get))
                    .route(web::delete().to(topic::delete)),
            )
            .service(
                web::resource("api/v1/topic/{topic_id}/plan/{text}")
                    .route(web::delete().to(topic::remove_plan))
                    .route(web::post().to(topic::add_plan)),
            )
            .service(
                web::resource("api/v1/topic/{topic_id}/vote/{user_id}")
                    .route(web::put().to(topic::update_vote_and_calculate)),
            )
            .service(
                web::resource("api/v1/topic/{topic_id}/user/{user_id}")
                    .route(web::post().to(topic::add_user))
                    .route(web::delete().to(topic::remove_user)),
            )
            // helper
            .service(web::resource("api/v1/nuclear").route(web::delete().to(nuclear)))
    })
    .bind("0.0.0.0:8080")?
    .run()
    .await
}
