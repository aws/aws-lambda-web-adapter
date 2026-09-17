#!/usr/bin/env bash
#
# Grants a PipelineExecutionRole permission to publish the version-named SSM pointers that
# publish-layer-parameter.sh writes. Run once per role, with administrator credentials in
# that role's account; AWS_PROFILE and the standard AWS CLI credential variables are
# honored.
#
# Usage:
#   configure-layer-parameter-iam.sh <pipeline-execution-role-arn>
#
# Deliberately region-agnostic. An earlier draft took a region and embedded it in the
# Resource ARN, which had two problems: `put-role-policy` replaces the whole named policy,
# so running it a second time for another region silently revoked the first one; and the
# partition came from the role ARN while the region came from argv, so a mismatched pair
# (an `aws` role with `cn-north-1`) produced a policy that matched nothing while reporting
# success. Wildcarding the region removes both — one invocation per role covers every
# region, and re-running it is idempotent.
set -euo pipefail
export AWS_PAGER=""

ROLE_ARN="${1:?pipeline execution role ARN required}"

IFS=: read -r ARN_PREFIX PARTITION SERVICE ARN_REGION ACCOUNT_ID RESOURCE <<<"$ROLE_ARN"
if [[ "$ARN_PREFIX" != "arn" || "$SERVICE" != "iam" || -n "$ARN_REGION" ||
      ! "$ACCOUNT_ID" =~ ^[0-9]{12}$ || "$RESOURCE" != role/* ]]; then
  echo "Invalid IAM role ARN: $ROLE_ARN" >&2
  exit 2
fi

ROLE_PATH="${RESOURCE#role/}"
ROLE_NAME="${ROLE_PATH##*/}"

CALLER_ACCOUNT="$(aws sts get-caller-identity --query Account --output text)"
if [[ "$CALLER_ACCOUNT" != "$ACCOUNT_ID" ]]; then
  echo "AWS credentials are for account $CALLER_ACCOUNT, but $ROLE_ARN is in $ACCOUNT_ID" >&2
  exit 1
fi

# The Deny is not redundant. The Allow has to wildcard the version segment — the whole
# point is writing a name that contains the package version — and that wildcard also
# covers `/latest`, which CloudFormation owns (LambdaAdapterLayerX86Parameter, and the
# value the e2e fixture resolves at deploy time). Without the Deny, a mistyped version or
# a future bug in the publish step could overwrite stack-managed state out of band, where
# nobody would think to look for drift.
POLICY_DOCUMENT="$(
  jq -cn \
    --arg x86 "arn:${PARTITION}:ssm:*:${ACCOUNT_ID}:parameter/lambda-web-adapter/layer/x86_64/*" \
    --arg arm "arn:${PARTITION}:ssm:*:${ACCOUNT_ID}:parameter/lambda-web-adapter/layer/arm64/*" \
    --arg x86latest "arn:${PARTITION}:ssm:*:${ACCOUNT_ID}:parameter/lambda-web-adapter/layer/x86_64/latest" \
    --arg armlatest "arn:${PARTITION}:ssm:*:${ACCOUNT_ID}:parameter/lambda-web-adapter/layer/arm64/latest" \
    '{
      Version: "2012-10-17",
      Statement: [
        {
          Sid: "PublishVersionedLayerParameters",
          Effect: "Allow",
          Action: "ssm:PutParameter",
          Resource: [$x86, $arm]
        },
        {
          Sid: "DenyCloudFormationOwnedLatestPointers",
          Effect: "Deny",
          Action: "ssm:PutParameter",
          Resource: [$x86latest, $armlatest]
        }
      ]
    }'
)"

aws iam put-role-policy \
  --role-name "$ROLE_NAME" \
  --policy-name LambdaWebAdapterVersionedLayerParameters \
  --policy-document "$POLICY_DOCUMENT"

echo "Granted ssm:PutParameter on /lambda-web-adapter/layer/{x86_64,arm64}/* to $ROLE_ARN"
echo "(all regions in partition $PARTITION; /latest explicitly denied)"
