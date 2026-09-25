# Multi-Tenancy

Lambda Web Adapter supports multi-tenancy by propagating the tenant ID from the Lambda invocation context to your web application as an `X-Amz-Tenant-Id` HTTP header.

## How It Works

When the Lambda invocation context carries a tenant ID, the adapter forwards it as an `X-Amz-Tenant-Id` HTTP header. When it does not, the adapter sets no such header.

The adapter reads the tenant ID only from the invocation context, never from the forwarded request. It removes any `X-Amz-Tenant-Id` header the caller sent before setting its own, so your application never reads a caller-supplied value from this header.

That makes the header exactly as trustworthy as whatever puts the tenant ID into the invocation context. Getting that part right is the subject of the prerequisites below.

> **Warning:** the stripping exists only while the adapter is in the request path — that is, when your application runs on Lambda behind the adapter. If you run the same image or application **without** the adapter (Amazon ECS, Amazon EKS, a local Docker host), nothing removes a caller-supplied `X-Amz-Tenant-Id`, and an application that treats it as an asserted identity is reading raw caller input. Establish the tenant another way in those deployments, or reject the header at your edge.

## Prerequisites

The tenant ID only reaches your application if the function uses [Lambda tenant isolation](https://docs.aws.amazon.com/lambda/latest/dg/tenant-isolation.html). That carries constraints worth knowing before you build on it:

- **Tenant isolation must be enabled when the function is created.** It is an immutable function property and cannot be added to an existing function.
- **Function URLs are not supported**, and neither are provisioned concurrency or SnapStart.
- **API Gateway REST APIs are the only supported HTTP trigger.** HTTP APIs cannot be used, because they cannot override the `X-Amz-Tenant-Id` header that Lambda's `Invoke` API requires.
- **Every invocation must carry a tenant ID.** Lambda rejects an invocation of a tenant-isolated function that has none, so such a request fails before your application runs.

With API Gateway REST you map a request property to `integration.request.header.X-Amz-Tenant-Id`, which is the header Lambda's `Invoke` API reads. **Map it from something the caller cannot choose** — an authorizer context value or a verified token claim:

```text
integration.request.header.X-Amz-Tenant-Id = context.authorizer.tenantId
```

A raw client header is not such a source. Mapping `method.request.header.x-tenant-id` straight through lets any caller name their own tenant: API Gateway forwards the value it was given, Lambda puts it in the invocation context, and the adapter then asserts it as `X-Amz-Tenant-Id` — so the application receives a caller-chosen tenant that looks like a platform-asserted one. Have your authorizer establish the tenant from the caller's credentials and map that.

See [Invoking Lambda functions with tenant isolation](https://docs.aws.amazon.com/lambda/latest/dg/tenant-isolation-invoke.html) for the full setup.

If the function does not use tenant isolation, no request carries a tenant ID and the adapter sets no `X-Amz-Tenant-Id` header.

## Reading the Tenant ID

```python
# FastAPI
@app.get("/")
def handler(request: Request):
    tenant_id = request.headers.get("x-amz-tenant-id")
```

```javascript
// Express.js
app.get('/', (req, res) => {
    const tenantId = req.headers['x-amz-tenant-id'];
});
```

The adapter itself needs no configuration; the prerequisites above are function and API Gateway settings.

## Do Not Fall Back to a Client-Supplied Tenant

If your application scopes data by tenant, treat a missing `X-Amz-Tenant-Id` as an error rather than falling back to a default tenant or to another header the caller controls. A fallback like this turns the tenant identity into caller input:

```python
# Don't do this: the caller chooses the tenant.
tenant_id = request.headers.get("x-amz-tenant-id") or request.headers.get("x-tenant-id")
```

A missing header on a tenant-isolated function means the deployment is wrong, not that the request belongs to a default tenant.
