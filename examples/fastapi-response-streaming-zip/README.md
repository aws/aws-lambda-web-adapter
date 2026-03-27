# FastAPI Response Streaming

This example shows how to use Lambda Web Adapter to run a FastAPI application with response streaming via a Function URL.

## How does it work?

We add Lambda Web Adapter layer to the function and configure wrapper script.

1. attach Lambda Adapter layer to your function. This layer contains the Lambda Adapter binary and a wrapper script.
    1. x86_64: `arn:aws:lambda:${AWS::Region}:753240598075:layer:LambdaAdapterLayerX86:27`
    2. arm64: `arn:aws:lambda:${AWS::Region}:753240598075:layer:LambdaAdapterLayerArm64:27`
2. configure Lambda environment variable `AWS_LAMBDA_EXEC_WRAPPER` to `/opt/bootstrap`. This is a wrapper script included in the layer.
3. set function handler to a startup command: `run.sh`. The wrapper script will execute this command to boot up your application.

To get more information of Wrapper script, please read Lambda documentation [here](https://docs.aws.amazon.com/lambda/latest/dg/runtimes-modify.html#runtime-wrapper).

This is the resource for Lambda function. The function urls's invoke mode is configured as "RESPONSE_STREAM", and Lambda environment variable "AWS_LWA_INVOKE_MODE" is set to "response_stream".

```yaml
  FastAPIFunction:
    Type: AWS::Serverless::Function
    Properties:
      CodeUri: app/
      Handler: run.sh
      Runtime: python3.9
      MemorySize: 256
      Environment:
        Variables:
          AWS_LAMBDA_EXEC_WRAPPER: /opt/bootstrap
          AWS_LWA_INVOKE_MODE: response_stream
          PORT: 8000
      Layers:
        - !Sub arn:aws:lambda:${AWS::Region}:753240598075:layer:LambdaAdapterLayerX86:27
      FunctionUrlConfig:
        AuthType: NONE
        InvokeMode: RESPONSE_STREAM
```

## Build and Deploy

Run the following commands to build and deploy the application to lambda.

```bash
sam build --use-container
sam deploy --guided
```

When the deployment completes, take note of FastAPI's Value. It is the API Gateway endpoint URL.

## Verify it works

Open FastAPI's URL in a browser, you should see "This is streaming from Lambda" streams back character by character.
