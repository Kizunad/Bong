import { type Static, Type } from "@sinclair/typebox";

import { type ValidationResult, validate } from "./validate.js";

const EntityId = Type.String({ minLength: 1 });
const MeridianIdText = Type.String({ minLength: 1 });

export const DuguPoisonStateV1 = Type.Object(
  {
    target: EntityId,
    active: Type.Boolean(),
    meridian_id: Type.String(),
    attacker: Type.String(),
    attached_at_tick: Type.Integer({ minimum: 0 }),
    poisoner_realm_tier: Type.Integer({ minimum: 0, maximum: 5 }),
    loss_per_tick: Type.Number({ minimum: 0 }),
    flow_capacity_after: Type.Number({ minimum: 0 }),
    qi_max_after: Type.Number({ minimum: 0 }),
    server_tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type DuguPoisonStateV1 = Static<typeof DuguPoisonStateV1>;

export const DuguPoisonProgressEventV1 = Type.Object(
  {
    target: EntityId,
    attacker: EntityId,
    meridian_id: MeridianIdText,
    flow_capacity_after: Type.Number({ minimum: 0 }),
    qi_max_after: Type.Number({ minimum: 0 }),
    actual_loss_this_tick: Type.Number({ minimum: 0 }),
    tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type DuguPoisonProgressEventV1 = Static<typeof DuguPoisonProgressEventV1>;

export const DuguObfuscationStateV1 = Type.Object(
  {
    entity: EntityId,
    active: Type.Boolean(),
    disrupted_until_tick: Type.Optional(Type.Integer({ minimum: 0 })),
    server_tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type DuguObfuscationStateV1 = Static<typeof DuguObfuscationStateV1>;

export const AntidoteResultV1 = Type.Union([
  Type.Literal("success"),
  Type.Literal("failed"),
]);
export type AntidoteResultV1 = Static<typeof AntidoteResultV1>;

export const AntidoteResultEventV1 = Type.Object(
  {
    healer: EntityId,
    target: EntityId,
    result: AntidoteResultV1,
    meridian_id: MeridianIdText,
    qi_max_after: Type.Number({ minimum: 0 }),
    tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type AntidoteResultEventV1 = Static<typeof AntidoteResultEventV1>;

export const DuguRevealedEventV1 = Type.Object(
  {
    revealed_player: EntityId,
    witness: EntityId,
    witness_realm: Type.String({ minLength: 1 }),
    at_position: Type.Tuple([Type.Number(), Type.Number(), Type.Number()]),
    at_tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type DuguRevealedEventV1 = Static<typeof DuguRevealedEventV1>;

export function validateDuguPoisonStateV1Contract(data: unknown): ValidationResult {
  return validate(DuguPoisonStateV1, data);
}

export function validateDuguPoisonProgressEventV1Contract(data: unknown): ValidationResult {
  return validate(DuguPoisonProgressEventV1, data);
}

export function validateDuguObfuscationStateV1Contract(data: unknown): ValidationResult {
  return validate(DuguObfuscationStateV1, data);
}

export function validateAntidoteResultEventV1Contract(data: unknown): ValidationResult {
  return validate(AntidoteResultEventV1, data);
}

export function validateDuguRevealedEventV1Contract(data: unknown): ValidationResult {
  return validate(DuguRevealedEventV1, data);
}
