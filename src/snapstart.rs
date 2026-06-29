//! SnapStart bridge: notifies the inner web application over HTTP at the
//! snapshot boundary and refreshes the adapter's HTTP client after restore.

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;
use lambda_http::{Body, BoxFuture, Error, SnapStartResource};
use tokio::time::timeout;
use url::Url;

use crate::{build_client, readiness, Protocol};

/// Maximum time the adapter waits for an inner-app hook to respond before
/// failing the SnapStart phase. Bounds a hung or unresponsive hook so the
/// snapshot/restore lifecycle cannot stall indefinitely.
const HOOK_TIMEOUT: Duration = Duration::from_secs(60);

/// Maximum time the adapter waits for the inner app to report ready after
/// restore (step 3). Tighter than [`HOOK_TIMEOUT`]: once the after-restore hook
/// has run, the app should become ready almost immediately, so a long stall here
/// indicates a failed restore rather than legitimate slow work.
const READINESS_TIMEOUT: Duration = Duration::from_secs(10);

/// A [`SnapStartResource`] that bridges the Lambda SnapStart lifecycle to the
/// inner web application running behind the adapter.
pub(crate) struct SnapStartHooks {
    /// Shared with the [`Adapter`](crate::Adapter); `after_restore` publishes the
    /// fresh client here so invocations stop using pre-snapshot connections.
    restored_client: Arc<OnceLock<Arc<Client<HttpConnector, Body>>>>,
    /// Client used to make the hook calls themselves (the adapter's base client).
    client: Arc<Client<HttpConnector, Body>>,
    /// `http://host:port` of the inner application.
    domain: Url,
    before_checkpoint_path: Option<String>,
    after_restore_path: Option<String>,
    /// Readiness-check endpoint, protocol, and healthy statuses — shared with the
    /// adapter so the post-restore readiness check (step 3) matches init behavior.
    healthcheck_url: Url,
    healthcheck_protocol: Protocol,
    healthcheck_healthy_status: Vec<u16>,
}

impl SnapStartHooks {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        restored_client: Arc<OnceLock<Arc<Client<HttpConnector, Body>>>>,
        client: Arc<Client<HttpConnector, Body>>,
        domain: Url,
        before_checkpoint_path: Option<String>,
        after_restore_path: Option<String>,
        healthcheck_url: Url,
        healthcheck_protocol: Protocol,
        healthcheck_healthy_status: Vec<u16>,
    ) -> Self {
        Self {
            restored_client,
            client,
            domain,
            before_checkpoint_path,
            after_restore_path,
            healthcheck_url,
            healthcheck_protocol,
            healthcheck_healthy_status,
        }
    }

    /// POSTs an empty body to `domain + path` using `client`. A non-2xx
    /// response, a transport error, or exceeding [`HOOK_TIMEOUT`] is an error.
    async fn post_hook(client: &Client<HttpConnector, Body>, domain: &Url, path: &str) -> Result<(), Error> {
        Self::post_hook_with_timeout(client, domain, path, HOOK_TIMEOUT).await
    }

    /// Implementation of [`post_hook`](Self::post_hook) with an explicit timeout,
    /// so tests can exercise the timeout path without waiting [`HOOK_TIMEOUT`].
    async fn post_hook_with_timeout(
        client: &Client<HttpConnector, Body>,
        domain: &Url,
        path: &str,
        hook_timeout: Duration,
    ) -> Result<(), Error> {
        let mut url = domain.clone();
        url.set_path(path);
        let req = hyper::Request::builder()
            .method(hyper::Method::POST)
            .uri(url.to_string())
            .body(Body::Empty)?;
        let resp = timeout(hook_timeout, client.request(req))
            .await
            .map_err(|_| Error::from(format!("SnapStart hook POST {path} timed out after {hook_timeout:?}")))??;
        if !resp.status().is_success() {
            return Err(Error::from(format!(
                "SnapStart hook POST {path} returned non-success status: {}",
                resp.status()
            )));
        }
        Ok(())
    }
}

impl SnapStartResource for SnapStartHooks {
    fn before_snapshot(&self) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            if let Some(path) = self.before_checkpoint_path.as_deref() {
                Self::post_hook(&self.client, &self.domain, path).await?;
            }
            Ok(())
        })
    }

    fn after_restore(&self) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            // 1. Publish a fresh client FIRST so the hook POST below (and all
            //    subsequent invocations) use post-restore connections rather
            //    than stale pre-snapshot ones. Ignore "already set".
            let fresh = Arc::new(build_client());
            let _ = self.restored_client.set(fresh.clone());

            // 2. Notify the app over the fresh client. Failure fails the restore;
            //    the fresh client stays published regardless.
            if let Some(path) = self.after_restore_path.as_deref() {
                Self::post_hook(&fresh, &self.domain, path).await?;
            }

            // 3. Confirm the app is serving again before traffic is admitted.
            self.check_readiness_with_timeout(&fresh, READINESS_TIMEOUT).await?;

            Ok(())
        })
    }
}

