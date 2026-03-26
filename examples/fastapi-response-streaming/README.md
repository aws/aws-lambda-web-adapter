# Serverless Bedtime Storyteller

This example shows streaming response from Amazon Bedrock with FastAPI on AWS Lambda.


![Architecture](imgs/serverless-storyteller-architecture.png)


## How does it work?

This example uses Anthropic Claude v2 model to generate bedtime stories. FastAPI provides the static web frontend and an inference API.  The inference API endpoint invokes Bedrock using Boto3, and streams the response. Both Lambda Web Adapter and function URL have response streaming mode enabled. So the response from Bedrock are streamed all the way back to the client. 

This function is packaged as a Docker image. Here is the content of the Dockerfile. 

```dockerfile
FROM public.ecr.aws/docker/library/python:3.12.0-slim-bullseye
COPY --from=public.ecr.aws/awsguru/aws-lambda-adapter:1.0.0 /lambda-adapter /opt/extensions/lambda-adapter

WORKDIR /app
ADD . .
RUN pip install -r requirements.txt

CMD ["python", "main.py"]
```

Notice that we only need to add the second line to install Lambda Web Adapter. 

```dockerfile
COPY --from=public.ecr.aws/awsguru/aws-lambda-adapter:1.0.0 /lambda-adapter /opt/extensions/
```

In the SAM template, we use an environment variable `AWS_LWA_INVOKE_MODE: RESPONSE_STREAM` to configure Lambda Web Adapter in response streaming mode. And adding a function url with `InvokeMode: RESPONSE_STREAM`. 

```yaml
  FastAPIFunction:
    Type: AWS::Serverless::Function
    Properties:
      PackageType: Image
      MemorySize: 512
      Environment:
        Variables:
          AWS_LWA_INVOKE_MODE: RESPONSE_STREAM
      FunctionUrlConfig:
        AuthType: NONE
        InvokeMode: RESPONSE_STREAM
      Policies:
      - Statement:
        - Sid: BedrockInvokePolicy
          Effect: Allow
          Action:
          - bedrock:InvokeModelWithResponseStream
          Resource: '*'
```      


## Build and deploy

Run the following commends to build and deploy this example. 

```bash
sam build --use-container
sam deploy --guided
```


## Test the example

After the deployment completes, open the `FastAPIFunctionUrl` shown in the output messages. You should see a simple web page. Here is a demo. 


![Demo](imgs/demo.gif)