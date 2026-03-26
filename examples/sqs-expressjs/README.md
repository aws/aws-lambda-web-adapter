# SQS Express.js example

This project demonstrates the integration of Amazon Simple Queue Service (SQS) with an Express.js application on AWS Lambda. It showcases how to effectively manage and process messages from an SQS queue within an serverless Express.js application environment.

In this Express.js application integrated with Amazon SQS, there is no explicit code required to poll the SQS queue. The AWS Lambda handles the polling of the SQS queue and Lambda Web Adapter forwards the event payload to the Express.js application vith a HTTP POST request. This simplifies the application code and allows developers to focus on processing the event payload rather than managing the queue polling logic.

The application can be deployed in an AWS account using the [Serverless Application Model](https://github.com/awslabs/serverless-application-model). The `template.yaml` file in the root folder contains the application definition.

The top level folder is a typical AWS SAM project. The `app` directory is an express.js application with a [Dockerfile](app/Dockerfile).

```dockerfile
FROM public.ecr.aws/docker/library/node:20-slim
COPY --from=public.ecr.aws/awsguru/aws-lambda-adapter:1.0.0 /lambda-adapter /opt/extensions/lambda-adapter
ENV PORT=8000 AWS_LWA_READINESS_CHECK_PROTOCOL=tcp
WORKDIR "/var/task"
ADD src/package.json /var/task/package.json
ADD src/package-lock.json /var/task/package-lock.json
RUN npm install --omit=dev
ADD src/ /var/task
CMD ["node", "index.js"]
```

Line 2 copies lambda adapter binary into /opt/extensions. This is the only change to run the express.js application on Lambda.

```dockerfile
COPY --from=public.ecr.aws/awsguru/aws-lambda-adapter:1.0.0 /lambda-adapter /opt/extensions/lambda-adapter
```

## Pre-requisites

The following tools should be installed and configured.

* [AWS CLI](https://aws.amazon.com/cli/)
* [SAM CLI](https://github.com/awslabs/aws-sam-cli)
* [Node](https://nodejs.org/en/)
* [Docker](https://www.docker.com/products/docker-desktop)

## Build

Navigate to the sample's folder and use the SAM CLI to build a container image

```shell
sam build
```
## Deploy

This command compiles the application and prepares a deployment package in the `.aws-sam` sub-directory.

To deploy the application in your AWS account, you can use the SAM CLI's guided deployment process and follow the instructions on the screen

```shell
sam deploy --guided
```

Please take note of the container image name.
Once the deployment is completed, the SAM CLI will print out the stack's outputs, including the new sqs queue URL.

```shell
...
---------------------------------------------------------------------------------------------------------------------------------------
Outputs                                                                                                                               
---------------------------------------------------------------------------------------------------------------------------------------
Key                 SqsQueueUrl                                                                                                       
Description         SQS URL the express Lambda Function will receive messages from                                                    
Value               https://sqs.us-west-2.amazonaws.com/xxxxxxxx/xxxxxxxx                                                    
---------------------------------------------------------------------------------------------------------------------------------------

```

## Test

Use the following command to send a message to the sqs queue.

```shell
aws sqs send-message --queue-url <replace with your sqs queue url> --message-body "Hello from CLI"
```

Run the following command to see the Lambda function's CloudWatch logs.

```shell
sam logs --tail --stack-name <replace with your stack name>
```

## Local Test

First, start the express.js server locally.

```shell
cd app/src
npm install
node index.js
```

Use `curl` to send a POST request to the express.js server.

```shell
curl -X POST -H "Content-Type: application/json" -d @events/sqs.json http://localhost:8080/events
```

You can also use your favorate IDE debugger to debug your application step by step.

