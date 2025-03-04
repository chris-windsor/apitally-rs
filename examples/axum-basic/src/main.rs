use std::{env, net::SocketAddr};

use apitally::{ApitallyClient, ApitallyLayer, RequestLoggingConfig};
use axum::{response::IntoResponse, routing::get, Json, Router};
use dotenvy::dotenv;
use dotenvy_macro::dotenv;
use serde::{Deserialize, Serialize};

#[tokio::main]
async fn main() {
    dotenv().expect("Unable to load .env file");

    let apitally_client_id = dotenv!("APITALLY_CLIENT_ID");
    let apitally_environment = dotenv!("APITALLY_ENVIRONMENT");
    let mut api_tally_client = ApitallyClient::new(apitally_client_id, apitally_environment);
    api_tally_client.set_request_logging_config(RequestLoggingConfig::blanket_enabled());

    let app = Router::new()
        .route("/one", get(|| async { "howdy from route one!" }))
        .route("/twotwo", get(|| async { "howdy from route two!" }))
        .route(
            "/dyn/:dynamic",
            get(|| async { "howdy from route dynamic!" }),
        )
        .route("/json", get(test_json_body))
        .layer(ApitallyLayer(api_tally_client));

    let port = env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse().unwrap();

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[derive(Deserialize, Serialize)]
struct TestJSONPayload {
    message: String,
}

async fn test_json_body(Json(payload): Json<TestJSONPayload>) -> impl IntoResponse {
    Json(TestJSONPayload {
        message: payload.message,
    })
}
