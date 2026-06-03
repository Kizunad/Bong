/**
 * plan-combat-skill-feedback-bridges-v1 P1 — BaomaiV4 schema 双端 sample 对拍测试。
 *
 * 每个变体：
 *   - 加载 samples/*.json（与 Rust serde 共享）
 *   - 用 TypeBox validate 函数正向校验所有 sample 条目
 *   - 反向拒绝：缺必填字段、额外字段、类型错误
 *   - channels.ts 常量 pin 测试
 */

import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

import { CHANNELS } from "../src/channels.js";
import {
  validateBaomaiV4ScarCircuitFormedV1,
  validateBaomaiV4ScarCircuitBrokenV1,
  validateBaomaiV4IronCocoonStageUpV1,
  validateBaomaiV4ResonanceLockV1,
  validateBaomaiV4ResonanceLockEndV1,
} from "../src/baomai-v4.js";

const __dirname = dirname(fileURLToPath(import.meta.url));

function loadSamples(filename: string): unknown[] {
  const path = join(__dirname, "..", "samples", filename);
  return JSON.parse(readFileSync(path, "utf-8")) as unknown[];
}

// ── channels pin ──────────────────────────────────────────────────────────────

describe("CHANNELS baomai_v4 constants pin", () => {
  it("BAOMAI_V4_SCAR_CIRCUIT_FORMED is frozen", () => {
    expect(
      CHANNELS.BAOMAI_V4_SCAR_CIRCUIT_FORMED,
      "channel constant must not change — server Rust and agent TS both depend on it",
    ).toBe("bong:baomai_v4/scar_circuit_formed");
  });

  it("BAOMAI_V4_SCAR_CIRCUIT_BROKEN is frozen", () => {
    expect(CHANNELS.BAOMAI_V4_SCAR_CIRCUIT_BROKEN).toBe(
      "bong:baomai_v4/scar_circuit_broken",
    );
  });

  it("BAOMAI_V4_IRON_COCOON_STAGE_UP is frozen", () => {
    expect(CHANNELS.BAOMAI_V4_IRON_COCOON_STAGE_UP).toBe(
      "bong:baomai_v4/iron_cocoon_stage_up",
    );
  });

  it("BAOMAI_V4_RESONANCE_LOCK is frozen", () => {
    expect(CHANNELS.BAOMAI_V4_RESONANCE_LOCK).toBe(
      "bong:baomai_v4/resonance_lock",
    );
  });

  it("BAOMAI_V4_RESONANCE_LOCK_END is frozen", () => {
    expect(CHANNELS.BAOMAI_V4_RESONANCE_LOCK_END).toBe(
      "bong:baomai_v4/resonance_lock_end",
    );
  });
});

// ── ScarCircuitFormed ─────────────────────────────────────────────────────────

describe("BaomaiV4ScarCircuitFormedV1", () => {
  it("all sample entries validate", () => {
    const samples = loadSamples("baomai_v4_scar_circuit_formed.json");
    expect(
      samples.length,
      "sample file must have at least 1 entry",
    ).toBeGreaterThanOrEqual(1);
    for (const [i, s] of samples.entries()) {
      const result = validateBaomaiV4ScarCircuitFormedV1(s);
      expect(result.ok, `sample[${i}] should be valid: ${JSON.stringify(result)}`).toBe(true);
    }
  });

  it("tiger_mouth circuit validates", () => {
    const result = validateBaomaiV4ScarCircuitFormedV1({
      v: 1,
      entity_id: "offline:TestPlayer",
      circuit: "tiger_mouth",
      tick: 100,
    });
    expect(result.ok).toBe(true);
  });

  it("ren_du_bridge circuit validates", () => {
    const result = validateBaomaiV4ScarCircuitFormedV1({
      v: 1,
      entity_id: "char:123",
      circuit: "ren_du_bridge",
      tick: 1,
    });
    expect(result.ok).toBe(true);
  });

  it("invalid circuit literal rejects", () => {
    const result = validateBaomaiV4ScarCircuitFormedV1({
      v: 1,
      entity_id: "offline:TestPlayer",
      circuit: "unknown_circuit",
      tick: 100,
    });
    expect(
      result.ok,
      "unknown circuit literal must be rejected — schema is a closed enum",
    ).toBe(false);
  });

  it("missing entity_id rejects", () => {
    const result = validateBaomaiV4ScarCircuitFormedV1({
      v: 1,
      circuit: "tiger_mouth",
      tick: 100,
    });
    expect(result.ok, "missing entity_id must fail validation").toBe(false);
  });

  it("additional fields rejected", () => {
    const result = validateBaomaiV4ScarCircuitFormedV1({
      v: 1,
      entity_id: "offline:TestPlayer",
      circuit: "tiger_mouth",
      tick: 100,
      extra: "forbidden",
    });
    expect(result.ok, "extra field must be rejected by additionalProperties:false").toBe(false);
  });
});

