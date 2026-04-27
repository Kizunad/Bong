import type { TSchema } from "@sinclair/typebox";

import { AgentCommandV1 } from "./agent-command.js";
import { AgentWorldModelEnvelopeV1, AgentWorldModelSnapshotV1 } from "./agent-world-model.js";
import { ArmorDurabilityChangedV1 } from "./armor-event.js";
import {
  AlchemyContaminationLevelV1,
  AlchemyInterventionV1,
  AlchemyOutcomeBucket,
  AlchemyRecipeEntryV1,
  AlchemyStageHintV1,
} from "./alchemy.js";
import { BotanyEcologySnapshotV1 } from "./botany.js";
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
  AlchemyFeedSlotRequestV1,
  AlchemyIgniteRequestV1,
  AlchemyInterventionRequestV1,
  AlchemyLearnRecipeRequestV1,
  AlchemyOpenFurnaceRequestV1,
  AlchemyTakeBackRequestV1,
  AlchemyTakePillRequestV1,
  AlchemyTurnPageRequestV1,
  BotanyHarvestRequestV1,
  CombatCreateNewCharacterRequestV1,
  CombatReincarnateRequestV1,
  CombatTerminateRequestV1,
  BreakthroughRequestV1,
  ClientRequestV1,
  DuoSheRequestV1,
  ForgeRequestV1,
  InsightDecisionRequestV1,
  MineralProbeRequestV1,
  SetMeridianTargetRequestV1,
  UseLifeCoreRequestV1,
} from "./client-request.js";
import { CombatRealtimeEventV1, CombatSummaryV1 } from "./combat-event.js";
import { CultivationDeathV1 } from "./cultivation-death.js";
import { DeathInsightRequestV1 } from "./death-insight.js";
import {
  DeathRegistryV1,
  AgingEventV1,
  DeceasedIndexEntryV1,
  DeceasedSnapshotV1,
  DuoSheEventV1,
  LifespanEventV1,
  LifespanCapByRealmV1,
  LifespanComponentV1,
  LifespanPreviewV1,
  RebirthChanceInputV1,
  RebirthChanceResultV1,
} from "./death-lifecycle.js";
import { ForgeEventV1 } from "./forge-event.js";
import { InventoryEventV1, InventorySnapshotV1 } from "./inventory.js";
import { InsightOfferV1 } from "./insight-offer.js";
import { InsightRequestV1 } from "./insight-request.js";
import { NarrationV1 } from "./narration.js";
import {
  ServerDataAlchemyContaminationV1,
  ServerDataAlchemyFurnaceV1,
  ServerDataAlchemyOutcomeForecastV1,
  ServerDataAlchemyOutcomeResolvedV1,
  ServerDataAlchemyRecipeBookV1,
  ServerDataAlchemySessionV1,
  ServerDataDeathScreenV1,
  ServerDataTerminateScreenV1,
  ServerDataBotanyHarvestProgressV1,
  ServerDataBotanySkillV1,
  ServerDataSkillCapChangedV1,
  ServerDataSkillLvUpV1,
  ServerDataSkillSnapshotV1,
  ServerDataSkillScrollUsedV1,
  ServerDataSkillXpGainV1,
  ServerDataV1,
} from "./server-data.js";
import {
  DaoxiangSpawnedV1,
  TsyCollapseCompletedV1,
  TsyCollapseStartedV1,
  TsyCorpseSpawnEventV1,
  TsyEnterEventV1,
  TsyExitEventV1,
  TsyZoneActivatedV1,
} from "./tsy.js";
import {
  CancelSearchRequestV1,
  ContainerStateV1,
  SearchAbortedV1,
  SearchCompletedV1,
  SearchProgressV1,
  SearchStartedV1,
  StartSearchRequestV1,
} from "./container-interaction.js";
import { VfxEventV1 } from "./vfx-event.js";
import { WorldStateV1 } from "./world-state.js";

