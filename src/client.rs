use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use axum::http::StatusCode;
use serde::Serialize;
use serde_json::json;
use uuid::Uuid;

#[derive(Clone)]
pub struct ApitallyClient {
    base_url: String,
    instance_id: Uuid,
    request_log_config: RequestLogConfig,
    request_stash: Arc<Mutex<HashMap<Uuid, RequestMeta>>>,
}

#[derive(Clone, Default)]
pub struct RequestLogConfig {
    pub enabled: bool,
    pub log_query_params: Option<bool>,
    pub log_request_headers: Option<bool>,
    pub log_request_body: Option<bool>,
    pub log_response_headers: Option<bool>,
    pub log_response_body: Option<bool>,
}

#[derive(Clone)]
pub struct RequestMeta {
    pub content_length: usize,
    pub matched_path: String,
    pub method: String,
    pub url: String,
}

#[derive(Clone)]
pub struct ResponseMeta {
    pub size: usize,
    pub status: StatusCode,
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
const URL_LOG_SUFFIX: &str = "log";

impl ApitallyClient {
    pub fn new(client_id: &str, environment: &str) -> Self {
        let base_url = format!("https://hub.apitally.io/v2/{client_id}/{environment}",);
        let instance_id = Uuid::new_v4();

        let instance = Self {
            base_url,
            instance_id,
            request_log_config: Default::default(),
            request_stash: Default::default(),
        };

        let _unhandled = instance.send_startup_data();

        instance
    }

    pub fn set_request_log_config(&mut self, request_log_config: RequestLogConfig) {
        self.request_log_config = request_log_config;
    }

    pub fn send_startup_data(&self) -> Result<(), Box<dyn std::error::Error>> {
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

        let base_url = self.base_url.clone();
        tokio::task::spawn(async move {
            let _unhandled = reqwest::Client::new()
                .post(format!("{base_url}/{URL_STARTUP_SUFFIX}",))
                .json(&json!(body))
                .send()
                .await;
        });

        Ok(())
    }

    pub fn stash_request_data(
        &self,
        request_key: Uuid,
        request_meta: RequestMeta,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let _unhandled = self
            .request_stash
            .lock()
            .unwrap()
            .insert(request_key, request_meta);

        Ok(())
    }

    pub fn send_request_data(
        &self,
        request_key: Uuid,
        response_meta: ResponseMeta,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let request_stash = self.request_stash.lock().unwrap();

        let request_meta = request_stash.get(&request_key).unwrap().clone();

        self.send_log_data(request_meta.clone(), response_meta.clone())?;

        let body = RequestsBundleMessage {
            time_offset: 0,
            instance_uuid: self.instance_id.clone(),
            message_uuid: request_key,
            requests: vec![CapturedRequest {
                consumer: None,
                method: request_meta.method,
                path: request_meta.matched_path,
                status_code: response_meta.status.as_u16(),
                request_count: 1,
                request_size_sum: request_meta.content_length,
                response_size_sum: response_meta.size,
                response_times: HashMap::from([("0".to_string(), 1)]),
                request_sizes: HashMap::from([("0".to_string(), 1)]),
                response_sizes: HashMap::from([("0".to_string(), 1)]),
            }],
            validation_errors: vec![],
            server_errors: vec![],
            consumers: vec![],
        };

        let base_url = self.base_url.clone();
        tokio::task::spawn(async move {
            let _unhandled = reqwest::Client::new()
                .post(format!("{base_url}/{URL_SYNC_SUFFIX}",))
                .json(&json!(body))
                .send()
                .await;
        });

        Ok(())
    }

    pub fn send_log_data(
        &self,
        request_meta: RequestMeta,
        response_meta: ResponseMeta,
    ) -> Result<(), Box<dyn std::error::Error>> {
        #[derive(Serialize)]
        struct RequestLogMessage {
            uuid: Uuid,
            request: RequestLogRequest,
            response: RequestLogResponse,
        }

        #[derive(Serialize)]
        struct RequestLogRequest {
            timestamp: f32,
            method: String,
            path: String,
            url: String,
            headers: Vec<(String, String)>,
            size: usize,
            consumer: String,
            body: String,
        }

        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct RequestLogResponse {
            status_code: u16,
            response_time: f32,
            headers: Vec<(String, String)>,
            size: usize,
            body: String,
        }

        let body = RequestLogMessage {
            uuid: Uuid::new_v4(),
            request: RequestLogRequest {
                timestamp: 0.0,
                method: request_meta.method,
                path: request_meta.matched_path,
                url: request_meta.url,
                headers: vec![],
                size: request_meta.content_length,
                consumer: "".to_string(),
                body: "".to_string(),
            },
            response: RequestLogResponse {
                status_code: response_meta.status.as_u16(),
                response_time: 100.0,
                headers: vec![],
                size: response_meta.size,
                body: "".to_string(),
            },
        };

        let base_url = self.base_url.clone();
        tokio::task::spawn(async move {
            let _unhandled = reqwest::Client::new()
                .post(format!("{base_url}/{URL_LOG_SUFFIX}",))
                .query(&[("uuid", Uuid::new_v4().to_string())])
                .body(serde_json::to_string(&body).unwrap())
                .send()
                .await;
        });

        Ok(())
    }
}
