/**
 * plan-agent-ui-data-v1 P0 — 天道 UI-as-Data 三个独立 schema。
 *
 * 三个 schema 独立，字段不得混用：
 *   AgentUiRequestCommandV1  — agent → server（Redis bong:agent_ui_cmd）
 *   AgentUiRequestPayloadV1  — server → client（CustomPayload via server_data）
 *   AgentUiResponsePayloadV1 — client → server（CustomPayload）+ server → agent（Redis）
 */

import { Type, type Static } from "@sinclair/typebox";
import { validate, type ValidationResult } from "../validate.js";

// ─── Action 字面联合 ─────────────────────────────────────────────────────────

/**
 * 天道 UI 面板交互动作枚举。
 * client → server CustomPayload + server → agent Redis 共用同一 schema。
 */
export const AgentUiActionType = Type.Union([
  Type.Literal("button_click"),
  Type.Literal("dismissed"),
  Type.Literal("timeout"),
  Type.Literal("replaced"),
  Type.Literal("error"),
  /** client OwoUI 解析 XML 失败时回传 */
  Type.Literal("parse_error"),
]);
export type AgentUiActionType = Static<typeof AgentUiActionType>;

// ─── Schema 1：Agent → Server（Redis bong:agent_ui_cmd）──────────────────────

/**
 * 天道 Agent 向 server 发布的 UI 指令。
 *
 * 安全字段 realm_gate / allowed_button_ids 仅存在于此 schema，
 * 绝不下发给 client（见 AgentUiRequestPayloadV1）。
 */
export const AgentUiRequestCommandV1 = Type.Object(
  {
    request_id: Type.String({ minLength: 1, maxLength: 128 }),
    target_player: Type.String({ minLength: 1, maxLength: 128 }),
    xml: Type.String({ maxLength: 8192 }),
    timeout_ticks: Type.Integer({ minimum: 20, maximum: 2400, default: 600 }),
    /**
     * 境界门控（1-indexed realm rank）：
     *   0 = 不门控（任意境界可见）
     *   1 = 醒灵+  2 = 引气+  3 = 凝脉+  4 = 固元+  5 = 通灵+  6 = 化虚+（最高境界）
     */
    realm_gate: Type.Integer({ minimum: 0, maximum: 6, default: 0 }),
    /**
     * 允许的按钮 ID 白名单，最多 16 条。
     * server 校验 button_click.params.button_id ∈ allowed_button_ids。
     * 不下发给 client。
     */
    allowed_button_ids: Type.Array(Type.String(), { maxItems: 16 }),
  },
  { additionalProperties: false },
);
export type AgentUiRequestCommandV1 = Static<typeof AgentUiRequestCommandV1>;

// ─── Schema 2：Server → Client（bong:server_data 变体 agent_ui_request）───────

/**
 * server 下发给 client 的动态 UI 面板请求。
 *
 * 不含 realm_gate / allowed_button_ids（安全字段仅在 server 内部处理）。
 */
export const AgentUiRequestPayloadV1 = Type.Object(
  {
    request_id: Type.String({ minLength: 1, maxLength: 128 }),
    target_player: Type.String({ minLength: 1, maxLength: 128 }),
    xml: Type.String({ maxLength: 8192 }),
    timeout_ticks: Type.Integer({ minimum: 20, maximum: 2400 }),
  },
  { additionalProperties: false },
);
export type AgentUiRequestPayloadV1 = Static<typeof AgentUiRequestPayloadV1>;

// ─── Schema 3：Client → Server CustomPayload + Server → Agent Redis ───────────

/**
 * 玩家面板交互响应，双向使用（C2S CustomPayload + server→agent Redis）。
 *
 * params 为 Record<string, string> 以保留可扩展性：
 *   button_click → params.button_id = "<id>"
 *   error        → params.reason = "realm_gate_rejected" | "invalid_button_id" | "player_offline"
 *                  realm_gate_rejected 还有 params.player_realm / params.required_realm
 */
export const AgentUiResponsePayloadV1 = Type.Object(
  {
    request_id: Type.String({ minLength: 1, maxLength: 128 }),
    action: AgentUiActionType,
    params: Type.Record(Type.String(), Type.String()),
  },
  { additionalProperties: false },
);
export type AgentUiResponsePayloadV1 = Static<typeof AgentUiResponsePayloadV1>;

// ─── Schema 4：Server → Client close 信号（bong:server_data 变体 agent_ui_close）──

/**
 * server 向 client 发送关闭信号。
 *
 * reason 为空 = Replaced（client 关闭不发任何 response）；
 * reason = "invalid_button_id" | "session_expired" 时 client 显示提示后关闭。
 */
export const AgentUiClosePayloadV1 = Type.Object(
  {
    request_id: Type.String({ minLength: 1, maxLength: 128 }),
    reason: Type.Optional(Type.String({ maxLength: 64 })),
  },
  { additionalProperties: false },
);
export type AgentUiClosePayloadV1 = Static<typeof AgentUiClosePayloadV1>;

// ─── Validate helpers ────────────────────────────────────────────────────────

/** TypeBox 契约校验：AgentUiResponsePayloadV1（server → agent Redis channel） */
export function validateAgentUiResponsePayloadV1Contract(data: unknown): ValidationResult {
  return validate(AgentUiResponsePayloadV1, data);
}

/** TypeBox 契约校验：AgentUiRequestCommandV1（agent → server Redis channel） */
export function validateAgentUiRequestCommandV1Contract(data: unknown): ValidationResult {
  return validate(AgentUiRequestCommandV1, data);
}
