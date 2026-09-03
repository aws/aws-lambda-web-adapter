//! SnapStart bridge: notifies the inner web application over HTTP at the
//! snapshot boundary and refreshes the adapter's HTTP client after restore.

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;
use lambda_http::{Body, BoxFuture, Error, SnapStartResource};
use tokio::time::timeout;
use url::Url;

use crate::{build_client, readiness, Pooling, Protocol};

/// Maximum time the adapter waits for an inner-app hook to respond before
/// failing the SnapStart phase. Bounds a hung or unresponsive hook so the
/// snapshot/restore lifecycle cannot stall indefinitely.
const HOOK_TIMEOUT: Duration = Duration::from_secs(60);

/// A [`SnapStartResource`] that bridges the Lambda SnapStart lifecycle to the
/// inner web application running behind the adapter.
pub(crate) struct SnapStartHooks {
    /// Shared with the [`Adapter`](crate::Adapter); `after_restore` publishes the
    /// fresh client here so invocations stop using pre-snapshot connections.
    restored_client: Arc<OnceLock<Arc<Client<HttpConnector, Body>>>>,
    /// The adapter's base (pre-snapshot) client, used for the BEFORE-CHECKPOINT hook
    /// only. `after_restore` deliberately uses the freshly built client instead, so
    /// its hook POST never travels over a connection captured in the snapshot.
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
    /// Idle keep-alive used to rebuild the client after restore, so the
    /// post-restore client honors the same `AWS_LWA_POOL_IDLE_TIMEOUT_SECONDS`.
    pool_idle_timeout: Duration,
    /// Bound on the post-restore readiness check (step 3), from
    /// `AWS_LWA_READINESS_CHECK_TIMEOUT_SECONDS`. `None` = unbounded (wait forever
    /// for the app to become ready), preserving historical behavior.
    readiness_timeout: Option<Duration>,
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
        pool_idle_timeout: Duration,
        readiness_timeout: Option<Duration>,
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
            pool_idle_timeout,
            readiness_timeout,
        }
    }

    /// Publishes `fresh` as the post-restore client, or adopts the one already
    /// published, returning whichever client invocations will actually use.
    ///
    /// `OnceLock::set` fails if the cell is already populated. Discarding that failure
    /// and carrying on with `fresh` would leave `after_restore` validating a client no
    /// request can reach: the hook POST and readiness check would report the restore
    /// healthy while every invocation kept using the earlier client. Returning the
    /// published client instead removes that divergence rather than reporting it, and
    /// the `warn!` records the unexpected second lifecycle run.
    fn publish_or_adopt(
        cell: &OnceLock<Arc<Client<HttpConnector, Body>>>,
        fresh: Arc<Client<HttpConnector, Body>>,
    ) -> Arc<Client<HttpConnector, Body>> {
        match cell.set(fresh.clone()) {
            Ok(()) => fresh,
            Err(_) => {
                tracing::warn!(
                    "post-restore client was already published; adopting it so the hook call and \
                     readiness check validate the client invocations actually use"
                );
                // `set` only fails when the cell is populated, so this cannot be None.
                cell.get().unwrap_or(&fresh).clone()
            }
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
            //    subsequent invocations) use post-restore connections rather than stale
            //    pre-snapshot ones. If one is somehow already published, adopt it, so
            //    steps 2 and 3 always validate the client invocations will use.
            //
            //    Pooling is ENABLED here even though `Adapter::new` disables it under
            //    SnapStart (see `base_client_pooling`), and the disagreement is
            //    deliberate. That restriction exists because `CLOCK_MONOTONIC` does not
            //    advance across the snapshot gap — measured on a deployed SnapStart
            //    container function, 0.54s of monotonic time for 161s of wall time — so
            //    hyper's `elapsed > idle_timeout` test cannot be trusted for an entry
            //    pooled before the boundary. This client is built AFTER the restore, so
            //    every entry it holds is post-boundary, and monotonic time tracks wall
            //    time normally from here on (measured: +6.079s/+6.059s monotonic
            //    against +6.1s/+6.0s wall, with idle gaps beyond the keep-alive
            //    expiring cleanly). Keeping the pool on is therefore both safe and the
            //    only way `AWS_LWA_POOL_IDLE_TIMEOUT_SECONDS` has any effect on the
            //    invocations that actually serve traffic.
            let fresh = Arc::new(build_client(self.pool_idle_timeout, Pooling::Enabled));
            let fresh = Self::publish_or_adopt(&self.restored_client, fresh);

            // 2. Notify the app over the fresh client. Failure fails the restore;
            //    the fresh client stays published regardless.
            if let Some(path) = self.after_restore_path.as_deref() {
                Self::post_hook(&fresh, &self.domain, path).await?;
            }

            // 3. Confirm the app is serving again before traffic is admitted.
            //    A configured timeout bounds the wait and fails the restore on
            //    expiry; when unset the wait is unbounded (historical behavior).
            match self.readiness_timeout {
                Some(t) => self.check_readiness_with_timeout(&fresh, t).await?,
                None => self.check_readiness_unbounded(&fresh).await,
            }

            Ok(())
        })
    }
}

