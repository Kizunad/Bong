import { Type, type Static } from "@sinclair/typebox";

export const ProcessingKindV1 = Type.Union(
  [
    Type.Literal("drying"),
    Type.Literal("grinding"),
    Type.Literal("forging_alchemy"),
    Type.Literal("extraction"),
  ],
  { description: "plan-lingtian-process-v1 §5.3 四类作物二级加工工艺" },
);
export type ProcessingKindV1 = Static<typeof ProcessingKindV1>;

export const ProcessingSessionDataV1 = Type.Object(
  {
    session_id: Type.String(),
    kind: ProcessingKindV1,
    recipe_id: Type.String(),
    progress_ticks: Type.Integer({ minimum: 0 }),
    duration_ticks: Type.Integer({ minimum: 0 }),
    player_id: Type.String(),
  },
  { additionalProperties: false },
);
export type ProcessingSessionDataV1 = Static<typeof ProcessingSessionDataV1>;

export const FreshnessUpdateV1 = Type.Object(
  {
    item_uuid: Type.String(),
    freshness: Type.Number({ minimum: 0, maximum: 1 }),
    profile_name: Type.String(),
  },
  { additionalProperties: false },
);
export type FreshnessUpdateV1 = Static<typeof FreshnessUpdateV1>;

export const ServerDataProcessingSessionV1 = Type.Intersect([
  Type.Object({ type: Type.Literal("processing_session"), v: Type.Literal(1) }),
  ProcessingSessionDataV1,
]);
export type ServerDataProcessingSessionV1 = Static<typeof ServerDataProcessingSessionV1>;

export const ServerDataFreshnessUpdateV1 = Type.Intersect([
  Type.Object({ type: Type.Literal("freshness_update"), v: Type.Literal(1) }),
  FreshnessUpdateV1,
]);
export type ServerDataFreshnessUpdateV1 = Static<typeof ServerDataFreshnessUpdateV1>;
