//! SnapStart bridge: notifies the inner web application over HTTP at the
//! snapshot boundary and refreshes the adapter's HTTP client after restore.

use std::sync::{Arc, OnceLock};

use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;
use lambda_http::{Body, BoxFuture, Error, SnapStartResource};
use url::Url;

use crate::build_client;

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
}

impl SnapStartHooks {
    pub(crate) fn new(
        restored_client: Arc<OnceLock<Arc<Client<HttpConnector, Body>>>>,
        client: Arc<Client<HttpConnector, Body>>,
        domain: Url,
        before_checkpoint_path: Option<String>,
        after_restore_path: Option<String>,
    ) -> Self {
        Self {
            restored_client,
            client,
            domain,
            before_checkpoint_path,
            after_restore_path,
        }
    }

    /// POSTs an empty body to `domain + path` using `client`. Non-2xx or a
    /// transport error is an error.
    async fn post_hook(client: &Client<HttpConnector, Body>, domain: &Url, path: &str) -> Result<(), Error> {
        let mut url = domain.clone();
        url.set_path(path);
        let req = hyper::Request::builder()
            .method(hyper::Method::POST)
            .uri(url.to_string())
            .body(Body::Empty)?;
        let resp = client.request(req).await?;
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
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::MockServer;

    fn hooks(server: &MockServer, before: Option<&str>, after: Option<&str>) -> SnapStartHooks {
        let domain: Url = format!("http://{}:{}", server.host(), server.port()).parse().unwrap();
        SnapStartHooks::new(
            Arc::new(OnceLock::new()),
            Arc::new(build_client()),
            domain,
            before.map(str::to_string),
            after.map(str::to_string),
        )
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
    async fn after_restore_publishes_client_when_path_unset() {
        let server = MockServer::start();
        let h = hooks(&server, None, None);
        assert!(h.after_restore().await.is_ok());
        assert!(h.restored_client.get().is_some());
    }
}
