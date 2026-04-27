import { describe, expect, it } from "vitest";

import {
  CancelSearchRequestV1,
  ContainerStateV1,
  SearchAbortedV1,
  SearchCompletedV1,
  SearchProgressV1,
  SearchStartedV1,
  StartSearchRequestV1,
  validateCancelSearchRequestV1Contract,
  validateContainerStateV1Contract,
  validateSearchAbortedV1Contract,
  validateSearchCompletedV1Contract,
  validateSearchProgressV1Contract,
  validateSearchStartedV1Contract,
  validateStartSearchRequestV1Contract,
} from "../src/container-interaction.js";

describe("plan-tsy-container-v1 §5.1 — ContainerStateV1", () => {
  const valid = {
    v: 1 as const,
    entity_id: 42,
    kind: "stone_casket" as const,
    family_id: "tsy_lingxu_01",
    world_pos: [10.5, 64.0, -3.5],
    locked: "stone_casket_key" as const,
    depleted: false,
    searched_by_player_id: "kiz",
  };

  it("accepts a valid container state", () => {
    expect(validateContainerStateV1Contract(valid).ok).toBe(true);
  });

  it("accepts depleted container with no searcher and no lock", () => {
    const depleted = {
      v: 1 as const,
      entity_id: 0,
      kind: "dry_corpse" as const,
      family_id: "tsy_lingxu_01",
      world_pos: [0, 0, 0],
      depleted: true,
    };
    expect(validateContainerStateV1Contract(depleted).ok).toBe(true);
  });

  it("rejects unknown kind", () => {
    const bad = { ...valid, kind: "treasure_chest" };
    expect(validateContainerStateV1Contract(bad).ok).toBe(false);
  });

  it("rejects extra fields (additionalProperties)", () => {
    const bad = { ...valid, extra: "nope" };
    expect(validateContainerStateV1Contract(bad).ok).toBe(false);
  });

  it("rejects missing v / family_id", () => {
    expect(
      validateContainerStateV1Contract({ ...valid, v: undefined }).ok,
    ).toBe(false);
    expect(
      validateContainerStateV1Contract({ ...valid, family_id: "" }).ok,
    ).toBe(false);
  });
});

describe("SearchStartedV1 / SearchProgressV1 / SearchCompletedV1 / SearchAbortedV1", () => {
  it("SearchStartedV1 accepts valid", () => {
    const ok = {
      v: 1 as const,
      player_id: "kiz",
      container_entity_id: 5,
      required_ticks: 80,
      at_tick: 100,
    };
    expect(validateSearchStartedV1Contract(ok).ok).toBe(true);
  });

  it("SearchStartedV1 rejects required_ticks 0", () => {
    const bad = {
      v: 1 as const,
      player_id: "kiz",
      container_entity_id: 5,
      required_ticks: 0,
      at_tick: 100,
    };
    expect(validateSearchStartedV1Contract(bad).ok).toBe(false);
  });

  it("SearchProgressV1 accepts elapsed=0", () => {
    expect(
      validateSearchProgressV1Contract({
        v: 1,
        player_id: "kiz",
        container_entity_id: 5,
        elapsed_ticks: 0,
        required_ticks: 80,
      }).ok,
    ).toBe(true);
  });

  it("SearchCompletedV1 accepts empty loot_preview", () => {
    expect(
      validateSearchCompletedV1Contract({
        v: 1,
        player_id: "kiz",
        container_entity_id: 5,
        family_id: "tsy_lingxu_01",
        loot_preview: [],
        at_tick: 200,
      }).ok,
    ).toBe(true);
  });

  it("SearchCompletedV1 rejects loot_preview entry with stack_count 0", () => {
    expect(
      validateSearchCompletedV1Contract({
        v: 1,
        player_id: "kiz",
        container_entity_id: 5,
        family_id: "tsy_lingxu_01",
        loot_preview: [
          {
            template_id: "iron_sword",
            display_name: "凡铁剑",
            stack_count: 0,
          },
        ],
        at_tick: 200,
      }).ok,
    ).toBe(false);
  });

  it("SearchAbortedV1 accepts each abort reason", () => {
    for (const reason of ["moved", "combat", "damaged", "cancelled"] as const) {
      expect(
        validateSearchAbortedV1Contract({
          v: 1,
          player_id: "kiz",
          container_entity_id: 5,
          reason,
          at_tick: 150,
        }).ok,
      ).toBe(true);
    }
  });

  it("SearchAbortedV1 rejects unknown reason", () => {
    expect(
      validateSearchAbortedV1Contract({
        v: 1,
        player_id: "kiz",
        container_entity_id: 5,
        reason: "exploded",
        at_tick: 150,
      }).ok,
    ).toBe(false);
  });
});

describe("StartSearchRequestV1 / CancelSearchRequestV1", () => {
  it("StartSearchRequestV1 minimal valid", () => {
    expect(
      validateStartSearchRequestV1Contract({ v: 1, container_entity_id: 5 })
        .ok,
    ).toBe(true);
  });

  it("CancelSearchRequestV1 minimal valid", () => {
    expect(validateCancelSearchRequestV1Contract({ v: 1 }).ok).toBe(true);
  });

  it("StartSearchRequestV1 rejects negative entity id", () => {
    expect(
      validateStartSearchRequestV1Contract({
        v: 1,
        container_entity_id: -1,
      }).ok,
    ).toBe(false);
  });
});

// 触摸引用，避免 lint 报"未使用 import"。
[
  ContainerStateV1,
  SearchStartedV1,
  SearchProgressV1,
  SearchCompletedV1,
  SearchAbortedV1,
  StartSearchRequestV1,
  CancelSearchRequestV1,
];
