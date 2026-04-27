import { describe, expect, it } from "vitest";

import {
  TsyEnterEventV1,
  TsyExitEventV1,
  validateTsyEnterEventV1Contract,
  validateTsyExitEventV1Contract,
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
