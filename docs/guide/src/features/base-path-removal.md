# Base Path Removal

When using API Gateway with multiple resources routed to different Lambda functions, your app may not be aware of the base path prefix.

## Example

If your API Gateway has:
- `/orders/{proxy+}` → Orders Lambda
- `/catalog/{proxy+}` → Catalog Lambda

The Orders app doesn't know about the `/orders` prefix. Set `AWS_LWA_REMOVE_BASE_PATH` to strip it:

```
AWS_LWA_REMOVE_BASE_PATH=/orders
```

A request to `/orders/123` will be forwarded to your app as `/123`.

See the [SpringBoot example](https://github.com/aws/aws-lambda-web-adapter/tree/main/examples/springboot) for a working implementation.
