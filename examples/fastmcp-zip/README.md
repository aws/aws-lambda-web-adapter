# FastMCP-zip

This example shows how to use Lambda Web Adapter to run a FastMCP server on managed python runtime. 

### How does it work?

We add Lambda Web Adapter layer to the function and configure wrapper script. 

1. attach Lambda Adapter layer to your function. This layer contains the Lambda Adapter binary and a wrapper script. 
    1. x86_64: `arn:aws:lambda:${AWS::Region}:753240598075:layer:LambdaAdapterLayerX86:27`
    2. arm64: `arn:aws:lambda:${AWS::Region}:753240598075:layer:LambdaAdapterLayerArm64:27`
2. configure Lambda environment variable `AWS_LAMBDA_EXEC_WRAPPER` to `/opt/bootstrap`. This is a wrapper script included in the layer.
3. set function handler to a startup command: `run.sh`. The wrapper script will execute this command to boot up your application. 
4. configure `AWS_LWA_READINESS_CHECK_PATH` to `/healthz` so the adapter waits for the MCP server to be ready before forwarding requests.

To get more information of Wrapper script, please read Lambda documentation [here](https://docs.aws.amazon.com/lambda/latest/dg/runtimes-modify.html#runtime-wrapper). 

### Build and Deploy

Run the following commands to build and deploy the application to lambda. 

```bash
sam build --use-container
sam deploy --guided
```
When the deployment completes, take note of McpApiUrl's Value. It is the API Gateway endpoint URL. 

### Verify it works

Use the FastMCP client to call a tool on the deployed server:

```python
import asyncio
from fastmcp import Client

client = Client("<McpApiUrl>")

async def call_tool(name: str):
    async with client:
        result = await client.call_tool("greet", {"name": name})
        print(result)

asyncio.run(call_tool("World"))
```

You should see `Hello, World!` in the output.
