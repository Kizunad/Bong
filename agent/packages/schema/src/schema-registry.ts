import type { TSchema } from "@sinclair/typebox";

import { AgentCommandV1 } from "./agent-command.js";
import { BiographyEntryV1 } from "./biography.js";
import { BreakthroughEventV1 } from "./breakthrough-event.js";
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
import {
  BreakthroughRequestV1,
  ClientRequestV1,
  ForgeRequestV1,
  InsightDecisionRequestV1,
  SetMeridianTargetRequestV1,
} from "./client-request.js";
import { CultivationDeathV1 } from "./cultivation-death.js";
import { ForgeEventV1 } from "./forge-event.js";
import { InventoryEventV1, InventorySnapshotV1 } from "./inventory.js";
import { InsightOfferV1 } from "./insight-offer.js";
import { InsightRequestV1 } from "./insight-request.js";
import { NarrationV1 } from "./narration.js";
import { ServerDataV1 } from "./server-data.js";
import { WorldStateV1 } from "./world-state.js";

export const SCHEMA_REGISTRY = {
  worldStateV1: WorldStateV1,
  agentCommandV1: AgentCommandV1,
  narrationV1: NarrationV1,
  chatMessageV1: ChatMessageV1,
  chatSignal: ChatSignal,
  inventorySnapshotV1: InventorySnapshotV1,
  inventoryEventV1: InventoryEventV1,
  serverDataV1: ServerDataV1,
  clientPayloadV1: ClientPayloadV1,
  clientPayloadWelcomeV1: WelcomePayloadV1,
  clientPayloadHeartbeatV1: HeartbeatPayloadV1,
  clientPayloadNarrationV1: ClientNarrationPayloadV1,
  clientPayloadZoneInfoV1: ZoneInfoPayloadV1,
  clientPayloadEventAlertV1: EventAlertPayloadV1,
  clientPayloadPlayerStateV1: PlayerStatePayloadV1,
  insightRequestV1: InsightRequestV1,
  insightOfferV1: InsightOfferV1,
  breakthroughEventV1: BreakthroughEventV1,
  forgeEventV1: ForgeEventV1,
  biographyEntryV1: BiographyEntryV1,
  cultivationDeathV1: CultivationDeathV1,
  clientRequestV1: ClientRequestV1,
  clientRequestSetMeridianTargetV1: SetMeridianTargetRequestV1,
  clientRequestBreakthroughV1: BreakthroughRequestV1,
  clientRequestForgeV1: ForgeRequestV1,
  clientRequestInsightDecisionV1: InsightDecisionRequestV1,
} as const satisfies Record<string, TSchema>;

export const GENERATED_SCHEMA_FILES = {
  "world-state-v1.json": SCHEMA_REGISTRY.worldStateV1,
  "agent-command-v1.json": SCHEMA_REGISTRY.agentCommandV1,
  "narration-v1.json": SCHEMA_REGISTRY.narrationV1,
  "chat-message-v1.json": SCHEMA_REGISTRY.chatMessageV1,
  "chat-signal.json": SCHEMA_REGISTRY.chatSignal,
  "inventory-snapshot-v1.json": SCHEMA_REGISTRY.inventorySnapshotV1,
  "inventory-event-v1.json": SCHEMA_REGISTRY.inventoryEventV1,
  "server-data-v1.json": SCHEMA_REGISTRY.serverDataV1,
  "client-payload-v1.json": SCHEMA_REGISTRY.clientPayloadV1,
  "client-payload-welcome-v1.json": SCHEMA_REGISTRY.clientPayloadWelcomeV1,
  "client-payload-heartbeat-v1.json": SCHEMA_REGISTRY.clientPayloadHeartbeatV1,
  "client-payload-narration-v1.json": SCHEMA_REGISTRY.clientPayloadNarrationV1,
  "client-payload-zone-info-v1.json": SCHEMA_REGISTRY.clientPayloadZoneInfoV1,
  "client-payload-event-alert-v1.json": SCHEMA_REGISTRY.clientPayloadEventAlertV1,
  "client-payload-player-state-v1.json": SCHEMA_REGISTRY.clientPayloadPlayerStateV1,
  "insight-request-v1.json": SCHEMA_REGISTRY.insightRequestV1,
  "insight-offer-v1.json": SCHEMA_REGISTRY.insightOfferV1,
  "breakthrough-event-v1.json": SCHEMA_REGISTRY.breakthroughEventV1,
  "forge-event-v1.json": SCHEMA_REGISTRY.forgeEventV1,
  "biography-entry-v1.json": SCHEMA_REGISTRY.biographyEntryV1,
  "cultivation-death-v1.json": SCHEMA_REGISTRY.cultivationDeathV1,
  "client-request-v1.json": SCHEMA_REGISTRY.clientRequestV1,
  "client-request-set-meridian-target-v1.json":
    SCHEMA_REGISTRY.clientRequestSetMeridianTargetV1,
  "client-request-breakthrough-v1.json": SCHEMA_REGISTRY.clientRequestBreakthroughV1,
  "client-request-forge-v1.json": SCHEMA_REGISTRY.clientRequestForgeV1,
  "client-request-insight-decision-v1.json":
    SCHEMA_REGISTRY.clientRequestInsightDecisionV1,
} as const satisfies Record<string, TSchema>;

export type SchemaRegistryKey = keyof typeof SCHEMA_REGISTRY;
export type GeneratedSchemaFileName = keyof typeof GENERATED_SCHEMA_FILES;
