import { Type, type Static } from "@sinclair/typebox";

import { validate, type ValidationResult } from "./validate.js";

export const NpcArchetypeV1 = Type.Union([
  Type.Literal("zombie"),
  Type.Literal("commoner"),
  Type.Literal("rogue"),
  Type.Literal("beast"),
  Type.Literal("disciple"),
  Type.Literal("guardian_relic"),
  Type.Literal("daoxiang"),
  Type.Literal("zhinian"),
  Type.Literal("fuya"),
]);
export type NpcArchetypeV1 = Static<typeof NpcArchetypeV1>;

export const NpcSpawnSourceV1 = Type.Union([
  Type.Literal("startup"),
  Type.Literal("seed"),
  Type.Literal("reproduction"),
  Type.Literal("agent_command"),
]);
export type NpcSpawnSourceV1 = Static<typeof NpcSpawnSourceV1>;

export const NpcDeathCauseV1 = Type.Union([
  Type.Literal("natural_aging"),
  Type.Literal("combat"),
  Type.Literal("despawned"),
  Type.Literal("duo_she"),
]);
export type NpcDeathCauseV1 = Static<typeof NpcDeathCauseV1>;

export const FactionIdV1 = Type.Union([
  Type.Literal("attack"),
  Type.Literal("defend"),
  Type.Literal("neutral"),
]);
export type FactionIdV1 = Static<typeof FactionIdV1>;

export const FactionEventKindV1 = Type.Union([
  Type.Literal("set_leader"),
  Type.Literal("clear_leader"),
  Type.Literal("set_leader_lineage"),
  Type.Literal("adjust_loyalty_bias"),
  Type.Literal("enqueue_mission"),
  Type.Literal("pop_mission"),
]);
export type FactionEventKindV1 = Static<typeof FactionEventKindV1>;

export const NpcSpawnedV1 = Type.Object(
  {
    v: Type.Literal(1),
    kind: Type.Literal("npc_spawned"),
    npc_id: Type.String({ minLength: 1 }),
    archetype: NpcArchetypeV1,
    source: NpcSpawnSourceV1,
    zone: Type.String({ minLength: 1 }),
    pos: Type.Tuple([Type.Number(), Type.Number(), Type.Number()]),
    initial_age_ticks: Type.Number({ minimum: 0 }),
    at_tick: Type.Number({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type NpcSpawnedV1 = Static<typeof NpcSpawnedV1>;

export const NpcDeathV1 = Type.Object(
  {
    v: Type.Literal(1),
    kind: Type.Literal("npc_death"),
    npc_id: Type.String({ minLength: 1 }),
    archetype: NpcArchetypeV1,
    cause: NpcDeathCauseV1,
    faction_id: Type.Optional(FactionIdV1),
    life_record_snapshot: Type.Optional(Type.String()),
    age_ticks: Type.Number({ minimum: 0 }),
    max_age_ticks: Type.Number({ minimum: 0 }),
    at_tick: Type.Number({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type NpcDeathV1 = Static<typeof NpcDeathV1>;

export const FactionEventV1 = Type.Object(
  {
    v: Type.Literal(1),
    kind: Type.Literal("faction_event"),
    faction_id: FactionIdV1,
    event_kind: FactionEventKindV1,
    leader_id: Type.Optional(Type.String({ minLength: 1 })),
    loyalty_bias: Type.Number({ minimum: 0, maximum: 1 }),
    mission_queue_size: Type.Integer({ minimum: 0 }),
    at_tick: Type.Number({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type FactionEventV1 = Static<typeof FactionEventV1>;

export function validateNpcSpawnedV1Contract(data: unknown): ValidationResult {
  return validate(NpcSpawnedV1, data);
}

export function validateNpcDeathV1Contract(data: unknown): ValidationResult {
  return validate(NpcDeathV1, data);
}

export function validateFactionEventV1Contract(data: unknown): ValidationResult {
  return validate(FactionEventV1, data);
}
