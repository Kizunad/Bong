import { Type, type Static } from "@sinclair/typebox";

import { validate, type ValidationResult } from "./validate.js";

// plan-mundane-fauna-v1 P3 生态可视化：每 zone × 物种一条聚合 count，用于天道 agent
// narration（凡兽绝迹 / 兽群迁徙 = 灵气恶化的可感征兆）。仿 `BotanyEcologySnapshotV1`。
export const FaunaSpeciesCountEntryV1 = Type.Object(
  {
    kind: Type.String({ minLength: 1 }),
    count: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type FaunaSpeciesCountEntryV1 = Static<typeof FaunaSpeciesCountEntryV1>;

export const FaunaZoneEcologyV1 = Type.Object(
  {
    zone: Type.String({ minLength: 1 }),
    spirit_qi: Type.Number({ minimum: -1, maximum: 1 }),
    species_counts: Type.Array(FaunaSpeciesCountEntryV1),
  },
  { additionalProperties: false },
);
export type FaunaZoneEcologyV1 = Static<typeof FaunaZoneEcologyV1>;

export const FaunaEcologySnapshotV1 = Type.Object(
  {
    v: Type.Literal(1),
    tick: Type.Integer({ minimum: 0 }),
    zones: Type.Array(FaunaZoneEcologyV1),
  },
  { additionalProperties: false },
);
export type FaunaEcologySnapshotV1 = Static<typeof FaunaEcologySnapshotV1>;

export function validateFaunaEcologySnapshotV1Contract(data: unknown): ValidationResult {
  return validate(FaunaEcologySnapshotV1, data);
}
