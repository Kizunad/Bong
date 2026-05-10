import { type Static, Type } from "@sinclair/typebox";

import { MeridianId } from "./cultivation.js";
import { type ValidationResult, validate } from "./validate.js";

export const BaomaiSkillIdV1 = Type.Union([
  Type.Literal("beng_quan"),
  Type.Literal("full_power_charge"),
  Type.Literal("full_power_release"),
  Type.Literal("mountain_shake"),
  Type.Literal("blood_burn"),
  Type.Literal("disperse"),
]);
export type BaomaiSkillIdV1 = Static<typeof BaomaiSkillIdV1>;

export const BaomaiSkillEventV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("baomai_skill_event"),
    skill_id: BaomaiSkillIdV1,
    caster_id: Type.String({ minLength: 1 }),
    target_id: Type.Optional(Type.String({ minLength: 1 })),
    tick: Type.Integer({ minimum: 0 }),
    qi_invested: Type.Number({ minimum: 0 }),
    damage: Type.Number({ minimum: 0 }),
    radius_blocks: Type.Optional(Type.Number({ minimum: 0 })),
    blood_multiplier: Type.Number({ minimum: 1 }),
    flow_rate_multiplier: Type.Number({ minimum: 1 }),
    meridian_ids: Type.Array(MeridianId),
  },
  { additionalProperties: false },
);
export type BaomaiSkillEventV1 = Static<typeof BaomaiSkillEventV1>;

export function validateBaomaiSkillEventV1Contract(data: unknown): ValidationResult {
  return validate(BaomaiSkillEventV1, data);
}
