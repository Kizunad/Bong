/**
 * 修炼侧 biography 事件（plan-cultivation §6.2）。
 *
 * Rust `BiographyEntryV1` 以 externally tagged enum 序列化，TS 侧用
 * tagged union 镜像。死亡终结快照 / 重生记录由战斗 plan 扩展。
 */
import { Type, type Static } from "@sinclair/typebox";

import { ColorKind, MeridianId, Realm } from "./cultivation.js";
import { validate, type ValidationResult } from "./validate.js";

const tickField = Type.Integer({ minimum: 0 });

const BreakthroughStarted = Type.Object(
  { BreakthroughStarted: Type.Object({ realm_target: Realm, tick: tickField }) },
  { additionalProperties: false },
);
const BreakthroughSucceeded = Type.Object(
  { BreakthroughSucceeded: Type.Object({ realm: Realm, tick: tickField }) },
  { additionalProperties: false },
);
const BreakthroughFailed = Type.Object(
  {
    BreakthroughFailed: Type.Object({
      realm_target: Realm,
      severity: Type.Number({ minimum: 0, maximum: 1 }),
      tick: tickField,
    }),
  },
  { additionalProperties: false },
);
const MeridianOpened = Type.Object(
  { MeridianOpened: Type.Object({ id: MeridianId, tick: tickField }) },
  { additionalProperties: false },
);
const MeridianClosed = Type.Object(
  {
    MeridianClosed: Type.Object({
      id: MeridianId,
      tick: tickField,
      reason: Type.String({ maxLength: 128 }),
    }),
  },
  { additionalProperties: false },
);
const ForgedRate = Type.Object(
  {
    ForgedRate: Type.Object({
      id: MeridianId,
      tier: Type.Integer({ minimum: 0, maximum: 16 }),
      tick: tickField,
    }),
  },
  { additionalProperties: false },
);
const ForgedCapacity = Type.Object(
  {
    ForgedCapacity: Type.Object({
      id: MeridianId,
      tier: Type.Integer({ minimum: 0, maximum: 16 }),
      tick: tickField,
    }),
  },
  { additionalProperties: false },
);
const ColorShift = Type.Object(
  {
    ColorShift: Type.Object({
      main: ColorKind,
      secondary: Type.Optional(ColorKind),
      tick: tickField,
    }),
  },
  { additionalProperties: false },
);
const InsightTaken = Type.Object(
  {
    InsightTaken: Type.Object({
      trigger: Type.String({ maxLength: 128 }),
      choice: Type.String({ maxLength: 256 }),
      tick: tickField,
    }),
  },
  { additionalProperties: false },
);
const Rebirth = Type.Object(
  {
    Rebirth: Type.Object({
      prior_realm: Realm,
      new_realm: Realm,
      tick: tickField,
    }),
  },
  { additionalProperties: false },
);
const DuguPoisonInflicted = Type.Object(
  {
    DuguPoisonInflicted: Type.Object({
      attacker_id: Type.String({ minLength: 1, maxLength: 128 }),
      target_id: Type.String({ minLength: 1, maxLength: 128 }),
      meridian_id: MeridianId,
      tick: tickField,
    }),
  },
  { additionalProperties: false },
);
const NearDeath = Type.Object(
  {
    NearDeath: Type.Object({
      cause: Type.String({ minLength: 1, maxLength: 512 }),
      tick: tickField,
    }),
  },
  { additionalProperties: false },
);
const Terminated = Type.Object(
  {
    Terminated: Type.Object({
      cause: Type.String({ minLength: 1, maxLength: 512 }),
      tick: tickField,
    }),
  },
  { additionalProperties: false },
);
const LifespanExtended = Type.Object(
  {
    LifespanExtended: Type.Object({
      source: Type.String({ minLength: 1, maxLength: 256 }),
      delta_years: Type.Integer(),
      tick: tickField,
    }),
  },
  { additionalProperties: false },
);
const DuoShePerformed = Type.Object(
  {
    DuoShePerformed: Type.Object({
      target_id: Type.String({ minLength: 1, maxLength: 128 }),
      host_prev_age: Type.Number({ minimum: 0 }),
      target_age: Type.Number({ minimum: 0 }),
      tick: tickField,
    }),
  },
  { additionalProperties: false },
);
const PossessedBy = Type.Object(
  {
    PossessedBy: Type.Object({
      host_id: Type.String({ minLength: 1, maxLength: 128 }),
      tick: tickField,
    }),
  },
  { additionalProperties: false },
);
const TribulationFled = Type.Object(
  {
    TribulationFled: Type.Object({
      wave: Type.Integer({ minimum: 0 }),
      tick: tickField,
    }),
  },
  { additionalProperties: false },
);

