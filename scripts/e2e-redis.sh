#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
EVIDENCE_DIR="$ROOT/.sisyphus/evidence"
TASK_ID="task-13"
SCRIPT_TAG="e2e-redis"
RUN_LABEL="${RUN_LABEL:-default}"
RUN_ID="$(date +%Y%m%d-%H%M%S)-$$-${RUN_LABEL}"
RUN_DIR="$EVIDENCE_DIR/${TASK_ID}-${SCRIPT_TAG}-run-${RUN_ID}"
LOG_FILE="$EVIDENCE_DIR/${TASK_ID}-${SCRIPT_TAG}.log"
ERROR_FILE="$EVIDENCE_DIR/${TASK_ID}-${SCRIPT_TAG}-error.log"
SUCCESS_FILE="$EVIDENCE_DIR/${TASK_ID}-${SCRIPT_TAG}-success.txt"
MANIFEST_FILE="$EVIDENCE_DIR/${TASK_ID}-${SCRIPT_TAG}-manifest.txt"

REDIS_URL="${REDIS_URL:-redis://127.0.0.1:6379}"
DEFAULT_REDIS_URL="redis://127.0.0.1:6379"
NODE_BIN="$ROOT/agent/node_modules/.bin"
RUST_PATH="/opt/rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin:$PATH"

REDIS_LOG="$RUN_DIR/redis.log"
SERVER_LOG="$RUN_DIR/server.log"
REDIS_SUB_LOG="$RUN_DIR/redis-sub.log"
TIANDAO_LOG="$RUN_DIR/tiandao.log"

PASS=0
FAIL=0
CURRENT_STAGE="init"
REDIS_PID=""
SERVER_PID=""
REDIS_SUB_PID=""
REDIS_PROVIDER=""
REDIS_SERVER_BIN=""
DOCKER_CONTAINER_NAME="bong-task-13-redis-${RUN_ID}"
DOCKER_REDIS_STARTED=0

mkdir -p "$EVIDENCE_DIR" "$RUN_DIR"
touch "$LOG_FILE"
exec > >(tee -a "$LOG_FILE") 2>&1

pass() {
  echo "  ✓ $1"
  PASS=$((PASS + 1))
}

write_manifest() {
  local status="$1"
  local stage_name="$2"
  local message="$3"
  printf "task=%s\nscript=%s\nrun_id=%s\nrun_label=%s\nstatus=%s\nstage=%s\nmessage=%s\ntimestamp=%s\nfiles:\n- %s\n- %s\n- %s\n- %s\n- %s\n- %s\n- %s\n- %s\n" \
    "$TASK_ID" \
    "$SCRIPT_TAG" \
    "$RUN_ID" \
    "$RUN_LABEL" \
    "$status" \
    "$stage_name" \
    "$message" \
    "$(date -Iseconds)" \
    "$LOG_FILE" \
    "$ERROR_FILE" \
    "$MANIFEST_FILE" \
    "$SUCCESS_FILE" \
    "$REDIS_LOG" \
    "$SERVER_LOG" \
    "$REDIS_SUB_LOG" \
    "$TIANDAO_LOG" >"$MANIFEST_FILE"
}

finalize_failure() {
  local stage_name="$1"
  local message="$2"
  FAIL=$((FAIL + 1))
  rm -f "$SUCCESS_FILE"
  printf "task=%s\nscript=%s\nstatus=FAILED\nstage=%s\nmessage=%s\nrun_id=%s\n" \
    "$TASK_ID" \
    "$SCRIPT_TAG" \
    "$stage_name" \
    "$message" \
    "$RUN_ID" >"$ERROR_FILE"
  write_manifest "FAILED" "$stage_name" "$message"
  echo "[evidence] manifest: $MANIFEST_FILE"
  echo "[evidence] run_dir: $RUN_DIR"
  echo "[$TASK_ID][FAIL][$stage_name] $message"
  exit 1
}

