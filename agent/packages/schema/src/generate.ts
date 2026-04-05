/**
 * 导出所有 schema 为 JSON Schema 文件。
 * 用途：Rust 侧可选用 jsonschema crate 做运行时校验，或纯参考对齐。
 *
 * Usage: npx tsx src/generate.ts
 */
import { writeFileSync, mkdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

import { WorldStateV1 } from "./world-state.js";
import { AgentCommandV1 } from "./agent-command.js";
import { NarrationV1 } from "./narration.js";
import { ChatMessageV1, ChatSignal } from "./chat-message.js";

const __dirname = dirname(fileURLToPath(import.meta.url));
const outDir = join(__dirname, "..", "generated");
mkdirSync(outDir, { recursive: true });

const schemas = {
  "world-state-v1": WorldStateV1,
  "agent-command-v1": AgentCommandV1,
  "narration-v1": NarrationV1,
  "chat-message-v1": ChatMessageV1,
  "chat-signal": ChatSignal,
};

for (const [name, schema] of Object.entries(schemas)) {
  const path = join(outDir, `${name}.json`);
  writeFileSync(path, JSON.stringify(schema, null, 2) + "\n");
  console.log(`wrote ${path}`);
}

console.log(`\n${Object.keys(schemas).length} schemas exported to ${outDir}`);