const TribulationIntercepted = Type.Object(
  {
    TribulationIntercepted: Type.Object({
      victim_id: Type.String({ minLength: 1, maxLength: 128 }),
      tag: Type.Optional(Type.String({ minLength: 1, maxLength: 64 })),
      tick: tickField,
    }),
  },
  { additionalProperties: false },
);

export const HeartDemonOutcomeV1 = Type.Union([
  Type.Literal("steadfast"),
  Type.Literal("obsession"),
  Type.Literal("no_solution"),
]);
export type HeartDemonOutcomeV1 = Static<typeof HeartDemonOutcomeV1>;

const HeartDemonRecord = Type.Object(
  {
    HeartDemonRecord: Type.Object({
      outcome: HeartDemonOutcomeV1,
      choice_idx: Type.Union([Type.Integer({ minimum: 0 }), Type.Null()]),
      tick: tickField,
    }),
  },
  { additionalProperties: false },
);

const JueBiSurvived = Type.Object(
  {
    JueBiSurvived: Type.Object({
      source: Type.String({ minLength: 1, maxLength: 128 }),
      tick: tickField,
    }),
  },
  { additionalProperties: false },
);

const JueBiKilled = Type.Object(
  {
    JueBiKilled: Type.Object({
      source: Type.String({ minLength: 1, maxLength: 128 }),
      tick: tickField,
    }),
  },
  { additionalProperties: false },
);

const TradeCompleted = Type.Object(
  {
    TradeCompleted: Type.Object({
      counterparty_id: Type.String({ minLength: 1, maxLength: 128 }),
      offered_item: Type.String({ minLength: 1, maxLength: 256 }),
      received_item: Type.String({ minLength: 1, maxLength: 256 }),
      tick: tickField,
    }),
  },
  { additionalProperties: false },
);

const FalseSkinShed = Type.Object(
  {
    FalseSkinShed: Type.Object({
      kind: Type.String({ minLength: 1, maxLength: 64 }),
      layers_shed: Type.Integer({ minimum: 1, maximum: 3 }),
      contam_absorbed: Type.Number({ minimum: 0 }),
      contam_overflow: Type.Number({ minimum: 0 }),
      attacker_id: Type.Optional(Type.Union([Type.String({ minLength: 1 }), Type.Null()])),
      tick: tickField,
    }),
  },
  { additionalProperties: false },
);

const SpawnTutorialCompleted = Type.Object(
  {
    SpawnTutorialCompleted: Type.Object({
      minutes_since_spawn: Type.Integer({ minimum: 0, maximum: 24 * 60 }),
      tick: tickField,
    }),
  },
  { additionalProperties: false },
);

export const BiographyEntryV1 = Type.Union([
  BreakthroughStarted,
  BreakthroughSucceeded,
  BreakthroughFailed,
  MeridianOpened,
  MeridianClosed,
  ForgedRate,
  ForgedCapacity,
  ColorShift,
  InsightTaken,
  Rebirth,
  DuguPoisonInflicted,
  NearDeath,
  Terminated,
  LifespanExtended,
  DuoShePerformed,
  PossessedBy,
  TribulationIntercepted,
  TribulationFled,
  HeartDemonRecord,
  JueBiSurvived,
  JueBiKilled,
  TradeCompleted,
  FalseSkinShed,
  SpawnTutorialCompleted,
]);
export type BiographyEntryV1 = Static<typeof BiographyEntryV1>;

export function validateBiographyEntryV1Contract(data: unknown): ValidationResult {
  return validate(BiographyEntryV1, data);
}
