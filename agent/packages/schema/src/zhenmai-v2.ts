import { type Static, Type } from "@sinclair/typebox";

import { MeridianId } from "./cultivation.js";
import { type ValidationResult, validate } from "./validate.js";

export const ZhenmaiSkillIdV1 = Type.Union([
  Type.Literal("parry"),
  Type.Literal("neutralize"),
  Type.Literal("multipoint"),
  Type.Literal("harden_meridian"),
  Type.Literal("sever_chain"),
]);
export type ZhenmaiSkillIdV1 = Static<typeof ZhenmaiSkillIdV1>;

export const ZhenmaiAttackKindV1 = Type.Union([
  Type.Literal("real_yuan"),
  Type.Literal("physical_carrier"),
  Type.Literal("tainted_yuan"),
  Type.Literal("array"),
]);
export type ZhenmaiAttackKindV1 = Static<typeof ZhenmaiAttackKindV1>;

export const ZhenmaiSkillEventV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("zhenmai_skill_event"),
    skill_id: ZhenmaiSkillIdV1,
    caster_id: Type.String({ minLength: 1 }),
    target_id: Type.Optional(Type.String({ minLength: 1 })),
    meridian_id: Type.Optional(MeridianId),
    meridian_ids: Type.Optional(Type.Array(MeridianId, { minItems: 1 })),
    attack_kind: Type.Optional(ZhenmaiAttackKindV1),
    reflected_qi: Type.Optional(Type.Number({ minimum: 0 })),
    k_drain: Type.Optional(Type.Number({ minimum: 0 })),
    self_damage_multiplier: Type.Optional(Type.Number({ minimum: 0, maximum: 1 })),
    grants_amplification: Type.Optional(Type.Boolean()),
    expires_at_tick: Type.Optional(Type.Integer({ minimum: 0 })),
    tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type ZhenmaiSkillEventV1 = Static<typeof ZhenmaiSkillEventV1>;

export function validateZhenmaiSkillEventV1Contract(data: unknown): ValidationResult {
  return validate(ZhenmaiSkillEventV1, data);
}
