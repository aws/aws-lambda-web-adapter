#!/usr/bin/env bash
#
# Publishes the version-named SSM pointer after CloudFormation has deployed a layer.
# The parameter deliberately lives outside the stack: retaining a
# parameter whose Name contains the package version made CloudFormation collide with
# the retained resource whenever a version was re-used.
#
# Usage:
#   publish-layer-parameter.sh <stack-name> <x86_64|arm64> <version> <region>
set -euo pipefail
export AWS_PAGER=""

STACK_NAME="${1:?stack name required}"
ARCHITECTURE="${2:?architecture required}"
VERSION="${3:?version required}"
REGION="${4:?region required}"

# A version containing a slash would write outside the path the grant covers — and, worse,
# could name a `latest` pointer. Fail before the API call rather than on an opaque
# AccessDenied.
if [[ "$VERSION" != "${VERSION//\//}" || -z "$VERSION" ]]; then
  echo "Refusing to publish a parameter for version '$VERSION'" >&2
  exit 2
fi

case "$ARCHITECTURE" in
  x86_64)
    OUTPUT_KEY="LambdaAdapterLayerX86Arn"
    DESCRIPTION_ARCH="X86_64"
    ;;
  arm64)
    OUTPUT_KEY="LambdaAdapterLayerArm64Arn"
    DESCRIPTION_ARCH="Arm64"
    ;;
  *)
    echo "Unsupported architecture: $ARCHITECTURE" >&2
    exit 2
    ;;
esac

LAYER_ARN="$(
  aws cloudformation describe-stacks \
    --stack-name "$STACK_NAME" \
    --region "$REGION" \
    --query "Stacks[0].Outputs[?OutputKey=='${OUTPUT_KEY}'].OutputValue | [0]" \
    --output text
)"

if [[ -z "$LAYER_ARN" || "$LAYER_ARN" == "None" ]]; then
  echo "Stack $STACK_NAME has no $OUTPUT_KEY output in $REGION" >&2
  exit 1
fi

PARAMETER_NAME="/lambda-web-adapter/layer/${ARCHITECTURE}/${VERSION}"
PARAMETER_VERSION="$(
  aws ssm put-parameter \
    --name "$PARAMETER_NAME" \
    --description "Layer ARN for the Lambda Web Adapter ${DESCRIPTION_ARCH} Layer: ${VERSION}" \
    --type String \
    --value "$LAYER_ARN" \
    --overwrite \
    --region "$REGION" \
    --query Version \
    --output text
)"

echo "Published $PARAMETER_NAME -> $LAYER_ARN (parameter version $PARAMETER_VERSION)"
