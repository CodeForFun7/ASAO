import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const venvPython = resolve(root, ".venv", "Scripts", "python.exe");
const python = existsSync(venvPython) ? venvPython : "python";
const env = {
  ...process.env,
  PYTHONPATH: [resolve(root, "src-python"), process.env.PYTHONPATH]
    .filter(Boolean)
    .join(process.platform === "win32" ? ";" : ":"),
};

const children = [
  spawn("pnpm", ["dev"], { cwd: root, env, stdio: "inherit", shell: process.platform === "win32" }),
  spawn(python, ["-m", "uvicorn", "chatbot.main:app", "--host", "127.0.0.1", "--port", "18743"], {
    cwd: root,
    env,
    stdio: "inherit",
    windowsHide: true,
  }),
];

let stopping = false;
function stop(code = 0) {
  if (stopping) return;
  stopping = true;
  for (const child of children) {
    if (!child.killed) child.kill("SIGTERM");
  }
  setTimeout(() => process.exit(code), 300);
}

for (const child of children) {
  child.on("error", (error) => {
    console.error(`Could not start ${child.spawnargs?.[0] ?? "development service"}: ${error.message}`);
    stop(1);
  });
  child.on("exit", (code) => {
    if (!stopping) {
      if (code !== 0) console.error("A development service stopped unexpectedly.");
      stop(code ?? 1);
    }
  });
}

process.on("SIGINT", () => stop(0));
process.on("SIGTERM", () => stop(0));
