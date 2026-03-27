FROM --platform=linux/amd64 public.ecr.aws/docker/library/python:3.14-slim AS builder
WORKDIR /var/task
COPY requirements.txt ./
RUN pip install --no-cache-dir --target=/var/task/deps -r requirements.txt

FROM --platform=linux/amd64 public.ecr.aws/docker/library/python:3.14-slim
COPY --from=public.ecr.aws/awsguru/aws-lambda-adapter:1.0.0 /lambda-adapter /opt/extensions/lambda-adapter
ENV PORT=8000 PYTHONPATH=/var/task/deps
ENV AWS_LWA_READINESS_CHECK_PATH=/healthz
ENV AWS_LWA_READINESS_CHECK_HEALTHY_STATUS=100-499
WORKDIR /var/task
COPY --from=builder /var/task/deps ./deps
COPY *.py ./
CMD exec python -m uvicorn --port=$PORT app:app
