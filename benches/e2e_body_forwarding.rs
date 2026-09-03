//! End-to-end benchmarks for body forwarding through the Lambda Web Adapter.
//!
//! This benchmark measures the full request flow including:
//! - Lambda event parsing
//! - Body conversion (zero-copy)
//! - HTTP client request to mock server
//! - Response handling
//!
//! Run with: cargo bench --bench e2e_body_forwarding
//!
//! For CI/CD, use: cargo bench --bench e2e_body_forwarding -- --noplot

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use http::{HeaderMap, Method, Request};
use httpmock::{Method::POST, MockServer};
use lambda_http::Body;
use lambda_web_adapter::{Adapter, AdapterOptions};
use std::sync::Arc;
use tokio::runtime::Runtime;
use tower::Service;

mod common;
use common::LambdaEventBuilder;

/// Add Lambda context to request (required for adapter to process it)
fn add_lambda_context(request: &mut Request<Body>) {
    use lambda_http::lambda_runtime::Config;
    use lambda_http::Context;

    let mut headers = HeaderMap::new();
    headers.insert("lambda-runtime-aws-request-id", "bench-id".parse().unwrap());
    headers.insert("lambda-runtime-deadline-ms", "999999".parse().unwrap());
    headers.insert("lambda-runtime-client-context", "{}".parse().unwrap());

    let conf = Config {
        function_name: "bench_function".into(),
        memory: 128,
        version: "latest".into(),
        log_stream: "/aws/lambda/bench".into(),
        log_group: "bench-group".into(),
    };

    let context = Context::new("bench-id", Arc::new(conf), &headers).unwrap();
    request.extensions_mut().insert(context);
}

/// Benchmark e2e request forwarding with text bodies of various sizes
fn bench_text_body(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let app_server = rt.block_on(async { MockServer::start_async().await });
    app_server.mock(|when, then| {
        when.method(POST).path("/api/data");
        then.status(200).body("OK");
    });

    let mut group = c.benchmark_group("e2e_text_body");

    // Reduce sample size for faster CI runs while maintaining statistical validity
    group.sample_size(50);

    // Test representative sizes: empty, small, medium, large, max Lambda payload
    let sizes = [
        0,
        1024,            // 1 KB
        64 * 1024,       // 64 KB
        128 * 1024,      // 128 KB
        256 * 1024,      // 256 KB
        512 * 1024,      // 512 KB
        1024 * 1024,     // 1 MB
        2 * 1024 * 1024, // 2 MB
        4 * 1024 * 1024, // 4 MB
        6 * 1024 * 1024, // 6 MB (Lambda payload limit)
    ];

    for size in sizes {
        group.throughput(Throughput::Bytes(size as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            let adapter = Adapter::new(&AdapterOptions {
                host: app_server.host(),
                port: app_server.port().to_string(),
                readiness_check_port: app_server.port().to_string(),
                readiness_check_path: "/".to_string(),
                ..Default::default()
            })
            .expect("Failed to create adapter");

            let body_content = "x".repeat(size);

            b.to_async(&rt).iter(|| {
                let body = body_content.clone();
                let mut adapter = adapter.clone();
                async move {
                    let req = LambdaEventBuilder::new()
                        .with_method(Method::POST)
                        .with_path("/api/data")
                        .with_header("content-type", "text/plain")
                        .with_body(&body)
                        .build();

                    let mut request = Request::from(req);
                    add_lambda_context(&mut request);

                    adapter.call(request).await.expect("Request failed")
                }
            });
        });
    }

    group.finish();
}

/// Benchmark e2e request forwarding with binary bodies of various sizes
fn bench_binary_body(c: &mut Criterion) {
    use base64::{engine::general_purpose::STANDARD, Engine};

    let rt = Runtime::new().unwrap();

    let app_server = rt.block_on(async { MockServer::start_async().await });
    app_server.mock(|when, then| {
        when.method(POST).path("/api/data");
        then.status(200).body("OK");
    });

    let mut group = c.benchmark_group("e2e_binary_body");

    // Reduce sample size for faster CI runs
    group.sample_size(50);

    // Target sizes for base64-encoded payload
    // Binary size = target_size * 3 / 4 (base64 expands by 4/3)
    let target_encoded_sizes = [
        0,
        1024,            // 1 KB
        64 * 1024,       // 64 KB
        128 * 1024,      // 128 KB
        256 * 1024,      // 256 KB
        512 * 1024,      // 512 KB
        1024 * 1024,     // 1 MB
        2 * 1024 * 1024, // 2 MB
        4 * 1024 * 1024, // 4 MB
        6 * 1024 * 1024, // 6 MB
    ];

    for target_size in target_encoded_sizes {
        // Calculate binary size needed to produce target encoded size
        let binary_size = target_size * 3 / 4;

        // Pre-encode to exclude base64 encoding time from benchmark
        let body_content: Vec<u8> = vec![0xAB; binary_size];
        let body_base64 = STANDARD.encode(&body_content);

        // Throughput based on base64-encoded size (actual payload size)
        group.throughput(Throughput::Bytes(body_base64.len() as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(body_base64.len()),
            &target_size,
            |b, &_size| {
                let adapter = Adapter::new(&AdapterOptions {
                    host: app_server.host(),
                    port: app_server.port().to_string(),
                    readiness_check_port: app_server.port().to_string(),
                    readiness_check_path: "/".to_string(),
                    ..Default::default()
                })
                .expect("Failed to create adapter");

                let body = body_base64.clone();

                b.to_async(&rt).iter(|| {
                    let body = body.clone();
                    let mut adapter = adapter.clone();
                    async move {
                        let req = LambdaEventBuilder::new()
                            .with_method(Method::POST)
                            .with_path("/api/data")
                            .with_header("content-type", "application/octet-stream")
                            .with_base64_body(&body)
                            .build();

                        let mut request = Request::from(req);
                        add_lambda_context(&mut request);

                        adapter.call(request).await.expect("Request failed")
                    }
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_text_body, bench_binary_body);
criterion_main!(benches);
