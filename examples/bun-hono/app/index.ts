import { Hono } from 'hono'

const app = new Hono()
const port = Number(process.env.PORT || 8080)

// Lambda Web Adapter forwards the API Gateway request context and the Lambda
// context to the app as JSON strings in two HTTP headers.
const parseContext = (header?: string) => (header ? JSON.parse(header) : null)

app.get('/', (c) => {
  const requestContext = parseContext(c.req.header('x-amzn-request-context'))
  const lambdaContext = parseContext(c.req.header('x-amzn-lambda-context'))

  // Return two fields only. The full contexts contain the AWS account ID.
  return c.json({
    message: 'Hello from Hono on Bun!',
    requestId: requestContext?.requestId ?? null,
    functionName: lambdaContext?.env_config?.function_name ?? null,
  })
})

app.get('/hello/:name', (c) => {
  return c.json({ message: `Hello, ${c.req.param('name')}!` })
})

const server = Bun.serve({ port, fetch: app.fetch })
console.log(`Hono app listening on port ${server.port}`)

// Lambda sends SIGTERM before it shuts down the execution environment.
process.on('SIGTERM', async () => {
  await server.stop()
  process.exit(0)
})
