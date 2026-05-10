import { describe, expect, it } from "vitest";

import { validateBaomaiSkillEventV1Contract } from "../src/baomai-v3.js";

describe("plan-baomai-v3 schema contracts", () => {
  it("accepts a disperse flow-rate window payload without immunity fields", () => {
    const result = validateBaomaiSkillEventV1Contract({
      v: 1,
      type: "baomai_skill_event",
      skill_id: "disperse",
      caster_id: "offline:Azure",
      tick: 120,
      qi_invested: 5350,
      damage: 0,
      blood_multiplier: 1,
      flow_rate_multiplier: 10,
      meridian_ids: ["Ren", "Du"],
    });

    expect(result.ok).toBe(true);
  });

  it("rejects unknown skill ids", () => {
    const result = validateBaomaiSkillEventV1Contract({
      v: 1,
      type: "baomai_skill_event",
      skill_id: "immune_god_mode",
      caster_id: "offline:Azure",
      tick: 120,
      qi_invested: 0,
      damage: 0,
      blood_multiplier: 1,
      flow_rate_multiplier: 1,
      meridian_ids: [],
    });

    expect(result.ok).toBe(false);
  });
});