export const SCHEMA_REGISTRY = {
  worldStateV1: WorldStateV1,
  agentCommandV1: AgentCommandV1,
  agentWorldModelEnvelopeV1: AgentWorldModelEnvelopeV1,
  agentWorldModelSnapshotV1: AgentWorldModelSnapshotV1,
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
  deathInsightRequestV1: DeathInsightRequestV1,
  deathRegistryV1: DeathRegistryV1,
  deceasedIndexEntryV1: DeceasedIndexEntryV1,
  deceasedSnapshotV1: DeceasedSnapshotV1,
  lifespanEventV1: LifespanEventV1,
  agingEventV1: AgingEventV1,
  duoSheEventV1: DuoSheEventV1,
  lifespanCapByRealmV1: LifespanCapByRealmV1,
  lifespanComponentV1: LifespanComponentV1,
  lifespanPreviewV1: LifespanPreviewV1,
  rebirthChanceInputV1: RebirthChanceInputV1,
  rebirthChanceResultV1: RebirthChanceResultV1,
  combatRealtimeEventV1: CombatRealtimeEventV1,
  combatSummaryV1: CombatSummaryV1,
  armorDurabilityChangedV1: ArmorDurabilityChangedV1,
  clientRequestV1: ClientRequestV1,
  clientRequestSetMeridianTargetV1: SetMeridianTargetRequestV1,
  clientRequestBreakthroughV1: BreakthroughRequestV1,
  clientRequestForgeV1: ForgeRequestV1,
  clientRequestInsightDecisionV1: InsightDecisionRequestV1,
  clientRequestDuoSheV1: DuoSheRequestV1,
  clientRequestUseLifeCoreV1: UseLifeCoreRequestV1,
  clientRequestMineralProbeV1: MineralProbeRequestV1,
  clientRequestBotanyHarvestV1: BotanyHarvestRequestV1,
  clientRequestCombatReincarnateV1: CombatReincarnateRequestV1,
  clientRequestCombatTerminateV1: CombatTerminateRequestV1,
  clientRequestCombatCreateNewCharacterV1: CombatCreateNewCharacterRequestV1,
  serverDataBotanyHarvestProgressV1: ServerDataBotanyHarvestProgressV1,
  serverDataBotanySkillV1: ServerDataBotanySkillV1,
  serverDataDeathScreenV1: ServerDataDeathScreenV1,
  serverDataTerminateScreenV1: ServerDataTerminateScreenV1,
  serverDataSkillXpGainV1: ServerDataSkillXpGainV1,
  serverDataSkillLvUpV1: ServerDataSkillLvUpV1,
  serverDataSkillCapChangedV1: ServerDataSkillCapChangedV1,
  serverDataSkillSnapshotV1: ServerDataSkillSnapshotV1,
  serverDataSkillScrollUsedV1: ServerDataSkillScrollUsedV1,
  botanyEcologySnapshotV1: BotanyEcologySnapshotV1,
  vfxEventV1: VfxEventV1,
  // 炼丹 (plan-alchemy-v1 §4)
  alchemyOutcomeBucket: AlchemyOutcomeBucket,
  alchemyInterventionV1: AlchemyInterventionV1,
  alchemyRecipeEntryV1: AlchemyRecipeEntryV1,
  alchemyStageHintV1: AlchemyStageHintV1,
  alchemyContaminationLevelV1: AlchemyContaminationLevelV1,
  serverDataAlchemyFurnaceV1: ServerDataAlchemyFurnaceV1,
  serverDataAlchemySessionV1: ServerDataAlchemySessionV1,
  serverDataAlchemyOutcomeForecastV1: ServerDataAlchemyOutcomeForecastV1,
  serverDataAlchemyOutcomeResolvedV1: ServerDataAlchemyOutcomeResolvedV1,
  serverDataAlchemyRecipeBookV1: ServerDataAlchemyRecipeBookV1,
  serverDataAlchemyContaminationV1: ServerDataAlchemyContaminationV1,
  clientRequestAlchemyOpenFurnaceV1: AlchemyOpenFurnaceRequestV1,
  clientRequestAlchemyFeedSlotV1: AlchemyFeedSlotRequestV1,
  clientRequestAlchemyTakeBackV1: AlchemyTakeBackRequestV1,
  clientRequestAlchemyIgniteV1: AlchemyIgniteRequestV1,
  clientRequestAlchemyInterventionV1: AlchemyInterventionRequestV1,
  clientRequestAlchemyTurnPageV1: AlchemyTurnPageRequestV1,
  clientRequestAlchemyLearnRecipeV1: AlchemyLearnRecipeRequestV1,
  clientRequestAlchemyTakePillV1: AlchemyTakePillRequestV1,
  // plan-tsy-zone-v1 §1.4
  tsyEnterEventV1: TsyEnterEventV1,
  tsyExitEventV1: TsyExitEventV1,
  // plan-tsy-loot-v1 §4.4
  tsyCorpseSpawnEventV1: TsyCorpseSpawnEventV1,
  // plan-tsy-lifecycle-v1 §1.5 / §3.1 / §4
  tsyZoneActivatedV1: TsyZoneActivatedV1,
  tsyCollapseStartedV1: TsyCollapseStartedV1,
  tsyCollapseCompletedV1: TsyCollapseCompletedV1,
  daoxiangSpawnedV1: DaoxiangSpawnedV1,
  // plan-tsy-container-v1 §5.1 — TSY 容器搜刮
  containerStateV1: ContainerStateV1,
  searchStartedV1: SearchStartedV1,
  searchProgressV1: SearchProgressV1,
  searchCompletedV1: SearchCompletedV1,
  searchAbortedV1: SearchAbortedV1,
  clientRequestStartSearchV1: StartSearchRequestV1,
  clientRequestCancelSearchV1: CancelSearchRequestV1,
} as const satisfies Record<string, TSchema>;

