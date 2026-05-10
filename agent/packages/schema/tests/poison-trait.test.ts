import { describe, expect, it } from "vitest";

import {
  ServerDataV1,
  validatePoisonDoseEventV1,
  validatePoisonOverdoseEventV1,
  validatePoisonTraitStateV1,
} from "../src/index.js";
import { validate } from "../src/validate.js";

describe("poison trait schema", () => {
  it("caps entity ids at JavaScript safe integer precision", () => {
    expect(validatePoisonDoseEventV1({
      v: 1,
      player_entity_id: Number.MAX_SAFE_INTEGER + 1,
      dose_amount: 5,
      side_effect_tag: "qi_focus_drift_2h",
      poison_level_after: 17,
      digestion_after: 50,
      at_tick: 100,
    }).ok).toBe(false);
  });

  it("validates overdose and state payload contracts", () => {
    expect(validatePoisonOverdoseEventV1({
      v: 1,
      player_entity_id: 7,
      severity: "moderate",
      overflow: 30,
      lifespan_penalty_years: 1,
      micro_tear_probability: 0.1,
      at_tick: 120,
    }).ok).toBe(true);
    expect(validatePoisonTraitStateV1({
      v: 1,
      player_entity_id: 7,
      poison_toxicity: 17,
      digestion_current: 50,
      digestion_capacity: 100,
      toxicity_tier_unlocked: false,
    }).ok).toBe(true);
  });

  it("accepts poison server_data envelope variants", () => {
    expect(validate(ServerDataV1, {
      v: 1,
      type: "poison_trait_state",
      player_entity_id: 7,
      poison_toxicity: 17,
      digestion_current: 50,
      digestion_capacity: 100,
      toxicity_tier_unlocked: false,
    }).ok).toBe(true);
  });
});
