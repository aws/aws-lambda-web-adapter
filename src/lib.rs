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
//!         adapter.check_init_health().await?;
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
mod snapstart;

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
const ENV_SNAPSTART_BEFORE_CHECKPOINT_PATH: &str = "AWS_LWA_SNAPSTART_BEFORE_CHECKPOINT_PATH";
const ENV_SNAPSTART_AFTER_RESTORE_PATH: &str = "AWS_LWA_SNAPSTART_AFTER_RESTORE_PATH";
const ENV_LAMBDA_RUNTIME_API_PROXY: &str = "AWS_LWA_LAMBDA_RUNTIME_API_PROXY";
const ENV_POOL_IDLE_TIMEOUT_SECONDS: &str = "AWS_LWA_POOL_IDLE_TIMEOUT_SECONDS";

/// Default idle-connection keep-alive for the adapter's inner-app HTTP client,
/// used when [`ENV_POOL_IDLE_TIMEOUT_SECONDS`] is unset or unparseable.
const DEFAULT_POOL_IDLE_TIMEOUT_SECONDS: u64 = 4;

const ENV_READINESS_CHECK_TIMEOUT_SECONDS: &str = "AWS_LWA_READINESS_CHECK_TIMEOUT_SECONDS";

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
use http_body_util::{BodyExt, Empty};
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;
use lambda_http::request::RequestContext;
pub use lambda_http::tracing;
use lambda_http::Body;
pub use lambda_http::Error;

// Re-export the body types that appear in the public `Service::Response`
// (`Response<BoxBody<Bytes, Error>>`), so downstream consumers driving the
// `Adapter` as a `tower::Service` can name that type without taking their own
// direct dependency on `bytes` / `http-body-util`.
pub use bytes::Bytes;
pub use http_body_util::combinators::BoxBody;
use lambda_http::{Request, RequestExt, Response};
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
use tokio::time::timeout;
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

    /// Inner-app path POSTed before the SnapStart snapshot is taken.
    /// When set, the adapter notifies the app so it can drain resources.
    /// Default: `None` (phase skipped).
    pub snapstart_before_checkpoint_path: Option<String>,

    /// Inner-app path POSTed after the SnapStart restore completes.
    /// When set, the adapter notifies the app so it can reconnect / reseed.
    /// Default: `None` (phase skipped).
    pub snapstart_after_restore_path: Option<String>,

    /// Idle-connection keep-alive for the adapter's HTTP client to the inner app.
    ///
    /// Configurable via `AWS_LWA_POOL_IDLE_TIMEOUT_SECONDS` (whole seconds).
    /// Default: 4 seconds.
    pub pool_idle_timeout: Duration,

    /// Bound on the readiness check: how long the adapter waits for the inner app
    /// to report ready before giving up. Applied to both the initial (cold-start)
    /// readiness check and the post-SnapStart-restore readiness check.
    ///
    /// Configurable via `AWS_LWA_READINESS_CHECK_TIMEOUT_SECONDS` (fractional
    /// seconds allowed, e.g. `0.5`).
    /// `None` (the default) means **unbounded**: the adapter waits indefinitely for
    /// the app to become ready, preserving the historical behavior. Setting it bounds
    /// both checks; on the restore check a timeout fails the restore.
    ///
    /// Note: the `async_init` initial-readiness path retains its own fixed ~9.8s
    /// bound and is unaffected by this option.
    pub readiness_check_timeout: Option<Duration>,
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
            snapstart_before_checkpoint_path: env::var(ENV_SNAPSTART_BEFORE_CHECKPOINT_PATH)
                .ok()
                .filter(|p| !p.is_empty()),
            snapstart_after_restore_path: env::var(ENV_SNAPSTART_AFTER_RESTORE_PATH)
                .ok()
                .filter(|p| !p.is_empty()),
            pool_idle_timeout: pool_idle_timeout_from_env(),
            readiness_check_timeout: readiness_check_timeout_from_env(),
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

/// Percent-decode `input` a single pass. Returns `None` if a `%` escape is
/// malformed (not followed by two hex digits) — the caller treats a decode
/// failure as an ambiguous input and fails closed.
fn percent_decode_once(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            // Need two hex digits after '%'.
            let hi = bytes.get(i + 1).copied()?;
            let lo = bytes.get(i + 2).copied()?;
            let h = (hi as char).to_digit(16)?;
            let l = (lo as char).to_digit(16)?;
            out.push((h * 16 + l) as u8);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    // The decoded bytes must remain valid UTF-8 to be a comparable path.
    String::from_utf8(out).ok()
}

/// Canonicalize a path into a list of lowercased segments for the strict,
/// fail-closed SnapStart hook guard.
///
/// The guard must block *every* spelling that the downstream app router would
/// resolve to the configured hook route, so this over-approximates: it
/// percent-decodes (a single pass, matching the router), splits on `/`,
/// drops empty segments (collapsing `//`, leading/trailing slashes), resolves
/// `.`/`..`, and lowercases each segment.
///
/// An encoded slash (`%2f`) is decoded to a literal `/` *before* splitting, so a
/// spelling like `/snapstart/%2fafter` collapses onto the same segment list as
/// the hook route and is caught — while a genuinely distinct route that merely
/// contains `%2f` produces a different segment list and is left alone. This keeps
/// the strictness targeted: it only bites paths that canonicalize onto the hook.
///
/// Returns `None` only for two genuinely undecidable inputs: a malformed `%` escape,
/// or a byte sequence that is not UTF-8 once decoded. A control/null byte is *not*
/// undecidable — it is stripped before canonicalization, which *widens* the blocked
/// equivalence class (`/snapstart/af%00ter` and `/snapstart/after%0A` canonicalize
/// onto the hook route and are blocked), because a router can still resolve such a
/// path to the hook: Python's `$` matches before a trailing newline.
///
/// [`matches_hook_path`] treats a `None` **request** path as *not the hook* and
/// passes it through (see `matches_hook_path` and
/// `test_matches_hook_path_undecidable_passes_through`); that is safe because
/// [`hook_target`] guarantees no hook route contains a literal `%`. A `None` on the
/// **configured** side is rejected outright by [`hook_target`], failing
/// initialization rather than leaving the route partially guarded.
fn canonicalize_hook_path(path: &str) -> Option<Vec<String>> {
    // Percent-decode a SINGLE pass, mirroring what the downstream app router
    // does. A router decodes exactly once, so `/snapstart/%61fter` reaches the
    // app as `/snapstart/after` (and must be guarded), while `/snapstart/%2561fter`
    // reaches it as the literal `/snapstart/%61fter` (a different route the app
    // does NOT resolve to the hook). Decoding more than once would over-decode
    // relative to the router — buying no extra protection while making a validly
    // single-encoded path like `/reports/100%25` (i.e. `/reports/100%`) look
    // undecidable and get a false 403. A malformed escape or non-UTF-8 result
    // still yields None — the only two cases that do; see the contract above.
    let current = percent_decode_once(path)?;
    // Control bytes are NOT "undecidable": a downstream router can still resolve a
    // path that carries them (e.g. Python's `$` matches immediately before a
    // trailing `\n`, so Starlette resolves `/hook\n` to the `/hook` route). Bailing
    // out with `None` here would make `matches_hook_path` pass such a request
    // through and leave the hook route externally reachable. So STRIP control bytes
    // (including DEL) and keep canonicalizing — `/snapstart/after%0A` then
    // canonicalizes to `["snapstart", "after"]` and is blocked. A malformed `%`
    // escape / non-UTF-8 result is different (the `?` above already returned None
    // for it) and keeps its pass-through, avoiding the `/reports/100%` false 403.
    let current: String = current.chars().filter(|c| !c.is_control()).collect();
    let mut segments: Vec<String> = Vec::new();
    for seg in current.split('/') {
        // Drop matrix / path parameters (everything from the first `;` in a
        // segment). Several supported frameworks strip these before routing —
        // Spring MVC's UrlPathHelper defaults to removeSemicolonContent=true, and
        // servlet containers strip `;jsessionid` — so `/snapstart/after;x=1`
        // resolves to `/snapstart/after`. The guard must block that spelling too;
        // stripping here (before empty/`.`/`..` classification) keeps the guard's
        // equivalence class aligned with the app router. This runs on both sides
        // via `hook_target`, so it stays symmetric.
        let seg = seg.split(';').next().unwrap_or(seg);
        match seg {
            "" | "." => continue, // collapse empty segments and `.`
            ".." => {
                segments.pop(); // resolve parent segment
            }
            s => segments.push(s.to_ascii_lowercase()),
        }
    }
    Some(segments)
}

/// Treats an empty configured value as unset.
///
/// An empty `AWS_LWA_SNAPSTART_*_PATH` means "this hook does not fire". Collapsing
/// it to `None` at the single point where [`Adapter::new`] reads it keeps the guard
/// target and the hook's POST target from disagreeing — see
/// `test_empty_hook_path_is_normalized_on_both_sides`.
fn non_empty(value: &Option<String>) -> Option<String> {
    value.as_deref().filter(|v| !v.is_empty()).map(str::to_string)
}

