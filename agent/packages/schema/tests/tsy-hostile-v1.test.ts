import { describe, expect, it } from "vitest";

import {
  TsyHostileArchetypeV1,
  TsyNpcSpawnedV1,
  TsySentinelPhaseChangedV1,
  validateTsyNpcSpawnedV1Contract,
  validateTsySentinelPhaseChangedV1Contract,
} from "../src/tsy-hostile-v1.js";
import { validate } from "../src/validate.js";

describe("plan-tsy-hostile-v1 §6 — TsyHostileArchetypeV1", () => {
  it("accepts all TSY hostile archetypes", () => {
    for (const archetype of [
      "daoxiang",
      "zhinian",
      "guardian_relic_sentinel",
      "fuya",
    ] as const) {
      expect(validate(TsyHostileArchetypeV1, archetype).ok).toBe(true);
    }
  });

  it("rejects ordinary GuardianRelic without the TSY sentinel marker", () => {
    expect(validate(TsyHostileArchetypeV1, "guardian_relic").ok).toBe(false);
  });
});

describe("plan-tsy-hostile-v1 §6 — TsyNpcSpawnedV1", () => {
  const valid = {
    v: 1 as const,
    kind: "tsy_npc_spawned" as const,
    family_id: "tsy_zongmen_yiji_01",
    archetype: "guardian_relic_sentinel" as const,
    count: 3,
    at_tick: 12000,
  };

  it("accepts a fully populated payload", () => {
    expect(validate(TsyNpcSpawnedV1, valid).ok).toBe(true);
  });

  it("round-trips through JSON.stringify / parse", () => {
    const parsed = JSON.parse(JSON.stringify(valid));
    expect(validateTsyNpcSpawnedV1Contract(parsed).ok).toBe(true);
  });

  it("accepts count = 0 for explicit empty spawn summaries", () => {
    expect(validate(TsyNpcSpawnedV1, { ...valid, count: 0 }).ok).toBe(true);
  });

  it("rejects fractional count", () => {
    expect(validate(TsyNpcSpawnedV1, { ...valid, count: 1.5 } as never).ok).toBe(false);
  });

  it("rejects unknown archetype", () => {
    expect(validate(TsyNpcSpawnedV1, { ...valid, archetype: "scav" } as never).ok).toBe(false);
  });

  it("rejects extra fields", () => {
    expect(validate(TsyNpcSpawnedV1, { ...valid, pos: [0, 64, 0] } as never).ok).toBe(false);
  });
});

describe("plan-tsy-hostile-v1 §6 — TsySentinelPhaseChangedV1", () => {
  const valid = {
    v: 1 as const,
    kind: "tsy_sentinel_phase_changed" as const,
    family_id: "tsy_zongmen_yiji_01",
    container_entity_id: 42,
    phase: 1,
    max_phase: 3,
    at_tick: 12345,
  };

  it("accepts a fully populated payload", () => {
    expect(validate(TsySentinelPhaseChangedV1, valid).ok).toBe(true);
  });

  it("round-trips through JSON.stringify / parse", () => {
    const parsed = JSON.parse(JSON.stringify(valid));
    expect(validateTsySentinelPhaseChangedV1Contract(parsed).ok).toBe(true);
  });

  it("accepts phase = 0 for initial stage notifications", () => {
    expect(validate(TsySentinelPhaseChangedV1, { ...valid, phase: 0 }).ok).toBe(true);
  });

  it("rejects max_phase = 0", () => {
    expect(validate(TsySentinelPhaseChangedV1, { ...valid, max_phase: 0 } as never).ok).toBe(
      false,
    );
  });

  it("rejects negative container entity id", () => {
    expect(
      validate(TsySentinelPhaseChangedV1, { ...valid, container_entity_id: -1 } as never).ok,
    ).toBe(false);
  });
});
