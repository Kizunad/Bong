import { Type, type Static } from "@sinclair/typebox";

import { EventKind, MAX_PAYLOAD_BYTES } from "./common.js";
import { Narration } from "./narration.js";
import { validate, type ValidationResult } from "./validate.js";

const ClientMessage = Type.String({ minLength: 1, maxLength: 160 });
const ZoneName = Type.String({ minLength: 1, maxLength: 64 });

export const ClientPayloadType = Type.Union([
  Type.Literal("welcome"),
  Type.Literal("heartbeat"),
  Type.Literal("narration"),
  Type.Literal("zone_info"),
  Type.Literal("event_alert"),
  Type.Literal("locust_swarm_warning"),
  Type.Literal("player_state"),
]);
export type ClientPayloadType = Static<typeof ClientPayloadType>;

export const WelcomePayloadV1 = Type.Object({
  v: Type.Literal(1),
  type: Type.Literal("welcome"),
  message: ClientMessage,
});
export type WelcomePayloadV1 = Static<typeof WelcomePayloadV1>;

export const HeartbeatPayloadV1 = Type.Object({
  v: Type.Literal(1),
  type: Type.Literal("heartbeat"),
  message: ClientMessage,
});
export type HeartbeatPayloadV1 = Static<typeof HeartbeatPayloadV1>;

export const ClientNarrationPayloadV1 = Type.Object({
  v: Type.Literal(1),
  type: Type.Literal("narration"),
  narrations: Type.Array(Narration, { minItems: 1, maxItems: 1 }),
});
export type ClientNarrationPayloadV1 = Static<typeof ClientNarrationPayloadV1>;

export const ZoneInfoPayload = Type.Object({
  zone: ZoneName,
  spirit_qi: Type.Number({ minimum: 0, maximum: 1 }),
  danger_level: Type.Integer({ minimum: 0, maximum: 5 }),
  active_events: Type.Optional(Type.Array(Type.String({ minLength: 1, maxLength: 64 }), { maxItems: 4 })),
});
export type ZoneInfoPayload = Static<typeof ZoneInfoPayload>;

export const ZoneInfoPayloadV1 = Type.Object({
  v: Type.Literal(1),
  type: Type.Literal("zone_info"),
  zone_info: ZoneInfoPayload,
});
export type ZoneInfoPayloadV1 = Static<typeof ZoneInfoPayloadV1>;

export const EventAlertSeverity = Type.Union([
  Type.Literal("info"),
  Type.Literal("warning"),
  Type.Literal("critical"),
]);
export type EventAlertSeverity = Static<typeof EventAlertSeverity>;

export const EventAlertPayload = Type.Object({
  kind: EventKind,
  title: Type.String({ minLength: 1, maxLength: 80 }),
  detail: Type.String({ minLength: 1, maxLength: 900 }),
  severity: EventAlertSeverity,
  zone: Type.Optional(ZoneName),
});
export type EventAlertPayload = Static<typeof EventAlertPayload>;

export const EventAlertPayloadV1 = Type.Object({
  v: Type.Literal(1),
  type: Type.Literal("event_alert"),
  event_alert: EventAlertPayload,
});
export type EventAlertPayloadV1 = Static<typeof EventAlertPayloadV1>;

export const LocustSwarmWarningPayloadV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("locust_swarm_warning"),
    zone: ZoneName,
    message: Type.String({ minLength: 1, maxLength: 500 }),
    duration_ticks: Type.Optional(Type.Integer({ minimum: 0 })),
    direction: Type.Optional(Type.String({ minLength: 1, maxLength: 64 })),
  },
  { additionalProperties: false },
);
export type LocustSwarmWarningPayloadV1 = Static<typeof LocustSwarmWarningPayloadV1>;

export const PlayerStatePayload = Type.Object({
  realm: Type.String({ minLength: 1, maxLength: 64 }),
  spirit_qi: Type.Number({ minimum: 0 }),
  spirit_qi_max: Type.Number({ minimum: 0 }),
  karma: Type.Number({ minimum: -1, maximum: 1 }),
  composite_power: Type.Number({ minimum: 0, maximum: 1 }),
  zone: ZoneName,
});
export type PlayerStatePayload = Static<typeof PlayerStatePayload>;

export const PlayerStatePayloadV1 = Type.Object({
  v: Type.Literal(1),
  type: Type.Literal("player_state"),
  player_state: PlayerStatePayload,
});
export type PlayerStatePayloadV1 = Static<typeof PlayerStatePayloadV1>;

export const ClientPayloadV1 = Type.Union([
  WelcomePayloadV1,
  HeartbeatPayloadV1,
  ClientNarrationPayloadV1,
  ZoneInfoPayloadV1,
  EventAlertPayloadV1,
  LocustSwarmWarningPayloadV1,
  PlayerStatePayloadV1,
]);
export type ClientPayloadV1 = Static<typeof ClientPayloadV1>;

export function getClientPayloadByteLength(payload: unknown): number {
  return Buffer.byteLength(JSON.stringify(payload), "utf8");
}

export function validateClientPayloadV1(data: unknown): ValidationResult {
  const result = validate(ClientPayloadV1, data);

  if (!result.ok) {
    return result;
  }

  const byteLength = getClientPayloadByteLength(data);

  if (byteLength > MAX_PAYLOAD_BYTES) {
    return {
      ok: false,
      errors: [`$: serialized payload exceeds ${MAX_PAYLOAD_BYTES} bytes (${byteLength})`],
    };
  }

  return result;
}
