import { Type, type Static } from "@sinclair/typebox";

import { Realm } from "./cultivation.js";
import { validate, type ValidationResult } from "./validate.js";

const Vec3 = Type.Tuple([Type.Number(), Type.Number(), Type.Number()]);

export const BreakthroughCinematicPhaseV1 = Type.Union([
  Type.Literal("prelude"),
  Type.Literal("charge"),
  Type.Literal("catalyze"),
  Type.Literal("apex"),
  Type.Literal("aftermath"),
]);
export type BreakthroughCinematicPhaseV1 = Static<typeof BreakthroughCinematicPhaseV1>;

export const BreakthroughCinematicOutcomeV1 = Type.Union([
  Type.Literal("pending"),
  Type.Literal("success"),
  Type.Literal("failure"),
  Type.Literal("interrupted"),
]);
export type BreakthroughCinematicOutcomeV1 = Static<typeof BreakthroughCinematicOutcomeV1>;

export const BreakthroughCinematicEventV1 = Type.Object(
  {
    v: Type.Literal(1),
    actor_id: Type.String({ minLength: 1, maxLength: 128 }),
    phase: BreakthroughCinematicPhaseV1,
    phase_tick: Type.Integer({ minimum: 0, maximum: 10000 }),
    phase_duration_ticks: Type.Integer({ minimum: 1, maximum: 10000 }),
    realm_from: Realm,
    realm_to: Realm,
    result: BreakthroughCinematicOutcomeV1,
    interrupted: Type.Boolean(),
    world_pos: Vec3,
    visible_radius_blocks: Type.Number({ minimum: 1, maximum: 10000 }),
    global: Type.Boolean(),
    distant_billboard: Type.Boolean(),
    season_overlay: Type.String({ minLength: 1, maxLength: 64 }),
    style: Type.String({ minLength: 1, maxLength: 64 }),
    at_tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type BreakthroughCinematicEventV1 = Static<typeof BreakthroughCinematicEventV1>;

export function validateBreakthroughCinematicEventV1Contract(data: unknown): ValidationResult {
  return validate(BreakthroughCinematicEventV1, data);
}
