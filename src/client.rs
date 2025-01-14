use axum::http::StatusCode;
use flate2::{Compression, GzBuilder};
use serde::Serialize;
use serde_json::json;
use std::{
    collections::HashMap,
    fs::OpenOptions,
    io::{Read, Seek, SeekFrom, Write},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

#[derive(Clone, Default)]
pub struct ApitallyClient {
    pub(crate) base_url: String,
    pub(crate) instance_id: Uuid,
    pub(crate) framework: String,
    pub(crate) request_log_config: RequestLoggingConfig,
    pub(crate) requests: Arc<Mutex<HashMap<Uuid, RequestMeta>>>,
    pub(crate) request_counts: Arc<Mutex<HashMap<RequestKey, usize>>>,
    pub(crate) request_size_sums: Arc<Mutex<HashMap<RequestKey, usize>>>,
    pub(crate) response_size_sums: Arc<Mutex<HashMap<RequestKey, usize>>>,
    pub(crate) response_times: Arc<Mutex<HashMap<RequestKey, HashMap<usize, usize>>>>,
    pub(crate) request_sizes: Arc<Mutex<HashMap<RequestKey, HashMap<usize, usize>>>>,
    pub(crate) response_sizes: Arc<Mutex<HashMap<RequestKey, HashMap<usize, usize>>>>,
}

#[derive(Clone, Default)]
pub struct RequestLoggingConfig {
    enabled: bool,
    log_query_params: bool,
    log_request_headers: bool,
    log_request_body: bool,
    log_response_headers: bool,
    log_response_body: bool,
}

impl RequestLoggingConfig {
    pub fn blanket_enabled() -> Self {
        Self {
            enabled: true,
            log_query_params: true,
            log_request_headers: true,
            log_request_body: true,
            log_response_headers: true,
            log_response_body: true,
        }
    }

    pub fn set_log_query_params(mut self, enabled: bool) -> Self {
        self.log_query_params = enabled;
        self
    }

    pub fn set_log_request_headers(mut self, log_request_headers: bool) -> Self {
        self.log_request_headers = log_request_headers;
        self
    }

    pub fn set_log_request_body(mut self, log_request_body: bool) -> Self {
        self.log_request_body = log_request_body;
        self
    }

    pub fn set_log_response_headers(mut self, log_response_headers: bool) -> Self {
        self.log_response_headers = log_response_headers;
        self
    }

    pub fn set_log_response_body(mut self, log_response_body: bool) -> Self {
        self.log_response_body = log_response_body;
        self
    }
}

#[derive(Eq, Hash, PartialEq, Clone)]
struct RequestKey {
    method: String,
    path: String,
    status: StatusCode,
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
            ..Default::default()
        };

        let _unhandled = instance.send_startup_data();

        let sync_instance = instance.clone();
        tokio::spawn(async move {
            loop {
                sync_instance.sync().await.expect("Sync Error");
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
        });

        instance
    }

    pub fn set_request_logging_config(&mut self, request_log_config: RequestLoggingConfig) {
        self.request_log_config = request_log_config;
    }

    fn send_startup_data(&self) -> Result<(), Box<dyn std::error::Error>> {
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
            client: self.framework.clone(),
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

    fn get_key_for_req(request_meta: &RequestMeta, response_meta: &ResponseMeta) -> RequestKey {
        RequestKey {
            method: request_meta.method.to_string(),
            path: request_meta.matched_path.to_string(),
            status: response_meta.status,
        }
    }

    pub(crate) fn stash_request_data(
        &self,
        request_key: Uuid,
        request_meta: RequestMeta,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let _unhandled = self
            .requests
            .lock()
            .unwrap()
            .insert(request_key, request_meta);

        Ok(())
    }

    pub(crate) fn stash_response_data(
        &self,
        request_key: Uuid,
        response_meta: ResponseMeta,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let requests_stash = self.requests.lock().unwrap();
        let request_meta = requests_stash.get(&request_key).unwrap().clone();
        let request_store_key = Self::get_key_for_req(&request_meta, &response_meta);

        let mut request_counts = self.request_counts.lock().unwrap();
        *request_counts.entry(request_store_key.clone()).or_insert(0) += 1;

        let mut request_size_sums = self.request_size_sums.lock().unwrap();
        *request_size_sums
            .entry(request_store_key.clone())
            .or_insert(request_meta.content_length) += 1;

        let mut response_size_sums = self.response_size_sums.lock().unwrap();
        *response_size_sums
            .entry(request_store_key.clone())
            .or_insert(response_meta.size) += 1;

        // todo: bulk these first
        if (self.request_log_config.enabled) {
            self.send_log_data(request_meta.clone(), response_meta.clone())?;
        }

        Ok(())
    }

    fn send_request_histogram(&self) -> Result<(), Box<dyn std::error::Error>> {
        let message_uuid = Uuid::new_v4();

        let mut request_counts = self.request_counts.lock().unwrap();

        let requests = request_counts
            .iter()
            .map(|(key, request_count)| {
                let request_size_sums = self.request_size_sums.lock().unwrap();
                let request_size_sum = request_size_sums.get(key).unwrap_or(&0);
                let response_size_sums = self.response_size_sums.lock().unwrap();
                let response_size_sum = response_size_sums.get(key).unwrap_or(&0);

                CapturedRequest {
                    consumer: None,
                    method: key.method.to_string(),
                    path: key.path.to_string(),
                    status_code: key.status.as_u16(),
                    request_size_sum: *request_size_sum,
                    response_size_sum: *response_size_sum,
                    request_count: *request_count,
                    response_times: HashMap::from([("0".to_string(), 1)]),
                    request_sizes: HashMap::from([("0".to_string(), 1)]),
                    response_sizes: HashMap::from([("0".to_string(), 1)]),
                }
            })
            .collect::<Vec<_>>();

        request_counts.clear();
        let mut request_size_sums = self.request_size_sums.lock().unwrap();
        request_size_sums.clear();
        let mut response_size_sums = self.response_size_sums.lock().unwrap();
        response_size_sums.clear();

        let body = RequestsBundleMessage {
            time_offset: 0,
            instance_uuid: self.instance_id.clone(),
            message_uuid,
            requests,
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

    fn send_log_data(
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
            timestamp: f64,
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

        let start = SystemTime::now();
        let timestamp = start
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards");
        let timestamp = timestamp.as_secs() as f64;

        let body = RequestLogMessage {
            uuid: Uuid::new_v4(),
            request: RequestLogRequest {
                timestamp,
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

        let temp_gzip_file = OpenOptions::new()
            .write(true)
            .create(true)
            .read(true)
            // todo: move to tmp
            .open("requestLog.gz")
            .unwrap();
        let mut gz_encoder = GzBuilder::new().write(temp_gzip_file, Compression::default());
        gz_encoder.write_all(format!("{}\n", serde_json::to_string(&body)?).as_bytes())?;
        gz_encoder.flush()?;
        let mut temp_gzip_file = gz_encoder.finish()?;
        temp_gzip_file.seek(SeekFrom::Start(0)).unwrap();
        let mut body: Vec<u8> = vec![];
        temp_gzip_file.read_to_end(&mut body)?;

        let base_url = self.base_url.clone();
        tokio::task::spawn(async move {
            let _unhandled = reqwest::Client::new()
                .post(format!("{base_url}/{URL_LOG_SUFFIX}",))
                .query(&[("uuid", Uuid::new_v4().to_string())])
                .body(body)
                .send()
                .await;
        });

        Ok(())
    }

    async fn sync(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.send_request_histogram()?;
        Ok(())
    }
}
