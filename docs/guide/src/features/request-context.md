# Request & Lambda Context

Lambda Web Adapter forwards API Gateway request context and Lambda invocation context to your web application via HTTP headers.

## Request Context

API Gateway sends metadata (requestId, requestTime, apiId, identity, authorizer) for each request. This is forwarded in the `x-amzn-request-context` header as a JSON string.

The identity and authorizer fields are particularly useful for client authorization.

```javascript
// Express.js example
app.get('/', (req, res) => {
    const requestContext = JSON.parse(req.headers['x-amzn-request-context']);
    const sourceIp = requestContext.identity?.sourceIp;
    const userId = requestContext.authorizer?.claims?.sub;
});
```

See the [API Gateway docs](https://docs.aws.amazon.com/apigateway/latest/developerguide/set-up-lambda-proxy-integrations.html#api-gateway-simple-proxy-for-lambda-input-format) for the full request context schema.

## Lambda Context

The Lambda invocation context (function name, memory, deadline, request ID, etc.) is forwarded in the `x-amzn-lambda-context` header as a JSON string.

```python
# Flask example
import json, time

@app.route('/')
def index():
    lambda_context = json.loads(request.headers.get('x-amzn-lambda-context', '{}'))
    request_id = lambda_context.get('request_id')
    function_name = lambda_context.get('env_config', {}).get('function_name')
    # `deadline` is the invocation deadline in epoch milliseconds
    remaining_ms = lambda_context.get('deadline', 0) - int(time.time() * 1000)
```

The header is the JSON serialization of the Rust runtime's [`lambda_runtime::Context`](https://docs.rs/lambda_runtime/latest/lambda_runtime/struct.Context.html). Its field names follow the Rust struct (snake_case), not the Node.js (`awsRequestId`), Python (`aws_request_id`) or .NET (`AwsRequestId`) context objects:

```json
{
  "request_id": "8f5e9b1a-...",
  "deadline": 1731456000000,
  "invoked_function_arn": "arn:aws:lambda:us-east-1:123456789012:function:my-fn",
  "xray_trace_id": "Root=1-...",
  "client_context": null,
  "identity": null,
  "tenant_id": null,
  "env_config": {
    "function_name": "my-fn",
    "memory": 512,
    "version": "$LATEST",
    "log_stream": "",
    "log_group": ""
  }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `request_id` | string | AWS request ID of the invocation |
| `deadline` | number | Invocation deadline, in milliseconds since the Unix epoch |
| `invoked_function_arn` | string | ARN used to invoke the function |
| `xray_trace_id` | string or null | X-Ray trace header |
| `client_context` | object or null | Set only for invocations from the AWS Mobile SDK: `client` (camelCase keys such as `installationId`, `appTitle`), `custom`, `environment` |
| `identity` | object or null | Set only for Amazon Cognito identity callers: `identityId`, `identityPoolId` (camelCase) |
| `tenant_id` | string or null | Tenant ID for [multi-tenant](https://docs.aws.amazon.com/lambda/latest/dg/tenant-isolation.html) functions |
| `env_config.function_name` | string | Function name |
| `env_config.memory` | number | Configured memory, in MB |
| `env_config.version` | string | Function version being executed |
| `env_config.log_stream` | string | Always empty (see below) |
| `env_config.log_group` | string | Always empty (see below) |

> **Note:** `env_config.log_stream` and `env_config.log_group` are always empty strings. The adapter runs as a Lambda extension, and [extensions do not receive](https://docs.aws.amazon.com/lambda/latest/dg/runtimes-extensions-api.html#runtimes-extensions-registration-api-e) `AWS_LAMBDA_LOG_GROUP_NAME` or `AWS_LAMBDA_LOG_STREAM_NAME`. Your application process does, so read them from its environment instead.
