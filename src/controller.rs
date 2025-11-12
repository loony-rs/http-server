use crate::DB;
use loony_server::extract::{Data, Path};
use loony_server::response::HttpResponse;
use serde_json::json;

pub async fn get_user(_app: Data<DB>, Path(user_id, name): Path) -> HttpResponse {
    // HttpResponse{value: json!({ "id": 1, "name": "User" }).to_string()}
    HttpResponse::new().json(json!({ "id": 1, "name": "User" })).unwrap()
}

pub async fn users() -> HttpResponse {
    // HttpResponse{value: json!([{ "id": 1, "name": "User" }]).to_string()}
    HttpResponse::new().json(json!({ "id": 1, "name": "User" })).unwrap()
}
