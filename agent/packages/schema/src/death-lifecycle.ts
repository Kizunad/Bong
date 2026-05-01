import { Type, type Static } from "@sinclair/typebox";

import { BiographyEntryV1 } from "./biography.js";
import { Realm } from "./cultivation.js";
import { DeceasedSocialSnapshotV1 } from "./social.js";
import { validate, type ValidationResult } from "./validate.js";

export const ZoneDeathKind = Type.Union(
  [
    Type.Literal("ordinary"),
    Type.Literal("death"),
    Type.Literal("negative"),
  ],
  { description: "死亡地点域标签" },
);
export type ZoneDeathKind = Static<typeof ZoneDeathKind>;

export const RebirthStage = Type.Union(
  [Type.Literal("fortune"), Type.Literal("tribulation")],
  { description: "运数期 / 劫数期" },
);
export type RebirthStage = Static<typeof RebirthStage>;

export const LifespanComponentV1 = Type.Object(
  {
    born_at_tick: Type.Integer({ minimum: 0 }),
    years_lived: Type.Number({ minimum: 0 }),
    cap_by_realm: Type.Integer({ minimum: 1 }),
    offline_pause_tick: Type.Optional(Type.Integer({ minimum: 0 })),
  },
  { additionalProperties: false },
);
export type LifespanComponentV1 = Static<typeof LifespanComponentV1>;

export const DeathRegistryV1 = Type.Object(
  {
    char_id: Type.String({ minLength: 1 }),
    death_count: Type.Integer({ minimum: 0 }),
    last_death_tick: Type.Optional(Type.Integer({ minimum: 0 })),
    /** 上一次死亡的 tick（不含当前已记录的死亡）。用于 24h 窗口判定。 */
    prev_death_tick: Type.Optional(Type.Integer({ minimum: 0 })),
    last_death_zone: Type.Optional(ZoneDeathKind),
  },
  { additionalProperties: false },
);
export type DeathRegistryV1 = Static<typeof DeathRegistryV1>;

export const RebirthChanceInputV1 = Type.Object(
  {
    registry: DeathRegistryV1,
    at_tick: Type.Integer({ minimum: 0 }),
    death_zone: ZoneDeathKind,
    karma: Type.Number(),
    has_shrine: Type.Boolean(),
    includes_current_death: Type.Optional(Type.Boolean()),
  },
  { additionalProperties: false },
);
export type RebirthChanceInputV1 = Static<typeof RebirthChanceInputV1>;

export const RebirthChanceResultV1 = Type.Object(
  {
    death_number: Type.Integer({ minimum: 1 }),
    stage: RebirthStage,
    chance: Type.Number({ minimum: 0, maximum: 1 }),
    guaranteed: Type.Boolean(),
    fortune_charge_cost: Type.Integer({ minimum: 0 }),
    skip_fortune_due_to_zone: Type.Boolean(),
    no_recent_death: Type.Boolean(),
    low_karma: Type.Boolean(),
    has_shrine: Type.Boolean(),
  },
  { additionalProperties: false },
);
export type RebirthChanceResultV1 = Static<typeof RebirthChanceResultV1>;

export const LifespanCapByRealmV1 = Type.Object(
  {
    mortal: Type.Integer({ const: 80 }),
    awaken: Type.Integer({ const: 120 }),
    induce: Type.Integer({ const: 200 }),
    condense: Type.Integer({ const: 350 }),
    solidify: Type.Integer({ const: 600 }),
    spirit: Type.Integer({ const: 1000 }),
    void: Type.Integer({ const: 2000 }),
  },
  { additionalProperties: false },
);
export type LifespanCapByRealmV1 = Static<typeof LifespanCapByRealmV1>;

export const LifespanPreviewV1 = Type.Object(
  {
    realm: Realm,
    remaining_years: Type.Number({ minimum: 0 }),
    death_penalty_years: Type.Integer({ minimum: 0 }),
    is_wind_candle: Type.Boolean(),
  },
  { additionalProperties: false },
);
export type LifespanPreviewV1 = Static<typeof LifespanPreviewV1>;

export const TerminationCategoryV1 = Type.Union(
  [
    Type.Literal("横死"),
    Type.Literal("善终"),
    Type.Literal("自主归隐"),
    Type.Literal("夺舍者"),
  ],
  { description: "亡者博物馆公开分类" },
);
export type TerminationCategoryV1 = Static<typeof TerminationCategoryV1>;

export const LifecycleStateV1 = Type.Union([
  Type.Literal("Alive"),
  Type.Literal("NearDeath"),
  Type.Literal("AwaitingRevival"),
  Type.Literal("Terminated"),
]);
export type LifecycleStateV1 = Static<typeof LifecycleStateV1>;

export const RevivalDecisionV1 = Type.Union([
  Type.Object({ Fortune: Type.Object({ chance: Type.Number({ minimum: 0, maximum: 1 }) }) }),
  Type.Object({ Tribulation: Type.Object({ chance: Type.Number({ minimum: 0, maximum: 1 }) }) }),
]);
export type RevivalDecisionV1 = Static<typeof RevivalDecisionV1>;

