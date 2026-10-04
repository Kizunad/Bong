import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { validatePlantCatalogV1, type PlantCatalogV1 } from "../src/plant-catalog.js";
import { PlantCatalog } from "../src/plant-catalog-node.js";

const source = JSON.parse(readFileSync(new URL("../../../../shared/botany/plants.json", import.meta.url), "utf8")) as PlantCatalogV1;

describe("shared plant catalog", () => {
  it("loads the shared document and accepts new IDs without a code registry", () => {
    expect(validatePlantCatalogV1(source)).toEqual({ ok: true, errors: [] });
    const entry = { ...structuredClone(source.plants[0]), id: "new_test_herb", aliases: ["test_alias"] };
    const catalog = new PlantCatalog({ schema_version: 1, plants: [entry] });
    expect(catalog.get("test_alias")?.id).toBe("new_test_herb");
    expect(catalog.get("unknown")).toBeUndefined();
  });

  it.each([
    (doc: PlantCatalogV1) => { doc.plants[1].id = doc.plants[0].id; },
    (doc: PlantCatalogV1) => { doc.plants[1].aliases = [doc.plants[0].id]; },
    (doc: PlantCatalogV1) => { doc.plants[0].visual.stages.mature.scale = 0; },
    (doc: PlantCatalogV1) => { doc.plants[0].visual.stages.mature.texture = "bong:../escape.png"; },
    (doc: PlantCatalogV1) => { doc.plants[0].name = " "; },
  ])("rejects invalid data before building lookup indexes", mutate => {
    const doc = structuredClone(source);
    mutate(doc);
    expect(validatePlantCatalogV1(doc).ok).toBe(false);
    expect(() => new PlantCatalog(doc)).toThrow("Invalid plant catalog");
  });
});
