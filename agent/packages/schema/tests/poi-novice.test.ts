import { describe, expect, it } from "vitest";

import {
  PoiNoviceKindV1,
  PoiSpawnedEventV1,
  TrespassEventV1,
  validatePoiSpawnedEventV1Contract,
  validateTrespassEventV1Contract,
} from "../src/poi-novice.js";
import { validate } from "../src/validate.js";

describe("plan-poi-novice-v1 — PoiNoviceKindV1", () => {
  it("accepts all six novice POI types", () => {
    for (const kind of [
      "forge_station",
      "alchemy_furnace",
      "rogue_village",
      "mutant_nest",
      "scroll_hidden",
      "spirit_herb_valley",
    ] as const) {
      expect(validate(PoiNoviceKindV1, kind).ok).toBe(true);
    }
  });

  it("rejects future POI kinds in v1", () => {
    expect(validate(PoiNoviceKindV1, "sect_market").ok).toBe(false);
  });
});

describe("plan-poi-novice-v1 — PoiSpawnedEventV1", () => {
  const valid = {
    v: 1 as const,
    kind: "poi_spawned" as const,
    poi_id: "spawn:forge_station",
    poi_type: "forge_station" as const,
    zone: "spawn",
    pos: [304, 71, 208],
    selection_strategy: "strict_radius_1500",
    qi_affinity: 0.15,
    danger_bias: 0,
  };

  it("accepts a fully populated spawned event", () => {
    expect(validatePoiSpawnedEventV1Contract(valid).ok).toBe(true);
  });

  it("rejects extra fields", () => {
    expect(validate(PoiSpawnedEventV1, { ...valid, hud_marker: true } as never).ok).toBe(false);
  });

  it("rejects out-of-range qi affinity", () => {
    expect(validate(PoiSpawnedEventV1, { ...valid, qi_affinity: 2 } as never).ok).toBe(false);
  });
});

describe("plan-poi-novice-v1 — TrespassEventV1", () => {
  const valid = {
    v: 1 as const,
    kind: "trespass" as const,
    village_id: "spawn:rogue_village",
    player_id: "offline:Azure",
    killed_npc_count: 3,
    refusal_until_wall_clock_secs: 1770000000,
  };

  it("accepts a fully populated trespass event", () => {
    expect(validateTrespassEventV1Contract(valid).ok).toBe(true);
  });

  it("requires at least one killed NPC", () => {
    expect(validate(TrespassEventV1, { ...valid, killed_npc_count: 0 } as never).ok).toBe(false);
  });

  it("rejects fractional killed count", () => {
    expect(validate(TrespassEventV1, { ...valid, killed_npc_count: 1.5 } as never).ok).toBe(false);
  });
});
