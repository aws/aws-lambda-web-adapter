import os
from anthropic import AnthropicBedrock
import secrets
from fasthtml.common import *
from starlette.responses import StreamingResponse
import asyncio

# Create a FastHTML application, setting the secret_key to avoid creating the .sesskey file on read only fs
app, rt = fast_app(
    hdrs=[
        Script(src="https://cdn.tailwindcss.com"),
        Link(rel="stylesheet", href="https://cdn.jsdelivr.net/npm/daisyui@4.11.1/dist/full.min.css"),
        Script(src="https://unpkg.com/htmx-ext-transfer-encoding-chunked@0.4.0/transfer-encoding-chunked.js"),
    ],
    ct_hdr=True,
    secret_key=secrets.token_hex(32),
    debug=True,
)

client = AnthropicBedrock()

def StoryInput():
    return Input(name='topic', id='topic-input', placeholder="Enter a topic for a bedtime story",
                 cls="input input-bordered w-full", hx_swap_oob='true')

@rt("/")
def index():
    page = Form(hx_post=send, hx_target="#story-output", hx_swap="beforeend", hx_ext="chunked-transfer")(
            Div(cls="flex space-x-2 mt-2")(
                Group(StoryInput(), Button("Generate", cls="btn btn-primary"), id="msg-group")
            ),
            P(id="story-output"),
           )
    return Titled('Serverless Bedtime Storyteller', page)

async def story_generator(content):
    response_txt = ''
    with client.messages.stream(
        max_tokens=1024,
        system="You are a world class writer.",
        messages=[{"role": "user", "content": content}],
        model="anthropic.claude-3-haiku-20240307-v1:0",
    ) as stream:
        for text in stream.text_stream:
            # print(text, end="", flush=True)
            response_txt += text
            yield to_xml(Div(
                        response_txt,
                        cls=f"mt-2 w-full rounded-t-lg text-sm whitespace-pre-wrap h-auto marked",
                        id=f"story-output",
                        hx_swap_oob="outerHTML",
                    ))
            await asyncio.sleep(0.1)

@app.post
async def send(topic: str):
    content = f"Please write a short and sweet bedtime story about {topic}."
    return StreamingResponse(story_generator(content), media_type="text/plain", 
                             headers={"X-Transfer-Encoding": "chunked"})

serve(port=int(os.environ.get("PORT", "8080")), reload=True)
