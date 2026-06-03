# Response Streaming

Lambda Web Adapter supports [Lambda response streaming](https://aws.amazon.com/blogs/compute/introducing-aws-lambda-response-streaming/), allowing your app to stream responses back to clients as they are generated.

## Enabling Response Streaming

Set the invoke mode environment variable:

```
AWS_LWA_INVOKE_MODE=response_stream
```

This should match your Lambda Function URL's invoke mode configuration.

## When to Use

Response streaming is useful for:

- Server-sent events (SSE)
- Large file downloads
- Real-time data feeds
- Reducing time-to-first-byte for slow-generating responses

## Limitations

- Compression (`AWS_LWA_ENABLE_COMPRESSION`) is not supported with response streaming. If both are enabled, compression is automatically disabled with a warning.
- Response streaming works with Lambda Function URLs and API Gateway. ALB does not support streaming.

## Examples

- [FastAPI with Response Streaming](https://github.com/aws/aws-lambda-web-adapter/tree/main/examples/fastapi-response-streaming)
- [FastAPI with Response Streaming in Zip](https://github.com/aws/aws-lambda-web-adapter/tree/main/examples/fastapi-response-streaming-zip)
- [Next.js Response Streaming](https://github.com/aws/aws-lambda-web-adapter/tree/main/examples/nextjs-response-streaming)
- [SpringBoot Response Streaming](https://github.com/aws/aws-lambda-web-adapter/tree/main/examples/springboot-response-streaming-zip)
- [FastHTML with Response Streaming](https://github.com/aws/aws-lambda-web-adapter/tree/main/examples/fasthtml-response-streaming)
