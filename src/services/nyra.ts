export interface NyraMessage {
  role: "user" | "assistant";
  content: string;
}

export async function sendNyraMessage(apiKey: string, messages: NyraMessage[]): Promise<string> {
  const response = await fetch("http://127.0.0.1:18743/chat", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ api_key: apiKey, messages }),
  });
  const payload = await response.json().catch(() => ({}));
  if (!response.ok) throw new Error(payload.detail ?? "Could not reach the Nyra backend.");
  return payload.reply;
}
