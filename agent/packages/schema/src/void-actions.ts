/**
 * plan-void-actions-v1 化虚专属 action IPC schema。
 * 与 server/src/schema/void_actions.rs 对齐。
 */
import { Type, type Static } from "@sinclair/typebox";
import { validate, type ValidationResult } from "./validate.js";

const JS_SAFE_INTEGER_MAX = Number.MAX_SAFE_INTEGER;

export const VoidActionKindV1 = Type.Union([
  Type.Literal("suppress_tsy"),
  Type.Literal("explode_zone"),
  Type.Literal("barrier"),
  Type.Literal("legacy_assign"),
]);
export type VoidActionKindV1 = Static<typeof VoidActionKindV1>;

export const VoidActionCostV1 = Type.Object(
  {
    qi: Type.Number({ minimum: 0 }),
    lifespan_years: Type.Integer({ minimum: 0 }),
    cooldown_ticks: Type.Integer({ minimum: 0, maximum: JS_SAFE_INTEGER_MAX }),
  },
  { additionalProperties: false },
);
export type VoidActionCostV1 = Static<typeof VoidActionCostV1>;

export const BarrierGeometryV1 = Type.Union([
  Type.Object(
    {
      kind: Type.Literal("circle"),
      center: Type.Tuple([Type.Number(), Type.Number(), Type.Number()]),
      radius: Type.Number({ minimum: 0 }),
    },
    { additionalProperties: false },
  ),
]);
export type BarrierGeometryV1 = Static<typeof BarrierGeometryV1>;

export const VoidActionRequestV1 = Type.Union([
  Type.Object(
    {
      kind: Type.Literal("suppress_tsy"),
      zone_id: Type.String({ minLength: 1, maxLength: 128 }),
    },
    { additionalProperties: false },
  ),
  Type.Object(
    {
      kind: Type.Literal("explode_zone"),
      zone_id: Type.String({ minLength: 1, maxLength: 128 }),
    },
    { additionalProperties: false },
  ),
  Type.Object(
    {
      kind: Type.Literal("barrier"),
      zone_id: Type.String({ minLength: 1, maxLength: 128 }),
      geometry: BarrierGeometryV1,
    },
    { additionalProperties: false },
  ),
  Type.Object(
    {
      kind: Type.Literal("legacy_assign"),
      inheritor_id: Type.String({ minLength: 1, maxLength: 128 }),
      item_instance_ids: Type.Optional(
        Type.Array(Type.Integer({ minimum: 0, maximum: JS_SAFE_INTEGER_MAX })),
      ),
      message: Type.Optional(Type.Union([Type.String({ maxLength: 512 }), Type.Null()])),
    },
    { additionalProperties: false },
  ),
]);
export type VoidActionRequestV1 = Static<typeof VoidActionRequestV1>;

export const VoidActionResponseV1 = Type.Object(
  {
    v: Type.Literal(1),
    accepted: Type.Boolean(),
    kind: VoidActionKindV1,
    reason: Type.String({ minLength: 1 }),
    cooldown_until_tick: Type.Optional(
      Type.Union([Type.Integer({ minimum: 0, maximum: JS_SAFE_INTEGER_MAX }), Type.Null()]),
    ),
    cost: Type.Optional(Type.Union([VoidActionCostV1, Type.Null()])),
  },
  { additionalProperties: false },
);
export type VoidActionResponseV1 = Static<typeof VoidActionResponseV1>;

export const VoidActionBroadcastV1 = Type.Object(
  {
    v: Type.Literal(1),
    kind: VoidActionKindV1,
    actor_id: Type.String({ minLength: 1, maxLength: 128 }),
    actor_name: Type.String({ minLength: 1, maxLength: 128 }),
    target: Type.String({ minLength: 1, maxLength: 128 }),
    at_tick: Type.Integer({ minimum: 0, maximum: JS_SAFE_INTEGER_MAX }),
    qi_cost: Type.Number({ minimum: 0 }),
    lifespan_cost_years: Type.Integer({ minimum: 0 }),
    scope: Type.Literal("broadcast"),
    public_text: Type.String({ minLength: 1, maxLength: 512 }),
  },
  { additionalProperties: false },
);
export type VoidActionBroadcastV1 = Static<typeof VoidActionBroadcastV1>;

export const VoidActionCooldownV1 = Type.Object(
  {
    kind: VoidActionKindV1,
    ready_at_tick: Type.Integer({ minimum: 0, maximum: JS_SAFE_INTEGER_MAX }),
  },
  { additionalProperties: false },
);
export type VoidActionCooldownV1 = Static<typeof VoidActionCooldownV1>;

export const VoidActionStateV1 = Type.Object(
  {
    v: Type.Literal(1),
    actor_id: Type.String({ minLength: 1, maxLength: 128 }),
    cooldowns: Type.Array(VoidActionCooldownV1),
  },
  { additionalProperties: false },
);
export type VoidActionStateV1 = Static<typeof VoidActionStateV1>;

export function validateVoidActionBroadcastV1Contract(data: unknown): ValidationResult {
  return validate(VoidActionBroadcastV1, data);
}

export function validateVoidActionRequestV1Contract(data: unknown): ValidationResult {
  return validate(VoidActionRequestV1, data);
}
