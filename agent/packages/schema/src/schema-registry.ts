import type { TSchema } from "@sinclair/typebox";

import { AgentCommandV1 } from "./agent-command.js";
import { ChatMessageV1, ChatSignal } from "./chat-message.js";
import { NarrationV1 } from "./narration.js";
import { ServerDataV1 } from "./server-data.js";
import { WorldStateV1 } from "./world-state.js";

export const SCHEMA_REGISTRY = {
  worldStateV1: WorldStateV1,
  agentCommandV1: AgentCommandV1,
  narrationV1: NarrationV1,
  chatMessageV1: ChatMessageV1,
  chatSignal: ChatSignal,
  serverDataV1: ServerDataV1,
} as const satisfies Record<string, TSchema>;

export const GENERATED_SCHEMA_FILES = {
  "world-state-v1.json": SCHEMA_REGISTRY.worldStateV1,
  "agent-command-v1.json": SCHEMA_REGISTRY.agentCommandV1,
  "narration-v1.json": SCHEMA_REGISTRY.narrationV1,
  "chat-message-v1.json": SCHEMA_REGISTRY.chatMessageV1,
  "chat-signal.json": SCHEMA_REGISTRY.chatSignal,
  "server-data-v1.json": SCHEMA_REGISTRY.serverDataV1,
} as const satisfies Record<string, TSchema>;

export type SchemaRegistryKey = keyof typeof SCHEMA_REGISTRY;
export type GeneratedSchemaFileName = keyof typeof GENERATED_SCHEMA_FILES;
