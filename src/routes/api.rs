use axum::Json;

pub async fn say_hello() -> Json<&'static str> {
    Json("Hello")
}
