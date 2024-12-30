pub mod client;

use std::task::{Context, Poll};

use axum::{
    extract::{MatchedPath, Request},
    response::Response,
};
use client::{ApitallyClient, RequestMeta};
use futures_util::future::BoxFuture;
use tower::{Layer, Service};

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

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request) -> Self::Future {
        let _unhandled = self.client.send_request_data(RequestMeta {
            uri: request
                .extensions()
                .get::<MatchedPath>()
                .unwrap()
                .as_str()
                .to_owned(),
        });

        let future = self.inner.call(request);
        Box::pin(async move {
            let response: Response = future.await?;

            Ok(response)
        })
    }
}
