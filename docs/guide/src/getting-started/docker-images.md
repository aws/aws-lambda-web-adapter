# Docker Images

To use Lambda Web Adapter with Docker images, package your web app in a Dockerfile and add one line to copy the adapter binary:

```dockerfile
COPY --from=public.ecr.aws/awsguru/aws-lambda-adapter:1.0.0 /lambda-adapter /opt/extensions/lambda-adapter
```

[Non-AWS base images](https://docs.aws.amazon.com/lambda/latest/dg/images-create.html) can be used since the [Runtime Interface Client](https://docs.aws.amazon.com/lambda/latest/dg/images-create.html#images-ric) ships with the adapter.

## Example: Node.js Express App

```dockerfile
FROM public.ecr.aws/docker/library/node:20-slim
COPY --from=public.ecr.aws/awsguru/aws-lambda-adapter:1.0.0 /lambda-adapter /opt/extensions/lambda-adapter
ENV PORT=7000
WORKDIR "/var/task"
ADD src/package.json /var/task/package.json
ADD src/package-lock.json /var/task/package-lock.json
RUN npm install --omit=dev
ADD src/ /var/task
CMD ["node", "index.js"]
```

This works with any base image except AWS managed base images. To use AWS managed base images, override the `ENTRYPOINT` to start your web app.

## Custom Port

If your app listens on a port other than 8080, set the `AWS_LWA_PORT` environment variable:

```dockerfile
ENV AWS_LWA_PORT=3000
```

## Available Images

Pre-compiled multi-arch images (x86_64 and arm64) are available at:

```
public.ecr.aws/awsguru/aws-lambda-adapter:1.0.0
```
