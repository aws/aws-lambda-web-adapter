# Remix example

A simple Remix application example. You can build and test it locally as a typical Remix application.

The Remix application is created using the following command: 

```bash
npx create-remix@latest --template remix-run/remix/templates/express
```

Using AWS Lambda Web Adapter, you can package this web application into Docker image, push to ECR, and deploy to Lambda, ECS/EKS, or EC2.

The application can be deployed in an AWS account using the [Serverless Application Model](https://github.com/awslabs/serverless-application-model). The `template.yaml` file in the root folder contains the application definition.

The top level folder is a typical AWS SAM project. The `remix-app` directory is a Remix application with a [Dockerfile](app/Dockerfile). 

```dockerfile
FROM public.ecr.aws/docker/library/node:20-bookworm-slim as builder
WORKDIR "/var/task"
ADD . .
RUN cd remix-app && npm install && npm run build && npm prune --omit=dev

FROM public.ecr.aws/docker/library/node:20-bookworm-slim
COPY --from=public.ecr.aws/awsguru/aws-lambda-adapter:1.0.0 /lambda-adapter /opt/extensions/lambda-adapter
WORKDIR "/var/task"
COPY --from=builder  /var/task/remix-app/build /var/task/build
COPY --from=builder  /var/task/remix-app/node_modules /var/task/node_modules
COPY --from=builder  /var/task/remix-app/server.js /var/task/server.js
COPY --from=builder  /var/task/remix-app/package.json /var/task/package.json
ENV NODE_ENV=production PORT=3000
CMD ["node", "server.js"]
```

## Pre-requisites

The following tools should be installed and configured. 
* [AWS CLI](https://aws.amazon.com/cli/)
* [SAM CLI](https://github.com/awslabs/aws-sam-cli)
* [Node](https://nodejs.org/en/)
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
Once the deployment is completed, the SAM CLI will print out the stack's outputs, including the new application URL. You can open web browser to visit the URL.

```shell
...
---------------------------------------------------------------------------------------------------------
OutputKey-Description                        OutputValue
---------------------------------------------------------------------------------------------------------
RemixFunctionApi - URL for application            https://xxxxxxxxxx.execute-api.us-west-2.amazonaws.com/
---------------------------------------------------------------------------------------------------------
...

$ curl https://xxxxxxxxxx.execute-api.us-west-2.amazonaws.com/
```

## Run the docker locally

We can run the same docker image locally, so that we know it can be deployed to ECS Fargate and EKS EC2 without code changes.

```shell
$ docker run -d -p 3000:3000 {ECR Image}

```

Open web browser to visit `http://localhost:3000/`

## Local test with SAM CLI

In general, you can test your web app locally without simulating AWS Lambda execution environment. But if you want to simulate Lambda and API Gateway locally, you can use SAM CLI.

```shell
sam local start-api --warm-containers EAGER
```

This command will start a local http endpoint and docker container to simulate API Gateway and Lambda. You can test it using `curl`, `postman`, and your web browser.