use std::{env, net::SocketAddr};

use apitally::{client::ApitallyClient, ApitallyLayer};
use axum::{routing::get, Router};
use dotenvy::dotenv;
use dotenvy_macro::dotenv;

#[tokio::main]
async fn main() {
    dotenv().expect("Unable to load .env file");

    let apitally_client_id = dotenv!("APITALLY_CLIENT_ID");
    let apitally_environment = dotenv!("APITALLY_ENVIRONMENT");
    let api_tally_client = ApitallyClient::new(apitally_client_id, apitally_environment);

    let app = Router::new()
        .route("/route-one", get(|| async { "howdy from route one!" }))
        .route("/route-two", get(|| async { "howdy from route two!" }))
        .route(
            "/route/:dynamic",
            get(|| async { "howdy from route dynamic!" }),
        )
        .layer(ApitallyLayer(api_tally_client));

    let port = env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse().unwrap();

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
