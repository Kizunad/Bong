import { Type, type Static } from "@sinclair/typebox";

import { Realm } from "./cultivation.js";
import { validate, type ValidationResult } from "./validate.js";

export const BreakthroughEventKind = Type.Union([
  Type.Literal("Started"),
  Type.Literal("Succeeded"),
  Type.Literal("Failed"),
]);
export type BreakthroughEventKind = Static<typeof BreakthroughEventKind>;

export const BreakthroughEventV1 = Type.Object(
  {
    kind: BreakthroughEventKind,
    from_realm: Realm,
    to_realm: Type.Optional(Realm),
    success_rate: Type.Optional(Type.Number({ minimum: 0, maximum: 1 })),
    severity: Type.Optional(Type.Number({ minimum: 0, maximum: 1 })),
  },
  { additionalProperties: false },
);
export type BreakthroughEventV1 = Static<typeof BreakthroughEventV1>;

export function validateBreakthroughEventV1Contract(data: unknown): ValidationResult {
  return validate(BreakthroughEventV1, data);
}
