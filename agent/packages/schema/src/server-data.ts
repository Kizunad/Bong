import { Type, type Static } from "@sinclair/typebox";

import {
  AlchemyContaminationLevelV1,
  AlchemyOutcomeBucket,
  AlchemyRecipeEntryV1,
  AlchemyStageHintV1,
} from "./alchemy.js";
import { BotanyHarvestModeV1 } from "./botany.js";
import { EventKind, MAX_PAYLOAD_BYTES } from "./common.js";
import { ColorKind, SkillMilestoneSnapshotV1 } from "./cultivation.js";
import {
  InventoryEventDroppedV1,
  InventoryEventDurabilityChangedV1,
  InventoryEventMovedV1,
  InventoryEventStackChangedV1,
  InventoryItemViewV1,
  InventorySnapshotV1,
} from "./inventory.js";
import { Narration } from "./narration.js";
import {
  SkillCapChangedPayloadV1,
  SkillEntrySnapshotV1,
  SkillLvUpPayloadV1,
  SkillScrollUsedPayloadV1,
  SkillSnapshotPayloadV1,
  SkillXpGainPayloadV1,
} from "./skill.js";
import { PlayerPowerBreakdown, Vec3 } from "./world-state.js";

const MERIDIAN_CHANNEL_COUNT = 20;

const CultivationOpenedArrayV1 = Type.Array(Type.Boolean(), {
  minItems: MERIDIAN_CHANNEL_COUNT,
  maxItems: MERIDIAN_CHANNEL_COUNT,
});

const CultivationFlowArrayV1 = Type.Array(Type.Number({ minimum: 0 }), {
  minItems: MERIDIAN_CHANNEL_COUNT,
  maxItems: MERIDIAN_CHANNEL_COUNT,
});

const CultivationIntegrityArrayV1 = Type.Array(
  Type.Number({ minimum: 0, maximum: 1 }),
  {
    minItems: MERIDIAN_CHANNEL_COUNT,
    maxItems: MERIDIAN_CHANNEL_COUNT,
  },
);

const CultivationProgressArrayV1 = Type.Array(
  Type.Number({ minimum: 0, maximum: 1 }),
  {
    minItems: MERIDIAN_CHANNEL_COUNT,
    maxItems: MERIDIAN_CHANNEL_COUNT,
  },
);

const CultivationCracksArrayV1 = Type.Array(
  Type.Integer({ minimum: 0, maximum: 255 }),
  {
    minItems: MERIDIAN_CHANNEL_COUNT,
    maxItems: MERIDIAN_CHANNEL_COUNT,
  },
);

export const ServerDataType = Type.Union([
  Type.Literal("welcome"),
  Type.Literal("heartbeat"),
  Type.Literal("narration"),
  Type.Literal("zone_info"),
  Type.Literal("event_alert"),
  Type.Literal("player_state"),
  Type.Literal("ui_open"),
  Type.Literal("cultivation_detail"),
  Type.Literal("inventory_event"),
  Type.Literal("inventory_snapshot"),
  Type.Literal("dropped_loot_sync"),
  Type.Literal("botany_harvest_progress"),
  Type.Literal("botany_skill"),
  Type.Literal("alchemy_furnace"),
  Type.Literal("alchemy_session"),
  Type.Literal("alchemy_outcome_forecast"),
  Type.Literal("alchemy_outcome_resolved"),
  Type.Literal("alchemy_recipe_book"),
  Type.Literal("alchemy_contamination"),
  Type.Literal("skill_xp_gain"),
  Type.Literal("skill_lv_up"),
  Type.Literal("skill_cap_changed"),
  Type.Literal("skill_scroll_used"),
  Type.Literal("skill_snapshot"),
]);
export type ServerDataType = Static<typeof ServerDataType>;

export const ServerDataWelcomeV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("welcome"),
    message: Type.String({ maxLength: MAX_PAYLOAD_BYTES }),
  },
  { additionalProperties: false },
);
export type ServerDataWelcomeV1 = Static<typeof ServerDataWelcomeV1>;

export const ServerDataHeartbeatV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("heartbeat"),
    message: Type.String({ maxLength: MAX_PAYLOAD_BYTES }),
  },
  { additionalProperties: false },
);
export type ServerDataHeartbeatV1 = Static<typeof ServerDataHeartbeatV1>;

export const ServerDataNarrationV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("narration"),
    narrations: Type.Array(Narration),
  },
  { additionalProperties: false },
);
export type ServerDataNarrationV1 = Static<typeof ServerDataNarrationV1>;

