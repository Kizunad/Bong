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
]);
export type BiographyEntryV1 = Static<typeof BiographyEntryV1>;

export function validateBiographyEntryV1Contract(data: unknown): ValidationResult {
  return validate(BiographyEntryV1, data);
}
