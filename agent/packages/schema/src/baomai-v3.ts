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

// ─── plan-combat-skill-feedback-bridges-v1 P2 — 爆脉 v3 残余事件 ─────────────

/** 山震 — 震波 AoE hit 事件（server → agent） */
export const BaomaiV3MountainShakeV1 = Type.Object(
  {
    v: Type.Literal(1),
    caster_id: Type.String({ minLength: 1 }),
    affected_count: Type.Integer({ minimum: 0 }),
    tick: Type.Integer({ minimum: 0 }),
    qi_spent: Type.Number({ minimum: 0 }),
    radius_blocks: Type.Number({ minimum: 0 }),
    shock_damage: Type.Number({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type BaomaiV3MountainShakeV1 = Static<typeof BaomaiV3MountainShakeV1>;

export function validateBaomaiV3MountainShakeV1(data: unknown): ValidationResult {
  return validate(BaomaiV3MountainShakeV1, data);
}

/** 血燃 — HP 燃换真元倍率事件（server → agent） */
export const BaomaiV3BloodBurnV1 = Type.Object(
  {
    v: Type.Literal(1),
    caster_id: Type.String({ minLength: 1 }),
    tick: Type.Integer({ minimum: 0 }),
    hp_burned: Type.Number({ minimum: 0 }),
    qi_multiplier: Type.Number({ minimum: 1 }),
    active_until_tick: Type.Integer({ minimum: 0 }),
    ended_in_near_death: Type.Boolean(),
  },
  { additionalProperties: false },
);
export type BaomaiV3BloodBurnV1 = Static<typeof BaomaiV3BloodBurnV1>;

export function validateBaomaiV3BloodBurnV1(data: unknown): ValidationResult {
  return validate(BaomaiV3BloodBurnV1, data);
}

/** 超越到期 — body transcendence 窗口自然结束（server → agent） */
export const BaomaiV3TranscendenceExpiredV1 = Type.Object(
  {
    v: Type.Literal(1),
    caster_id: Type.String({ minLength: 1 }),
    tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type BaomaiV3TranscendenceExpiredV1 = Static<typeof BaomaiV3TranscendenceExpiredV1>;

export function validateBaomaiV3TranscendenceExpiredV1(data: unknown): ValidationResult {
  return validate(BaomaiV3TranscendenceExpiredV1, data);
}

/** 过载涟漪 — 经脉过载裂纹扩散事件（server → agent） */
export const BaomaiV3OverloadRippleV1 = Type.Object(
  {
    v: Type.Literal(1),
    caster_id: Type.String({ minLength: 1 }),
    tick: Type.Integer({ minimum: 0 }),
    skill_id: BaomaiSkillIdV1,
    severity_delta: Type.Number({ minimum: 0 }),
    total_severity: Type.Number({ minimum: 0 }),
    meridian_ids: Type.Array(Type.String({ minLength: 1 })),
  },
  { additionalProperties: false },
);
export type BaomaiV3OverloadRippleV1 = Static<typeof BaomaiV3OverloadRippleV1>;

export function validateBaomaiV3OverloadRippleV1(data: unknown): ValidationResult {
  return validate(BaomaiV3OverloadRippleV1, data);
}
