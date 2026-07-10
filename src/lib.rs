// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

//! # Lambda Web Adapter
//!
//! Lambda Web Adapter allows you to run web applications on AWS Lambda without code changes.
//! It acts as a bridge between the Lambda Runtime API and your web application, translating
//! Lambda events into HTTP requests and forwarding them to your application.
//!
//! ## Overview
//!
//! The adapter works by:
//! 1. Starting as a Lambda extension alongside your web application
//! 2. Waiting for your application to become ready (via health checks)
//! 3. Receiving Lambda events and converting them to HTTP requests
//! 4. Forwarding requests to your application and returning responses to Lambda
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use lambda_web_adapter::{Adapter, AdapterOptions, Error};
//!
//! fn main() -> Result<(), Error> {
//!     // Apply proxy config before starting tokio runtime
//!     Adapter::apply_runtime_proxy_config();
//!
//!     let runtime = tokio::runtime::Builder::new_multi_thread()
//!         .enable_all()
//!         .build()?;
//!
//!     runtime.block_on(async {
//!         let options = AdapterOptions::default();
//!         let mut adapter = Adapter::new(&options)?;
//!         
//!         adapter.register_default_extension();
//!         adapter.check_init_health().await;
//!         adapter.run().await
//!     })
//! }
//! ```
//!
//! ## Configuration
//!
//! The adapter is configured via environment variables. All variables use the `AWS_LWA_` prefix:
//!
//! | Variable | Description | Default |
//! |----------|-------------|---------|
//! | `AWS_LWA_PORT` | Port your application listens on (falls back to `PORT`) | `8080` |
//! | `AWS_LWA_HOST` | Host your application binds to | `127.0.0.1` |
//! | `AWS_LWA_READINESS_CHECK_PATH` | Health check endpoint path | `/` |
//! | `AWS_LWA_READINESS_CHECK_PORT` | Health check port | Same as `AWS_LWA_PORT` |
//! | `AWS_LWA_READINESS_CHECK_PROTOCOL` | Protocol for health checks (`HTTP` or `TCP`) | `HTTP` |
//! | `AWS_LWA_READINESS_CHECK_HEALTHY_STATUS` | Status codes considered healthy (e.g., `200-399,404`) | `100-499` |
//! | `AWS_LWA_ASYNC_INIT` | Enable async initialization | `false` |
//! | `AWS_LWA_REMOVE_BASE_PATH` | Base path to strip from requests | None |
//! | `AWS_LWA_INVOKE_MODE` | Lambda invoke mode (`buffered` or `response_stream`) | `buffered` |
//! | `AWS_LWA_ENABLE_COMPRESSION` | Enable response compression (buffered mode only) | `false` |
//!
//! ## Response Streaming
//!
//! For applications that need to stream responses (e.g., Server-Sent Events, large file downloads),
//! set `AWS_LWA_INVOKE_MODE=response_stream`. This requires configuring your Lambda function URL
//! with `InvokeMode: RESPONSE_STREAM`.

mod readiness;

// Environment variable names (AWS_LWA_ prefix)
const ENV_PORT: &str = "AWS_LWA_PORT";
const ENV_HOST: &str = "AWS_LWA_HOST";
const ENV_READINESS_CHECK_PORT: &str = "AWS_LWA_READINESS_CHECK_PORT";
const ENV_READINESS_CHECK_PATH: &str = "AWS_LWA_READINESS_CHECK_PATH";
const ENV_READINESS_CHECK_PROTOCOL: &str = "AWS_LWA_READINESS_CHECK_PROTOCOL";
const ENV_READINESS_CHECK_HEALTHY_STATUS: &str = "AWS_LWA_READINESS_CHECK_HEALTHY_STATUS";
const ENV_REMOVE_BASE_PATH: &str = "AWS_LWA_REMOVE_BASE_PATH";
const ENV_PASS_THROUGH_PATH: &str = "AWS_LWA_PASS_THROUGH_PATH";
const ENV_ASYNC_INIT: &str = "AWS_LWA_ASYNC_INIT";
const ENV_ENABLE_COMPRESSION: &str = "AWS_LWA_ENABLE_COMPRESSION";
const ENV_INVOKE_MODE: &str = "AWS_LWA_INVOKE_MODE";
const ENV_AUTHORIZATION_SOURCE: &str = "AWS_LWA_AUTHORIZATION_SOURCE";
const ENV_ERROR_STATUS_CODES: &str = "AWS_LWA_ERROR_STATUS_CODES";
const ENV_LAMBDA_RUNTIME_API_PROXY: &str = "AWS_LWA_LAMBDA_RUNTIME_API_PROXY";

// Deprecated environment variable names (without prefix)
const ENV_PORT_DEPRECATED: &str = "PORT";
const ENV_HOST_DEPRECATED: &str = "HOST";
const ENV_READINESS_CHECK_PORT_DEPRECATED: &str = "READINESS_CHECK_PORT";
const ENV_READINESS_CHECK_PATH_DEPRECATED: &str = "READINESS_CHECK_PATH";
const ENV_READINESS_CHECK_PROTOCOL_DEPRECATED: &str = "READINESS_CHECK_PROTOCOL";
const ENV_REMOVE_BASE_PATH_DEPRECATED: &str = "REMOVE_BASE_PATH";
const ENV_ASYNC_INIT_DEPRECATED: &str = "ASYNC_INIT";

// Lambda runtime environment variable
const ENV_LAMBDA_RUNTIME_API: &str = "AWS_LAMBDA_RUNTIME_API";

// Captures the original AWS_LAMBDA_RUNTIME_API value before apply_runtime_proxy_config()
// overwrites it with the proxy address. Extension registration uses the original so it
// reaches the real Lambda Runtime API directly, bypassing the proxy.
// Outer Option distinguishes "not yet captured" (None) from "captured but env was unset"
// (Some(None)).
static ORIGINAL_LAMBDA_RUNTIME_API: OnceLock<Option<String>> = OnceLock::new();

use http::{
    header::{HeaderName, HeaderValue},
    Method, StatusCode,
};
use http_body::Body as HttpBody;
use http_body_util::BodyExt;
use hyper::body::Incoming;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;
use lambda_http::request::RequestContext;
pub use lambda_http::tracing;
use lambda_http::Body;
pub use lambda_http::Error;
use lambda_http::{Request, RequestExt, Response};
use readiness::Checkpoint;
use std::borrow::Cow;
use std::fmt::Debug;
use std::{
    env,
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, OnceLock,
    },
    time::Duration,
};
use tokio::{net::TcpStream, time::timeout};
use tokio_retry::{strategy::FixedInterval, Retry};
use tower::{Service, ServiceBuilder};
use tower_http::compression::CompressionLayer;
use url::Url;

/// Protocol used for readiness checks.
///
/// The adapter supports two protocols for checking if your web application is ready:
///
/// - [`Protocol::Http`] - Performs an HTTP GET request and checks the response status code
/// - [`Protocol::Tcp`] - Attempts a TCP connection to verify the port is listening
///
/// # Examples
///
/// ```rust
/// use lambda_web_adapter::Protocol;
///
/// // Parse from string (case-insensitive)
/// let http: Protocol = "http".into();
/// let tcp: Protocol = "TCP".into();
///
/// assert_eq!(http, Protocol::Http);
/// assert_eq!(tcp, Protocol::Tcp);
/// ```
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Protocol {
    /// HTTP protocol - performs GET request and validates response status.
    /// This is the default and recommended protocol for most applications.
    #[default]
    Http,
    /// TCP protocol - only checks if a TCP connection can be established.
    /// Useful for applications that don't have an HTTP health endpoint.
    Tcp,
}