wait_for_pattern() {
  local file="$1"
  local pattern="$2"
  local timeout_secs="$3"
  local elapsed=0

  while [ "$elapsed" -lt "$timeout_secs" ]; do
    if [ -f "$file" ] && grep -Eq "$pattern" "$file"; then
      return 0
    fi
    sleep 1
    elapsed=$((elapsed + 1))
  done

  return 1
}

probe_redis() {
  (
    cd "$ROOT/agent/packages/tiandao"
    PATH="$NODE_BIN:$PATH" REDIS_URL="$REDIS_URL" node --input-type=module <<'NODE'
import Redis from "ioredis";

const IORedis = Redis.default ?? Redis;
const url = process.env.REDIS_URL ?? "redis://127.0.0.1:6379";
const client = new IORedis(url, {
  lazyConnect: true,
  maxRetriesPerRequest: 1,
  enableOfflineQueue: false,
});

try {
  await client.connect();
  const pong = await client.ping();
  if (pong !== "PONG") {
    process.exit(1);
  }
  await client.quit();
  process.exit(0);
} catch {
  try {
    client.disconnect();
  } catch {
    // ignore disconnect cleanup failures
  }
  process.exit(1);
}
NODE
  ) >/dev/null 2>&1
}

start_redis_subscriber() {
  (
    cd "$ROOT/agent/packages/tiandao"
    PATH="$NODE_BIN:$PATH" REDIS_URL="$REDIS_URL" node --input-type=module <<'NODE'
import Redis from "ioredis";

const IORedis = Redis.default ?? Redis;
const url = process.env.REDIS_URL ?? "redis://127.0.0.1:6379";
const channels = ["bong:world_state", "bong:agent_command", "bong:agent_narrate"];
const sub = new IORedis(url, { maxRetriesPerRequest: 1 });

const shutdown = async () => {
  try {
    await sub.unsubscribe(...channels);
  } catch {
    // ignore unsubscribe failure during shutdown
  }
  try {
    await sub.quit();
  } catch {
    try {
      sub.disconnect();
    } catch {
      // ignore disconnect failure during shutdown
    }
  }
  process.exit(0);
};

process.on("SIGINT", shutdown);
process.on("SIGTERM", shutdown);

await sub.subscribe(...channels);
console.log(`[task-13][redis-sub] subscribed ${channels.join(",")}`);
sub.on("message", (channel, message) => {
  console.log(`[task-13][redis-sub] channel=${channel} payload=${message}`);
});

setInterval(() => {}, 1000);
NODE
  ) >"$REDIS_SUB_LOG" 2>&1 &
  REDIS_SUB_PID="$!"
}

start_local_redis_binary() {
  "$REDIS_SERVER_BIN" --save "" --appendonly no --bind 127.0.0.1 --port 6379 --loglevel warning >"$REDIS_LOG" 2>&1 &
  REDIS_PID="$!"
  REDIS_PROVIDER="binary:$REDIS_SERVER_BIN"
}

