import { describe, expect, it } from "vitest";

import { validateTiandaoHuntNarrationRequestV1Contract } from "../src/tiandao-hunt-narration.js";

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
    ["negative attention", { attention_level: -0.01 }],
    ["attention above max", { attention_level: 100.01 }],
    ["bad recent_actions", { recent_actions: "activity:meditating" }],
    ["too many recent_actions", { recent_actions: Array.from({ length: 9 }, (_, i) => `a${i}`) }],
    ["bad narration_count", { narration_count: -1 }],
    ["extra field", { extra: true }],
  ])("rejects %s", (_name, patch) => {
    const result = validateTiandaoHuntNarrationRequestV1Contract({
      ...basePayload,
      ...patch,
    });

    expect(result.ok).toBe(false);
  });

  it.each(["character_id", "realm", "zone", "recent_actions"] as const)(
    "rejects missing %s",
    (field) => {
      const payload = { ...basePayload };
      delete payload[field];

      const result = validateTiandaoHuntNarrationRequestV1Contract(payload);

      expect(result.ok).toBe(false);
    },
  );
});
