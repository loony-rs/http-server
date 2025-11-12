mod controller;
mod connection;

use loony_server::{
    App, HttpServer, responder::Responder, route, router::Router
};
use crate::{connection::pg_connection};
use deadpool_postgres::Pool;

async fn index() -> impl Responder {
    String::from("Hello World")
}

fn routes() -> Router {
    Router::new()
    .route(route::get("/").to(index))
    .service(
        route::scope("/user")
        .route(route::get("/all").to(controller::users))
        .route(route::get("/get/:user_id").to(controller::get_user))
        .route(route::get("/delete/:user_id").to(controller::get_user))
        .route(route::get("/update/:user_id").to(controller::get_user))
    )
}

#[derive(Debug, Clone)]
pub struct AppState {
    name: String,
}

#[derive(Clone)]
pub struct DB {
    pub session: Pool,
}

#[tokio::main]
async fn main() {

    let conn = pg_connection().await;

    let db = DB {
        session: conn,
    };
    
    HttpServer::new(move ||
        App::new()
        .app_data( AppState {
            name: "loony".to_owned(),
        })
        .data(db.clone())
        .routes(routes)
    )
    .bind(2000)
    .run().await;

}   