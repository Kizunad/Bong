import { Type, type Static, type TSchema } from "@sinclair/typebox";
import { validate, type ValidationResult } from "./validate.js";

// 植物条目来自 shared/botany/plants.json。这里只定义结构与已实现的能力，
// 不枚举植物 ID；新增采用现有能力的物种无需修改三端代码。
const object = <T extends Record<string, TSchema>>(fields: T) =>
  Type.Object(fields, { additionalProperties: false });
const choices = <T extends string>(values: T[]) => Type.Union(values.map(value => Type.Literal(value)));
const id = Type.String({ pattern: "^[a-z0-9][a-z0-9_.-]*$" });
const resource = Type.String({ pattern: "^[a-z0-9_.-]+:[a-z0-9_./-]+$" });
const unit = Type.Number({ minimum: 0, maximum: 1 });
const positive = Type.Number({ exclusiveMinimum: 0 });
const nonnegative = Type.Number({ minimum: 0 });
const ticks = Type.Integer({ minimum: 0, maximum: Number.MAX_SAFE_INTEGER });
const byte = Type.Integer({ minimum: 0, maximum: 255 });
const rgb = Type.Integer({ minimum: 0, maximum: 0xffffff });
const nullable = <T extends TSchema>(schema: T) => Type.Union([schema, Type.Null()]);

const transform = {
  texture: resource,
  scale: positive,
  offset: Type.Tuple([Type.Number(), Type.Number(), Type.Number()]),
};
export const PlantStageModelV1 = Type.Union([
  object({ kind: Type.Literal("billboard"), ...transform }),
  object({
    kind: Type.Literal("geo"),
    ...transform,
    geometry: resource,
    animation: Type.Optional(object({ resource, idle: Type.String({ minLength: 1 }) })),
  }),
]);

const environmentLock = Type.Union([
  object({ neg_pressure: object({ min: unit }) }),
  object({ qi_vein_flow: object({ min: unit }) }),
  object({ fracture_mask: object({ min: unit }) }),
  object({ ruin_density: object({ min: unit }) }),
  object({ sky_island_mask: object({ min: unit, surface: choices(["top", "bottom"]) }) }),
  object({ underground_tier: object({ tier: byte }) }),
  Type.Literal("portal_rift_active"),
  object({ adjacent_decoration: object({
    kind: Type.Union([id, Type.Array(id, { minItems: 1, uniqueItems: true })]),
    radius: byte,
  }) }),
  object({ adjacent_light_block: object({ radius: byte }) }),
  Type.Literal("snow_surface"),
  object({ time_phase: Type.Literal("open") }),
]);
const harvestHazard = Type.Union([
  object({ qi_drain_on_approach: object({ radius_blocks: byte, drain_per_sec: nonnegative }) }),
  object({ wound_on_bare_hand: object({
    wound: choices(["abrasion", "laceration", "fracture"]),
    required_tool: nullable(choices(["CaiYaoDao", "BaoChu", "CaoLian", "DunQiJia", "GuaDao", "GuHaiQian", "BingJiaShouTao"])),
  }) }),
  object({ dispersal_on_fail: object({ dispersal_chance: unit }) }),
  object({ resonance_vision: object({ duration_secs: byte, composure_loss: unit }) }),
  object({ season_required: object({ phase: Type.Literal("open") }) }),
  object({ attracts_mobs: object({
    mob_kind: choices(["spirit_mice", "mimic_spider"]), min_count: byte, max_count: byte,
  }) }),
]);

