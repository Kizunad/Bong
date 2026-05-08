import { describe, expect, it } from "vitest";

import {
  validateCraftOutcomeV1Contract,
  validateCraftSessionStateV1Contract,
  validateCraftStartReqV1Contract,
  validateRecipeListV1Contract,
  validateRecipeUnlockedV1Contract,
} from "../src/craft.js";

describe("plan-craft-v1 §3 IPC schema (TypeBox)", () => {
  describe("CraftStartReqV1", () => {
    it("validates a minimal payload", () => {
      const result = validateCraftStartReqV1Contract({
        v: 1,
        player_id: "offline:Alice",
        recipe_id: "craft.example.eclipse_needle.iron",
        ts: 1234567,
      });
      expect(result.ok).toBe(true);
    });
    it("rejects extra properties", () => {
      const result = validateCraftStartReqV1Contract({
        v: 1,
        player_id: "x",
        recipe_id: "y",
        ts: 1,
        extra: "not allowed",
      });
      expect(result.ok).toBe(false);
    });
    it("rejects v != 1", () => {
      const result = validateCraftStartReqV1Contract({
        v: 2,
        player_id: "x",
        recipe_id: "y",
        ts: 1,
      });
      expect(result.ok).toBe(false);
    });
  });

  describe("CraftSessionStateV1", () => {
    it("validates active session", () => {
      const result = validateCraftSessionStateV1Contract({
        v: 1,
        player_id: "offline:Alice",
        active: true,
        recipe_id: "craft.example.poison_decoction.fan",
        elapsed_ticks: 60,
        total_ticks: 1800,
        ts: 1,
      });
      expect(result.ok).toBe(true);
    });
    it("validates inactive session without recipe_id", () => {
      const result = validateCraftSessionStateV1Contract({
        v: 1,
        player_id: "offline:Alice",
        active: false,
        elapsed_ticks: 0,
        total_ticks: 0,
        ts: 1,
      });
      expect(result.ok).toBe(true);
    });
    it("rejects negative elapsed_ticks", () => {
      const result = validateCraftSessionStateV1Contract({
        v: 1,
        player_id: "x",
        active: true,
        recipe_id: "y",
        elapsed_ticks: -1,
        total_ticks: 100,
        ts: 1,
      });
      expect(result.ok).toBe(false);
    });
  });

  describe("CraftOutcomeV1 (union)", () => {
    it("validates Completed variant", () => {
      const result = validateCraftOutcomeV1Contract({
        kind: "completed",
        v: 1,
        player_id: "offline:Alice",
        recipe_id: "craft.example.eclipse_needle.iron",
        output_template: "eclipse_needle_iron",
        output_count: 3,
        completed_at_tick: 5000,
        ts: 1,
      });
      expect(result.ok).toBe(true);
    });
    it("validates Failed variant with player_cancelled reason", () => {
      const result = validateCraftOutcomeV1Contract({
        kind: "failed",
        v: 1,
        player_id: "offline:Alice",
        recipe_id: "craft.example.eclipse_needle.iron",
        reason: "player_cancelled",
        material_returned: 2,
        qi_refunded: 0,
        ts: 1,
      });
      expect(result.ok).toBe(true);
    });
    it("rejects unknown reason", () => {
      const result = validateCraftOutcomeV1Contract({
        kind: "failed",
        v: 1,
        player_id: "x",
        recipe_id: "y",
        reason: "surprise",
        material_returned: 0,
        qi_refunded: 0,
        ts: 1,
      });
      expect(result.ok).toBe(false);
    });
    it("rejects unknown kind discriminator", () => {
      const result = validateCraftOutcomeV1Contract({
        kind: "in_progress",
        v: 1,
        player_id: "x",
        recipe_id: "y",
        ts: 1,
      });
      expect(result.ok).toBe(false);
    });
  });

  describe("RecipeUnlockedV1", () => {
    it("validates Scroll source", () => {
      const result = validateRecipeUnlockedV1Contract({
        v: 1,
        player_id: "offline:Alice",
        recipe_id: "craft.example.eclipse_needle.iron",
        source: { kind: "scroll", item_template: "scroll_eclipse_needle_iron" },
        unlocked_at_tick: 5000,
        ts: 1,
      });
      expect(result.ok).toBe(true);
    });
    it("validates Mentor source", () => {
      const result = validateRecipeUnlockedV1Contract({
        v: 1,
        player_id: "offline:Alice",
        recipe_id: "craft.example.poison_decoction.fan",
        source: { kind: "mentor", npc_archetype: "poison_master" },
        unlocked_at_tick: 5000,
        ts: 1,
      });
      expect(result.ok).toBe(true);
    });
    it("validates Insight source with each trigger variant", () => {
      for (const trigger of ["breakthrough", "near_death", "defeat_stronger"]) {
        const result = validateRecipeUnlockedV1Contract({
          v: 1,
          player_id: "offline:Alice",
          recipe_id: "craft.example.fake_skin.light",
          source: { kind: "insight", trigger },
          unlocked_at_tick: 5000,
          ts: 1,
        });
        expect(result.ok, `trigger=${trigger}`).toBe(true);
      }
    });
    it("rejects Mentor without npc_archetype", () => {
      const result = validateRecipeUnlockedV1Contract({
        v: 1,
        player_id: "x",
        recipe_id: "y",
        source: { kind: "mentor" },
        unlocked_at_tick: 1,
        ts: 1,
      });
      expect(result.ok).toBe(false);
    });
    it("rejects Insight with unknown trigger", () => {
      const result = validateRecipeUnlockedV1Contract({
        v: 1,
        player_id: "x",
        recipe_id: "y",
        source: { kind: "insight", trigger: "miracle" },
        unlocked_at_tick: 1,
        ts: 1,
      });
      expect(result.ok).toBe(false);
    });
  });

  describe("RecipeListV1", () => {
    it("validates empty recipes array", () => {
      const result = validateRecipeListV1Contract({
        v: 1,
        player_id: "offline:Alice",
        recipes: [],
        ts: 1,
      });
      expect(result.ok).toBe(true);
    });
    it("validates payload with full recipe entry", () => {
      const result = validateRecipeListV1Contract({
        v: 1,
        player_id: "offline:Alice",
        recipes: [
          {
            id: "craft.example.eclipse_needle.iron",
            category: "anqi_carrier",
            display_name: "蚀针（凡铁档）",
            materials: [["iron_needle", 3], ["chi_xui_cao", 1]],
            qi_cost: 8,
            time_ticks: 3600,
            output: ["eclipse_needle_iron", 3],
            requirements: { qi_color_min: ["Insidious", 0.05] },
            unlocked: false,
          },
        ],
        ts: 1,
      });
      expect(result.ok).toBe(true);
    });
    it("rejects recipe with negative qi_cost", () => {
      const result = validateRecipeListV1Contract({
        v: 1,
        player_id: "x",
        recipes: [
          {
            id: "y",
            category: "tool",
            display_name: "x",
            materials: [],
            qi_cost: -1,
            time_ticks: 0,
            output: ["z", 1],
            requirements: {},
            unlocked: true,
          },
        ],
        ts: 1,
      });
      expect(result.ok).toBe(false);
    });
    it("rejects unknown category", () => {
      const result = validateRecipeListV1Contract({
        v: 1,
        player_id: "x",
        recipes: [
          {
            id: "y",
            category: "future_category",
            display_name: "x",
            materials: [],
            qi_cost: 0,
            time_ticks: 0,
            output: ["z", 1],
            requirements: {},
            unlocked: true,
          },
        ],
        ts: 1,
      });
      expect(result.ok).toBe(false);
    });
  });
});
