# FastMCP example

A basic FastMCP (Model Context Protocol) server example. You can build and test it locally as a typical MCP server application.

Using AWS Lambda Web Adapter, you can package this web application into Docker image, push to ECR, and deploy to Lambda, ECS/EKS, or EC2.

The application can be deployed in an AWS account using the [Serverless Application Model](https://github.com/awslabs/serverless-application-model). The `template.yaml` file in the root folder contains the application definition.

The top level folder is a typical AWS SAM project. The `my_mcp_server` directory is a FastMCP application with a [Dockerfile](my_mcp_server/Dockerfile).

```dockerfile
FROM --platform=linux/amd64 public.ecr.aws/docker/library/python:3.14-slim AS builder
WORKDIR /var/task
COPY requirements.txt ./
RUN pip install --no-cache-dir --target=/var/task/deps -r requirements.txt

FROM --platform=linux/amd64 public.ecr.aws/docker/library/python:3.14-slim
COPY --from=public.ecr.aws/awsguru/aws-lambda-adapter:1.0.0 /lambda-adapter /opt/extensions/lambda-adapter
ENV PORT=8000 PYTHONPATH=/var/task/deps
ENV AWS_LWA_READINESS_CHECK_PATH=/healthz
ENV AWS_LWA_READINESS_CHECK_HEALTHY_STATUS=100-499
WORKDIR /var/task
COPY --from=builder /var/task/deps ./deps
COPY *.py ./
CMD exec python -m uvicorn --port=$PORT app:app
```

Line 7 copies lambda web adapter binary into /opt/extensions. This is the change to run the FastMCP application on Lambda.

```dockerfile
COPY --from=public.ecr.aws/awsguru/aws-lambda-adapter:1.0.0 /lambda-adapter /opt/extensions/lambda-adapter
```

## Pre-requisites

The following tools should be installed and configured.
* [AWS CLI](https://aws.amazon.com/cli/)
* [SAM CLI](https://github.com/awslabs/aws-sam-cli)
* [Python](https://www.python.org/)
* [Docker](https://www.docker.com/products/docker-desktop)


## Deploy to Lambda
Navigate to the sample's folder and use the SAM CLI to build a container image
```shell
$ sam build
```

This command compiles the application and prepares a deployment package in the `.aws-sam` sub-directory.

To deploy the application in your AWS account, you can use the SAM CLI's guided deployment process and follow the instructions on the screen

```shell
$ sam deploy --guided
```
Please take note of the container image name.
Once the deployment is completed, the SAM CLI will print out the stack's outputs, including the new application URL. 

## Test with FastMCP client

You can test the deployed MCP server using the FastMCP Python client:

```python
import asyncio
from fastmcp import Client

client = Client("https://xxxxxxxxxx.execute-api.us-west-2.amazonaws.com/mcp")

async def call_tool(name: str):
    async with client:
        result = await client.call_tool("greet", {"name": name})
        print(result)

asyncio.run(call_tool("World"))
```
