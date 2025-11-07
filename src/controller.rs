use crate::DB;
use loony_server::extract::Data;
use loony_server::response::HttpResponse;
use serde_json::json;

pub async fn get_user(_app: Data<DB>, _params: String) -> HttpResponse {
    HttpResponse{value: json!({ "id": 1, "name": "User" }).to_string()}
}

pub async fn users() -> HttpResponse {
    HttpResponse{value: json!([{ "id": 1, "name": "User" }]).to_string()}
}