export const ServerDataZoneInfoV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("zone_info"),
    zone: Type.String(),
    spirit_qi: Type.Number({ minimum: -1, maximum: 1 }),
    danger_level: Type.Integer({ minimum: 0, maximum: 5 }),
    active_events: Type.Optional(Type.Array(Type.String())),
  },
  { additionalProperties: false },
);
export type ServerDataZoneInfoV1 = Static<typeof ServerDataZoneInfoV1>;

export const ServerDataEventAlertV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("event_alert"),
    event: EventKind,
    message: Type.String({ maxLength: 500 }),
    zone: Type.Optional(Type.String()),
    duration_ticks: Type.Optional(Type.Integer({ minimum: 0 })),
  },
  { additionalProperties: false },
);
export type ServerDataEventAlertV1 = Static<typeof ServerDataEventAlertV1>;

export const ServerDataPlayerStateV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("player_state"),
    player: Type.Optional(Type.String()),
    realm: Type.String(),
    spirit_qi: Type.Number({ minimum: 0, maximum: 160 }),
    karma: Type.Number({ minimum: -1, maximum: 1 }),
    composite_power: Type.Number({ minimum: 0, maximum: 1 }),
    breakdown: PlayerPowerBreakdown,
    zone: Type.String(),
  },
  { additionalProperties: false },
);
export type ServerDataPlayerStateV1 = Static<typeof ServerDataPlayerStateV1>;

export const ServerDataUiOpenV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("ui_open"),
    ui: Type.Optional(Type.String({ description: "logical UI key" })),
    xml: Type.String({ maxLength: 10_240 }),
  },
  { additionalProperties: false },
);
export type ServerDataUiOpenV1 = Static<typeof ServerDataUiOpenV1>;

export const ServerDataCultivationDetailV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("cultivation_detail"),
    realm: Type.String(),
    opened: CultivationOpenedArrayV1,
    flow_rate: CultivationFlowArrayV1,
    flow_capacity: CultivationFlowArrayV1,
    integrity: CultivationIntegrityArrayV1,
    open_progress: Type.Optional(CultivationProgressArrayV1),
    cracks_count: Type.Optional(CultivationCracksArrayV1),
    contamination_total: Type.Number({ minimum: 0 }),
    recent_skill_milestones_summary: Type.Optional(Type.String({ maxLength: 4096 })),
    skill_milestones: Type.Optional(Type.Array(SkillMilestoneSnapshotV1)),
  },
  { additionalProperties: false },
);
export type ServerDataCultivationDetailV1 = Static<
  typeof ServerDataCultivationDetailV1
>;

export const ServerDataInventorySnapshotV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("inventory_snapshot"),
    ...InventorySnapshotV1.properties,
  },
  { additionalProperties: false },
);
export type ServerDataInventorySnapshotV1 = Static<
  typeof ServerDataInventorySnapshotV1
>;

export const DroppedLootEntryV1 = Type.Object(
  {
    instance_id: Type.Integer({ minimum: 0 }),
    source_container_id: Type.String(),
    source_row: Type.Integer({ minimum: 0 }),
    source_col: Type.Integer({ minimum: 0 }),
    world_pos: Vec3,
    item: InventoryItemViewV1,
  },
  { additionalProperties: false },
);
export type DroppedLootEntryV1 = Static<typeof DroppedLootEntryV1>;

export const ServerDataDroppedLootSyncV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("dropped_loot_sync"),
    drops: Type.Array(DroppedLootEntryV1),
  },
  { additionalProperties: false },
);
export type ServerDataDroppedLootSyncV1 = Static<typeof ServerDataDroppedLootSyncV1>;

const ServerDataInventoryEventMovedV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("inventory_event"),
    ...InventoryEventMovedV1.properties,
  },
  { additionalProperties: false },
);

const ServerDataInventoryEventStackChangedV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("inventory_event"),
    ...InventoryEventStackChangedV1.properties,
  },
  { additionalProperties: false },
);

const ServerDataInventoryEventDroppedV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("inventory_event"),
    ...InventoryEventDroppedV1.properties,
  },
  { additionalProperties: false },
);

const ServerDataInventoryEventDurabilityChangedV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("inventory_event"),
    ...InventoryEventDurabilityChangedV1.properties,
  },
  { additionalProperties: false },
);

