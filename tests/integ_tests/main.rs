pub mod common;

use std::env;
use std::io;
use std::io::prelude::*;
use std::sync::Arc;

use crate::common::LambdaEventBuilder;
use http::HeaderMap;
use http::Request;
use http::{Method, Response};
use httpmock::{
    Method::{DELETE, GET, POST, PUT},
    MockServer,
};
use hyper::body::Incoming;
use lambda_http::Body;
use lambda_http::Context;
use lambda_web_adapter::{Adapter, AdapterOptions, LambdaInvokeMode, Protocol};
use tower::{Service, ServiceBuilder};

use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use http_body_util::BodyExt;
use lambda_http::lambda_runtime::Config;
use serde_json::json;
use tower_http::compression::{CompressionBody, CompressionLayer};

#[test]
fn test_adapter_options_from_env() {
    env::set_var("AWS_LWA_PORT", "3000");
    env::set_var("AWS_LWA_HOST", "localhost");
    env::set_var("AWS_LWA_READINESS_CHECK_PORT", "8000");
    env::set_var("AWS_LWA_READINESS_CHECK_PROTOCOL", "TCP");
    env::set_var("AWS_LWA_READINESS_CHECK_PATH", "/healthcheck");
    env::set_var("AWS_LWA_REMOVE_BASE_PATH", "/prod");
    env::set_var("AWS_LWA_ASYNC_INIT", "true");
    env::set_var("AWS_LWA_ENABLE_COMPRESSION", "true");
    env::set_var("AWS_LWA_INVOKE_MODE", "buffered");
    env::set_var("AWS_LWA_AUTHORIZATION_SOURCE", "auth-token");

    // Initialize adapter with env options
    let options = AdapterOptions::default();
    Adapter::new(&options).expect("Failed to create adapter");

    assert_eq!("3000", options.port);
    assert_eq!("localhost", options.host);
    assert_eq!("8000", options.readiness_check_port);
    assert_eq!("/healthcheck", options.readiness_check_path);
    assert_eq!(Protocol::Tcp, options.readiness_check_protocol);
    assert_eq!(Some("/prod".into()), options.base_path);
    assert!(options.async_init);
    assert!(options.compression);
    assert_eq!(LambdaInvokeMode::Buffered, options.invoke_mode);
    assert_eq!(Some("auth-token".into()), options.authorization_source);
}

#[test]
fn test_adapter_options_from_namespaced_env() {
    env::set_var("AWS_LWA_PORT", "3000");
    env::set_var("AWS_LWA_HOST", "localhost");
    env::set_var("AWS_LWA_READINESS_CHECK_HEALTHY_STATUS", "200-399");
    env::set_var("AWS_LWA_READINESS_CHECK_PORT", "8000");
    env::set_var("AWS_LWA_READINESS_CHECK_PROTOCOL", "TCP");
    env::set_var("AWS_LWA_READINESS_CHECK_PATH", "/healthcheck");
    env::set_var("AWS_LWA_REMOVE_BASE_PATH", "/prod");
    env::set_var("AWS_LWA_ASYNC_INIT", "true");
    env::set_var("AWS_LWA_ENABLE_COMPRESSION", "true");
    env::set_var("AWS_LWA_INVOKE_MODE", "response_stream");
    env::set_var("AWS_LWA_AUTHORIZATION_SOURCE", "auth-token");

    // Initialize adapter with env options
    let options = AdapterOptions::default();
    Adapter::new(&options).expect("Failed to create adapter");

    assert_eq!("3000", options.port);
    // Check that healthy status codes are 200-399
    assert!(options.readiness_check_healthy_status.contains(&200));
    assert!(options.readiness_check_healthy_status.contains(&399));
    assert!(!options.readiness_check_healthy_status.contains(&400));
    assert_eq!("localhost", options.host);
    assert_eq!("8000", options.readiness_check_port);
    assert_eq!("/healthcheck", options.readiness_check_path);
    assert_eq!(Protocol::Tcp, options.readiness_check_protocol);
    assert_eq!(Some("/prod".into()), options.base_path);
    assert!(options.async_init);
    assert!(options.compression);
    assert_eq!(LambdaInvokeMode::ResponseStream, options.invoke_mode);
    assert_eq!(Some("auth-token".into()), options.authorization_source);
}

#[test]
fn test_readiness_check_port_fallback_to_lwa_port() {
    env::remove_var("AWS_LWA_READINESS_CHECK_PORT");
    env::set_var("AWS_LWA_PORT", "3000");

    // Initialize adapter with env options
    let options = AdapterOptions::default();
    Adapter::new(&options).expect("Failed to create adapter");

    assert_eq!("3000", options.readiness_check_port);
}

#[test]
fn test_port_fallback_to_port_env() {
    env::remove_var("AWS_LWA_PORT");
    env::remove_var("AWS_LWA_READINESS_CHECK_PORT");
    env::set_var("PORT", "9090");

    let options = AdapterOptions::default();
    Adapter::new(&options).expect("Failed to create adapter");

    assert_eq!("9090", options.port);

    env::remove_var("PORT");
}

#[tokio::test]
async fn test_http_readiness_check() {
    // Start app server
    let app_server = MockServer::start();
    let healthcheck = app_server.mock(|when, then| {
        when.method(GET).path("/healthcheck");
        then.status(200).body("OK");
    });

    // Prepare adapter configuration
    let options = AdapterOptions {
        host: app_server.host(),
        port: app_server.port().to_string(),
        readiness_check_port: app_server.port().to_string(),
        readiness_check_path: "/healthcheck".to_string(),
        ..Default::default()
    };

    // Initialize adapter and do readiness check
    let mut adapter = Adapter::new(&options).expect("Failed to create adapter");
    adapter.check_init_health().await;

    // Assert app server's healthcheck endpoint got called
    healthcheck.assert();
}

