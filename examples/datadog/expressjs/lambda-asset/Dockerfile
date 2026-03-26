FROM public.ecr.aws/docker/library/node:slim

COPY --from=public.ecr.aws/datadog/lambda-extension:84 /opt/. /opt/
COPY --from=public.ecr.aws/awsguru/aws-lambda-adapter:1.0.0 /lambda-adapter /opt/extensions/lambda-adapter

EXPOSE 8080
WORKDIR "/var/task"
ADD src/ /var/task/
ADD src/package.json /var/task/package.json
ADD src/package-lock.json /var/task/package-lock.json
RUN npm install --omit=dev
RUN chmod +rx -R /var/task/node_modules
RUN chmod +rx /var/task/index.js
CMD ["node", "index.js"]
