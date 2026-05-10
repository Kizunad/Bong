import { describe, expect, it } from "vitest";

import {
  ClientRequestV1,
  validateZhenfaV2EventV1Contract,
} from "../src/index.js";
import { validate } from "../src/validate.js";

describe("zhenfa-v2 schema", () => {
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

    expect(
      validateZhenfaV2EventV1Contract({
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
        reveal_chance_per_tick: 0.002,
        self_weight_multiplier: 0.5,
        target_weight_multiplier: 1.5,
      }).ok,
    ).toBe(true);

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
  });
});
