import { CHANNELS, REDIS_V1_CHANNELS } from "../src/channels.js";
import {
  validateHealerNpcAiStateV1Contract,
  validateYidaoEventV1Contract,
  validateYidaoHudStateV1Contract,
} from "../src/yidao.js";
import { describe, expect, it } from "vitest";

describe("yidao schema", () => {
  it("accepts treatment event payloads and rejects unknown fields", () => {
    const valid = {
      v: 1,
      kind: "mass_heal",
      tick: 42,
      medic_id: "entity_bits:1",
      patient_ids: ["entity_bits:2", "entity_bits:3"],
      skill: "mass_meridian_repair",
      meridian_id: "Lung",
      success_count: 2,
      failure_count: 0,
      qi_transferred: 120,
      contam_reduced: 0,
      hp_restored: 0,
      karma_delta: 0.2,
      medic_qi_max_delta: -0.04,
      patient_qi_max_delta: 0,
      contract_state: "patient",
      detail: "mass repair",
    };

    expect(validateYidaoEventV1Contract(valid).ok).toBe(true);
    expect(validateYidaoEventV1Contract({ ...valid, extra: true }).ok).toBe(false);
  });

  it("declares healer AI and HUD server-data DTOs", () => {
    expect(
      validateHealerNpcAiStateV1Contract({
        healer_id: "npc:doctor",
        active_action: "meridian_repair",
        queue_len: 2,
        reputation: 5,
        retreating: false,
      }).ok,
    ).toBe(true);

    expect(
      validateYidaoHudStateV1Contract({
        healer_id: "npc:doctor",
        reputation: 5,
        peace_mastery: 48,
        karma: 3.5,
        active_skill: "life_extension",
        patient_ids: ["entity_bits:2"],
        patient_hp_percent: 0.5,
        patient_contam_total: null,
        severed_meridian_count: 1,
        contract_count: 2,
        mass_preview_count: 0,
      }).ok,
    ).toBe(true);
  });

  it("freezes the redis channel name", () => {
    expect(CHANNELS.YIDAO_EVENT).toBe("bong:yidao/event");
    expect(REDIS_V1_CHANNELS).toContain(CHANNELS.YIDAO_EVENT);
  });
});