start_inline_resp_redis() {
  cat >"$RUN_DIR/inline-redis.mjs" <<'NODE'
import net from "node:net";

const HOST = process.env.TASK13_REDIS_HOST ?? "127.0.0.1";
const PORT = Number(process.env.TASK13_REDIS_PORT ?? "6379");

let nextClientId = 1;
const hashes = new Map();
const lists = new Map();
const channelSubscribers = new Map();
const patternSubscribers = new Map();
const patternCache = new Map();
const clients = new Set();

function log(message) {
  console.log(`[task-13][inline-redis] ${message}`);
}

function asString(value) {
  return value === null || value === undefined ? "" : String(value);
}

function simple(value) {
  return { kind: "simple", value: asString(value) };
}

function errorReply(value) {
  return { kind: "error", value: asString(value) };
}

function integer(value) {
  return { kind: "int", value: Number(value) };
}

function bulk(value) {
  return { kind: "bulk", value: value === null || value === undefined ? null : asString(value) };
}

function array(value) {
  return { kind: "array", value };
}

function encode(value) {
  if (value?.kind === "simple") {
    return Buffer.from(`+${value.value}\r\n`);
  }

  if (value?.kind === "error") {
    return Buffer.from(`-${value.value}\r\n`);
  }

  if (value?.kind === "int") {
    return Buffer.from(`:${value.value}\r\n`);
  }

  if (value?.kind === "bulk") {
    if (value.value === null) {
      return Buffer.from(`$-1\r\n`);
    }

    const body = Buffer.from(value.value);
    return Buffer.concat([Buffer.from(`$${body.length}\r\n`), body, Buffer.from(`\r\n`)]);
  }

  if (value?.kind === "array") {
    const parts = [Buffer.from(`*${value.value.length}\r\n`)];
    for (const item of value.value) {
      parts.push(encode(item));
    }
    return Buffer.concat(parts);
  }

  if (Array.isArray(value)) {
    return encode(array(value));
  }

  if (typeof value === "number") {
    return encode(integer(value));
  }

  if (typeof value === "string") {
    return encode(bulk(value));
  }

  if (value === null || value === undefined) {
    return encode(bulk(null));
  }

  throw new Error(`cannot encode value: ${JSON.stringify(value)}`);
}

function readLine(buffer, offset) {
  const lineEnd = buffer.indexOf("\r\n", offset);
  if (lineEnd === -1) {
    return null;
  }
  return {
    line: buffer.subarray(offset, lineEnd).toString("utf8"),
    next: lineEnd + 2,
  };
}

function parseFrame(buffer, offset = 0) {
  if (offset >= buffer.length) {
    return null;
  }

  const prefix = String.fromCharCode(buffer[offset]);
  const line = readLine(buffer, offset + 1);
  if (!line) {
    return null;
  }

  if (prefix === "+") {
    return { value: line.line, next: line.next };
  }

  if (prefix === ":") {
    return { value: Number(line.line), next: line.next };
  }

  if (prefix === "-") {
    return { value: new Error(line.line), next: line.next };
  }

  if (prefix === "$") {
    const length = Number(line.line);
    if (Number.isNaN(length)) {
      throw new Error(`invalid bulk length: ${line.line}`);
    }
    if (length === -1) {
      return { value: null, next: line.next };
    }
    const end = line.next + length;
    if (buffer.length < end + 2) {
      return null;
    }
    const value = buffer.subarray(line.next, end).toString("utf8");
    return { value, next: end + 2 };
  }

  if (prefix === "*") {
    const count = Number(line.line);
    if (Number.isNaN(count)) {
      throw new Error(`invalid array length: ${line.line}`);
    }
    if (count === -1) {
      return { value: null, next: line.next };
    }

    let next = line.next;
    const items = [];
    for (let index = 0; index < count; index += 1) {
      const parsed = parseFrame(buffer, next);
      if (!parsed) {
        return null;
      }
      items.push(parsed.value);
      next = parsed.next;
    }

    return { value: items, next };
  }

  throw new Error(`unsupported RESP prefix: ${prefix}`);
}

function sendFrame(client, frame) {
  if (!client.socket.destroyed) {
    client.socket.write(encode(frame));
  }
}

function sendFrames(client, frames) {
  if (!client.socket.destroyed) {
    client.socket.write(Buffer.concat(frames.map((frame) => encode(frame))));
  }
}

function getHash(key) {
  let hash = hashes.get(key);
  if (!hash) {
    hash = new Map();
    hashes.set(key, hash);
  }
  return hash;
}

function getList(key) {
  let list = lists.get(key);
  if (!list) {
    list = [];
    lists.set(key, list);
  }
  return list;
}

function addSubscription(map, key, client) {
  let bucket = map.get(key);
  if (!bucket) {
    bucket = new Set();
    map.set(key, bucket);
  }
  bucket.add(client);
}

function removeSubscription(map, key, client) {
  const bucket = map.get(key);
  if (!bucket) {
    return;
  }
  bucket.delete(client);
  if (bucket.size === 0) {
    map.delete(key);
  }
}

function subscriptionCount(client) {
  return client.channels.size + client.patterns.size;
}

function globMatcher(pattern) {
  const cached = patternCache.get(pattern);
  if (cached) {
    return cached;
  }

  const escaped = pattern
    .replace(/[|\\{}()[\]^$+?.]/g, "\\$&")
    .replace(/\*/g, ".*")
    .replace(/\?/g, ".");
  const regex = new RegExp(`^${escaped}$`);
  patternCache.set(pattern, regex);
  return regex;
}

function normalizeRange(list, startRaw, stopRaw) {
  let start = Number.parseInt(asString(startRaw), 10);
  let stop = Number.parseInt(asString(stopRaw), 10);

  if (!Number.isInteger(start) || !Number.isInteger(stop)) {
    return null;
  }

  const length = list.length;
  if (start < 0) {
    start += length;
  }
  if (stop < 0) {
    stop += length;
  }

  if (start < 0) {
    start = 0;
  }
  if (stop < 0) {
    return [];
  }
  if (start >= length) {
    return [];
  }
  if (stop >= length) {
    stop = length - 1;
  }
  if (start > stop) {
    return [];
  }

  return list.slice(start, stop + 1);
}

function executeCommand(client, args, fromExec = false) {
  const command = asString(args[0]).toUpperCase();
  const rest = args.slice(1);

  if (
    client.txQueue &&
    !fromExec &&
    !["MULTI", "EXEC", "DISCARD", "QUIT"].includes(command)
  ) {
    client.txQueue.push(args);
    return simple("QUEUED");
  }

  switch (command) {
    case "PING":
      return rest.length > 0 ? bulk(rest[0]) : simple("PONG");

    case "INFO":
      return bulk("# Server\r\nredis_version:7.0.0\r\nloading:0\r\n");

    case "CLIENT": {
      const subcommand = asString(rest[0]).toUpperCase();
      if (subcommand === "SETINFO") {
        return simple("OK");
      }
      if (subcommand === "SETNAME") {
        client.connectionName = asString(rest[1]);
        return simple("OK");
      }
      if (subcommand === "GETNAME") {
        return bulk(client.connectionName || null);
      }
      if (subcommand === "ID") {
        return integer(client.id);
      }
      if (subcommand === "INFO") {
        return bulk(`id=${client.id} name=${client.connectionName ?? ""}`);
      }
      return errorReply(`ERR unsupported CLIENT subcommand ${subcommand}`);
    }

    case "SELECT":
    case "AUTH":
      return simple("OK");

    case "COMMAND":
      return array([]);

    case "MULTI":
      client.txQueue = [];
      return simple("OK");

    case "DISCARD":
      client.txQueue = null;
      return simple("OK");

    case "EXEC": {
      if (!client.txQueue) {
        return errorReply("ERR EXEC without MULTI");
      }

      const queue = client.txQueue;
      client.txQueue = null;
      const replies = [];
      for (const queued of queue) {
        const reply = executeCommand(client, queued, true);
        replies.push(reply ?? bulk(null));
      }
      return array(replies);
    }

    case "HGETALL": {
      const key = asString(rest[0]);
      const hash = hashes.get(key);
      if (!hash) {
        return array([]);
      }
      const entries = [...hash.entries()].sort(([left], [right]) => left.localeCompare(right));
      const flattened = entries.flatMap(([field, value]) => [bulk(field), bulk(value)]);
      return array(flattened);
    }

    case "HSET": {
      const key = asString(rest[0]);
      const pairs = rest.slice(1);
      if (pairs.length === 0 || pairs.length % 2 !== 0) {
        return errorReply("ERR wrong number of arguments for 'HSET'");
      }
      const hash = getHash(key);
      let added = 0;
      for (let index = 0; index < pairs.length; index += 2) {
        const field = asString(pairs[index]);
        const value = asString(pairs[index + 1]);
        if (!hash.has(field)) {
          added += 1;
        }
        hash.set(field, value);
      }
      return integer(added);
    }

    case "RPUSH": {
      const key = asString(rest[0]);
      const values = rest.slice(1).map((value) => asString(value));
      if (values.length === 0) {
        return errorReply("ERR wrong number of arguments for 'RPUSH'");
      }
      const list = getList(key);
      list.push(...values);
      return integer(list.length);
    }

    case "LRANGE": {
      const key = asString(rest[0]);
      const list = lists.get(key) ?? [];
      const range = normalizeRange(list, rest[1], rest[2]);
      if (range === null) {
        return errorReply("ERR value is not an integer or out of range");
      }
      return array(range.map((value) => bulk(value)));
    }

    case "LTRIM": {
      const key = asString(rest[0]);
      const list = lists.get(key) ?? [];
      const range = normalizeRange(list, rest[1], rest[2]);
      if (range === null) {
        return errorReply("ERR value is not an integer or out of range");
      }
      lists.set(key, [...range]);
      return simple("OK");
    }

    case "SUBSCRIBE": {
      const channels = rest.map((value) => asString(value));
      const frames = [];
      if (channels.length === 0) {
        frames.push(array([bulk("subscribe"), bulk(null), integer(subscriptionCount(client))]));
      } else {
        for (const channel of channels) {
          client.channels.add(channel);
          addSubscription(channelSubscribers, channel, client);
          frames.push(array([bulk("subscribe"), bulk(channel), integer(subscriptionCount(client))]));
        }
      }
      sendFrames(client, frames);
      return null;
    }

    case "UNSUBSCRIBE": {
      const channels = rest.length > 0 ? rest.map((value) => asString(value)) : [...client.channels];
      const frames = [];
      if (channels.length === 0) {
        frames.push(array([bulk("unsubscribe"), bulk(null), integer(subscriptionCount(client))]));
      } else {
        for (const channel of channels) {
          client.channels.delete(channel);
          removeSubscription(channelSubscribers, channel, client);
          frames.push(array([bulk("unsubscribe"), bulk(channel), integer(subscriptionCount(client))]));
        }
      }
      sendFrames(client, frames);
      return null;
    }

    case "PSUBSCRIBE": {
      const patterns = rest.map((value) => asString(value));
      const frames = [];
      if (patterns.length === 0) {
        frames.push(array([bulk("psubscribe"), bulk(null), integer(subscriptionCount(client))]));
      } else {
        for (const pattern of patterns) {
          client.patterns.add(pattern);
          addSubscription(patternSubscribers, pattern, client);
          frames.push(array([bulk("psubscribe"), bulk(pattern), integer(subscriptionCount(client))]));
        }
      }
      sendFrames(client, frames);
      return null;
    }

    case "PUNSUBSCRIBE": {
      const patterns = rest.length > 0 ? rest.map((value) => asString(value)) : [...client.patterns];
      const frames = [];
      if (patterns.length === 0) {
        frames.push(array([bulk("punsubscribe"), bulk(null), integer(subscriptionCount(client))]));
      } else {
        for (const pattern of patterns) {
          client.patterns.delete(pattern);
          removeSubscription(patternSubscribers, pattern, client);
          frames.push(array([bulk("punsubscribe"), bulk(pattern), integer(subscriptionCount(client))]));
        }
      }
      sendFrames(client, frames);
      return null;
    }

    case "PUBLISH": {
      const channel = asString(rest[0]);
      const message = asString(rest[1]);
      let delivered = 0;

      for (const subscriber of channelSubscribers.get(channel) ?? []) {
        sendFrame(subscriber, array([bulk("message"), bulk(channel), bulk(message)]));
        delivered += 1;
      }

      for (const [pattern, subscribers] of patternSubscribers.entries()) {
        if (!globMatcher(pattern).test(channel)) {
          continue;
        }
        for (const subscriber of subscribers) {
          sendFrame(subscriber, array([bulk("pmessage"), bulk(pattern), bulk(channel), bulk(message)]));
          delivered += 1;
        }
      }

      return integer(delivered);
    }

    case "QUIT":
      sendFrame(client, simple("OK"));
      client.socket.end();
      return null;

    default:
      return errorReply(`ERR unknown command \`${command}\``);
  }
}

function cleanupClient(client) {
  for (const channel of client.channels) {
    removeSubscription(channelSubscribers, channel, client);
  }
  for (const pattern of client.patterns) {
    removeSubscription(patternSubscribers, pattern, client);
  }
  client.channels.clear();
  client.patterns.clear();
  clients.delete(client);
}

function processBuffer(client) {
  while (client.buffer.length > 0) {
    let parsed;
    try {
      parsed = parseFrame(client.buffer);
    } catch (error) {
      sendFrame(client, errorReply(`ERR ${error instanceof Error ? error.message : String(error)}`));
      client.socket.destroy();
      return;
    }

    if (!parsed) {
      return;
    }

    client.buffer = client.buffer.subarray(parsed.next);
    if (!Array.isArray(parsed.value)) {
      sendFrame(client, errorReply("ERR protocol error: expected array command"));
      continue;
    }

    const args = parsed.value.map((value) => (value instanceof Error ? value.message : value));
    const command = asString(args[0]).toUpperCase();
    log(`client=${client.id} command=${command}`);

    const reply = executeCommand(client, args);
    if (reply !== null) {
      sendFrame(client, reply);
    }
  }
}

const server = net.createServer((socket) => {
  const client = {
    id: nextClientId,
    socket,
    buffer: Buffer.alloc(0),
    txQueue: null,
    channels: new Set(),
    patterns: new Set(),
    connectionName: null,
  };
  nextClientId += 1;
  clients.add(client);

  log(`client-connected id=${client.id}`);

  socket.on("data", (chunk) => {
    client.buffer = Buffer.concat([client.buffer, chunk]);
    processBuffer(client);
  });

  socket.on("close", () => {
    cleanupClient(client);
    log(`client-closed id=${client.id}`);
  });

  socket.on("error", (error) => {
    log(`client-error id=${client.id} error=${error.message}`);
  });
});

const shutdown = () => {
  log("shutdown requested");
  for (const client of [...clients]) {
    client.socket.destroy();
  }
  server.close(() => process.exit(0));
};

process.on("SIGINT", shutdown);
process.on("SIGTERM", shutdown);

server.listen(PORT, HOST, () => {
  log(`listening on ${HOST}:${PORT}`);
});
NODE

  PATH="$NODE_BIN:$PATH" TASK13_REDIS_HOST="127.0.0.1" TASK13_REDIS_PORT="6379" node "$RUN_DIR/inline-redis.mjs" >"$REDIS_LOG" 2>&1 &
  REDIS_PID="$!"
  REDIS_PROVIDER="inline-resp-fallback"
}

