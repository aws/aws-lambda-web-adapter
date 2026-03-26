FROM public.ecr.aws/docker/library/node:18.19-slim as builder
RUN npm i -g pnpm

WORKDIR /app

COPY . .
RUN pnpm install && pnpm run build

FROM public.ecr.aws/docker/library/node:18.19-slim as runner
COPY --from=public.ecr.aws/awsguru/aws-lambda-adapter:1.0.0 /lambda-adapter /opt/extensions/lambda-adapter

ENV PORT=8080 NODE_ENV=production
ENV AWS_LWA_ENABLE_COMPRESSION=true

WORKDIR ${LAMBDA_TASK_ROOT}
COPY --from=builder /app/package.json ./package.json
COPY --from=builder /app/dist ./dist
COPY --from=builder /app/node_modules ./node_modules

CMD [ "node", "dist/main.js" ]
