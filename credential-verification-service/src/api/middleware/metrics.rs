use axum::http;
use futures_util::FutureExt;
use futures_util::future::BoxFuture;
use prometheus_client::registry::Unit;
use prometheus_client::{
    encoding::EncodeLabelSet,
    metrics::{family::Family, histogram},
    registry::Registry,
};
use std::{future::Future, task};

/// tower layer adding monitoring to a service.
#[derive(Debug, Clone)]
pub struct MetricsLayer {
    /// Metric tracking the response status code and response duration.
    request_metrics: Family<QueryLabels, histogram::Histogram>,
}

impl MetricsLayer {
    pub fn new(registry: &mut Registry) -> Self {
        let request_metrics: Family<QueryLabels, _> = Family::new_with_constructor(|| {
            histogram::Histogram::new(histogram::exponential_buckets(0.010, 2.0, 10))
        });
        registry.register_with_unit(
            "rest_request_duration",
            "Duration in seconds for responding to requests for the REST API",
            Unit::Seconds,
            request_metrics.clone(),
        );
        Self { request_metrics }
    }
}

impl<S> tower::Layer<S> for MetricsLayer {
    type Service = MetricsService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        MetricsService {
            inner,
            request_metrics: self.request_metrics.clone(),
        }
    }
}

/// Service middleware tracking metrics.
#[derive(Debug, Clone)]
pub struct MetricsService<S> {
    /// The inner service.
    inner: S,
    /// Metric tracking the response status code and response duration.
    request_metrics: Family<QueryLabels, histogram::Histogram>,
}

/// Type representing the Prometheus labels used for metrics related to
/// queries to the REST API.
#[derive(Debug, Clone, EncodeLabelSet, PartialEq, Eq, Hash)]
struct QueryLabels {
    /// Path in the request.
    path: String,
    /// The response status code.
    status: Option<u16>,
}

impl<S, ReqBody, RespBody> tower::Service<http::Request<ReqBody>> for MetricsService<S>
where
    S: tower::Service<http::Request<ReqBody>, Response = http::Response<RespBody>>,
    S::Future: Send + 'static,
{
    type Error = S::Error;
    type Future = BoxFuture<'static, <S::Future as Future>::Output>;
    type Response = S::Response;

    fn poll_ready(&mut self, cx: &mut task::Context<'_>) -> task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: http::Request<ReqBody>) -> Self::Future {
        let start = tokio::time::Instant::now();
        let endpoint = String::from(req.uri().path());
        let request_metrics = self.request_metrics.clone();

        self.inner
            .call(req)
            .inspect(move |res| {
                let status = res.as_ref().ok().map(|resp| resp.status().as_u16());

                request_metrics
                    .get_or_create(&QueryLabels {
                        path: endpoint,
                        status,
                    })
                    .observe(start.elapsed().as_secs_f64());
            })
            .boxed()
    }
}
