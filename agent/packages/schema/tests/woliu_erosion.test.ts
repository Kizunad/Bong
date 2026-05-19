import { describe, expect, it } from "vitest";

import {
  VoidErosionStageV1,
  VoidErosionStateV1,
  VoidErosionEventV1,
  VoidErosionVisualSyncPayloadV1,
  VoidErosionTiandaoModifierV1,
  validateVoidErosionStateV1Contract,
  validateVoidErosionEventV1Contract,
  validateVoidErosionVisualSyncV1Contract,
  validateVoidErosionTiandaoModifierV1Contract,
} from "../src/woliu_erosion.js";
import { Value } from "@sinclair/typebox/value";

describe("VoidErosionStateV1", () => {
  it("should validate a valid state", () => {
    const state: VoidErosionStateV1 = {
      entity: "player_123",
      stage: "echo_body",
      cumulative_erosion: 250.0,
      ambient_active: true,
      contam_mult: 1.5,
      efficiency: 0.8,
      server_tick: 99999,
    };
    const result = validateVoidErosionStateV1Contract(state);
    expect(result.ok).toBe(true);
  });

  it("should reject unknown fields", () => {
    const result = validateVoidErosionStateV1Contract({
      entity: "test",
      stage: "none",
      cumulative_erosion: 0,
      ambient_active: false,
      contam_mult: 1.0,
      efficiency: 1.0,
      server_tick: 0,
      unknown_field: 42,
    });
    expect(result.ok).toBe(false);
  });

  it("should reject invalid stage string", () => {
    const result = validateVoidErosionStateV1Contract({
      entity: "test",
      stage: "corrupted",
      cumulative_erosion: 0,
      ambient_active: false,
      contam_mult: 1.0,
      efficiency: 1.0,
      server_tick: 0,
    });
    expect(result.ok).toBe(false);
  });

  it("should validate all 5 stage variants", () => {
    const stages: VoidErosionStageV1[] = [
      "none",
      "low_pressure",
      "void_shadow",
      "echo_body",
      "void_eroded",
    ];
    for (const stage of stages) {
      const state: VoidErosionStateV1 = {
        entity: "test",
        stage,
        cumulative_erosion: 0,
        ambient_active: false,
        contam_mult: 1.0,
        efficiency: 1.0,
        server_tick: 0,
      };
      const result = validateVoidErosionStateV1Contract(state);
      expect(result.ok).toBe(true);
    }
  });
});

describe("VoidErosionEventV1", () => {
  it("should validate a valid event", () => {
    const event: VoidErosionEventV1 = {
      entity: "player_456",
      from_stage: "low_pressure",
      to_stage: "void_shadow",
      cumulative_erosion: 80.0,
      server_tick: 50000,
    };
    const result = validateVoidErosionEventV1Contract(event);
    expect(result.ok).toBe(true);
  });

  it("should validate all stage transitions", () => {
    const transitions: [VoidErosionStageV1, VoidErosionStageV1][] = [
      ["none", "low_pressure"],
      ["low_pressure", "void_shadow"],
      ["void_shadow", "echo_body"],
      ["echo_body", "void_eroded"],
    ];
    for (const [from, to] of transitions) {
      const event: VoidErosionEventV1 = {
        entity: "test",
        from_stage: from,
        to_stage: to,
        cumulative_erosion: 100,
        server_tick: 0,
      };
      const result = validateVoidErosionEventV1Contract(event);
      expect(result.ok).toBe(true);
    }
  });
});

describe("VoidErosionVisualSyncPayloadV1", () => {
  it("should validate a valid visual sync payload", () => {
    const payload: VoidErosionVisualSyncPayloadV1 = {
      entity_id: "player_789",
      stage: 3,
      cumulative_erosion: 250.0,
      ambient_active: true,
      model_alpha: 0.55,
      sound_distortion_active: true,
      server_tick: 12345,
    };
    const result = validateVoidErosionVisualSyncV1Contract(payload);
    expect(result.ok).toBe(true);
  });

  it("should reject unknown fields", () => {
    const result = validateVoidErosionVisualSyncV1Contract({
      entity_id: "test",
      stage: 0,
      cumulative_erosion: 0,
      ambient_active: false,
      model_alpha: 1.0,
      sound_distortion_active: false,
      server_tick: 0,
      extra: 42,
    });
    expect(result.ok).toBe(false);
  });

  it("should validate all 5 stages", () => {
    for (let stage = 0; stage <= 4; stage++) {
      const payload: VoidErosionVisualSyncPayloadV1 = {
        entity_id: `player_${stage}`,
        stage,
        cumulative_erosion: stage * 100,
        ambient_active: stage >= 1,
        model_alpha: 1.0 - stage * 0.15,
        sound_distortion_active: stage >= 3,
        server_tick: 0,
      };
      const result = validateVoidErosionVisualSyncV1Contract(payload);
      expect(result.ok).toBe(true);
    }
  });

  it("should reject stage out of range", () => {
    const result = validateVoidErosionVisualSyncV1Contract({
      entity_id: "test",
      stage: 5,
      cumulative_erosion: 0,
      ambient_active: false,
      model_alpha: 1.0,
      sound_distortion_active: false,
      server_tick: 0,
    });
    expect(result.ok).toBe(false);
  });

  it("should reject model_alpha out of range", () => {
    const result = validateVoidErosionVisualSyncV1Contract({
      entity_id: "test",
      stage: 0,
      cumulative_erosion: 0,
      ambient_active: false,
      model_alpha: 1.5,
      sound_distortion_active: false,
      server_tick: 0,
    });
    expect(result.ok).toBe(false);
  });
});

describe("VoidErosionTiandaoModifierV1", () => {
  it("should validate a valid tiandao modifier payload", () => {
    const payload: VoidErosionTiandaoModifierV1 = {
      entity: "player_void",
      stage: "echo_body",
      detection_modifier: 0.6,
      server_tick: 55555,
    };
    const result = validateVoidErosionTiandaoModifierV1Contract(payload);
    expect(result.ok).toBe(true);
  });

  it("should reject unknown fields", () => {
    const result = validateVoidErosionTiandaoModifierV1Contract({
      entity: "test",
      stage: "none",
      detection_modifier: 1.0,
      server_tick: 0,
      foo: "bar",
    });
    expect(result.ok).toBe(false);
  });

  it("should validate all stages with correct modifiers", () => {
    const cases: [VoidErosionStageV1, number][] = [
      ["none", 1.0],
      ["low_pressure", 1.0],
      ["void_shadow", 1.0],
      ["echo_body", 0.6],
      ["void_eroded", 0.0],
    ];
    for (const [stage, modifier] of cases) {
      const payload: VoidErosionTiandaoModifierV1 = {
        entity: "test",
        stage,
        detection_modifier: modifier,
        server_tick: 0,
      };
      const result = validateVoidErosionTiandaoModifierV1Contract(payload);
      expect(result.ok).toBe(true);
    }
  });

  it("should reject detection_modifier out of range", () => {
    const result = validateVoidErosionTiandaoModifierV1Contract({
      entity: "test",
      stage: "none",
      detection_modifier: 1.5,
      server_tick: 0,
    });
    expect(result.ok).toBe(false);
  });
});