impl From<&str> for Protocol {
    fn from(value: &str) -> Self {
        match value.to_lowercase().as_str() {
            "http" => Protocol::Http,
            "tcp" => Protocol::Tcp,
            _ => Protocol::Http,
        }
    }
}

/// Lambda function invoke mode.
///
/// Controls how Lambda handles the response from your function:
///
/// - [`LambdaInvokeMode::Buffered`] - Lambda buffers the entire response before returning it
/// - [`LambdaInvokeMode::ResponseStream`] - Lambda streams the response as it's generated
///
/// # Response Streaming
///
/// Response streaming is useful for:
/// - Server-Sent Events (SSE)
/// - Large file downloads
/// - Real-time data feeds
/// - Reducing time-to-first-byte (TTFB)
///
/// To use response streaming, you must also configure your Lambda function URL
/// with `InvokeMode: RESPONSE_STREAM`.
///
/// # Examples
///
/// ```rust
/// use lambda_web_adapter::LambdaInvokeMode;
///
/// let buffered: LambdaInvokeMode = "buffered".into();
/// let streaming: LambdaInvokeMode = "response_stream".into();
///
/// assert_eq!(buffered, LambdaInvokeMode::Buffered);
/// assert_eq!(streaming, LambdaInvokeMode::ResponseStream);
/// ```
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LambdaInvokeMode {
    /// Buffered mode - Lambda buffers the entire response before returning.
    /// This is the default mode and works with all Lambda invocation methods.
    #[default]
    Buffered,
    /// Response streaming mode - Lambda streams the response as it's generated.
    /// Requires Lambda function URL with `InvokeMode: RESPONSE_STREAM`.
    ResponseStream,
}

impl From<&str> for LambdaInvokeMode {
    fn from(value: &str) -> Self {
        match value.to_lowercase().as_str() {
            "buffered" => LambdaInvokeMode::Buffered,
            "response_stream" => LambdaInvokeMode::ResponseStream,
            _ => LambdaInvokeMode::Buffered,
        }
    }
}

/// Configuration options for the Lambda Web Adapter.
///
/// This struct holds all configuration parameters for the adapter. It can be constructed
/// manually or using [`Default::default()`] which reads values from environment variables.
///
/// # Environment Variables
///
/// When using `Default::default()`, the following environment variables are read:
///
/// | Field | Environment Variable | Fallback | Default |
/// |-------|---------------------|----------|---------|
/// | `host` | `AWS_LWA_HOST` | `HOST` | `127.0.0.1` |
/// | `port` | `AWS_LWA_PORT` | `PORT` | `8080` |
/// | `readiness_check_port` | `AWS_LWA_READINESS_CHECK_PORT` | `READINESS_CHECK_PORT` | Same as `port` |
/// | `readiness_check_path` | `AWS_LWA_READINESS_CHECK_PATH` | `READINESS_CHECK_PATH` | `/` |
/// | `readiness_check_protocol` | `AWS_LWA_READINESS_CHECK_PROTOCOL` | `READINESS_CHECK_PROTOCOL` | `HTTP` |
/// | `readiness_check_healthy_status` | `AWS_LWA_READINESS_CHECK_HEALTHY_STATUS` | - | `100-499` |
/// | `base_path` | `AWS_LWA_REMOVE_BASE_PATH` | `REMOVE_BASE_PATH` | None |
/// | `async_init` | `AWS_LWA_ASYNC_INIT` | `ASYNC_INIT` | `false` |
/// | `compression` | `AWS_LWA_ENABLE_COMPRESSION` | - | `false` |
/// | `invoke_mode` | `AWS_LWA_INVOKE_MODE` | - | `buffered` |
///
/// # Deprecated Environment Variables
///
/// The non-prefixed environment variables (e.g., `HOST`, `READINESS_CHECK_PORT`) are deprecated
/// and will be removed in version 2.0. Please use the `AWS_LWA_` prefixed versions.
/// Note: `PORT` is not deprecated and remains a supported fallback for `AWS_LWA_PORT`.
///
/// # Examples
///
/// ```rust
/// use lambda_web_adapter::{AdapterOptions, Protocol, LambdaInvokeMode};
///
/// // Use defaults from environment variables
/// let options = AdapterOptions::default();
///
/// // Or configure manually
/// let options = AdapterOptions {
///     host: "127.0.0.1".to_string(),
///     port: "3000".to_string(),
///     readiness_check_path: "/health".to_string(),
///     readiness_check_protocol: Protocol::Http,
///     invoke_mode: LambdaInvokeMode::ResponseStream,
///     ..Default::default()
/// };
/// ```
pub struct AdapterOptions {
    /// Host address where the web application is listening.
    /// Default: `127.0.0.1`
    pub host: String,

    /// Port where the web application is listening.
    /// Falls back to `PORT` env var, then default `8080`.
    pub port: String,

    /// Port to use for readiness checks. Defaults to the same as `port`.
    /// Useful when your application exposes health checks on a different port.
    pub readiness_check_port: String,

    /// HTTP path for readiness checks.
    /// Default: `/`
    pub readiness_check_path: String,

    /// Protocol to use for readiness checks.
    /// Default: [`Protocol::Http`]
    pub readiness_check_protocol: Protocol,

    /// List of HTTP status codes considered healthy for readiness checks.
    ///
    /// Can be configured via `AWS_LWA_READINESS_CHECK_HEALTHY_STATUS` using:
    /// - Single codes: `200,201,204`
    /// - Ranges: `200-399`
    /// - Mixed: `200-299,301,302,400-499`
    ///
    /// Default: `100-499` (all 1xx, 2xx, 3xx, and 4xx status codes)
    pub readiness_check_healthy_status: Vec<u16>,

    /// Base path to strip from incoming requests.
    ///
    /// Useful when your Lambda is behind an API Gateway with a stage name
    /// or custom path that your application doesn't expect.
    ///
    /// Example: If set to `/prod`, a request to `/prod/api/users` becomes `/api/users`.
    pub base_path: Option<String>,

    /// Path to forward pass-through events to.
    /// Default: `/events`
    pub pass_through_path: String,

    /// Enable async initialization mode.
    ///
    /// When `true`, the adapter will cancel readiness checks after ~9.8 seconds
    /// to avoid Lambda's 10-second init timeout. The application can continue
    /// booting in the background and will be checked again on the first request.
    ///
    /// Default: `false`
    pub async_init: bool,

    /// Enable response compression.
    ///
    /// When `true`, responses will be compressed using gzip, deflate, or brotli
    /// based on the `Accept-Encoding` header.
    ///
    /// Note: Compression is not supported with response streaming
    /// (`LambdaInvokeMode::ResponseStream`). If both are enabled, compression
    /// will be automatically disabled with a warning.
    ///
    /// Default: `false`
    pub compression: bool,

    /// Lambda invoke mode for response handling.
    /// Default: [`LambdaInvokeMode::Buffered`]
    pub invoke_mode: LambdaInvokeMode,

