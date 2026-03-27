# ASP.NET ZIP

This example shows how to deploy an ASP.NET application using the Lambda Web Adapter packaged as a ZIP file.

The `dotnet8` runtime is used.

```yaml
Resources:
  AspNetOnLambdaWebAdapterFunction:
    Type: AWS::Serverless::Function
    Properties:
      CodeUri: src/
      Handler: AspNetWebApi
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

A shell script is used as the handler to startup the ASP.NET web application.

```run.sh
#!/bin/bash

./AspNetLambdaZipWebAdapter
```

## Build & Deploy

Make sure .NET 8 is already installed. Run the following commands on a x86_64 machine. 

```shell
sam build --use-container
sam deploy -g
```

Once deployed, SAM will return two API URL's. One using the default `WeatherForecast` controller. The other, demonstrates how you can retrieve details about the inbound request context and the Lambda execution context. 

The request and Lambda context data is retrieved using 2 header values:

```
app.MapGet("/context",
        ([FromHeader(Name = "x-amzn-request-context")] string requestContext, [FromHeader(Name = "x-amzn-lambda-context")] string lambdaContext) =>
        {
            var jsonOptions = new JsonSerializerOptions()
            {
                PropertyNameCaseInsensitive = true,
            };

            return new
            {
                // LambdaContext is a custom class
                lambdaContext = JsonSerializer.Deserialize<LambdaContext>(
                    lambdaContext,
                    jsonOptions),
                // APIGatewayHttpApiV2ProxyRequest.ProxyRequestContext comes from the Amazon.Lambda.APIGatewayEvents Nuget package
                requestContext = JsonSerializer.Deserialize<APIGatewayHttpApiV2ProxyRequest.ProxyRequestContext>(
                    requestContext,
                    jsonOptions)
            };
        })
    .WithName("Context");
```