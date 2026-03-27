FROM golang:1.18-alpine AS build_base
RUN apk add --no-cache git
WORKDIR /tmp/gin

COPY . .
RUN go mod download

RUN GOOS=linux CGO_ENABLED=0 go build -o bootstrap .
FROM alpine:3.9 
RUN apk add ca-certificates
COPY --from=public.ecr.aws/awsguru/aws-lambda-adapter:1.0.0 /lambda-adapter /opt/extensions/lambda-adapter
COPY --from=build_base /tmp/gin/bootstrap /app/bootstrap

ENV PORT=8000 GIN_MODE=release
EXPOSE 8000

CMD ["/app/bootstrap"]