    /// Header name to copy to the `Authorization` header.
    ///
    /// Useful when your authorization token comes in a custom header
    /// (e.g., from API Gateway authorizers) and your application expects
    /// it in the standard `Authorization` header.
    pub authorization_source: Option<String>,

    /// HTTP status codes that should trigger a Lambda error response.
    ///
    /// When the web application returns one of these status codes,
    /// the adapter will return an error to Lambda instead of the response.
    /// This can be useful for triggering Lambda retry behavior.
    pub error_status_codes: Option<Vec<u16>>,
}

/// Helper to get env var with deprecation warning for old name
fn get_env_with_deprecation(new_name: &str, old_name: &str, default: &str) -> String {
    if let Ok(val) = env::var(new_name) {
        return val;
    }
    if let Ok(val) = env::var(old_name) {
        tracing::warn!(
            "Environment variable '{}' is deprecated and will be removed in version 2.0. Please use '{}' instead.",
            old_name,
            new_name
        );
        return val;
    }
    default.to_string()
}

/// Helper to get optional env var with deprecation warning for old name
fn get_optional_env_with_deprecation(new_name: &str, old_name: &str) -> Option<String> {
    if let Ok(val) = env::var(new_name) {
        return Some(val);
    }
    if let Ok(val) = env::var(old_name) {
        tracing::warn!(
            "Environment variable '{}' is deprecated and will be removed in version 2.0. Please use '{}' instead.",
            old_name,
            new_name
        );
        return Some(val);
    }
    None
}

impl Default for AdapterOptions {
    fn default() -> Self {
        let port = env::var(ENV_PORT)
            .or_else(|_| env::var(ENV_PORT_DEPRECATED))
            .unwrap_or_else(|_| "8080".to_string());

        // Handle readiness check healthy status codes
        let readiness_check_healthy_status = if let Ok(val) = env::var(ENV_READINESS_CHECK_HEALTHY_STATUS) {
            parse_status_codes(&val)
        } else {
            // Default: 100-499
            (100..500).collect()
        };

        AdapterOptions {
            host: get_env_with_deprecation(ENV_HOST, ENV_HOST_DEPRECATED, "127.0.0.1"),
            port: port.clone(),
            readiness_check_port: get_env_with_deprecation(
                ENV_READINESS_CHECK_PORT,
                ENV_READINESS_CHECK_PORT_DEPRECATED,
                &port,
            ),
            readiness_check_healthy_status,
            readiness_check_path: get_env_with_deprecation(
                ENV_READINESS_CHECK_PATH,
                ENV_READINESS_CHECK_PATH_DEPRECATED,
                "/",
            ),
            readiness_check_protocol: get_env_with_deprecation(
                ENV_READINESS_CHECK_PROTOCOL,
                ENV_READINESS_CHECK_PROTOCOL_DEPRECATED,
                "HTTP",
            )
            .as_str()
            .into(),
            base_path: get_optional_env_with_deprecation(ENV_REMOVE_BASE_PATH, ENV_REMOVE_BASE_PATH_DEPRECATED),
            pass_through_path: env::var(ENV_PASS_THROUGH_PATH).unwrap_or_else(|_| "/events".to_string()),
            async_init: get_env_with_deprecation(ENV_ASYNC_INIT, ENV_ASYNC_INIT_DEPRECATED, "false")
                .parse()
                .unwrap_or(false),
            compression: env::var(ENV_ENABLE_COMPRESSION)
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .unwrap_or(false),
            invoke_mode: env::var(ENV_INVOKE_MODE)
                .unwrap_or_else(|_| "buffered".to_string())
                .as_str()
                .into(),
            authorization_source: env::var(ENV_AUTHORIZATION_SOURCE).ok(),
            error_status_codes: env::var(ENV_ERROR_STATUS_CODES)
                .ok()
                .map(|codes| parse_status_codes(&codes)),
        }
    }
}

/// Parses a comma-separated string of status codes and ranges into a vector.
///
/// Supports:
/// - Single codes: `"200,201,204"` → `[200, 201, 204]`
/// - Ranges: `"200-299"` → `[200, 201, ..., 299]`
/// - Mixed: `"200-299,404,500-502"` → `[200, ..., 299, 404, 500, 501, 502]`
///
/// Invalid entries are logged as warnings and skipped.
fn parse_status_codes(input: &str) -> Vec<u16> {
    input
        .split(',')
        .flat_map(|part| {
            let part = part.trim();
            if part.contains('-') {
                let range: Vec<&str> = part.split('-').collect();
                if range.len() == 2 {
                    if let (Ok(start), Ok(end)) = (range[0].parse::<u16>(), range[1].parse::<u16>()) {
                        return (start..=end).collect::<Vec<_>>();
                    }
                }
                tracing::warn!("Failed to parse status code range: {}", part);
                vec![]
            } else {
                part.parse::<u16>().map_or_else(
                    |_| {
                        if !part.is_empty() {
                            tracing::warn!("Failed to parse status code: {}", part);
                        }
                        vec![]
                    },
                    |code| vec![code],
                )
            }
        })
        .collect()
}

/// Returns `s` with bytes that `http::HeaderValue` rejects removed.
///
/// RFC 7230 limits header field values to visible ASCII plus SP/HTAB; bytes
/// `< 0x20` (except `\t` = 0x09) and DEL (`0x7F`) are forbidden. The
/// `x-amzn-request-context` and `x-amzn-lambda-context` headers carry
/// JSON serialized from the Lambda event, which can echo arbitrary bytes
/// from the original request path. Without this, a request whose path
/// contains control bytes (e.g. from a security scanner) would fail the
/// whole invocation with `InvalidHeaderValue`.
///
/// Returns `Cow::Borrowed` when no forbidden byte is present (the common
/// case), avoiding any allocation.
fn strip_forbidden_header_bytes(s: &str) -> Cow<'_, [u8]> {
    let bytes = s.as_bytes();
    if bytes.iter().all(|&b| b == b'\t' || (b >= 0x20 && b != 0x7F)) {
        Cow::Borrowed(bytes)
    } else {
        Cow::Owned(
            bytes
                .iter()
                .copied()
                .filter(|&b| b == b'\t' || (b >= 0x20 && b != 0x7F))
                .collect(),
        )
    }
}

/// The Lambda Web Adapter.
///
/// This is the main struct that handles forwarding Lambda events to your web application.
/// It implements the [`tower::Service`] trait, allowing it to be used with the Lambda runtime.
///
/// # Type Parameters
///
/// - `C` - The HTTP connector type (typically [`hyper_util::client::legacy::connect::HttpConnector`])
/// - `B` - The request body type (typically [`lambda_http::Body`])
///
/// # Lifecycle
///
/// 1. Create an adapter with [`Adapter::new()`]
/// 2. Register as a Lambda extension with [`Adapter::register_default_extension()`]
/// 3. Wait for the web app to be ready with [`Adapter::check_init_health()`]
/// 4. Start processing events with [`Adapter::run()`]
///
/// # Examples
///
/// ```rust,no_run
/// use lambda_web_adapter::{Adapter, AdapterOptions};
///
/// # async fn example() -> Result<(), lambda_web_adapter::Error> {
/// let options = AdapterOptions::default();
/// let mut adapter = Adapter::new(&options)?;
///
/// adapter.register_default_extension();
/// adapter.check_init_health().await;
/// adapter.run().await
/// # }
/// ```
#[derive(Clone)]
pub struct Adapter<C, B> {
    client: Arc<Client<C, B>>,
    healthcheck_url: Url,
    healthcheck_protocol: Protocol,
    healthcheck_healthy_status: Vec<u16>,
    async_init: bool,
    ready_at_init: Arc<AtomicBool>,
    domain: Url,
    base_path: Option<String>,
    pass_through_path: String,
    compression: bool,
    invoke_mode: LambdaInvokeMode,
    authorization_source: Option<String>,
    error_status_codes: Option<Vec<u16>>,
}

