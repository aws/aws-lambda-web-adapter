# Zip Packages

AWS Lambda Web Adapter works with AWS managed Lambda runtimes via Lambda Layers.

## Setup Steps

### 1. Attach the Lambda Web Adapter Layer

#### AWS Commercial Regions

| Architecture | Layer ARN |
|-------------|-----------|
| x86_64 | `arn:aws:lambda:${AWS::Region}:753240598075:layer:LambdaAdapterLayerX86:27` |
| arm64 | `arn:aws:lambda:${AWS::Region}:753240598075:layer:LambdaAdapterLayerArm64:27` |

#### AWS China Regions

| Region | Architecture | Layer ARN |
|--------|-------------|-----------|
| cn-north-1 (Beijing) | x86_64 | `arn:aws-cn:lambda:cn-north-1:041581134020:layer:LambdaAdapterLayerX86:27` |
| cn-northwest-1 (Ningxia) | x86_64 | `arn:aws-cn:lambda:cn-northwest-1:069767869989:layer:LambdaAdapterLayerX86:27` |

### 2. Set the Exec Wrapper

Configure the Lambda environment variable:

```
AWS_LAMBDA_EXEC_WRAPPER=/opt/bootstrap
```

### 3. Set the Function Handler

Set your function handler to your web application's startup script, e.g. `run.sh`.

## SAM Template Example

```yaml
Resources:
  MyFunction:
    Type: AWS::Serverless::Function
    Properties:
      Runtime: nodejs20.x
      Handler: run.sh
      Layers:
        - !Sub arn:aws:lambda:${AWS::Region}:753240598075:layer:LambdaAdapterLayerX86:27
      Environment:
        Variables:
          AWS_LAMBDA_EXEC_WRAPPER: /opt/bootstrap
          AWS_LWA_PORT: 7000
```

## Windows Users

If you create your Zip package on Windows, your startup script (e.g. `run.sh`) must meet two requirements to work in the Lambda Linux runtime:

1. **Use LF line endings** — Windows defaults to CRLF (`\r\n`), which causes `/bin/sh` to fail with `cannot execute: required file not found`.
2. **Set Unix file permissions to 755** — Zip files created on Windows don't preserve Unix execute permissions.

Most zip utilities on Windows don't handle Unix permissions. You can work around this by using WSL, a build script that sets permissions explicitly, or a tool like `7-Zip` with the `-mcu` flag. See [#611](https://github.com/awslabs/aws-lambda-web-adapter/issues/611) for more details.

For a complete working example, see the [Express.js in Zip](https://github.com/awslabs/aws-lambda-web-adapter/tree/main/examples/expressjs-zip) example.
