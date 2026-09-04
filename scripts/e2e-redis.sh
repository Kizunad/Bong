#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
source "$ROOT/scripts/lib/bong-server-lifecycle.sh"
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
FALLBACK_WORLD_READY_PATTERN='\[bong\]\[world\] BOT_FALLBACK_FLAT_READY anchors=[1-9][0-9]* chunks=[1-9][0-9]* view_distance_chunks=[1-9][0-9]*'
# CI can have a slower SQLite/Redis shutdown path after the 100-NPC proof. Keep
# the default lifecycle helper contract at 10s, but give this disposable E2E
# transaction a bounded 30s graceful window before identity-safe KILL fallback.
E2E_SERVER_STOP_GRACE_SECONDS="${BONG_E2E_SERVER_STOP_GRACE_SECONDS:-30}"
E2E_SERVER_STOP_KILL_GRACE_SECONDS="${BONG_E2E_SERVER_STOP_KILL_GRACE_SECONDS:-2}"
TIANDAO_TIMEOUT_SECONDS="${BONG_E2E_TIANDAO_TIMEOUT_SECONDS:-120}"
TIANDAO_KILL_GRACE_SECONDS="${BONG_E2E_TIANDAO_KILL_GRACE_SECONDS:-5}"

REDIS_LOG="$RUN_DIR/redis.log"
SERVER_LOG="$RUN_DIR/server.log"
REDIS_SUB_LOG="$RUN_DIR/redis-sub.log"
TIANDAO_LOG="$RUN_DIR/tiandao.log"
NORTH_RIFT_SERVER_LOG="$RUN_DIR/north-rift-preview-server.log"
NORTH_RIFT_BOT_LOG="$RUN_DIR/north-rift-preview-bot.log"

PASS=0
FAIL=0
CURRENT_STAGE="init"
REDIS_PID=""
SERVER_PID=""
SERVER_PGID=""
SERVER_OWNER_STARTTIME=""
SERVER_OWNER_EXECUTABLE_IDENTITY=""
SERVER_AUTHORITY_UNCERTAIN=0
SERVER_STARTUP_CONTROL_FD=""
SERVER_STARTUP_READY_FD=""
NORTH_RIFT_DB_STASH=""
PERSISTENCE_TRANSACTION_ACTIVE=0
PERSISTENCE_STASH_READY=0
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
  printf "task=%s\nscript=%s\nrun_id=%s\nrun_label=%s\nstatus=%s\nstage=%s\nmessage=%s\ntimestamp=%s\nfiles:\n- %s\n- %s\n- %s\n- %s\n- %s\n- %s\n- %s\n- %s\n- %s\n- %s\n" \
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
    "$TIANDAO_LOG" \
    "$NORTH_RIFT_SERVER_LOG" \
    "$NORTH_RIFT_BOT_LOG" >"$MANIFEST_FILE"
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
    PATH="$NODE_BIN:$PATH" REDIS_URL="$REDIS_URL" \
      timeout --signal=TERM --kill-after=2s 10s \
      node --input-type=module <<'NODE'
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
  const preview = message.length > 256 ? `${message.slice(0, 256)}...` : message;
  console.log(`[task-13][redis-sub] channel=${channel} bytes=${Buffer.byteLength(message)} payload_preview=${preview}`);
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

# The production helper lives in the lifecycle library so its child-enumeration
# fail-closed contract is executable-testable without running the full e2e.
kill_tree() {
  bong_server_kill_tree "$@"
}

port_open() {
  bong_server_port_is_open "$@"
}

resolve_server_cargo_target() {
  bong_scoped_cargo_target "$1"
}

