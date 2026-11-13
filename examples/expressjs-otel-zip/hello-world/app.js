const express = require('express')
const app = express()
const port = process.env['PORT'] || 8080


app.get('/', (req, res) => {
    // deserialize request context from the http header 'x-amzn-request-context'
    let requestContextHeader = req.headers['x-amzn-request-context'] || null
    let requestContext = requestContextHeader != null? JSON.parse(requestContextHeader) : null

    // deserialize lambda context from the http header 'x-amzn-lambda-context'
    let lambdaContextHeader = req.headers['x-amzn-lambda-context'] || null
    let lambdaContext = lambdaContextHeader != null? JSON.parse(lambdaContextHeader) : null

    res.send({
        message: 'Hi there!', 
        requestContext,
        lambdaContext
    })
})

app.listen(port, () => {
    console.log(`Example app listening at http://localhost:${port}`)
})