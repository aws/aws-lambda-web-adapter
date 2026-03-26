import { App, CfnOutput, Duration, Stack, StackProps } from "aws-cdk-lib";
import * as lambda from "aws-cdk-lib/aws-lambda";

import { FunctionUrlAuthType, Code } from "aws-cdk-lib/aws-lambda";
import * as path from "path";

const app = new App();

class LwaStack extends Stack {
  constructor(scope: App, id: string, props?: StackProps) {
    super(scope, id, props);

    const lwa_lambda = new lambda.Function(this, id, {
      code: Code.fromAsset(path.join(__dirname, "../../lambda-asset/src")),
      runtime: lambda.Runtime.NODEJS_20_X,
      handler: "run.sh",
      functionName: id + "-lambda",
      timeout: Duration.seconds(9),
    });

    const functionUrl = lwa_lambda.addFunctionUrl({
      authType: FunctionUrlAuthType.NONE,
    });

    new CfnOutput(this, "LambdaFunctionUrl", {
      value: functionUrl.url,
      description: "The Lambda Function URL",
    });

    lwa_lambda.addEnvironment(
      "AWS_LWA_LAMBDA_RUNTIME_API_PROXY",
      "127.0.0.1:9002",
    );

    lwa_lambda.addEnvironment("DD_TRACE_PARTIAL_FLUSH_MIN_SPANS", "1");
    lwa_lambda.addEnvironment("DD_TRACE_PARTIAL_FLUSH_ENABLED", "false");
    lwa_lambda.addEnvironment("DD_API_KEY", process.env.DD_API_KEY || "");
    lwa_lambda.addEnvironment("DD_SERVICE", id);
    lwa_lambda.addEnvironment("AWS_LAMBDA_EXEC_WRAPPER", "/opt/bootstrap");

    const lwa_lambda_layer = lambda.LayerVersion.fromLayerVersionArn(
      this,
      "lwa_lambda-layer",
      "arn:aws:lambda:us-east-1:753240598075:layer:LambdaAdapterLayerX86:27",
    );
    const dd_layer = lambda.LayerVersion.fromLayerVersionArn(
      this,
      "lwa_lambda-dd-layer",
      "arn:aws:lambda:us-east-1:464622532012:layer:Datadog-Extension:77",
    );
    lwa_lambda.addLayers(lwa_lambda_layer, dd_layer);
  }
}

new LwaStack(app, "lwa-stack", {});