export const LifecycleV1 = Type.Object(
  {
    character_id: Type.String({ minLength: 1 }),
    death_count: Type.Integer({ minimum: 0 }),
    fortune_remaining: Type.Integer({ minimum: 0 }),
    last_death_tick: Type.Optional(Type.Integer({ minimum: 0 })),
    last_revive_tick: Type.Optional(Type.Integer({ minimum: 0 })),
    spawn_anchor: Type.Optional(Type.Tuple([Type.Number(), Type.Number(), Type.Number()])),
    near_death_deadline_tick: Type.Optional(Type.Integer({ minimum: 0 })),
    awaiting_decision: Type.Optional(RevivalDecisionV1),
    revival_decision_deadline_tick: Type.Optional(Type.Integer({ minimum: 0 })),
    weakened_until_tick: Type.Optional(Type.Integer({ minimum: 0 })),
    state: LifecycleStateV1,
  },
  { additionalProperties: false },
);
export type LifecycleV1 = Static<typeof LifecycleV1>;

export const DeathInsightRecordV1 = Type.Object(
  {
    tick: Type.Integer({ minimum: 0 }),
    text: Type.String({ minLength: 1, maxLength: 2000 }),
    style: Type.String({ minLength: 1, maxLength: 64 }),
  },
  { additionalProperties: false },
);
export type DeathInsightRecordV1 = Static<typeof DeathInsightRecordV1>;

export const LifeRecordV1 = Type.Object(
  {
    character_id: Type.String({ minLength: 1 }),
    created_at: Type.Integer({ minimum: 0 }),
    biography: Type.Array(BiographyEntryV1, { maxItems: 2048 }),
    insights_taken: Type.Array(Type.Any(), { maxItems: 2048 }),
    death_insights: Type.Array(DeathInsightRecordV1, { maxItems: 2048 }),
    spirit_root_first: Type.Optional(Type.String({ minLength: 1, maxLength: 128 })),
  },
  { additionalProperties: false },
);
export type LifeRecordV1 = Static<typeof LifeRecordV1>;

export const DeceasedIndexEntryV1 = Type.Object(
  {
    char_id: Type.String({ minLength: 1 }),
    died_at_tick: Type.Integer({ minimum: 0 }),
    path: Type.String({ minLength: 1 }),
    termination_category: TerminationCategoryV1,
  },
  { additionalProperties: false },
);
export type DeceasedIndexEntryV1 = Static<typeof DeceasedIndexEntryV1>;

export const DeceasedSnapshotV1 = Type.Object(
  {
    char_id: Type.String({ minLength: 1 }),
    died_at_tick: Type.Integer({ minimum: 0 }),
    termination_category: TerminationCategoryV1,
    lifecycle: LifecycleV1,
    life_record: LifeRecordV1,
    social: Type.Optional(DeceasedSocialSnapshotV1),
  },
  { additionalProperties: false },
);
export type DeceasedSnapshotV1 = Static<typeof DeceasedSnapshotV1>;

export const LifespanEventV1 = Type.Object(
  {
    v: Type.Literal(1),
    character_id: Type.String({ minLength: 1 }),
    at_tick: Type.Integer({ minimum: 0 }),
    kind: Type.Union([
      Type.Literal("aging"),
      Type.Literal("death_penalty"),
      Type.Literal("extension"),
    ]),
    delta_years: Type.Integer(),
    source: Type.String({ minLength: 1, maxLength: 256 }),
  },
  { additionalProperties: false },
);
export type LifespanEventV1 = Static<typeof LifespanEventV1>;

export const AgingEventKindV1 = Type.Union([
  Type.Literal("wind_candle"),
  Type.Literal("natural_death"),
  Type.Literal("tick_rate"),
]);
export type AgingEventKindV1 = Static<typeof AgingEventKindV1>;

export const AgingEventV1 = Type.Object(
  {
    v: Type.Literal(1),
    character_id: Type.String({ minLength: 1 }),
    at_tick: Type.Integer({ minimum: 0 }),
    kind: AgingEventKindV1,
    years_lived: Type.Number({ minimum: 0 }),
    cap_by_realm: Type.Integer({ minimum: 1 }),
    remaining_years: Type.Number({ minimum: 0 }),
    tick_rate_multiplier: Type.Number({ minimum: 0 }),
    source: Type.String({ minLength: 1, maxLength: 256 }),
  },
  { additionalProperties: false },
);
export type AgingEventV1 = Static<typeof AgingEventV1>;

export const DuoSheEventV1 = Type.Object(
  {
    v: Type.Literal(1),
    host_id: Type.String({ minLength: 1 }),
    target_id: Type.String({ minLength: 1 }),
    at_tick: Type.Integer({ minimum: 0 }),
    karma_delta: Type.Number(),
    host_prev_age: Type.Number({ minimum: 0 }),
    target_age: Type.Number({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type DuoSheEventV1 = Static<typeof DuoSheEventV1>;

export const RebirthEventV1 = Type.Object(
  {
    v: Type.Literal(1),
    character_id: Type.String({ minLength: 1 }),
    at_tick: Type.Integer({ minimum: 0 }),
    prior_realm: Realm,
    new_realm: Realm,
  },
  { additionalProperties: false },
);
export type RebirthEventV1 = Static<typeof RebirthEventV1>;

export function validateDeceasedIndexEntryV1Contract(data: unknown): ValidationResult {
  return validate(DeceasedIndexEntryV1, data);
}

export function validateDeceasedSnapshotV1Contract(data: unknown): ValidationResult {
  return validate(DeceasedSnapshotV1, data);
}

export function validateLifespanEventV1Contract(data: unknown): ValidationResult {
  return validate(LifespanEventV1, data);
}

export function validateAgingEventV1Contract(data: unknown): ValidationResult {
  return validate(AgingEventV1, data);
}

export function validateDuoSheEventV1Contract(data: unknown): ValidationResult {
  return validate(DuoSheEventV1, data);
}

export function validateRebirthEventV1Contract(data: unknown): ValidationResult {
  return validate(RebirthEventV1, data);
}
