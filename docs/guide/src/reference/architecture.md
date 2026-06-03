# Architecture

## Overview

AWS Lambda Web Adapter is a Lambda Extension that bridges the gap between Lambda's event-driven invocation model and traditional HTTP web applications.

## Supported Triggers

- Amazon API Gateway REST API
- Amazon API Gateway HTTP API (v2 event format)
- Application Load Balancer (ALB)
- Lambda Function URLs
- Non-HTTP triggers (SQS, SNS, S3, DynamoDB, Kinesis, Kafka, EventBridge, Bedrock Agents) via pass-through

## Request Flow

1. Lambda receives an event from a trigger (API Gateway, ALB, Function URL, etc.)
2. The adapter converts the Lambda event into a standard HTTP request
3. The HTTP request is forwarded to your web application on the configured port
4. Your app processes the request and returns an HTTP response
5. The adapter converts the HTTP response back into a Lambda event response
6. Lambda returns the response to the caller

## Extension Lifecycle

The adapter runs as a Lambda Extension (since v0.2.0):

1. **Init phase**: Lambda starts the adapter extension and your web application process
2. **Readiness check**: The adapter polls your app until it responds (every 10ms)
3. **Invoke phase**: The adapter starts the Lambda runtime client and forwards events
4. **Shutdown phase**: Lambda sends SIGTERM, allowing graceful shutdown

When running outside Lambda (EC2, Fargate, local), the adapter does not start — your app runs as a normal web server.

## Binary Response Encoding

The adapter automatically detects binary responses based on the `Content-Type` header and encodes them appropriately for the Lambda response format.

## Technology Stack

- Written in Rust
- Built on [AWS Lambda Rust Runtime](https://github.com/aws/aws-lambda-rust-runtime) (`lambda_http` crate)
- Uses `hyper` as the HTTP client
- Uses `tower` for middleware (compression)
- Compiled to static musl binaries for x86_64 and aarch64
