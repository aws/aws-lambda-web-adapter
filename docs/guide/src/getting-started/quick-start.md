# Quick Start

AWS Lambda Web Adapter works with Lambda functions packaged as both Docker images and Zip packages. Choose the approach that fits your workflow.

## Which Packaging Method?

| Method | Best For |
|--------|----------|
| [Docker Images](./docker-images.md) | Full control over runtime, multi-stage builds, consistent local/cloud environments |
| [Zip Packages](./zip-packages.md) | AWS managed runtimes, simpler deployment, smaller package sizes |

## Minimal Example (Docker)

Add one line to your existing Dockerfile:

```dockerfile
COPY --from=public.ecr.aws/awsguru/aws-lambda-adapter:1.0.0 /lambda-adapter /opt/extensions/lambda-adapter
```

That's it. Your web app now runs on Lambda.

## Minimal Example (Zip)

1. Attach the Lambda Web Adapter layer to your function
2. Set `AWS_LAMBDA_EXEC_WRAPPER` to `/opt/bootstrap`
3. Set your function handler to your app's startup script (e.g. `run.sh`)

## Port Configuration

By default, the adapter expects your app on port `8080`. Set the `AWS_LWA_PORT` environment variable (or `PORT`) to change it.

> **Avoid ports 9001 and 3000.** Port 9001 is used by the Lambda Runtime API, and port 3000 is used by the CloudWatch Lambda Insight extension. Inside Lambda, your app runs as a non-root user and cannot listen on ports below 1024.
