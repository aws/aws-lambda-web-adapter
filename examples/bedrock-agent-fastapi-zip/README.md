# Agents for Amazon Bedrock with FastAPI example

This project demonstrates the integration of "Agents for Amazon Bedrock" with a FastAPI application on AWS Lambda. It showcases how to effectively build an Agents for Amazon Bedrock within an serverless FastAPI application environment.

The application can be deployed in an AWS account using the [Serverless Application Model](https://github.com/aws/serverless-application-model). The `template.yaml` file in the root folder contains the application definition.

## How does it work?

[Agents for Amazon Bedrock](https://docs.aws.amazon.com/bedrock/latest/userguide/agents.html) offers you the ability to build and configure autonomous agents in your application.

Action groups define the tasks that you want your agent to help customers carry out. An action group consists of the following components that you set up.

- An OpenAPI schema that define the APIs that your action group should call. Your agent uses the API schema to determine the fields it needs to elicit from the customer to populate for the API request.

- A Lambda function that defines the business logic for the action that your agent will carry out.

In this example, we build a FastAPI app to query S3 buckets in the user's AWS account. The FastAPI apps automatically generate OpenAPI schema. And Lambda Web Adapter passes through the Bedrock Agent Action invocation payload to the FastAPI at path `/events` via Lambda Web Adapter's new non-http pass through feature. In the FastAPI app, a middleware intercepts the payload to `/events` route, transforms it into a http request, and continues to invoke the correct FastAPI routes. In this way, developers can build a Bedrocket Agent Action Group with a single Lambda Function.

## Pre-requisites

The following tools should be installed and configured.

* [AWS CLI](https://aws.amazon.com/cli/)
* [SAM CLI](https://github.com/aws/aws-sam-cli)
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
pip install boto3
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
sam build --use-container
```

This command compiles the application and prepares a deployment package in the `.aws-sam` sub-directory.

To deploy the application in your AWS account, you can use the SAM CLI's guided deployment process and follow the instructions on the screen

```shell
sam deploy --guided --capabilities CAPABILITY_NAMED_IAM
```

## Test locally

Sample event exists in events directory. You can test locally with bellow command.

```shell
sam local invoke --event events/s3_bucket_count.json
```

## Test

Test your agent on Management Console. (see [reference](https://docs.aws.amazon.com/bedrock/latest/userguide/agents-test.html))
