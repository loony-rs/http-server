use crate::DB;
use loony_server::extract::{Data, Path};
use loony_server::response::HttpResponse;
use serde_json::json;

pub async fn get_user(_app: Data<DB>, Path(user_id): Path<i32>) -> HttpResponse {
    // HttpResponse{value: json!({ "id": 1, "name": "User" }).to_string()}
    HttpResponse::new().json(json!({ "id": user_id })).unwrap()
}

pub async fn get_user_name(_app: Data<DB>, Path((user_id, name)): Path<(i32, String)>) -> HttpResponse {
    // HttpResponse{value: json!({ "id": 1, "name": "User" }).to_string()}
    HttpResponse::new().json(json!({ "id": user_id, "name": name })).unwrap()
}

pub async fn delete_user(_app: Data<DB>, Path(user_id): Path<i32>) -> HttpResponse {
    // HttpResponse{value: json!({ "id": 1, "name": "User" }).to_string()}
    HttpResponse::new().json(json!({ "id": 1, "name": "User" })).unwrap()
}

pub async fn update_user(_app: Data<DB>, Path(user_id): Path<i32>) -> HttpResponse {
    // HttpResponse{value: json!({ "id": 1, "name": "User" }).to_string()}
    HttpResponse::new().json(json!({ "id": 1, "name": "User" })).unwrap()
}

pub async fn users() -> HttpResponse {
    // HttpResponse{value: json!([{ "id": 1, "name": "User" }]).to_string()}
    HttpResponse::new().json(json!({ "id": 1, "name": "User" })).unwrap()
}
