from fastapi import FastAPI, HTTPException
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel, Field

from .graph import run_turn

app = FastAPI(title="Nyra local agent")
app.add_middleware(
    CORSMiddleware,
    allow_origins=["http://localhost:1420", "http://tauri.localhost", "https://tauri.localhost"],
    allow_methods=["POST", "OPTIONS"],
    allow_headers=["content-type"],
)


class Message(BaseModel):
    role: str
    content: str


class ChatRequest(BaseModel):
    api_key: str = Field(min_length=1, max_length=300)
    messages: list[Message] = Field(min_length=1, max_length=50)


@app.post("/chat")
def chat(body: ChatRequest):
    try:
        reply = run_turn(body.api_key, [message.model_dump() for message in body.messages])
        return {"reply": reply}
    except Exception as exc:
        raise HTTPException(status_code=502, detail=f"Assistant request failed: {exc}") from exc
