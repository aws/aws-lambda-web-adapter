# ASP.NET example

A basic ASP.NET application example. You can build and test it locally as a typical ASP.NET application.

Using AWS Lambda Adapter, you can package this web application into Docker image, push to ECR, and deploy to Lambda, ECS/EKS, or EC2.

The application can be deployed in an AWS account using the [Serverless Application Model](https://github.com/awslabs/serverless-application-model). The `template.yaml` file in the root folder contains the application definition.

The top level folder is a typical AWS SAM project. The `src` directory is an ASP.NET application with a [Dockerfile](app/Dockerfile). 

```dockerfile
FROM mcr.microsoft.com/dotnet/aspnet:8.0-preview as base
WORKDIR /app

FROM mcr.microsoft.com/dotnet/sdk:8.0-preview AS build
COPY . ./src
WORKDIR /src
RUN ls
RUN dotnet build "AspNetLambdaWebAdapter.csproj" -c Release -o /app/build

FROM build AS publish
RUN dotnet publish "AspNetLambdaWebAdapter.csproj" -c Release -o /app/publish

FROM base AS final
ENV ASPNETCORE_URLS=http://+:8080
COPY --from=public.ecr.aws/awsguru/aws-lambda-adapter:1.0.0 /lambda-adapter /opt/extensions/lambda-adapter
WORKDIR /app
COPY --from=publish /app/publish .
ENTRYPOINT ["dotnet", "AspNetLambdaWebAdapter.dll"]
```

Line 12 copies lambda adapter binary into /opt/extensions. This is required to run ASP.NET application on Lambda. The `ASPNETCORE_URLS` environment variable is also set to 8080. This is required for the Lambda Web Adapter to work.

```dockerfile
COPY --from=public.ecr.aws/awsguru/aws-lambda-adapter:1.0.0 /lambda-adapter /opt/extensions/lambda-adapter
```

## Pre-requisites

The following tools should be installed and configured. 
* [AWS CLI](https://aws.amazon.com/cli/)
* [SAM CLI](https://github.com/awslabs/aws-sam-cli)
* [.NET 6](https://nodejs.org/en/)
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
ApiUrl - URL for application            https://xxxxxxxxxx.execute-api.us-west-2.amazonaws.com/
---------------------------------------------------------------------------------------------------------
...

$ curl https://xxxxxxxxxx.execute-api.us-west-2.amazonaws.com/
```

## Run the docker locally

We can run the same docker image locally, so that we know it can be deployed to ECS Fargate and EKS EC2 without code changes.

```shell
$ docker run -d -p 8080:8080 {ECR Image}

```

Use curl to verify the docker container works.

```shell
$ curl localhost:8080/ 
```
