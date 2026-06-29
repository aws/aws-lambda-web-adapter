use std::time::Instant;

use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;
use lambda_http::Body;
use tokio::net::TcpStream;
use tokio_retry::{strategy::FixedInterval, Retry};
use url::Url;

use crate::Protocol;

/// Performs a single readiness check against `url` using `protocol`.
///
/// For HTTP: issues a GET via `client` and checks the status is in `healthy_status`.
/// For TCP: attempts to establish a TCP connection. Returns `Ok(())` when ready.
pub(crate) async fn check_web_readiness(
    client: &Client<HttpConnector, Body>,
    url: &Url,
    protocol: Protocol,
    healthy_status: &[u16],
) -> Result<(), i8> {
    match protocol {
        Protocol::Http => {
            // url is validated in Adapter::new(); this conversion should always succeed.
            let uri: http::Uri = url
                .as_str()
                .parse()
                .expect("BUG: healthcheck_url should be valid - validated in Adapter::new()");

            match client.get(uri).await {
                Ok(response) if healthy_status.contains(&response.status().as_u16()) => {
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
            // url is validated in Adapter::new(); host and port should exist.
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

/// Waits for the web application to become ready, retrying on a fixed 10ms
/// interval and logging progress at increasing checkpoints. Returns `true` once
/// the app is ready. Callers bound the total wait with an external timeout.
pub(crate) async fn wait_until_ready(
    client: &Client<HttpConnector, Body>,
    url: &Url,
    protocol: Protocol,
    healthy_status: &[u16],
) -> bool {
    let mut checkpoint = Checkpoint::new();
    Retry::spawn(FixedInterval::from_millis(10), || {
        if checkpoint.lapsed() {
            tracing::info!(url = %url.to_string(), "app is not ready after {}ms", checkpoint.next_ms());
            checkpoint.increment();
        }
        check_web_readiness(client, url, protocol, healthy_status)
    })
    .await
    .is_ok()
}

pub(crate) struct Checkpoint {
    start: Instant,
    interval_ms: u128,
    next_ms: u128,
}

impl Checkpoint {
    pub fn new() -> Checkpoint {
        // The default function timeout is 3 seconds. This will alert the users. See #520
        let interval_ms = 2000;

        let start = Instant::now();
        Checkpoint {
            start,
            interval_ms,
            next_ms: start.elapsed().as_millis() + interval_ms,
        }
    }

    pub const fn next_ms(&self) -> u128 {
        self.next_ms
    }

    pub const fn increment(&mut self) {
        self.next_ms += self.interval_ms;
    }

    pub fn lapsed(&self) -> bool {
        self.start.elapsed().as_millis() >= self.next_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checkpoint_new() {
        let checkpoint = Checkpoint::new();
        assert_eq!(checkpoint.next_ms(), 2000);
        assert!(!checkpoint.lapsed());
    }

    #[test]
    fn test_checkpoint_increment() {
        let mut checkpoint = Checkpoint::new();
        checkpoint.increment();
        assert_eq!(checkpoint.next_ms(), 4000);
        assert!(!checkpoint.lapsed());
    }

    #[test]
    fn test_checkpoint_lapsed() {
        let checkpoint = Checkpoint {
            start: Instant::now(),
            interval_ms: 0,
            next_ms: 0,
        };
        assert!(checkpoint.lapsed());
    }
}
