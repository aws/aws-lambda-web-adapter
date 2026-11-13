# Bun Hono example

This example shows how to use Lambda Web Adapter to run a [Hono](https://hono.dev) application on [Bun](https://bun.sh) in a container image.

The application is a Bun HTTP server in TypeScript. Bun runs the `index.ts` file directly, thus the application has no build step. The application has no Lambda-specific code. You can run the same image on Lambda, on Amazon ECS, or on your computer.

The top level folder is an AWS SAM project. The `app` directory contains the application and its [Dockerfile](app/Dockerfile).

This line of the Dockerfile copies the Lambda Web Adapter binary into `/opt/extensions`. It is the only change that Lambda needs.

```dockerfile
COPY --from=public.ecr.aws/awsguru/aws-lambda-adapter:1.1.0 /lambda-adapter /opt/extensions/lambda-adapter
```

The examples in this repository get their base images from Amazon ECR Public. Bun has no official image there. Thus the Dockerfile starts from the Node.js image and installs Bun with npm.

The application listens on the port that the `PORT` environment variable gives. Lambda Web Adapter uses the same variable to find the application.

## Pre-requisites

Install and configure these tools.

* [AWS CLI](https://aws.amazon.com/cli/)
* [SAM CLI](https://github.com/aws/aws-sam-cli)
* [Docker](https://www.docker.com/products/docker-desktop)

## Deploy to Lambda

Go to the folder of this example. Use SAM CLI to build the container image.

```shell
sam build
```

Deploy the application to your AWS account. SAM CLI asks you for the deployment settings.

```shell
sam deploy --guided
```

When the deployment is complete, find the `BunHonoApi` output. Its value is the URL of the API Gateway endpoint. Send a request to that URL.

```shell
curl https://<api-id>.execute-api.<region>.amazonaws.com/
```

```json
{"message":"Hello from Hono on Bun!","requestId":"<request-id>","functionName":"<function-name>"}
```

Lambda Web Adapter sends the request context from API Gateway and the Lambda context to the application. It puts them in the `x-amzn-request-context` and `x-amzn-lambda-context` headers. The application reads the request ID from the first header and the function name from the second header.

The two contexts contain your AWS account ID. Thus the application does not return the full contexts.

The application also has a route with a path parameter.

```shell
curl https://<api-id>.execute-api.<region>.amazonaws.com/hello/lambda
```

```json
{"message":"Hello, lambda!"}
```

## Run the docker locally

You can run the same image without Lambda. Build the image and start a container.

```shell
docker build -t bun-hono app
docker run --rm -p 8000:8000 bun-hono
```

Then send a request from a second terminal.

```shell
curl http://localhost:8000/
```

## Clean up

Delete the stack when you do not need the example.

```shell
sam delete
```
