import { Type, type Static } from "@sinclair/typebox";

import { Realm } from "./cultivation.js";

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
