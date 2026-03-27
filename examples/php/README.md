# PHP example

A basic PHP application runs inside AWS Lambda.

You can package this PHP application into Docker image, push to ECR, and deploy to Lambda, ECS/EKS, or EC2.

The application can be deployed in an AWS account using
the [Serverless Application Model](https://github.com/awslabs/serverless-application-model). The `template.yaml` file in
the root folder contains the application definition.

The top level folder is a typical AWS SAM project. The `app` directory is the nginx configuration with
a [Dockerfile](Dockerfile).

```dockerfile
FROM public.ecr.aws/awsguru/php:82.2023.3.11.1 AS builder
COPY --from=composer /usr/bin/composer /usr/local/bin/composer

COPY app /var/task/app
WORKDIR /var/task/app

RUN composer install --prefer-dist --optimize-autoloader --no-dev --no-interaction

FROM public.ecr.aws/awsguru/php:82.2023.3.11.1
COPY --from=public.ecr.aws/awsguru/aws-lambda-adapter:1.0.0 /lambda-adapter /opt/extensions/lambda-adapter
COPY --from=builder /var/task /var/task

# config files
ADD nginx/conf/nginx.conf      /opt/nginx/conf/nginx.conf
ADD php/php.ini                /opt/php/php.ini
ADD php/etc/php-fpm.conf       /opt/php/etc/php-fpm.conf
ADD php/php.d/extensions.ini   /opt/php/php.d/extensions.ini
```

## Pre-requisites

The following tools should be installed and configured.

* [AWS CLI](https://aws.amazon.com/cli/)
* [SAM CLI](https://github.com/awslabs/aws-sam-cli)
* [Docker](https://www.docker.com/products/docker-desktop)

## Deploy to Lambda

Navigate to the sample's folder and use the SAM CLI to build a container image

```shell
$ sam build
```

This command compiles the application and prepares a deployment package in the `.aws-sam` subdirectory.

To deploy the application in your AWS account, you can use the SAM CLI's guided deployment process and follow the
instructions on the screen

```shell
$ sam deploy --guided
```

Please take note of the container image name.
Once the deployment is completed, the SAM CLI will print out the stack's outputs, including the new application URL. You
can use `curl` or a web browser to make a call to the URL

```shell
...
---------------------------------------------------------------------------------------------------------
OutputKey-Description                        OutputValue
---------------------------------------------------------------------------------------------------------
PhpFunction - URL for application            https://xxxxxxxxxx.execute-api.us-west-2.amazonaws.com/Prod/
---------------------------------------------------------------------------------------------------------
...

$ curl https://xxxxxxxxxx.execute-api.us-west-2.amazonaws.com/Prod/
```

## Run the docker locally

We can run the same docker image locally, so that we know it can be deployed to ECS Fargate and EKS EC2 without code
changes.

```shell
$ docker run -d -p 8080:8080 {ECR Image}
```

Use curl to verify the docker container works.

```shell
$ curl localhost:8080/
```
