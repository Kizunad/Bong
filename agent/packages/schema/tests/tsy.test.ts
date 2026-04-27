import { describe, expect, it } from "vitest";

import {
  DaoxiangSpawnedV1,
  TsyCollapseCompletedV1,
  TsyCollapseStartedV1,
  TsyCorpseSpawnEventV1,
  TsyEnterEventV1,
  TsyExitEventV1,
  TsyZoneActivatedV1,
  validateDaoxiangSpawnedV1Contract,
  validateTsyCollapseCompletedV1Contract,
  validateTsyCollapseStartedV1Contract,
  validateTsyCorpseSpawnEventV1Contract,
  validateTsyEnterEventV1Contract,
  validateTsyExitEventV1Contract,
  validateTsyZoneActivatedV1Contract,
} from "../src/tsy.js";
import { validate } from "../src/validate.js";

describe("plan-tsy-zone-v1 §1.4 — TsyEnterEventV1", () => {
  const valid = {
    v: 1 as const,
    kind: "tsy_enter" as const,
    tick: 12345,
    player_id: "kiz",
    family_id: "tsy_lingxu_01",
    return_to: {
      dimension: "minecraft:overworld",
      pos: [0.0, 65.0, 0.0],
    },
    filtered_items: [
      {
        instance_id: 7,
        template_id: "bone_coin",
        reason: "spirit_quality_too_high" as const,
      },
    ],
  };

  it("accepts a fully populated payload", () => {
    const result = validate(TsyEnterEventV1, valid);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("round-trips through JSON.stringify / parse", () => {
    const json = JSON.stringify(valid);
    const parsed = JSON.parse(json);
    expect(validateTsyEnterEventV1Contract(parsed).ok).toBe(true);
  });

  it("accepts payload with empty filtered_items list (no qi-quality items in inv)", () => {
    const result = validate(TsyEnterEventV1, { ...valid, filtered_items: [] });
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("rejects payload missing return_to", () => {
    const { return_to: _, ...without } = valid;
    expect(validate(TsyEnterEventV1, without).ok).toBe(false);
  });

  it("rejects return_to.pos with wrong arity", () => {
    const bad = { ...valid, return_to: { dimension: "minecraft:overworld", pos: [0, 65] } };
    expect(validate(TsyEnterEventV1, bad).ok).toBe(false);
  });

  it("rejects unknown filter reason", () => {
    const bad = {
      ...valid,
      filtered_items: [
        { instance_id: 1, template_id: "x", reason: "ate_my_homework" },
      ],
    };
    expect(validate(TsyEnterEventV1, bad).ok).toBe(false);
  });

  it("rejects extra top-level fields", () => {
    expect(validate(TsyEnterEventV1, { ...valid, surprise: true } as never).ok).toBe(false);
  });

  it("rejects v != 1", () => {
    expect(validate(TsyEnterEventV1, { ...valid, v: 2 } as never).ok).toBe(false);
  });
});

describe("plan-tsy-zone-v1 §1.4 — TsyExitEventV1", () => {
  const valid = {
    v: 1 as const,
    kind: "tsy_exit" as const,
    tick: 99999,
    player_id: "kiz",
    family_id: "tsy_lingxu_01",
    duration_ticks: 12000,
    qi_drained_total: 350.5,
  };

  it("accepts a fully populated payload", () => {
    const result = validate(TsyExitEventV1, valid);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("round-trips through JSON.stringify / parse", () => {
    const parsed = JSON.parse(JSON.stringify(valid));
    expect(validateTsyExitEventV1Contract(parsed).ok).toBe(true);
  });

  it("rejects negative duration_ticks", () => {
    expect(validate(TsyExitEventV1, { ...valid, duration_ticks: -1 } as never).ok).toBe(false);
  });

  it("rejects negative qi_drained_total", () => {
    expect(validate(TsyExitEventV1, { ...valid, qi_drained_total: -0.001 } as never).ok).toBe(false);
  });

  it("accepts qi_drained_total = 0 (P0 placeholder before loot plan accumulates)", () => {
    const result = validate(TsyExitEventV1, { ...valid, qi_drained_total: 0 });
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });
});

describe("plan-tsy-loot-v1 §4.4 — TsyCorpseSpawnEventV1", () => {
  const valid = {
    v: 1 as const,
    kind: "tsy_corpse_spawn" as const,
    tick: 50000,
    corpse_entity_id: "npc_42v3",
    original_player_id: "offline:Foo",
    original_display_name: "Foo",
    family_id: "tsy_lingxu_01",
    death_cause: "tsy_drain",
    pos: [128.5, 64.0, -45.2],
  };

  it("accepts a fully populated payload", () => {
    const result = validate(TsyCorpseSpawnEventV1, valid);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("round-trips through JSON.stringify / parse", () => {
    const parsed = JSON.parse(JSON.stringify(valid));
    expect(validateTsyCorpseSpawnEventV1Contract(parsed).ok).toBe(true);
  });

  it("rejects pos with wrong arity", () => {
    expect(validate(TsyCorpseSpawnEventV1, { ...valid, pos: [0, 0] } as never).ok).toBe(false);
  });

  it("rejects empty original_player_id", () => {
    expect(validate(TsyCorpseSpawnEventV1, { ...valid, original_player_id: "" } as never).ok).toBe(
      false,
    );
  });

  it("rejects unknown extra field", () => {
    expect(validate(TsyCorpseSpawnEventV1, { ...valid, surprise: 1 } as never).ok).toBe(false);
  });

  it("accepts pvp death_cause string", () => {
    const result = validate(TsyCorpseSpawnEventV1, {
      ...valid,
      death_cause: "attack_intent:offline:Bob",
    });
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });
});

describe("plan-tsy-lifecycle-v1 §1.5 — TsyZoneActivatedV1", () => {
  const valid = {
    v: 1 as const,
    kind: "tsy_zone_activated" as const,
    tick: 1000,
    family_id: "tsy_lingxu_01",
    source_class: "dao_lord" as const,
  };

  it("accepts a fully populated payload", () => {
    expect(validate(TsyZoneActivatedV1, valid).ok).toBe(true);
  });

  it("round-trips through JSON.stringify / parse", () => {
    const parsed = JSON.parse(JSON.stringify(valid));
    expect(validateTsyZoneActivatedV1Contract(parsed).ok).toBe(true);
  });

  it("accepts all three source_class values", () => {
    for (const sc of ["dao_lord", "sect_ruins", "battle_sediment"] as const) {
      expect(validate(TsyZoneActivatedV1, { ...valid, source_class: sc }).ok).toBe(true);
    }
  });

  it("rejects unknown source_class", () => {
    expect(
      validate(TsyZoneActivatedV1, { ...valid, source_class: "ascended_sage" } as never).ok,
    ).toBe(false);
  });

  it("rejects extra field", () => {
    expect(validate(TsyZoneActivatedV1, { ...valid, surprise: 1 } as never).ok).toBe(false);
  });
});

describe("plan-tsy-lifecycle-v1 §3.1 — TsyCollapseStartedV1", () => {
  const valid = {
    v: 1 as const,
    kind: "tsy_collapse_started" as const,
    tick: 50000,
    family_id: "tsy_lingxu_01",
    duration_ticks: 600,
  };

  it("accepts a fully populated payload", () => {
    expect(validate(TsyCollapseStartedV1, valid).ok).toBe(true);
  });

  it("round-trips through JSON.stringify / parse", () => {
    const parsed = JSON.parse(JSON.stringify(valid));
    expect(validateTsyCollapseStartedV1Contract(parsed).ok).toBe(true);
  });

  it("rejects negative duration_ticks", () => {
    expect(
      validate(TsyCollapseStartedV1, { ...valid, duration_ticks: -1 } as never).ok,
    ).toBe(false);
  });

  it("rejects unknown kind", () => {
    expect(
      validate(TsyCollapseStartedV1, { ...valid, kind: "tsy_zone_activated" } as never).ok,
    ).toBe(false);
  });
});

describe("plan-tsy-lifecycle-v1 §3.3 — TsyCollapseCompletedV1", () => {
  const valid = {
    v: 1 as const,
    kind: "tsy_collapse_completed" as const,
    tick: 50600,
    family_id: "tsy_lingxu_01",
  };

  it("accepts a fully populated payload", () => {
    expect(validate(TsyCollapseCompletedV1, valid).ok).toBe(true);
  });

  it("round-trips through JSON.stringify / parse", () => {
    const parsed = JSON.parse(JSON.stringify(valid));
    expect(validateTsyCollapseCompletedV1Contract(parsed).ok).toBe(true);
  });

  it("rejects empty family_id", () => {
    expect(validate(TsyCollapseCompletedV1, { ...valid, family_id: "" } as never).ok).toBe(false);
  });
});

describe("plan-tsy-lifecycle-v1 §4 — DaoxiangSpawnedV1", () => {
  const valid = {
    v: 1 as const,
    kind: "daoxiang_spawned" as const,
    tick: 60000,
    daoxiang_entity_id: "npc_99v1",
    from_family: "tsy_lingxu_01",
    from_corpse_death_cause: "tsy_drain",
    pos: [128.0, 64.0, -32.0],
    mode: "natural" as const,
  };

  it("accepts a fully populated payload (natural)", () => {
    expect(validate(DaoxiangSpawnedV1, valid).ok).toBe(true);
  });

  it("accepts collapse_accelerated mode", () => {
    expect(validate(DaoxiangSpawnedV1, { ...valid, mode: "collapse_accelerated" }).ok).toBe(true);
  });

  it("round-trips through JSON.stringify / parse", () => {
    const parsed = JSON.parse(JSON.stringify(valid));
    expect(validateDaoxiangSpawnedV1Contract(parsed).ok).toBe(true);
  });

  it("rejects pos with wrong arity", () => {
    expect(validate(DaoxiangSpawnedV1, { ...valid, pos: [0, 0] } as never).ok).toBe(false);
  });

  it("rejects unknown mode", () => {
    expect(validate(DaoxiangSpawnedV1, { ...valid, mode: "spawn_aggro" } as never).ok).toBe(false);
  });
});