start_docker_redis() {
  docker rm -f "$DOCKER_CONTAINER_NAME" >/dev/null 2>&1 || true
  if ! docker run -d --rm --name "$DOCKER_CONTAINER_NAME" -p 6379:6379 redis:7-alpine >"$REDIS_LOG" 2>&1; then
    return 1
  fi
  DOCKER_REDIS_STARTED=1
  REDIS_PROVIDER="docker:redis:7-alpine"
  return 0
}

ensure_redis() {
  if probe_redis; then
    REDIS_PROVIDER="existing:${REDIS_URL}"
    return 0
  fi

  if [ "$REDIS_URL" != "$DEFAULT_REDIS_URL" ]; then
    finalize_failure "redis" "Redis at $REDIS_URL is unavailable and auto-provision only supports $DEFAULT_REDIS_URL"
  fi

  REDIS_SERVER_BIN="$(command -v redis-server || command -v valkey-server || true)"
  if [ -n "$REDIS_SERVER_BIN" ]; then
    start_local_redis_binary
  elif command -v docker >/dev/null 2>&1; then
    if ! start_docker_redis; then
      echo "[redis] docker provider unavailable, falling back to inline RESP provider"
      start_inline_resp_redis
    fi
  else
    echo "[redis] no binary or docker provider available, falling back to inline RESP provider"
    start_inline_resp_redis
  fi

  local elapsed=0
  while [ "$elapsed" -lt 30 ]; do
    if probe_redis; then
      return 0
    fi
    sleep 1
    elapsed=$((elapsed + 1))
  done

  finalize_failure "redis" "Redis provider '$REDIS_PROVIDER' did not become healthy within 30s"
}

