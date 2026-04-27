import { Type, type Static } from "@sinclair/typebox";

import { validate, type ValidationResult } from "./validate.js";

/** plan-tsy-hostile-v1 §6 — TSY 敌对 NPC archetype wire 名称。 */
export const TsyHostileArchetypeV1 = Type.Union([
  Type.Literal("daoxiang"),
  Type.Literal("zhinian"),
  Type.Literal("guardian_relic_sentinel"),
  Type.Literal("fuya"),
]);
export type TsyHostileArchetypeV1 = Static<typeof TsyHostileArchetypeV1>;

/** plan-tsy-hostile-v1 §6 — TSY hostile spawn 汇总事件。
 *
 * Server → Agent 单向；agent narration 后续按 `archetype` 和 `count` 做秘境威胁播报。
 */
export const TsyNpcSpawnedV1 = Type.Object(
  {
    v: Type.Literal(1),
    kind: Type.Literal("tsy_npc_spawned"),
    family_id: Type.String({ minLength: 1 }),
    archetype: TsyHostileArchetypeV1,
    count: Type.Integer({ minimum: 0 }),
    at_tick: Type.Number({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type TsyNpcSpawnedV1 = Static<typeof TsyNpcSpawnedV1>;

export function validateTsyNpcSpawnedV1Contract(data: unknown): ValidationResult {
  return validate(TsyNpcSpawnedV1, data);
}

/** plan-tsy-hostile-v1 §6 — 秘境守灵阶段变化事件。
 *
 * `container_entity_id` 使用 server ECS entity 序号，保持 plan 约定的 number 形态。
 */
export const TsySentinelPhaseChangedV1 = Type.Object(
  {
    v: Type.Literal(1),
    kind: Type.Literal("tsy_sentinel_phase_changed"),
    family_id: Type.String({ minLength: 1 }),
    container_entity_id: Type.Number({ minimum: 0 }),
    phase: Type.Integer({ minimum: 0 }),
    max_phase: Type.Integer({ minimum: 1 }),
    at_tick: Type.Number({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type TsySentinelPhaseChangedV1 = Static<typeof TsySentinelPhaseChangedV1>;

export function validateTsySentinelPhaseChangedV1Contract(
  data: unknown,
): ValidationResult {
  return validate(TsySentinelPhaseChangedV1, data);
}