export const GENERATED_SCHEMA_FILES = {
  "world-state-v1.json": SCHEMA_REGISTRY.worldStateV1,
  "agent-command-v1.json": SCHEMA_REGISTRY.agentCommandV1,
  "agent-world-model-envelope-v1.json": SCHEMA_REGISTRY.agentWorldModelEnvelopeV1,
  "agent-world-model-snapshot-v1.json": SCHEMA_REGISTRY.agentWorldModelSnapshotV1,
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
  "death-insight-request-v1.json": SCHEMA_REGISTRY.deathInsightRequestV1,
  "death-registry-v1.json": SCHEMA_REGISTRY.deathRegistryV1,
  "deceased-index-entry-v1.json": SCHEMA_REGISTRY.deceasedIndexEntryV1,
  "deceased-snapshot-v1.json": SCHEMA_REGISTRY.deceasedSnapshotV1,
  "lifespan-event-v1.json": SCHEMA_REGISTRY.lifespanEventV1,
  "aging-event-v1.json": SCHEMA_REGISTRY.agingEventV1,
  "duo-she-event-v1.json": SCHEMA_REGISTRY.duoSheEventV1,
  "lifespan-cap-by-realm-v1.json": SCHEMA_REGISTRY.lifespanCapByRealmV1,
  "lifespan-component-v1.json": SCHEMA_REGISTRY.lifespanComponentV1,
  "lifespan-preview-v1.json": SCHEMA_REGISTRY.lifespanPreviewV1,
  "rebirth-chance-input-v1.json": SCHEMA_REGISTRY.rebirthChanceInputV1,
  "rebirth-chance-result-v1.json": SCHEMA_REGISTRY.rebirthChanceResultV1,
  "combat-realtime-event-v1.json": SCHEMA_REGISTRY.combatRealtimeEventV1,
  "combat-summary-v1.json": SCHEMA_REGISTRY.combatSummaryV1,
  "armor-durability-changed-v1.json": SCHEMA_REGISTRY.armorDurabilityChangedV1,
  "client-request-v1.json": SCHEMA_REGISTRY.clientRequestV1,
  "client-request-set-meridian-target-v1.json":
    SCHEMA_REGISTRY.clientRequestSetMeridianTargetV1,
  "client-request-breakthrough-v1.json": SCHEMA_REGISTRY.clientRequestBreakthroughV1,
  "client-request-forge-v1.json": SCHEMA_REGISTRY.clientRequestForgeV1,
  "client-request-insight-decision-v1.json":
    SCHEMA_REGISTRY.clientRequestInsightDecisionV1,
  "client-request-duo-she-v1.json": SCHEMA_REGISTRY.clientRequestDuoSheV1,
  "client-request-use-life-core-v1.json":
    SCHEMA_REGISTRY.clientRequestUseLifeCoreV1,
  "client-request-mineral-probe-v1.json": SCHEMA_REGISTRY.clientRequestMineralProbeV1,
  "client-request-botany-harvest-v1.json":
    SCHEMA_REGISTRY.clientRequestBotanyHarvestV1,
  "client-request-combat-reincarnate-v1.json":
    SCHEMA_REGISTRY.clientRequestCombatReincarnateV1,
  "client-request-combat-terminate-v1.json":
    SCHEMA_REGISTRY.clientRequestCombatTerminateV1,
  "client-request-combat-create-new-character-v1.json":
    SCHEMA_REGISTRY.clientRequestCombatCreateNewCharacterV1,
  "server-data-botany-harvest-progress-v1.json":
    SCHEMA_REGISTRY.serverDataBotanyHarvestProgressV1,
  "server-data-botany-skill-v1.json":
    SCHEMA_REGISTRY.serverDataBotanySkillV1,
  "server-data-death-screen-v1.json": SCHEMA_REGISTRY.serverDataDeathScreenV1,
  "server-data-terminate-screen-v1.json": SCHEMA_REGISTRY.serverDataTerminateScreenV1,
  "server-data-skill-xp-gain-v1.json":
    SCHEMA_REGISTRY.serverDataSkillXpGainV1,
  "server-data-skill-lv-up-v1.json":
    SCHEMA_REGISTRY.serverDataSkillLvUpV1,
  "server-data-skill-cap-changed-v1.json":
    SCHEMA_REGISTRY.serverDataSkillCapChangedV1,
  "server-data-skill-snapshot-v1.json":
    SCHEMA_REGISTRY.serverDataSkillSnapshotV1,
  "server-data-skill-scroll-used-v1.json":
    SCHEMA_REGISTRY.serverDataSkillScrollUsedV1,
  "botany-ecology-snapshot-v1.json": SCHEMA_REGISTRY.botanyEcologySnapshotV1,
  "vfx-event-v1.json": SCHEMA_REGISTRY.vfxEventV1,
  // 炼丹 (plan-alchemy-v1 §4)
  "alchemy-outcome-bucket.json": SCHEMA_REGISTRY.alchemyOutcomeBucket,
  "alchemy-intervention-v1.json": SCHEMA_REGISTRY.alchemyInterventionV1,
  "alchemy-recipe-entry-v1.json": SCHEMA_REGISTRY.alchemyRecipeEntryV1,
  "alchemy-stage-hint-v1.json": SCHEMA_REGISTRY.alchemyStageHintV1,
  "alchemy-contamination-level-v1.json":
    SCHEMA_REGISTRY.alchemyContaminationLevelV1,
  "server-data-alchemy-furnace-v1.json":
    SCHEMA_REGISTRY.serverDataAlchemyFurnaceV1,
  "server-data-alchemy-session-v1.json":
    SCHEMA_REGISTRY.serverDataAlchemySessionV1,
  "server-data-alchemy-outcome-forecast-v1.json":
    SCHEMA_REGISTRY.serverDataAlchemyOutcomeForecastV1,
  "server-data-alchemy-outcome-resolved-v1.json":
    SCHEMA_REGISTRY.serverDataAlchemyOutcomeResolvedV1,
  "server-data-alchemy-recipe-book-v1.json":
    SCHEMA_REGISTRY.serverDataAlchemyRecipeBookV1,
  "server-data-alchemy-contamination-v1.json":
    SCHEMA_REGISTRY.serverDataAlchemyContaminationV1,
  "client-request-alchemy-open-furnace-v1.json":
    SCHEMA_REGISTRY.clientRequestAlchemyOpenFurnaceV1,
  "client-request-alchemy-feed-slot-v1.json":
    SCHEMA_REGISTRY.clientRequestAlchemyFeedSlotV1,
  "client-request-alchemy-take-back-v1.json":
    SCHEMA_REGISTRY.clientRequestAlchemyTakeBackV1,
  "client-request-alchemy-ignite-v1.json":
    SCHEMA_REGISTRY.clientRequestAlchemyIgniteV1,
  "client-request-alchemy-intervention-v1.json":
    SCHEMA_REGISTRY.clientRequestAlchemyInterventionV1,
  "client-request-alchemy-turn-page-v1.json":
    SCHEMA_REGISTRY.clientRequestAlchemyTurnPageV1,
  "client-request-alchemy-learn-recipe-v1.json":
    SCHEMA_REGISTRY.clientRequestAlchemyLearnRecipeV1,
  "client-request-alchemy-take-pill-v1.json":
    SCHEMA_REGISTRY.clientRequestAlchemyTakePillV1,
  // plan-tsy-zone-v1 §1.4 — JSON Schema 导出供 Rust serde 双端校验
  "tsy-enter-event-v1.json": SCHEMA_REGISTRY.tsyEnterEventV1,
  "tsy-exit-event-v1.json": SCHEMA_REGISTRY.tsyExitEventV1,
  // plan-tsy-loot-v1 §4.4
  "tsy-corpse-spawn-event-v1.json": SCHEMA_REGISTRY.tsyCorpseSpawnEventV1,
  // plan-tsy-lifecycle-v1 §1.5 / §3.1 / §4
  "tsy-zone-activated-v1.json": SCHEMA_REGISTRY.tsyZoneActivatedV1,
  "tsy-collapse-started-v1.json": SCHEMA_REGISTRY.tsyCollapseStartedV1,
  "tsy-collapse-completed-v1.json": SCHEMA_REGISTRY.tsyCollapseCompletedV1,
  "daoxiang-spawned-v1.json": SCHEMA_REGISTRY.daoxiangSpawnedV1,
  // plan-tsy-container-v1 §5.1
  "container-state-v1.json": SCHEMA_REGISTRY.containerStateV1,
  "search-started-v1.json": SCHEMA_REGISTRY.searchStartedV1,
  "search-progress-v1.json": SCHEMA_REGISTRY.searchProgressV1,
  "search-completed-v1.json": SCHEMA_REGISTRY.searchCompletedV1,
  "search-aborted-v1.json": SCHEMA_REGISTRY.searchAbortedV1,
  "client-request-start-search-v1.json": SCHEMA_REGISTRY.clientRequestStartSearchV1,
  "client-request-cancel-search-v1.json": SCHEMA_REGISTRY.clientRequestCancelSearchV1,
} as const satisfies Record<string, TSchema>;

export type SchemaRegistryKey = keyof typeof SCHEMA_REGISTRY;
export type GeneratedSchemaFileName = keyof typeof GENERATED_SCHEMA_FILES;
