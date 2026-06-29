FROM public.ecr.aws/docker/library/python:3.12-slim
COPY --from=public.ecr.aws/awsguru/aws-lambda-adapter:1.0.1 /lambda-adapter /opt/extensions/lambda-adapter
ENV PORT=8000
ENV AWS_LWA_SNAPSTART_BEFORE_CHECKPOINT_PATH=/snapstart/before
ENV AWS_LWA_SNAPSTART_AFTER_RESTORE_PATH=/snapstart/after
WORKDIR /var/task
COPY requirements.txt ./
RUN python -m pip install -r requirements.txt
COPY *.py ./
CMD exec uvicorn --port=$PORT main:app
