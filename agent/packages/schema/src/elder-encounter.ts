/**
 * plan-dying-elder-v1 P3 — ElderEncounterEventV1 IPC schema (TypeScript side).
 *
 * server → agent 叙事频道 `bong:elder_encounter`（`CHANNELS.ELDER_ENCOUNTER`）的 payload。
 * 镜像 Rust `schema::elder_encounter` 模块。
 *
 * agent 订阅后按 `event_kind` 生成两类 narration：
 * - 感知类（zone scope）：`appeared`, `dan_received`
 * - 死亡广播类（broadcast scope）：`betrayal`, `dead_natural`, `dead_player_kill`
 */

import { type Static, Type } from "@sinclair/typebox";

import { type ValidationResult, validate } from "./validate.js";

/**
 * 垂死大能遭遇事件种类（wire format, snake_case）。
 * 对应 Rust `ElderEncounterEventKindV1`。
 */
export const ElderEncounterEventKindV1 = Type.Union([
  /** 大能首次出现在坍缩渊（Plea 态入场）。 */
  Type.Literal("appeared"),
  /** 玩家向大能交付了一颗回元丹（Recovering 态计数更新）。 */
  Type.Literal("dan_received"),
  /** 大能翻脸夺舍（Betrayal 态，SoulSeize 已发生）。 */
  Type.Literal("betrayal"),
  /** 大能自然力竭死亡（真元耗尽 / 守信自裁）。 */
  Type.Literal("dead_natural"),
  /** 大能被玩家击杀（外部击杀路线）。 */
  Type.Literal("dead_player_kill"),
]);
export type ElderEncounterEventKindV1 = Static<typeof ElderEncounterEventKindV1>;

/**
 * 垂死大能遭遇事件 payload（server → agent，`bong:elder_encounter`）。
 * 对应 Rust `ElderEncounterEventV1`（`deny_unknown_fields`）。
 */
export const ElderEncounterEventV1 = Type.Object(
  {
    /** 大能所在 TSY zone 名称（agent zone-scoped narration 用）。 */
    zone_name: Type.String({ minLength: 1 }),
    /** 大能 ECS entity index（u32，区分多次遭遇）。 */
    elder_entity_idx: Type.Integer({ minimum: 0 }),
    /** 事件种类（感知 vs. 死亡广播）。 */
    event_kind: ElderEncounterEventKindV1,
    /**
     * 翻脸概率（0.0-1.0）。
     * - `appeared` 事件：填实际值（SpiritEye 激活时 client HUD 显示具体 %）
     * - 其他事件：0.0（agent 不使用此字段）
     */
    betray_probability: Type.Number({ minimum: 0, maximum: 1 }),
    /**
     * 累计收到丹数量。
     * - `dan_received` 事件：递增值
     * - 其他事件：0
     */
    dan_count: Type.Integer({ minimum: 0 }),
    /**
     * 承诺传授的功法 ID（地阶；`appeared` 时填实际值，其他可填空字符串）。
     */
    offered_skill_id: Type.String(),
    /**
     * 大能当前真元比例（qi_current / qi_max_cache，[0.0, 1.0]）。
     * - `appeared` / `dan_received` 时填实际值（大能真元恢复进度）
     * - 死亡类事件（betrayal / dead_natural / dead_player_kill）填 0.0
     * - server `ElderEncounterEventV1.qi_fraction: f32` 对齐
     */
    qi_fraction: Type.Number({ minimum: 0, maximum: 1 }),
    /** Server 游戏 tick（时序审计用）。 */
    server_tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type ElderEncounterEventV1 = Static<typeof ElderEncounterEventV1>;

/**
 * 校验 `ElderEncounterEventV1` payload 是否符合契约。
 */
export function validateElderEncounterEventV1Contract(
  data: unknown,
): ValidationResult {
  return validate(ElderEncounterEventV1, data);
}
