# Agents for Amazon Bedrock with FastAPI example

This project demonstrates the integration of "Agents for Amazon Bedrock" with a FastAPI application on AWS Lambda. It showcases how to effectively build an Agents for Amazon Bedrock within an serverless FastAPI application environment.

The application can be deployed in an AWS account using the [Serverless Application Model](https://github.com/awslabs/serverless-application-model). The `template.yaml` file in the root folder contains the application definition.

The top level folder is a typical AWS SAM project. The `app` directory is an FastAPI application with a [Dockerfile](app/Dockerfile).

```dockerfile
FROM public.ecr.aws/docker/library/python:3.12.0-slim
COPY --from=public.ecr.aws/awsguru/aws-lambda-adapter:1.0.0 /lambda-adapter /opt/extensions/lambda-adapter
ENV PORT=8000 AWS_LWA_READINESS_CHECK_PROTOCOL=tcp 
WORKDIR /var/task
COPY requirements.txt ./
RUN python -m pip install -r requirements.txt
COPY *.py ./
CMD exec uvicorn --port=$PORT main:app
```

Line 2 copies lambda adapter binary into /opt/extensions. This is the only change to run the FastAPI application on Lambda.

```dockerfile
COPY --from=public.ecr.aws/awsguru/aws-lambda-adapter:1.0.0 /lambda-adapter /opt/extensions/lambda-adapter
```

## Pre-requisites

The following tools should be installed and configured.

* [AWS CLI](https://aws.amazon.com/cli/)
* [SAM CLI](https://github.com/awslabs/aws-sam-cli)
* [Python](https://www.python.org/)
* [Docker](https://www.docker.com/products/docker-desktop)

## Generate OpenAPI schema

Before you create your agent, you should set up action groups that you want to add to your agent. When you create an action group, you must define the APIs that the agent can invoke with an OpenAPI schema in JSON or YAML format. (see [reference](https://docs.aws.amazon.com/bedrock/latest/userguide/agents-api-schema.html))

FastAPI can generate OpenAPI schema.

Please install the required dependency in a virtual environment first.

```shell
python3 -m venv .venv
source .venv/bin/activate
pip install -r app/requirements.txt
cd app/
```

(in app directory)

```shell
python -c "import main;import json; print(json.dumps(main.app.openapi()))" > openapi.json
```

## Update template.yaml

Update the Payload part of ActionGroups defined in template.yaml with the OpenAPI schema value.

```yaml
ApiSchema:
    Payload: '<<Open API schema>>'
```

(in example root directory)

```shell
sed -i "s@\\\\n@\\\\\\\\\\\\\\\\n@g" app/openapi.json
sed -i "s@<<Open API schema>>@`cat app/openapi.json`@g" template.yaml
```

## Deploy to Lambda

Navigate to the sample's folder and use the SAM CLI to build a container image

```shell
sam build
```

This command compiles the application and prepares a deployment package in the `.aws-sam` sub-directory.

To deploy the application in your AWS account, you can use the SAM CLI's guided deployment process and follow the instructions on the screen

```shell
sam deploy --guided --capabilities CAPABILITY_NAMED_IAM
```

Sample event exists in events directory. You can test locally with bellow command.

```shell
sam local invoke --event events/s3_bucket_count.json
```

## Test

Test your agent on Management Console. (see [reference](https://docs.aws.amazon.com/bedrock/latest/userguide/agents-test.html))
