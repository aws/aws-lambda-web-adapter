# Sinatra example
A basic example of a Sinatra application.

Using AWS Lambda Adapter, You can package this web application into Docker image, push to ECR, and deploy to Lambda, ECS/EKS, or EC2.

The application can be deployed in an AWS account using the [Serverless Application Model](https://github.com/awslabs/serverless-application-model). The `template.yaml` file in the root folder contains the application definition.

The top level folder is a typical AWS SAM project. The `app` directory is a Sinatra application with a Dockerfile.

```dockerfile
FROM public.ecr.aws/docker/library/ruby:3.3
COPY --from=public.ecr.aws/awsguru/aws-lambda-adapter:1.0.0 /lambda-adapter /opt/extensions/lambda-adapter
WORKDIR /var/task
COPY Gemfile Gemfile.lock ./
RUN bundle install
COPY . .
CMD ["bundle", "exec", "ruby", "app.rb", "-o", "0.0.0.0", "-p", "8080"]
```

## Pre-requisites

The following tools should be installed and configured.
* [AWS CLI](https://aws.amazon.com/cli/)
* [SAM CLI](https://github.com/awslabs/aws-sam-cli)
* [Docker](https://www.docker.com/products/docker-desktop)
* [Ruby](https://www.ruby-lang.org/)

## Deploy to Lambda

Navigate to the sample's folder and use the SAM CLI to build a container image:

```shell
$ sam build
```

This command compiles the application and prepares a deployment package in the `.aws-sam` sub-directory.

To deploy the application in your AWS account, you can use the SAM CLI's guided deployment process and follow the instructions on the screen:

```shell
$ sam deploy --guided
```

Please take note of the container image name.
Once the deployment is completed, the SAM CLI will print out the stack's outputs, including the new application URL. You can use `curl` or a web browser to make a call to the URL.

NOTE: This SAM does not use Amazon API Gateway, but uses AWS Lambda function URLs to create HTTP endpoints.

```shell
...
CloudFormation outputs from deployed stack
-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
Outputs
-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
Key                 SinatraFunction1
Description         Function URL endpoint on AWS Lambda
Value               https://xxxxxxxxxxxxxxxxxxxxxxxxx.lambda-url.REGION.on.aws/
-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------...

$ curl https://xxxxxxxxxxxxxxxxxxxxxxxxx.lambda-url.REGION.on.aws/
```

## Run the docker locally

We can run the same docker image locally, so that we know it can be deployed to ECS Fargate and EKS EC2 without code changes.

```shell
$ docker run -it --rm -p 8080:8080 {ECR Image}
```

Use curl from another session to checking that the Docker container is working.

```shell
$ curl localhost:8080/
#=> Hello! I am <b>Sinatra</b>.
```
