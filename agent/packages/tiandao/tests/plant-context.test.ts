import { expect, it } from "vitest";
import { loadPlantCatalog } from "@bong/schema/plants";
import { plantDefinitionsBlock } from "../src/context.js";
import { WorldModel } from "../src/world-model.js";
import { createTestWorldState } from "./support/fakes.js";

it("enriches observed plants from the packaged shared catalog without inventing unknown definitions", () => {
  const worldModel = new WorldModel();
  worldModel.ingestBotanyEcology({
    v: 1, tick: 1,
    zones: [{ zone: "test", spirit_qi: 0.8, variant_counts: [], plant_counts: [
      { kind: "ning_mai_cao", count: 2 },
      { kind: "unknown_test_herb", count: 1 },
      { kind: "chi_sui_cao", count: 0 },
    ] }],
  });
  const text = plantDefinitionsBlock.render({ state: createTestWorldState(), chatSignals: [], worldModel });
  expect(text).toContain(loadPlantCatalog().get("ning_mai_cao")!.description);
  expect(text).toContain("凝脉草");
  expect(text).not.toContain("unknown_test_herb");
  expect(text).not.toContain("chi_sui_cao");
});
