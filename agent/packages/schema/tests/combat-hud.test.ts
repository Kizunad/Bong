import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

import {
  ServerDataCombatHudStateV1,
  ServerDataType,
  ServerDataV1,
} from "../src/server-data.js";
import { CombatHudStateV1 } from "../src/combat-hud.js";
import { renderGeneratedSchemas } from "../src/generated-artifacts.js";
import {
  GENERATED_SCHEMA_FILES,
  SCHEMA_REGISTRY,
} from "../src/schema-registry.js";
import { validate } from "../src/validate.js";

const __dirname = dirname(fileURLToPath(import.meta.url));
const samplesDir = join(__dirname, "..", "samples");

const COMBAT_HUD_FIELDS = [
  "hp_percent",
  "qi_percent",
  "stamina_percent",
  "combat_active",
  "derived",
] as const;

function loadSample(name: string): unknown {
  return JSON.parse(readFileSync(join(samplesDir, name), "utf8"));
}

describe("combat_hud_state ServerDataV1 contract", () => {
  it.each([
    "server-data.combat-hud-state.active.sample.json",
    "server-data.combat-hud-state.inactive.sample.json",
  ])("accepts positive sample %s", (sampleName) => {
    const sample = loadSample(sampleName);
    const payloadResult = validate(ServerDataCombatHudStateV1, sample);
    const unionResult = validate(ServerDataV1, sample);

    expect(
      payloadResult.ok,
      `${sampleName} 应满足 ServerDataCombatHudStateV1：${payloadResult.errors.join("; ")}`,
    ).toBe(true);
    expect(
      unionResult.ok,
      `${sampleName} 应被 ServerDataV1 union 接受：${unionResult.errors.join("; ")}`,
    ).toBe(true);
  });

  it.each([
    "server-data.combat-hud-state.invalid-missing-field.sample.json",
    "server-data.combat-hud-state.invalid-out-of-range.sample.json",
    "server-data.combat-hud-state.invalid-extra-field.sample.json",
  ])("rejects negative sample %s", (sampleName) => {
    const sample = loadSample(sampleName);

    expect(
      validate(ServerDataCombatHudStateV1, sample).ok,
      `${sampleName} 必须被 wrapper 拒绝，防止缺少或伪造权威战斗状态`,
    ).toBe(false);
    expect(
      validate(ServerDataV1, sample).ok,
      `${sampleName} 必须被 ServerDataV1 union 拒绝，防止绕过 combat_hud_state 契约`,
    ).toBe(false);
  });

  it("registers the canonical combat_hud_state type and rejects unversioned variants", () => {
    expect(
      validate(ServerDataType, "combat_hud_state").ok,
      "combat_hud_state 必须登记为 canonical ServerDataType wire tag",
    ).toBe(true);
    expect(
      validate(ServerDataType, "combat_hud_state_v2").ok,
      "combat_hud_state_v2 未经版本登记时必须拒绝",
    ).toBe(false);
  });

  it("pins combat_active as a required boolean in TypeBox and generated JSON", () => {
    expect(Object.keys(CombatHudStateV1.properties)).toEqual(COMBAT_HUD_FIELDS);
    expect(Object.keys(ServerDataCombatHudStateV1.properties)).toEqual([
      "v",
      "type",
      ...COMBAT_HUD_FIELDS,
    ]);

    const generatedState = JSON.parse(
      renderGeneratedSchemas()["combat-hud-state-v1.json"],
    ) as {
      properties: Record<string, Record<string, unknown>>;
      required: string[];
      additionalProperties: boolean;
    };
    expect(generatedState.properties.combat_active).toEqual({ type: "boolean" });
    expect(generatedState.required).toEqual(COMBAT_HUD_FIELDS);
    expect(generatedState.additionalProperties).toBe(false);

    const generatedWrapper = JSON.parse(
      renderGeneratedSchemas()["server-data-combat-hud-state-v1.json"],
    ) as {
      properties: Record<string, Record<string, unknown>>;
      required: string[];
      additionalProperties: boolean;
    };
    expect(generatedWrapper.properties.combat_active).toEqual({ type: "boolean" });
    expect(generatedWrapper.required).toEqual(["v", "type", ...COMBAT_HUD_FIELDS]);
    expect(generatedWrapper.additionalProperties).toBe(false);
    expect(renderGeneratedSchemas()["server-data-v1.json"]).toContain(
      '"const": "combat_hud_state"',
    );
  });

  it("registers standalone and wrapped generated artifacts", () => {
    expect(SCHEMA_REGISTRY.combatHudStateV1).toBe(CombatHudStateV1);
    expect(SCHEMA_REGISTRY.serverDataCombatHudStateV1).toBe(
      ServerDataCombatHudStateV1,
    );
    expect(GENERATED_SCHEMA_FILES["combat-hud-state-v1.json"]).toBe(
      CombatHudStateV1,
    );
    expect(GENERATED_SCHEMA_FILES["server-data-combat-hud-state-v1.json"]).toBe(
      ServerDataCombatHudStateV1,
    );
  });
});
