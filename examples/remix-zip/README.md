# remix-zip

This example shows how to use Lambda Adapter to run an Remix application on AWS Lambda. 

The Remix application is created using the following command: 

```bash
npx create-remix@latest --template remix-run/remix/templates/express
```

### How does it work?

We add Lambda Adapter layer to the function and configure wrapper script. 

1. attach Lambda Adapter layer to your function. This layer contains the Lambda Adapter binary and a wrapper script. 
    1. x86_64: `arn:aws:lambda:${AWS::Region}:753240598075:layer:LambdaAdapterLayerX86:27`
    2. arm64: `arn:aws:lambda:${AWS::Region}:753240598075:layer:LambdaAdapterLayerArm64:27`
2. configure Lambda environment variable `AWS_LAMBDA_EXEC_WRAPPER` to `/opt/bootstrap`. This is a wrapper script included in the layer.
3. set function handler to a startup command: `run.sh`. The wrapper script will execute this command to boot up your application. 

To get more information of Wrapper script, please read Lambda documentation [here](https://docs.aws.amazon.com/lambda/latest/dg/runtimes-modify.html#runtime-wrapper). 

### Build and Deploy

Run the following commands to build and deploy the application to lambda. SAM CLI will use GNU Make to build the Remix application.

```bash
sam build
sam deploy --guided
```
When the deployment completes, take note of RemixFunctionApi's Value. It is the API Gateway endpoint URL. 

### Verify it works

Open RemixFunctionApi's URL in a browser, you should see the "Weclome to Remix" page. 

## Local test with SAM CLI

In general, you can test your web app locally without simulating AWS Lambda execution environment. But if you want to simulate Lambda and API Gateway locally, you can use SAM CLI.

```shell
sam local start-api --warm-containers EAGER --region us-west-2
```

This command will start a local http endpoint and docker container to simulate API Gateway and Lambda. Please modify the region to match the actual region you are using. You can test it using `curl`, `postman`, and your web browser.