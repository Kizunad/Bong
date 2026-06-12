import { describe, expect, it } from "vitest";

import {
  ClientRequestV1,
  validateZhenfaV2EventV1Contract,
} from "../src/index.js";
import { validate } from "../src/validate.js";

describe("zhenfa-v2 schema", () => {
  const deceiveHeavenExposureEvent = (chance: unknown) => ({
    v: 1,
    event: "deceive_heaven_exposed",
    array_id: 8,
    kind: "deceive_heaven",
    owner: "offline:Azure",
    breaker: "offline:Breaker",
    x: 12,
    y: 64,
    z: -9,
    tick: 160,
    reveal_chance_lifetime: chance,
    self_weight_multiplier: 0.5,
    target_weight_multiplier: 1.5,
  });

  it("accepts new array kinds in zhenfa_place requests", () => {
    const result = validate(ClientRequestV1, {
      v: 1,
      type: "zhenfa_place",
      x: 1,
      y: 64,
      z: -2,
      kind: "deceive_heaven",
      carrier: "beast_core_inlaid",
      qi_invest_ratio: 0.9,
    });

    expect(result.ok).toBe(true);

    const contentResult = validate(ClientRequestV1, {
      v: 1,
      type: "zhenfa_place",
      x: 2,
      y: 64,
      z: -3,
      kind: "slow_trap",
      qi_invest_ratio: 0,
      item_instance_id: 9002,
      target_face: "top",
    });

    expect(contentResult.ok).toBe(true);

    const networkArrayResult = validate(ClientRequestV1, {
      v: 1,
      type: "zhenfa_place",
      x: 3,
      y: 64,
      z: -4,
      kind: "network_array",
      qi_invest_ratio: 0,
      item_instance_id: 9003,
    });
    expect(networkArrayResult.ok).toBe(true);

    for (const [kind, itemInstanceId, targetFace] of [
      ["beast_trap", 9004, "north"],
      ["trip_wire", 9005, "north"],
      ["decoy_stake", 9006, "top"],
    ] as const) {
      const trapRuntimeResult = validate(ClientRequestV1, {
        v: 1,
        type: "zhenfa_place",
        x: 4,
        y: 64,
        z: -5,
        kind,
        qi_invest_ratio: 0,
        item_instance_id: itemInstanceId,
        target_face: targetFace,
      });
      expect(trapRuntimeResult.ok, `${kind} must remain valid in shared schema`).toBe(true);
    }

    const omittedFaceResult = validate(ClientRequestV1, {
      v: 1,
      type: "zhenfa_place",
      x: 2,
      y: 64,
      z: -3,
      kind: "slow_trap",
      qi_invest_ratio: 0,
      item_instance_id: 9002,
    });
    expect(omittedFaceResult.ok).toBe(true);

    const invalidTargetFaceResult = validate(ClientRequestV1, {
      v: 1,
      type: "zhenfa_place",
      x: 2,
      y: 64,
      z: -3,
      kind: "slow_trap",
      qi_invest_ratio: 0,
      item_instance_id: 9002,
      target_face: "diagonal",
    });
    expect(invalidTargetFaceResult.ok).toBe(false);

    const extraFieldResult = validate(ClientRequestV1, {
      v: 1,
      type: "zhenfa_place",
      x: 2,
      y: 64,
      z: -3,
      kind: "slow_trap",
      qi_invest_ratio: 0,
      item_instance_id: 9002,
      target_face: "top",
      unused: true,
    });
    expect(extraFieldResult.ok).toBe(false);

    const nullTargetFaceResult = validate(ClientRequestV1, {
      v: 1,
      type: "zhenfa_place",
      x: 2,
      y: 64,
      z: -3,
      kind: "slow_trap",
      qi_invest_ratio: 0,
      item_instance_id: 9002,
      target_face: null,
    });
    expect(nullTargetFaceResult.ok).toBe(false);
  });

  it("validates zhenfa-v2 deploy and exposure events", () => {
    expect(
      validateZhenfaV2EventV1Contract({
        v: 1,
        event: "deploy",
        array_id: 7,
        kind: "lingju",
        owner: "offline:Azure",
        zone: "spawn",
        x: 12,
        y: 64,
        z: -9,
        tick: 120,
        radius: 20,
        density_multiplier: 1.5,
        tiandao_gaze_weight: 1,
      }).ok,
    ).toBe(true);

    for (const chance of [0, 1]) {
      const result = validateZhenfaV2EventV1Contract(deceiveHeavenExposureEvent(chance));
      expect(result.ok, `reveal_chance_lifetime=${chance} should be accepted`).toBe(true);
    }

    expect(
      validateZhenfaV2EventV1Contract({
        v: 1,
        event: "deploy",
        array_id: 9,
        kind: "illusion",
        owner: "offline:Azure",
        x: 12,
        y: 64,
        z: -9,
        tick: 180,
        reveal_threshold: 50,
      }).ok,
    ).toBe(true);

    expect(
      validateZhenfaV2EventV1Contract({
        v: 1,
        event: "deploy",
        array_id: 11,
        kind: "network_array",
        owner: "offline:Azure",
        x: 12,
        y: 64,
        z: -9,
        tick: 200,
        radius: 12,
        density_multiplier: 1.0,
        tiandao_gaze_weight: 0.5,
      }).ok,
    ).toBe(true);

    expect(
      validateZhenfaV2EventV1Contract({
        v: 1,
        event: "deploy",
        array_id: 12,
        kind: "unknown_array",
        owner: "offline:Azure",
        x: 12,
        y: 64,
        z: -9,
        tick: 201,
      }).ok,
    ).toBe(false);

    expect(
      validateZhenfaV2EventV1Contract({
        v: 1,
        event: "deceive_heaven_exposed",
        array_id: 10,
        kind: "deceive_heaven",
        owner: "offline:Azure",
        x: 12,
        y: 64,
        z: -9,
        tick: 180,
        reveal_chance_per_tick: 0.002,
      }).ok,
    ).toBe(false);

    for (const chance of [-0.001, 1.001, "0.5", null, Number.NaN]) {
      const result = validateZhenfaV2EventV1Contract(deceiveHeavenExposureEvent(chance));
      expect(result.ok, `reveal_chance_lifetime=${String(chance)} should be rejected`).toBe(false);
    }
  });
});
