import { readFileSync } from "node:fs";
import { validatePlantCatalogV1, type PlantCatalogV1, type PlantDefinitionV1 } from "./plant-catalog.js";

/** 静态植物定义查询；数量、灵气、成长状态仍以服务端快照为准。 */
export class PlantCatalog {
  private readonly byId = new Map<string, PlantDefinitionV1>();
  private readonly aliases = new Map<string, string>();

  constructor(data: unknown) {
    const result = validatePlantCatalogV1(data);
    if (!result.ok) throw new Error(`Invalid plant catalog: ${result.errors.join("; ")}`);
    const catalog = structuredClone(data) as PlantCatalogV1;
    for (const plant of catalog.plants) {
      this.byId.set(plant.id, plant);
      for (const alias of plant.aliases) this.aliases.set(alias, plant.id);
    }
  }

  get(id: string): PlantDefinitionV1 | undefined {
    const canonical = this.aliases.get(id) ?? id;
    const plant = this.byId.get(canonical);
    return plant ? structuredClone(plant) : undefined;
  }
}

let builtin: PlantCatalog | undefined;

export function loadPlantCatalog(): PlantCatalog {
  builtin ??= new PlantCatalog(JSON.parse(readFileSync(new URL("./plants.json", import.meta.url), "utf8")));
  return builtin;
}
