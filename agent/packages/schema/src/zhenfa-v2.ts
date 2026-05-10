import { Type, type Static } from "@sinclair/typebox";
import { validate, type ValidationResult } from "./validate.js";

export const ZhenfaArrayKindV2 = Type.Union([
  Type.Literal("trap"),
  Type.Literal("ward"),
  Type.Literal("shrine_ward"),
  Type.Literal("lingju"),
  Type.Literal("deceive_heaven"),
  Type.Literal("illusion"),
]);
export type ZhenfaArrayKindV2 = Static<typeof ZhenfaArrayKindV2>;

export const ZhenfaV2EventKind = Type.Union([
  Type.Literal("deploy"),
  Type.Literal("decay"),
  Type.Literal("breakthrough"),
  Type.Literal("deceive_heaven_exposed"),
]);
export type ZhenfaV2EventKind = Static<typeof ZhenfaV2EventKind>;

export const ZhenfaV2EventV1 = Type.Object(
  {
    v: Type.Literal(1),
    event: ZhenfaV2EventKind,
    array_id: Type.Integer({ minimum: 0 }),
    kind: ZhenfaArrayKindV2,
    owner: Type.String({ minLength: 1 }),
    x: Type.Integer(),
    y: Type.Integer(),
    z: Type.Integer(),
    tick: Type.Integer({ minimum: 0 }),
    radius: Type.Optional(Type.Number({ minimum: 0 })),
    density_multiplier: Type.Optional(Type.Number({ minimum: 0 })),
    tiandao_gaze_weight: Type.Optional(Type.Number({ minimum: 0 })),
    reveal_chance_per_tick: Type.Optional(Type.Number({ minimum: 0, maximum: 1 })),
    reveal_threshold: Type.Optional(Type.Number({ minimum: 0 })),
    self_weight_multiplier: Type.Optional(Type.Number({ minimum: 0 })),
    target_weight_multiplier: Type.Optional(Type.Number({ minimum: 0 })),
    force_break: Type.Optional(Type.Boolean()),
  },
  { additionalProperties: false },
);
export type ZhenfaV2EventV1 = Static<typeof ZhenfaV2EventV1>;

export function validateZhenfaV2EventV1Contract(data: unknown): ValidationResult {
  return validate(ZhenfaV2EventV1, data);
}
