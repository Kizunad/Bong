import http from "node:http";
import { spawn } from "node:child_process";
import crypto from "node:crypto";

const PORT = Number(process.env.PORT || 8080);
const REFRESH_TOKEN = process.env.REFRESH_TOKEN;
const PROJECT_ROOT = process.env.PROJECT_ROOT || "/workspace/app";
const COMPOSE_FILE = process.env.COMPOSE_FILE || `${PROJECT_ROOT}/docker-compose.yml`;
const COMPOSE_PROJECT_NAME = process.env.COMPOSE_PROJECT_NAME || "mofa-library";

let refreshInFlight = false;

function json(res, statusCode, payload) {
  const body = JSON.stringify(payload);
  res.writeHead(statusCode, {
    "Content-Type": "application/json; charset=utf-8",
    "Content-Length": Buffer.byteLength(body),
  });
  res.end(body);
}

function timingSafeEqualString(a, b) {
  const aBuf = Buffer.from(a);
  const bBuf = Buffer.from(b);
  if (aBuf.length !== bBuf.length) {
    return false;
  }
  return crypto.timingSafeEqual(aBuf, bBuf);
}

function isAuthorized(req) {
  if (!REFRESH_TOKEN) return false;
  const header = req.headers.authorization;
  if (!header || !header.startsWith("Bearer ")) return false;
  const provided = header.slice("Bearer ".length).trim();
  if (!provided) return false;
  if (provided.length !== REFRESH_TOKEN.length) return false;
  return timingSafeEqualString(provided, REFRESH_TOKEN);
}

function runCommand(args) {
  return new Promise((resolve, reject) => {
    const child = spawn("docker", args, {
      cwd: PROJECT_ROOT,
      env: process.env,
      stdio: ["ignore", "pipe", "pipe"],
    });

    let stdout = "";
    let stderr = "";

    child.stdout.on("data", (chunk) => {
      stdout += chunk.toString();
    });

    child.stderr.on("data", (chunk) => {
      stderr += chunk.toString();
    });

    child.on("error", reject);
    child.on("close", (code) => {
      if (code === 0) {
        resolve({ stdout, stderr });
        return;
      }
      const error = new Error(`docker ${args.join(" ")} exited with code ${code}`);
      error.stdout = stdout;
      error.stderr = stderr;
      reject(error);
    });
  });
}

async function refreshLibrary() {
  const commonArgs = ["compose", "-f", COMPOSE_FILE, "-p", COMPOSE_PROJECT_NAME];
  await runCommand([...commonArgs, "up", "-d", "--build", "--force-recreate", "--no-deps", "library"]);
}

const server = http.createServer(async (req, res) => {
  const url = new URL(req.url || "/", `http://${req.headers.host || "localhost"}`);

  if (req.method === "GET" && url.pathname === "/healthz") {
    json(res, 200, { ok: true });
    return;
  }

  if (url.pathname !== "/refresh") {
    json(res, 404, { error: "not_found" });
    return;
  }

  if (req.method !== "POST") {
    res.setHeader("Allow", "POST");
    json(res, 405, { error: "method_not_allowed" });
    return;
  }

  if (!isAuthorized(req)) {
    json(res, 401, { error: "unauthorized" });
    return;
  }

  if (refreshInFlight) {
    json(res, 409, { error: "refresh_in_progress" });
    return;
  }

  refreshInFlight = true;
  const startedAt = Date.now();

  try {
    const result = await refreshLibrary();
    json(res, 200, {
      ok: true,
      message: "library refreshed",
      durationMs: Date.now() - startedAt,
      stdout: result.stdout.trim() || undefined,
    });
  } catch (error) {
    console.error("[refresh-api] refresh failed:", error);
    json(res, 500, {
      ok: false,
      error: "refresh_failed",
      message: error.message,
      stderr: error.stderr?.trim() || undefined,
    });
  } finally {
    refreshInFlight = false;
  }
});

server.listen(PORT, "0.0.0.0", () => {
  console.log(`[refresh-api] listening on :${PORT}`);
});
