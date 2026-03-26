# deno-zip

This example shows how to deploy a Deno app on Lambda with SnapStart enabled.

The Deno app is compiled to a single binary using `deno compile`, packaged into Zip file and deployed to Lambda with Web Adapter.

We use `java11` runtime to get SnapStart support with one caveat: no runtime hooks.

```yaml
  DenoFunction:
    Type: AWS::Serverless::Function 
    Properties:
      CodeUri: src
      Handler: app
      Runtime: java11
      AutoPublishAlias: live
      SnapStart:
        ApplyOn: PublishedVersions
      Architectures:
        - x86_64
      Layers:
        - !Sub arn:aws:lambda:${AWS::Region}:753240598075:layer:LambdaAdapterLayerX86:27
      MemorySize: 512
      Environment:
        Variables:
          AWS_LAMBDA_EXEC_WRAPPER: /opt/bootstrap
          DENO_DIR: /tmp
          PORT: 8000
      Events:
        HelloWorld:
          Type: HttpApi
    Metadata:
      BuildMethod: makefile
```

## build and deploy

Make sure Deno is already installed. Run the following commands on a x86_64 machine. 

```shell
sam build 
sam deploy -g
```
