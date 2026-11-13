from fastapi import FastAPI
from background import BackgroundTaskExtension
import time

app = FastAPI()

# create a background task extension
background_task = BackgroundTaskExtension()

# add a middleware to send done message when a request is done
# this middleware will be called for each request
@app.middleware("http")
async def send_done_message(request, call_next):
    response = await call_next(request)
    # only Lambda invokes carry this header; skip readiness checks
    if "x-amzn-lambda-context" in request.headers:
        background_task.done()
    return response

def mock_task(seconds):
    print("in mock_task method \n")
    time.sleep(seconds)

@app.post("/tasks", status_code=201)
async def create_task():
    print("in create_task method \n")
    background_task.add_task(mock_task, 2)
    background_task.add_task(mock_task, 3)
    return "task created"

@app.get("/")
async def root():
    print("in root method \n")
    return {"message": "Hello World"}