impl Adapter<HttpConnector, Body> {
    /// Creates a new HTTP Adapter instance.
    ///
    /// This function initializes a new HTTP client configured to communicate with
    /// your web application. When Lambda SnapStart is detected
    /// (`AWS_LAMBDA_INITIALIZATION_TYPE=snap-start`), connection pooling is
    /// disabled to prevent stale connections after restore, where
    /// `CLOCK_MONOTONIC` inconsistencies can cause hyper's pool to reuse dead
    /// connections. Otherwise, a 4-second idle timeout is used for connection
    /// pooling.
    ///
    /// # Arguments
    ///
    /// * `options` - Configuration options for the adapter
    ///
    /// # Returns
    ///
    /// Returns `Ok(Adapter)` on success, or an error if the configuration is invalid.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The configured host, port, or readiness check path contain invalid URL characters
    /// - TCP protocol is configured but the URL is missing host or port
    ///
    /// # Examples
    ///
    /// ```rust
    /// use lambda_web_adapter::{Adapter, AdapterOptions};
    ///
    /// let options = AdapterOptions::default();
    /// let adapter = Adapter::new(&options).expect("Failed to create adapter");
    /// ```
    pub fn new(options: &AdapterOptions) -> Result<Adapter<HttpConnector, Body>, Error> {
        let mut builder = Client::builder(hyper_util::rt::TokioExecutor::new());

        // When running under SnapStart, CLOCK_MONOTONIC can be inconsistent after
        // restore, causing hyper's pool to reuse dead connections (hyper#3810,
        // rust-lang/rust#79462). Disable pooling in that case. For localhost
        // communication the overhead of new TCP connections is negligible.
        if env::var("AWS_LAMBDA_INITIALIZATION_TYPE").as_deref() == Ok("snap-start") {
            builder.pool_max_idle_per_host(0);
        } else {
            builder.pool_idle_timeout(Duration::from_secs(4));
        }

        let client = builder.build(HttpConnector::new());

        let schema = "http";

        let healthcheck_url: Url = format!(
            "{}://{}:{}{}",
            schema, options.host, options.readiness_check_port, options.readiness_check_path
        )
        .parse()
        .map_err(|e| {
            Error::from(format!(
                "Invalid healthcheck URL configuration (host={}, port={}, path={}): {}",
                options.host, options.readiness_check_port, options.readiness_check_path, e
            ))
        })?;

        let domain: Url = format!("{}://{}:{}", schema, options.host, options.port)
            .parse()
            .map_err(|e| {
                Error::from(format!(
                    "Invalid domain URL configuration (host={}, port={}): {}",
                    options.host, options.port, e
                ))
            })?;

        // Validate TCP protocol requirements
        if options.readiness_check_protocol == Protocol::Tcp {
            if healthcheck_url.host().is_none() {
                return Err(Error::from("TCP readiness check requires a valid host in the URL"));
            }
            if healthcheck_url.port().is_none() {
                return Err(Error::from("TCP readiness check requires a port in the URL"));
            }
        }

        let compression = if options.compression && options.invoke_mode == LambdaInvokeMode::ResponseStream {
            tracing::warn!("Compression is not supported with response streaming. Disabling compression.");
            false
        } else {
            options.compression
        };

        Ok(Adapter {
            client: Arc::new(client),
            healthcheck_url,
            healthcheck_protocol: options.readiness_check_protocol,
            healthcheck_healthy_status: options.readiness_check_healthy_status.clone(),
            domain,
            base_path: options.base_path.clone(),
            pass_through_path: options.pass_through_path.clone(),
            async_init: options.async_init,
            ready_at_init: Arc::new(AtomicBool::new(false)),
            compression,
            invoke_mode: options.invoke_mode,
            authorization_source: options.authorization_source.clone(),
            error_status_codes: options.error_status_codes.clone(),
        })
    }
}

impl Adapter<HttpConnector, Body> {
    /// Registers the adapter as a Lambda extension.
    ///
    /// Lambda extensions are loaded before the function handler and can perform
    /// initialization tasks. This registration ensures the adapter is ready to
    /// receive events before your function starts processing.
    ///
    /// The registration happens asynchronously in a background task. If registration
    /// fails, the process will exit with code 1 to signal Lambda that initialization
    /// failed.
    ///
    /// # Panics
    ///
    /// This method spawns a background task that will call `std::process::exit(1)`
    /// if extension registration fails, terminating the Lambda execution environment.
    pub fn register_default_extension(&self) {
        // register as an external extension
        tokio::task::spawn(async move {
            if let Err(e) = Self::register_extension_internal().await {
                tracing::error!(error = %e, "Extension registration failed - terminating process");
                std::process::exit(1);
            }
        });
    }

    /// Internal implementation of extension registration.
    ///
    /// Registers with the Lambda Extensions API and waits for the next event.
    /// This keeps the extension alive for the duration of the Lambda instance.
    async fn register_extension_internal() -> Result<(), Error> {
        // Prefer the original (pre-proxy) value if apply_runtime_proxy_config() captured one.
        // Otherwise fall back to the current env var.
        let aws_lambda_runtime_api: String = match ORIGINAL_LAMBDA_RUNTIME_API.get() {
            Some(captured) => captured.clone().unwrap_or_else(|| "127.0.0.1:9001".to_string()),
            None => env::var(ENV_LAMBDA_RUNTIME_API).unwrap_or_else(|_| "127.0.0.1:9001".to_string()),
        };
        let client = Client::builder(hyper_util::rt::TokioExecutor::new()).build(HttpConnector::new());

        let register_req = hyper::Request::builder()
            .method(Method::POST)
            .uri(format!("http://{aws_lambda_runtime_api}/2020-01-01/extension/register"))
            .header("Lambda-Extension-Name", "lambda-adapter")
            .body(Body::from("{ \"events\": [] }"))?;

        let register_res = client.request(register_req).await?;

        if register_res.status() != StatusCode::OK {
            return Err(Error::from(format!(
                "Extension registration failed with status: {}",
                register_res.status()
            )));
        }

        let extension_id = register_res
            .headers()
            .get("Lambda-Extension-Identifier")
            .ok_or_else(|| Error::from("Missing Lambda-Extension-Identifier header"))?;

        let next_req = hyper::Request::builder()
            .method(Method::GET)
            .uri(format!(
                "http://{aws_lambda_runtime_api}/2020-01-01/extension/event/next"
            ))
            .header("Lambda-Extension-Identifier", extension_id)
            .body(Body::Empty)?;

        client.request(next_req).await?;

        Ok(())
    }

