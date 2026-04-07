import { Type, type Static } from "@sinclair/typebox";
import { NarrationScope, NarrationStyle } from "./common.js";

export const Narration = Type.Object(
  {
    scope: NarrationScope,
    target: Type.Optional(Type.String({ description: "zone name or player uuid, required when scope != broadcast" })),
    text: Type.String({ maxLength: 500 }),
    style: NarrationStyle,
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
