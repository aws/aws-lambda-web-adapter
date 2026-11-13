#!/bin/bash

# Load the ADOT auto-instrumentation before app.js, so it can patch http and express
# as they are required. It is shipped in node_modules: bundling breaks require-time patching.
export NODE_OPTIONS="--require @aws/aws-distro-opentelemetry-node-autoinstrumentation/register"
export OTEL_SERVICE_NAME=expressjs-otel
# Traces go to the Lambda X-Ray endpoint over UDP. There is no OTLP collector for
# metrics or logs, so turn those exporters off.
export OTEL_METRICS_EXPORTER=${OTEL_METRICS_EXPORTER:-none}
export OTEL_LOGS_EXPORTER=${OTEL_LOGS_EXPORTER:-none}
exec node app.js