    /// Checks if the web application is ready during Lambda initialization.
    ///
    /// This method performs readiness checks against your web application using
    /// the configured protocol (HTTP or TCP) and endpoint.
    ///
    /// # Async Initialization
    ///
    /// If `async_init` is enabled in the adapter options, this method will:
    /// - Attempt readiness checks for up to 9.8 seconds
    /// - Return early if the timeout is reached (to avoid Lambda's 10s init timeout)
    /// - Allow the application to continue booting in the background
    ///
    /// The first request will re-check readiness if the application wasn't ready
    /// during initialization.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use lambda_web_adapter::{Adapter, AdapterOptions};
    ///
    /// # async fn example() -> Result<(), lambda_web_adapter::Error> {
    /// let options = AdapterOptions::default();
    /// let mut adapter = Adapter::new(&options)?;
    /// adapter.check_init_health().await;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn check_init_health(&mut self) {
        let ready_at_init = if self.async_init {
            timeout(Duration::from_secs_f32(9.8), self.check_readiness())
                .await
                .unwrap_or_default()
        } else {
            self.check_readiness().await
        };
        self.ready_at_init.store(ready_at_init, Ordering::SeqCst);
    }

    /// Performs a single readiness check against the configured endpoint.
    async fn check_readiness(&self) -> bool {
        let url = self.healthcheck_url.clone();
        let protocol = self.healthcheck_protocol;
        self.is_web_ready(&url, &protocol).await
    }

    /// Waits for the web application to become ready, with retries.
    ///
    /// Uses a fixed 10ms interval between retry attempts and logs progress
    /// at increasing intervals (100ms, 500ms, 1s, 2s, 5s, 10s).
    async fn is_web_ready(&self, url: &Url, protocol: &Protocol) -> bool {
        let mut checkpoint = Checkpoint::new();
        Retry::spawn(FixedInterval::from_millis(10), || {
            if checkpoint.lapsed() {
                tracing::info!(url = %url.to_string(), "app is not ready after {}ms", checkpoint.next_ms());
                checkpoint.increment();
            }
            self.check_web_readiness(url, protocol)
        })
        .await
        .is_ok()
    }

    /// Performs a single readiness check using the configured protocol.
    ///
    /// For HTTP: Makes a GET request and checks if the status code is in the healthy range.
    /// For TCP: Attempts to establish a TCP connection.
    async fn check_web_readiness(&self, url: &Url, protocol: &Protocol) -> Result<(), i8> {
        match protocol {
            Protocol::Http => {
                // url is already validated in Adapter::new(), this conversion should always succeed
                // If it fails, it indicates a programming error, not a runtime condition
                let uri: http::Uri = url
                    .as_str()
                    .parse()
                    .expect("BUG: healthcheck_url should be valid - validated in Adapter::new()");

                match self.client.get(uri).await {
                    Ok(response) if self.healthcheck_healthy_status.contains(&response.status().as_u16()) => {
                        tracing::debug!("app is ready");
                        Ok(())
                    }
                    _ => {
                        tracing::trace!("app is not ready");
                        Err(-1)
                    }
                }
            }
            Protocol::Tcp => {
                // url is already validated in Adapter::new(), host and port should exist
                // If they don't, it indicates a programming error, not a runtime condition
                let host = url
                    .host_str()
                    .expect("BUG: healthcheck_url should have host - validated in Adapter::new()");
                let port = url
                    .port()
                    .expect("BUG: healthcheck_url should have port - validated in Adapter::new()");

                match TcpStream::connect(format!("{}:{}", host, port)).await {
                    Ok(_) => Ok(()),
                    Err(_) => Err(-1),
                }
            }
        }
    }

    /// Starts the adapter and begins processing Lambda events.
    ///
    /// This method blocks and runs the Lambda runtime loop, receiving events
    /// and forwarding them to your web application.
    ///
    /// # Safety
    ///
    /// If `AWS_LWA_LAMBDA_RUNTIME_API_PROXY` is set, [`Adapter::apply_runtime_proxy_config()`]
    /// must be called BEFORE starting the tokio runtime to avoid race conditions.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` when the Lambda runtime shuts down gracefully, or an error
    /// if there's a fatal issue with the runtime.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use lambda_web_adapter::{Adapter, AdapterOptions};
    ///
    /// # async fn example() -> Result<(), lambda_web_adapter::Error> {
    /// let options = AdapterOptions::default();
    /// let adapter = Adapter::new(&options)?;
    /// adapter.run().await
    /// # }
    /// ```
    pub async fn run(self) -> Result<(), Error> {
        match (self.compression, self.invoke_mode) {
            (true, LambdaInvokeMode::Buffered) => {
                let svc = ServiceBuilder::new().layer(CompressionLayer::new()).service(self);
                lambda_http::run_concurrent(svc).await
            }
            (_, LambdaInvokeMode::Buffered) => lambda_http::run_concurrent(self).await,
            (_, LambdaInvokeMode::ResponseStream) => lambda_http::run_with_streaming_response_concurrent(self).await,
        }
    }

    /// Applies runtime API proxy configuration from environment variables.
    ///
    /// If `AWS_LWA_LAMBDA_RUNTIME_API_PROXY` is set, this method overwrites
    /// `AWS_LAMBDA_RUNTIME_API` to redirect Lambda runtime calls through the proxy.
    ///
    /// # Important
    ///
    /// This method **must** be called before starting the tokio runtime to avoid
    /// race conditions with environment variable modification in a multi-threaded context.
    ///
    /// # Safety Note
    ///
    /// This function uses `std::env::set_var` which modifies process-wide state.
    /// In future Rust versions, this will be marked `unsafe` due to potential race
    /// conditions. Calling this before spawning any threads ensures safety.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use lambda_web_adapter::Adapter;
    ///
    /// fn main() {
    ///     // Call before starting tokio runtime
    ///     Adapter::apply_runtime_proxy_config();
    ///
    ///     let runtime = tokio::runtime::Builder::new_multi_thread()
    ///         .enable_all()
    ///         .build()
    ///         .unwrap();
    ///
    ///     runtime.block_on(async {
    ///         // ... adapter setup and run
    ///     });
    /// }
    /// ```
    pub fn apply_runtime_proxy_config() {
        if let Ok(runtime_proxy) = env::var(ENV_LAMBDA_RUNTIME_API_PROXY) {
            // Capture the original value before we overwrite it, so extension
            // registration can still reach the real Lambda Runtime API.
            let original = env::var(ENV_LAMBDA_RUNTIME_API).ok();
            let _ = ORIGINAL_LAMBDA_RUNTIME_API.set(original);

            // We need to overwrite the env variable because lambda_http::run()
            // calls lambda_runtime::run() which doesn't allow changing the client URI.
            //
            // This is safe here because it's called before the tokio runtime starts,
            // ensuring no other threads exist yet.
            env::set_var(ENV_LAMBDA_RUNTIME_API, runtime_proxy);
        }
    }

    /// Forwards a Lambda event to the web application and returns the response.
    ///
    /// This method:
    /// 1. Checks readiness if async_init is enabled and app wasn't ready at init
    /// 2. Transforms the Lambda event into an HTTP request
    /// 3. Adds Lambda context headers (`x-amzn-request-context`, `x-amzn-lambda-context`)
    /// 4. Strips the base path if configured
    /// 5. Forwards the request to the web application
    /// 6. Returns the response (or error if status code is in error_status_codes)
    async fn fetch_response(&self, event: Request) -> Result<Response<Incoming>, Error> {
        if self.async_init && !self.ready_at_init.load(Ordering::SeqCst) {
            self.is_web_ready(&self.healthcheck_url, &self.healthcheck_protocol)
                .await;
            self.ready_at_init.store(true, Ordering::SeqCst);
        }

        let request_context = event.request_context();
        let lambda_context = event.lambda_context();
        let path = event.raw_http_path().to_string();
        let mut path = path.as_str();
        let (parts, body) = event.into_parts();

        // strip away Base Path if environment variable REMOVE_BASE_PATH is set.
        if let Some(base_path) = self.base_path.as_deref() {
            let stripped = path.trim_start_matches(base_path);
            if stripped.len() != path.len() {
                tracing::debug!(base_path = %base_path, original = %path, stripped = %stripped, "stripped base path");
            }
            path = stripped;
        }

        if matches!(request_context, RequestContext::PassThrough) && parts.method == Method::POST {
            path = self.pass_through_path.as_str();
        }

        let mut req_headers = parts.headers;

        // include request context in http header "x-amzn-request-context"
        req_headers.insert(
            HeaderName::from_static("x-amzn-request-context"),
            HeaderValue::from_bytes(&strip_forbidden_header_bytes(&serde_json::to_string(&request_context)?))?,
        );

        // include lambda context in http header "x-amzn-lambda-context"
        req_headers.insert(
            HeaderName::from_static("x-amzn-lambda-context"),
            HeaderValue::from_bytes(&strip_forbidden_header_bytes(&serde_json::to_string(&lambda_context)?))?,
        );

        // Multi-tenancy support: propagate tenant_id from Lambda context
        if let Some(ref tenant_id) = lambda_context.tenant_id {
            if let Ok(value) = HeaderValue::from_str(tenant_id) {
                req_headers.insert(HeaderName::from_static("x-amz-tenant-id"), value);
                tracing::debug!(tenant_id = %tenant_id, "propagating tenant_id header");
            } else {
                tracing::warn!(tenant_id = %tenant_id, "tenant_id contains invalid header characters, skipping");
            }
        }

        if let Some(authorization_source) = self.authorization_source.as_deref() {
            if let Some(original) = req_headers.remove(authorization_source) {
                req_headers.insert("authorization", original);
            } else {
                tracing::warn!("\"{}\" header not found in request headers", authorization_source);
            }
        }

        let mut app_url = self.domain.clone();
        app_url.set_path(path);
        app_url.set_query(parts.uri.query().filter(|q| !q.is_empty()));

        tracing::debug!(app_url = %app_url, req_headers = ?req_headers, "sending request to app server");

        let mut builder = hyper::Request::builder().method(parts.method).uri(app_url.to_string());
        if let Some(headers) = builder.headers_mut() {
            headers.extend(req_headers);
        }

        // Convert body without copying by moving ownership of the underlying data
        let body_bytes = match body {
            Body::Empty => Vec::new(),
            Body::Text(s) => s.into_bytes(),
            Body::Binary(b) => b,
            // Body is marked #[non_exhaustive], handle future variants
            _ => body.to_vec(),
        };
        let request = builder.body(Body::Binary(body_bytes))?;

        let mut app_response = self.client.request(request).await?;

        // Check if status code should trigger an error
        if let Some(error_codes) = &self.error_status_codes {
            let status = app_response.status().as_u16();
            if error_codes.contains(&status) {
                let body_bytes = app_response
                    .into_body()
                    .collect()
                    .await
                    .map(|c| c.to_bytes())
                    .unwrap_or_default();
                let body_str = String::from_utf8_lossy(&body_bytes);
                return Err(Error::from(format!(
                    "{{\"statusCode\":{},\"body\":{}}}",
                    status,
                    serde_json::to_string(&*body_str).unwrap_or_else(|_| format!("\"{}\"", body_str))
                )));
            }
        }

        // remove "transfer-encoding" from the response to support "sam local start-api"
        app_response.headers_mut().remove("transfer-encoding");

        tracing::debug!(status = %app_response.status(), body_size = ?app_response.body().size_hint().lower(),
            app_headers = ?app_response.headers().clone(), "responding to lambda event");

        Ok(app_response)
    }
}