#[tokio::test]
async fn test_http_basic_request() {
    // Start app server
    let app_server = MockServer::start();
    let hello = app_server.mock(|when, then| {
        when.method(GET).path("/hello");
        then.status(200).body("Hello World");
    });

    // Initialize adapter
    let mut adapter = Adapter::new(&AdapterOptions {
        host: app_server.host(),
        port: app_server.port().to_string(),
        readiness_check_port: app_server.port().to_string(),
        readiness_check_path: "/healthcheck".to_string(),
        ..Default::default()
    })
    .expect("Failed to create adapter");

    // // Call the adapter service with basic request
    let req = LambdaEventBuilder::new().with_path("/hello").build();

    // We convert to Request object because it allows us to add
    // the lambda Context
    let mut request = Request::from(req);
    add_lambda_context_to_request(&mut request);

    let response = adapter.call(request).await.expect("Request failed");

    // Assert endpoint was called once
    hello.assert();

    // and response has expected content
    assert_eq!(200, response.status());
    assert_eq!(response.headers().get("content-length").unwrap(), "11");
    assert_eq!("Hello World", body_to_string(response).await);
}

#[tokio::test]
async fn test_http_headers() {
    // Start app server
    let app_server = MockServer::start();

    // An endpoint that expects and returns headers
    let test_endpoint = app_server.mock(|when, then| {
        when.method(GET).path("/").header("foo", "bar");
        then.status(200).header("fizz", "buzz").body("OK");
    });

    // Initialize adapter and do readiness check
    let mut adapter = Adapter::new(&AdapterOptions {
        host: app_server.host(),
        port: app_server.port().to_string(),
        readiness_check_port: app_server.port().to_string(),
        readiness_check_path: "/healthcheck".to_string(),
        ..Default::default()
    })
    .expect("Failed to create adapter");

    // Prepare request
    let req = LambdaEventBuilder::new()
        .with_path("/")
        .with_header("foo", "bar")
        .build();

    // We convert to Request object because it allows us to add
    // the Lambda Context
    let mut request = Request::from(req);
    add_lambda_context_to_request(&mut request);

    // Call the adapter service with request
    let response = adapter.call(request).await.expect("Request failed");

    // Assert endpoint was called once
    test_endpoint.assert();

    // and response has expected content
    assert_eq!(200, response.status());
    assert!(response.headers().contains_key("fizz"));
    assert_eq!("buzz", response.headers().get("fizz").unwrap());
    assert_eq!("OK", body_to_string(response).await);
}

#[tokio::test]
async fn test_http_path_encoding() {
    // Start app server
    let app_server = MockServer::start();

    // An endpoint that expects and returns headers
    let test_endpoint = app_server.mock(|when, then| {
        when.method(GET).path("/A%C3%B1o_1234");
        then.status(200).body("Ok");
    });

    // Initialize adapter and do readiness check
    let mut adapter = Adapter::new(&AdapterOptions {
        host: app_server.host(),
        port: app_server.port().to_string(),
        readiness_check_port: app_server.port().to_string(),
        readiness_check_path: "/healthcheck".to_string(),
        ..Default::default()
    })
    .expect("Failed to create adapter");

    // Prepare request
    let req = LambdaEventBuilder::new().with_path("/Año_1234").build();

    // We convert to Request object because it allows us to add
    // the lambda Context
    let mut request = Request::from(req);
    add_lambda_context_to_request(&mut request);

    // Call the adapter service with request
    let response = adapter.call(request).await.expect("Request failed");

    // Assert endpoint was called once
    test_endpoint.assert();

    // and response has expected content
    assert_eq!(200, response.status());
    assert_eq!("Ok", body_to_string(response).await);
}

#[tokio::test]
async fn test_http_query_params() {
    // Start app server
    let app_server = MockServer::start();

    // An endpoint that expects and returns headers
    let test_endpoint = app_server.mock(|when, then| {
        when.method(GET)
            .path("/")
            .query_param("foo", "bar")
            .query_param("fizz", "buzz");
        then.status(200).body("OK");
    });

    // Initialize adapter
    let mut adapter = Adapter::new(&AdapterOptions {
        host: app_server.host(),
        port: app_server.port().to_string(),
        readiness_check_port: app_server.port().to_string(),
        readiness_check_path: "/healthcheck".to_string(),
        ..Default::default()
    })
    .expect("Failed to create adapter");

    // Prepare request
    let req = LambdaEventBuilder::new()
        .with_path("/")
        .with_query("foo", "bar")
        .with_query("fizz", "buzz")
        .build();

    // We convert to Request object because it allows us to add
    // the lambda Context
    let mut request = Request::from(req);
    add_lambda_context_to_request(&mut request);

    // Call the adapter service with request
    let response = adapter.call(request).await.expect("Request failed");

    // Assert endpoint was called once
    test_endpoint.assert();

    // and response has expected content
    assert_eq!(200, response.status());
    assert_eq!("OK", body_to_string(response).await);
}

