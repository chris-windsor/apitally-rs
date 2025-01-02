use std::task::{Context, Poll};

use axum::{
    body::HttpBody,
    extract::{MatchedPath, Request},
    http::header::CONTENT_LENGTH,
    response::Response,
};
use futures_util::future::BoxFuture;
use tower::{Layer, Service};
use uuid::Uuid;

use crate::client::{RequestMeta, ResponseMeta};
use crate::ApitallyClient;

#[derive(Clone)]
pub struct ApitallyLayer(pub ApitallyClient);

impl<S> Layer<S> for ApitallyLayer {
    type Service = ApitallyMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        ApitallyMiddleware {
            inner,
            client: self.0.clone(),
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
    S: Service<Request, Response = Response> + Send + 'static,
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

        let _unhandled = self.client.stash_request_data(
            request_key,
            RequestMeta {
                content_length: match request.headers().get(CONTENT_LENGTH) {
                    Some(content_length) => content_length.to_str().unwrap().parse().unwrap(),
                    None => 0,
                },
                matched_path: match request.extensions().get::<MatchedPath>() {
                    Some(matched_path) => matched_path.as_str().to_owned(),
                    None => request.uri().path().to_owned(),
                },
                method: request.method().as_str().to_owned(),
                url: request.uri().to_string(),
            },
        );

        let future = self.inner.call(request);
        let client = self.client.clone();
        Box::pin(async move {
            let response: Response = future.await?;

            let _unhandled = client.send_request_data(
                request_key,
                ResponseMeta {
                    status: response.status(),
                    size: response.body().size_hint().exact().unwrap_or(0) as usize,
                },
            );

            Ok(response)
        })
    }
}
