import { Type, type Static } from "@sinclair/typebox";

import { validate, type ValidationResult } from "./validate.js";

export const PseudoVeinSeasonV1 = Type.Union([
  Type.Literal("summer"),
  Type.Literal("summer_to_winter"),
  Type.Literal("winter"),
  Type.Literal("winter_to_summer"),
]);
export type PseudoVeinSeasonV1 = Static<typeof PseudoVeinSeasonV1>;

export const PseudoVeinSnapshotV1 = Type.Object(
  {
    v: Type.Literal(1),
    id: Type.String({ minLength: 1 }),
    center_xz: Type.Tuple([Type.Number(), Type.Number()]),
    spirit_qi_current: Type.Number({ minimum: 0, maximum: 1 }),
    occupants: Type.Array(Type.String({ minLength: 1 })),
    spawned_at_tick: Type.Integer({ minimum: 0 }),
    estimated_decay_at_tick: Type.Integer({ minimum: 0 }),
    season_at_spawn: PseudoVeinSeasonV1,
  },
  { additionalProperties: false },
);
export type PseudoVeinSnapshotV1 = Static<typeof PseudoVeinSnapshotV1>;

export const PseudoVeinDissipateEventV1 = Type.Object(
  {
    v: Type.Literal(1),
    id: Type.String({ minLength: 1 }),
    center_xz: Type.Tuple([Type.Number(), Type.Number()]),
    storm_anchors: Type.Array(Type.Tuple([Type.Number(), Type.Number()]), {
      minItems: 1,
      maxItems: 3,
    }),
    storm_duration_ticks: Type.Integer({ minimum: 6000, maximum: 12000 }),
    qi_redistribution: Type.Object(
      {
        refill_to_hungry_ring: Type.Number({ minimum: 0, maximum: 1 }),
        collected_by_tiandao: Type.Number({ minimum: 0, maximum: 1 }),
      },
      { additionalProperties: false },
    ),
  },
  { additionalProperties: false },
);
export type PseudoVeinDissipateEventV1 = Static<typeof PseudoVeinDissipateEventV1>;

export function validatePseudoVeinSnapshotV1Contract(data: unknown): ValidationResult {
  return validate(PseudoVeinSnapshotV1, data);
}

export function validatePseudoVeinDissipateEventV1Contract(data: unknown): ValidationResult {
  return validate(PseudoVeinDissipateEventV1, data);
}
