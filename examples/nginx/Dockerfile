FROM public.ecr.aws/awsguru/nginx:1.23.2023.3.11.1

COPY --from=public.ecr.aws/awsguru/aws-lambda-adapter:1.0.0 /lambda-adapter /opt/extensions/lambda-adapter

# config files
ADD nginx/conf/nginx.conf /opt/nginx/conf/nginx.conf

# code files
COPY app /var/task/

EXPOSE 8080