/// Precomputes the guard target for a configured hook path.
///
/// Runs the operator-configured path through the SAME `Url::set_path`
/// transformation that `SnapStartHooks::post_hook` uses to reach the app
/// (`domain.set_path(configured)`), so the guard protects exactly the route the
/// app actually serves — not the raw env-var string. Returns:
///
/// * `Ok(None)` — no hook configured (unset, or set to the empty string).
/// * `Ok(Some(segments))` — the normal case: the post-`set_path` route
///   canonicalized (percent-decode, collapse `//`/`.`/`..`, case-fold).
/// * `Err(_)` — the route cannot be guarded exactly. Three cases, all always a
///   misconfiguration of a control-plane path, and all rejected rather than
///   downgraded to a weaker guard or to no guard at all (the app still *serves*
///   such a route, so anything less leaves a state-mutating route reachable):
///   1. It could not be canonicalized at all (a malformed `%` escape, or non-UTF-8
///      after decoding).
///   2. Its canonical form contains a literal `%`. This is what makes
///      [`matches_hook_path`]'s pass-through of an undecidable *request* path safe
///      on every framework, without modelling per-framework decoding: an
///      undecidable request path is either rejected by the router outright (Node
///      throws `URIError`, so Express answers 400; Go and Spring likewise 400) or
///      decoded leniently into a path containing a literal `%` or U+FFFD (Python's
///      `unquote`) — and neither can equal a `%`-free hook route.
///   3. It collapses to the app root (`/`, `//`, `/..`, `/.`, `/foo/..`, `/%2f`, …).
///      Guarding the root would 403 all normal traffic, so the guard cannot cover
///      it — and `SnapStartHooks::post_hook` would still POST to `/` on every
///      lifecycle event, which is a 405 on an app that does not handle `POST /`
///      and fails the phase. A hook path must be one "your normal application
///      traffic does not use", which the root never is.
///
/// `Adapter::new` propagates the error, failing initialization with an actionable
/// message rather than starting up with the hook route reachable or with a hook
/// that fails every restore.
fn hook_target(domain: &Url, configured: &Option<String>) -> Result<Option<Vec<String>>, Error> {
    let Some(configured) = configured.as_deref() else {
        return Ok(None);
    };
    if configured.is_empty() {
        return Ok(None);
    }
    // Normalize the configured path exactly as post_hook will send it, so the two
    // sides of the guard comparison cannot diverge by construction.
    let mut u = domain.clone();
    u.set_path(configured);
    let outbound = u.path().to_string();
    match canonicalize_hook_path(&outbound) {
        // A configured path that canonicalizes to the root (e.g. "/", "//", "/..",
        // "/.", "/foo/..", "/%2f") cannot be guarded: matching it would 403 every
        // request to `/`. Reject it rather than silently disabling the guard, because
        // `after_restore` POSTs the RAW configured path regardless of the guard
        // target, so "no hook" here still leaves the hook firing at `/`. This check
        // must live AFTER canonicalization: a raw pre-check on the configured string
        // misses the spellings that only collapse to root once `..`/`.`/encoded-slash
        // resolve.
        Some(segments) if segments.is_empty() => Err(Error::from(format!(
            "SnapStart hook path {configured:?} collapses to the application root \
             (normalized to {outbound:?}). It cannot be guarded — matching it would return 403 \
             for every request to `/` — and the hook would still POST to `/` on every SnapStart \
             lifecycle event, failing the phase on any app that does not handle `POST /`. Choose \
             a dedicated path your normal traffic does not use, such as `/snapstart/after`."
        ))),
        // A literal `%` anywhere in the canonical route breaks the invariant that
        // lets the request side pass undecidable paths through (see case 2 above).
        Some(segments) if segments.iter().any(|s| s.contains('%')) => Err(Error::from(format!(
            "SnapStart hook path {configured:?} resolves to a route containing a literal `%` \
             ({outbound:?} decodes to /{}). The 403 guard cannot cover every spelling a web \
             framework resolves onto such a route, so it is rejected rather than left partially \
             protected. Choose a hook path without a percent sign.",
            segments.join("/")
        ))),
        Some(segments) => Ok(Some(segments)),
        None => Err(Error::from(format!(
            "SnapStart hook path {configured:?} is not canonicalizable after URL normalization \
             (normalized to {outbound:?}): it contains a malformed % escape or a byte sequence \
             that is not UTF-8 once decoded. The 403 guard cannot cover the encoded spellings \
             of such a route, so it is rejected rather than left partially protected. Choose a \
             hook path without a percent sign."
        ))),
    }
}

/// True if the outbound request path resolves to the precomputed hook route.
///
/// Both sides derive from `Url::set_path`: `want` is computed by [`hook_target`]
/// from `domain.set_path(configured)`, and `outbound_request_path` is the request's
/// `app_url.path()` (also post-`set_path`; see `fetch_response`). Because the two
/// sides share the identical normalization, a configured value that `set_path`
/// rewrites (e.g. `/snapstart\after` → `/snapstart/after`) is guarded on its
/// rewritten form, closing the divergence where the app served a route the guard
/// did not protect.
///
/// The request path is canonicalized and compared as segment lists. An undecidable
/// request path (a malformed escape, or non-UTF-8 once decoded) passes through.
/// That is safe — not merely a heuristic — because [`hook_target`] guarantees
/// `want` contains no literal `%`: a router either rejects an undecidable path
/// outright (400) or decodes it leniently to something containing a literal `%` or
/// U+FFFD, and neither can equal a `%`-free route. Passing through is what keeps a
/// request like `/reports/100%` from taking a false 403 under an unrelated hook.
///
/// Single-target convenience form, used by the tests; production goes through
/// [`matches_any_hook_path`] so the request path is canonicalized only once.
#[cfg(test)]
fn matches_hook_path(want: &Option<Vec<String>>, outbound_request_path: &str) -> bool {
    matches_any_hook_path(&[want], outbound_request_path)
}

/// [`matches_hook_path`] against several targets, canonicalizing the request path
/// **once**.
///
/// This runs on every invocation, and both examples plus the guide configure both
/// hooks — so calling the single-target form twice would repeat the percent-decode,
/// control-byte filter, split and per-segment `to_ascii_lowercase` (and their
/// allocations) for an identical result. Costs nothing when no hook is configured:
/// the all-`None` check short-circuits before canonicalizing.
fn matches_any_hook_path(wants: &[&Option<Vec<String>>], outbound_request_path: &str) -> bool {
    if wants.iter().all(|w| w.is_none()) {
        return false;
    }
    let Some(got) = canonicalize_hook_path(outbound_request_path) else {
        return false; // undecidable: not the hook, passes through
    };
    wants.iter().any(|w| w.as_ref().is_some_and(|want| want == &got))
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
/// adapter.check_init_health().await?;
/// adapter.run().await
/// # }
/// ```
#[derive(Clone)]
pub struct Adapter<C, B> {
    client: Arc<Client<C, B>>,
    restored_client: Arc<OnceLock<Arc<Client<C, B>>>>,
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
    snapstart_before_checkpoint_path: Option<String>,
    snapstart_after_restore_path: Option<String>,
    /// Precomputed guard target for the before-checkpoint hook path, derived from
    /// `domain.set_path(configured)` so it matches the route the app actually
    /// serves (see [`hook_target`]).
    hook_target_before_checkpoint: Option<Vec<String>>,
    /// Precomputed guard target for the after-restore hook path (see [`hook_target`]).
    hook_target_after_restore: Option<Vec<String>>,
    pool_idle_timeout: Duration,
    readiness_check_timeout: Option<Duration>,
}

/// Builds the hyper client used to talk to the inner web application.
///
/// Shared by [`Adapter::new`] and the SnapStart after-restore hook so the
/// post-restore client is built identically to the original. `idle_timeout` is the
/// idle-connection keep-alive, resolved from [`AdapterOptions::pool_idle_timeout`]
/// (env `AWS_LWA_POOL_IDLE_TIMEOUT_SECONDS`, default 4 seconds).
///
/// When `pooling` is [`Pooling::Disabled`] the client sets
/// `pool_max_idle_per_host(0)`, which turns hyper's pool off outright
/// (`Config::is_enabled()` is `max_idle_per_host > 0`): a finished connection is
/// dropped rather than parked, so reuse is impossible *by construction*.
///
/// That distinction is load-bearing, and a zero `idle_timeout` is NOT a substitute.
/// With the pool enabled, reuse is decided at checkout by
/// `now.saturating_duration_since(idle_at) > timeout`; that saturates to `ZERO` when
/// the recorded instant is ahead of `now`, and `ZERO > ZERO` is false, so the entry
/// is treated as fresh and handed out. A monotonic clock that has not advanced
/// across a restore is exactly the condition hyper#3810 / rust-lang/rust#79462
/// describe — so an expiry-based scheme would depend on the very clock the
/// workaround exists to distrust.
///
/// This function reads no environment: the caller decides, so the post-restore
/// rebuild cannot silently inherit the pre-snapshot restriction.
fn build_client(idle_timeout: Duration, pooling: Pooling) -> Client<HttpConnector, Body> {
    let mut builder = Client::builder(hyper_util::rt::TokioExecutor::new());
    builder.pool_idle_timeout(idle_timeout);
    if pooling == Pooling::Disabled {
        builder.pool_max_idle_per_host(0);
    }
    builder.build(HttpConnector::new())
}

/// Builds the client used to talk to the Lambda Runtime API (RAPID) for extension
/// registration.
///
/// Idle pooling is disabled. Under SnapStart, a connection parked here is captured in
/// the snapshot and dead after restore — the same hazard `lambda_runtime` handles by
/// calling `reset_pool()` on its own RAPID client in the restore lifecycle. Nothing
/// resets or re-establishes this one, and [`Adapter::register_default_extension`]
/// terminates the process with `exit(1)` if its request fails, so handing out a dead
/// connection would kill a restored environment before it serves anything.
///
/// Pooling costs nothing to give up here: this client issues exactly two requests —
/// `register`, then the long poll for the first extension event — and the long poll's
/// own in-flight connection is unaffected by the idle-pool setting.
fn runtime_api_client() -> Client<HttpConnector, Body> {
    let mut builder = Client::builder(hyper_util::rt::TokioExecutor::new());
    builder.pool_max_idle_per_host(0);
    builder.build(HttpConnector::new())
}

/// Whether a client may keep idle connections alive for reuse. See [`build_client`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pooling {
    /// Normal keep-alive, bounded by the configured idle timeout.
    Enabled,
    /// No connection is retained at all — required before a SnapStart snapshot.
    Disabled,
}

/// Connection-pool policy for the client [`Adapter::new`] builds — the one used
/// before a SnapStart snapshot is taken.
///
/// Disabled under SnapStart, so no connection can be captured in the snapshot and
/// handed out — dead — after a restore (hyper#3810, rust-lang/rust#79462).
///
/// Why the pool must be *off* rather than expiry-bounded, measured on a deployed
/// SnapStart container function: `CLOCK_MONOTONIC` does not advance across the
/// snapshot gap. One restore showed the monotonic clock moving **0.54s** while wall
/// time moved **161s**. hyper decides reuse with
/// `now.saturating_duration_since(idle_at) > idle_timeout`, so a connection pooled
/// before the snapshot reads as half a second idle after restore no matter how long
/// the snapshot actually sat — fresh under any sane timeout, and dead. No idle
/// timeout, including `Duration::ZERO`, can fix that (`ZERO > ZERO` is false).
///
/// `run()` additionally rebuilds a fresh client in the after-restore hook, but
/// keeping this one safe by construction also protects a consumer driving the
/// `Service` impl directly, who never triggers that hook — and that consumer has no
/// other protection, so it must not depend on the clock.
///
/// The cost is that a pre-snapshot readiness poll reconnects on every 10ms attempt
/// (measured: 27 connections per 300ms of polling, versus 1 with keep-alive). That is
/// confined to init, which under SnapStart runs once per published version rather
/// than per restore.
///
/// The configured idle timeout is NOT lost: it is kept on
/// [`Adapter::pool_idle_timeout`] and applied to the after-restore rebuild, whose
/// pool starts empty and therefore cannot hold a snapshotted connection. That is what
/// makes `AWS_LWA_POOL_IDLE_TIMEOUT_SECONDS` effective for the invocations that
/// actually serve traffic, instead of a no-op for the life of the environment.
///
/// The two sites deliberately disagree — pooling off here, on in `after_restore` —
/// and that is sound because the clock anomaly is confined to the snapshot boundary.
/// Measured after restore on the same deployment, `CLOCK_MONOTONIC` tracks wall time
/// exactly (+6.079s / +6.059s monotonic against +6.1s / +6.0s wall), and requests
/// separated by idle gaps longer than the configured 4s keep-alive all succeeded. A
/// client built after restore holds only post-restore entries, so its expiry
/// accounting is reliable; this one may hold pre-boundary entries, so its is not.
fn base_client_pooling() -> Pooling {
    if env::var("AWS_LAMBDA_INITIALIZATION_TYPE").as_deref() == Ok("snap-start") {
        Pooling::Disabled
    } else {
        Pooling::Enabled
    }
}

