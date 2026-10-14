FROM public.ecr.aws/docker/library/node:22-slim
COPY --from=public.ecr.aws/awsguru/aws-lambda-adapter:1.1.0 /lambda-adapter /opt/extensions/lambda-adapter
# Bun has no official image on Amazon ECR Public, so install it with npm.
RUN npm install --global bun@1.4.2
# Listen on 8000 (the app reads PORT) so it doesn't collide with SAM's Runtime
# Interface Emulator on 8080 during local testing.
ENV PORT=8000
# The Lambda file system is read-only, so turn off Bun's transpiler cache.
ENV BUN_RUNTIME_TRANSPILER_CACHE_PATH=0
EXPOSE 8000
WORKDIR /var/task
COPY package.json bun.lock ./
RUN bun install --frozen-lockfile --production
COPY index.ts ./
CMD ["bun", "run", "index.ts"]
