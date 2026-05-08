/**
 * plan-meridian-severed-v1 §1 P3 — 经脉永久 SEVERED 事件 IPC schema。
 *
 * 7 类来源（VoluntarySever / BackfireOverload / OverloadTear / CombatWound /
 * TribulationFail / DuguDistortion / Other）统一经 `bong:meridian_severed`
 * 通道发布，由 agent 端 narration runtime 订阅 → 渲染叙事 → 发布到 AGENT_NARRATE。
 *
 * server 端 component 是 `cultivation::meridian::severed::MeridianSeveredPermanent`，
 * 与本 schema 字段一一对应；`source` 字段在两侧序列化形态保持一致以便双端校验。
 * 双端 sample 文件（`samples/meridian-severed-event.sample.json`）+ schema-registry
 * 注册留给首个 runtime 调用方（plan-yidao-v1 / 各 v2 流派 plan）接入时一起补，
 * 避免本 plan 独建 sample 但下游零调用方。
 */

import { Type, type Static } from "@sinclair/typebox";

import { MeridianId } from "./cultivation.js";
import { validate, type ValidationResult } from "./validate.js";

/** SEVERED 来源 7 类（与 server SeveredSource enum 对齐）。 */
export const SeveredSource = Type.Union(
  [
    Type.Literal("VoluntarySever"),
    Type.Literal("BackfireOverload"),
    Type.Literal("OverloadTear"),
    Type.Literal("CombatWound"),
    Type.Literal("TribulationFail"),
    Type.Literal("DuguDistortion"),
    Type.Object(
      { Other: Type.String({ minLength: 1, maxLength: 64 }) },
      { additionalProperties: false },
    ),
  ],
  { description: "SEVERED 来源 7 类（plan §4），Other 用于扩展未预见来源" },
);
export type SeveredSource = Static<typeof SeveredSource>;

/** plan §1 P3 — `MeridianSeveredEvent` Redis 通道载荷。 */
export const MeridianSeveredEventV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("meridian_severed"),
    /** 玩家或 NPC 的 wire id（"player:<uuid>" 或 "entity:<bits>"） */
    entity_id: Type.String({ minLength: 1, maxLength: 128 }),
    /** 哪条经脉断了 */
    meridian_id: MeridianId,
    /** 来源 7 类 */
    source: SeveredSource,
    /** 服务端 tick 时戳 */
    at_tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type MeridianSeveredEventV1 = Static<typeof MeridianSeveredEventV1>;

export function validateMeridianSeveredEventV1Contract(data: unknown): ValidationResult {
  return validate(MeridianSeveredEventV1, data);
}