/// Reads a `Duration` in seconds from environment variable `name`, falling back to
/// `default_secs` when the var is unset or unusable. Surrounding whitespace is
/// tolerated.
///
/// Accepts fractional seconds (`0.5`), matching
/// [`readiness_check_timeout_from_env`] — the two knobs are siblings and taking
/// different numeric formats would be a trap. A value that is set but unusable (a
/// stray unit suffix like `30s`, non-numeric, negative, NaN, infinity, or an
/// overflowing magnitude) emits a `warn!` before falling back, for the same reason
/// its sibling does: a set value silently becoming the default is the opposite of
/// the operator's intent, and silence makes the misconfiguration invisible.
fn duration_secs_from_env(name: &str, default_secs: u64) -> Duration {
    let default = Duration::from_secs(default_secs);
    let Ok(raw) = env::var(name) else {
        return default; // unset: silent, this is the normal case
    };
    let trimmed = raw.trim();
    match trimmed.parse::<f64>().map(Duration::try_from_secs_f64) {
        Ok(Ok(d)) => d,
        // Negative, NaN, infinite, or beyond Duration's range.
        Ok(Err(_)) | Err(_) => {
            tracing::warn!(
                variable = %name,
                value = %trimmed,
                default = ?default,
                "environment variable is set but is not a usable number of seconds \
                 (e.g. use `4` or `0.5`, not `4s`); falling back to the default"
            );
            default
        }
    }
}

/// Reads the inner-app connection pool idle timeout from
/// [`ENV_POOL_IDLE_TIMEOUT_SECONDS`] (`AWS_LWA_POOL_IDLE_TIMEOUT_SECONDS`).
/// Falls back to [`DEFAULT_POOL_IDLE_TIMEOUT_SECONDS`] when unset or unparseable.
fn pool_idle_timeout_from_env() -> Duration {
    duration_secs_from_env(ENV_POOL_IDLE_TIMEOUT_SECONDS, DEFAULT_POOL_IDLE_TIMEOUT_SECONDS)
}

/// Reads the readiness-check timeout from
/// [`ENV_READINESS_CHECK_TIMEOUT_SECONDS`] (`AWS_LWA_READINESS_CHECK_TIMEOUT_SECONDS`).
/// Accepts fractional seconds (e.g. `0.5`). Returns `None` — an unbounded readiness
/// wait (historical behavior) — in these cases:
/// * **unset**: silent (the default).
/// * **`<= 0` (including `0`)**: a zero/negative value reads as "fail fast" but a
///   zero timeout would expire instantly and fail every check, so it is treated as
///   "no bound" — and this emits a `warn!` so the non-obvious mapping is visible.
/// * **set but unusable** (a stray suffix like `10s`, non-numeric, NaN, infinity,
///   or an overflowing magnitude): emits a `warn!` before falling back, because
///   silently ignoring a misconfigured bound would let a sync-init cold start hang
///   to the Lambda function timeout with no diagnostic.
fn readiness_check_timeout_from_env() -> Option<Duration> {
    // Unset -> silent unbounded (historical default).
    let raw = env::var(ENV_READINESS_CHECK_TIMEOUT_SECONDS).ok()?;
    let trimmed = raw.trim();

    // Parse as fractional seconds; a value that does not parse as a finite,
    // representable, positive Duration is REJECTED and falls back to unbounded.
    // In every set-but-rejected case we WARN, because a set value silently
    // becoming "wait forever" is the opposite of the operator's intent and would
    // let a sync-init cold start hang to the Lambda function timeout with no
    // signal. This includes `<= 0` (including `0`): a zero/negative bound reads
    // naturally as "fail fast / don't wait", but a zero timeout would expire
    // instantly and fail every check, so it is treated as "no bound" — and the
    // warning makes that non-obvious mapping visible.
    match trimmed.parse::<f64>() {
        Ok(secs) if secs <= 0.0 => {
            tracing::warn!(
                value = %trimmed,
                "AWS_LWA_READINESS_CHECK_TIMEOUT_SECONDS is <= 0; a zero/negative bound is \
                 treated as no timeout (waiting for readiness indefinitely), not fail-fast"
            );
            None
        }
        Ok(secs) => match Duration::try_from_secs_f64(secs) {
            Ok(d) if !d.is_zero() => Some(d),
            // secs > 0 but not representable as a Duration (NaN is caught by the
            // <= 0.0 arm not matching; this covers infinity / overflow).
            _ => {
                tracing::warn!(
                    value = %trimmed,
                    "AWS_LWA_READINESS_CHECK_TIMEOUT_SECONDS is set but out of range; \
                     ignoring it and waiting for readiness without a timeout"
                );
                None
            }
        },
        Err(_) => {
            tracing::warn!(
                value = %trimmed,
                "AWS_LWA_READINESS_CHECK_TIMEOUT_SECONDS is set but not a number of seconds \
                 (e.g. use `10` or `0.5`, not `10s`); ignoring it and waiting for readiness \
                 without a timeout"
            );
            None
        }
    }
}

impl Adapter<HttpConnector, Body> {
    /// Creates a new HTTP Adapter instance.
    ///
    /// This function initializes a new HTTP client configured to communicate with
    /// your web application. The idle-connection keep-alive comes from
    /// [`AdapterOptions::pool_idle_timeout`] (`AWS_LWA_POOL_IDLE_TIMEOUT_SECONDS`,
    /// default 4 seconds). Under SnapStart
    /// (`AWS_LAMBDA_INITIALIZATION_TYPE=snap-start`) this client is built with idle
    /// keep-alive disabled instead, so no connection can be captured in the snapshot
    /// and handed out dead after a restore; the configured value is retained and
    /// applied to the client rebuilt in the after-restore hook, which is what serves
    /// invocations. See the private `base_client_pooling` for the rationale.
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
    /// - A SnapStart hook path cannot be guarded: it is not canonicalizable, its
    ///   decoded form contains a literal `%`, it collapses to the application root,
    ///   or it resolves to the same route as `AWS_LWA_PASS_THROUGH_PATH`
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
        let client = build_client(options.pool_idle_timeout, base_client_pooling());

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

        // Normalize an empty hook path to "unset" BEFORE anything reads it, so the
        // guard target and the path the hook POSTs to cannot disagree. `hook_target`
        // treats `Some("")` as "no hook", but `SnapStartHooks` would still take its
        // `if let Some(path)` branch and POST to `Url::set_path("")` — which is `/`,
        // the unguarded application root. Env-derived options already drop empties;
        // this covers an `AdapterOptions` built directly.
        let snapstart_before_checkpoint_path = non_empty(&options.snapstart_before_checkpoint_path);
        let snapstart_after_restore_path = non_empty(&options.snapstart_after_restore_path);

        // Precompute the SnapStart hook guard targets while `domain` is still in
        // scope, so the guard compares against the route the app actually serves
        // (`domain.set_path(configured)`) rather than the raw configured string.
        // A hook path the guard cannot cover fails initialization here rather than
        // starting up with a state-mutating route left externally reachable.
        let hook_target_before_checkpoint = hook_target(&domain, &snapstart_before_checkpoint_path)?;
        let hook_target_after_restore = hook_target(&domain, &snapstart_after_restore_path)?;

