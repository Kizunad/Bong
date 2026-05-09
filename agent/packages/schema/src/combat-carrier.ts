import { type Static, Type } from "@sinclair/typebox";

import { type ValidationResult, validate } from "./validate.js";

export const CarrierKindV1 = Type.Union([
  Type.Literal("bone_chip"),
  Type.Literal("yibian_shougu"),
  Type.Literal("lingmu_arrow"),
  Type.Literal("dyed_bone"),
  Type.Literal("fenglinghe_bone"),
  Type.Literal("shanggu_bone"),
]);
export type CarrierKindV1 = Static<typeof CarrierKindV1>;

export const CarrierChargePhaseV1 = Type.Union([
  Type.Literal("idle"),
  Type.Literal("charging"),
  Type.Literal("charged"),
]);
export type CarrierChargePhaseV1 = Static<typeof CarrierChargePhaseV1>;

export const CarrierStateV1 = Type.Object(
  {
    carrier: Type.String({ minLength: 1 }),
    phase: CarrierChargePhaseV1,
    progress: Type.Number({ minimum: 0, maximum: 1 }),
    sealed_qi: Type.Number({ minimum: 0 }),
    sealed_qi_initial: Type.Number({ minimum: 0 }),
    half_life_remaining_ticks: Type.Integer({ minimum: 0 }),
    item_instance_id: Type.Optional(Type.Integer({ minimum: 0 })),
  },
  { additionalProperties: false },
);
export type CarrierStateV1 = Static<typeof CarrierStateV1>;

