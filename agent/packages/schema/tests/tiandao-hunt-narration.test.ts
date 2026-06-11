import { describe, expect, it } from "vitest";

import { validateTiandaoHuntNarrationRequestV1Contract } from "../src/tiandao-hunt-narration.js";

const U32_MAX = 4_294_967_295;
const MAX_ID = "p".repeat(128);
const TOO_LONG_ID = "p".repeat(129);
const MAX_ACTION = "a".repeat(128);
const TOO_LONG_ACTION = "a".repeat(129);

const basePayload = {
  v: 1,
  character_id: "offline:Alice",
  realm: "Spirit",
  attention_level: 72.5,
  response_level: "tribulation",
  zone: "血谷",
  recent_actions: ["activity:meditating", "countermeasure:deceive_heaven_revealed"],
  narration_count: 2,
};

describe("TiandaoHuntNarrationRequestV1", () => {
  it.each(["watch", "pressure", "tribulation", "annihilate"] as const)(
    "accepts response_level=%s",
    (responseLevel) => {
      const result = validateTiandaoHuntNarrationRequestV1Contract({
        ...basePayload,
        response_level: responseLevel,
      });

      expect(result.ok, result.errors.join("; ")).toBe(true);
    },
  );

  it.each([
    ["none response", { response_level: "none" }],
    ["bad realm", { realm: "GoldenCore" }],
    ["version below contract", { v: 0 }],
    ["version above contract", { v: 2 }],
    ["empty character_id", { character_id: "" }],
    ["too long character_id", { character_id: TOO_LONG_ID }],
    ["empty zone", { zone: "" }],
    ["too long zone", { zone: TOO_LONG_ID }],
    ["negative attention", { attention_level: -0.01 }],
    ["attention above max", { attention_level: 100.01 }],
    ["bad recent_actions", { recent_actions: "activity:meditating" }],
    ["empty recent_action item", { recent_actions: [""] }],
    ["too long recent_action item", { recent_actions: [TOO_LONG_ACTION] }],
    ["too many recent_actions", { recent_actions: Array.from({ length: 9 }, (_, i) => `a${i}`) }],
    ["negative narration_count", { narration_count: -1 }],
    ["narration_count above u32", { narration_count: U32_MAX + 1 }],
    ["extra field", { extra: true }],
  ])("rejects %s", (_name, patch) => {
    const result = validateTiandaoHuntNarrationRequestV1Contract({
      ...basePayload,
      ...patch,
    });

    expect(result.ok).toBe(false);
  });

  it.each([
    ["attention lower boundary", { attention_level: 0 }],
    ["attention upper boundary", { attention_level: 100 }],
    ["empty recent_actions", { recent_actions: [] }],
    ["max length character_id", { character_id: MAX_ID }],
    ["max length zone", { zone: MAX_ID }],
    ["max length recent_action item", { recent_actions: [MAX_ACTION] }],
    ["zero narration_count", { narration_count: 0 }],
    ["legacy 16-bit narration_count boundary", { narration_count: 65_535 }],
    ["above legacy 16-bit narration_count", { narration_count: 65_536 }],
    ["u32 max narration_count", { narration_count: U32_MAX }],
  ])("accepts %s", (_name, patch) => {
    const result = validateTiandaoHuntNarrationRequestV1Contract({
      ...basePayload,
      ...patch,
    });

    expect(result.ok, `expected boundary payload to pass; actual: ${JSON.stringify(result)}`).toBe(true);
  });

  it.each(["character_id", "realm", "zone", "recent_actions"] as const)(
    "rejects missing %s",
    (field) => {
      const payload = { ...basePayload };
      delete payload[field];

      const result = validateTiandaoHuntNarrationRequestV1Contract(payload);

      expect(
        result.ok,
        `expected validation to fail because ${field} is missing; actual: ${JSON.stringify(result)}`,
      ).toBe(false);
    },
  );
});
