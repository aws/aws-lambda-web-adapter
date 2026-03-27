# FastAPI Response Streaming with Lambda Managed Instances

This example shows how to use Lambda Web Adapter to run a FastAPI application with response streaming on [Lambda Managed Instances](https://docs.aws.amazon.com/lambda/latest/dg/lambda-managed-instances.html) (LMI).

Lambda Managed Instances allows a single Lambda execution environment to handle multiple concurrent requests, improving throughput and reducing costs for I/O-bound workloads like streaming responses.

## Prerequisites

Lambda Managed Instances requires a VPC with:
- At least one subnet (two subnets recommended)
- A security group that allows outbound traffic

If you don't have a VPC configured, you can use the default VPC or create one.

## How does it work?

This example combines three Lambda features:

1. **Lambda Web Adapter** - Runs your FastAPI app on Lambda without code changes
2. **Response Streaming** - Streams responses back to clients as they're generated
3. **Lambda Managed Instances** - Handles multiple concurrent requests per execution environment

### Key Configuration

```yaml
LMICapacityProvider:
  Type: AWS::Serverless::CapacityProvider
  Properties:
    CapacityProviderName: !Sub "${AWS::StackName}-capacity-provider"
    VpcConfig:
      SubnetIds: !Ref SubnetIds
      SecurityGroupIds: !Ref SecurityGroupIds
    ScalingConfig:
      MaxVCpuCount: 20
      AverageCPUUtilization: 70.0

FastAPIFunction:
  Type: AWS::Serverless::Function
  Properties:
    CodeUri: app/
    Handler: run.sh
    Runtime: python3.13
    MemorySize: 2048
    Environment:
      Variables:
        AWS_LAMBDA_EXEC_WRAPPER: /opt/bootstrap
        AWS_LWA_INVOKE_MODE: response_stream
        PORT: 8000
    Layers:
      - !Sub arn:aws:lambda:${AWS::Region}:753240598075:layer:LambdaAdapterLayerX86:27
    CapacityProviderConfig:
      Arn: !GetAtt LMICapacityProvider.Arn
      PerExecutionEnvironmentMaxConcurrency: 64
    FunctionUrlConfig:
      AuthType: NONE
      InvokeMode: RESPONSE_STREAM
```

- `AWS::Serverless::CapacityProvider` - Creates the LMI capacity provider with VPC configuration
- `CapacityProviderConfig.Arn` - References the capacity provider
- `CapacityProviderConfig.PerExecutionEnvironmentMaxConcurrency: 64` - Up to 64 concurrent requests per instance
- `AWS_LWA_INVOKE_MODE: response_stream` - Configures Lambda Web Adapter for streaming
- `FunctionUrlConfig.InvokeMode: RESPONSE_STREAM` - Enables streaming on the Function URL

## Build and Deploy

First, get your VPC subnet and security group IDs:

```bash
# List subnets in your default VPC
aws ec2 describe-subnets --filters "Name=default-for-az,Values=true" \
  --query 'Subnets[*].[SubnetId,AvailabilityZone]' --output table

# List security groups
aws ec2 describe-security-groups --filters "Name=group-name,Values=default" \
  --query 'SecurityGroups[*].[GroupId,GroupName]' --output table
```

Build and deploy:

```bash
sam build --use-container
sam deploy --guided
```

During guided deployment, you'll be prompted for:
- `SubnetIds` - Comma-separated list of subnet IDs (e.g., `subnet-abc123,subnet-def456`)
- `SecurityGroupIds` - Comma-separated list of security group IDs (e.g., `sg-abc123`)

## Verify it works

Open the Function URL in a browser. You should see a message stream back character by character, with a unique request ID prefix like `[a1b2c3d4] This is streaming from Lambda Managed Instances!`.

### Test concurrent requests

To verify LMI is working, send multiple concurrent requests:

```bash
# Get your function URL
URL=$(aws cloudformation describe-stacks --stack-name fastapi-response-streaming-lmi \
  --query 'Stacks[0].Outputs[?OutputKey==`FastAPIFunctionUrl`].OutputValue' --output text)

# Send 10 concurrent requests
for i in {1..10}; do curl -s "$URL" & done; wait
```

Each response will have a different request ID, but they may share the same execution environment (visible in CloudWatch logs).

## Considerations

When using LMI with streaming:

- **VPC**: LMI requires VPC configuration. Ensure your subnets have internet access (via NAT Gateway) if your function needs to call external services
- **Shared state**: FastAPI/Uvicorn handles concurrency natively, but avoid mutable global state
- **Memory**: With 64 concurrent requests, ensure sufficient memory (2048MB in this example)
- **Timeouts**: Streaming responses can run up to 15 minutes with Function URLs
- **Scaling**: `MaxVCpuCount` controls the maximum vCPUs the capacity provider can provision across all instances
