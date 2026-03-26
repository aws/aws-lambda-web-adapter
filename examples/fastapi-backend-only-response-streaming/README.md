# Serverless Streaming with Lambda Web Adapter and Bedrock

This example demonstrates how to set up a serverless streaming service using AWS Lambda, Lambda Web Adapter, and Amazon Bedrock. The service can be easily consumed by any frontend application through simple GET requests, without the need for websockets.

## Overview

This project showcases:
- Streaming responses from Amazon Bedrock (using Anthropic Claude v2 model)
- Using FastAPI with AWS Lambda
- Implementing Lambda Web Adapter for response streaming
- Creating a Function URL that supports response streaming

The setup allows any frontend to consume the streaming service via GET requests to the Function URL.

## How It Works

1. A FastAPI application is set up to handle requests and interact with Bedrock.
2. The application is packaged as a Docker image, including the Lambda Web Adapter.
3. AWS SAM is used to deploy the Lambda function with the necessary configurations.
4. A Function URL is created with response streaming enabled.
5. Frontends can send GET requests to this URL to receive streamed responses.

## Key Components

### Dockerfile

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

Run the following commands to build and deploy this example. 

```bash
sam build --use-container
sam deploy --guided
```


## Test the example

After the deployment completes, use the `FastAPIFunctionUrl` shown in the output messages to send get requests with your query to the /api/stream route.


```python
import requests
from botocore.auth import SigV4Auth
from botocore.awsrequest import AWSRequest
import boto3
import json
import time

session = boto3.Session()
credentials = session.get_credentials()
region = 'us-east-1'

payload = {"query": query}

request = AWSRequest(
    method='GET',
    url=f'{func_url}/api/stream',
    data=json.dumps(payload),
    headers={'Content-Type': 'application/json'}
)

SigV4Auth(credentials, "lambda", region).add_auth(request)
buffer = ""
response= requests.get(
    request.url,
    data=request.data,
    headers=dict(request.headers),
    stream=True
)

for chunk in response.iter_content(chunk_size=64):
    print(chunk.decode('utf-8'), end='', flush=True)
```