cleanup() {
  if [ -n "$REDIS_SUB_PID" ] && kill -0 "$REDIS_SUB_PID" 2>/dev/null; then
    kill "$REDIS_SUB_PID" 2>/dev/null || true
    wait "$REDIS_SUB_PID" 2>/dev/null || true
  fi

  if [ -n "$SERVER_PID" ] && kill -0 "$SERVER_PID" 2>/dev/null; then
    kill "$SERVER_PID" 2>/dev/null || true
    wait "$SERVER_PID" 2>/dev/null || true
  fi

  if [ -n "$REDIS_PID" ] && kill -0 "$REDIS_PID" 2>/dev/null; then
    kill "$REDIS_PID" 2>/dev/null || true
    wait "$REDIS_PID" 2>/dev/null || true
  fi

  if [ "$DOCKER_REDIS_STARTED" -eq 1 ]; then
    docker rm -f "$DOCKER_CONTAINER_NAME" >/dev/null 2>&1 || true
  fi
}

trap cleanup EXIT

echo "===== $TASK_ID $SCRIPT_TAG ====="
echo "run_label: $RUN_LABEL"
echo "run_id: $RUN_ID"
echo "run_dir: $RUN_DIR"
echo "log_file: $LOG_FILE"

echo ""
CURRENT_STAGE="pre-cleanup"
echo "=== [$TASK_ID][$SCRIPT_TAG][0/7] Pre-cleanup ==="
bash "$ROOT/scripts/stop.sh" >/dev/null 2>&1 || true
pass "pre-cleanup complete"

