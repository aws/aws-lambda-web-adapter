# Request & Lambda Context

Lambda Web Adapter forwards API Gateway request context and Lambda invocation context to your web application via HTTP headers.

## Request Context

API Gateway sends metadata (requestId, requestTime, apiId, identity, authorizer) for each request. VPC Lattice event structure version V2 also sends request context metadata. These contexts are forwarded in the `x-amzn-request-context` header as a JSON string.

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

For VPC Lattice, set the target group's Lambda event structure version to V2. The adapter supports the V2 payload format and forwards its request context. V1 events are treated as non-HTTP events and passed through to `AWS_LWA_PASS_THROUGH_PATH`, which defaults to `/events`.

## Lambda Context

The Lambda invocation context (function name, memory, timeout, request ID, etc.) is forwarded in the `x-amzn-lambda-context` header as a JSON string.

```python
# Flask example
@app.route('/')
def index():
    lambda_context = json.loads(request.headers.get('x-amzn-lambda-context', '{}'))
    function_name = lambda_context.get('function_name')
    remaining_time = lambda_context.get('deadline_ms')
```

See the [Lambda Context docs](https://docs.aws.amazon.com/lambda/latest/dg/nodejs-context.html) for the full list of available properties.
