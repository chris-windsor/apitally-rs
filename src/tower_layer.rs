use crate::{
    client::{RequestMeta, ResponseMeta},
    ApitallyClient,
};
use axum::{
    body::{self, Body},
    extract::{MatchedPath, Request},
    http::{
        header::{CONTENT_LENGTH, CONTENT_TYPE, HOST},
        uri::Scheme,
    },
    response::Response,
};
use futures_util::future::BoxFuture;
use std::time::SystemTime;
use std::{
    str::FromStr,
    task::{Context, Poll},
};
use tower::{Layer, Service};
use uuid::Uuid;

#[derive(Clone)]
pub struct ApitallyLayer(pub ApitallyClient);

const BODY_SIZE_LIMIT: usize = 50_000;

impl<S> Layer<S> for ApitallyLayer {
    type Service = ApitallyMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        ApitallyMiddleware {
            inner,
            client: ApitallyClient {
                framework: "rs:axum".to_string(),
                ..self.0.clone()
            },
        }
    }
}

#[derive(Clone)]
pub struct ApitallyMiddleware<S> {
    inner: S,
    client: ApitallyClient,
}

impl<S> Service<Request> for ApitallyMiddleware<S>
where
    S: Service<Request, Response = Response> + Send + 'static + Clone,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request) -> Self::Future {
        let request_key = Uuid::new_v4();
        let inner = self.inner.clone();
        let client = self.client.clone();

        Box::pin(async move {
            let request_start_time = SystemTime::now();
            let heads = request.headers();
            let url = format!(
                "{}://{}{}",
                request
                    .uri()
                    .scheme()
                    .unwrap_or(&Scheme::from_str("http").unwrap()),
                heads.get(HOST).unwrap().to_str().unwrap(),
                request.uri().path()
            );

            let (parts, body) = request.into_parts();
            let bytes = body::to_bytes(body, BODY_SIZE_LIMIT)
                .await
                .unwrap_or_default();
            let body_clone = bytes.clone();
            let request = Request::from_parts(parts, Body::from(bytes));

            let matched_path = request
                .extensions()
                .get::<MatchedPath>()
                .map(|matched_path| matched_path.as_str().to_owned())
                .unwrap_or_else(|| request.uri().to_string());

            client
                .stash_request_data(
                    request_key,
                    RequestMeta {
                        body: body_clone,
                        content_length: request
                            .headers()
                            .get(CONTENT_LENGTH)
                            .and_then(|header_value| header_value.to_str().ok())
                            .and_then(|header_value| header_value.parse().ok())
                            .unwrap_or(0),
                        content_type: request
                            .headers()
                            .get(CONTENT_TYPE)
                            .and_then(|header_value| header_value.to_str().ok())
                            .unwrap_or_default()
                            .to_string(),
                        headers: request
                            .headers()
                            .iter()
                            .map(|(header_name, header_value)| {
                                (
                                    header_name.as_str().parse().ok().unwrap(),
                                    header_value.to_str().unwrap().to_string(),
                                )
                            })
                            .collect(),
                        matched_path,
                        method: request.method().as_str().to_owned(),
                        url,
                    },
                )
                .ok();

            let mut service = inner;
            let response: Response = service.call(request).await?;

            let (parts, body) = response.into_parts();
            let bytes = body::to_bytes(body, BODY_SIZE_LIMIT)
                .await
                .unwrap_or_default();
            let body_clone = bytes.clone();
            let response = Response::from_parts(parts, Body::from(bytes));
            let body_size = body_clone.len();

            let request_processing_time = SystemTime::now()
                .duration_since(request_start_time)
                .unwrap();

            client
                .stash_response_data(
                    request_key,
                    ResponseMeta {
                        body: body_clone,
                        content_type: response
                            .headers()
                            .get(CONTENT_TYPE)
                            .and_then(|header_value| header_value.to_str().ok())
                            .unwrap_or_default()
                            .to_string(),
                        headers: response
                            .headers()
                            .iter()
                            .map(|(header_name, header_value)| {
                                (
                                    header_name.as_str().parse().ok().unwrap(),
                                    header_value.to_str().unwrap().to_string(),
                                )
                            })
                            .collect(),
                        size: body_size,
                        status: response.status(),
                        time: request_processing_time.as_secs_f32(),
                    },
                )
                .ok();

            Ok(response)
        })
    }
}