// ── ScarCircuitBroken ─────────────────────────────────────────────────────────

describe("BaomaiV4ScarCircuitBrokenV1", () => {
  it("all sample entries validate", () => {
    const samples = loadSamples("baomai_v4_scar_circuit_broken.json");
    for (const [i, s] of samples.entries()) {
      const result = validateBaomaiV4ScarCircuitBrokenV1(s);
      expect(result.ok, `sample[${i}] should be valid`).toBe(true);
    }
  });

  it("all four CircuitBreakReason literals validate", () => {
    const reasons = ["healed", "deepened", "severed", "closed"] as const;
    for (const reason of reasons) {
      const result = validateBaomaiV4ScarCircuitBrokenV1({
        v: 1,
        entity_id: "offline:TestPlayer",
        circuit: "heart_lung",
        reason,
        tick: 50,
      });
      expect(result.ok, `reason '${reason}' should validate`).toBe(true);
    }
  });

  it("invalid reason rejects", () => {
    const result = validateBaomaiV4ScarCircuitBrokenV1({
      v: 1,
      entity_id: "offline:TestPlayer",
      circuit: "heart_lung",
      reason: "exploded",
      tick: 50,
    });
    expect(result.ok).toBe(false);
  });

  it("missing reason rejects", () => {
    const result = validateBaomaiV4ScarCircuitBrokenV1({
      v: 1,
      entity_id: "offline:TestPlayer",
      circuit: "heart_lung",
      tick: 50,
    });
    expect(result.ok).toBe(false);
  });
});

// ── IronCocoonStageUp ─────────────────────────────────────────────────────────

describe("BaomaiV4IronCocoonStageUpV1", () => {
  it("all sample entries validate", () => {
    const samples = loadSamples("baomai_v4_iron_cocoon_stage_up.json");
    for (const [i, s] of samples.entries()) {
      const result = validateBaomaiV4IronCocoonStageUpV1(s);
      expect(result.ok, `sample[${i}] should be valid`).toBe(true);
    }
  });

  it("all IronCocoonStage literals validate", () => {
    const stages = ["none", "tough_skin", "iron_bone", "dull_flesh", "scar_forged"] as const;
    for (let i = 0; i + 1 < stages.length; i++) {
      const result = validateBaomaiV4IronCocoonStageUpV1({
        v: 1,
        entity_id: "offline:TestPlayer",
        from: stages[i],
        to: stages[i + 1],
        total_overloads: 50 * (i + 1),
        tick: 100,
      });
      expect(result.ok, `from=${stages[i]} to=${stages[i + 1]} should validate`).toBe(true);
    }
  });

  it("invalid stage literal rejects", () => {
    const result = validateBaomaiV4IronCocoonStageUpV1({
      v: 1,
      entity_id: "offline:TestPlayer",
      from: "none",
      to: "diamond_body",
      total_overloads: 50,
      tick: 100,
    });
    expect(result.ok).toBe(false);
  });

  it("negative total_overloads rejects", () => {
    const result = validateBaomaiV4IronCocoonStageUpV1({
      v: 1,
      entity_id: "offline:TestPlayer",
      from: "none",
      to: "tough_skin",
      total_overloads: -1,
      tick: 100,
    });
    expect(result.ok, "negative total_overloads must fail — minimum is 0").toBe(false);
  });
});

