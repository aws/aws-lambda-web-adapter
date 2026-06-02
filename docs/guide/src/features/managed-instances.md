# Lambda Managed Instances

Lambda Web Adapter supports [Lambda Managed Instances](https://docs.aws.amazon.com/lambda/latest/dg/lambda-managed-instances.html), which allows a single Lambda execution environment to handle multiple concurrent requests. This can improve throughput and reduce costs for I/O-bound workloads.

## How It Works

When running on Lambda Managed Instances, the adapter automatically handles concurrent invocations by forwarding multiple requests to your web application simultaneously. Since most web frameworks (Express.js, FastAPI, Spring Boot, etc.) are already designed to handle concurrent requests, your application should work without modification.

## Considerations

When using Lambda Managed Instances, keep these points in mind:

- **Shared state**: Global variables and in-memory caches are shared across concurrent requests. Ensure your application handles shared state safely.
- **Connection pooling**: Use connection pools for databases and external services rather than single connections.
- **File system**: The `/tmp` directory is shared across concurrent requests. Use unique file names or implement file locking to avoid conflicts.
- **Resource limits**: Memory and CPU are shared across concurrent requests. Monitor resource usage under concurrent load.

Lambda Managed Instances works with both buffered and response streaming modes.

Check out the [FastAPI with Response Streaming on Lambda Managed Instances](https://github.com/aws/aws-lambda-web-adapter/tree/main/examples/fastapi-response-streaming-lmi) example.
