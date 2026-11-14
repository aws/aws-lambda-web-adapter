# Local Debugging

Lambda Web Adapter lets you develop locally with familiar tools and debuggers — just run your web app and test it directly.

## Running Locally

Since the adapter only activates inside the Lambda execution environment, your app runs as a normal web server locally:

```bash
# Node.js
node index.js

# Python
uvicorn main:app --port 8080

# Java
./mvnw spring-boot:run
```

## Simulating Lambda with SAM CLI

To simulate the full Lambda runtime environment locally, use [AWS SAM CLI](https://docs.aws.amazon.com/serverless-application-model/latest/developerguide/install-sam-cli.html):

```bash
sam local start-api
```

> **Note:** `sam local` starts a Lambda Runtime Interface Emulator on port 8080. Your web application should avoid port 8080 if you plan to use `sam local`. Set `AWS_LWA_PORT` to a different port.

### Zip packages: downloading the adapter layer

For zip-packaged functions (the `*-zip` examples), the adapter comes from the `LambdaAdapterLayerX86` / `LambdaAdapterLayerArm64` layer. Before it starts the container, `sam local` downloads that layer from AWS with a `GetLayerVersion` call, even though your function runs locally. When the download fails, `sam local start-api` still starts, but every request returns `{"message":"Internal server error"}`. Run with `--debug` to see which of these two errors you hit.

**`Invalid Layer name` (region mismatch).** The layer ARN in the template uses `${AWS::Region}`. Without `--region`, SAM CLI resolves that from `AWS_REGION` and otherwise falls back to `us-east-1`, while the Lambda client that downloads the layer uses your profile's region. When the two differ, Lambda rejects the ARN:

```text
botocore.errorfactory.InvalidParameterValueException: An error occurred (InvalidParameterValueException)
when calling the GetLayerVersion operation: Invalid Layer name: arn:aws:lambda:us-east-1:753240598075:layer:LambdaAdapterLayerX86
```

Pass `--region`, which sets both to the same region:

```bash
sam local start-api --region us-west-2
sam local invoke MyFunction --event event.json --region us-west-2
```

Add `--profile my-profile` if you don't use the default credentials.

**`Layers require credentials to download the layers locally.` (no credentials).** SAM CLI found no AWS credentials at all. Configure credentials (for example `aws configure` or `aws sso login`), or pass `--profile`.

SAM CLI caches the downloaded layer, by default under `~/.aws-sam/layers-pkg`. Once the layer is cached, later runs skip the download and don't call AWS, until the layer ARN changes (for example a new layer version or a different region).

If you can't provide AWS credentials at all:

- **Run the app directly.** The adapter only activates inside Lambda, so `uvicorn main:app --port 8000` (or the equivalent for your framework) is the fastest inner loop. See [Running Locally](#running-locally).
- **Use an image-based example** such as `examples/fastapi`. Its Dockerfile copies the adapter from `public.ecr.aws/awsguru/aws-lambda-adapter`, so `sam local` doesn't download a layer.

## Testing Individual Invocations

```bash
sam local invoke MyFunction --event event.json
```
