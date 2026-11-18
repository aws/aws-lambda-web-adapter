# expressjs-otel-zip

This example shows how to trace an express.js application running on Lambda Web Adapter with [AWS Distro for OpenTelemetry (ADOT)](https://aws-otel.github.io/) and send the traces to AWS X-Ray.

### How does it work?

The adapter's wrapper script starts `run.sh`, and `run.sh` loads the ADOT auto-instrumentation into Node.js before `app.js` runs.

1. Attach the Lambda Adapter layer to your function: `arn:aws:lambda:${AWS::Region}:753240598075:layer:LambdaAdapterLayerX86:30` (arm64: `arn:aws:lambda:${AWS::Region}:753240598075:layer:LambdaAdapterLayerArm64:30`). This is the only layer the example needs.
2. Keep `AWS_LAMBDA_EXEC_WRAPPER` set to the adapter's `/opt/bootstrap`. A function has only one exec wrapper, and the adapter needs it.

    **No ADOT layer is needed.** The ADOT Lambda layer (`AWSOpenTelemetryDistroJs`) contains this same SDK plus an `/opt/otel-instrument` exec wrapper, and that wrapper can't be used next to the adapter's. The npm package in `node_modules` does the work instead: inside Lambda it sends traces straight to the function's X-Ray endpoint over UDP, with no collector or layer involved.
3. Install `@aws/aws-distro-opentelemetry-node-autoinstrumentation` as a dependency and ship it in the function's `node_modules`. Don't bundle it: the auto-instrumentation patches `http` and `express` when they are `require`d. If you do use a bundler, mark it and `@opentelemetry/*` as external.
4. Set the OpenTelemetry options in `run.sh`:

    ```bash
    #!/bin/bash
    export NODE_OPTIONS="--require @aws/aws-distro-opentelemetry-node-autoinstrumentation/register"
    export OTEL_SERVICE_NAME=expressjs-otel
    export OTEL_METRICS_EXPORTER=${OTEL_METRICS_EXPORTER:-none}
    export OTEL_LOGS_EXPORTER=${OTEL_LOGS_EXPORTER:-none}
    exec node app.js
    ```

    Traces go to X-Ray directly, as described above. Nothing receives metrics or logs over OTLP in Lambda, so those two exporters are turned off. Without that, the SDK would try `localhost:4318` and log a connection error on every flush.
5. Turn on active tracing (`Tracing: Active`). SAM then adds the `AWSXrayWriteOnlyAccess` managed policy to the function's role.

### Build and Deploy

Run the following commands to build and deploy the application to Lambda.

```bash
sam build
sam deploy --guided
```

When the deployment completes, take note of HelloWorldApi's Value. It is the API Gateway endpoint URL.

### Verify it works

Open HelloWorldApi's URL in a browser. You should see "Hi there!" on the page.

Then open the CloudWatch console, choose **Traces** (or the X-Ray trace map), and look for the `expressjs-otel` service. Spans are batched, so invoke the function a few times if the first traces don't appear right away. A trace has the `GET /` server span with express middleware and `request handler - /` child spans.
