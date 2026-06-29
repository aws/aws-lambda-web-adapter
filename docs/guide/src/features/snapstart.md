# SnapStart

[Lambda SnapStart](https://docs.aws.amazon.com/lambda/latest/dg/snapstart.html) snapshots an initialized execution environment and restores it on later cold starts, reducing startup latency. Because the adapter runs your web application as a separate process, the application does not have direct access to the SnapStart lifecycle. The adapter bridges this gap with two optional HTTP hooks.

## Hooks

| Variable | When the adapter calls it | Use it to |
|----------|---------------------------|-----------|
| `AWS_LWA_SNAPSTART_BEFORE_CHECKPOINT_PATH` | Before the snapshot is taken | Drain or close resources that will not survive the snapshot |
| `AWS_LWA_SNAPSTART_AFTER_RESTORE_PATH` | After restore, before serving traffic | Reconnect, refresh credentials, reseed randomness, regenerate unique identifiers |

Both hooks are opt-in and independent — each fires only when its variable is set.

## How it works

The adapter always registers for the SnapStart lifecycle; the Lambda runtime invokes the hooks only when your function runs under SnapStart. When it does, the adapter participates as follows:

1. **Before checkpoint** — if `AWS_LWA_SNAPSTART_BEFORE_CHECKPOINT_PATH` is set, the adapter sends an empty `POST` to that path on your application, then signals Lambda that it is ready for the snapshot.
2. **After restore** — Lambda restores the environment. The adapter first refreshes its own HTTP connection to your application (so it never reuses a connection captured in the snapshot); then, if `AWS_LWA_SNAPSTART_AFTER_RESTORE_PATH` is set, sends an empty `POST` to that path; and finally re-runs the readiness check before admitting traffic.

Each hook is an empty `POST`, and your application must respond with a `2xx` status. A non-`2xx` response, a connection failure, or taking longer than 60 seconds to respond fails the SnapStart phase — initialization for the before-checkpoint hook, restore for the after-restore hook — rather than serving traffic against an improperly prepared application. The final readiness check runs on every restore (whether or not an after-restore path is configured); if your application does not report ready within 10 seconds of restore, the restore fails.

## Why you need the hooks

State captured in a snapshot is shared across every restored environment. Two classes of problem follow:

- **Stale connections.** Database connections, cached DNS, and keep-alive HTTP connections captured in the snapshot are dead by the time the environment is restored. Close them in the before-checkpoint hook and re-establish them in the after-restore hook.
- **Uniqueness and entropy.** Values seeded once at initialization — random number generators, UUID seeds, security tokens — become identical across every restored environment. Reseed them in the after-restore hook.

## Securing the hook paths

The hook paths are control-plane operations. External requests (via API Gateway or ALB) that target a configured hook path receive `403 Forbidden` and are never forwarded to your application. The guard matches the exact configured path, so choose paths your normal application traffic does not use (for example, `/snapstart/before` and `/snapstart/after`).

## Example

```python
from fastapi import FastAPI, Response

app = FastAPI()
pool = None  # your database/connection pool


@app.post("/snapstart/before")
async def before_checkpoint():
    # Close resources that won't survive the snapshot.
    if pool is not None:
        await pool.close()
    return Response(status_code=200)


@app.post("/snapstart/after")
async def after_restore():
    # Re-establish resources and reseed anything that must be unique.
    global pool
    pool = await create_pool()
    return Response(status_code=200)
```

Configure the function with:

```
AWS_LWA_SNAPSTART_BEFORE_CHECKPOINT_PATH=/snapstart/before
AWS_LWA_SNAPSTART_AFTER_RESTORE_PATH=/snapstart/after
```

See the [fastapi-snapstart-zip example](https://github.com/aws/aws-lambda-web-adapter/tree/main/examples/fastapi-snapstart-zip) for a complete, deployable application.
