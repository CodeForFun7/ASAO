// Runs the Vercel function on http://localhost:3000 for testing, without the Vercel CLI.
// Reads DATABASE_URL and DATABASE_CA_CERT from .env.local (see package.json "dev").
import { createServer } from "node:http";
import type { VercelRequest, VercelResponse } from "@vercel/node";
import handler from "../api/[action]";

const PORT = Number(process.env.PORT ?? 3000);

createServer(async (req, res) => {
  const url = new URL(req.url ?? "/", `http://localhost:${PORT}`);
  const match = /^\/api\/([a-z-]+)$/.exec(url.pathname);
  if (!match) {
    res.writeHead(404).end();
    return;
  }

  let raw = "";
  for await (const chunk of req) raw += chunk;
  let body: unknown = undefined;
  try {
    body = raw ? JSON.parse(raw) : undefined;
  } catch {
    body = raw;
  }

  // The few VercelRequest/VercelResponse helpers the handler uses.
  const vreq = Object.assign(req, { query: { action: match[1] }, body, cookies: {} }) as unknown as VercelRequest;
  const vres = res as unknown as VercelResponse;
  vres.status = (code: number) => {
    res.statusCode = code;
    return vres;
  };
  vres.json = (data: unknown) => {
    res.setHeader("Content-Type", "application/json");
    res.end(JSON.stringify(data));
    return vres;
  };
  await handler(vreq, vres);
}).listen(PORT, () => console.log(`ASAO accounts API on http://localhost:${PORT}`));
