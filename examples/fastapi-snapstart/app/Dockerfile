FROM public.ecr.aws/docker/library/python:3.12-slim
COPY --from=048972532408.dkr.ecr.us-west-2.amazonaws.com/aws-lambda-adapter:1.1.0 /lambda-adapter /opt/extensions/lambda-adapter
ENV PORT=8000
WORKDIR /var/task
COPY requirements.txt ./
RUN python -m pip install -r requirements.txt
COPY *.py ./
CMD exec uvicorn --port=$PORT main:app