// ── ResonanceLock ─────────────────────────────────────────────────────────────

describe("BaomaiV4ResonanceLockV1", () => {
  it("all sample entries validate", () => {
    const samples = loadSamples("baomai_v4_resonance_lock.json");
    for (const [i, s] of samples.entries()) {
      const result = validateBaomaiV4ResonanceLockV1(s);
      expect(result.ok, `sample[${i}] should be valid`).toBe(true);
    }
  });

  it("valid payload validates", () => {
    const result = validateBaomaiV4ResonanceLockV1({
      v: 1,
      fighter_a_id: "offline:PlayerA",
      fighter_b_id: "offline:PlayerB",
      started_at: 1000,
      ends_at: 1060,
    });
    expect(result.ok).toBe(true);
  });

  it("missing fighter_b_id rejects", () => {
    const result = validateBaomaiV4ResonanceLockV1({
      v: 1,
      fighter_a_id: "offline:PlayerA",
      started_at: 1000,
      ends_at: 1060,
    });
    expect(result.ok).toBe(false);
  });
});

// ── ResonanceLockEnd ──────────────────────────────────────────────────────────

describe("BaomaiV4ResonanceLockEndV1", () => {
  it("all sample entries validate", () => {
    const samples = loadSamples("baomai_v4_resonance_lock_end.json");
    for (const [i, s] of samples.entries()) {
      const result = validateBaomaiV4ResonanceLockEndV1(s);
      expect(result.ok, `sample[${i}] should be valid`).toBe(true);
    }
  });

  it("expired reason validates", () => {
    const result = validateBaomaiV4ResonanceLockEndV1({
      v: 1,
      fighter_a_id: "offline:PlayerA",
      fighter_b_id: "offline:PlayerB",
      reason: { type: "expired" },
      tick: 1060,
    });
    expect(result.ok).toBe(true);
  });

  it("retreated reason validates", () => {
    const result = validateBaomaiV4ResonanceLockEndV1({
      v: 1,
      fighter_a_id: "offline:PlayerA",
      fighter_b_id: "offline:PlayerB",
      reason: { type: "retreated", entity_id: "offline:PlayerA" },
      tick: 1030,
    });
    expect(result.ok).toBe(true);
  });

  it("death reason validates", () => {
    const result = validateBaomaiV4ResonanceLockEndV1({
      v: 1,
      fighter_a_id: "offline:PlayerA",
      fighter_b_id: "offline:PlayerB",
      reason: { type: "death", entity_id: "offline:PlayerB" },
      tick: 1010,
    });
    expect(result.ok).toBe(true);
  });

  it("unknown reason type rejects", () => {
    const result = validateBaomaiV4ResonanceLockEndV1({
      v: 1,
      fighter_a_id: "offline:PlayerA",
      fighter_b_id: "offline:PlayerB",
      reason: { type: "surrendered" },
      tick: 1010,
    });
    expect(result.ok).toBe(false);
  });

  it("retreated missing entity_id rejects", () => {
    const result = validateBaomaiV4ResonanceLockEndV1({
      v: 1,
      fighter_a_id: "offline:PlayerA",
      fighter_b_id: "offline:PlayerB",
      reason: { type: "retreated" },
      tick: 1010,
    });
    expect(result.ok).toBe(false);
  });

  it("missing reason rejects", () => {
    const result = validateBaomaiV4ResonanceLockEndV1({
      v: 1,
      fighter_a_id: "offline:PlayerA",
      fighter_b_id: "offline:PlayerB",
      tick: 1010,
    });
    expect(result.ok).toBe(false);
  });
});
