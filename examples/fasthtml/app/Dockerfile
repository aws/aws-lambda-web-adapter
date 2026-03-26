FROM public.ecr.aws/docker/library/python:3.12-slim
COPY --from=public.ecr.aws/awsguru/aws-lambda-adapter:1.0.0 /lambda-adapter /opt/extensions/lambda-adapter
ENV PORT=8000
WORKDIR /var/task
COPY requirements.txt ./
RUN python -m pip install -r requirements.txt
COPY *.py ./
CMD python main.py
