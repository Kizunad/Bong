import { Type, type Static } from "@sinclair/typebox";

import { validate, type ValidationResult } from "./validate.js";

export const PoiNoviceKindV1 = Type.Union([
  Type.Literal("forge_station"),
  Type.Literal("alchemy_furnace"),
  Type.Literal("rogue_village"),
  Type.Literal("mutant_nest"),
  Type.Literal("scroll_hidden"),
  Type.Literal("spirit_herb_valley"),
  Type.Literal("herb_patch"),
  Type.Literal("qi_spring"),
  Type.Literal("trade_spot"),
  Type.Literal("shelter_spot"),
  Type.Literal("water_source"),
]);
export type PoiNoviceKindV1 = Static<typeof PoiNoviceKindV1>;

export const PoiSpawnedEventV1 = Type.Object(
  {
    v: Type.Literal(1),
    kind: Type.Literal("poi_spawned"),
    poi_id: Type.String({ minLength: 1 }),
    poi_type: PoiNoviceKindV1,
    zone: Type.String({ minLength: 1 }),
    pos: Type.Array(Type.Number(), { minItems: 3, maxItems: 3 }),
    selection_strategy: Type.String({ minLength: 1 }),
    qi_affinity: Type.Number({ minimum: -1, maximum: 1 }),
    danger_bias: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type PoiSpawnedEventV1 = Static<typeof PoiSpawnedEventV1>;

export const TrespassEventV1 = Type.Object(
  {
    v: Type.Literal(1),
    kind: Type.Literal("trespass"),
    village_id: Type.String({ minLength: 1 }),
    player_id: Type.String({ minLength: 1 }),
    killed_npc_count: Type.Integer({ minimum: 1 }),
    refusal_until_wall_clock_secs: Type.Number({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type TrespassEventV1 = Static<typeof TrespassEventV1>;

export function validatePoiSpawnedEventV1Contract(data: unknown): ValidationResult {
  return validate(PoiSpawnedEventV1, data);
}

export function validateTrespassEventV1Contract(data: unknown): ValidationResult {
  return validate(TrespassEventV1, data);
}