#[tokio::test]
async fn test_http_post_put_delete() {
    // Start app server
    let app_server = MockServer::start();
    let post_endpoint = app_server.mock(|when, then| {
        when.path("/").method(POST);
        then.status(200).body("POST Success");
    });

    let put_endpoint = app_server.mock(|when, then| {
        when.path("/").method(PUT);
        then.status(200).body("PUT Success");
    });

    let delete_endpoint = app_server.mock(|when, then| {
        when.path("/").method(DELETE);
        then.status(200).body("DELETE Success");
    });

    // Initialize adapter
    let mut adapter = Adapter::new(&AdapterOptions {
        host: app_server.host(),
        port: app_server.port().to_string(),
        readiness_check_port: app_server.port().to_string(),
        readiness_check_path: "/healthcheck".to_string(),
        ..Default::default()
    })
    .expect("Failed to create adapter");

    // Prepare requests
    let post_req = LambdaEventBuilder::new()
        .with_method(Method::POST)
        .with_path("/")
        .build();

    let put_req = LambdaEventBuilder::new()
        .with_method(Method::PUT)
        .with_path("/")
        .build();

    let delete_req = LambdaEventBuilder::new()
        .with_method(Method::DELETE)
        .with_path("/")
        .build();
    // We convert to Request object because it allows us to add the lambda Context
    let mut post_request = Request::from(post_req);
    add_lambda_context_to_request(&mut post_request);

    let mut put_request = Request::from(put_req);
    add_lambda_context_to_request(&mut put_request);

    let mut delete_request = Request::from(delete_req);
    add_lambda_context_to_request(&mut delete_request);

    // Call the adapter service with requests
    let post_response = adapter.call(post_request).await.expect("Request failed");
    let put_response = adapter.call(put_request).await.expect("Request failed");
    let delete_response = adapter.call(delete_request).await.expect("Request failed");

    // Assert endpoints were called
    post_endpoint.assert();
    put_endpoint.assert();
    delete_endpoint.assert();

    assert_eq!(200, post_response.status());
    assert_eq!(200, put_response.status());
    assert_eq!(200, delete_response.status());
    assert_eq!("POST Success", body_to_string(post_response).await);
    assert_eq!("PUT Success", body_to_string(put_response).await);
    assert_eq!("DELETE Success", body_to_string(delete_response).await);
}

