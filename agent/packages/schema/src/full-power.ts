import { type Static, Type } from "@sinclair/typebox";

import { Vec3 } from "./world-state.js";

export const FullPowerChargingStateV1 = Type.Object(
  {
    caster_uuid: Type.String({ minLength: 1 }),
    active: Type.Boolean(),
    qi_committed: Type.Number({ minimum: 0 }),
    target_qi: Type.Number({ minimum: 0 }),
    started_tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type FullPowerChargingStateV1 = Static<typeof FullPowerChargingStateV1>;

export const FullPowerReleaseV1 = Type.Object(
  {
    caster_uuid: Type.String({ minLength: 1 }),
    target_uuid: Type.Optional(Type.String({ minLength: 1 })),
    qi_released: Type.Number({ minimum: 0 }),
    tick: Type.Integer({ minimum: 0 }),
    hit_position: Type.Optional(Vec3),
  },
  { additionalProperties: false },
);
export type FullPowerReleaseV1 = Static<typeof FullPowerReleaseV1>;

export const FullPowerExhaustedStateV1 = Type.Object(
  {
    caster_uuid: Type.String({ minLength: 1 }),
    active: Type.Boolean(),
    started_tick: Type.Integer({ minimum: 0 }),
    recovery_at_tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type FullPowerExhaustedStateV1 = Static<typeof FullPowerExhaustedStateV1>;
