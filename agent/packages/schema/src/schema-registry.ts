import type { TSchema } from "@sinclair/typebox";

import { AgentCommandV1 } from "./agent-command.js";
import { ChatMessageV1, ChatSignal } from "./chat-message.js";
import {
  ClientNarrationPayloadV1,
  ClientPayloadV1,
  EventAlertPayloadV1,
  HeartbeatPayloadV1,
  PlayerStatePayloadV1,
  WelcomePayloadV1,
  ZoneInfoPayloadV1,
} from "./client-payload.js";
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
  clientPayloadV1: ClientPayloadV1,
  clientPayloadWelcomeV1: WelcomePayloadV1,
  clientPayloadHeartbeatV1: HeartbeatPayloadV1,
  clientPayloadNarrationV1: ClientNarrationPayloadV1,
  clientPayloadZoneInfoV1: ZoneInfoPayloadV1,
  clientPayloadEventAlertV1: EventAlertPayloadV1,
  clientPayloadPlayerStateV1: PlayerStatePayloadV1,
} as const satisfies Record<string, TSchema>;

export const GENERATED_SCHEMA_FILES = {
  "world-state-v1.json": SCHEMA_REGISTRY.worldStateV1,
  "agent-command-v1.json": SCHEMA_REGISTRY.agentCommandV1,
  "narration-v1.json": SCHEMA_REGISTRY.narrationV1,
  "chat-message-v1.json": SCHEMA_REGISTRY.chatMessageV1,
  "chat-signal.json": SCHEMA_REGISTRY.chatSignal,
  "server-data-v1.json": SCHEMA_REGISTRY.serverDataV1,
  "client-payload-v1.json": SCHEMA_REGISTRY.clientPayloadV1,
  "client-payload-welcome-v1.json": SCHEMA_REGISTRY.clientPayloadWelcomeV1,
  "client-payload-heartbeat-v1.json": SCHEMA_REGISTRY.clientPayloadHeartbeatV1,
  "client-payload-narration-v1.json": SCHEMA_REGISTRY.clientPayloadNarrationV1,
  "client-payload-zone-info-v1.json": SCHEMA_REGISTRY.clientPayloadZoneInfoV1,
  "client-payload-event-alert-v1.json": SCHEMA_REGISTRY.clientPayloadEventAlertV1,
  "client-payload-player-state-v1.json": SCHEMA_REGISTRY.clientPayloadPlayerStateV1,
} as const satisfies Record<string, TSchema>;

export type SchemaRegistryKey = keyof typeof SCHEMA_REGISTRY;
export type GeneratedSchemaFileName = keyof typeof GENERATED_SCHEMA_FILES;
