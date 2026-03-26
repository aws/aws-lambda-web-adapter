# FastAPI example

A basic FastAPI application example. You can build and test it locally as a typical FastAPI application.

Using AWS Lambda Web Adapter, You can package this web application into Docker image, push to ECR, and deploy to Lambda, ECS/EKS, or EC2.

The application can be deployed in an AWS account using the [Serverless Application Model](https://github.com/awslabs/serverless-application-model). The `template.yaml` file in the root folder contains the application definition.

The top level folder is a typical AWS SAM project. The `app` directory is a FastAPI application with a [Dockerfile](app/Dockerfile).

```dockerfile
FROM public.ecr.aws/docker/library/python:3.8.12-slim-buster
COPY --from=public.ecr.aws/awsguru/aws-lambda-adapter:1.0.0 /lambda-adapter /opt/extensions/lambda-adapter
ENV PORT=8000
WORKDIR /var/task
COPY requirements.txt ./
RUN python -m pip install -r requirements.txt
COPY *.py ./
CMD exec uvicorn --port=$PORT main:app
```

Line 2 copies lambda web adapter binary into /opt/extensions. This is the change to run the FastAPI application on Lambda.

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
Once the deployment is completed, the SAM CLI will print out the stack's outputs, including the new application URL. You can use `curl` or a web browser to make a call to the URL

```shell
...
---------------------------------------------------------------------------------------------------------
OutputKey-Description                        OutputValue
---------------------------------------------------------------------------------------------------------
FastAPIURL - URL for application            https://xxxxxxxxxx.execute-api.us-west-2.amazonaws.com/
---------------------------------------------------------------------------------------------------------
...

$ curl https://xxxxxxxxxx.execute-api.us-west-2.amazonaws.com/
```

## Run the docker locally

We can run the same docker image locally, so that we know it can be deployed to ECS Fargate and EKS EC2 without code changes.

```shell
$ docker run -d -p 8000:8000 {ECR Image}

```

Use curl to verify the docker container works.

```shell
$ curl localhost:8000/ 
```
