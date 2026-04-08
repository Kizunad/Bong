/**
 * 导出所有 schema 为 JSON Schema 文件。
 * 用途：Rust 侧可选用 jsonschema crate 做运行时校验，或纯参考对齐。
 *
 * Usage: npx tsx src/generate.ts
 */
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { AgentCommandV1 } from "./agent-command.js";
import { ChatMessageV1, ChatSignal } from "./chat-message.js";
import {
  ClientPayloadV1,
  ClientNarrationPayloadV1,
  EventAlertPayloadV1,
  HeartbeatPayloadV1,
  PlayerStatePayloadV1,
  WelcomePayloadV1,
  ZoneInfoPayloadV1,
} from "./client-payload.js";
import { NarrationV1 } from "./narration.js";
import { WorldStateV1 } from "./world-state.js";

const __dirname = dirname(fileURLToPath(import.meta.url));
const outDir = join(__dirname, "..", "generated");
mkdirSync(outDir, { recursive: true });

const schemas = {
  "world-state-v1": WorldStateV1,
  "agent-command-v1": AgentCommandV1,
  "narration-v1": NarrationV1,
  "chat-message-v1": ChatMessageV1,
  "chat-signal": ChatSignal,
  "client-payload-v1": ClientPayloadV1,
  "client-payload-welcome-v1": WelcomePayloadV1,
  "client-payload-heartbeat-v1": HeartbeatPayloadV1,
  "client-payload-narration-v1": ClientNarrationPayloadV1,
  "client-payload-zone-info-v1": ZoneInfoPayloadV1,
  "client-payload-event-alert-v1": EventAlertPayloadV1,
  "client-payload-player-state-v1": PlayerStatePayloadV1,
};

for (const [name, schema] of Object.entries(schemas)) {
  const path = join(outDir, `${name}.json`);
  writeFileSync(path, `${JSON.stringify(schema, null, 2)}\n`);
  console.log(`wrote ${path}`);
}

console.log(`\n${Object.keys(schemas).length} schemas exported to ${outDir}`);