impl SnapStartHooks {
    /// Step 3 of [`after_restore`](SnapStartResource::after_restore): retry-until-ready
    /// over `client`, bounded by `readiness_timeout`. A timeout or an unready app is an
    /// error, which fails the restore (reported to `/restore/error`). Split out with an
    /// explicit timeout so tests can exercise the failure path without waiting
    /// [`READINESS_TIMEOUT`].
    async fn check_readiness_with_timeout(
        &self,
        client: &Client<HttpConnector, Body>,
        readiness_timeout: Duration,
    ) -> Result<(), Error> {
        let ready = timeout(
            readiness_timeout,
            readiness::wait_until_ready(
                client,
                &self.healthcheck_url,
                self.healthcheck_protocol,
                &self.healthcheck_healthy_status,
            ),
        )
        .await
        .map_err(|_| {
            Error::from(format!(
                "SnapStart after-restore readiness check timed out after {readiness_timeout:?}"
            ))
        })?;
        if !ready {
            return Err(Error::from("SnapStart after-restore readiness check failed"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::MockServer;

    /// Builds hooks pointed at `server`, with the readiness check targeting
    /// `health_path` on the same server.
    fn hooks_with_health(
        server: &MockServer,
        before: Option<&str>,
        after: Option<&str>,
        health_path: &str,
    ) -> SnapStartHooks {
        let domain: Url = format!("http://{}:{}", server.host(), server.port()).parse().unwrap();
        let healthcheck_url: Url = format!("http://{}:{}{}", server.host(), server.port(), health_path)
            .parse()
            .unwrap();
        SnapStartHooks::new(
            Arc::new(OnceLock::new()),
            Arc::new(build_client()),
            domain,
            before.map(str::to_string),
            after.map(str::to_string),
            healthcheck_url,
            Protocol::Http,
            (100..500).collect(),
        )
    }

    /// Builds hooks with a readiness check that always passes (a mocked `/health`
    /// returning 200), for tests focused on the before/after hook behavior.
    fn hooks(server: &MockServer, before: Option<&str>, after: Option<&str>) -> SnapStartHooks {
        server.mock(|when, then| {
            when.path("/health");
            then.status(200);
        });
        hooks_with_health(server, before, after, "/health")
    }

    #[tokio::test]
    async fn before_snapshot_posts_when_set() {
        let server = MockServer::start();
        let m = server.mock(|when, then| {
            when.method(httpmock::Method::POST).path("/before");
            then.status(200);
        });
        let h = hooks(&server, Some("/before"), None);
        assert!(h.before_snapshot().await.is_ok());
        m.assert();
    }

    #[tokio::test]
    async fn before_snapshot_noop_when_unset() {
        let server = MockServer::start();
        let h = hooks(&server, None, None);
        assert!(h.before_snapshot().await.is_ok());
    }

    #[tokio::test]
    async fn before_snapshot_non_2xx_is_error() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(httpmock::Method::POST).path("/before");
            then.status(500);
        });
        let h = hooks(&server, Some("/before"), None);
        assert!(h.before_snapshot().await.is_err());
    }

    #[tokio::test]
    async fn after_restore_publishes_client_then_posts() {
        let server = MockServer::start();
        let m = server.mock(|when, then| {
            when.method(httpmock::Method::POST).path("/after");
            then.status(200);
        });
        let h = hooks(&server, None, Some("/after"));
        assert!(h.restored_client.get().is_none());
        assert!(h.after_restore().await.is_ok());
        assert!(h.restored_client.get().is_some(), "fresh client published");
        m.assert();
    }

    #[tokio::test]
    async fn after_restore_publishes_client_even_when_hook_fails() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(httpmock::Method::POST).path("/after");
            then.status(503);
        });
        let h = hooks(&server, None, Some("/after"));
        let result = h.after_restore().await;
        assert!(result.is_err(), "hook failure returns Err");
        assert!(
            h.restored_client.get().is_some(),
            "client published despite hook failure"
        );
    }

    #[tokio::test]
    async fn post_hook_times_out_when_app_is_slow() {
        let server = MockServer::start();
        // The app takes far longer to respond than the timeout we pass below.
        server.mock(|when, then| {
            when.method(httpmock::Method::POST).path("/slow");
            then.status(200).delay(Duration::from_secs(2));
        });
        let domain: Url = format!("http://{}:{}", server.host(), server.port()).parse().unwrap();
        let client = build_client();

        let result =
            SnapStartHooks::post_hook_with_timeout(&client, &domain, "/slow", Duration::from_millis(100)).await;

        let err = result.expect_err("slow hook should time out");
        assert!(err.to_string().contains("timed out"), "unexpected error: {err}");
    }

    #[tokio::test]
    async fn after_restore_publishes_client_when_path_unset() {
        let server = MockServer::start();
        let h = hooks(&server, None, None);
        assert!(h.after_restore().await.is_ok());
        assert!(h.restored_client.get().is_some());
    }

    #[tokio::test]
    async fn after_restore_readiness_check_runs_over_fresh_client() {
        // No after-restore POST configured: step 3 must still run and pass.
        let server = MockServer::start();
        let health = server.mock(|when, then| {
            when.path("/ready");
            then.status(200);
        });
        let h = hooks_with_health(&server, None, None, "/ready");
        assert!(h.after_restore().await.is_ok());
        health.assert();
    }

    #[tokio::test]
    async fn check_readiness_times_out_when_app_never_ready() {
        // Health endpoint always reports unhealthy; the bounded readiness check
        // should give up and fail rather than retry forever.
        let server = MockServer::start();
        server.mock(|when, then| {
            when.path("/never");
            then.status(503);
        });
        let h = hooks_with_health(&server, None, None, "/never");
        let client = build_client();

        let result = h
            .check_readiness_with_timeout(&client, Duration::from_millis(100))
            .await;

        let err = result.expect_err("unready app should fail the readiness check");
        assert!(err.to_string().contains("timed out"), "unexpected error: {err}");
    }
}