echo ""
CURRENT_STAGE="redis"
echo "=== [$TASK_ID][$SCRIPT_TAG][1/7] Redis provider ==="
ensure_redis
echo "[redis] provider: $REDIS_PROVIDER"
pass "redis ready"

echo ""
CURRENT_STAGE="schema"
echo "=== [$TASK_ID][$SCRIPT_TAG][2/7] Schema build ==="
if (cd "$ROOT/agent/packages/schema" && PATH="$NODE_BIN:$PATH" npm run build) >>"$REDIS_LOG" 2>&1; then
  pass "schema build"
else
  finalize_failure "schema" "schema build failed; see $REDIS_LOG"
fi

echo ""
CURRENT_STAGE="server"
echo "=== [$TASK_ID][$SCRIPT_TAG][3/7] Server startup ==="
(
  export PATH="$RUST_PATH"
  export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/bong-target}"
  # e2e 只验 tiandao↔server Redis 闭环 — 100 rogue 种群在 CI 单核上吃完
  # brain.rs scorer + navigator/movement/lifecycle 的 per-NPC 开销会把 TPS
  # 从 20 拖到 ~6，tiandao 40×500ms 窗口错过所有 bong:world_state publish。
  # 在此显式 seed=0，功能验证走单元测试（spawn.rs 覆盖 0/10/100 三档）。
  export BONG_ROGUE_SEED_COUNT="${BONG_ROGUE_SEED_COUNT:-0}"
  cd "$ROOT/server"
  cargo run --release
) >"$SERVER_LOG" 2>&1 &
SERVER_PID="$!"

