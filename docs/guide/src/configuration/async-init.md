# Async Initialization

Lambda managed runtimes offer up to 10 seconds for function initialization with burst CPU. If your function can't complete initialization within that window, Lambda restarts it and bills for the init time.

## How It Works

When `AWS_LWA_ASYNC_INIT` is enabled:

1. The adapter performs readiness checks for up to 9.8 seconds
2. If the app isn't ready by then, the adapter signals Lambda that init is complete
3. Readiness checking continues during the first handler invocation
4. This avoids the restart penalty while using the free init CPU burst

## Enabling

```
AWS_LWA_ASYNC_INIT=true
```

## When to Use

Enable this when your application has a long startup time (e.g. loading large ML models, warming caches, establishing connection pools) that might exceed the 10-second init window.

## Not applied under SnapStart or Provisioned Concurrency

`AWS_LWA_ASYNC_INIT` only makes sense for on-demand cold starts, where the initialization phase is short and the adapter must report init complete before a slow application is ready. SnapStart and Provisioned Concurrency have no such limit, and reporting early there is harmful: SnapStart would snapshot a half-initialized application, and Provisioned Concurrency would mark the environment ready while the application is still booting — the latency you provisioned concurrency to avoid.

The adapter therefore ignores the setting when `AWS_LAMBDA_INITIALIZATION_TYPE` is `snap-start` or `provisioned-concurrency`, and logs a warning so the override is visible.
