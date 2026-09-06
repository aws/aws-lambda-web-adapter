import random

from fastapi import FastAPI, Response

app = FastAPI()


class ConnectionPool:
    """A stand-in for a real database/connection pool.

    In a real application these methods would open and close sockets to a
    database. Here we just track state so the SnapStart lifecycle is observable.
    """

    def __init__(self):
        self.connected = False
        self.connection_id = None

    def connect(self):
        # A fresh, unique id per environment — the kind of value that must be
        # regenerated after a SnapStart restore so it is not shared across
        # every restored environment.
        self.connection_id = random.randint(1, 1_000_000_000)
        self.connected = True

    def close(self):
        self.connected = False


pool = ConnectionPool()
pool.connect()


@app.get("/")
async def root():
    return {
        "message": "Hello from FastAPI on Lambda SnapStart (container image)",
        "connected": pool.connected,
        "connection_id": pool.connection_id,
    }


@app.post("/snapstart/before")
async def before_checkpoint():
    """Called by the adapter before the snapshot is taken.

    Close resources that will not survive the snapshot.
    """
    pool.close()
    return Response(status_code=200)


@app.post("/snapstart/after")
async def after_restore():
    """Called by the adapter after the environment is restored.

    Re-establish connections and regenerate per-environment unique values.
    """
    # Reseed from OS entropy first. Python's random module keeps its state in
    # process memory, which is captured in the snapshot — without reseeding,
    # every restored environment would draw the same "unique" value.
    random.seed()
    pool.connect()
    return Response(status_code=200)
