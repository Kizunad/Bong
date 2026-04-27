import { Type, type Static } from "@sinclair/typebox";

import { validate, type ValidationResult } from "./validate.js";

/** plan-tsy-zone-v1 §1.4 — 跨位面锚点 wire 形态。
 *
 *  与 Rust 端 `world::tsy::DimensionAnchor` 双端对齐：
 *  - `dimension`: identifier 字符串（"minecraft:overworld" / "bong:tsy"）
 *  - `pos`: `[x, y, z]`，顺序固定，f64 精度
 */
export const TsyDimensionAnchorV1 = Type.Object(
  {
    dimension: Type.String({ minLength: 1 }),
    pos: Type.Array(Type.Number(), { minItems: 3, maxItems: 3 }),
  },
  { additionalProperties: false },
);
export type TsyDimensionAnchorV1 = Static<typeof TsyDimensionAnchorV1>;

/** plan §4 entry filter — 单个被剥离物品的 wire 形态。
 *
 *  - `instance_id`: u64，对应 server 端 `ItemInstance.instance_id`
 *  - `template_id`: 物品模板 id（剥离后保留 — 物品类型不变，只是失灵）
 *  - `reason`: P0 仅 `spirit_quality_too_high`；后续如需多原因再扩 union
 */
export const TsyFilteredItemV1 = Type.Object(
  {
    instance_id: Type.Number({ minimum: 0 }),
    template_id: Type.String({ minLength: 1 }),
    reason: Type.Literal("spirit_quality_too_high"),
  },
  { additionalProperties: false },
);
export type TsyFilteredItemV1 = Static<typeof TsyFilteredItemV1>;

/** plan §1.4 — 玩家进入活坍缩渊（TSY）的 IPC 事件。
 *
 *  Server → Agent 单向；agent 用此事件做"踏进秘境"narration / 风险评估。
 */
export const TsyEnterEventV1 = Type.Object(
  {
    v: Type.Literal(1),
    kind: Type.Literal("tsy_enter"),
    tick: Type.Number({ minimum: 0 }),
    player_id: Type.String({ minLength: 1 }),
    family_id: Type.String({ minLength: 1 }),
    return_to: TsyDimensionAnchorV1,
    filtered_items: Type.Array(TsyFilteredItemV1),
  },
  { additionalProperties: false },
);
export type TsyEnterEventV1 = Static<typeof TsyEnterEventV1>;

export function validateTsyEnterEventV1Contract(data: unknown): ValidationResult {
  return validate(TsyEnterEventV1, data);
}

/** plan §1.4 — 玩家从 TSY 出关回主世界。
 *
 *  - `duration_ticks`: 入场到出场的 server tick 差
 *  - `qi_drained_total`: 本次秘境内被抽走的真元累计（点；可能 > spirit_qi_max
 *    若中途回了真元）。P0 server 端尚未维护 running 累计，此字段先约定 wire shape；
 *    runtime emit 时填 0，loot plan 阶段再正确累计。
 */
export const TsyExitEventV1 = Type.Object(
  {
    v: Type.Literal(1),
    kind: Type.Literal("tsy_exit"),
    tick: Type.Number({ minimum: 0 }),
    player_id: Type.String({ minLength: 1 }),
    family_id: Type.String({ minLength: 1 }),
    duration_ticks: Type.Number({ minimum: 0 }),
    qi_drained_total: Type.Number({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type TsyExitEventV1 = Static<typeof TsyExitEventV1>;

export function validateTsyExitEventV1Contract(data: unknown): ValidationResult {
  return validate(TsyExitEventV1, data);
}

/** plan-tsy-loot-v1 §4.4 — 玩家在 TSY 内死亡时 spawn 的干尸事件。
 *
 *  Server → Agent 单向；agent 用此事件做"留下了一具干尸"narration / 后续 P2
 *  lifecycle 道伥激活推演。
 *
 *  - `corpse_entity_id`：server 端 ECS Entity 序列化值（与 `canonical_npc_id` 同
 *    格式 `"npc_{idx}v{gen}"`），用于 client / agent 引用同一具干尸。
 *  - `original_player_id`：canonical 玩家 id（如 `"offline:Foo"`）。
 *  - `family_id`：所在 TSY family（如 `"tsy_lingxu_01"`），与 zone 名同源。
 *  - `pos`：死亡点世界坐标 `[x, y, z]`，f64 精度。
 */
export const TsyCorpseSpawnEventV1 = Type.Object(
  {
    v: Type.Literal(1),
    kind: Type.Literal("tsy_corpse_spawn"),
    tick: Type.Number({ minimum: 0 }),
    corpse_entity_id: Type.String({ minLength: 1 }),
    original_player_id: Type.String({ minLength: 1 }),
    original_display_name: Type.String({ minLength: 1 }),
    family_id: Type.String({ minLength: 1 }),
    death_cause: Type.String({ minLength: 1 }),
    pos: Type.Array(Type.Number(), { minItems: 3, maxItems: 3 }),
  },
  { additionalProperties: false },
);
export type TsyCorpseSpawnEventV1 = Static<typeof TsyCorpseSpawnEventV1>;

export function validateTsyCorpseSpawnEventV1Contract(data: unknown): ValidationResult {
  return validate(TsyCorpseSpawnEventV1, data);
}
