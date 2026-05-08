// plan-lingtian-weather-v1 §4.2 — WeatherEvent 双端 schema。
//
// SeasonV1 / SeasonStateV1 已在 world-state.ts（jiezeq-v1 P0 落地），本文件
// 只新增天气事件相关的 5 类 + payload 形态。

import { Type, type Static } from "@sinclair/typebox";

import { validate, type ValidationResult } from "./validate.js";

/** plan §3 五类天气事件 wire 名（snake_case）。 */
export const WeatherEventKindV1 = Type.Union([
  Type.Literal("thunderstorm"),
  Type.Literal("drought_wind"),
  Type.Literal("blizzard"),
  Type.Literal("heavy_haze"),
  Type.Literal("ling_mist"),
]);
export type WeatherEventKindV1 = Static<typeof WeatherEventKindV1>;

/**
 * plan §4.2 — server → client / agent 推送的天气事件 payload。
 *
 * - `zone_id`：受影响的 zone（MVP 单 zone 时一般是 "default"）
 * - `kind`：事件类型
 * - `started_at_lingtian_tick`：开始 tick（用于 client 内插 / 进度估算）
 * - `expires_at_lingtian_tick`：自然过期 tick（client 可据此倒计时）
 * - `remaining_ticks`：当前距过期的剩余 lingtian-tick（≥0）
 *
 * 时间换算：1 game-day = 1440 lingtian-tick = 24 game-hour，1 game-hour = 60
 * lingtian-tick（与 plan-lingtian-v1 §5.1 一致）。
 */
export const WeatherEventDataV1 = Type.Object(
  {
    v: Type.Literal(1),
    zone_id: Type.String({ minLength: 1 }),
    kind: WeatherEventKindV1,
    started_at_lingtian_tick: Type.Integer({ minimum: 0 }),
    expires_at_lingtian_tick: Type.Integer({ minimum: 0 }),
    remaining_ticks: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type WeatherEventDataV1 = Static<typeof WeatherEventDataV1>;

/**
 * plan §4.4 — Redis pub channel `bong:weather_event_update` 的事件 envelope。
 *
 * 两种事件 kind：
 * - `started`：新事件触发（server 写完 ActiveWeather 后立即发）
 * - `expired`：事件自然过期（weather_apply_to_plot_system 清理时发）
 *
 * 历史上曾设计 `cleared` 用于 dev cmd / event cancel 路径，但本 plan 范围内
 * 无 producer，已删除以避免 wire 漂移。需要时由后续 plan 重新加入。
 */
export const WeatherEventUpdateV1 = Type.Object(
  {
    v: Type.Literal(1),
    kind: Type.Union([Type.Literal("started"), Type.Literal("expired")]),
    data: WeatherEventDataV1,
  },
  { additionalProperties: false },
);
export type WeatherEventUpdateV1 = Static<typeof WeatherEventUpdateV1>;

export function validateWeatherEventDataV1Contract(
  data: unknown,
): ValidationResult {
  return validate(WeatherEventDataV1, data);
}

export function validateWeatherEventUpdateV1Contract(
  data: unknown,
): ValidationResult {
  return validate(WeatherEventUpdateV1, data);
}
