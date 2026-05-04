import { type Static, Type } from "@sinclair/typebox";

import { ColorKind } from "./cultivation.js";

export const StyleTelemetryColorSnapshotV1 = Type.Object(
  {
    main: ColorKind,
    secondary: Type.Optional(ColorKind),
    is_chaotic: Type.Boolean(),
    is_hunyuan: Type.Boolean(),
  },
  { additionalProperties: false },
);
export type StyleTelemetryColorSnapshotV1 = Static<
  typeof StyleTelemetryColorSnapshotV1
>;

export const StyleBalanceTelemetryEventV1 = Type.Object(
  {
    v: Type.Literal(1),
    attacker_player_id: Type.String({ minLength: 1 }),
    defender_player_id: Type.String({ minLength: 1 }),
    attacker_color: Type.Optional(StyleTelemetryColorSnapshotV1),
    defender_color: Type.Optional(StyleTelemetryColorSnapshotV1),
    cause: Type.String({ minLength: 1 }),
    resolved_at_tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type StyleBalanceTelemetryEventV1 = Static<
  typeof StyleBalanceTelemetryEventV1
>;
