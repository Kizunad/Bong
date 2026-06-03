/**
 * plan-combat-skill-feedback-bridges-v1 P0 — MeridianSeveredEventV1 TypeBox sample-pin 对拍。
 *
 * 与 server/src/schema/meridian_severed.rs 的 Rust serde 测试使用同一个
 * `samples/meridian-severed-event.sample.json`（7 条），确保双端 wire 形态对齐。
 */

import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

import {
  CHANNELS,
  validateMeridianSeveredEventV1Contract,
} from "../src/index.js";

const __dirname = dirname(fileURLToPath(import.meta.url));
const samplesDir = join(__dirname, "..", "samples");

function loadSamplesArray(name: string): unknown[] {
  const raw = readFileSync(join(samplesDir, name), "utf-8");
  const parsed: unknown = JSON.parse(raw);
  if (!Array.isArray(parsed)) throw new Error(`${name}: expected JSON array`);
  return parsed;
}

describe("MeridianSeveredEventV1 sample-pin（Rust ↔ TS 双端 wire 对拍）", () => {
  const samples = loadSamplesArray("meridian-severed-event.sample.json");

  it("sample 文件恰好包含 7 条（与 Rust serde 测试数量对齐）", () => {
    expect(samples).toHaveLength(7);
  });

  it("全部 7 条 sample 均通过 TypeBox validateMeridianSeveredEventV1Contract（正例）", () => {
    for (let i = 0; i < samples.length; i++) {
      const result = validateMeridianSeveredEventV1Contract(samples[i]);
      expect(
        result.ok,
        `sample[${i}] should be accepted by TypeBox validator; errors: ${result.errors.join("; ")}`,
      ).toBe(true);
    }
  });

  it("sample[0] VoluntarySever — 字段值 pin", () => {
    const s = samples[0] as Record<string, unknown>;
    expect(s["v"]).toBe(1);
    expect(s["type"]).toBe("meridian_severed");
    expect(s["entity_id"]).toBe("offline:XiaoMing");
    expect(s["meridian_id"]).toBe("Lung");
    expect(s["source"]).toBe("VoluntarySever");
    expect(s["at_tick"]).toBe(1200);
  });

  it("sample[6] Other — source 是对象 {Other: string} 形态（外部标签 wire 对齐）", () => {
    const s = samples[6] as Record<string, unknown>;
    expect(s["source"]).toEqual({ Other: "上古遗物反噬" });
    expect(s["meridian_id"]).toBe("Chong");
    const result = validateMeridianSeveredEventV1Contract(s);
    expect(result.ok).toBe(true);
  });

  it("CHANNELS.MERIDIAN_SEVERED 对应正确频道名（channel pin）", () => {
    expect(CHANNELS.MERIDIAN_SEVERED).toBe("bong:meridian_severed");
  });

  it("反例：额外字段被 additionalProperties:false 拒绝", () => {
    const extra = {
      v: 1,
      type: "meridian_severed",
      entity_id: "offline:TestPlayer",
      meridian_id: "Lung",
      source: "CombatWound",
      at_tick: 100,
      extra_field: "forbidden",
    };
    const result = validateMeridianSeveredEventV1Contract(extra);
    expect(
      result.ok,
      `extra_field should be rejected by additionalProperties:false; errors: ${result.errors.join("; ")}`,
    ).toBe(false);
  });

  it("反例：缺少 meridian_id 被拒绝", () => {
    const missing = {
      v: 1,
      type: "meridian_severed",
      entity_id: "offline:TestPlayer",
      source: "CombatWound",
      at_tick: 100,
    };
    const result = validateMeridianSeveredEventV1Contract(missing);
    expect(result.ok).toBe(false);
  });

  it("反例：无效 source 字符串被拒绝", () => {
    const invalid = {
      v: 1,
      type: "meridian_severed",
      entity_id: "offline:TestPlayer",
      meridian_id: "Lung",
      source: "ImmortalGodMode",
      at_tick: 100,
    };
    const result = validateMeridianSeveredEventV1Contract(invalid);
    expect(result.ok).toBe(false);
  });
});
