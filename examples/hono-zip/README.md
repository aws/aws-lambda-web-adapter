# hono-zip

This example shows how to use Lambda Web Adapter to run a [Hono](https://hono.dev) application on the managed Node.js runtime.

The application is a Node.js HTTP server. It uses [@hono/node-server](https://github.com/honojs/node-server) to start the server, and it has no Lambda-specific code. Thus you can run the same `index.js` file on Lambda, in a container, or on your computer.

### How does it work?

Add the Lambda Web Adapter layer to the function and configure the wrapper script.

1. Attach the Lambda Web Adapter layer to the function. The layer contains the Lambda Web Adapter binary and a wrapper script.
    1. x86_64: `arn:aws:lambda:${AWS::Region}:753240598075:layer:LambdaAdapterLayerX86:30`
    2. arm64: `arn:aws:lambda:${AWS::Region}:753240598075:layer:LambdaAdapterLayerArm64:30`
2. Set the Lambda environment variable `AWS_LAMBDA_EXEC_WRAPPER` to `/opt/bootstrap`. This is the wrapper script in the layer.
3. Set the function handler to the startup command `run.sh`. The wrapper script runs this command to start the application.

The application listens on the port that the `PORT` environment variable gives. Lambda Web Adapter uses the same variable to find the application.

For more information about wrapper scripts, refer to the [Lambda documentation](https://docs.aws.amazon.com/lambda/latest/dg/runtimes-modify.html#runtime-wrapper).

### Build and Deploy

Run these commands to build the application and deploy it to Lambda.

```bash
sam build
sam deploy --guided
```

When the deployment is complete, find the `HonoApi` output. Its value is the URL of the API Gateway endpoint.

### Verify it works

Send a request to the `HonoApi` URL.

```bash
curl https://<api-id>.execute-api.<region>.amazonaws.com/
```

```json
{"message":"Hello from Hono!","requestId":"<request-id>","functionName":"<function-name>"}
```

Lambda Web Adapter sends the request context from API Gateway and the Lambda context to the application. It puts them in the `x-amzn-request-context` and `x-amzn-lambda-context` headers. The application reads the request ID from the first header and the function name from the second header.

The two contexts contain your AWS account ID. Thus the application does not return the full contexts.

The application also has a route with a path parameter.

```bash
curl https://<api-id>.execute-api.<region>.amazonaws.com/hello/lambda
```

```json
{"message":"Hello, lambda!"}
```

### Run it locally

You can run the application without Lambda.

```bash
cd app
npm install
PORT=8000 npm start
```

Then send a request from a second terminal.

```bash
curl http://localhost:8000/
```

### Clean up

Delete the stack when you do not need the example.

```bash
sam delete
```