export const ServerDataInventoryEventV1 = Type.Union([
  ServerDataInventoryEventMovedV1,
  ServerDataInventoryEventDroppedV1,
  ServerDataInventoryEventStackChangedV1,
  ServerDataInventoryEventDurabilityChangedV1,
]);
export type ServerDataInventoryEventV1 = Static<typeof ServerDataInventoryEventV1>;

export const ServerDataBotanyHarvestProgressV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("botany_harvest_progress"),
    session_id: Type.String({ minLength: 1 }),
    target_id: Type.String({ minLength: 1 }),
    target_name: Type.String({ minLength: 1 }),
    plant_kind: Type.String({ minLength: 1 }),
    mode: BotanyHarvestModeV1,
    progress: Type.Number({ minimum: 0, maximum: 1 }),
    auto_selectable: Type.Boolean(),
    request_pending: Type.Boolean(),
    interrupted: Type.Boolean(),
    completed: Type.Boolean(),
    detail: Type.String(),
    // plan §1.3 投影锚定：目标植物世界坐标，client 侧做 world→screen 投影定位浮窗。
    // 省略时 client 回退到准星右侧锚点。
    target_pos: Type.Optional(
      Type.Tuple([Type.Number(), Type.Number(), Type.Number()]),
    ),
  },
  { additionalProperties: false },
);
export type ServerDataBotanyHarvestProgressV1 = Static<
  typeof ServerDataBotanyHarvestProgressV1
>;

export const ServerDataBotanySkillV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("botany_skill"),
    level: Type.Integer({ minimum: 0 }),
    xp: Type.Integer({ minimum: 0 }),
    xp_to_next_level: Type.Integer({ minimum: 1 }),
    auto_unlock_level: Type.Integer({ minimum: 1 }),
  },
  { additionalProperties: false },
);
export type ServerDataBotanySkillV1 = Static<typeof ServerDataBotanySkillV1>;

// ─── 炼丹推送（plan-alchemy-v1 §4） ────────────────────────────────────────

export const ServerDataAlchemyFurnaceV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("alchemy_furnace"),
    furnace_id: Type.String(),
    tier: Type.Integer({ minimum: 1, maximum: 9 }),
    integrity: Type.Number({ minimum: 0 }),
    integrity_max: Type.Number({ minimum: 0 }),
    owner_name: Type.String(),
    has_session: Type.Boolean(),
  },
  { additionalProperties: false },
);
export type ServerDataAlchemyFurnaceV1 = Static<typeof ServerDataAlchemyFurnaceV1>;

export const ServerDataAlchemySessionV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("alchemy_session"),
    /** null = 未起炉。 */
    recipe_id: Type.Union([Type.String(), Type.Null()]),
    active: Type.Boolean(),
    elapsed_ticks: Type.Integer({ minimum: 0 }),
    target_ticks: Type.Integer({ minimum: 0 }),
    temp_current: Type.Number({ minimum: 0, maximum: 1 }),
    temp_target: Type.Number({ minimum: 0, maximum: 1 }),
    temp_band: Type.Number({ minimum: 0 }),
    qi_injected: Type.Number({ minimum: 0 }),
    qi_target: Type.Number({ minimum: 0 }),
    status_label: Type.String(),
    stages: Type.Array(AlchemyStageHintV1),
    /** 服务端预格式化后给 client 直接显示（含色码）。 */
    interventions_recent: Type.Array(Type.String(), { maxItems: 8 }),
  },
  { additionalProperties: false },
);
export type ServerDataAlchemySessionV1 = Static<typeof ServerDataAlchemySessionV1>;

export const ServerDataAlchemyOutcomeForecastV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("alchemy_outcome_forecast"),
    perfect_pct: Type.Number({ minimum: 0, maximum: 100 }),
    good_pct: Type.Number({ minimum: 0, maximum: 100 }),
    flawed_pct: Type.Number({ minimum: 0, maximum: 100 }),
    waste_pct: Type.Number({ minimum: 0, maximum: 100 }),
    explode_pct: Type.Number({ minimum: 0, maximum: 100 }),
    perfect_note: Type.String(),
    good_note: Type.String(),
    flawed_note: Type.String(),
  },
  { additionalProperties: false },
);
export type ServerDataAlchemyOutcomeForecastV1 = Static<
  typeof ServerDataAlchemyOutcomeForecastV1
>;

