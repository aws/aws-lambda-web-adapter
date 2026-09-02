# Environment Variables

All configuration is done through environment variables, set either in your Dockerfile or as Lambda function configuration.

## Reference Table

| Variable | Description | Default |
|----------|-------------|---------|
| `AWS_LWA_PORT` | Traffic port your app listens on (falls back to `PORT`) | `8080` |
| `AWS_LWA_READINESS_CHECK_PORT` | Readiness check port | Same as `AWS_LWA_PORT` |
| `AWS_LWA_READINESS_CHECK_PATH` | Readiness check path | `/` |
| `AWS_LWA_READINESS_CHECK_PROTOCOL` | Readiness check protocol: `http` or `tcp` | `http` |
| `AWS_LWA_READINESS_CHECK_HEALTHY_STATUS` | HTTP status codes considered healthy (e.g. `200-399` or `200,201,204,301-399`) | `100-499` |
| `AWS_LWA_ASYNC_INIT` | Enable asynchronous initialization | `false` |
| `AWS_LWA_REMOVE_BASE_PATH` | Base path to remove from request path | None |
| `AWS_LWA_ENABLE_COMPRESSION` | Enable gzip/br compression (buffered mode only) | `false` |
| `AWS_LWA_INVOKE_MODE` | Invoke mode: `buffered` or `response_stream` | `buffered` |
| `AWS_LWA_PASS_THROUGH_PATH` | Path for non-HTTP event payloads | `/events` |
| `AWS_LWA_AUTHORIZATION_SOURCE` | Header name to replace with `Authorization` | None |
| `AWS_LWA_ERROR_STATUS_CODES` | HTTP status codes that cause Lambda invocation failure (e.g. `500,502-504`) | None |
| `AWS_LWA_LAMBDA_RUNTIME_API_PROXY` | Proxy URL for Lambda Runtime API requests | None |
| `AWS_LWA_SNAPSTART_BEFORE_CHECKPOINT_PATH` | Inner-app path the adapter POSTs to before a SnapStart snapshot | None |
| `AWS_LWA_SNAPSTART_AFTER_RESTORE_PATH` | Inner-app path the adapter POSTs to after a SnapStart restore | None |
| `AWS_LWA_POOL_IDLE_TIMEOUT_SECONDS` | Idle keep-alive (seconds) for the adapter's connection to your app | `4` |
| `AWS_LWA_READINESS_CHECK_TIMEOUT_SECONDS` | Seconds (fractional allowed, e.g. `0.5`) the adapter waits for the app to report ready (cold-start init **and** after a SnapStart restore). On expiry the adapter **fails** rather than serving: cold-start init fails (the runtime never starts) and a restore fails. Unset = wait indefinitely. The `async_init` path keeps its own ~9.8s bound (non-fatal) and is unaffected. | unset (unbounded) |

## Deprecated Variables

The following non-namespaced variables are deprecated and will be removed in v2.0. Migrate to the `AWS_LWA_` prefixed versions.

| Deprecated | Replacement |
|-----------|-------------|
| `HOST` | N/A |
| `READINESS_CHECK_PORT` | `AWS_LWA_READINESS_CHECK_PORT` |
| `READINESS_CHECK_PATH` | `AWS_LWA_READINESS_CHECK_PATH` |
| `READINESS_CHECK_PROTOCOL` | `AWS_LWA_READINESS_CHECK_PROTOCOL` |
| `REMOVE_BASE_PATH` | `AWS_LWA_REMOVE_BASE_PATH` |
| `ASYNC_INIT` | `AWS_LWA_ASYNC_INIT` |
| `AWS_LWA_READINESS_CHECK_MIN_UNHEALTHY_STATUS` | `AWS_LWA_READINESS_CHECK_HEALTHY_STATUS` |

> `PORT` is **not** deprecated and remains a supported fallback for `AWS_LWA_PORT`.
