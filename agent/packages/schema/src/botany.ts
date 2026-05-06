import { Type, type Static } from "@sinclair/typebox";

import { validate, type ValidationResult } from "./validate.js";

export const BotanyHarvestModeV1 = Type.Union([
  Type.Literal("manual"),
  Type.Literal("auto"),
]);
export type BotanyHarvestModeV1 = Static<typeof BotanyHarvestModeV1>;

export const BotanyHarvestPhaseV1 = Type.Union([
  Type.Literal("pending"),
  Type.Literal("in_progress"),
  Type.Literal("completed"),
  Type.Literal("interrupted"),
  Type.Literal("trampled"),
]);
export type BotanyHarvestPhaseV1 = Static<typeof BotanyHarvestPhaseV1>;

// plan §7 植物变异 canonical 标签
export const BotanyVariantV1 = Type.Union([
  Type.Literal("none"),
  Type.Literal("thunder"),
  Type.Literal("tainted"),
]);
export type BotanyVariantV1 = Static<typeof BotanyVariantV1>;

// plan §7 生态可视化：每 zone 一条聚合数据，用于天道 agent 消费 + 运维观测
export const BotanyPlantCountEntryV1 = Type.Object(
  {
    kind: Type.String({ minLength: 1 }),
    count: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type BotanyPlantCountEntryV1 = Static<typeof BotanyPlantCountEntryV1>;

export const BotanyVariantCountEntryV1 = Type.Object(
  {
    variant: BotanyVariantV1,
    count: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type BotanyVariantCountEntryV1 = Static<
  typeof BotanyVariantCountEntryV1
>;

export const BotanyZoneEcologyV1 = Type.Object(
  {
    zone: Type.String({ minLength: 1 }),
    spirit_qi: Type.Number({ minimum: -1, maximum: 1 }),
    plant_counts: Type.Array(BotanyPlantCountEntryV1),
    variant_counts: Type.Array(BotanyVariantCountEntryV1),
  },
  { additionalProperties: false },
);
export type BotanyZoneEcologyV1 = Static<typeof BotanyZoneEcologyV1>;

export const BotanyEcologySnapshotV1 = Type.Object(
  {
    v: Type.Literal(1),
    tick: Type.Integer({ minimum: 0 }),
    zones: Type.Array(BotanyZoneEcologyV1),
  },
  { additionalProperties: false },
);
export type BotanyEcologySnapshotV1 = Static<typeof BotanyEcologySnapshotV1>;

export function validateBotanyEcologySnapshotV1Contract(data: unknown): ValidationResult {
  return validate(BotanyEcologySnapshotV1, data);
}
