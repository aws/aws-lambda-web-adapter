# expressjs-otel-zip

This example shows how to trace an express.js application running on Lambda Web Adapter with [AWS Distro for OpenTelemetry (ADOT)](https://aws-otel.github.io/) and send the traces to AWS X-Ray.

### How does it work?

The adapter's wrapper script starts `run.sh`, and `run.sh` loads the ADOT auto-instrumentation into Node.js before `app.js` runs.

1. Attach two layers to your function:
    1. the Lambda Adapter layer: `arn:aws:lambda:${AWS::Region}:753240598075:layer:LambdaAdapterLayerX86:30` (arm64: `arn:aws:lambda:${AWS::Region}:753240598075:layer:LambdaAdapterLayerArm64:30`)
    2. the ADOT Node.js layer, `AWSOpenTelemetryDistroJs`. The account ID and version differ by Region, so the template takes it as the `AdotLayerArn` parameter. Most Regions use `arn:aws:lambda:<region>:615299751070:layer:AWSOpenTelemetryDistroJs:16` (v0.13.0). Check the [layer ARN table](https://github.com/aws-observability/aws-otel-js-instrumentation/releases/latest) for your Region. One ARN covers both x86_64 and arm64.
2. Keep `AWS_LAMBDA_EXEC_WRAPPER` set to the adapter's `/opt/bootstrap`. Don't switch it to ADOT's `/opt/otel-instrument`: a function has only one exec wrapper, and the adapter needs it.
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

    Inside Lambda, ADOT sends traces over UDP to the function's X-Ray endpoint, so no collector is needed. There is no collector for metrics or logs either, which is why those two exporters are turned off.
5. Turn on active tracing (`Tracing: Active`). SAM then adds the `AWSXrayWriteOnlyAccess` managed policy to the function's role.

### Build and Deploy

Run the following commands to build and deploy the application to Lambda.

```bash
sam build
sam deploy --guided
```

`sam deploy --guided` asks for `AdotLayerArn`; use the ARN for the Region you deploy to. When the deployment completes, take note of HelloWorldApi's Value. It is the API Gateway endpoint URL.

### Verify it works

Open HelloWorldApi's URL in a browser. You should see "Hi there!" on the page.

Then open the CloudWatch console, choose **Traces** (or the X-Ray trace map), and look for the `expressjs-otel` service. Spans are batched, so invoke the function a few times if the first traces don't appear right away. A trace has the `GET /` server span with express middleware and `request handler - /` child spans.
