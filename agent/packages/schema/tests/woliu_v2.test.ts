import { describe, expect, it } from "vitest";

import {
  validateTurbulenceFieldV1Contract,
  validateWoliuBackfireV1Contract,
  validateWoliuPullDisplaceV1Contract,
  validateWoliuSkillCastV1Contract,
} from "../src/woliu_v2.js";

describe("plan-woliu-v2 schema contracts", () => {
  it("accepts a five-skill cast payload with visual and audio channels", () => {
    expect(validateWoliuSkillCastV1Contract({
      caster: "player:kiz",
      skill: "heart",
      tick: 120,
      lethal_radius: 5,
      influence_radius: 300,
      turbulence_radius: 75,
      absorbed_qi: 0.8,
      swirl_qi: 99,
      animation_id: "bong:vortex_spiral_stance",
      particle_id: "bong:vortex_spiral",
      sound_recipe_id: "vortex_low_hum",
      icon_texture: "bong:textures/gui/skill/woliu_heart.png",
    }).ok).toBe(true);
  });

  it("accepts woliu-v3 vacuum package skill ids", () => {
    for (const skill of ["vacuum_palm", "vortex_shield", "vacuum_lock", "vortex_resonance", "turbulence_burst"] as const) {
      expect(validateWoliuSkillCastV1Contract({
        caster: "player:kiz",
        skill,
        tick: 121,
        lethal_radius: 1,
        influence_radius: 8,
        turbulence_radius: 1.5,
        absorbed_qi: 15,
        swirl_qi: 2,
        animation_id: `bong:woliu_${skill}`,
        particle_id: `bong:woliu_${skill}_spiral`,
        sound_recipe_id: `woliu_${skill}`,
        icon_texture: "bong:textures/gui/skill/woliu_mouth.png",
      }).ok).toBe(true);
    }
  });

  it("rejects unknown skill ids", () => {
    expect(validateWoliuSkillCastV1Contract({
      caster: "player:kiz",
      skill: "black_hole",
      tick: 1,
      lethal_radius: 1,
      influence_radius: 1,
      turbulence_radius: 1,
      absorbed_qi: 0,
      swirl_qi: 0,
      animation_id: "bong:vortex_palm_open",
      particle_id: "bong:vortex_spiral",
      sound_recipe_id: "vortex_low_hum",
      icon_texture: "bong:textures/gui/skill/woliu_hold.png",
    }).ok).toBe(false);
  });

  it("accepts severed backfire payloads", () => {
    expect(validateWoliuBackfireV1Contract({
      caster: "player:kiz",
      skill: "heart",
      level: "severed",
      cause: "tsy_negative_field",
      overflow_qi: 80,
      tick: 42,
    }).ok).toBe(true);
  });

  it("accepts turbulence field narration payloads", () => {
    expect(validateTurbulenceFieldV1Contract({
      caster: "player:kiz",
      skill: "mouth",
      center: [1, 64, 1],
      radius: 10,
      intensity: 0.75,
      swirl_qi: 30,
      tick: 44,
    }).ok).toBe(true);
  });

  it("accepts pull displacement payloads", () => {
    expect(validateWoliuPullDisplaceV1Contract({
      caster: "player:kiz",
      target: "entity:2",
      displacement_blocks: 3.5,
      tick: 45,
    }).ok).toBe(true);
  });
});
