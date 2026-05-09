import { Type, type Static } from "@sinclair/typebox";

import { MeridianId } from "./cultivation.js";
import { validate, type ValidationResult } from "./validate.js";

export const YidaoSkillIdV1 = Type.Union([
  Type.Literal("meridian_repair"),
  Type.Literal("contam_purge"),
  Type.Literal("emergency_resuscitate"),
  Type.Literal("life_extension"),
  Type.Literal("mass_meridian_repair"),
]);
export type YidaoSkillIdV1 = Static<typeof YidaoSkillIdV1>;

export const YidaoEventKindV1 = Type.Union([
  Type.Literal("meridian_heal"),
  Type.Literal("contam_purge"),
  Type.Literal("emergency_resuscitate"),
  Type.Literal("life_extension"),
  Type.Literal("mass_heal"),
  Type.Literal("karma_accumulation"),
  Type.Literal("medical_contract"),
]);
export type YidaoEventKindV1 = Static<typeof YidaoEventKindV1>;

export const MedicalContractStateV1 = Type.Union([
  Type.Literal("stranger"),
  Type.Literal("patient"),
  Type.Literal("long_term_patient"),
  Type.Literal("bonded"),
]);
export type MedicalContractStateV1 = Static<typeof MedicalContractStateV1>;

export const YidaoEventV1 = Type.Object(
  {
    v: Type.Literal(1),
    kind: YidaoEventKindV1,
    tick: Type.Integer({ minimum: 0 }),
    medic_id: Type.String({ minLength: 1, maxLength: 128 }),
    patient_ids: Type.Array(Type.String({ minLength: 1, maxLength: 128 })),
    skill: YidaoSkillIdV1,
    meridian_id: Type.Optional(MeridianId),
    success_count: Type.Integer({ minimum: 0 }),
    failure_count: Type.Integer({ minimum: 0 }),
    qi_transferred: Type.Number({ minimum: 0 }),
    contam_reduced: Type.Number({ minimum: 0 }),
    hp_restored: Type.Number({ minimum: 0 }),
    karma_delta: Type.Number({ minimum: 0 }),
    medic_qi_max_delta: Type.Number({ maximum: 0 }),
    patient_qi_max_delta: Type.Number({ maximum: 0 }),
    contract_state: Type.Optional(MedicalContractStateV1),
    detail: Type.String({ minLength: 1, maxLength: 256 }),
  },
  { additionalProperties: false },
);
export type YidaoEventV1 = Static<typeof YidaoEventV1>;

export const HealerNpcAiStateV1 = Type.Object(
  {
    healer_id: Type.String({ minLength: 1, maxLength: 128 }),
    active_action: Type.String({ minLength: 1, maxLength: 64 }),
    queue_len: Type.Integer({ minimum: 0 }),
    reputation: Type.Integer(),
    retreating: Type.Boolean(),
  },
  { additionalProperties: false },
);
export type HealerNpcAiStateV1 = Static<typeof HealerNpcAiStateV1>;

export const YidaoHudStateV1 = Type.Object(
  {
    healer_id: Type.String({ minLength: 1, maxLength: 128 }),
    reputation: Type.Integer(),
    peace_mastery: Type.Number({ minimum: 0, maximum: 100 }),
    karma: Type.Number({ minimum: 0 }),
    active_skill: Type.Union([YidaoSkillIdV1, Type.Null()]),
    patient_ids: Type.Array(Type.String({ minLength: 1, maxLength: 128 })),
    patient_hp_percent: Type.Union([
      Type.Number({ minimum: 0, maximum: 1 }),
      Type.Null(),
    ]),
    patient_contam_total: Type.Union([Type.Number({ minimum: 0 }), Type.Null()]),
    severed_meridian_count: Type.Integer({ minimum: 0 }),
    contract_count: Type.Integer({ minimum: 0 }),
    mass_preview_count: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type YidaoHudStateV1 = Static<typeof YidaoHudStateV1>;

export function validateYidaoEventV1Contract(data: unknown): ValidationResult {
  return validate(YidaoEventV1, data);
}

export function validateHealerNpcAiStateV1Contract(data: unknown): ValidationResult {
  return validate(HealerNpcAiStateV1, data);
}

export function validateYidaoHudStateV1Contract(data: unknown): ValidationResult {
  return validate(YidaoHudStateV1, data);
}