if wait_for_pattern "$SERVER_LOG" "\\[bong\\]\\[world\\] creating overworld test area" 300; then
  pass "server world bootstrap"
else
  finalize_failure "server" "missing world bootstrap anchor in $SERVER_LOG"
fi

if wait_for_pattern "$SERVER_LOG" "\\[bong\\]\\[redis\\] subscribed to bong:agent_command, bong:agent_narrate(, .+)?" 300; then
  pass "server redis subscribed"
else
  finalize_failure "server" "missing redis subscribed anchor in $SERVER_LOG"
fi

echo ""
CURRENT_STAGE="proof"
echo "=== [$TASK_ID][$SCRIPT_TAG][4/7] Redis channel proof subscriber ==="
start_redis_subscriber
if wait_for_pattern "$REDIS_SUB_LOG" "\\[task-13\\]\\[redis-sub\\] subscribed" 30; then
  pass "redis subscriber ready"
else
  finalize_failure "proof" "redis subscriber did not start; see $REDIS_SUB_LOG"
fi

echo ""
CURRENT_STAGE="tiandao"
echo "=== [$TASK_ID][$SCRIPT_TAG][5/7] Non-mock Tiandao one-tick closure ==="
(
  cd "$RUN_DIR"
  PATH="$NODE_BIN:$PATH" REDIS_URL="$REDIS_URL" npx tsx "$ROOT/agent/packages/tiandao/src/task-13-one-tick.ts"
) >"$TIANDAO_LOG" 2>&1

