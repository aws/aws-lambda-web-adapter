# FastAPI with Lambda SnapStart

This example shows how to use Lambda Web Adapter's SnapStart hooks to drain and re-establish a connection pool around the snapshot/restore boundary, running a FastAPI application on the managed python runtime.

### How does it work?

We add the Lambda Web Adapter layer to the function and configure the wrapper script.

1. attach Lambda Adapter layer to your function. This layer contains the Lambda Adapter binary and a wrapper script.
    1. x86_64: `arn:aws:lambda:${AWS::Region}:753240598075:layer:LambdaAdapterLayerX86:28`
    2. arm64: `arn:aws:lambda:${AWS::Region}:753240598075:layer:LambdaAdapterLayerArm64:28`
2. configure Lambda environment variable `AWS_LAMBDA_EXEC_WRAPPER` to `/opt/bootstrap`. This is a wrapper script included in the layer.
3. set function handler to a startup command: `run.sh`. The wrapper script will execute this command to boot up your application.

To get more information of Wrapper script, please read Lambda documentation [here](https://docs.aws.amazon.com/lambda/latest/dg/runtimes-modify.html#runtime-wrapper).

#### SnapStart hooks

[Lambda SnapStart](https://docs.aws.amazon.com/lambda/latest/dg/snapstart.html) initializes your function, takes a snapshot of the initialized execution environment, and restores from that snapshot to serve invocations. Some resources do not survive the snapshot (open connections) and some values must not be shared across every restored environment (unique ids, seeds). The Lambda Web Adapter bridges the SnapStart lifecycle to your inner web app through two opt-in environment variables:

- `AWS_LWA_SNAPSTART_BEFORE_CHECKPOINT_PATH` is set to `/snapstart/before`. Before the snapshot is taken, the adapter sends an empty HTTP `POST` to this path. The app uses it to drain and close resources that will not survive the snapshot (in this example, it closes the connection pool).
- `AWS_LWA_SNAPSTART_AFTER_RESTORE_PATH` is set to `/snapstart/after`. After the environment is restored and before traffic is served, the adapter sends an empty HTTP `POST` to this path. The app uses it to re-establish connections and regenerate per-environment unique values (in this example, it reconnects the pool and generates a fresh `connection_id`).

Both hook routes must return a `2xx` status code. A non-2xx response or a connection failure fails the SnapStart phase. The adapter also automatically refreshes its own HTTP connection to the inner app after restore, so you do not have to manage the adapter's client.

These hook routes are protected: the adapter only allows them to be invoked internally during the SnapStart lifecycle. External callers that request `/snapstart/before` or `/snapstart/after` receive a `403 Forbidden`.

### Build and Deploy

Run the following commands to build and deploy the application to lambda.

```bash
sam build --use-container
sam deploy --guided
```
When the deployment completes, take note of FastAPISnapStartApi's Value. It is the API Gateway endpoint URL.

### Verify it works

Open FastAPISnapStartApi's URL in a browser. The `/` response shows `connected: true` and a `connection_id`, for example:

```json
{
  "message": "Hello from FastAPI on Lambda SnapStart",
  "connected": true,
  "connection_id": 482913007
}
```

After a SnapStart restore, the `connection_id` is regenerated rather than shared across every restored environment, because the adapter calls the after-restore hook (`/snapstart/after`), which reconnects the pool and generates a fresh id. This is exactly the behavior you want for any per-environment value (connections, random seeds, unique identifiers) that must not be duplicated across restored snapshots.
