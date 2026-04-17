import { Type, type Static } from "@sinclair/typebox";

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
