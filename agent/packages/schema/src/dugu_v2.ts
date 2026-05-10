import { type Static, Type } from "@sinclair/typebox";

import { type ValidationResult, validate } from "./validate.js";

export const DuguV2SkillIdV1 = Type.Union([
  Type.Literal("eclipse"),
  Type.Literal("self_cure"),
  Type.Literal("penetrate"),
  Type.Literal("shroud"),
  Type.Literal("reverse"),
]);
export type DuguV2SkillIdV1 = Static<typeof DuguV2SkillIdV1>;

export const DuguTaintTierV1 = Type.Union([
  Type.Literal("immediate"),
  Type.Literal("temporary"),
  Type.Literal("permanent"),
]);
export type DuguTaintTierV1 = Static<typeof DuguTaintTierV1>;

export const DuguV2SkillCastV1 = Type.Object(
  {
    caster: Type.String({ minLength: 1 }),
    target: Type.Optional(Type.String({ minLength: 1 })),
    skill: DuguV2SkillIdV1,
    tick: Type.Integer({ minimum: 0 }),
    taint_tier: Type.Optional(DuguTaintTierV1),
    hp_loss: Type.Number({ minimum: 0 }),
    qi_loss: Type.Number({ minimum: 0 }),
    qi_max_loss: Type.Number({ minimum: 0 }),
    permanent_decay_rate_per_min: Type.Number({ minimum: 0 }),
    returned_zone_qi: Type.Number({ minimum: 0 }),
    reveal_probability: Type.Number({ minimum: 0, maximum: 1 }),
    animation_id: Type.String({ minLength: 1 }),
    particle_id: Type.String({ minLength: 1 }),
    sound_recipe_id: Type.String({ minLength: 1 }),
    icon_texture: Type.String({ minLength: 1 }),
  },
  { additionalProperties: false },
);
export type DuguV2SkillCastV1 = Static<typeof DuguV2SkillCastV1>;

export const DuguSelfCureProgressV1 = Type.Object(
  {
    caster: Type.String({ minLength: 1 }),
    hours_used: Type.Number({ minimum: 0 }),
    daily_hours_after: Type.Number({ minimum: 0, maximum: 6 }),
    gain_percent: Type.Number({ minimum: 0 }),
    insidious_color_percent: Type.Number({ minimum: 0, maximum: 100 }),
    morphology_percent: Type.Number({ minimum: 0, maximum: 100 }),
    self_revealed: Type.Boolean(),
    tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type DuguSelfCureProgressV1 = Static<typeof DuguSelfCureProgressV1>;

export const DuguReverseTriggeredV1 = Type.Object(
  {
    caster: Type.String({ minLength: 1 }),
    affected_targets: Type.Integer({ minimum: 0 }),
    burst_damage: Type.Number({ minimum: 0 }),
    returned_zone_qi: Type.Number({ minimum: 0 }),
    juebi_delay_ticks: Type.Optional(Type.Integer({ minimum: 0 })),
    center: Type.Tuple([Type.Number(), Type.Number(), Type.Number()]),
    tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type DuguReverseTriggeredV1 = Static<typeof DuguReverseTriggeredV1>;

export function validateDuguV2SkillCastV1Contract(data: unknown): ValidationResult {
  return validate(DuguV2SkillCastV1, data);
}

export function validateDuguSelfCureProgressV1Contract(data: unknown): ValidationResult {
  return validate(DuguSelfCureProgressV1, data);
}

export function validateDuguReverseTriggeredV1Contract(data: unknown): ValidationResult {
  return validate(DuguReverseTriggeredV1, data);
}
