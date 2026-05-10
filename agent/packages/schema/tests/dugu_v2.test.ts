import { describe, expect, it } from "vitest";

import { CHANNELS } from "../src/channels.js";
import {
  validateDuguReverseTriggeredV1Contract,
  validateDuguSelfCureProgressV1Contract,
  validateDuguV2SkillCastV1Contract,
} from "../src/dugu_v2.js";

describe("dugu v2 contracts", () => {
  it("declares dedicated narration channels", () => {
    expect(CHANNELS.DUGU_V2_CAST).toBe("bong:dugu_v2/cast");
    expect(CHANNELS.DUGU_V2_SELF_CURE).toBe("bong:dugu_v2/self_cure");
    expect(CHANNELS.DUGU_V2_REVERSE).toBe("bong:dugu_v2/reverse");
  });

  it("accepts cast, self-cure, and reverse payloads", () => {
    expect(
      validateDuguV2SkillCastV1Contract({
        caster: "player:a",
        target: "player:b",
        skill: "eclipse",
        tick: 1,
        taint_tier: "permanent",
        hp_loss: 20,
        qi_loss: 40,
        qi_max_loss: 0,
        permanent_decay_rate_per_min: 0.001,
        returned_zone_qi: 39.6,
        reveal_probability: 0.03,
        animation_id: "bong:dugu_needle_throw",
        particle_id: "bong:dugu_taint_pulse",
        sound_recipe_id: "dugu_needle_hiss",
        icon_texture: "bong:textures/gui/skill/dugu_eclipse.png",
      }).ok,
    ).toBe(true);
    expect(
      validateDuguSelfCureProgressV1Contract({
        caster: "player:a",
        hours_used: 1,
        daily_hours_after: 1,
        gain_percent: 1.5,
        insidious_color_percent: 12,
        morphology_percent: 12,
        self_revealed: false,
        tick: 2,
      }).ok,
    ).toBe(true);
    expect(
      validateDuguReverseTriggeredV1Contract({
        caster: "player:a",
        affected_targets: 3,
        burst_damage: 180,
        returned_zone_qi: 14.85,
        juebi_delay_ticks: 600,
        center: [0, 64, 0],
        tick: 3,
      }).ok,
    ).toBe(true);
  });
});
