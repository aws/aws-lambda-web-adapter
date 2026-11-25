import { serve } from '@hono/node-server'
import { Hono } from 'hono'

const app = new Hono()
const port = Number(process.env.PORT || 8080)

// Lambda Web Adapter forwards the API Gateway request context and the Lambda
// context to the app as JSON strings in two HTTP headers.
const parseContext = (header) => (header ? JSON.parse(header) : null)

app.get('/', (c) => {
  const requestContext = parseContext(c.req.header('x-amzn-request-context'))
  const lambdaContext = parseContext(c.req.header('x-amzn-lambda-context'))

  // Return two fields only. The full contexts contain the AWS account ID.
  return c.json({
    message: 'Hello from Hono!',
    requestId: requestContext?.requestId ?? null,
    functionName: lambdaContext?.env_config?.function_name ?? null,
  })
})

app.get('/hello/:name', (c) => {
  return c.json({ message: `Hello, ${c.req.param('name')}!` })
})

const server = serve({ fetch: app.fetch, port }, (info) => {
  console.log(`Hono app listening on port ${info.port}`)
})

// Lambda sends SIGTERM before it shuts down the execution environment.
process.on('SIGTERM', () => {
  server.close(() => process.exit(0))
})
