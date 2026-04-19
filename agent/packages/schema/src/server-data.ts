import { Type, type Static } from "@sinclair/typebox";

import { BotanyHarvestModeV1 } from "./botany.js";
import { EventKind, MAX_PAYLOAD_BYTES } from "./common.js";
import {
  InventoryEventDurabilityChangedV1,
  InventoryEventMovedV1,
  InventoryEventStackChangedV1,
  InventorySnapshotV1,
} from "./inventory.js";
import { Narration } from "./narration.js";
import { PlayerPowerBreakdown } from "./world-state.js";

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
  Type.Literal("botany_harvest_progress"),
  Type.Literal("botany_skill"),
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
  ServerDataBotanyHarvestProgressV1,
  ServerDataBotanySkillV1,
]);
export type ServerDataV1 = Static<typeof ServerDataV1>;