/// Implementation of [`tower::Service`] for the adapter.
///
/// This allows the adapter to be used directly with the Lambda runtime,
/// which expects a `Service` that can handle Lambda events.
impl Service<Request> for Adapter<HttpConnector, Body> {
    type Response = Response<Incoming>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut core::task::Context<'_>) -> core::task::Poll<Result<(), Self::Error>> {
        core::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, event: Request) -> Self::Future {
        let adapter = self.clone();
        Box::pin(async move { adapter.fetch_response(event).await })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::{Method::GET, MockServer};

    #[test]
    fn test_parse_status_codes() {
        assert_eq!(parse_status_codes("500,502-504,422"), vec![500, 502, 503, 504, 422]);
        assert_eq!(
            parse_status_codes("500, 502-504, 422"), // with spaces
            vec![500, 502, 503, 504, 422]
        );
        assert_eq!(parse_status_codes("500"), vec![500]);
        assert_eq!(parse_status_codes("500-502"), vec![500, 501, 502]);
        assert_eq!(parse_status_codes("invalid"), Vec::<u16>::new());
        assert_eq!(parse_status_codes("500-invalid"), Vec::<u16>::new());
        assert_eq!(parse_status_codes(""), Vec::<u16>::new());
    }

    #[tokio::test]
    async fn test_status_200_is_ok() {
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
        let adapter = Adapter::new(&options).expect("Failed to create adapter");

        let url = adapter.healthcheck_url.clone();
        let protocol = adapter.healthcheck_protocol;

        //adapter.check_init_health().await;

        assert!(adapter.check_web_readiness(&url, &protocol).await.is_ok());

        // Assert app server's healthcheck endpoint got called
        healthcheck.assert();
    }

    #[tokio::test]
    async fn test_status_500_is_bad() {
        // Start app server
        let app_server = MockServer::start();
        let healthcheck = app_server.mock(|when, then| {
            when.method(GET).path("/healthcheck");
            then.status(500).body("OK");
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
        let adapter = Adapter::new(&options).expect("Failed to create adapter");

        let url = adapter.healthcheck_url.clone();
        let protocol = adapter.healthcheck_protocol;

        //adapter.check_init_health().await;

        assert!(adapter.check_web_readiness(&url, &protocol).await.is_err());

        // Assert app server's healthcheck endpoint got called
        healthcheck.assert();
    }

    #[tokio::test]
    async fn test_status_403_is_bad_when_configured() {
        // Start app server
        let app_server = MockServer::start();
        let healthcheck = app_server.mock(|when, then| {
            when.method(GET).path("/healthcheck");
            then.status(403).body("OK");
        });

        // Prepare adapter configuration - only 200-399 are healthy
        let options = AdapterOptions {
            host: app_server.host(),
            port: app_server.port().to_string(),
            readiness_check_port: app_server.port().to_string(),
            readiness_check_path: "/healthcheck".to_string(),
            readiness_check_healthy_status: (200..400).collect(),
            ..Default::default()
        };

        // Initialize adapter and do readiness check
        let adapter = Adapter::new(&options).expect("Failed to create adapter");

        let url = adapter.healthcheck_url.clone();
        let protocol = adapter.healthcheck_protocol;

        //adapter.check_init_health().await;

        assert!(adapter.check_web_readiness(&url, &protocol).await.is_err());

        // Assert app server's healthcheck endpoint got called
        healthcheck.assert();
    }

    #[tokio::test]
    async fn test_tcp_readiness_check_success() {
        // Start a TCP listener to simulate an app
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let options = AdapterOptions {
            host: "127.0.0.1".to_string(),
            port: port.to_string(),
            readiness_check_port: port.to_string(),
            readiness_check_path: "/".to_string(),
            readiness_check_protocol: Protocol::Tcp,
            ..Default::default()
        };

        let adapter = Adapter::new(&options).expect("Failed to create adapter");
        let url = adapter.healthcheck_url.clone();
        let protocol = adapter.healthcheck_protocol;

        assert_eq!(protocol, Protocol::Tcp);
        assert!(adapter.check_web_readiness(&url, &protocol).await.is_ok());
    }

    #[tokio::test]
    async fn test_tcp_readiness_check_failure() {
        // Use a port that nothing is listening on
        let options = AdapterOptions {
            host: "127.0.0.1".to_string(),
            port: "19999".to_string(),
            readiness_check_port: "19999".to_string(),
            readiness_check_path: "/".to_string(),
            readiness_check_protocol: Protocol::Tcp,
            ..Default::default()
        };

        let adapter = Adapter::new(&options).expect("Failed to create adapter");
        let url = adapter.healthcheck_url.clone();
        let protocol = adapter.healthcheck_protocol;

        assert!(adapter.check_web_readiness(&url, &protocol).await.is_err());
    }

    #[test]
    fn test_protocol_from_str() {
        assert_eq!(Protocol::from("http"), Protocol::Http);
        assert_eq!(Protocol::from("HTTP"), Protocol::Http);
        assert_eq!(Protocol::from("tcp"), Protocol::Tcp);
        assert_eq!(Protocol::from("TCP"), Protocol::Tcp);
        assert_eq!(Protocol::from("unknown"), Protocol::Http); // defaults to Http
        assert_eq!(Protocol::from(""), Protocol::Http);
    }

    #[test]
    fn test_invoke_mode_from_str() {
        assert_eq!(LambdaInvokeMode::from("buffered"), LambdaInvokeMode::Buffered);
        assert_eq!(LambdaInvokeMode::from("BUFFERED"), LambdaInvokeMode::Buffered);
        assert_eq!(
            LambdaInvokeMode::from("response_stream"),
            LambdaInvokeMode::ResponseStream
        );
        assert_eq!(
            LambdaInvokeMode::from("RESPONSE_STREAM"),
            LambdaInvokeMode::ResponseStream
        );
        assert_eq!(LambdaInvokeMode::from("unknown"), LambdaInvokeMode::Buffered); // defaults to Buffered
        assert_eq!(LambdaInvokeMode::from(""), LambdaInvokeMode::Buffered);
    }

    #[test]
    fn test_adapter_new_invalid_host() {
        let options = AdapterOptions {
            host: "invalid host with spaces".to_string(),
            port: "8080".to_string(),
            readiness_check_port: "8080".to_string(),
            readiness_check_path: "/".to_string(),
            ..Default::default()
        };

        let result = Adapter::new(&options);
        assert!(result.is_err());
    }

    #[test]
    fn test_adapter_new_valid_config() {
        let options = AdapterOptions {
            host: "127.0.0.1".to_string(),
            port: "3000".to_string(),
            readiness_check_port: "3000".to_string(),
            readiness_check_path: "/health".to_string(),
            readiness_check_protocol: Protocol::Http,
            ..Default::default()
        };

        let adapter = Adapter::new(&options);
        assert!(adapter.is_ok());
    }

    #[test]
    fn test_parse_status_codes_single_range() {
        let codes = parse_status_codes("200-204");
        assert_eq!(codes, vec![200, 201, 202, 203, 204]);
    }

    #[test]
    fn test_parse_status_codes_mixed_with_spaces() {
        let codes = parse_status_codes("200, 301-303, 404");
        assert_eq!(codes, vec![200, 301, 302, 303, 404]);
    }

    #[test]
    fn test_parse_status_codes_invalid_range_format() {
        // Three-part range should produce empty
        let codes = parse_status_codes("200-300-400");
        assert!(codes.is_empty());
    }

    // Combined into one test because ORIGINAL_LAMBDA_RUNTIME_API is a process-wide
    // OnceLock that can only be set once — separate tests would race on it.
    #[test]
    fn test_apply_runtime_proxy_config() {
        // Cleanup
        env::remove_var(ENV_LAMBDA_RUNTIME_API_PROXY);
        env::remove_var(ENV_LAMBDA_RUNTIME_API);

        // Case 1: proxy unset → no overwrite, no capture
        Adapter::apply_runtime_proxy_config();
        assert!(env::var(ENV_LAMBDA_RUNTIME_API).is_err());
        assert!(ORIGINAL_LAMBDA_RUNTIME_API.get().is_none());

        // Case 2: proxy set with a real original → original captured, env overwritten
        env::set_var(ENV_LAMBDA_RUNTIME_API, "real-api:9001");
        env::set_var(ENV_LAMBDA_RUNTIME_API_PROXY, "127.0.0.1:9002");
        Adapter::apply_runtime_proxy_config();
        assert_eq!(env::var(ENV_LAMBDA_RUNTIME_API).unwrap(), "127.0.0.1:9002");
        assert_eq!(
            ORIGINAL_LAMBDA_RUNTIME_API.get(),
            Some(&Some("real-api:9001".to_string())),
            "extension registration should see the pre-proxy Runtime API"
        );

        // Cleanup
        env::remove_var(ENV_LAMBDA_RUNTIME_API_PROXY);
        env::remove_var(ENV_LAMBDA_RUNTIME_API);
    }

    #[test]
    fn test_compression_disabled_with_response_stream() {
        let options = AdapterOptions {
            compression: true,
            invoke_mode: LambdaInvokeMode::ResponseStream,
            ..Default::default()
        };

        let adapter = Adapter::new(&options).expect("Failed to create adapter");
        assert!(
            !adapter.compression,
            "Compression should be disabled when invoke mode is ResponseStream"
        );
    }

    #[test]
    fn test_compression_enabled_with_buffered() {
        let options = AdapterOptions {
            compression: true,
            invoke_mode: LambdaInvokeMode::Buffered,
            ..Default::default()
        };

        let adapter = Adapter::new(&options).expect("Failed to create adapter");
        assert!(
            adapter.compression,
            "Compression should remain enabled when invoke mode is Buffered"
        );
    }

    /// Helper to create a Lambda Context with an optional tenant_id.
    fn make_lambda_context(tenant_id: Option<&str>) -> lambda_http::Context {
        use lambda_http::lambda_runtime::Config;
        let mut headers = http::HeaderMap::new();
        headers.insert("lambda-runtime-aws-request-id", "test-id".parse().unwrap());
        headers.insert("lambda-runtime-deadline-ms", "123".parse().unwrap());
        headers.insert("lambda-runtime-client-context", "{}".parse().unwrap());
        if let Some(tid) = tenant_id {
            headers.insert("lambda-runtime-aws-tenant-id", tid.parse().unwrap());
        }
        let conf = Config {
            function_name: "test_function".into(),
            memory: 128,
            version: "latest".into(),
            log_stream: "/aws/lambda/test_function".into(),
            log_group: "2023/09/15/[$LATEST]ab831cef03e94457a94b6efcbe22406a".into(),
        };
        lambda_http::Context::new("test-id", Arc::new(conf), &headers).unwrap()
    }

    #[tokio::test]
    async fn test_tenant_id_header_propagated() {
        let app_server = MockServer::start();
        app_server.mock(|when, then| {
            when.method(GET).path("/hello").header("x-amz-tenant-id", "tenant-abc");
            then.status(200).body("OK");
        });

        let options = AdapterOptions {
            host: app_server.host(),
            port: app_server.port().to_string(),
            readiness_check_port: app_server.port().to_string(),
            readiness_check_path: "/".to_string(),
            ..Default::default()
        };

        let adapter = Adapter::new(&options).expect("Failed to create adapter");

        // Build a minimal ALB request
        let alb_req = lambda_http::request::LambdaRequest::Alb({
            let mut req = lambda_http::aws_lambda_events::alb::AlbTargetGroupRequest::default();
            req.http_method = Method::GET;
            req.path = Some("/hello".into());
            req
        });
        let mut request = Request::from(alb_req);
        request.extensions_mut().insert(make_lambda_context(Some("tenant-abc")));

        let response = adapter.fetch_response(request).await.expect("Request failed");
        assert_eq!(200, response.status().as_u16());
    }

    #[tokio::test]
    async fn test_tenant_id_header_absent_when_no_tenant() {
        let app_server = MockServer::start();
        app_server.mock(|when, then| {
            when.method(GET)
                .path("/hello")
                .is_true(|req| !req.headers().iter().any(|(k, _)| k == "x-amz-tenant-id"));
            then.status(200).body("OK");
        });

        let options = AdapterOptions {
            host: app_server.host(),
            port: app_server.port().to_string(),
            readiness_check_port: app_server.port().to_string(),
            readiness_check_path: "/".to_string(),
            ..Default::default()
        };

        let adapter = Adapter::new(&options).expect("Failed to create adapter");

        let alb_req = lambda_http::request::LambdaRequest::Alb({
            let mut req = lambda_http::aws_lambda_events::alb::AlbTargetGroupRequest::default();
            req.http_method = Method::GET;
            req.path = Some("/hello".into());
            req
        });
        let mut request = Request::from(alb_req);
        request.extensions_mut().insert(make_lambda_context(None));

        let response = adapter.fetch_response(request).await.expect("Request failed");
        assert_eq!(200, response.status().as_u16());
    }

    #[test]
    fn test_strip_forbidden_header_bytes() {
        // Tab (0x09) and printable ASCII are preserved; CR/LF, NUL, DEL, and other
        // C0 control bytes are removed.
        let out = strip_forbidden_header_bytes("a\tb\nc\rd\u{00}e\u{04}f\u{18}g\u{7f}h");
        assert_eq!(out.as_ref(), b"a\tbcdefgh");
        assert!(
            matches!(out, Cow::Owned(_)),
            "input had forbidden bytes — must allocate"
        );

        // UTF-8 multi-byte characters are preserved (all continuation bytes >= 0x80
        // and lead bytes >= 0xC0 are above the 0x7F threshold).
        let out = strip_forbidden_header_bytes("héllo");
        assert_eq!(out.as_ref(), "héllo".as_bytes());
    }

    /// Fast path: input that is already header-safe must not allocate.
    #[test]
    fn test_strip_forbidden_header_bytes_all_clean() {
        let input = r#"{"http":{"path":"/api/users"},"requestId":"abc-123"}"#;
        let out = strip_forbidden_header_bytes(input);
        assert_eq!(out.as_ref(), input.as_bytes());
        assert!(
            matches!(out, Cow::Borrowed(_)),
            "header-safe input must not allocate (Cow::Borrowed expected)"
        );

        // Tab is allowed and should also stay on the borrowed fast path.
        let input = "tab\there";
        let out = strip_forbidden_header_bytes(input);
        assert!(matches!(out, Cow::Borrowed(_)));
        assert_eq!(out.as_ref(), input.as_bytes());
    }

    /// Regression test for https://github.com/aws/aws-lambda-web-adapter/issues/732
    ///
    /// When the Lambda event's request context contains bytes that are forbidden in
    /// HTTP header values (control bytes < 0x20 except \t, and 0x7F), serializing
    /// the request context to JSON and inserting it as `x-amzn-request-context`
    /// must not fail. Such bytes can appear when scanners (e.g. nuclei) probe a
    /// Lambda Function URL with crafted paths.
    #[tokio::test]
    async fn test_request_context_with_control_bytes_in_path() {
        let app_server = MockServer::start();
        app_server.mock(|when, then| {
            when.method(GET).is_true(|req| {
                let headers = req.headers();

                // --- x-amzn-request-context: this is where the control bytes
                // came from (echoed via request_context.http.path).
                let Some(req_ctx) = headers.get("x-amzn-request-context") else {
                    return false;
                };
                if req_ctx
                    .as_bytes()
                    .iter()
                    .any(|&b| b != b'\t' && (b < 0x20 || b == 0x7F))
                {
                    return false;
                }
                // Stripped JSON must deserialize back into the typed RequestContext
                // (not just generic JSON) — proving the structure consumers rely on
                // survives sanitization.
                let Ok(ctx) = serde_json::from_slice::<RequestContext>(req_ctx.as_bytes()) else {
                    return false;
                };
                if !matches!(ctx, RequestContext::ApiGatewayV2(_)) {
                    return false;
                }

                // --- x-amzn-lambda-context: parallel assertion — the second
                // call site also goes through strip_forbidden_header_bytes, so
                // the header must be present, header-safe, and round-trip into
                // a Context value.
                let Some(lambda_ctx) = headers.get("x-amzn-lambda-context") else {
                    return false;
                };
                if lambda_ctx
                    .as_bytes()
                    .iter()
                    .any(|&b| b != b'\t' && (b < 0x20 || b == 0x7F))
                {
                    return false;
                }
                serde_json::from_slice::<serde_json::Value>(lambda_ctx.as_bytes())
                    .ok()
                    .and_then(|v| v.get("request_id").and_then(|r| r.as_str()).map(str::to_owned))
                    .is_some()
            });
            then.status(200).body("OK");
        });

        let options = AdapterOptions {
            host: app_server.host(),
            port: app_server.port().to_string(),
            readiness_check_port: app_server.port().to_string(),
            readiness_check_path: "/".to_string(),
            ..Default::default()
        };

        let adapter = Adapter::new(&options).expect("Failed to create adapter");

        // Build an ApiGatewayV2 request whose request_context.http.path contains
        // control bytes that http::HeaderValue rejects (DEL = 0x7F, plus 0x04, 0x18).
        let v2_req = lambda_http::request::LambdaRequest::ApiGatewayV2({
            use lambda_http::aws_lambda_events::apigw::ApiGatewayV2httpRequest;
            let mut req = ApiGatewayV2httpRequest::default();
            req.raw_path = Some("/hello".into());
            req.request_context.http.method = Method::GET;
            req.request_context.http.path = Some("/\u{04}\u{7f}\u{18};{curl,http://test.oast.site}".into());
            req
        });
        let mut request = Request::from(v2_req);
        request.extensions_mut().insert(make_lambda_context(None));

        let response = adapter
            .fetch_response(request)
            .await
            .expect("Request failed despite control bytes in request context path");
        assert_eq!(200, response.status().as_u16());
    }
}
