# Non-HTTP Event Triggers

Lambda Web Adapter supports all non-HTTP event triggers including SQS, SNS, S3, DynamoDB, Kinesis, Kafka, EventBridge, and Bedrock Agents.

## How It Works

The adapter forwards the raw event payload to your web application via an HTTP POST to a configurable path (default: `/events`).

1. Lambda receives a non-HTTP event (e.g. SQS message)
2. The adapter POSTs the event JSON body to `http://127.0.0.1:{port}/events`
3. Your app processes the event and returns a JSON response
4. The adapter forwards the response back to Lambda

## Configuration

```
AWS_LWA_PASS_THROUGH_PATH=/events
```

## Example Handler

```python
# FastAPI
@app.post("/events")
async def handle_event(request: Request):
    event = await request.json()
    # Process SQS, SNS, S3, etc.
    return {"statusCode": 200, "body": "processed"}
```

```javascript
// Express.js
app.post('/events', (req, res) => {
    const event = req.body;
    // Process the event
    res.json({ statusCode: 200, body: 'processed' });
});
```

## Examples

- [SQS Express.js](https://github.com/aws/aws-lambda-web-adapter/tree/main/examples/sqs-expressjs)
- [Bedrock Agent FastAPI](https://github.com/aws/aws-lambda-web-adapter/tree/main/examples/bedrock-agent-fastapi)
- [Bedrock Agent FastAPI in Zip](https://github.com/aws/aws-lambda-web-adapter/tree/main/examples/bedrock-agent-fastapi-zip)
