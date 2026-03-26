# ASP.NET ZIP

This example shows how to deploy an ASP.NET application using the Lambda Web Adapter packaged as a ZIP file.

The `dotnet8` runtime is used.

```yaml
Resources:
  AspNetOnLambdaWebAdapterFunction:
    Type: AWS::Serverless::Function
    Properties:
      CodeUri: src/
      Handler: AspNetLambdaZipWebAdapter
      MemorySize: 1024
      Environment:
        Variables:
          AWS_LAMBDA_EXEC_WRAPPER: /opt/bootstrap
          RUST_LOG: info
      Layers:
        - !Sub arn:aws:lambda:${AWS::Region}:753240598075:layer:LambdaAdapterLayerX86:27
      Events:
        Api:
          Type: HttpApi
          Properties:
            Path: /{proxy+}
            Method: ANY
```

## Build & Deploy

Make sure .NET 8 is already installed. Run the following commands on a x86_64 machine. 

```shell
sam build --use-container
sam deploy -g
```
