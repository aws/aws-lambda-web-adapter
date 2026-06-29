# FastAPI with Lambda SnapStart (container image)

This example shows how to use the Lambda Web Adapter's SnapStart hooks to drain and
re-establish a connection pool around the snapshot/restore boundary, packaged as a
**container image** (OCI) rather than a zip.

> **Availability:** Lambda SnapStart for container images (OCI) is expected to launch
> in early July. Until it is available in your account/Region, deploying this example
> with `SnapStart: ApplyOn: PublishedVersions` on an `Image` package type will be
> rejected. The application itself runs unchanged with or without SnapStart — the
> hooks are simply not invoked when SnapStart is off.

For the zip-packaged equivalent, see
[fastapi-snapstart-zip](../fastapi-snapstart-zip).

## How does it work?

The [Dockerfile](app/Dockerfile) copies the Lambda Web Adapter binary into
`/opt/extensions`:

```dockerfile
FROM public.ecr.aws/docker/library/python:3.12-slim
COPY --from=public.ecr.aws/awsguru/aws-lambda-adapter:1.1.0 /lambda-adapter /opt/extensions/lambda-adapter
ENV PORT=8000
WORKDIR /var/task
COPY requirements.txt ./
RUN python -m pip install -r requirements.txt
COPY *.py ./
CMD exec uvicorn --port=$PORT main:app
```

The two SnapStart hook endpoints are configured as function environment variables in
[`template.yaml`](template.yaml), keeping the image itself generic:

```yaml
      Environment:
        Variables:
          AWS_LWA_SNAPSTART_BEFORE_CHECKPOINT_PATH: /snapstart/before
          AWS_LWA_SNAPSTART_AFTER_RESTORE_PATH: /snapstart/after
```

When the function runs under SnapStart, the adapter calls your application at the
snapshot boundary:

- **Before the snapshot** — `AWS_LWA_SNAPSTART_BEFORE_CHECKPOINT_PATH` is set to
  `/snapstart/before`. The adapter sends an empty HTTP `POST` to this path. The app uses
  it to drain and close resources that will not survive the snapshot (in this example,
  it closes the connection pool).
- **After restore**, before traffic is served, the adapter performs three steps in
  order:
  1. It refreshes its own HTTP connection to the inner app, so it never reuses a
     connection captured in the snapshot (this is automatic — you do not configure or
     manage the adapter's client).
  2. `AWS_LWA_SNAPSTART_AFTER_RESTORE_PATH` is set to `/snapstart/after`. The adapter
     sends an empty HTTP `POST` to this path over the refreshed connection. The app uses
     it to re-establish connections and regenerate per-environment unique values (in
     this example, it reconnects the pool and generates a fresh `connection_id`).
  3. It re-runs the readiness check against your app before admitting traffic.

Both hook routes must return a `2xx` status code. A non-2xx response, a connection
failure, or taking longer than 60 seconds to respond fails the SnapStart phase.
Likewise, if the readiness check does not pass within 10 seconds of restore, the
restore fails — so traffic is never served against an app that has not finished
recovering.

These hook routes are protected: the adapter only allows them to be invoked internally
during the SnapStart lifecycle. External callers that request `/snapstart/before` or
`/snapstart/after` receive a `403 Forbidden`.

Because the adapter is packaged inside the image, the same container also runs
unchanged on Amazon ECS, Amazon EKS, or a local Docker host.

## Pre-requisites

* [AWS CLI](https://aws.amazon.com/cli/)
* [SAM CLI](https://github.com/aws/aws-sam-cli)
* [Docker](https://www.docker.com/products/docker-desktop)

## Build and Deploy

Build the container image and deploy with SAM:

```bash
sam build
sam deploy --guided
```

When the deployment completes, take note of the `FastAPISnapStartApi` output — it is
the API Gateway endpoint URL.

## Verify it works

Open the API URL in a browser or with `curl`:

```bash
curl https://xxxxxxxxxx.execute-api.us-west-2.amazonaws.com/
```

The response reports the connection state, for example:

```json
{
  "message": "Hello from FastAPI on Lambda SnapStart (container image)",
  "connected": true,
  "connection_id": 426384719
}
```

After a SnapStart restore, the `connection_id` is regenerated rather than shared across
every restored environment, because the adapter calls the after-restore hook
(`/snapstart/after`), which reconnects the pool and generates a fresh id. This is
exactly the behavior you want for any per-environment value (connections, random seeds,
unique identifiers) that must not be duplicated across restored snapshots.

## Run the container locally

The same image runs locally — without SnapStart, the hooks are simply never invoked:

```bash
docker run -d -p 8000:8000 {ECR Image}
curl localhost:8000/
```
