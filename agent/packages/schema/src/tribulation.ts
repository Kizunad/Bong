/**
 * plan-tribulation-v1 §6 天劫 IPC schema。与 `server/src/schema/tribulation.rs` 对齐。
 */
import { Type, type Static } from "@sinclair/typebox";
import { validate, type ValidationResult } from "./validate.js";

export const TribulationKindV1 = Type.Union([
  Type.Literal("du_xu"),
  Type.Literal("zone_collapse"),
  Type.Literal("targeted"),
  Type.Literal("jue_bi"),
  Type.Literal("ascension_quota_open"),
]);
export type TribulationKindV1 = Static<typeof TribulationKindV1>;

export const TribulationPhaseV1 = Type.Union([
  Type.Object({ kind: Type.Literal("omen") }, { additionalProperties: false }),
  Type.Object({ kind: Type.Literal("lock") }, { additionalProperties: false }),
  Type.Object(
    {
      kind: Type.Literal("wave"),
      wave: Type.Integer({ minimum: 0 }),
    },
    { additionalProperties: false },
  ),
  Type.Object({ kind: Type.Literal("heart_demon") }, { additionalProperties: false }),
  Type.Object({ kind: Type.Literal("settle") }, { additionalProperties: false }),
]);
export type TribulationPhaseV1 = Static<typeof TribulationPhaseV1>;

export const DuXuOutcomeV1 = Type.Union([
  Type.Literal("ascended"),
  Type.Literal("half_step"),
  Type.Literal("failed"),
  Type.Literal("killed"),
  Type.Literal("fled"),
]);
export type DuXuOutcomeV1 = Static<typeof DuXuOutcomeV1>;

export const DuXuResultReasonV1 = Type.Union([
  Type.Literal("void_quota_exceeded"),
]);
export type DuXuResultReasonV1 = Static<typeof DuXuResultReasonV1>;

export const DuXuResultV1 = Type.Object(
  {
    char_id: Type.String({ minLength: 1 }),
    outcome: DuXuOutcomeV1,
    killer: Type.Optional(Type.String({ minLength: 1 })),
    waves_survived: Type.Integer({ minimum: 0 }),
    reason: Type.Optional(DuXuResultReasonV1),
  },
  { additionalProperties: false },
);
export type DuXuResultV1 = Static<typeof DuXuResultV1>;

export const TribulationEventV1 = Type.Object(
  {
    v: Type.Literal(1),
    kind: TribulationKindV1,
    phase: TribulationPhaseV1,
    char_id: Type.Optional(Type.String({ minLength: 1 })),
    actor_name: Type.Optional(Type.String({ minLength: 1 })),
    zone: Type.Optional(Type.String({ minLength: 1 })),
    source: Type.Optional(Type.String({ minLength: 1 })),
    epicenter: Type.Optional(Type.Tuple([Type.Number(), Type.Number(), Type.Number()])),
    wave_current: Type.Optional(Type.Integer({ minimum: 0 })),
    wave_total: Type.Optional(Type.Integer({ minimum: 0 })),
    result: Type.Optional(DuXuResultV1),
    occupied_slots: Type.Optional(Type.Integer({ minimum: 0 })),
  },
  { additionalProperties: false },
);
export type TribulationEventV1 = Static<typeof TribulationEventV1>;

export function validateTribulationEventV1Contract(data: unknown): ValidationResult {
  return validate(TribulationEventV1, data);
}