export const ServerDataAlchemyOutcomeResolvedV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("alchemy_outcome_resolved"),
    bucket: AlchemyOutcomeBucket,
    recipe_id: Type.Union([Type.String(), Type.Null()]),
    pill: Type.Optional(Type.String()),
    quality: Type.Optional(Type.Number({ minimum: 0, maximum: 1 })),
    toxin_amount: Type.Optional(Type.Number({ minimum: 0 })),
    toxin_color: Type.Optional(ColorKind),
    qi_gain: Type.Optional(Type.Number({ minimum: 0 })),
    side_effect_tag: Type.Optional(Type.String()),
    flawed_path: Type.Boolean(),
    damage: Type.Optional(Type.Number({ minimum: 0 })),
    meridian_crack: Type.Optional(Type.Number({ minimum: 0 })),
  },
  { additionalProperties: false },
);
export type ServerDataAlchemyOutcomeResolvedV1 = Static<
  typeof ServerDataAlchemyOutcomeResolvedV1
>;

export const ServerDataAlchemyRecipeBookV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("alchemy_recipe_book"),
    learned: Type.Array(AlchemyRecipeEntryV1),
    current_index: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type ServerDataAlchemyRecipeBookV1 = Static<typeof ServerDataAlchemyRecipeBookV1>;

export const ServerDataAlchemyContaminationV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("alchemy_contamination"),
    /** 通常 mellow + violent 各一条；可扩展更多色。 */
    levels: Type.Array(AlchemyContaminationLevelV1, { minItems: 0, maxItems: 10 }),
    metabolism_note: Type.String(),
  },
  { additionalProperties: false },
);
export type ServerDataAlchemyContaminationV1 = Static<
  typeof ServerDataAlchemyContaminationV1
>;

export const ServerDataSkillXpGainV1 = Type.Object(
  {
    type: Type.Literal("skill_xp_gain"),
    ...SkillXpGainPayloadV1.properties,
  },
  { additionalProperties: false },
);
export type ServerDataSkillXpGainV1 = Static<typeof ServerDataSkillXpGainV1>;

export const ServerDataSkillLvUpV1 = Type.Object(
  {
    type: Type.Literal("skill_lv_up"),
    ...SkillLvUpPayloadV1.properties,
  },
  { additionalProperties: false },
);
export type ServerDataSkillLvUpV1 = Static<typeof ServerDataSkillLvUpV1>;

export const ServerDataSkillCapChangedV1 = Type.Object(
  {
    type: Type.Literal("skill_cap_changed"),
    ...SkillCapChangedPayloadV1.properties,
  },
  { additionalProperties: false },
);
export type ServerDataSkillCapChangedV1 = Static<
  typeof ServerDataSkillCapChangedV1
>;

export const ServerDataSkillScrollUsedV1 = Type.Object(
  {
    type: Type.Literal("skill_scroll_used"),
    ...SkillScrollUsedPayloadV1.properties,
  },
  { additionalProperties: false },
);
export type ServerDataSkillScrollUsedV1 = Static<
  typeof ServerDataSkillScrollUsedV1
>;

export const ServerDataSkillSnapshotV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("skill_snapshot"),
    char_id: Type.Integer({ minimum: 0 }),
    skills: Type.Record(Type.String({ minLength: 1 }), SkillEntrySnapshotV1),
  },
  { additionalProperties: false },
);
export type ServerDataSkillSnapshotV1 = Static<typeof ServerDataSkillSnapshotV1>;

export const ServerDataV1 = Type.Union([
  ServerDataWelcomeV1,
  ServerDataHeartbeatV1,
  ServerDataNarrationV1,
  ServerDataZoneInfoV1,
  ServerDataEventAlertV1,
  ServerDataPlayerStateV1,
  ServerDataUiOpenV1,
  ServerDataCultivationDetailV1,
  ServerDataInventorySnapshotV1,
  ServerDataInventoryEventV1,
  ServerDataDroppedLootSyncV1,
  ServerDataBotanyHarvestProgressV1,
  ServerDataBotanySkillV1,
  ServerDataAlchemyFurnaceV1,
  ServerDataAlchemySessionV1,
  ServerDataAlchemyOutcomeForecastV1,
  ServerDataAlchemyOutcomeResolvedV1,
  ServerDataAlchemyRecipeBookV1,
  ServerDataAlchemyContaminationV1,
  ServerDataSkillXpGainV1,
  ServerDataSkillLvUpV1,
  ServerDataSkillCapChangedV1,
  ServerDataSkillScrollUsedV1,
  ServerDataSkillSnapshotV1,
]);
export type ServerDataV1 = Static<typeof ServerDataV1>;
