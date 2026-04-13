import { Type, type Static } from "@sinclair/typebox";

import { EventKind, MAX_PAYLOAD_BYTES } from "./common.js";
import { Narration } from "./narration.js";
import { PlayerPowerBreakdown } from "./world-state.js";

export const ServerDataType = Type.Union([
  Type.Literal("welcome"),
  Type.Literal("heartbeat"),
  Type.Literal("narration"),
  Type.Literal("zone_info"),
  Type.Literal("event_alert"),
  Type.Literal("player_state"),
  Type.Literal("ui_open"),
  Type.Literal("cultivation_detail"),
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
    opened: Type.Array(Type.Boolean()),
    flow_rate: Type.Array(Type.Number()),
    flow_capacity: Type.Array(Type.Number()),
    integrity: Type.Array(Type.Number()),
    open_progress: Type.Array(Type.Number()),
    cracks_count: Type.Array(Type.Integer({ minimum: 0, maximum: 255 })),
    contamination_total: Type.Number(),
  },
  { additionalProperties: false },
);
export type ServerDataCultivationDetailV1 = Static<
  typeof ServerDataCultivationDetailV1
>;

export const ServerDataV1 = Type.Union([
  ServerDataWelcomeV1,
  ServerDataHeartbeatV1,
  ServerDataNarrationV1,
  ServerDataZoneInfoV1,
  ServerDataEventAlertV1,
  ServerDataPlayerStateV1,
  ServerDataUiOpenV1,
  ServerDataCultivationDetailV1,
]);
export type ServerDataV1 = Static<typeof ServerDataV1>;
