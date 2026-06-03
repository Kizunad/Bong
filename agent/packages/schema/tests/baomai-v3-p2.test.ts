/**
 * plan-combat-skill-feedback-bridges-v1 P2 — BaomaiV3 残余事件 schema 双端 sample 对拍测试。
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
  validateBaomaiV3MountainShakeV1,
  validateBaomaiV3BloodBurnV1,
  validateBaomaiV3TranscendenceExpiredV1,
  validateBaomaiV3OverloadRippleV1,
} from "../src/baomai-v3.js";

const __dirname = dirname(fileURLToPath(import.meta.url));

function loadSamples(filename: string): unknown[] {
  const path = join(__dirname, "..", "samples", filename);
  return JSON.parse(readFileSync(path, "utf-8")) as unknown[];
}

// ── channels pin ──────────────────────────────────────────────────────────────

describe("CHANNELS baomai_v3 P2 constants pin", () => {
  it("BAOMAI_V3_MOUNTAIN_SHAKE is frozen", () => {
    expect(
      CHANNELS.BAOMAI_V3_MOUNTAIN_SHAKE,
      "channel constant must not change — server Rust and agent TS both depend on it",
    ).toBe("bong:baomai_v3/mountain_shake");
  });

  it("BAOMAI_V3_BLOOD_BURN is frozen", () => {
    expect(CHANNELS.BAOMAI_V3_BLOOD_BURN).toBe("bong:baomai_v3/blood_burn");
  });

  it("BAOMAI_V3_TRANSCENDENCE_EXPIRED is frozen", () => {
    expect(CHANNELS.BAOMAI_V3_TRANSCENDENCE_EXPIRED).toBe(
      "bong:baomai_v3/transcendence_expired",
    );
  });

  it("BAOMAI_V3_OVERLOAD_RIPPLE is frozen", () => {
    expect(CHANNELS.BAOMAI_V3_OVERLOAD_RIPPLE).toBe("bong:baomai_v3/overload_ripple");
  });
});

// ── BaomaiV3MountainShakeV1 ───────────────────────────────────────────────────

describe("BaomaiV3MountainShakeV1", () => {
  it("all sample entries validate", () => {
    const samples = loadSamples("baomai_v3_mountain_shake.json");
    expect(
      samples.length,
      "sample file must have at least 1 entry",
    ).toBeGreaterThanOrEqual(1);
    for (const [i, s] of samples.entries()) {
      const result = validateBaomaiV3MountainShakeV1(s);
      expect(result.ok, `sample[${i}] should be valid: ${JSON.stringify(result)}`).toBe(true);
    }
  });

  it("valid payload validates", () => {
    const result = validateBaomaiV3MountainShakeV1({
      v: 1,
      caster_id: "offline:TestPlayer",
      affected_count: 3,
      tick: 200,
      qi_spent: 1200.0,
      radius_blocks: 5.0,
      shock_damage: 420.0,
    });
    expect(result.ok).toBe(true);
  });

  it("affected_count=0 (no hits) validates", () => {
    const result = validateBaomaiV3MountainShakeV1({
      v: 1,
      caster_id: "offline:TestPlayer",
      affected_count: 0,
      tick: 1,
      qi_spent: 0.0,
      radius_blocks: 0.0,
      shock_damage: 0.0,
    });
    expect(result.ok, "affected_count=0 must be valid (no targets hit)").toBe(true);
  });

  it("missing caster_id rejects", () => {
    const result = validateBaomaiV3MountainShakeV1({
      v: 1,
      affected_count: 3,
      tick: 200,
      qi_spent: 1200.0,
      radius_blocks: 5.0,
      shock_damage: 420.0,
    });
    expect(result.ok, "missing caster_id must fail").toBe(false);
  });

  it("additional fields rejected", () => {
    const result = validateBaomaiV3MountainShakeV1({
      v: 1,
      caster_id: "offline:TestPlayer",
      affected_count: 1,
      tick: 1,
      qi_spent: 0,
      radius_blocks: 0,
      shock_damage: 0,
      extra: "forbidden",
    });
    expect(result.ok, "extra field must be rejected by additionalProperties:false").toBe(false);
  });

  it("negative qi_spent rejects", () => {
    const result = validateBaomaiV3MountainShakeV1({
      v: 1,
      caster_id: "offline:TestPlayer",
      affected_count: 0,
      tick: 1,
      qi_spent: -1.0,
      radius_blocks: 0,
      shock_damage: 0,
    });
    expect(result.ok, "negative qi_spent must fail — minimum is 0").toBe(false);
  });
});

// ── BaomaiV3BloodBurnV1 ───────────────────────────────────────────────────────

describe("BaomaiV3BloodBurnV1", () => {
  it("all sample entries validate", () => {
    const samples = loadSamples("baomai_v3_blood_burn.json");
    expect(samples.length).toBeGreaterThanOrEqual(1);
    for (const [i, s] of samples.entries()) {
      const result = validateBaomaiV3BloodBurnV1(s);
      expect(result.ok, `sample[${i}] should be valid`).toBe(true);
    }
  });

  it("ended_in_near_death=true validates", () => {
    const result = validateBaomaiV3BloodBurnV1({
      v: 1,
      caster_id: "offline:VoidMaster",
      tick: 500,
      hp_burned: 300.0,
      qi_multiplier: 5.0,
      active_until_tick: 500,
      ended_in_near_death: true,
    });
    expect(result.ok).toBe(true);
  });

  it("ended_in_near_death=false validates", () => {
    const result = validateBaomaiV3BloodBurnV1({
      v: 1,
      caster_id: "offline:TestPlayer",
      tick: 300,
      hp_burned: 150.0,
      qi_multiplier: 3.5,
      active_until_tick: 360,
      ended_in_near_death: false,
    });
    expect(result.ok).toBe(true);
  });

  it("missing ended_in_near_death rejects", () => {
    const result = validateBaomaiV3BloodBurnV1({
      v: 1,
      caster_id: "offline:TestPlayer",
      tick: 300,
      hp_burned: 150.0,
      qi_multiplier: 3.5,
      active_until_tick: 360,
    });
    expect(result.ok, "missing ended_in_near_death must fail").toBe(false);
  });

  it("qi_multiplier < 1 rejects", () => {
    const result = validateBaomaiV3BloodBurnV1({
      v: 1,
      caster_id: "offline:TestPlayer",
      tick: 300,
      hp_burned: 150.0,
      qi_multiplier: 0.5, // below minimum:1
      active_until_tick: 360,
      ended_in_near_death: false,
    });
    expect(result.ok, "qi_multiplier < 1 must fail — minimum is 1").toBe(false);
  });

  it("additional fields rejected", () => {
    const result = validateBaomaiV3BloodBurnV1({
      v: 1,
      caster_id: "offline:TestPlayer",
      tick: 300,
      hp_burned: 150.0,
      qi_multiplier: 1.0,
      active_until_tick: 360,
      ended_in_near_death: false,
      extra_field: "bad",
    });
    expect(result.ok).toBe(false);
  });
});

// ── BaomaiV3TranscendenceExpiredV1 ────────────────────────────────────────────

describe("BaomaiV3TranscendenceExpiredV1", () => {
  it("all sample entries validate", () => {
    const samples = loadSamples("baomai_v3_transcendence_expired.json");
    expect(samples.length).toBeGreaterThanOrEqual(1);
    for (const [i, s] of samples.entries()) {
      const result = validateBaomaiV3TranscendenceExpiredV1(s);
      expect(result.ok, `sample[${i}] should be valid`).toBe(true);
    }
  });

  it("minimal valid payload validates", () => {
    const result = validateBaomaiV3TranscendenceExpiredV1({
      v: 1,
      caster_id: "offline:TestPlayer",
      tick: 700,
    });
    expect(result.ok).toBe(true);
  });

  it("missing tick rejects", () => {
    const result = validateBaomaiV3TranscendenceExpiredV1({
      v: 1,
      caster_id: "offline:TestPlayer",
    });
    expect(result.ok, "missing tick must fail").toBe(false);
  });

  it("missing caster_id rejects", () => {
    const result = validateBaomaiV3TranscendenceExpiredV1({
      v: 1,
      tick: 700,
    });
    expect(result.ok, "missing caster_id must fail").toBe(false);
  });

  it("additional fields rejected", () => {
    const result = validateBaomaiV3TranscendenceExpiredV1({
      v: 1,
      caster_id: "offline:TestPlayer",
      tick: 700,
      extra: "bad",
    });
    expect(result.ok, "extra field must be rejected").toBe(false);
  });

  it("empty caster_id rejects (minLength:1)", () => {
    const result = validateBaomaiV3TranscendenceExpiredV1({
      v: 1,
      caster_id: "",
      tick: 700,
    });
    expect(result.ok, "empty caster_id must fail — minLength:1").toBe(false);
  });
});

// ── BaomaiV3OverloadRippleV1 ──────────────────────────────────────────────────

describe("BaomaiV3OverloadRippleV1", () => {
  it("all sample entries validate", () => {
    const samples = loadSamples("baomai_v3_overload_ripple.json");
    expect(samples.length).toBeGreaterThanOrEqual(1);
    for (const [i, s] of samples.entries()) {
      const result = validateBaomaiV3OverloadRippleV1(s);
      expect(result.ok, `sample[${i}] should be valid`).toBe(true);
    }
  });

  it("all BaomaiSkillId literals validate", () => {
    const skills = [
      "beng_quan",
      "full_power_charge",
      "full_power_release",
      "mountain_shake",
      "blood_burn",
      "disperse",
    ] as const;
    for (const skill of skills) {
      const result = validateBaomaiV3OverloadRippleV1({
        v: 1,
        caster_id: "offline:TestPlayer",
        tick: 100,
        skill_id: skill,
        severity_delta: 0.05,
        total_severity: 0.1,
        meridian_ids: ["Lung"],
      });
      expect(result.ok, `skill_id '${skill}' should validate`).toBe(true);
    }
  });

  it("invalid skill_id rejects", () => {
    const result = validateBaomaiV3OverloadRippleV1({
      v: 1,
      caster_id: "offline:TestPlayer",
      tick: 100,
      skill_id: "unknown_skill",
      severity_delta: 0.05,
      total_severity: 0.1,
      meridian_ids: [],
    });
    expect(result.ok, "unknown skill_id must be rejected — closed enum").toBe(false);
  });

  it("empty meridian_ids array validates", () => {
    const result = validateBaomaiV3OverloadRippleV1({
      v: 1,
      caster_id: "offline:TestPlayer",
      tick: 100,
      skill_id: "beng_quan",
      severity_delta: 0.05,
      total_severity: 0.1,
      meridian_ids: [],
    });
    expect(result.ok, "empty meridian_ids should be valid").toBe(true);
  });

  it("missing skill_id rejects", () => {
    const result = validateBaomaiV3OverloadRippleV1({
      v: 1,
      caster_id: "offline:TestPlayer",
      tick: 100,
      severity_delta: 0.05,
      total_severity: 0.1,
      meridian_ids: [],
    });
    expect(result.ok, "missing skill_id must fail").toBe(false);
  });

  it("negative severity_delta rejects", () => {
    const result = validateBaomaiV3OverloadRippleV1({
      v: 1,
      caster_id: "offline:TestPlayer",
      tick: 100,
      skill_id: "beng_quan",
      severity_delta: -0.05,
      total_severity: 0.1,
      meridian_ids: [],
    });
    expect(result.ok, "negative severity_delta must fail — minimum:0").toBe(false);
  });

  it("additional fields rejected", () => {
    const result = validateBaomaiV3OverloadRippleV1({
      v: 1,
      caster_id: "offline:TestPlayer",
      tick: 100,
      skill_id: "beng_quan",
      severity_delta: 0.05,
      total_severity: 0.1,
      meridian_ids: [],
      extra: "bad",
    });
    expect(result.ok).toBe(false);
  });
});
