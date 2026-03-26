FROM public.ecr.aws/docker/library/python:slim

COPY --from=public.ecr.aws/datadog/lambda-extension:84 /opt/. /opt/
COPY --from=public.ecr.aws/awsguru/aws-lambda-adapter:1.0.0 /lambda-adapter /opt/extensions/lambda-adapter

EXPOSE 8080
WORKDIR "/var/task"
ADD src/ /var/task/
RUN pip install -r requirements.txt
RUN chmod +rx /var/task/handler.py
ENV PORT=8080
ENV PYTHONUNBUFFERED=1

CMD ["python", "handler.py"]
