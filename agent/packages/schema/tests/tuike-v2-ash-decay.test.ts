/**
 * plan-combat-skill-feedback-bridges-v1 P6 — TuikeAshDecayV1 TypeBox sample-pin 对拍。
 *
 * 与 server/src/schema/tuike_v2.rs 的 Rust serde 测试字段对齐：
 *   owner_id / output_item_id / tier / tick
 *
 * 样本文件：`samples/tuike_v2_ash_decay.json`（5 条：fan/light/mid/heavy/ancient）。
 */

import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

import {
  CHANNELS,
  validateTuikeAshDecayV1Contract,
} from "../src/index.js";

const __dirname = dirname(fileURLToPath(import.meta.url));
const samplesDir = join(__dirname, "..", "samples");

function loadSamplesArray(name: string): unknown[] {
  const raw = readFileSync(join(samplesDir, name), "utf-8");
  const parsed: unknown = JSON.parse(raw);
  if (!Array.isArray(parsed)) throw new Error(`${name}: expected JSON array`);
  return parsed;
}

describe("TuikeAshDecayV1 sample-pin（Rust ↔ TS 双端 wire 对拍）", () => {
  const samples = loadSamplesArray("tuike_v2_ash_decay.json");

  it("sample 文件恰好包含 5 条（fan/light/mid/heavy/ancient 各一）", () => {
    expect(samples).toHaveLength(5);
  });

  it("全部 5 条 sample 均通过 TypeBox validateTuikeAshDecayV1Contract（正例）", () => {
    for (let i = 0; i < samples.length; i++) {
      const result = validateTuikeAshDecayV1Contract(samples[i]);
      expect(
        result.ok,
        `sample[${i}] should be accepted by TypeBox validator; errors: ${result.errors.join("; ")}`,
      ).toBe(true);
    }
  });

  it("sample[0] mid tier — 字段值 pin", () => {
    const s = samples[0] as Record<string, unknown>;
    expect(s["owner_id"]).toBe("offline:XiaoMing");
    expect(s["output_item_id"]).toBe("bong:false_skin_ash");
    expect(s["tier"]).toBe("mid");
    expect(s["tick"]).toBe(1000);
  });

  it("sample[4] ancient tier — output_item_id = relic shard（边界：上古档专属物品）", () => {
    const s = samples[4] as Record<string, unknown>;
    expect(s["tier"]).toBe("ancient");
    expect(s["output_item_id"]).toBe("bong:false_skin_ancient_relic_shard");
    const result = validateTuikeAshDecayV1Contract(s);
    expect(result.ok).toBe(true);
  });

  it("CHANNELS.TUIKE_V2_ASH_DECAY 对应正确频道名（channel pin）", () => {
    expect(CHANNELS.TUIKE_V2_ASH_DECAY).toBe("bong:tuike_v2/ash_decay");
  });

  it("反例：额外字段被 additionalProperties:false 拒绝", () => {
    const extra = {
      owner_id: "offline:TestPlayer",
      output_item_id: "bong:false_skin_ash",
      tier: "mid",
      tick: 100,
      extra_field: "forbidden",
    };
    const result = validateTuikeAshDecayV1Contract(extra);
    expect(
      result.ok,
      `extra_field should be rejected; errors: ${result.errors.join("; ")}`,
    ).toBe(false);
  });

  it("反例：缺少 owner_id 被拒绝", () => {
    const missing = {
      output_item_id: "bong:false_skin_ash",
      tier: "mid",
      tick: 100,
    };
    const result = validateTuikeAshDecayV1Contract(missing);
    expect(result.ok).toBe(false);
  });

  it("反例：无效 tier 字符串被拒绝", () => {
    const invalid = {
      owner_id: "offline:TestPlayer",
      output_item_id: "bong:false_skin_ash",
      tier: "ultra_ancient",
      tick: 100,
    };
    const result = validateTuikeAshDecayV1Contract(invalid);
    expect(result.ok).toBe(false);
  });

  it("反例：output_item_id 空字符串被拒绝（minLength:1）", () => {
    const empty = {
      owner_id: "offline:TestPlayer",
      output_item_id: "",
      tier: "fan",
      tick: 1,
    };
    const result = validateTuikeAshDecayV1Contract(empty);
    expect(result.ok).toBe(false);
  });
});
