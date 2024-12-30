use std::collections::HashMap;

use axum::http::StatusCode;
use serde::Serialize;
use serde_json::{json, Value};
use uuid::Uuid;

#[derive(Clone)]
pub struct ApitallyClient {
    base_url: String,
    instance_id: Uuid,
}

pub struct RequestMeta {
    pub uri: String,
}

#[derive(Serialize)]
struct RequestsBundleMessage {
    time_offset: usize,
    instance_uuid: Uuid,
    message_uuid: Uuid,
    requests: Vec<CapturedRequest>,
    validation_errors: Vec<ValidationError>,
    server_errors: Vec<ServerError>,
    consumers: Vec<String>,
}

#[derive(Serialize)]
struct CapturedRequest {
    consumer: Option<String>,
    method: String,
    path: String,
    status_code: u16,
    request_count: usize,
    request_size_sum: usize,
    response_size_sum: usize,
    response_times: HashMap<String, usize>,
    request_sizes: HashMap<String, usize>,
    response_sizes: HashMap<String, usize>,
}

#[derive(Serialize)]
struct ValidationError {}

#[derive(Serialize)]
struct ServerError {}

const URL_STARTUP_SUFFIX: &str = "startup";
const URL_SYNC_SUFFIX: &str = "sync";

impl ApitallyClient {
    pub fn new(client_id: &str, environment: &str) -> Self {
        let base_url = format!("https://hub.apitally.io/v2/{client_id}/{environment}",);
        let instance_id = Uuid::new_v4();

        let instance = Self {
            base_url,
            instance_id,
        };

        let _unhandled = instance.send_startup_data();

        instance
    }

    fn send_data(&self, url_suffix: &str, data: Value) -> Result<(), reqwest::Error> {
        let base_url = self.base_url.clone();
        let url_suffix = url_suffix.to_owned();

        tokio::task::spawn(async move {
            let _unhandled = reqwest::Client::new()
                .post(format!("{base_url}/{url_suffix}",))
                .json(&data)
                .send()
                .await;
        });

        Ok(())
    }

    pub fn send_startup_data(&self) -> Result<(), reqwest::Error> {
        let message_id = Uuid::new_v4();

        #[derive(Serialize)]
        struct StartUpMessage {
            instance_uuid: Uuid,
            message_uuid: Uuid,
            paths: Vec<String>,
            versions: HashMap<String, String>,
            client: String,
        }

        let body = StartUpMessage {
            instance_uuid: self.instance_id.clone(),
            message_uuid: message_id,
            paths: vec![],
            versions: HashMap::new(),
            client: String::from("rs:axum"),
        };

        let _ignored = self.send_data(URL_STARTUP_SUFFIX, json!(body));

        Ok(())
    }

    pub fn send_request_data(&self, req_meta: RequestMeta) -> Result<(), reqwest::Error> {
        let message_uuid = Uuid::new_v4();

        let body = RequestsBundleMessage {
            time_offset: 0,
            instance_uuid: self.instance_id.clone(),
            message_uuid,
            requests: vec![CapturedRequest {
                consumer: None,
                method: String::from("POST"),
                path: req_meta.uri,
                status_code: StatusCode::OK.as_u16(),
                request_count: 1,
                request_size_sum: 100,
                response_size_sum: 100,
                response_times: HashMap::from([("0".to_string(), 1)]),
                request_sizes: HashMap::from([("0".to_string(), 1)]),
                response_sizes: HashMap::from([("0".to_string(), 1)]),
            }],
            validation_errors: vec![],
            server_errors: vec![],
            consumers: vec![],
        };

        let _unhandled = self.send_data(URL_SYNC_SUFFIX, json!(body));

        Ok(())
    }
}