start_server_process_group() {
  local log_file="$1" preview_mode="$2" test_override_mode="${3:-0}"
  local actual_pgid="" artifact_dir="" build_helper="" built_binary="" build_timeout="" cargo_target owner_pid=""
  local owner_starttime="" owner_executable_identity="" supervisor="" build_token="" ready_line="" committed_line=""
  local owner_snapshot="" control_fd="" ready_fd="" cleanup_status=2

  supervisor="$ROOT/scripts/lib/bong-process-group-supervisor.py"
  build_helper="$ROOT/scripts/lib/bong-pre-handshake-build.py"
  build_token="$ROOT/scripts/build-token.sh"
  local server_directory="$ROOT/server"
  # Only the in-repo supervisor protocol fixture may opt into replacement binaries.
  if [ "${BONG_E2E_SUPERVISOR_TEST_MODE:-0}" = "1" ]; then
    [ "$test_override_mode" = "1" ] || {
      echo "FAIL: e2e supervisor test overrides require an explicit harness mode" >&2
      return 2
    }
    supervisor="${BONG_E2E_SUPERVISOR:-$supervisor}"
    build_token="${BONG_E2E_BUILD_TOKEN:-$build_token}"
    server_directory="${BONG_E2E_SERVER_DIRECTORY:-$server_directory}"
  elif [ -n "${BONG_E2E_SUPERVISOR:-}${BONG_E2E_BUILD_TOKEN:-}${BONG_E2E_SERVER_DIRECTORY:-}" ]; then
    echo "FAIL: e2e supervisor overrides require BONG_E2E_SUPERVISOR_TEST_MODE=1" >&2
    return 2
  fi
  SERVER_PID=""
  SERVER_PGID=""
  SERVER_OWNER_STARTTIME=""
  SERVER_OWNER_EXECUTABLE_IDENTITY=""
  SERVER_STARTUP_CONTROL_FD=""
  SERVER_STARTUP_READY_FD=""
  # No process authority exists during the isolated build phase. This keeps a
  # preview persistence stash recoverable when compilation fails or times out.
  SERVER_AUTHORITY_UNCERTAIN=0

  server_directory="$(readlink -f -- "$server_directory")" || {
    echo "FAIL: server directory does not resolve to a real directory" >&2
    return 2
  }
  [ -d "$server_directory" ] || {
    echo "FAIL: server directory is not a directory: $server_directory" >&2
    return 2
  }
  cargo_target="$(resolve_server_cargo_target "$server_directory")" || {
    echo "FAIL: CARGO_TARGET_DIR could not be resolved" >&2
    return 2
  }
  build_timeout="${BONG_E2E_BUILD_TIMEOUT_SECONDS:-600}"
  [[ "$build_timeout" =~ ^[1-9][0-9]*$ ]] || {
    echo "FAIL: BONG_E2E_BUILD_TIMEOUT_SECONDS must be a positive integer" >&2
    return 2
  }
  artifact_dir="$(mktemp -d "${TMPDIR:-/tmp}/bong-e2e-prebuilt.XXXXXXXX")" || return 1
  chmod 700 -- "$artifact_dir"
  built_binary="$artifact_dir/bong-server"
  if ! env \
    PATH="$RUST_PATH" \
    python3 "$build_helper" \
      "$server_directory" "$cargo_target" "$build_token" "$build_timeout" "$built_binary" \
      >>"$log_file" 2>&1; then
    rm -f -- "$built_binary"
    rmdir -- "$artifact_dir" 2>/dev/null || true
    echo "FAIL: release server build failed or exceeded ${build_timeout}s" >&2
    return 1
  fi
  [ -f "$built_binary" ] || {
    rm -f -- "$built_binary"
    rmdir -- "$artifact_dir" 2>/dev/null || true
    echo "FAIL: successful release build did not produce $built_binary" >&2
    return 1
  }

  # From coproc creation until COMMITTED, startup may own an unpublishable
  # process group. Fail closed until the complete pinned authority is committed.
  SERVER_AUTHORITY_UNCERTAIN=1
  coproc BONG_SERVER_SUPERVISOR {
    exec env \
      PATH="$RUST_PATH" \
      CARGO_TARGET_DIR="$cargo_target" \
      BONG_ROGUE_SEED_COUNT="$([ "$preview_mode" -eq 1 ] && printf '0' || printf '%s' "${BONG_ROGUE_SEED_COUNT:-100}")" \
      BONG_SKIP_SKIN_PREFETCH="${BONG_SKIP_SKIN_PREFETCH:-1}" \
      BONG_PREVIEW_MODE="$preview_mode" \
      python3 "$supervisor" "$server_directory" "$built_binary" \
      2>>"$log_file"
  }
  owner_pid=""
  ready_fd="${BONG_SERVER_SUPERVISOR[0]}"
  control_fd="${BONG_SERVER_SUPERVISOR[1]}"
  SERVER_STARTUP_READY_FD="$ready_fd"
  SERVER_STARTUP_CONTROL_FD="$control_fd"

  if ! IFS= read -r -t 5 -u "$ready_fd" ready_line \
    || [[ "$ready_line" != 'READY pid='[0-9]* ]]; then
    rm -f -- "$built_binary"
    rmdir -- "$artifact_dir" 2>/dev/null || true
    exec {control_fd}>&-
    exec {ready_fd}<&-
    # Do not wait on an unpinned startup PID: it can outlive a failed protocol
    # transaction and is never authority. The supervisor receives EOF and rolls
    # its own private group back.
    SERVER_STARTUP_CONTROL_FD=""
    SERVER_STARTUP_READY_FD=""
    echo "FAIL: server supervisor did not publish startup rollback readiness" >&2
    return 1
  fi
  # READY means the supervisor has copied the token-pinned private artifact.
  rm -f -- "$built_binary"
  rmdir -- "$artifact_dir" 2>/dev/null || true
  # READY is emitted by the post-setsid supervisor itself and carries that exact
  # PID, avoiding Bash coproc wrapper ambiguity. The following identity snapshot
  # still pins starttime, executable inode, and PGID before C is sent.
  owner_pid="${ready_line#READY pid=}"
  [[ "$owner_pid" =~ ^[0-9]+$ ]] || {
    exec {control_fd}>&-
    exec {ready_fd}<&-
    SERVER_STARTUP_CONTROL_FD=""
    SERVER_STARTUP_READY_FD=""
    echo "FAIL: server supervisor readiness line carried an invalid owner PID" >&2
    return 1
  }

  for _ in $(seq 1 500); do
    if bong_server_process_is_running "$owner_pid"; then
      actual_pgid="$(ps -o pgid= -p "$owner_pid" 2>/dev/null || true)"
      actual_pgid="${actual_pgid//[[:space:]]/}"
      if [ "$actual_pgid" = "$owner_pid" ]; then
        owner_snapshot="$(bong_server_process_starttime_and_group "$owner_pid" 2>/dev/null || true)"
        read -r owner_starttime actual_pgid <<< "$owner_snapshot"
        owner_executable_identity="$(
          bong_server_process_executable_identity "$owner_pid" 2>/dev/null || true
        )"
        if [ "$actual_pgid" = "$owner_pid" ] \
          && [[ "$owner_starttime" =~ ^[0-9]+$ ]] \
          && [[ "$owner_executable_identity" =~ ^[0-9]+:[0-9]+$ ]]; then
          if ! printf C >&"$control_fd"; then
            break
          fi
          # C is one-way control. Close the write end immediately so no process
          # can mistake a still-open control channel for uncommitted authority.
          exec {control_fd}>&-
          control_fd=""
          SERVER_STARTUP_CONTROL_FD=""
          if [ -n "${BONG_E2E_TEST_AFTER_COMMIT_WRITE_HOOK:-}" ]; then
            "$BONG_E2E_TEST_AFTER_COMMIT_WRITE_HOOK" "$owner_pid"
          fi
          if IFS= read -r -t 5 -u "$ready_fd" committed_line \
            && [ "$committed_line" = COMMITTED ]; then
            if [ -n "${BONG_E2E_TEST_AFTER_ACK_HOOK:-}" ]; then
              "$BONG_E2E_TEST_AFTER_ACK_HOOK" "$owner_pid"
            fi
            if bong_server_pinned_process_group_status \
              "$owner_pid" "$owner_starttime" "$owner_executable_identity" "$actual_pgid"; then
              exec {ready_fd}<&-
              SERVER_STARTUP_READY_FD=""
              SERVER_PID="$owner_pid"
              SERVER_PGID="$actual_pgid"
              SERVER_OWNER_STARTTIME="$owner_starttime"
              SERVER_OWNER_EXECUTABLE_IDENTITY="$owner_executable_identity"
              SERVER_AUTHORITY_UNCERTAIN=0
              return 0
            fi
          fi
          break
        fi
      fi
    else
      break
    fi
    sleep 0.01
  done

  # Never publish partial authority. If the full pre-C candidate still pins, its
  # owner-bound stop helper may clean it. A changed/dead/uninspectable candidate
  # is deliberately left for diagnosis: numeric PGID teardown would be unsafe.
  [ -n "$control_fd" ] && exec {control_fd}>&-
  exec {ready_fd}<&-
  SERVER_STARTUP_CONTROL_FD=""
  SERVER_STARTUP_READY_FD=""
  if [ -n "$owner_starttime" ] && [ -n "$owner_executable_identity" ] \
    && [ -n "$actual_pgid" ] \
    && bong_server_pinned_process_group_status \
      "$owner_pid" "$owner_starttime" "$owner_executable_identity" "$actual_pgid"; then
    if bong_server_stop_owned_process_group_and_release_port \
      "$owner_pid" "$owner_starttime" "$owner_executable_identity" "$actual_pgid" 25565; then
      cleanup_status=0
    else
      cleanup_status=$?
    fi
  fi
  # A bounded wait reaps a normal rollback/cleanup owner without accidentally
  # turning an unpinned PID into authority. Never wait indefinitely here.
  if [ "$cleanup_status" -eq 0 ]; then
    echo "FAIL: server supervisor commit acknowledgement failed; pinned rollback completed" >&2
  else
    echo "FAIL: server supervisor commit acknowledgement failed; authority was not published" >&2
  fi
  return 1
}

