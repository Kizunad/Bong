import { type Static, Type } from "@sinclair/typebox";

import { type ValidationResult, validate } from "./validate.js";

const EntityId = Type.Integer({ minimum: 0, maximum: Number.MAX_SAFE_INTEGER });

/** 变异阶段名称 — 对齐 server MutationStage 枚举。 */
export const MutationStageV1 = Type.Union([
  Type.Literal("none"),
  Type.Literal("subtle"),
  Type.Literal("visible"),
  Type.Literal("heavy"),
  Type.Literal("bestial"),
]);
export type MutationStageV1 = Static<typeof MutationStageV1>;

/**
 * Server → Agent: 变异阶段推进事件（plan-dandao-path-v1 §6.4）。
 *
 * 阶段 3+（heavy / bestial）触发天道 narration。
 */
export const MutationEventV1 = Type.Object(
  {
    v: Type.Literal(1),
    entity_id: EntityId,
    from_stage: MutationStageV1,
    to_stage: MutationStageV1,
    cumulative_toxin: Type.Number({ minimum: 0 }),
    at_tick: Type.Integer({ minimum: 0, maximum: Number.MAX_SAFE_INTEGER }),
  },
  { additionalProperties: false },
);
export type MutationEventV1 = Static<typeof MutationEventV1>;

export function validateMutationEventV1(data: unknown): ValidationResult {
  return validate(MutationEventV1, data);
}
