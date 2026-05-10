import { type Static, Type } from "@sinclair/typebox";

import { ColorKind } from "./cultivation.js";
import { validate, type ValidationResult } from "./validate.js";

export const StyleTelemetryColorSnapshotV1 = Type.Object(
  {
    main: ColorKind,
    secondary: Type.Optional(ColorKind),
    is_chaotic: Type.Boolean(),
    is_hunyuan: Type.Boolean(),
  },
  { additionalProperties: false },
);
export type StyleTelemetryColorSnapshotV1 = Static<
  typeof StyleTelemetryColorSnapshotV1
>;

export const StyleBalanceTelemetryEventV1 = Type.Object(
  {
    v: Type.Literal(1),
    attacker_player_id: Type.String({ minLength: 1 }),
    defender_player_id: Type.String({ minLength: 1 }),
    attacker_color: Type.Optional(StyleTelemetryColorSnapshotV1),
    defender_color: Type.Optional(StyleTelemetryColorSnapshotV1),
    attacker_style: Type.Optional(Type.String({ minLength: 1 })),
    defender_style: Type.Optional(Type.String({ minLength: 1 })),
    attacker_rejection_rate: Type.Optional(Type.Number({ minimum: 0, maximum: 1 })),
    defender_resistance: Type.Optional(Type.Number({ minimum: 0, maximum: 1 })),
    defender_drain_affinity: Type.Optional(Type.Number({ minimum: 0, maximum: 1 })),
    attacker_qi: Type.Optional(Type.Number({
      minimum: 0,
      description: "Injected qi before distance attenuation.",
    })),
    distance_blocks: Type.Optional(Type.Number({
      minimum: 0,
      description: "Collision distance in blocks.",
    })),
    effective_hit: Type.Optional(Type.Number({
      minimum: 0,
      description: "Post-rejection hit value before defender mitigation.",
    })),
    defender_lost: Type.Optional(Type.Number({
      minimum: 0,
      description: "Qi lost by the defender after mitigation.",
    })),
    defender_absorbed: Type.Optional(Type.Number({
      minimum: 0,
      description: "Qi absorbed by defender drain affinity.",
    })),
    cause: Type.String({ minLength: 1 }),
    resolved_at_tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type StyleBalanceTelemetryEventV1 = Static<
  typeof StyleBalanceTelemetryEventV1
>;

export function validateStyleBalanceTelemetryEventV1Contract(
  data: unknown,
): ValidationResult {
  return validate(StyleBalanceTelemetryEventV1, data);
}