impl SnapStartHooks {
    /// Step 3 of [`after_restore`](SnapStartResource::after_restore): retry-until-ready
    /// over `client`, bounded by `readiness_timeout`. A timeout or an unready app is an
    /// error, which fails the restore (reported to `/restore/error`). Split out with an
    /// explicit timeout so tests can exercise the failure path without waiting a long
    /// configured timeout (`AWS_LWA_READINESS_CHECK_TIMEOUT_SECONDS`).
    async fn check_readiness_with_timeout(
        &self,
        client: &Client<HttpConnector, Body>,
        readiness_timeout: Duration,
    ) -> Result<(), Error> {
        timeout(readiness_timeout, self.wait_ready(client)).await.map_err(|_| {
            Error::from(format!(
                "SnapStart after-restore readiness check timed out after {readiness_timeout:?}"
            ))
        })
    }

    /// Unbounded variant of [`check_readiness_with_timeout`](Self::check_readiness_with_timeout):
    /// waits indefinitely for the app to become ready. Used when
    /// `AWS_LWA_READINESS_CHECK_TIMEOUT_SECONDS` is unset (historical behavior).
    ///
    /// This cannot fail, only block: [`readiness::wait_until_ready`] retries forever.
    /// An app that never recovers therefore holds the restore open until Lambda's own
    /// restore timeout fires, with the escalating `app is not ready after {}ms` log as
    /// the only adapter-side signal. Set `AWS_LWA_READINESS_CHECK_TIMEOUT_SECONDS` to
    /// convert that into a reported `/restore/error` instead.
    async fn check_readiness_unbounded(&self, client: &Client<HttpConnector, Body>) {
        self.wait_ready(client).await;
    }

    /// Shared readiness wait against the configured healthcheck endpoint.
    async fn wait_ready(&self, client: &Client<HttpConnector, Body>) {
        readiness::wait_until_ready(
            client,
            &self.healthcheck_url,
            self.healthcheck_protocol,
            &self.healthcheck_healthy_status,
        )
        .await
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
            Arc::new(build_client(Duration::from_secs(4), Pooling::Enabled)),
            domain,
            before.map(str::to_string),
            after.map(str::to_string),
            healthcheck_url,
            Protocol::Http,
            (100..500).collect(),
            Duration::from_secs(4),
            Some(Duration::from_secs(10)),
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

    /// When the post-restore client is already published, `after_restore` must run its
    /// hook POST and readiness check over THAT client — the one invocations use — not
    /// over a freshly built one nobody can see.
    ///
    /// Regression for the bot `[ERROR_HANDLING]` finding: `let _ = ...set(fresh)`
    /// discarded the "already set" case and then used `fresh` for steps 2 and 3, so a
    /// second `after_restore` would report the restore healthy on the basis of a client
    /// the request path never touches, with no signal anywhere. Latent today (the
    /// runtime drives the lifecycle once) — this pins it so it cannot become real.
    #[test]
    fn publish_or_adopt_keeps_the_client_invocations_use() {
        let cell: OnceLock<Arc<Client<HttpConnector, Body>>> = OnceLock::new();
        let first = Arc::new(build_client(Duration::from_secs(4), Pooling::Enabled));
        let adopted = SnapStartHooks::publish_or_adopt(&cell, first.clone());
        assert!(
            Arc::ptr_eq(&adopted, &first),
            "first call publishes and returns its own client"
        );

        // A second call must adopt the published client and discard its own.
        let second = Arc::new(build_client(Duration::from_secs(4), Pooling::Enabled));
        let adopted = SnapStartHooks::publish_or_adopt(&cell, second.clone());
        assert!(
            Arc::ptr_eq(&adopted, &first),
            "second call must return the ALREADY-PUBLISHED client, not its own"
        );
        assert!(!Arc::ptr_eq(&adopted, &second));
        assert!(
            Arc::ptr_eq(cell.get().unwrap(), &first),
            "published client is unchanged"
        );
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
        let client = build_client(Duration::from_secs(4), Pooling::Enabled);

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
        let client = build_client(Duration::from_secs(4), Pooling::Enabled);

        let result = h
            .check_readiness_with_timeout(&client, Duration::from_millis(100))
            .await;

        let err = result.expect_err("unready app should fail the readiness check");
        assert!(err.to_string().contains("timed out"), "unexpected error: {err}");
    }
}
