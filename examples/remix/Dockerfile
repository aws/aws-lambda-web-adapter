FROM public.ecr.aws/docker/library/node:20-bookworm-slim as builder
WORKDIR "/var/task"
ADD . .
RUN cd remix-app && npm install && npm run build && npm prune --omit=dev

FROM public.ecr.aws/docker/library/node:20-bookworm-slim
COPY --from=public.ecr.aws/awsguru/aws-lambda-adapter:1.0.0 /lambda-adapter /opt/extensions/lambda-adapter
WORKDIR "/var/task"
COPY --from=builder  /var/task/remix-app/build /var/task/build
COPY --from=builder  /var/task/remix-app/node_modules /var/task/node_modules
COPY --from=builder  /var/task/remix-app/server.js /var/task/server.js
COPY --from=builder  /var/task/remix-app/package.json /var/task/package.json
ENV NODE_ENV=production PORT=3000
CMD ["node", "server.js"]