// wild 是迁出旧 Rust 表的现有野生生态规则，不是新灵田的种植规则。
const wild = object({
  item_id: id,
  zone_tags: Type.Array(choices(["plains", "mountain", "marsh", "blood_valley", "cave", "wastes", "negative_field"]), { uniqueItems: true }),
  density_factor: nonnegative,
  growth_cost: nonnegative,
  survive_threshold: Type.Number({ minimum: -1, maximum: 1 }),
  max_age_ticks: Type.Integer({ minimum: 1, maximum: Number.MAX_SAFE_INTEGER }),
  regen_ticks: ticks,
  spawn_mode: choices(["zone_refresh", "static_point", "event_triggered", "spread_by_crawl"]),
  restore_ratio: unit,
  v2: nullable(object({
    survival_mode: choices(["qi_absorb", "neg_pressure_feed", "pressure_differential", "spirit_crystallize", "ruin_resonance", "thermal_convection", "portal_siphon", "dual_metabolism", "photo_luminance", "water_pulse"]),
    env_locks: Type.Array(environmentLock),
    harvest_hazards: Type.Array(harvestHazard),
    base_mesh_ref: id,
    tint_rgb: rgb,
    tint_rgb_secondary: nullable(rgb),
    model_overlay: choices(["none", "emissive", "dual_phase"]),
    icon_prompt: Type.String(),
  })),
});

export const PlantDefinitionV1 = object({
  id,
  name: Type.String({ minLength: 1 }),
  description: Type.String(),
  aliases: Type.Array(id, { uniqueItems: true }),
  visual: object({
    tint_rgb: rgb,
    stages: object({
      mature: PlantStageModelV1,
      seedling: Type.Optional(PlantStageModelV1),
      growing: Type.Optional(PlantStageModelV1),
      wilted: Type.Optional(PlantStageModelV1),
    }),
  }),
  wild: Type.Optional(wild),
});
export type PlantDefinitionV1 = Static<typeof PlantDefinitionV1>;

export const PlantCatalogV1 = object({
  schema_version: Type.Literal(1),
  plants: Type.Array(PlantDefinitionV1, { minItems: 1 }),
});
export type PlantCatalogV1 = Static<typeof PlantCatalogV1>;

export function validatePlantCatalogV1(data: unknown): ValidationResult {
  const result = validate(PlantCatalogV1, data);
  if (!result.ok) return result;
  const catalog = data as PlantCatalogV1;
  const identifiers = new Map<string, string>();
  const errors: string[] = [];
  for (const plant of catalog.plants) {
    if (!plant.name.trim()) errors.push(`${plant.id}: name must not be blank`);
    for (const key of [plant.id, ...plant.aliases]) {
      const previous = identifiers.get(key);
      if (previous !== undefined) errors.push(`${plant.id}: identifier '${key}' already belongs to ${previous}`);
      identifiers.set(key, plant.id);
    }
    for (const [stage, model] of Object.entries(plant.visual.stages)) {
      if (!model.texture.endsWith(".png")) errors.push(`${plant.id}.${stage}: texture must be a PNG`);
      const resources = [model.texture];
      if (model.kind === "geo") {
        if (!model.geometry.endsWith(".geo.json")) errors.push(`${plant.id}.${stage}: geometry must end in .geo.json`);
        resources.push(model.geometry);
        if (model.animation) {
          resources.push(model.animation.resource);
          if (!model.animation.resource.endsWith(".animation.json") || !model.animation.idle.trim()) {
            errors.push(`${plant.id}.${stage}: invalid animation resource or idle name`);
          }
        }
      }
      for (const path of resources) {
        if (path.split(/[:/]/).some(part => !/^[a-z0-9][a-z0-9_.-]*$/.test(part))) {
          errors.push(`${plant.id}.visual.stages.${stage}: invalid resource path '${path}'`);
        }
      }
    }
    for (const hazard of plant.wild?.v2?.harvest_hazards ?? []) {
      if ("attracts_mobs" in hazard && (hazard.attracts_mobs.min_count < 1 || hazard.attracts_mobs.max_count < hazard.attracts_mobs.min_count)) {
        errors.push(`${plant.id}.wild.v2.harvest_hazards: invalid mob count range`);
      }
    }
  }
  return { ok: errors.length === 0, errors };
}
