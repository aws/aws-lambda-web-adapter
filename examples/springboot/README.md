# Spring Boot 2 example

A basic pet store application written with the Spring Boot 2 framework. You can build and test it locally as a typical Spring Boot 2 application.

Using AWS Lambda Adapter, you can package this web application into Docker image, push to ECR, and deploy to Lambda, ECS/EKS, or EC2.

The application can be deployed in an AWS account using the [Serverless Application Model](https://github.com/awslabs/serverless-application-model). The `template.yaml` file in the root folder contains the application definition.

The top level folder is a typical AWS SAM project. The `app` directory is a Spring Boot application with a multi-stage [Dockerfile](app/Dockerfile).

```dockerfile
FROM public.ecr.aws/sam/build-java8.al2:latest as build-image
WORKDIR "/task"
COPY src src/
COPY pom.xml ./
RUN mvn -q clean package

FROM public.ecr.aws/docker/library/amazoncorretto:8u322-al2
COPY --from=public.ecr.aws/awsguru/aws-lambda-adapter:1.0.0 /lambda-adapter /opt/extensions/lambda-adapter
ENV PORT=8000
WORKDIR /opt
COPY --from=build-image /task/target/petstore-0.0.1-SNAPSHOT.jar /opt
CMD ["java", "-jar", "petstore-0.0.1-SNAPSHOT.jar", "--server.port=${PORT}"]
```

Line 7 copies lambda adapter binary to /opt/extensions. This is the only change to run the Spring Boot application on Lambda.

```dockerfile
COPY --from=public.ecr.aws/awsguru/aws-lambda-adapter:1.0.0 /lambda-adapter /opt/extensions/lambda-adapter
```

## Remove the base path

The pet store application is deployed under /v1/{proxy+}. But the application does not know that. So in the SAM template file, we configured environment variable `REMOVE_BASE_PATH=/v1`.
This configuration tells the Adapter to remove `/v1` from http request path, so that the pet store application works without changing code.

## Pre-requisites

The following tools should be installed and configured.

* [AWS CLI](https://aws.amazon.com/cli/)
* [SAM CLI](https://github.com/awslabs/aws-sam-cli)
* [Maven](https://maven.apache.org/)
* [Docker](https://www.docker.com/products/docker-desktop)

## Deploy to Lambda

Navigate to the sample's folder and use the SAM CLI to build a container image

```shell
aws ecr-public get-login-password --region us-east-1 | docker login --username AWS --password-stdin public.ecr.aws
sam build
```

This command compiles the application and prepares a deployment package in the `.aws-sam` sub-directory.

To deploy the application in your AWS account, you can use the SAM CLI's guided deployment process and follow the instructions on the screen

```shell
sam deploy --guided
```

Please take note of the container image name.
Once the deployment is completed, the SAM CLI will print out the stack's outputs, including the new application URL. You can use `curl` or a web browser to make a call to the URL

```shell
...
---------------------------------------------------------------------------------------------------------
OutputKey-Description                        OutputValue
---------------------------------------------------------------------------------------------------------
PetStoreApi - URL for application            https://xxxxxxxxxx.execute-api.us-west-2.amazonaws.com/v1/pets
---------------------------------------------------------------------------------------------------------
...

curl https://xxxxxxxxxx.execute-api.us-west-2.amazonaws.com/v1/pets
```

## Run the docker locally

You can run the same docker image locally, so that we know it can be deployed to ECS Fargate and EKS EC2 without code changes.

```shell
docker run -d -p 8000:8000 {ECR Image}

```

Use curl to verify the docker container works.

```shell
curl localhost:8000/pets
```

## Local test with SAM CLI

In general, you can test your web app locally without simulating AWS Lambda execution environment. But if you want to simulate Lambda and API Gateway locally, you can use SAM CLI. 

```shell
sam local start-api --warm-containers EAGER
```

This command will start a local http endpoint and docker container to simulate API Gateway and Lambda. You can test it using `curl`, `postman`, and web browser. 

## Clean up

This example use provisioned concurrency to reduce cold start time. It incurs additional cost. You can remove the whole example with the following command.

```shell
sam delete
```