if wait_for_pattern "$TIANDAO_LOG" "\\[tiandao\\] connected to Redis at" 60; then
  pass "tiandao connected"
else
  finalize_failure "tiandao" "Tiandao never connected to Redis; see $TIANDAO_LOG"
fi

if wait_for_pattern "$TIANDAO_LOG" "\\[tiandao\\] === tick end === commands:" 60; then
  pass "tick end proof"
else
  finalize_failure "tiandao" "missing tick end anchor in $TIANDAO_LOG"
fi

if wait_for_pattern "$TIANDAO_LOG" "\\[redis-ipc\\] published [0-9]+ commands to bong:agent_command" 60; then
  pass "merged command proof"
else
  finalize_failure "tiandao" "missing merged command anchor in $TIANDAO_LOG"
fi

echo ""
CURRENT_STAGE="anchors"
echo "=== [$TASK_ID][$SCRIPT_TAG][6/7] Cross-process anchors ==="
if wait_for_pattern "$REDIS_SUB_LOG" "channel=bong:world_state" 45; then
  pass "world_state proof"
else
  finalize_failure "anchors" "missing world_state proof in $REDIS_SUB_LOG"
fi

if wait_for_pattern "$REDIS_SUB_LOG" "channel=bong:agent_command" 45; then
  pass "agent_command proof"
else
  finalize_failure "anchors" "missing agent_command proof in $REDIS_SUB_LOG"
fi

if wait_for_pattern "$REDIS_SUB_LOG" "channel=bong:agent_narrate" 45; then
  pass "agent_narrate proof"
else
  finalize_failure "anchors" "missing agent_narrate proof in $REDIS_SUB_LOG"
fi

if wait_for_pattern "$SERVER_LOG" "\\[bong\\]\\[network\\] command_anchor stage=end" 45; then
  pass "server execution anchor"
else
  finalize_failure "anchors" "missing server execution anchor in $SERVER_LOG"
fi

if wait_for_pattern "$TIANDAO_LOG" "\\[redis-ipc\\] published [0-9]+ narrations to bong:agent_narrate" 45; then
  pass "typed narration proof"
else
  finalize_failure "anchors" "missing typed narration anchor in $TIANDAO_LOG"
fi

CURRENT_STAGE="summary"
echo ""
echo "=== [$TASK_ID][$SCRIPT_TAG] Evidence paths ==="
echo "  log: $LOG_FILE"
echo "  error: $ERROR_FILE"
echo "  manifest: $MANIFEST_FILE"
echo "  run_dir: $RUN_DIR"
echo "  redis: $REDIS_LOG"
echo "  server: $SERVER_LOG"
echo "  redis-sub: $REDIS_SUB_LOG"
echo "  tiandao: $TIANDAO_LOG"

echo ""
echo "=== [$TASK_ID][$SCRIPT_TAG] Result ==="
echo "Result: $PASS passed, $FAIL failed"

if [ "$FAIL" -eq 0 ]; then
  printf "task=%s\nstatus=PASS\nrun_id=%s\nmessage=all-anchors-passed\n" "$TASK_ID" "$RUN_ID" >"$SUCCESS_FILE"
  write_manifest "PASS" "complete" "all-anchors-passed"
  echo "ALL PASS"
  exit 0
fi

finalize_failure "$CURRENT_STAGE" "unexpected failure state"
