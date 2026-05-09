import { type Static, Type } from "@sinclair/typebox";

import { type ValidationResult, validate } from "./validate.js";

export const VortexFieldStateV1 = Type.Object(
  {
    caster: Type.String({ minLength: 1 }),
    active: Type.Boolean(),
    center: Type.Tuple([Type.Number(), Type.Number(), Type.Number()]),
    radius: Type.Number({ minimum: 0 }),
    delta: Type.Number({ minimum: 0, maximum: 0.8 }),
    env_qi_at_cast: Type.Number({ minimum: -1, maximum: 1 }),
    maintain_remaining_ticks: Type.Integer({ minimum: 0 }),
    intercepted_count: Type.Integer({ minimum: 0 }),
    active_skill_id: Type.String(),
    charge_progress: Type.Number({ minimum: 0, maximum: 1 }),
    cooldown_until_ms: Type.Integer({ minimum: 0 }),
    backfire_level: Type.String(),
    turbulence_radius: Type.Number({ minimum: 0 }),
    turbulence_intensity: Type.Number({ minimum: 0, maximum: 1 }),
    turbulence_until_ms: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type VortexFieldStateV1 = Static<typeof VortexFieldStateV1>;

export const VortexBackfireCauseV1 = Type.Union([
  Type.Literal("env_qi_too_low"),
  Type.Literal("exceed_maintain_max"),
  Type.Literal("exceed_delta_cap"),
]);
export type VortexBackfireCauseV1 = Static<typeof VortexBackfireCauseV1>;

export const VortexBackfireEventV1 = Type.Object(
  {
    caster: Type.String({ minLength: 1 }),
    cause: VortexBackfireCauseV1,
    meridian_severed: Type.String({ minLength: 1 }),
    tick: Type.Integer({ minimum: 0 }),
    env_qi: Type.Number({ minimum: -1, maximum: 1 }),
    delta: Type.Number({ minimum: 0, maximum: 0.8 }),
    resisted: Type.Boolean(),
  },
  { additionalProperties: false },
);
export type VortexBackfireEventV1 = Static<typeof VortexBackfireEventV1>;

export const ProjectileQiDrainedEventV1 = Type.Object(
  {
    field_caster: Type.String({ minLength: 1 }),
    projectile: Type.String({ minLength: 1 }),
    owner: Type.Optional(Type.String({ minLength: 1 })),
    drained_amount: Type.Number({ minimum: 0 }),
    remaining_payload: Type.Number({ minimum: 0 }),
    delta: Type.Number({ minimum: 0, maximum: 0.8 }),
    tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type ProjectileQiDrainedEventV1 = Static<typeof ProjectileQiDrainedEventV1>;

export function validateVortexFieldStateV1Contract(data: unknown): ValidationResult {
  return validate(VortexFieldStateV1, data);
}

export function validateVortexBackfireEventV1Contract(data: unknown): ValidationResult {
  return validate(VortexBackfireEventV1, data);
}

export function validateProjectileQiDrainedEventV1Contract(data: unknown): ValidationResult {
  return validate(ProjectileQiDrainedEventV1, data);
}