stop_server() {
  local pid="$SERVER_PID" pgid="$SERVER_PGID"
  local owner_starttime="$SERVER_OWNER_STARTTIME"
  local owner_executable_identity="$SERVER_OWNER_EXECUTABLE_IDENTITY"
  local stop_status

  if [ "$SERVER_AUTHORITY_UNCERTAIN" -ne 0 ]; then
    echo "FAIL: server process-group authority is uncertain; refusing teardown or restore" >&2
    return 1
  fi

  if [ -n "$pid" ] || [ -n "$pgid" ] \
    || [ -n "$owner_starttime" ] || [ -n "$owner_executable_identity" ]; then
    if [ -z "$pid" ] || [ -z "$pgid" ] \
      || [ -z "$owner_starttime" ] || [ -z "$owner_executable_identity" ]; then
      echo "FAIL: incomplete server process-group authority (pid=${pid:-missing}, pgid=${pgid:-missing})" >&2
      return 1
    fi
    local graceful_stop_seconds="${E2E_SERVER_STOP_GRACE_SECONDS:-30}"
    local kill_stop_seconds="${E2E_SERVER_STOP_KILL_GRACE_SECONDS:-2}"
    if bong_server_stop_owned_process_group_and_release_port \
        "$pid" "$owner_starttime" "$owner_executable_identity" "$pgid" 25565 \
        "$graceful_stop_seconds" "$kill_stop_seconds"; then
      SERVER_PID=""
      SERVER_PGID=""
      SERVER_OWNER_STARTTIME=""
      SERVER_OWNER_EXECUTABLE_IDENTITY=""
      SERVER_AUTHORITY_UNCERTAIN=0
      return 0
    else
      stop_status=$?
    fi
    echo "FAIL: server process group stop did not complete (status=$stop_status, graceful=${graceful_stop_seconds}s, kill=${kill_stop_seconds}s)" >&2
    return "$stop_status"
  fi
  # Outside a READY transaction there are no stashed developer bytes to expose:
  # an empty PID remains an ordinary no-op. Restore authorization is stricter and
  # requires fresh shared-port evidence for this cleanup invocation.
  if [ "$PERSISTENCE_STASH_READY" -eq 1 ]; then
    bong_server_confirm_port_released 25565
    return $?
  fi
  return 0
}