export const CarrierChargedEventV1 = Type.Object(
  {
    carrier: Type.String({ minLength: 1 }),
    instance_id: Type.Integer({ minimum: 0 }),
    qi_amount: Type.Number({ minimum: 0 }),
    qi_color: Type.String({ minLength: 1 }),
    full_charge: Type.Boolean(),
    tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type CarrierChargedEventV1 = Static<typeof CarrierChargedEventV1>;

export const CarrierImpactEventV1 = Type.Object(
  {
    attacker: Type.String({ minLength: 1 }),
    target: Type.String({ minLength: 1 }),
    carrier_kind: CarrierKindV1,
    hit_distance: Type.Number({ minimum: 0 }),
    sealed_qi_initial: Type.Number({ minimum: 0 }),
    hit_qi: Type.Number({ minimum: 0 }),
    wound_damage: Type.Number({ minimum: 0 }),
    contam_amount: Type.Number({ minimum: 0 }),
    tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type CarrierImpactEventV1 = Static<typeof CarrierImpactEventV1>;

export const ProjectileDespawnReasonV1 = Type.Union([
  Type.Literal("hit_target"),
  Type.Literal("hit_block"),
  Type.Literal("out_of_range"),
  Type.Literal("natural_decay"),
]);
export type ProjectileDespawnReasonV1 = Static<typeof ProjectileDespawnReasonV1>;

export const ProjectileDespawnedEventV1 = Type.Object(
  {
    owner: Type.Optional(Type.String({ minLength: 1 })),
    projectile: Type.String({ minLength: 1 }),
    reason: ProjectileDespawnReasonV1,
    distance: Type.Number({ minimum: 0 }),
    qi_evaporated: Type.Number({ minimum: 0 }),
    residual_qi: Type.Number({ minimum: 0 }),
    pos: Type.Tuple([Type.Number(), Type.Number(), Type.Number()]),
    tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type ProjectileDespawnedEventV1 = Static<typeof ProjectileDespawnedEventV1>;

export const AnqiSkillKindV1 = Type.Union([
  Type.Literal("single_snipe"),
  Type.Literal("multi_shot"),
  Type.Literal("soul_inject"),
  Type.Literal("armor_pierce"),
  Type.Literal("echo_fractal"),
]);
export type AnqiSkillKindV1 = Static<typeof AnqiSkillKindV1>;

export const AnqiContainerKindV1 = Type.Union([
  Type.Literal("hand_slot"),
  Type.Literal("quiver"),
  Type.Literal("pocket_pouch"),
  Type.Literal("fenglinghe"),
]);
export type AnqiContainerKindV1 = Static<typeof AnqiContainerKindV1>;

export const MultiShotEventV1 = Type.Object(
  {
    caster: Type.String({ minLength: 1 }),
    carrier_kind: CarrierKindV1,
    projectile_count: Type.Integer({ minimum: 1 }),
    cone_degrees: Type.Number({ minimum: 0 }),
    tracking_degrees: Type.Number({ minimum: 0 }),
    tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type MultiShotEventV1 = Static<typeof MultiShotEventV1>;

export const QiInjectionEventV1 = Type.Object(
  {
    caster: Type.String({ minLength: 1 }),
    target: Type.Optional(Type.Union([Type.String({ minLength: 1 }), Type.Null()])),
    skill: AnqiSkillKindV1,
    carrier_kind: CarrierKindV1,
    payload_qi: Type.Number({ minimum: 0 }),
    wound_qi: Type.Number({ minimum: 0 }),
    contamination_qi: Type.Number({ minimum: 0 }),
    overload_ratio: Type.Number({ minimum: 0 }),
    triggers_overload_tear: Type.Boolean(),
    tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type QiInjectionEventV1 = Static<typeof QiInjectionEventV1>;

export const EchoFractalEventV1 = Type.Object(
  {
    caster: Type.String({ minLength: 1 }),
    carrier_kind: CarrierKindV1,
    local_qi_density: Type.Number({ minimum: 0 }),
    threshold: Type.Number({ minimum: 0 }),
    echo_count: Type.Integer({ minimum: 1 }),
    damage_per_echo: Type.Number({ minimum: 0 }),
    tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type EchoFractalEventV1 = Static<typeof EchoFractalEventV1>;

export const CarrierAbrasionEventV1 = Type.Object(
  {
    carrier: Type.String({ minLength: 1 }),
    container: AnqiContainerKindV1,
    direction: Type.Union([Type.Literal("store"), Type.Literal("draw")]),
    lost_qi: Type.Number({ minimum: 0 }),
    after_qi: Type.Number({ minimum: 0 }),
    tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type CarrierAbrasionEventV1 = Static<typeof CarrierAbrasionEventV1>;

export const ContainerSwapEventV1 = Type.Object(
  {
    carrier: Type.String({ minLength: 1 }),
    from: AnqiContainerKindV1,
    to: AnqiContainerKindV1,
    switching_until_tick: Type.Integer({ minimum: 0 }),
    tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type ContainerSwapEventV1 = Static<typeof ContainerSwapEventV1>;

export function validateCarrierStateV1Contract(data: unknown): ValidationResult {
  return validate(CarrierStateV1, data);
}

export function validateCarrierChargedEventV1Contract(data: unknown): ValidationResult {
  return validate(CarrierChargedEventV1, data);
}

export function validateCarrierImpactEventV1Contract(data: unknown): ValidationResult {
  return validate(CarrierImpactEventV1, data);
}

export function validateProjectileDespawnedEventV1Contract(data: unknown): ValidationResult {
  return validate(ProjectileDespawnedEventV1, data);
}

export function validateMultiShotEventV1Contract(data: unknown): ValidationResult {
  return validate(MultiShotEventV1, data);
}

export function validateQiInjectionEventV1Contract(data: unknown): ValidationResult {
  return validate(QiInjectionEventV1, data);
}

export function validateEchoFractalEventV1Contract(data: unknown): ValidationResult {
  return validate(EchoFractalEventV1, data);
}

export function validateCarrierAbrasionEventV1Contract(data: unknown): ValidationResult {
  return validate(CarrierAbrasionEventV1, data);
}

export function validateContainerSwapEventV1Contract(data: unknown): ValidationResult {
  return validate(ContainerSwapEventV1, data);
}