        // A hook path that resolves to the same route as the pass-through path is
        // unguardable in a different way: `fetch_response` rewrites the path to
        // `pass_through_path` for a PassThrough POST *before* the guard runs, so every
        // non-HTTP trigger event would canonicalize onto the hook route and get a 403
        // instead of reaching the app. Fail here rather than silently swallowing that
        // whole class of events with only a per-invocation warning.
        //
        // Only relevant when a hook exists. And note the `.unwrap_or(None)`:
        // `pass_through_path` is unrelated configuration read straight from the
        // environment, so a value `hook_target` would reject (root-collapsing,
        // `%`-bearing, non-canonicalizable) must NOT fail initialization here — it is
        // also incapable of colliding, since hook targets are canonicalizable and
        // non-empty by construction and so nothing rewritten onto such a path can
        // canonicalize onto one.
        if hook_target_before_checkpoint.is_some() || hook_target_after_restore.is_some() {
            let pass_through_target = hook_target(&domain, &Some(options.pass_through_path.clone())).unwrap_or(None);
            for (configured, target) in [
                (&snapstart_before_checkpoint_path, &hook_target_before_checkpoint),
                (&snapstart_after_restore_path, &hook_target_after_restore),
            ] {
                if target.is_some() && *target == pass_through_target {
                    return Err(Error::from(format!(
                        "SnapStart hook path {:?} resolves to the same route as the pass-through path \
                         {:?} (AWS_LWA_PASS_THROUGH_PATH). Non-HTTP trigger events are rewritten onto \
                         that path before the hook guard runs, so every such event would be rejected \
                         with 403 instead of reaching your application. Choose a different hook path.",
                        configured.as_deref().unwrap_or_default(),
                        options.pass_through_path
                    )));
                }
            }
        }

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
            restored_client: Arc::new(OnceLock::new()),
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
            snapstart_before_checkpoint_path,
            snapstart_after_restore_path,
            hook_target_before_checkpoint,
            hook_target_after_restore,
            pool_idle_timeout: options.pool_idle_timeout,
            readiness_check_timeout: options.readiness_check_timeout,
        })
    }

    /// Returns the active inner-app HTTP client: the restored client if a
    /// SnapStart restore has published one, otherwise the base client.
    fn client(&self) -> &Arc<Client<HttpConnector, Body>> {
        self.restored_client.get().unwrap_or(&self.client)
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
        let client = runtime_api_client();

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
    /// during initialization. The async path always returns `Ok`.
    ///
    /// # Synchronous Initialization
    ///
    /// Without `async_init`, this waits for the app to report ready before the
    /// Lambda runtime starts serving. If `readiness_check_timeout`
    /// (`AWS_LWA_READINESS_CHECK_TIMEOUT_SECONDS`) is set and the app does not
    /// become ready within it, this returns `Err`: init fails and no traffic is
    /// served against an app that never came up. When the timeout is unset the
    /// wait is unbounded and this returns `Ok` once the check completes
    /// (historical behavior).
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use lambda_web_adapter::{Adapter, AdapterOptions};
    ///
    /// # async fn example() -> Result<(), lambda_web_adapter::Error> {
    /// let options = AdapterOptions::default();
    /// let mut adapter = Adapter::new(&options)?;
    /// adapter.check_init_health().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn check_init_health(&mut self) -> Result<(), Error> {
        let ready_at_init = if self.async_init {
            // async_init keeps its own fixed bound, independent of
            // readiness_check_timeout (see AdapterOptions::readiness_check_timeout).
            // A timeout here is non-fatal: the app keeps booting and the first
            // request re-checks readiness.
            // `is_ok()` means the wait COMPLETED within the bound; the readiness wait
            // itself never reports "not ready" (it retries until it is).
            timeout(Duration::from_secs_f32(9.8), self.check_readiness())
                .await
                .is_ok()
        } else if let Some(t) = self.readiness_check_timeout {
            // Bound the sync-init readiness wait when configured. On expiry, refuse
            // to serve: fail init rather than admit traffic to an app that never
            // reported ready — this is the point of configuring the bound.
            match timeout(t, self.check_readiness()).await {
                Ok(()) => true,
                Err(_) => {
                    return Err(Error::from(format!(
                        "web application did not become ready within {t:?} \
                         (AWS_LWA_READINESS_CHECK_TIMEOUT_SECONDS); failing initialization"
                    )));
                }
            }
        } else {
            // Unset: unbounded wait (historical behavior). It only returns once the
            // app is ready, so reaching this point means ready.
            self.check_readiness().await;
            true
        };
        self.ready_at_init.store(ready_at_init, Ordering::SeqCst);
        Ok(())
    }

    /// Waits for the app to report ready against the configured endpoint. Returns
    /// only once it does; callers impose any bound with an external timeout.
    async fn check_readiness(&self) {
        let url = self.healthcheck_url.clone();
        let protocol = self.healthcheck_protocol;
        self.is_web_ready(&url, &protocol).await
    }

    /// Waits for the web application to become ready, with retries.
    ///
    /// Uses a fixed 10ms interval between retry attempts and logs progress
    /// at increasing intervals (100ms, 500ms, 1s, 2s, 5s, 10s). Returns only once
    /// the app is ready — see [`readiness::wait_until_ready`].
    async fn is_web_ready(&self, url: &Url, protocol: &Protocol) {
        readiness::wait_until_ready(self.client(), url, *protocol, &self.healthcheck_healthy_status).await;
    }

    /// Performs a single readiness check using the configured protocol.
    ///
    /// For HTTP: Makes a GET request and checks if the status code is in the healthy range.
    /// For TCP: Attempts to establish a TCP connection.
    ///
    /// Used by tests; `Adapter`'s own readiness path goes through [`is_web_ready`](Self::is_web_ready).
    #[cfg(test)]
    async fn check_web_readiness(&self, url: &Url, protocol: &Protocol) -> Result<(), i8> {
        readiness::check_web_readiness(self.client(), url, *protocol, &self.healthcheck_healthy_status).await
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
        let hooks = Arc::new(snapstart::SnapStartHooks::new(
            self.restored_client.clone(),
            self.client.clone(),
            self.domain.clone(),
            self.snapstart_before_checkpoint_path.clone(),
            self.snapstart_after_restore_path.clone(),
            self.healthcheck_url.clone(),
            self.healthcheck_protocol,
            self.healthcheck_healthy_status.clone(),
            self.pool_idle_timeout,
            self.readiness_check_timeout,
        ));
        match (self.compression, self.invoke_mode) {
            (true, LambdaInvokeMode::Buffered) => {
                let svc = ServiceBuilder::new().layer(CompressionLayer::new()).service(self);
                Self::register_and_run(lambda_http::runtime_concurrent(svc), hooks).await
            }
            (_, LambdaInvokeMode::Buffered) => {
                Self::register_and_run(lambda_http::runtime_concurrent(self), hooks).await
            }
            (_, LambdaInvokeMode::ResponseStream) => {
                Self::register_and_run(lambda_http::streaming_runtime_concurrent(self), hooks).await
            }
        }
    }

    /// Registers the SnapStart hooks on `runtime` and starts the concurrent event loop.
    ///
    /// Each `run()` arm builds a different runtime type (buffered vs. streaming),
    /// so the shared "register, then run" tail lives here as a generic helper.
    ///
    /// Applies [`TracingLayer`](lambda_http::lambda_runtime::layers::TracingLayer)
    /// before registering the SnapStart hooks. The free
    /// `lambda_runtime::run_concurrent` helper adds this layer internally, but this
    /// crate builds the runtime via `lambda_http::runtime_concurrent` (to attach
    /// `register_snapstart_resource`), which does not — so without it every
    /// per-invocation adapter log line (including the SnapStart hook-path 403 warn)
    /// would lose its `requestId` / `xrayTraceId` span fields.
    async fn register_and_run<S>(
        runtime: lambda_http::lambda_runtime::Runtime<S>,
        hooks: Arc<snapstart::SnapStartHooks>,
    ) -> Result<(), Error>
    where
        S: lambda_http::Service<lambda_http::lambda_runtime::LambdaInvocation, Response = (), Error = Error>
            + Clone
            + Send
            + 'static,
        S::Future: Send,
    {
        runtime
            .layer(lambda_http::lambda_runtime::layers::TracingLayer::new())
            .register_snapstart_resource(hooks)
            .run_concurrent()
            .await
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
    async fn fetch_response(&self, event: Request) -> Result<Response<BoxBody<Bytes, Error>>, Error> {
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
        // Strip exactly ONE leading occurrence, and only on a path-segment boundary,
        // so `/api/api/order` -> `/api/order` (not `/order`) and `/apiorder` is left
        // untouched (a partial-segment prefix must not be stripped). A configured
        // trailing slash is normalized away first, so `/api/` behaves like `/api`
        // (otherwise `/api/order` would fail the segment-boundary check and pass
        // through unstripped, a regression for trailing-slash base paths).
        if let Some(base_path) = self.base_path.as_deref() {
            let base_path = base_path.strip_suffix('/').unwrap_or(base_path);
            if let Some(rest) = path.strip_prefix(base_path) {
                if rest.is_empty() || rest.starts_with('/') {
                    let stripped = if rest.is_empty() { "/" } else { rest };
                    tracing::debug!(base_path = %base_path, original = %path, stripped = %stripped, "stripped base path");
                    path = stripped;
                }
            }
        }

        if matches!(request_context, RequestContext::PassThrough) && parts.method == Method::POST {
            path = self.pass_through_path.as_str();
        }

        // Block external traffic to the SnapStart hook paths. These routes are
        // control-plane operations driven only by the adapter's own hook calls
        // (which target `domain` directly and never reach this function).
        //
        // Build the outbound app URL FIRST, then run the guard against the exact
        // path that will be sent (`app_url.path()`). `Url::set_path` applies the
        // WHATWG normalization the request actually carries — e.g. for the `http`
        // scheme it rewrites `\` to `/` and resolves `.`/`..` — so guarding on the
        // raw event path could diverge from what the app receives (a `\` spelling
        // would sail past a raw-path guard yet reach the hook route). Guarding on
        // `app_url.path()` makes the guard structurally incapable of that
        // divergence; `matches_hook_path` still layers percent-decode / case-fold /
        // empty-segment collapse on top, for the spellings the app router (not
        // `Url`) resolves.
        let mut app_url = self.domain.clone();
        app_url.set_path(path);

        // The match is strict: it canonicalizes the outbound path (percent-decode,
        // strip control bytes, collapse `//`/`.`/`..`, case-fold) so that every
        // spelling the downstream app router would resolve to the hook route is
        // blocked — not just the exact configured string. Only a path that stays
        // undecidable (a malformed `%` escape, or non-UTF-8 once decoded) is treated
        // as NOT the hook and passed through rather than 403'd; that cannot reach a
        // hook route, because `hook_target` rejects any route containing a literal
        // `%`. See `matches_hook_path`.
        let outbound_path = app_url.path();
        if matches_any_hook_path(
            &[&self.hook_target_before_checkpoint, &self.hook_target_after_restore],
            outbound_path,
        ) {
            tracing::warn!(path = %outbound_path, "rejecting external request to SnapStart hook path");
            return Ok(Response::builder()
                .status(StatusCode::FORBIDDEN)
                .body(Empty::<Bytes>::new().map_err(Error::from).boxed())?);
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

        // `app_url` was built (path set + hook guard) before the header work above.
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

        let mut app_response = self.client().request(request).await?;

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

        // Box the body into a uniform type so synthetic responses (e.g. the 403
        // hook-path guard) can share the return type with proxied responses.
        Ok(app_response.map(|body| body.map_err(Error::from).boxed()))
    }
}

/// Implementation of [`tower::Service`] for the adapter.
///
/// This allows the adapter to be used directly with the Lambda runtime,
/// which expects a `Service` that can handle Lambda events.
impl Service<Request> for Adapter<HttpConnector, Body> {
    type Response = Response<BoxBody<Bytes, Error>>;
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

    // Both cases live in one test because they mutate the same process-global env
    // vars; splitting them lets Rust's parallel test runner interleave the
    // set/remove calls and clobber each other's state.
    #[test]
    fn test_snapstart_paths() {
        // Default case: unset env vars -> both None.
        std::env::remove_var(ENV_SNAPSTART_BEFORE_CHECKPOINT_PATH);
        std::env::remove_var(ENV_SNAPSTART_AFTER_RESTORE_PATH);
        let options = AdapterOptions::default();
        assert_eq!(options.snapstart_before_checkpoint_path, None);
        assert_eq!(options.snapstart_after_restore_path, None);

        // Set case: env vars present -> parsed into Some(..).
        std::env::set_var(ENV_SNAPSTART_BEFORE_CHECKPOINT_PATH, "/snapstart/before");
        std::env::set_var(ENV_SNAPSTART_AFTER_RESTORE_PATH, "/snapstart/after");
        let options = AdapterOptions::default();
        assert_eq!(
            options.snapstart_before_checkpoint_path.as_deref(),
            Some("/snapstart/before")
        );
        assert_eq!(
            options.snapstart_after_restore_path.as_deref(),
            Some("/snapstart/after")
        );

        std::env::remove_var(ENV_SNAPSTART_BEFORE_CHECKPOINT_PATH);
        std::env::remove_var(ENV_SNAPSTART_AFTER_RESTORE_PATH);
    }

    // All cases share one test because they mutate the same process-global env
    // var; separate tests would let the parallel runner clobber each other.
    #[test]
    fn test_pool_idle_timeout() {
        // Unset -> default 4s.
        std::env::remove_var(ENV_POOL_IDLE_TIMEOUT_SECONDS);
        assert_eq!(pool_idle_timeout_from_env(), Duration::from_secs(4));
        assert_eq!(AdapterOptions::default().pool_idle_timeout, Duration::from_secs(4));

        // Explicit value -> parsed, and surfaced on AdapterOptions.
        std::env::set_var(ENV_POOL_IDLE_TIMEOUT_SECONDS, "30");
        assert_eq!(pool_idle_timeout_from_env(), Duration::from_secs(30));
        assert_eq!(AdapterOptions::default().pool_idle_timeout, Duration::from_secs(30));

        // Zero is honored (disables idle keep-alive by timeout).
        std::env::set_var(ENV_POOL_IDLE_TIMEOUT_SECONDS, "0");
        assert_eq!(pool_idle_timeout_from_env(), Duration::from_secs(0));

        // Surrounding whitespace tolerated.
        std::env::set_var(ENV_POOL_IDLE_TIMEOUT_SECONDS, "  15  ");
        assert_eq!(pool_idle_timeout_from_env(), Duration::from_secs(15));

        // Fractional seconds are accepted, matching the sibling
        // AWS_LWA_READINESS_CHECK_TIMEOUT_SECONDS. Previously `0.5` failed to parse
        // as u64 and silently became the 4s default.
        std::env::set_var(ENV_POOL_IDLE_TIMEOUT_SECONDS, "0.5");
        assert_eq!(pool_idle_timeout_from_env(), Duration::from_millis(500));
        std::env::set_var(ENV_POOL_IDLE_TIMEOUT_SECONDS, "4.5");
        assert_eq!(pool_idle_timeout_from_env(), Duration::from_millis(4500));

        // Genuinely unusable values still fall back to the default (and now warn).
        for bad in ["not-a-number", "30s", "-1", "NaN", "inf", "1e400"] {
            std::env::set_var(ENV_POOL_IDLE_TIMEOUT_SECONDS, bad);
            assert_eq!(
                pool_idle_timeout_from_env(),
                Duration::from_secs(4),
                "{bad:?} must fall back to the default"
            );
        }

        std::env::remove_var(ENV_POOL_IDLE_TIMEOUT_SECONDS);
    }

    /// Serves `n` sequential requests over `client` and returns how many TCP
    /// connections the server had to accept. One connection means the idle pool
    /// was reused (keep-alive honored); `n` means every request reconnected.
    async fn connections_used(client: &Client<HttpConnector, Body>, n: usize) -> usize {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let accepted = Arc::new(AtomicUsize::new(0));
        let counter = accepted.clone();
        tokio::spawn(async move {
            loop {
                let (stream, _) = listener.accept().await.unwrap();
                counter.fetch_add(1, Ordering::SeqCst);
                tokio::spawn(async move {
                    let svc = hyper::service::service_fn(|_req: hyper::Request<hyper::body::Incoming>| async {
                        Ok::<_, std::convert::Infallible>(hyper::Response::new(String::from("ok")))
                    });
                    let _ = hyper::server::conn::http1::Builder::new()
                        .serve_connection(hyper_util::rt::TokioIo::new(stream), svc)
                        .await;
                });
            }
        });

        for _ in 0..n {
            let req = hyper::Request::builder()
                .uri(format!("http://{addr}/"))
                .body(Body::Empty)
                .unwrap();
            let resp = client.request(req).await.unwrap();
            // Drain the body so the connection is eligible to return to the pool.
            let _ = resp.into_body().collect().await.unwrap();
            tokio::time::sleep(Duration::from_millis(40)).await;
        }
        accepted.load(Ordering::SeqCst)
    }

    /// `AWS_LWA_POOL_IDLE_TIMEOUT_SECONDS` must actually take effect on the client
    /// that serves invocations after a SnapStart restore.
    ///
    /// Regression for the bot finding: `build_client` used to apply
    /// `pool_max_idle_per_host(0)` whenever `AWS_LAMBDA_INITIALIZATION_TYPE=snap-start`,
    /// and that variable stays set for the whole lifetime of a restored environment.
    /// Both call sites went through it, so the post-restore client — the one
    /// `Adapter::client()` returns for every invocation after a restore — never
    /// retained a connection, making the configured timeout a no-op on exactly the
    /// functions this feature targets and reconnecting on every single invocation
    /// (each one a fresh file descriptor, which Lambda limits).
    ///
    /// The snapshot hazard only applies to the client built BEFORE the snapshot; a
    /// client built inside `after_restore` starts with an empty pool and cannot hold
    /// a snapshotted connection, so it is safe for it to pool normally.
    #[tokio::test]
    async fn test_pool_idle_timeout_applies_under_snapstart() {
        std::env::set_var("AWS_LAMBDA_INITIALIZATION_TYPE", "snap-start");
        // This is the call `SnapStartHooks::after_restore` makes.
        let restored = build_client(Duration::from_secs(4), Pooling::Enabled);
        let used = connections_used(&restored, 3).await;
        std::env::remove_var("AWS_LAMBDA_INITIALIZATION_TYPE");
        assert_eq!(
            used, 1,
            "the post-restore client must honor the configured idle keep-alive and reuse \
             its connection, but it opened {used} connections for 3 requests"
        );
    }

    /// The Lambda Runtime API client must not retain an idle connection either.
    ///
    /// `register_extension_internal` built a default-pooled client. Under SnapStart any
    /// connection it parks is captured in the snapshot and dead after restore — the
    /// same hazard `lambda_runtime`'s own restore path handles by calling
    /// `reset_pool()` on its RAPID client. Nothing re-establishes or resets this one,
    /// and its failure path is `std::process::exit(1)`, so a reused dead connection
    /// would terminate the restored environment. It also has nothing to gain from
    /// pooling: it makes exactly two requests, `register` and then the long poll for
    /// the first extension event.
    #[tokio::test]
    async fn test_runtime_api_client_does_not_retain_connections() {
        let retained = connection_retained_after_request(&runtime_api_client()).await;
        assert!(
            !retained,
            "the Runtime API client must drop its connection rather than park a socket \
             that a snapshot would capture"
        );
    }

    /// The pre-snapshot client must still never retain an idle connection, so nothing
    /// dead can be captured in the snapshot and handed out after a restore
    /// (hyper#3810). This also covers a consumer driving the `Service` impl directly,
    /// who never triggers the after-restore rebuild.
    ///
    /// This counts reuse; `test_pre_snapshot_client_pool_is_disabled_not_merely_expiring`
    /// pins the stronger, clock-independent property that the pool is off entirely.
    #[tokio::test]
    async fn test_adapter_new_client_never_pools_under_snapstart() {
        std::env::set_var("AWS_LAMBDA_INITIALIZATION_TYPE", "snap-start");
        let options = AdapterOptions {
            pool_idle_timeout: Duration::from_secs(4),
            ..Default::default()
        };
        let adapter = Adapter::new(&options).unwrap();
        let used = connections_used(&adapter.client, 3).await;
        std::env::remove_var("AWS_LAMBDA_INITIALIZATION_TYPE");
        assert_eq!(
            used, 3,
            "the pre-snapshot client must not retain an idle connection, but it reused one \
             ({used} connections for 3 requests)"
        );
        // The configured value is still carried through for the post-restore rebuild.
        assert_eq!(adapter.pool_idle_timeout, Duration::from_secs(4));
    }

    /// Issues one request over `client`, then reports whether the connection is still
    /// open afterwards (i.e. parked in hyper's idle pool).
    ///
    /// This distinguishes a *disabled* pool from a pool whose entries merely expire:
    /// with `pool_max_idle_per_host(0)` the connection is dropped as soon as the
    /// response completes, so the server side finishes; with
    /// `pool_idle_timeout(Duration::ZERO)` the socket stays parked and is only
    /// evicted at the next checkout. It observes the connection's lifetime rather
    /// than elapsed time, so unlike a reuse count it cannot be satisfied by a clock
    /// that happens to have advanced.
    async fn connection_retained_after_request(client: &Client<HttpConnector, Body>) -> bool {
        use std::sync::atomic::{AtomicBool, Ordering};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let closed = Arc::new(AtomicBool::new(false));
        let flag = closed.clone();
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let svc = hyper::service::service_fn(|_req: hyper::Request<hyper::body::Incoming>| async {
                Ok::<_, std::convert::Infallible>(hyper::Response::new(String::from("ok")))
            });
            // Returns once the peer closes the connection.
            let _ = hyper::server::conn::http1::Builder::new()
                .serve_connection(hyper_util::rt::TokioIo::new(stream), svc)
                .await;
            flag.store(true, Ordering::SeqCst);
        });

        let req = hyper::Request::builder()
            .uri(format!("http://{addr}/"))
            .body(Body::Empty)
            .unwrap();
        let resp = client.request(req).await.unwrap();
        let _ = resp.into_body().collect().await.unwrap();
        // Give the client a moment to either drop or park the connection.
        tokio::time::sleep(Duration::from_millis(100)).await;
        !closed.load(Ordering::SeqCst)
    }

    /// The pre-snapshot client must make reuse impossible *by construction*, not by
    /// relying on the monotonic clock.
    ///
    /// Regression for the bot `[BUG]` finding: `pool_idle_timeout(Duration::ZERO)`
    /// leaves hyper's pool enabled and parks the connection, deciding reuse at
    /// checkout via `now.saturating_duration_since(idle_at) > timeout`. That
    /// saturates to `ZERO` when the recorded instant is ahead of `now`, and
    /// `ZERO > ZERO` is false — so the entry counts as fresh and is handed out. A
    /// monotonic clock that did not advance across a restore is exactly the
    /// condition hyper#3810 / rust-lang/rust#79462 describe, and exactly what the
    /// original `pool_max_idle_per_host(0)` was written to distrust. Under `run()`
    /// it is masked by the after-restore rebuild, but the direct-`Service` consumer
    /// this restriction exists for is the one path where it can fail.
    ///
    /// `pool_max_idle_per_host(0)` disables the pool, so no clock is consulted.
    #[tokio::test]
    async fn test_pre_snapshot_client_pool_is_disabled_not_merely_expiring() {
        std::env::set_var("AWS_LAMBDA_INITIALIZATION_TYPE", "snap-start");
        let options = AdapterOptions {
            pool_idle_timeout: Duration::from_secs(4),
            ..Default::default()
        };
        let adapter = Adapter::new(&options).unwrap();
        let retained = connection_retained_after_request(&adapter.client).await;
        std::env::remove_var("AWS_LAMBDA_INITIALIZATION_TYPE");
        assert!(
            !retained,
            "the pre-snapshot client must DROP its connection, not park it in the idle \
             pool where a non-advancing clock could see it as fresh and reuse it"
        );
    }

    /// The post-restore client, by contrast, must keep pooling: its pool starts empty
    /// so it cannot hold a snapshotted connection, and retaining one is the whole
    /// point of `AWS_LWA_POOL_IDLE_TIMEOUT_SECONDS`.
    #[tokio::test]
    async fn test_post_restore_client_retains_its_connection() {
        std::env::set_var("AWS_LAMBDA_INITIALIZATION_TYPE", "snap-start");
        // The call `SnapStartHooks::after_restore` makes.
        let restored = build_client(Duration::from_secs(4), Pooling::Enabled);
        let retained = connection_retained_after_request(&restored).await;
        std::env::remove_var("AWS_LAMBDA_INITIALIZATION_TYPE");
        assert!(
            retained,
            "the post-restore client must keep its connection alive for reuse"
        );
    }

    /// Without SnapStart nothing changes: keep-alive to the inner app is honored, so
    /// the safety mechanism above must not cost every other deployment its pooling.
    #[tokio::test]
    async fn test_adapter_new_client_pools_without_snapstart() {
        std::env::remove_var("AWS_LAMBDA_INITIALIZATION_TYPE");
        let options = AdapterOptions {
            pool_idle_timeout: Duration::from_secs(4),
            ..Default::default()
        };
        let adapter = Adapter::new(&options).unwrap();
        let used = connections_used(&adapter.client, 3).await;
        assert_eq!(used, 1, "keep-alive must be honored without SnapStart, got {used}");
    }

    // All cases share one test because they mutate the same process-global env
    // var; separate tests would let the parallel runner clobber each other.
    #[test]
    fn test_readiness_check_timeout() {
        // Unset -> None (unbounded), surfaced on AdapterOptions.
        std::env::remove_var(ENV_READINESS_CHECK_TIMEOUT_SECONDS);
        assert_eq!(readiness_check_timeout_from_env(), None);
        assert_eq!(AdapterOptions::default().readiness_check_timeout, None);

        // Explicit value -> Some(secs), parsed and surfaced.
        std::env::set_var(ENV_READINESS_CHECK_TIMEOUT_SECONDS, "45");
        assert_eq!(readiness_check_timeout_from_env(), Some(Duration::from_secs(45)));
        assert_eq!(
            AdapterOptions::default().readiness_check_timeout,
            Some(Duration::from_secs(45))
        );

        // Fractional seconds are accepted.
        std::env::set_var(ENV_READINESS_CHECK_TIMEOUT_SECONDS, "0.5");
        assert_eq!(readiness_check_timeout_from_env(), Some(Duration::from_millis(500)));

        // Surrounding whitespace tolerated.
        std::env::set_var(ENV_READINESS_CHECK_TIMEOUT_SECONDS, "  20  ");
        assert_eq!(readiness_check_timeout_from_env(), Some(Duration::from_secs(20)));

        // Unparseable / non-finite / negative -> None (unbounded), never a panic.
        for bad in ["nope", "-1", "NaN", "inf", "0", "0.0", "1e300", "99999999999999999999"] {
            std::env::set_var(ENV_READINESS_CHECK_TIMEOUT_SECONDS, bad);
            assert_eq!(
                readiness_check_timeout_from_env(),
                None,
                "value {bad:?} should be rejected"
            );
        }

        std::env::remove_var(ENV_READINESS_CHECK_TIMEOUT_SECONDS);
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
    async fn test_check_init_health_fails_when_sync_init_readiness_timeout_expires() {
        // App server that never reports ready (always 500) so the readiness
        // retry loop runs until the configured timeout fires.
        let app_server = MockServer::start();
        app_server.mock(|when, then| {
            when.method(GET).path("/healthcheck");
            then.status(500).body("nope");
        });

        // Sync init (async_init defaults to false) with a short configured bound.
        let options = AdapterOptions {
            host: app_server.host(),
            port: app_server.port().to_string(),
            readiness_check_port: app_server.port().to_string(),
            readiness_check_path: "/healthcheck".to_string(),
            readiness_check_timeout: Some(Duration::from_millis(100)),
            ..Default::default()
        };

        let mut adapter = Adapter::new(&options).expect("Failed to create adapter");

        // Refuse to serve: a configured timeout that expires fails initialization.
        let result = adapter.check_init_health().await;
        assert!(
            result.is_err(),
            "sync-init readiness timeout should fail init, got {result:?}"
        );
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
    async fn test_external_request_to_hook_path_is_forbidden() {
        // App server should NOT be called for a guarded path.
        let app_server = MockServer::start();
        let guarded = app_server.mock(|when, then| {
            when.path("/snapstart/after");
            then.status(200).body("should not be called");
        });

        let options = AdapterOptions {
            host: app_server.host(),
            port: app_server.port().to_string(),
            readiness_check_port: app_server.port().to_string(),
            readiness_check_path: "/".to_string(),
            snapstart_after_restore_path: Some("/snapstart/after".to_string()),
            ..Default::default()
        };
        let adapter = Adapter::new(&options).expect("Failed to create adapter");

        // External request (ALB) targeting the guarded hook path.
        let alb_req = lambda_http::request::LambdaRequest::Alb({
            let mut req = lambda_http::aws_lambda_events::alb::AlbTargetGroupRequest::default();
            req.http_method = Method::POST;
            req.path = Some("/snapstart/after".into());
            req
        });
        let mut request = Request::from(alb_req);
        request.extensions_mut().insert(make_lambda_context(None));

        let response = adapter
            .fetch_response(request)
            .await
            .expect("guard returns Ok response");
        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        // The inner app must not have been contacted.
        guarded.assert_calls(0);
    }

    #[tokio::test]
    async fn test_non_hook_path_is_proxied_normally() {
        let app_server = MockServer::start();
        let hello = app_server.mock(|when, then| {
            when.path("/hello");
            then.status(200).body("OK");
        });

        let options = AdapterOptions {
            host: app_server.host(),
            port: app_server.port().to_string(),
            readiness_check_port: app_server.port().to_string(),
            readiness_check_path: "/".to_string(),
            snapstart_after_restore_path: Some("/snapstart/after".to_string()),
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
        assert_eq!(response.status(), StatusCode::OK);
        hello.assert();
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

    #[tokio::test]
    async fn test_client_helper_returns_restored_when_set() {
        let options = AdapterOptions {
            host: "127.0.0.1".to_string(),
            port: "8080".to_string(),
            readiness_check_port: "8080".to_string(),
            ..Default::default()
        };
        let adapter = Adapter::new(&options).expect("Failed to create adapter");

        // Before restore: client() returns the base client.
        let base_ptr = Arc::as_ptr(adapter.client()) as *const ();

        // Publish a fresh client.
        let fresh = Arc::new(build_client(Duration::from_secs(4), Pooling::Enabled));
        let fresh_ptr = Arc::as_ptr(&fresh) as *const ();
        assert!(adapter.restored_client.set(fresh).is_ok(), "set should succeed once");

        // After restore: client() returns the restored client (different pointer).
        let now_ptr = Arc::as_ptr(adapter.client()) as *const ();
        assert_ne!(now_ptr, base_ptr);
        assert_eq!(now_ptr, fresh_ptr);
    }

    // ---------------------------------------------------------------------
    // Strict fail-closed SnapStart hook-path guard
    // ---------------------------------------------------------------------

    /// Build an ALB request for an arbitrary method + raw path.
    fn alb_request(method: Method, raw_path: &str) -> Request {
        let alb_req = lambda_http::request::LambdaRequest::Alb({
            let mut req = lambda_http::aws_lambda_events::alb::AlbTargetGroupRequest::default();
            req.http_method = method;
            req.path = Some(raw_path.into());
            req
        });
        let mut request = Request::from(alb_req);
        request.extensions_mut().insert(make_lambda_context(None));
        request
    }

    /// The guard must block the ENTIRE equivalence class of spellings that the
    /// downstream app router would resolve to the configured hook route — not just
    /// the exact configured string. Each of these must yield 403 and never reach
    /// the app.
    #[tokio::test]
    async fn test_hook_guard_blocks_equivalence_class() {
        let blocked = [
            "/snapstart/after",                // canonical
            "snapstart/after",                 // missing leading slash (set_path still routes it)
            "/snapstart/after/",               // trailing slash
            "//snapstart//after",              // duplicate empty segments
            "/snapstart/./after",              // dot segment
            "/foo/../snapstart/after",         // parent segment resolves onto the hook
            "/snapstart/%61fter",              // percent-encoded 'a'
            "/SnapStart/After",                // case variance
            "/snapstart/%2fafter",             // encoded slash decodes to '/' -> matches hook route
            "/snapstart\\after",               // backslash: Url::set_path normalizes it to '/'
            "/snapstart/after;x=1",            // matrix param: stripped by Spring MVC / servlet routing
            "/snapstart/after;jsessionid=abc", // servlet session param variant
            "/snapstart;a=b/after",            // matrix param on a non-terminal segment
            "/snapstart/after%0A",             // trailing LF: Starlette `$` matches before `\n`
            "/snapstart/after%0a",             // lowercase-encoded LF
            "/snapstart/after%0d%0a",          // trailing CRLF
            "/snapstart/after%00",             // trailing NUL
        ];

        for raw in blocked {
            let app_server = MockServer::start();
            // Match ANY path; if the guard fails open, this proves the app was hit.
            let hook = app_server.mock(|when, then| {
                when.any_request();
                then.status(200).body("should not be reached for guarded paths");
            });
            let options = AdapterOptions {
                host: app_server.host(),
                port: app_server.port().to_string(),
                readiness_check_port: app_server.port().to_string(),
                readiness_check_path: "/".to_string(),
                snapstart_after_restore_path: Some("/snapstart/after".to_string()),
                ..Default::default()
            };
            let adapter = Adapter::new(&options).expect("Failed to create adapter");
            let response = adapter
                .fetch_response(alb_request(Method::POST, raw))
                .await
                .expect("guard returns Ok response");
            assert_eq!(
                response.status(),
                StatusCode::FORBIDDEN,
                "path {raw:?} must be blocked (403) by the strict guard"
            );
            hook.assert_calls(0);
        }
    }

    /// The guard must NOT collapse into "block everything": a genuinely distinct
    /// route that merely shares a prefix with the hook path is proxied normally.
    #[tokio::test]
    async fn test_hook_guard_allows_distinct_route() {
        let allowed = ["/snapstart/after-report", "/snapstart", "/snapstart/afterx", "/hello"];
        for raw in allowed {
            let app_server = MockServer::start();
            let route = app_server.mock(|when, then| {
                when.path(raw);
                then.status(200).body("OK");
            });
            let options = AdapterOptions {
                host: app_server.host(),
                port: app_server.port().to_string(),
                readiness_check_port: app_server.port().to_string(),
                readiness_check_path: "/".to_string(),
                snapstart_after_restore_path: Some("/snapstart/after".to_string()),
                ..Default::default()
            };
            let adapter = Adapter::new(&options).expect("Failed to create adapter");
            let response = adapter
                .fetch_response(alb_request(Method::GET, raw))
                .await
                .expect("request failed");
            assert_eq!(
                response.status(),
                StatusCode::OK,
                "distinct route {raw:?} must NOT be blocked"
            );
            route.assert();
        }
    }

    // ---------------------------------------------------------------------
    // Base-path strip: single occurrence, segment-aware
    // ---------------------------------------------------------------------

    /// `/api/api/order` with base_path `/api` must strip exactly ONE occurrence
    /// (-> `/api/order`), and a partial-segment prefix like `/apiorder` must not be
    /// stripped at all.
    #[tokio::test]
    async fn test_base_path_strip_single_and_segment_aware() {
        // (base_path, request_path, path the app should receive)
        let cases = [
            ("/api", "/api/api/order", "/api/order"),  // strip once, not repeatedly
            ("/api", "/apiorder", "/apiorder"),        // partial segment: not stripped
            ("/api", "/api/order", "/order"),          // normal single strip
            ("/api", "/api", "/"),                     // exact base path -> root
            ("/api", "/other", "/other"),              // no prefix: untouched
            ("/api/", "/api/order", "/order"),         // trailing slash: normalized, still strips
            ("/api/", "/api/api/order", "/api/order"), // trailing slash + repeated segment
            ("/api/", "/api", "/"),                    // trailing slash, exact -> root
        ];

        for (base, req_path, expected) in cases {
            let app_server = MockServer::start();
            let route = app_server.mock(|when, then| {
                when.path(expected);
                then.status(200).body("OK");
            });
            let options = AdapterOptions {
                host: app_server.host(),
                port: app_server.port().to_string(),
                readiness_check_port: app_server.port().to_string(),
                readiness_check_path: "/".to_string(),
                base_path: Some(base.to_string()),
                ..Default::default()
            };
            let adapter = Adapter::new(&options).expect("Failed to create adapter");
            let response = adapter
                .fetch_response(alb_request(Method::GET, req_path))
                .await
                .unwrap_or_else(|e| panic!("request for base={base} path={req_path} failed: {e}"));
            assert_eq!(
                response.status(),
                StatusCode::OK,
                "base={base} req={req_path} should proxy to {expected}"
            );
            route.assert();
        }
    }

    // ---------------------------------------------------------------------
    // Canonicalization helper unit tests (pin the contract directly)
    // ---------------------------------------------------------------------

    #[test]
    fn test_percent_decode_once() {
        assert_eq!(
            percent_decode_once("/snapstart/%61fter").as_deref(),
            Some("/snapstart/after")
        );
        assert_eq!(percent_decode_once("/a%2fb").as_deref(), Some("/a/b")); // %2f -> '/'
        assert_eq!(percent_decode_once("plain").as_deref(), Some("plain"));
        // Malformed escapes -> None.
        assert_eq!(percent_decode_once("/a%"), None);
        assert_eq!(percent_decode_once("/a%2"), None);
        assert_eq!(percent_decode_once("/a%zz"), None);
    }

    #[test]
    fn test_canonicalize_hook_path_equivalence() {
        let want = canonicalize_hook_path("/snapstart/after").unwrap();
        for spelling in [
            "snapstart/after",
            "/snapstart/after/",
            "//snapstart//after",
            "/snapstart/./after",
            "/foo/../snapstart/after",
            "/snapstart/%61fter",
            "/SnapStart/After",
            "/snapstart/%2fafter", // encoded slash decodes to a real slash
        ] {
            assert_eq!(
                canonicalize_hook_path(spelling).as_ref(),
                Some(&want),
                "{spelling:?} should canonicalize onto the hook route"
            );
        }
    }

    #[test]
    fn test_canonicalize_hook_path_distinct_and_ambiguous() {
        let hook = canonicalize_hook_path("/snapstart/after").unwrap();
        // Distinct routes must NOT canonicalize onto the hook.
        for distinct in [
            "/snapstart/after-report",
            "/snapstart",
            "/snapstart/afterx",
            "/hello",
            "/foo/%2fbar",
            // Single-pass decode: `%2561` decodes ONCE to the literal `%61fter`,
            // which the app router does NOT resolve to `/snapstart/after`.
            "/snapstart/%2561fter",
        ] {
            assert_ne!(canonicalize_hook_path(distinct).as_ref(), Some(&hook), "{distinct:?}");
        }
        // A validly single-encoded literal percent (`%25` -> `%`) is DECIDABLE and
        // must not fall into the fail-closed branch (regression: decode-until-stable
        // used to 403 `/reports/100%25`). It canonicalizes to a concrete route.
        assert_eq!(
            canonicalize_hook_path("/reports/100%25"),
            Some(vec!["reports".to_string(), "100%".to_string()]),
        );
        // A malformed `%` escape is undecidable -> None (request side passes through).
        assert_eq!(canonicalize_hook_path("/snapstart/%2"), None);
        // A control byte is NOT undecidable: it is stripped and canonicalization
        // continues, so `/snapstart/af\u{0}ter` collapses onto the hook route and
        // will be blocked (a router like Starlette resolves it to `/snapstart/after`).
        assert_eq!(
            canonicalize_hook_path("/snapstart/af\u{0}ter"),
            Some(vec!["snapstart".to_string(), "after".to_string()]),
        );
    }

    /// Test helper mirroring the real guard: the configured path is canonicalized
    /// by [`hook_target`] (through `set_path`), and the request path is passed
    /// through the same `set_path` normalization the request side uses in
    /// `fetch_response` (`app_url.path()`). Both sides therefore share the identical
    /// transformation, exactly as in production.
    ///
    /// Panics on a configured path `hook_target` rejects; that path never reaches
    /// the guard in production either, because `Adapter::new` fails first (see
    /// `test_non_canonicalizable_configured_hook_path_is_rejected`).
    fn guard_blocks(configured: &str, request: &str) -> bool {
        let domain: Url = "http://127.0.0.1:8080".parse().unwrap();
        let want =
            hook_target(&domain, &Some(configured.to_string())).expect("configured hook path must be canonicalizable");
        let mut u = domain.clone();
        u.set_path(request);
        matches_hook_path(&want, u.path())
    }

    /// Regression for the single-pass fix: with a hook that shares a first
    /// segment, a validly single-encoded request under that segment must NOT be
    /// blocked (bot finding: `/reports/100%25` vs hook `/reports/snapshot`).
    #[test]
    fn test_matches_hook_path_valid_encoded_percent_not_blocked() {
        assert!(
            !guard_blocks("/reports/snapshot", "/reports/100%25"),
            "/reports/100%25 must not 403"
        );
        // The genuine single-encoded hook spelling is still blocked.
        assert!(guard_blocks("/reports/snapshot", "/reports/%73napshot"));
    }

    /// The configured side must be normalized through `set_path` too, so a
    /// configured value that `set_path` rewrites still guards the route the app
    /// actually serves (bot SECURITY finding: `/snapstart\after`).
    #[test]
    fn test_matches_hook_path_configured_side_normalized_through_set_path() {
        // Configured with a backslash: set_path rewrites it to `/snapstart/after`,
        // which is the route the app serves — so requests to it must be blocked.
        assert!(guard_blocks("/snapstart\\after", "/snapstart/after"));
        assert!(guard_blocks("/snapstart\\after", "/snapstart\\after"));
        assert!(guard_blocks("/snapstart\\after", "/SnapStart/After/"));
        // A genuinely different route is still allowed.
        assert!(!guard_blocks("/snapstart\\after", "/snapstart/other"));

        // Dot segments / duplicate slashes in the configured value are resolved by
        // set_path + canonicalize, so the effective route is still guarded.
        assert!(guard_blocks("/a/../snapstart//after", "/snapstart/after"));
    }

    #[test]
    fn test_matches_hook_path_undecidable_passes_through() {
        assert!(guard_blocks("/snapstart/after", "/SnapStart/After/"));
        assert!(!guard_blocks("/snapstart/after", "/snapstart/after-report"));
        assert!(!matches_hook_path(&None, "/snapstart/after")); // no hook configured
        assert!(!matches_hook_path(&None, "/snapstart/%2")); // ...and none => never match

        // A malformed `%` escape is undecidable — the app router cannot decode it
        // to the hook route either — so it is NOT the hook and passes through, even
        // when it shares the hook's leading segment.
        assert!(!guard_blocks("/snapstart/after", "/snapstart/%2"));
        assert!(!guard_blocks("/snapstart/after", "/snapstart/%")); // trailing bare %

        // A control byte is DIFFERENT: a router can still resolve the surrounding
        // path to the hook (Python's `$` matches before a trailing `\n`), so the
        // guard strips control bytes and BLOCKS the request rather than passing it
        // through. `af\u{0}ter` canonicalizes to the `after` segment.
        assert!(guard_blocks("/snapstart/after", "/snapstart/af\u{0}ter"));

        // Unrelated undecidable routes are likewise not blocked (bot findings:
        // /reports/100% under a shared-prefix hook, /100%, /other/%2).
        assert!(!guard_blocks("/snapstart/after", "/reports/100%"));
        assert!(!guard_blocks("/snapstart/after", "/other/%2"));
        assert!(!guard_blocks("/snapstart/after", "/100%"));
        assert!(!guard_blocks("/snapstart/after", "/%2"));
        // The exact bot-reported case: hook shares the first segment.
        assert!(!guard_blocks("/reports/snapshot", "/reports/100%"));
    }

    /// An empty configured hook path must mean "no hook" on BOTH sides: the guard
    /// target and the path the hook actually POSTs to.
    ///
    /// Regression: `hook_target` short-circuits on `configured.is_empty()` and
    /// returns `Ok(None)` ("no hook"), but `Adapter::new` used to store the raw
    /// `Some("")`, which `run()` hands to `SnapStartHooks`. `before_snapshot` /
    /// `after_restore` then take their `if let Some(path)` branch and call
    /// `post_hook(.., "")` — and `Url::set_path("")` yields `/`, so the adapter
    /// POSTed to the unguarded application root on every lifecycle event (a 405 on
    /// both FastAPI examples, which `post_hook` treats as fatal). That is the same
    /// guard-versus-POST divergence the root-collapse rejection closed; `""` slipped
    /// past it by returning before canonicalization. Normalizing to `None` here
    /// keeps the documented "empty means unset" semantics while making the two
    /// sides agree by construction.
    #[test]
    fn test_empty_hook_path_is_normalized_on_both_sides() {
        let options = AdapterOptions {
            snapstart_before_checkpoint_path: Some(String::new()),
            snapstart_after_restore_path: Some(String::new()),
            ..Default::default()
        };
        let adapter = Adapter::new(&options).expect("empty hook paths mean 'no hook', not an error");
        assert_eq!(
            adapter.snapstart_before_checkpoint_path, None,
            "an empty before-checkpoint path must not leave a hook that POSTs to /"
        );
        assert_eq!(
            adapter.snapstart_after_restore_path, None,
            "an empty after-restore path must not leave a hook that POSTs to /"
        );
        // The guard side already agreed; assert both halves together so they cannot
        // drift apart again.
        assert_eq!(adapter.hook_target_before_checkpoint, None);
        assert_eq!(adapter.hook_target_after_restore, None);
    }

    /// A configured hook path that cannot be canonicalized must be REJECTED, not
    /// degraded to a raw string compare.
    ///
    /// Regression for the bot SECURITY finding: the old `HookTarget::Raw` fallback
    /// compared raw strings on both sides, so a configured `/snapstart/after%`
    /// (bare `%`, undecidable) left every encoded spelling of that same route
    /// unguarded — verified against uvicorn/Starlette, which serves the route as
    /// `/snapstart/after%` and resolves a request for `/snapstart/after%25` onto it.
    /// A non-canonicalizable hook path is always a misconfiguration, so fail init
    /// rather than ship a guard that reads as protective but is not.
    #[test]
    fn test_non_canonicalizable_configured_hook_path_is_rejected() {
        let domain: Url = "http://127.0.0.1:8080".parse().unwrap();
        for cfg in ["/snapstart/after%", "/snapstart/%2", "/snapstart/%zz"] {
            assert!(
                hook_target(&domain, &Some(cfg.to_string())).is_err(),
                "configured hook path {cfg:?} must be rejected, not silently degraded"
            );
        }
        // A canonicalizable path is still accepted.
        assert_eq!(
            hook_target(&domain, &Some("/snapstart/after".to_string())).unwrap(),
            Some(vec!["snapstart".to_string(), "after".to_string()])
        );
    }

    /// A configured hook path whose canonical form contains a literal `%` must also
    /// be rejected — this is what makes the request-side pass-through provably safe.
    ///
    /// Rejecting only *non-canonicalizable* configs is not enough: configuring the
    /// same route the "correct" way (`/snapstart/after%25`, canonical `after%`) left
    /// the bare-`%` spelling reachable, because an undecidable request path passes
    /// through the guard while uvicorn/Starlette still resolves it onto the route
    /// (verified end-to-end: `POST /snapstart/after%` -> 200, handler ran).
    ///
    /// With no `%` in any hook route, the pass-through cannot be exploited on ANY
    /// framework, without the adapter modelling per-framework decoding: an
    /// undecidable request path either is rejected by the router outright (Node
    /// throws `URIError` -> Express 400; Go and Spring likewise 400), or is decoded
    /// leniently into a path containing a literal `%` or U+FFFD (Python's
    /// `unquote`) — and neither can equal a `%`-free hook route.
    #[test]
    fn test_configured_hook_path_with_literal_percent_is_rejected() {
        let domain: Url = "http://127.0.0.1:8080".parse().unwrap();
        for cfg in ["/snapstart/after%25", "/reports/100%25", "/%25/after"] {
            assert!(
                hook_target(&domain, &Some(cfg.to_string())).is_err(),
                "configured hook path {cfg:?} canonicalizes to a route containing `%` \
                 and must be rejected"
            );
        }
        // Sanity: a `%`-free route is unaffected, and a request carrying a literal
        // percent under an unrelated route still must NOT be blocked.
        assert!(hook_target(&domain, &Some("/snapstart/after".to_string())).is_ok());
        assert!(!guard_blocks("/reports/snapshot", "/reports/100%25"));
        assert!(!guard_blocks("/reports/snapshot", "/reports/100%"));
    }

    /// `Adapter::new` must surface that rejection, so a misconfigured function
    /// fails initialization with a clear error instead of starting with a
    /// weakened guard on a state-mutating route.
    #[test]
    fn test_adapter_new_fails_on_non_canonicalizable_hook_path() {
        let options = AdapterOptions {
            snapstart_after_restore_path: Some("/snapstart/after%".to_string()),
            ..Default::default()
        };
        let err = Adapter::new(&options)
            .err()
            .expect("Adapter::new must reject a non-canonicalizable hook path");
        let msg = err.to_string();
        assert!(
            msg.contains("/snapstart/after%"),
            "error must name the offending path, got: {msg}"
        );
    }

    #[test]
    fn test_matches_hook_path_empty_config_never_matches() {
        // An unset or empty configured hook path means "no hook": it must never 403
        // the app root (bot finding: AWS_LWA_..._PATH="" blocks "/").
        assert!(!guard_blocks("", "/"), "empty config must not block /");
        assert!(!guard_blocks("", "/anything"), "empty config must not block /anything");
        assert!(!matches_hook_path(&None, "/"));
    }

    /// A configured hook path that collapses to the app root must be REJECTED.
    ///
    /// Regression for the bot `[BUG]` finding: `hook_target` returned `Ok(None)`
    /// for these, silently disabling the guard, while `SnapStartHooks::after_restore`
    /// still POSTs to the raw configured path (it reads `after_restore_path`, not the
    /// guard target). The two therefore diverged with no diagnostic: with
    /// `AWS_LWA_SNAPSTART_AFTER_RESTORE_PATH=/..` the adapter POSTs to `/` on every
    /// restore, which is a 405 on both FastAPI examples (they declare only
    /// `@app.get("/")`), and `post_hook` treats any non-2xx as fatal — so every
    /// restore failed with nothing explaining why.
    ///
    /// Rejecting is the consistent resolution: the guard cannot protect the app root
    /// without 403-ing all normal traffic, and the docs require a hook path "your
    /// normal application traffic does not use" — which the root never is. Same rule
    /// as the `%` cases: if the adapter cannot guard it, it refuses to run with it.
    #[test]
    fn test_root_collapsing_configured_hook_path_is_rejected() {
        let domain: Url = "http://127.0.0.1:8080".parse().unwrap();
        for cfg in ["/", "//", "///", "/..", "/.", "/foo/..", "/%2f", "/a/../..", "/./"] {
            assert!(
                hook_target(&domain, &Some(cfg.to_string())).is_err(),
                "configured hook path {cfg:?} collapses to the app root and must be rejected"
            );
        }
        // Unset and empty remain "no hook", not an error.
        assert!(hook_target(&domain, &None).unwrap().is_none());
        assert!(hook_target(&domain, &Some(String::new())).unwrap().is_none());
        // A real route is still accepted.
        assert!(hook_target(&domain, &Some("/snapstart/after".to_string())).is_ok());
    }

    /// `Adapter::new` must surface the root-collapse rejection, so the operator gets
    /// one clear error at init instead of a 405-driven restore failure every restore.
    #[test]
    fn test_adapter_new_fails_on_root_collapsing_hook_path() {
        let options = AdapterOptions {
            snapstart_after_restore_path: Some("/..".to_string()),
            ..Default::default()
        };
        let err = Adapter::new(&options)
            .err()
            .expect("Adapter::new must reject a hook path that collapses to the root");
        let msg = err.to_string();
        assert!(msg.contains("/.."), "error must name the offending path, got: {msg}");
    }

    /// A hook path that collides with `AWS_LWA_PASS_THROUGH_PATH` must be rejected at
    /// init, because the pass-through rewrite happens BEFORE the guard.
    ///
    /// Regression for the bot finding: `fetch_response` rewrites `path` to
    /// `pass_through_path` for a `RequestContext::PassThrough` POST, and only then
    /// runs the guard on the rewritten path. So configuring the hook at `/events`
    /// (the default pass-through path) makes EVERY non-HTTP trigger event canonicalize
    /// onto the guarded route and get a 403 instead of reaching the app — silently,
    /// with only a per-invocation `warn!`. Init-time validation already exists for the
    /// other unguardable hook paths, so this belongs there too.
    #[test]
    fn test_adapter_new_fails_when_hook_path_collides_with_pass_through_path() {
        // The default pass-through path is `/events`.
        for hook in ["/events", "/Events", "/events/", "/./events", "/%65vents"] {
            let options = AdapterOptions {
                snapstart_after_restore_path: Some(hook.to_string()),
                ..Default::default()
            };
            let err = Adapter::new(&options).err().unwrap_or_else(|| {
                panic!("hook path {hook:?} collides with the pass-through path and must be rejected")
            });
            let msg = err.to_string();
            assert!(
                msg.contains("pass-through") || msg.contains("AWS_LWA_PASS_THROUGH_PATH"),
                "error should explain the pass-through collision, got: {msg}"
            );
        }

        // A custom pass-through path moves the collision with it.
        let options = AdapterOptions {
            pass_through_path: "/queue".to_string(),
            snapstart_after_restore_path: Some("/events".to_string()),
            ..Default::default()
        };
        assert!(
            Adapter::new(&options).is_ok(),
            "/events must be fine once the pass-through path is elsewhere"
        );
        let options = AdapterOptions {
            pass_through_path: "/queue".to_string(),
            snapstart_after_restore_path: Some("/queue".to_string()),
            ..Default::default()
        };
        assert!(
            Adapter::new(&options).is_err(),
            "the collision follows the configured value"
        );
    }

    /// The pass-through collision check must not fail init on an unguardable
    /// `AWS_LWA_PASS_THROUGH_PATH`, which is unrelated configuration.
    ///
    /// Regression for the bot `[BUG]` finding on de0ea31: the check ran
    /// `hook_target(&domain, &Some(pass_through_path))?`, so a pass-through path that
    /// `hook_target` rejects — `/` collapses to the root, and `AWS_LWA_PASS_THROUGH_PATH`
    /// is read straight from the environment with no prior validation — aborted
    /// `Adapter::new` with a SnapStart-flavored error, even with no hook configured and
    /// therefore no guard and nothing to collide with.
    ///
    /// Such a path cannot collide: hook targets are canonicalizable and non-empty by
    /// construction, so a request rewritten onto a root-collapsing, `%`-bearing, or
    /// non-canonicalizable pass-through path can never canonicalize onto one.
    #[test]
    fn test_unguardable_pass_through_path_does_not_fail_init() {
        for pass_through in ["/", "//", "/..", "/reports/100%25", "/bad/%2"] {
            // No hook configured: nothing to validate against at all.
            let options = AdapterOptions {
                pass_through_path: pass_through.to_string(),
                ..Default::default()
            };
            assert!(
                Adapter::new(&options).is_ok(),
                "pass-through path {pass_through:?} must not fail init when no hook is configured"
            );

            // Hook configured: still no collision possible with such a path.
            let options = AdapterOptions {
                pass_through_path: pass_through.to_string(),
                snapstart_after_restore_path: Some("/snapstart/after".to_string()),
                ..Default::default()
            };
            assert!(
                Adapter::new(&options).is_ok(),
                "pass-through path {pass_through:?} cannot collide with a canonical hook route"
            );
        }
    }
}
