# AWS Lambda Web Adapter

Run web applications on AWS Lambda — with zero code changes.

AWS Lambda Web Adapter lets developers build web apps with familiar frameworks (Express.js, Next.js, Flask, SpringBoot, ASP.NET, Laravel, and anything that speaks HTTP 1.1/1.0) and run them on AWS Lambda. The same Docker image can run on AWS Lambda, Amazon EC2, AWS Fargate, and local machines.

![Lambda Web Adapter Overview](https://github.com/aws/aws-lambda-web-adapter/raw/main/docs/images/lambda-adapter-overview.png)

## Key Features

- Supports Amazon API Gateway (REST & HTTP API), Lambda Function URLs, and Application Load Balancer
- Works with Lambda managed runtimes, custom runtimes, and Docker OCI images
- Supports Lambda Managed Instances for multi-concurrent request handling
- Framework and language agnostic — no new code dependencies
- Automatic binary response encoding
- Graceful shutdown support
- Response payload compression (gzip/brotli)
- Response streaming
- Multi-tenancy via tenant ID propagation
- Non-HTTP event trigger support (SQS, SNS, S3, DynamoDB, Kinesis, Kafka, EventBridge, Bedrock Agents)

## How It Works

AWS Lambda Web Adapter runs as a [Lambda Extension](https://docs.aws.amazon.com/lambda/latest/dg/lambda-extensions.html). When the Docker image runs inside AWS Lambda, the adapter starts automatically alongside your application. When running outside Lambda (EC2, Fargate, locally), the adapter does not run at all — your app just works as a normal web server.

1. Lambda starts the adapter extension and your web application
2. The adapter performs a readiness check against your app (default: `http://127.0.0.1:8080/`)
3. Once your app responds, the adapter starts the Lambda runtime client
4. Incoming Lambda events are converted to HTTP requests and forwarded to your app
5. Your app's HTTP responses are converted back to Lambda event responses

![Lambda Adapter Runtime](https://github.com/aws/aws-lambda-web-adapter/raw/main/docs/images/lambda-adapter-runtime.png)

## Pre-built Binaries

Pre-compiled binaries are available from the public ECR repository:

```
public.ecr.aws/awsguru/aws-lambda-adapter
```

Multi-arch images are provided for both x86_64 and arm64 architectures.
