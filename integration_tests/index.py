from axon import Axon

app = Axon(__file__)


@app.get("/")
async def h():
    return "Hello, world!"


app.start()
