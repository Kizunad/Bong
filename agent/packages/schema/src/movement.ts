import { Type, type Static } from "@sinclair/typebox";

export const MovementActionRequestV1 = Type.Union([
  Type.Literal("dash"),
  Type.Literal("slide"),
  Type.Literal("double_jump"),
]);
export type MovementActionRequestV1 = Static<typeof MovementActionRequestV1>;

export const MovementActionV1 = Type.Union([
  Type.Literal("none"),
  Type.Literal("dashing"),
  Type.Literal("sliding"),
  Type.Literal("double_jumping"),
]);
export type MovementActionV1 = Static<typeof MovementActionV1>;

export const MovementZoneKindV1 = Type.Union([
  Type.Literal("normal"),
  Type.Literal("dead"),
  Type.Literal("negative"),
  Type.Literal("residue_ash"),
]);
export type MovementZoneKindV1 = Static<typeof MovementZoneKindV1>;

export const MovementStateV1 = Type.Object(
  {
    current_speed_multiplier: Type.Number({ minimum: 0 }),
    stamina_cost_active: Type.Boolean(),
    movement_action: MovementActionV1,
    zone_kind: MovementZoneKindV1,
    dash_cooldown_remaining_ticks: Type.Integer({ minimum: 0 }),
    slide_cooldown_remaining_ticks: Type.Integer({ minimum: 0 }),
    double_jump_charges_remaining: Type.Integer({ minimum: 0 }),
    double_jump_charges_max: Type.Integer({ minimum: 0 }),
    hitbox_height_blocks: Type.Number({ minimum: 0 }),
    stamina_current: Type.Number({ minimum: 0 }),
    stamina_max: Type.Number({ exclusiveMinimum: 0 }),
    low_stamina: Type.Boolean(),
    last_action_tick: Type.Optional(Type.Integer({ minimum: 0 })),
    rejected_action: Type.Optional(MovementActionRequestV1),
  },
  { additionalProperties: false },
);
export type MovementStateV1 = Static<typeof MovementStateV1>;