#[tokio::test]
async fn test_http_request_body_forwarded() {
    // Start app server
    let app_server = MockServer::start();

    // Test 1: Text body (JSON)
    let json_endpoint = app_server.mock(|when, then| {
        when.method(POST)
            .path("/api/json")
            .header("content-type", "application/json")
            .body(r#"{"name":"test","value":123}"#);
        then.status(201).body("JSON Created");
    });

    // Test 2: Empty body
    let empty_endpoint = app_server.mock(|when, then| {
        when.method(POST).path("/api/empty").body("");
        then.status(204);
    });

    // Test 3: Binary body (with base64 encoding as ALB/API Gateway would send)
    let binary_data: Vec<u8> = vec![0x00, 0x01, 0x02, 0xFF, 0xFE, 0x89, 0x50, 0x4E, 0x47];
    let expected_binary = binary_data.clone();
    let binary_endpoint = app_server.mock(|when, then| {
        when.method(POST)
            .path("/api/binary")
            .is_true(move |req| req.body().as_ref() == expected_binary.as_slice());
        then.status(200).body("Binary OK");
    });

    // Initialize adapter
    let mut adapter = Adapter::new(&AdapterOptions {
        host: app_server.host(),
        port: app_server.port().to_string(),
        readiness_check_port: app_server.port().to_string(),
        readiness_check_path: "/healthcheck".to_string(),
        ..Default::default()
    })
    .expect("Failed to create adapter");

    // Test Text body (Body::Text)
    let json_req = LambdaEventBuilder::new()
        .with_method(Method::POST)
        .with_path("/api/json")
        .with_header("content-type", "application/json")
        .with_body(r#"{"name":"test","value":123}"#)
        .build();
    let mut json_request = Request::from(json_req);
    add_lambda_context_to_request(&mut json_request);
    let json_response = adapter.call(json_request).await.expect("JSON request failed");
    json_endpoint.assert();
    assert_eq!(201, json_response.status());
    assert_eq!("JSON Created", body_to_string(json_response).await);

    // Test Empty body (Body::Empty)
    let empty_req = LambdaEventBuilder::new()
        .with_method(Method::POST)
        .with_path("/api/empty")
        .build();
    let mut empty_request = Request::from(empty_req);
    add_lambda_context_to_request(&mut empty_request);
    let empty_response = adapter.call(empty_request).await.expect("Empty request failed");
    empty_endpoint.assert();
    assert_eq!(204, empty_response.status());

    // Test Binary body (Body::Binary)
    let binary_req = LambdaEventBuilder::new()
        .with_method(Method::POST)
        .with_path("/api/binary")
        .with_binary_body(&binary_data)
        .build();
    let mut binary_request = Request::from(binary_req);
    add_lambda_context_to_request(&mut binary_request);
    let binary_response = adapter.call(binary_request).await.expect("Binary request failed");
    binary_endpoint.assert();
    assert_eq!(200, binary_response.status());
    assert_eq!("Binary OK", body_to_string(binary_response).await);
}

#[tokio::test]
async fn test_http_compress() {
    // Start app server
    let app_server = MockServer::start();
    let hello = app_server.mock(|when, then| {
        when.method(GET).path("/hello");
        then.status(200)
            .header("content-type", "text/plain")
            .body("Hello World Hello World Hello World Hello World Hello World");
    });

    // Initialize adapter
    let adapter = Adapter::new(&AdapterOptions {
        host: app_server.host(),
        port: app_server.port().to_string(),
        readiness_check_port: app_server.port().to_string(),
        readiness_check_path: "/healthcheck".to_string(),
        compression: true,
        ..Default::default()
    })
    .expect("Failed to create adapter");

    let mut svc = ServiceBuilder::new().layer(CompressionLayer::new()).service(adapter);

    // // Call the adapter service with basic request
    let req = LambdaEventBuilder::new()
        .with_path("/hello")
        .with_header("accept-encoding", "gzip")
        .build();

    // We convert to Request object because it allows us to add
    // the lambda Context
    let mut request = Request::from(req);
    add_lambda_context_to_request(&mut request);

    let response = svc.call(request).await.expect("Request failed");

    // Assert endpoint was called once
    hello.assert();

    // and response has expected content
    assert_eq!(200, response.status());
    assert_eq!(response.headers().get("content-encoding").unwrap(), "gzip");
    assert_eq!(
        "Hello World Hello World Hello World Hello World Hello World",
        compressed_body_to_string(response).await
    );
}

#[tokio::test]
async fn test_http_compress_disallowed_type() {
    // Start app server
    let app_server = MockServer::start();
    let hello = app_server.mock(|when, then| {
        when.method(GET).path("/hello");
        then.status(200)
            .header("content-type", "application/octet-stream")
            .body("Hello World Hello World Hello World Hello World Hello World");
    });

    // Initialize adapter
    let mut adapter = Adapter::new(&AdapterOptions {
        host: app_server.host(),
        port: app_server.port().to_string(),
        readiness_check_port: app_server.port().to_string(),
        readiness_check_path: "/healthcheck".to_string(),
        compression: true,
        ..Default::default()
    })
    .expect("Failed to create adapter");

    // // Call the adapter service with basic request
    let req = LambdaEventBuilder::new()
        .with_path("/hello")
        .with_header("accept-encoding", "gzip")
        .build();

    // We convert to Request object because it allows us to add
    // the lambda Context
    let mut request = Request::from(req);
    add_lambda_context_to_request(&mut request);

    let response = adapter.call(request).await.expect("Request failed");

    // Assert endpoint was called once
    hello.assert();

    // and response has expected content
    assert_eq!(200, response.status());
    assert_eq!(response.headers().get("content-length").unwrap(), "59"); // uncompressed: 59
    assert!(!response.headers().contains_key("content-encoding"));
    assert_eq!(
        "Hello World Hello World Hello World Hello World Hello World",
        body_to_string(response).await
    );
}

#[tokio::test]
async fn test_http_compress_already_compressed() {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(b"Hello World Hello World Hello World Hello World Hello World")
        .unwrap();
    let gzipped_body = encoder.finish().unwrap();

    // Start app server
    let app_server = MockServer::start();
    let hello = app_server.mock(|when, then| {
        when.method(GET).path("/hello");
        then.status(200).header("content-encoding", "gzip").body(&gzipped_body);
    });

    // Initialize adapter
    let adapter = Adapter::new(&AdapterOptions {
        host: app_server.host(),
        port: app_server.port().to_string(),
        readiness_check_port: app_server.port().to_string(),
        readiness_check_path: "/healthcheck".to_string(),
        compression: true,
        ..Default::default()
    })
    .expect("Failed to create adapter");

    let mut svc = ServiceBuilder::new().layer(CompressionLayer::new()).service(adapter);

    // Call the adapter service with basic request
    let req = LambdaEventBuilder::new()
        .with_path("/hello")
        .with_header("accept-encoding", "gzip")
        .build();

    // We convert to Request object because it allows us to add
    // the lambda Context
    let mut request = Request::from(req);
    add_lambda_context_to_request(&mut request);

    let response = svc.call(request).await.expect("Request failed");

    // Assert endpoint was called once
    hello.assert();

    // and response has expected content
    assert_eq!(200, response.status());
    assert_eq!(response.headers().get("content-length").unwrap(), "48"); // uncompressed: 59
    assert_eq!(response.headers().get("content-encoding").unwrap(), "gzip");
    assert_eq!(
        "Hello World Hello World Hello World Hello World Hello World",
        compressed_body_to_string(response).await
    );
}

#[tokio::test]
async fn test_http_context_headers() {
    // Start app server
    let app_server = MockServer::start();

    // An endpoint that expects and returns headers
    let test_endpoint = app_server.mock(|when, then| {
        when.method(GET)
            .path("/")
            .header_exists("x-amzn-lambda-context")
            .header_exists("x-amzn-request-context");
        then.status(200).header("fizz", "buzz").body("OK");
    });

    // Initialize adapter and do readiness check
    let mut adapter = Adapter::new(&AdapterOptions {
        host: app_server.host(),
        port: app_server.port().to_string(),
        readiness_check_port: app_server.port().to_string(),
        readiness_check_path: "/healthcheck".to_string(),
        ..Default::default()
    })
    .expect("Failed to create adapter");

    // Prepare request
    let req = LambdaEventBuilder::new().with_path("/").build();

    // We convert to Request object because it allows us to add
    // the Lambda Context
    let mut request = Request::from(req);
    add_lambda_context_to_request(&mut request);

    // Call the adapter service with request
    let response = adapter.call(request).await.expect("Request failed");

    // Assert endpoint was called once
    test_endpoint.assert();

    // and response has expected content
    assert_eq!(200, response.status());
    assert!(response.headers().contains_key("fizz"));
    assert_eq!("buzz", response.headers().get("fizz").unwrap());
    assert_eq!("OK", body_to_string(response).await);
}

#[tokio::test]
async fn test_http_content_encoding_suffix() {
    // Start app server
    let app_server = MockServer::start();

    let json_data = json!({
        "name": "John Doe",
        "age": 43,
        "gender": "Male"
    })
    .to_string();

    // An endpoint that expects and returns headers
    let test_endpoint = app_server.mock(|when, then| {
        when.method(GET).path("/json");
        then.status(200)
            .header("content-type", "application/graphql-response+json; charset=utf-8")
            .body(&json_data);
    });

    // Initialize adapter and do readiness check
    let mut adapter = Adapter::new(&AdapterOptions {
        host: app_server.host(),
        port: app_server.port().to_string(),
        readiness_check_port: app_server.port().to_string(),
        readiness_check_path: "/healthcheck".to_string(),
        ..Default::default()
    })
    .expect("Failed to create adapter");

    // Prepare request
    let req = LambdaEventBuilder::new().with_path("/json").build();

    // We convert to Request object because it allows us to add
    // the Lambda Context
    let mut request = Request::from(req);
    add_lambda_context_to_request(&mut request);

    // Call the adapter service with request
    let response = adapter.call(request).await.expect("Request failed");

    // Assert endpoint was called once
    test_endpoint.assert();

    // and response has expected content
    assert_eq!(200, response.status());
    assert!(response.headers().contains_key("content-type"));
    assert_eq!(
        "application/graphql-response+json; charset=utf-8",
        response.headers().get("content-type").unwrap()
    );
    assert_eq!(json_data.to_owned(), body_to_string(response).await);
}

#[tokio::test]
async fn test_http_error_status_codes() {
    // Start app server
    let app_server = MockServer::start();
    let error_endpoint = app_server.mock(|when, then| {
        when.method(GET).path("/error");
        then.status(502).body("Bad Gateway");
    });

    // Initialize adapter with error status codes
    let mut adapter = Adapter::new(&AdapterOptions {
        host: app_server.host(),
        port: app_server.port().to_string(),
        readiness_check_port: app_server.port().to_string(),
        readiness_check_path: "/healthcheck".to_string(),
        error_status_codes: Some(vec![500, 502, 503, 504]),
        ..Default::default()
    })
    .expect("Failed to create adapter");

    // Call the adapter service with request that should trigger error
    let req = LambdaEventBuilder::new().with_path("/error").build();
    let mut request = Request::from(req);
    add_lambda_context_to_request(&mut request);

    let result = adapter.call(request).await;
    assert!(result.is_err(), "Expected error response for status code 502");
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("502"), "Error should contain status code");
    assert!(err_msg.contains("Bad Gateway"), "Error should preserve response body");

    // Assert endpoint was called
    error_endpoint.assert();
}

#[tokio::test]
async fn test_http_authorization_source() {
    // Start app server
    let app_server = MockServer::start();
    let hello = app_server.mock(|when, then| {
        when.method(GET).path("/hello").header_exists("Authorization");
        then.status(200).body("Hello World");
    });

    // Initialize adapter
    let mut adapter = Adapter::new(&AdapterOptions {
        host: app_server.host(),
        port: app_server.port().to_string(),
        readiness_check_port: app_server.port().to_string(),
        readiness_check_path: "/healthcheck".to_string(),
        authorization_source: Some("auth-token".to_string()),
        ..Default::default()
    })
    .expect("Failed to create adapter");

    // // Call the adapter service with basic request
    let req = LambdaEventBuilder::new()
        .with_path("/hello")
        .with_header("auth-token", "Bearer token")
        .build();

    // We convert to Request object because it allows us to add
    // the lambda Context
    let mut request = Request::from(req);
    add_lambda_context_to_request(&mut request);

    let response = adapter.call(request).await.expect("Request failed");

    // Assert endpoint was called once
    hello.assert();

    // and response has expected content
    assert_eq!(200, response.status());
    assert_eq!(response.headers().get("content-length").unwrap(), "11");
    assert_eq!("Hello World", body_to_string(response).await);
}

#[tokio::test]
async fn test_http_context_multi_headers() {
    // Start app server
    let app_server = MockServer::start();

    // An endpoint that expects and returns headers
    let test_endpoint = app_server.mock(|when, then| {
        when.method(GET)
            .path("/")
            .is_true(|req| {
                req.headers()
                    .iter()
                    .filter(|(key, _value)| *key == "x-amzn-lambda-context")
                    .count()
                    == 1
            })
            .is_true(|req| {
                req.headers()
                    .iter()
                    .filter(|(key, _value)| *key == "x-amzn-request-context")
                    .count()
                    == 1
            });
        then.status(200).header("fizz", "buzz").body("OK");
    });

    // Initialize adapter and do readiness check
    let mut adapter = Adapter::new(&AdapterOptions {
        host: app_server.host(),
        port: app_server.port().to_string(),
        readiness_check_port: app_server.port().to_string(),
        readiness_check_path: "/healthcheck".to_string(),
        ..Default::default()
    })
    .expect("Failed to create adapter");

    // Prepare request
    let req = LambdaEventBuilder::new()
        .with_path("/")
        .with_header("x-amzn-lambda-context", "header_from_client_1")
        .with_header("x-amzn-lambda-context", "header_from_client_2")
        .with_header("x-amzn-request-context", "header_from_client_1")
        .with_header("x-amzn-request-context", "header_from_client_2")
        .build();

    // We convert to Request object because it allows us to add
    // the Lambda Context
    let mut request = Request::from(req);
    add_lambda_context_to_request(&mut request);

    // Call the adapter service with request
    let response = adapter.call(request).await.expect("Request failed");

    // Assert endpoint was called once
    test_endpoint.assert();

    // and response has expected content
    assert_eq!(200, response.status());
    assert!(response.headers().contains_key("fizz"));
    assert_eq!("buzz", response.headers().get("fizz").unwrap());
    assert_eq!("OK", body_to_string(response).await);
}

#[tokio::test]
async fn test_http_base_path_stripping() {
    // Start app server
    let app_server = MockServer::start();

    // The app expects requests at /api/data (without the base path prefix)
    let test_endpoint = app_server.mock(|when, then| {
        when.method(GET).path("/api/data");
        then.status(200).body("Base path stripped");
    });

    // Initialize adapter with base_path set
    let mut adapter = Adapter::new(&AdapterOptions {
        host: app_server.host(),
        port: app_server.port().to_string(),
        readiness_check_port: app_server.port().to_string(),
        readiness_check_path: "/healthcheck".to_string(),
        base_path: Some("/prod".to_string()),
        ..Default::default()
    })
    .expect("Failed to create adapter");

    // Send request with the base path prefix — adapter should strip it
    let req = LambdaEventBuilder::new().with_path("/prod/api/data").build();
    let mut request = Request::from(req);
    add_lambda_context_to_request(&mut request);

    let response = adapter.call(request).await.expect("Request failed");

    test_endpoint.assert();
    assert_eq!(200, response.status());
    assert_eq!("Base path stripped", body_to_string(response).await);
}

#[tokio::test]
async fn test_http_base_path_no_match() {
    // Start app server
    let app_server = MockServer::start();

    // When path doesn't start with base_path, it should pass through unchanged
    let test_endpoint = app_server.mock(|when, then| {
        when.method(GET).path("/other/path");
        then.status(200).body("No stripping");
    });

    let mut adapter = Adapter::new(&AdapterOptions {
        host: app_server.host(),
        port: app_server.port().to_string(),
        readiness_check_port: app_server.port().to_string(),
        readiness_check_path: "/healthcheck".to_string(),
        base_path: Some("/prod".to_string()),
        ..Default::default()
    })
    .expect("Failed to create adapter");

    let req = LambdaEventBuilder::new().with_path("/other/path").build();
    let mut request = Request::from(req);
    add_lambda_context_to_request(&mut request);

    let response = adapter.call(request).await.expect("Request failed");

    test_endpoint.assert();
    assert_eq!(200, response.status());
    assert_eq!("No stripping", body_to_string(response).await);
}

#[tokio::test]
async fn test_http_base_path_root_after_strip() {
    // Start app server
    let app_server = MockServer::start();

    // When the path equals the base path exactly, the result should be empty string
    // which url crate normalizes to "/"
    let test_endpoint = app_server.mock(|when, then| {
        when.method(GET).path("/");
        then.status(200).body("Root");
    });

    let mut adapter = Adapter::new(&AdapterOptions {
        host: app_server.host(),
        port: app_server.port().to_string(),
        readiness_check_port: app_server.port().to_string(),
        readiness_check_path: "/healthcheck".to_string(),
        base_path: Some("/prod".to_string()),
        ..Default::default()
    })
    .expect("Failed to create adapter");

    let req = LambdaEventBuilder::new().with_path("/prod").build();
    let mut request = Request::from(req);
    add_lambda_context_to_request(&mut request);

    let response = adapter.call(request).await.expect("Request failed");

    test_endpoint.assert();
    assert_eq!(200, response.status());
}

#[test]
fn test_deprecated_env_var_fallback() {
    // Clear all LWA-prefixed vars
    env::remove_var("AWS_LWA_PORT");
    env::remove_var("AWS_LWA_HOST");
    env::remove_var("AWS_LWA_READINESS_CHECK_PORT");
    env::remove_var("AWS_LWA_READINESS_CHECK_PATH");
    env::remove_var("AWS_LWA_READINESS_CHECK_PROTOCOL");
    env::remove_var("AWS_LWA_REMOVE_BASE_PATH");
    env::remove_var("AWS_LWA_ASYNC_INIT");
    env::remove_var("AWS_LWA_ENABLE_COMPRESSION");
    env::remove_var("AWS_LWA_INVOKE_MODE");
    env::remove_var("AWS_LWA_AUTHORIZATION_SOURCE");
    env::remove_var("AWS_LWA_READINESS_CHECK_HEALTHY_STATUS");

    // Set only deprecated (non-prefixed) env vars
    env::set_var("PORT", "4000");
    env::set_var("HOST", "0.0.0.0");
    env::set_var("READINESS_CHECK_PORT", "4001");
    env::set_var("READINESS_CHECK_PATH", "/ready");
    env::set_var("READINESS_CHECK_PROTOCOL", "TCP");
    env::set_var("REMOVE_BASE_PATH", "/stage");
    env::set_var("ASYNC_INIT", "true");

    let options = AdapterOptions::default();

    assert_eq!("4000", options.port);
    assert_eq!("0.0.0.0", options.host);
    assert_eq!("4001", options.readiness_check_port);
    assert_eq!("/ready", options.readiness_check_path);
    assert_eq!(Protocol::Tcp, options.readiness_check_protocol);
    assert_eq!(Some("/stage".into()), options.base_path);
    assert!(options.async_init);

    // Clean up
    env::remove_var("PORT");
    env::remove_var("HOST");
    env::remove_var("READINESS_CHECK_PORT");
    env::remove_var("READINESS_CHECK_PATH");
    env::remove_var("READINESS_CHECK_PROTOCOL");
    env::remove_var("REMOVE_BASE_PATH");
    env::remove_var("ASYNC_INIT");
}

#[test]
fn test_namespaced_env_overrides_deprecated() {
    // Set both deprecated and new vars — new should win
    env::set_var("PORT", "4000");
    env::set_var("AWS_LWA_PORT", "5000");
    env::set_var("HOST", "0.0.0.0");
    env::set_var("AWS_LWA_HOST", "localhost");

    let options = AdapterOptions::default();

    assert_eq!("5000", options.port);
    assert_eq!("localhost", options.host);

    // Clean up
    env::remove_var("PORT");
    env::remove_var("AWS_LWA_PORT");
    env::remove_var("HOST");
    env::remove_var("AWS_LWA_HOST");
}

#[tokio::test]
async fn test_http_authorization_source_missing_header() {
    // Start app server
    let app_server = MockServer::start();

    // The endpoint should still be called, just without Authorization header
    let test_endpoint = app_server.mock(|when, then| {
        when.method(GET)
            .path("/hello")
            .is_true(|req| !req.headers().iter().any(|(k, _)| k == "authorization"));
        then.status(200).body("No Auth");
    });

    // Configure adapter with authorization_source pointing to a header that won't exist
    let mut adapter = Adapter::new(&AdapterOptions {
        host: app_server.host(),
        port: app_server.port().to_string(),
        readiness_check_port: app_server.port().to_string(),
        readiness_check_path: "/healthcheck".to_string(),
        authorization_source: Some("x-missing-auth".to_string()),
        ..Default::default()
    })
    .expect("Failed to create adapter");

    // Send request WITHOUT the x-missing-auth header
    let req = LambdaEventBuilder::new().with_path("/hello").build();
    let mut request = Request::from(req);
    add_lambda_context_to_request(&mut request);

    let response = adapter.call(request).await.expect("Request failed");

    test_endpoint.assert();
    assert_eq!(200, response.status());
    assert_eq!("No Auth", body_to_string(response).await);
}

#[tokio::test]
async fn test_http_error_status_codes_non_matching() {
    // Start app server
    let app_server = MockServer::start();
    let endpoint = app_server.mock(|when, then| {
        when.method(GET).path("/ok");
        then.status(404).body("Not Found");
    });

    // Configure error_status_codes with 500-504 only — 404 should pass through normally
    let mut adapter = Adapter::new(&AdapterOptions {
        host: app_server.host(),
        port: app_server.port().to_string(),
        readiness_check_port: app_server.port().to_string(),
        readiness_check_path: "/healthcheck".to_string(),
        error_status_codes: Some(vec![500, 502, 503, 504]),
        ..Default::default()
    })
    .expect("Failed to create adapter");

    let req = LambdaEventBuilder::new().with_path("/ok").build();
    let mut request = Request::from(req);
    add_lambda_context_to_request(&mut request);

    let response = adapter
        .call(request)
        .await
        .expect("Request should succeed for non-error status");

    endpoint.assert();
    assert_eq!(404, response.status());
    assert_eq!("Not Found", body_to_string(response).await);
}

#[tokio::test]
async fn test_http_async_init_ready_at_init() {
    // Start app server
    let app_server = MockServer::start();
    let healthcheck = app_server.mock(|when, then| {
        when.method(GET).path("/healthcheck");
        then.status(200).body("OK");
    });
    let hello = app_server.mock(|when, then| {
        when.method(GET).path("/hello");
        then.status(200).body("Hello");
    });

    // Initialize adapter with async_init enabled
    let mut adapter = Adapter::new(&AdapterOptions {
        host: app_server.host(),
        port: app_server.port().to_string(),
        readiness_check_port: app_server.port().to_string(),
        readiness_check_path: "/healthcheck".to_string(),
        async_init: true,
        ..Default::default()
    })
    .expect("Failed to create adapter");

    // Perform init health check — app is already running so it should succeed
    adapter.check_init_health().await;

    healthcheck.assert();

    // Now send a request — should work without re-checking readiness
    let req = LambdaEventBuilder::new().with_path("/hello").build();
    let mut request = Request::from(req);
    add_lambda_context_to_request(&mut request);

    let response = adapter.call(request).await.expect("Request failed");

    hello.assert();
    assert_eq!(200, response.status());
    assert_eq!("Hello", body_to_string(response).await);
}

#[tokio::test]
async fn test_http_tcp_readiness_check() {
    // Start a mock server (which listens on TCP)
    let app_server = MockServer::start();

    // Initialize adapter with TCP readiness check
    let mut adapter = Adapter::new(&AdapterOptions {
        host: app_server.host(),
        port: app_server.port().to_string(),
        readiness_check_port: app_server.port().to_string(),
        readiness_check_path: "/".to_string(),
        readiness_check_protocol: Protocol::Tcp,
        ..Default::default()
    })
    .expect("Failed to create adapter");

    // TCP readiness check should succeed since MockServer is listening
    adapter.check_init_health().await;

    // Now verify the adapter can still forward requests
    let hello = app_server.mock(|when, then| {
        when.method(GET).path("/hello");
        then.status(200).body("TCP Ready");
    });

    let req = LambdaEventBuilder::new().with_path("/hello").build();
    let mut request = Request::from(req);
    add_lambda_context_to_request(&mut request);

    let response = adapter.call(request).await.expect("Request failed");

    hello.assert();
    assert_eq!(200, response.status());
    assert_eq!("TCP Ready", body_to_string(response).await);
}

async fn body_to_string(res: Response<Incoming>) -> String {
    let body_bytes = res.collect().await.unwrap().to_bytes();
    String::from_utf8_lossy(&body_bytes).to_string()
}

async fn compressed_body_to_string(res: Response<CompressionBody<Incoming>>) -> String {
    let body_bytes = res.collect().await.unwrap().to_bytes();
    decode_reader(&body_bytes).unwrap()
}

fn decode_reader(bytes: &[u8]) -> io::Result<String> {
    let mut gz = GzDecoder::new(bytes);
    let mut s = String::new();
    gz.read_to_string(&mut s)?;
    Ok(s)
}

fn add_lambda_context_to_request(request: &mut Request<Body>) {
    // create a HeaderMap to build the lambda context
    let mut headers = HeaderMap::new();
    headers.insert("lambda-runtime-aws-request-id", "my_id".parse().unwrap());
    headers.insert("lambda-runtime-deadline-ms", "123".parse().unwrap());
    headers.insert("lambda-runtime-client-context", "{}".parse().unwrap());

    let conf = Config {
        function_name: "test_function".into(),
        memory: 128,
        version: "latest".into(),
        log_stream: "/aws/lambda/test_function".into(),
        log_group: "2023/09/15/[$LATEST]ab831cef03e94457a94b6efcbe22406a".into(),
    };

    // converts HeaderMap to Context
    let context = Context::new("my_id", Arc::new(conf), &headers).expect("Couldn't convert HeaderMap to Context");

    // add Context to the request
    request.extensions_mut().insert(context);
}

#[tokio::test]
async fn test_concurrent_request_forwarding() {
    let app_server = MockServer::start();
    let endpoint_a = app_server.mock(|when, then| {
        when.method(GET).path("/a");
        then.status(200).body("response_a");
    });
    let endpoint_b = app_server.mock(|when, then| {
        when.method(GET).path("/b");
        then.status(200).body("response_b");
    });

    let adapter = Adapter::new(&AdapterOptions {
        host: app_server.host(),
        port: app_server.port().to_string(),
        readiness_check_port: app_server.port().to_string(),
        readiness_check_path: "/".to_string(),
        ..Default::default()
    })
    .expect("Failed to create adapter");

    // Fire 4 concurrent requests through cloned adapters
    let mut join_set = tokio::task::JoinSet::new();
    for path in ["/a", "/b", "/a", "/b"] {
        let mut a = adapter.clone();
        let req = LambdaEventBuilder::new().with_path(path).build();
        let mut request = Request::from(req);
        add_lambda_context_to_request(&mut request);
        join_set.spawn(async move { (path.to_string(), a.call(request).await) });
    }

    while let Some(result) = join_set.join_next().await {
        let (path, response) = result.expect("Task panicked");
        let response = response.expect("Request failed");
        assert_eq!(200, response.status());
        let expected_body = format!("response_{}", &path[1..]);
        assert_eq!(expected_body, body_to_string(response).await);
    }

    endpoint_a.assert_calls(2);
    endpoint_b.assert_calls(2);
}

#[tokio::test]
async fn test_concurrent_post_body_isolation() {
    let app_server = MockServer::start();

    // Endpoints that echo back a specific field to verify body isolation
    let json_a = app_server.mock(|when, then| {
        when.method(POST).path("/submit").body(r#"{"id":"a"}"#);
        then.status(200).body("ack_a");
    });
    let json_b = app_server.mock(|when, then| {
        when.method(POST).path("/submit").body(r#"{"id":"b"}"#);
        then.status(200).body("ack_b");
    });

    let adapter = Adapter::new(&AdapterOptions {
        host: app_server.host(),
        port: app_server.port().to_string(),
        readiness_check_port: app_server.port().to_string(),
        readiness_check_path: "/".to_string(),
        ..Default::default()
    })
    .expect("Failed to create adapter");

    let mut join_set = tokio::task::JoinSet::new();
    for (body, expected) in [
        (r#"{"id":"a"}"#, "ack_a"),
        (r#"{"id":"b"}"#, "ack_b"),
        (r#"{"id":"a"}"#, "ack_a"),
        (r#"{"id":"b"}"#, "ack_b"),
    ] {
        let mut a = adapter.clone();
        let req = LambdaEventBuilder::new()
            .with_method(Method::POST)
            .with_path("/submit")
            .with_body(body)
            .build();
        let mut request = Request::from(req);
        add_lambda_context_to_request(&mut request);
        let expected = expected.to_string();
        join_set.spawn(async move { (expected, a.call(request).await) });
    }

    while let Some(result) = join_set.join_next().await {
        let (expected, response) = result.expect("Task panicked");
        let response = response.expect("Request failed");
        assert_eq!(200, response.status());
        assert_eq!(expected, body_to_string(response).await);
    }

    json_a.assert_calls(2);
    json_b.assert_calls(2);
}

fn add_lambda_context_with_tenant(request: &mut Request<Body>, tenant_id: &str) {
    let mut headers = HeaderMap::new();
    headers.insert("lambda-runtime-aws-request-id", "my_id".parse().unwrap());
    headers.insert("lambda-runtime-deadline-ms", "123".parse().unwrap());
    headers.insert("lambda-runtime-client-context", "{}".parse().unwrap());
    headers.insert("lambda-runtime-aws-tenant-id", tenant_id.parse().unwrap());

    let conf = Config {
        function_name: "test_function".into(),
        memory: 128,
        version: "latest".into(),
        log_stream: "/aws/lambda/test_function".into(),
        log_group: "2023/09/15/[$LATEST]ab831cef03e94457a94b6efcbe22406a".into(),
    };

    let context = Context::new("my_id", Arc::new(conf), &headers).expect("Couldn't convert HeaderMap to Context");
    request.extensions_mut().insert(context);
}

#[tokio::test]
async fn test_tenant_id_propagated_from_client_context() {
    let app_server = MockServer::start();
    let endpoint = app_server.mock(|when, then| {
        when.method(GET)
            .path("/hello")
            .header("x-amz-tenant-id", "tenant-abc-123");
        then.status(200).body("OK");
    });

    let mut adapter = Adapter::new(&AdapterOptions {
        host: app_server.host(),
        port: app_server.port().to_string(),
        readiness_check_port: app_server.port().to_string(),
        readiness_check_path: "/healthcheck".to_string(),
        ..Default::default()
    })
    .expect("Failed to create adapter");

    let req = LambdaEventBuilder::new().with_path("/hello").build();
    let mut request = Request::from(req);
    add_lambda_context_with_tenant(&mut request, "tenant-abc-123");

    let response = adapter.call(request).await.expect("Request failed");

    endpoint.assert();
    assert_eq!(200, response.status());
}

#[tokio::test]
async fn test_no_tenant_id_header_when_absent() {
    let app_server = MockServer::start();
    let endpoint = app_server.mock(|when, then| {
        when.method(GET)
            .path("/hello")
            .is_true(|req| !req.headers().iter().any(|(k, _)| k == "x-amz-tenant-id"));
        then.status(200).body("OK");
    });

    let mut adapter = Adapter::new(&AdapterOptions {
        host: app_server.host(),
        port: app_server.port().to_string(),
        readiness_check_port: app_server.port().to_string(),
        readiness_check_path: "/healthcheck".to_string(),
        ..Default::default()
    })
    .expect("Failed to create adapter");

    let req = LambdaEventBuilder::new().with_path("/hello").build();
    let mut request = Request::from(req);
    add_lambda_context_to_request(&mut request);

    let response = adapter.call(request).await.expect("Request failed");

    endpoint.assert();
    assert_eq!(200, response.status());
}
