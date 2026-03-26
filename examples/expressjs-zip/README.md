# expressjs-zip

This example shows how to use Lambda Adapter to run an express.js application on managed node.js runtime. 

### How does it work?

We add Lambda Adapter layer to the function and configure wrapper script. 

1. attach Lambda Adapter layer to your function. This layer contains the Lambda Adapter binary and a wrapper script. 
    1. x86_64: `arn:aws:lambda:${AWS::Region}:753240598075:layer:LambdaAdapterLayerX86:27`
    2. arm64: `arn:aws:lambda:${AWS::Region}:753240598075:layer:LambdaAdapterLayerArm64:27`
2. configure Lambda environment variable `AWS_LAMBDA_EXEC_WRAPPER` to `/opt/bootstrap`. This is a wrapper script included in the layer.
3. set function handler to a startup command: `run.sh`. The wrapper script will execute this command to boot up your application. 

To get more information of Wrapper script, please read Lambda documentation [here](https://docs.aws.amazon.com/lambda/latest/dg/runtimes-modify.html#runtime-wrapper). 

### Build and Deploy

Run the following commands to build and deploy the application to lambda. 

```bash
sam build
sam deploy --guided
```
When the deployment completes, take note of HelloWorldApi's Value. It is the API Gateway endpoint URL. 

### Verify it works

Open HelloWorldApi's URL in a browser, you should see "Hi there!" on the page. 