cleanup() {
  if [ -n "$REDIS_SUB_PID" ] && kill -0 "$REDIS_SUB_PID" 2>/dev/null; then
    kill_tree "$REDIS_SUB_PID"
    wait "$REDIS_SUB_PID" 2>/dev/null || true
  fi

  STOP_SERVER_CONFIRMED=0
  if stop_server; then
    STOP_SERVER_CONFIRMED=1
  else
    echo "FAIL: preview server did not stop/release port; persistence restore is forbidden" >&2
  fi

  # 持久化 transaction 覆盖 stash → 专用 preview 停服 → restore 的整段。
  # cleanup 绝不能先解锁：必须先停服，再还原；还原/完成失败则留下 durable
  # handoff marker，之后的 e2e 会 fail closed 而不是覆盖开发者存档。
  if [ "$PERSISTENCE_TRANSACTION_ACTIVE" -eq 1 ]; then
    if [ "$PERSISTENCE_STASH_READY" -eq 1 ]; then
      if bong_server_finalize_preview_persistence_after_stop \
        "$ROOT/server/data" "$NORTH_RIFT_DB_STASH" "$STOP_SERVER_CONFIRMED"; then
        PERSISTENCE_STASH_READY=0
      fi
    else
      # No stash path was committed: pre-manifest/stale failure, safe to clear.
      bong_server_persistence_transaction_complete || bong_server_persistence_transaction_release
    fi
    PERSISTENCE_TRANSACTION_ACTIVE=0
  fi

  if [ -n "$REDIS_PID" ] && kill -0 "$REDIS_PID" 2>/dev/null; then
    kill_tree "$REDIS_PID"
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
echo "=== [$TASK_ID][$SCRIPT_TAG][0/8] Pre-cleanup ==="
bash "$ROOT/scripts/stop.sh" >/dev/null 2>&1 || true
pass "pre-cleanup complete"

echo ""
CURRENT_STAGE="redis"
echo "=== [$TASK_ID][$SCRIPT_TAG][1/8] Redis provider ==="
ensure_redis
echo "[redis] provider: $REDIS_PROVIDER"
pass "redis ready"

echo ""
CURRENT_STAGE="schema"
echo "=== [$TASK_ID][$SCRIPT_TAG][2/8] Schema build ==="
if (
  cd "$ROOT/agent/packages/schema"
  PATH="$NODE_BIN:$PATH" timeout --signal=TERM --kill-after=5s 300s npm run build
) >>"$REDIS_LOG" 2>&1; then
  pass "schema build"
else
  finalize_failure "schema" "schema build failed; see $REDIS_LOG"
fi

echo ""
CURRENT_STAGE="server"
echo "=== [$TASK_ID][$SCRIPT_TAG][3/8] Server startup ==="
if ! start_server_process_group "$SERVER_LOG" 0; then
  finalize_failure "server" "failed to establish dedicated server process group; see $SERVER_LOG"
fi

if wait_for_pattern "$SERVER_LOG" "$FALLBACK_WORLD_READY_PATTERN" 300; then
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
echo "=== [$TASK_ID][$SCRIPT_TAG][4/8] Redis channel proof subscriber ==="
start_redis_subscriber
if wait_for_pattern "$REDIS_SUB_LOG" "\\[task-13\\]\\[redis-sub\\] subscribed" 30; then
  pass "redis subscriber ready"
else
  finalize_failure "proof" "redis subscriber did not start; see $REDIS_SUB_LOG"
fi

echo ""
CURRENT_STAGE="tiandao"
echo "=== [$TASK_ID][$SCRIPT_TAG][5/8] Non-mock Tiandao one-tick closure ==="
if ! [[ "$TIANDAO_TIMEOUT_SECONDS" =~ ^[1-9][0-9]*$ ]]; then
  finalize_failure \
    "tiandao" \
    "BONG_E2E_TIANDAO_TIMEOUT_SECONDS must be a positive integer; log=$TIANDAO_LOG; run_dir=$RUN_DIR"
fi
if ! [[ "$TIANDAO_KILL_GRACE_SECONDS" =~ ^[1-9][0-9]*$ ]]; then
  finalize_failure \
    "tiandao" \
    "BONG_E2E_TIANDAO_KILL_GRACE_SECONDS must be a positive integer; log=$TIANDAO_LOG; run_dir=$RUN_DIR"
fi
if ! command -v timeout >/dev/null 2>&1; then
  finalize_failure \
    "tiandao" \
    "required timeout command is unavailable; log=$TIANDAO_LOG; run_dir=$RUN_DIR"
fi
TIANDAO_TSX="$NODE_BIN/tsx"
if [ ! -x "$TIANDAO_TSX" ]; then
  finalize_failure \
    "tiandao" \
    "workspace tsx executable is missing: $TIANDAO_TSX; run npm ci in $ROOT/agent; log=$TIANDAO_LOG; run_dir=$RUN_DIR"
fi
TIANDAO_STARTED_AT="$(date +%s)"
TIANDAO_EXIT=0
if (
  cd "$RUN_DIR"
  PATH="$NODE_BIN:$PATH" REDIS_URL="$REDIS_URL" \
    timeout \
    --signal=TERM \
    --kill-after="${TIANDAO_KILL_GRACE_SECONDS}s" \
    "${TIANDAO_TIMEOUT_SECONDS}s" \
    "$TIANDAO_TSX" "$ROOT/agent/packages/tiandao/src/task-13-one-tick.ts"
) >"$TIANDAO_LOG" 2>&1
then
  TIANDAO_EXIT=0
else
  TIANDAO_EXIT=$?
fi
TIANDAO_ELAPSED_SECONDS=$(( $(date +%s) - TIANDAO_STARTED_AT ))
echo "[tiandao] command=$TIANDAO_TSX exit=$TIANDAO_EXIT elapsed=${TIANDAO_ELAPSED_SECONDS}s timeout=${TIANDAO_TIMEOUT_SECONDS}s log=$TIANDAO_LOG run_dir=$RUN_DIR"

if [ "$TIANDAO_EXIT" -eq 124 ] || [ "$TIANDAO_EXIT" -eq 137 ]; then
  finalize_failure \
    "tiandao" \
    "Non-mock Tiandao timed out (exit=$TIANDAO_EXIT, elapsed=${TIANDAO_ELAPSED_SECONDS}s, limit=${TIANDAO_TIMEOUT_SECONDS}s); command=$TIANDAO_TSX; log=$TIANDAO_LOG; run_dir=$RUN_DIR"
fi
if [ "$TIANDAO_EXIT" -ne 0 ]; then
  finalize_failure \
    "tiandao" \
    "Non-mock Tiandao exited with code $TIANDAO_EXIT after ${TIANDAO_ELAPSED_SECONDS}s; command=$TIANDAO_TSX; log=$TIANDAO_LOG; run_dir=$RUN_DIR"
fi

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
echo "=== [$TASK_ID][$SCRIPT_TAG][6/8] Cross-process anchors ==="
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

TPS_LINE="$(grep -E "actual TPS = [0-9]+([.][0-9]+)?" "$SERVER_LOG" | tail -n 1 || true)"
TPS_VALUE="$(printf '%s\n' "$TPS_LINE" | sed -nE 's/.*actual TPS = ([0-9]+([.][0-9]+)?).*/\1/p')"
if [ -n "$TPS_VALUE" ] && awk -v tps="$TPS_VALUE" 'BEGIN { exit !(tps >= 15.0) }'; then
  pass "100 NPC TPS gate (${TPS_VALUE} >= 15)"
else
  finalize_failure "anchors" "100 NPC TPS gate failed: ${TPS_LINE:-missing actual TPS line}; see $SERVER_LOG"
fi

if wait_for_pattern "$TIANDAO_LOG" "\\[redis-ipc\\] published [0-9]+ narrations to bong:agent_narrate" 45; then
  pass "typed narration proof"
else
  finalize_failure "anchors" "missing typed narration anchor in $TIANDAO_LOG"
fi

echo ""
CURRENT_STAGE="north-rift-preview"
echo "=== [$TASK_ID][$SCRIPT_TAG][7/8] North-rift dedicated preview bot ==="
# `/preview_tp` 的 consumer 只在 BONG_PREVIEW_MODE=1 注册，同时会把已加入
# client 的 ViewDistance 提到 32。绝不能把该 env 塞进常规 bot --all server：
# 先在上面的普通 release server 完成 100 NPC TPS gate，再完整停服；这里只另起
# 一个无 rogue seed 的专用 release server，运行唯一 north-rift bot 后立即清理。
run_north_rift_preview() {
  # The lifecycle lock spans ordinary-server stop through persistence restore.
  # Production start/dev-reload use the same lock, so neither can open
  # server/data while the preview transaction has moved its SQLite snapshot.
  if ! stop_server; then
    finalize_failure "north-rift-preview" "ordinary server stopped but port 25565 stayed occupied"
  fi

  # Recheck under the lifecycle lock immediately before transaction begin. A
  # listener with no PID authority is unsafe and must leave developer data intact.
  if ! bong_server_confirm_port_released 25565; then
    finalize_failure "north-rift-preview" "port 25565 is occupied before persistence stash; refusing to move live SQLite files"
  fi

  # 优雅关服（SIGTERM → AppExit → Last）现在真正可达，上面 stop_server 会让
  # 普通 e2e server 在退出前把运行期 zone 快照（被 100 NPC seed 消耗过的
  # spirit_qi）刷进 server/data/bong.db。但下面这台专用 preview server 与
  # 普通 server 共用同一个相对 cwd 持久化路径，而 terrain_north_rift_scorch_
  # zone_identity 场景断言的是 zones.json 的 pristine 权威身份数值——必须先
  # 把开发者本地真实存档挪走，让专用 preview server 从干净持久化状态启动，
  # 场景通过 / 脚本退出后再原样还原，不能影响本机开发者的真实存档。
  NORTH_RIFT_DB_STASH="$RUN_DIR/north-rift-db-stash"
  if ! bong_server_persistence_transaction_begin "$ROOT/server/data"; then
    finalize_failure "north-rift-preview" "failed to acquire exclusive server/data persistence transaction (or an unrecovered handoff exists)"
  fi
  PERSISTENCE_TRANSACTION_ACTIVE=1
  if ! bong_server_stash_persistence "$ROOT/server/data" "$NORTH_RIFT_DB_STASH"; then
    # Helper creates the durable stash-path marker only after V3 manifest publish
    # and validation, before its first move. A pre-publish/stale failure remains
    # unready and cleanup only clears ACTIVE without touching that leaf.
    if [ "${BONG_SERVER_PERSISTENCE_STASH_READY:-0}" -eq 1 ]; then
      PERSISTENCE_STASH_READY=1
    fi
    finalize_failure "north-rift-preview" "failed to atomically publish and stash local server/data/bong.db before dedicated preview server"
  fi
  PERSISTENCE_STASH_READY=1

  NORTH_RIFT_RUN_TAG="nr$(( $$ % 1000 ))"
  NORTH_RIFT_OPERATOR="B${NORTH_RIFT_RUN_TAG}NRift"
  export BONG_OPERATORS="$NORTH_RIFT_OPERATOR"
  export BONG_OPERATORS_ALLOW_OFFLINE=1
  if ! start_server_process_group "$NORTH_RIFT_SERVER_LOG" 1; then
    unset BONG_OPERATORS BONG_OPERATORS_ALLOW_OFFLINE
    finalize_failure \
      "north-rift-preview" \
      "failed to establish dedicated preview server process group; see $NORTH_RIFT_SERVER_LOG"
  fi
  unset BONG_OPERATORS BONG_OPERATORS_ALLOW_OFFLINE

  if ! wait_for_pattern "$NORTH_RIFT_SERVER_LOG" "\\[bong\\]\\[preview\\] BONG_PREVIEW_MODE=1" 300; then
    finalize_failure \
      "north-rift-preview" \
      "dedicated server did not activate preview mode; see $NORTH_RIFT_SERVER_LOG"
  fi
  if ! wait_for_pattern "$NORTH_RIFT_SERVER_LOG" "$FALLBACK_WORLD_READY_PATTERN" 300; then
    finalize_failure \
      "north-rift-preview" \
      "dedicated preview server missed world bootstrap; see $NORTH_RIFT_SERVER_LOG"
  fi
  NORTH_RIFT_PORT_READY=0
  NORTH_RIFT_LISTENER_INSPECTION_FAILED=0
  for _ in $(seq 1 50); do
    if bong_server_owned_process_group_owns_ipv4_listener \
        "$SERVER_PID" "$SERVER_OWNER_STARTTIME" \
        "$SERVER_OWNER_EXECUTABLE_IDENTITY" "$SERVER_PGID" 25565; then
      listener_status=0
    else
      listener_status=$?
    fi
    if [ "$listener_status" -ne 0 ] && [ "$listener_status" -ne 1 ]; then
      NORTH_RIFT_LISTENER_INSPECTION_FAILED=1
      break
    fi
    if [ "$listener_status" -eq 0 ] && port_open 25565; then
      if bong_server_owned_process_group_owns_ipv4_listener \
          "$SERVER_PID" "$SERVER_OWNER_STARTTIME" \
          "$SERVER_OWNER_EXECUTABLE_IDENTITY" "$SERVER_PGID" 25565; then
        NORTH_RIFT_PORT_READY=1
        break
      else
        listener_status=$?
      fi
      if [ "$listener_status" -ne 1 ]; then
        NORTH_RIFT_LISTENER_INSPECTION_FAILED=1
        break
      fi
    fi
    if ! bong_server_pinned_process_group_status \
        "$SERVER_PID" "$SERVER_OWNER_STARTTIME" \
        "$SERVER_OWNER_EXECUTABLE_IDENTITY" "$SERVER_PGID"; then
      break
    fi
    sleep 0.2
  done
  if [ "$NORTH_RIFT_PORT_READY" -ne 1 ]; then
    if [ "$NORTH_RIFT_LISTENER_INSPECTION_FAILED" -eq 1 ]; then
      listener_failure="dedicated preview server listener ownership became uninspectable"
    else
      listener_failure="dedicated preview server did not prove ownership of port 25565"
    fi
    finalize_failure \
      "north-rift-preview" \
      "$listener_failure; see $NORTH_RIFT_SERVER_LOG"
  fi

  NORTH_RIFT_RUN_TAG="nr$(( $$ % 1000 ))"
  # review finding：run tag 在父 shell 产生后必须进入子进程环境 —— 仅作普通 shell
  # 变量时子进程看不到。放进环境赋值前缀（并保留 --run-tag CLI 直传），run_scenarios.py
  # 无论走 CLI 还是环境默认都能拿到本次 run 的隔离段。
  if BOT_E2E_NORTH_RIFT_PREVIEW=1 \
    NORTH_RIFT_RUN_TAG="$NORTH_RIFT_RUN_TAG" \
    timeout --signal=TERM --kill-after=5s 300s \
    python3 "$ROOT/scripts/bot/run_scenarios.py" \
      --host 127.0.0.1 \
      --port 25565 \
      --run-tag "$NORTH_RIFT_RUN_TAG" \
      --scenario terrain_north_rift_scorch_zone_identity \
      >"$NORTH_RIFT_BOT_LOG" 2>&1; then
    pass "north-rift preview_tp zone_info + ambient_zone bot"
  else
    tail -n 80 "$NORTH_RIFT_BOT_LOG" || true
    tail -n 80 "$NORTH_RIFT_SERVER_LOG" || true
    finalize_failure \
      "north-rift-preview" \
      "dedicated north-rift protocol bot failed; see $NORTH_RIFT_BOT_LOG"
  fi

  if ! stop_server; then
    finalize_failure \
      "north-rift-preview" \
      "dedicated preview bot passed but server did not release port 25565"
  fi

  if ! bong_server_restore_persistence "$ROOT/server/data" "$NORTH_RIFT_DB_STASH"; then
    finalize_failure "north-rift-preview" "failed to restore local server/data/bong.db after dedicated preview server; durable handoff will remain"
  fi
  if ! bong_server_persistence_transaction_complete; then
    finalize_failure "north-rift-preview" "restored local server/data/bong.db but could not clear the durable persistence handoff"
  fi
  PERSISTENCE_STASH_READY=0
  PERSISTENCE_TRANSACTION_ACTIVE=0
  pass "north-rift dedicated preview server cleanup"
}

if ! bong_server_with_preview_persistence_lock run_north_rift_preview; then
  finalize_failure "north-rift-preview" "failed to hold lifecycle exclusion through north-rift preview persistence transaction"
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
echo "  north-rift preview server: $NORTH_RIFT_SERVER_LOG"
echo "  north-rift preview bot: $NORTH_RIFT_BOT_LOG"

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
