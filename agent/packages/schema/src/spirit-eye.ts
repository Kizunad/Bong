import { Type, type Static } from "@sinclair/typebox";

import { validate, type ValidationResult } from "./validate.js";

export const SpiritEyePositionV1 = Type.Object(
  {
    x: Type.Number(),
    y: Type.Number(),
    z: Type.Number(),
  },
  { additionalProperties: false },
);
export type SpiritEyePositionV1 = Static<typeof SpiritEyePositionV1>;

export const SpiritEyeMigrateReasonV1 = Type.Union([
  Type.Literal("usage_pressure"),
  Type.Literal("periodic_drift"),
]);
export type SpiritEyeMigrateReasonV1 = Static<typeof SpiritEyeMigrateReasonV1>;

export const SpiritEyeMigrateV1 = Type.Object(
  {
    v: Type.Literal(1),
    eye_id: Type.String({ minLength: 1, maxLength: 160 }),
    from: SpiritEyePositionV1,
    to: SpiritEyePositionV1,
    reason: SpiritEyeMigrateReasonV1,
    usage_pressure: Type.Number({ minimum: 0 }),
    tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type SpiritEyeMigrateV1 = Static<typeof SpiritEyeMigrateV1>;

export const SpiritEyeDiscoveredV1 = Type.Object(
  {
    v: Type.Literal(1),
    eye_id: Type.String({ minLength: 1, maxLength: 160 }),
    character_id: Type.String({ minLength: 1, maxLength: 160 }),
    pos: SpiritEyePositionV1,
    zone: Type.Optional(Type.String({ minLength: 1, maxLength: 160 })),
    qi_concentration: Type.Number({ minimum: 0 }),
    discovered_at_tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type SpiritEyeDiscoveredV1 = Static<typeof SpiritEyeDiscoveredV1>;

export const SpiritEyeUsedForBreakthroughV1 = Type.Object(
  {
    v: Type.Literal(1),
    eye_id: Type.String({ minLength: 1, maxLength: 160 }),
    character_id: Type.String({ minLength: 1, maxLength: 160 }),
    realm_from: Type.String({ minLength: 1, maxLength: 64 }),
    realm_to: Type.String({ minLength: 1, maxLength: 64 }),
    usage_pressure: Type.Number({ minimum: 0 }),
    tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type SpiritEyeUsedForBreakthroughV1 = Static<typeof SpiritEyeUsedForBreakthroughV1>;

export const SpiritEyeCoordinateNoteV1 = Type.Object(
  {
    v: Type.Literal(1),
    eye_id: Type.String({ minLength: 1, maxLength: 160 }),
    owner_character_id: Type.String({ minLength: 1, maxLength: 160 }),
    pos: SpiritEyePositionV1,
    zone: Type.Optional(Type.String({ minLength: 1, maxLength: 160 })),
    qi_concentration: Type.Number({ minimum: 0 }),
    discovered_at_tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type SpiritEyeCoordinateNoteV1 = Static<typeof SpiritEyeCoordinateNoteV1>;

export const DeathInsightSpiritEyeV1 = Type.Object(
  {
    eye_id: Type.String({ minLength: 1, maxLength: 160 }),
    zone: Type.Optional(Type.String({ minLength: 1, maxLength: 160 })),
    pos: SpiritEyePositionV1,
    qi_concentration: Type.Number({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type DeathInsightSpiritEyeV1 = Static<typeof DeathInsightSpiritEyeV1>;

export function validateSpiritEyeMigrateV1Contract(data: unknown): ValidationResult {
  return validate(SpiritEyeMigrateV1, data);
}

export function validateSpiritEyeDiscoveredV1Contract(data: unknown): ValidationResult {
  return validate(SpiritEyeDiscoveredV1, data);
}

export function validateSpiritEyeUsedForBreakthroughV1Contract(
  data: unknown,
): ValidationResult {
  return validate(SpiritEyeUsedForBreakthroughV1, data);
}
