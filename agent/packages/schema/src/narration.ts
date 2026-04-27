import { Type, type Static } from "@sinclair/typebox";

import { NarrationKind, NarrationScope, NarrationStyle } from "./common.js";
import { validate, type ValidationResult } from "./validate.js";

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

const hasOwn = Object.prototype.hasOwnProperty;

export const Narration = Type.Object(
  {
    scope: NarrationScope,
    target: Type.Optional(Type.String({ description: "zone name or player uuid, required when scope != broadcast" })),
    text: Type.String({ maxLength: 500 }),
    style: NarrationStyle,
    kind: Type.Optional(NarrationKind),
  },
  { additionalProperties: false },
);
export type Narration = Static<typeof Narration>;

export const NarrationV1 = Type.Object(
  {
    v: Type.Literal(1),
    narrations: Type.Array(Narration),
  },
  { additionalProperties: false },
);
export type NarrationV1 = Static<typeof NarrationV1>;

export function validateNarrationV1Contract(data: unknown): ValidationResult {
  const result = validate(NarrationV1, data);
  if (!result.ok) {
    return result;
  }

  if (!isRecord(data) || !Array.isArray(data.narrations)) {
    return {
      ok: false,
      errors: ["/narrations: NarrationV1.narrations must be an array"],
    };
  }

  const errors: string[] = [];
  data.narrations.forEach((entry, index) => {
    if (!isRecord(entry)) {
      errors.push(`/narrations/${index}: Narration entry must be an object`);
      return;
    }

    const hasTarget = hasOwn.call(entry, "target");
    if (entry.scope !== "broadcast" && !hasTarget) {
      errors.push(
        `/narrations/${index}/target: target is required when scope is \`${String(entry.scope)}\``,
      );
    }
  });

  if (errors.length > 0) {
    return { ok: false, errors };
  }

  return { ok: true, errors: [] };
}
