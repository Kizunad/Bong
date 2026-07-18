/**
 * plan-agent-ui-data-v1 P0 — 天道 UI-as-Data 四个独立 schema。
 *
 * 四个 schema 独立，字段不得混用：
 *   AgentUiRequestCommandV1  — agent → server（Redis bong:agent_ui_cmd）
 *   AgentUiRequestPayloadV1  — server → client（CustomPayload via server_data）
 *   AgentUiClientResponsePayloadV1 — client → server（CustomPayload）
 *   AgentUiResponsePayloadV1 — server → agent（Redis bong:agent_ui_response）
 */

import { Type, type Static } from "@sinclair/typebox";
import { validate, type ValidationResult } from "../validate.js";

// ─── Action 字面联合 ─────────────────────────────────────────────────────────

/**
 * 天道 UI 面板交互动作枚举。
 * client → server 与 server → agent 两条独立 payload schema 共用同一组字面量。
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

// 标准 JSON Schema 的 maxLength 按 Unicode code point 计数，但 TypeBox Value.Check
// 在 JavaScript runtime 中按 UTF-16 code unit 计数。为避免 astral 字符造成接受集合
// 分叉，长度与 well-formed Unicode 约束统一放进同一个 ECMA-262 pattern：无 flag
// 解释把 surrogate pair 作为第二分支的一次重复，Unicode-aware `u` 解释把 astral
// 字符作为第一分支的一次重复；两者都精确计数 1..=128 个 Unicode code points，
// 并拒绝 JavaScript 可表示、但 Rust serde_json 无法接收的 lone surrogate。
const AGENT_UI_ID_PATTERN =
  "^(?:(?![\\uD800-\\uDFFF])[\\s\\S]|[\\uD800-\\uDBFF][\\uDC00-\\uDFFF]){1,128}$";

function agentUiIdV1() {
  return Type.String({
    pattern: AGENT_UI_ID_PATTERN,
  });
}

// ─── Schema 1：Agent → Server（Redis bong:agent_ui_cmd）──────────────────────

/**
 * 天道 Agent 向 server 发布的 UI 指令。
 *
 * 安全字段 realm_gate / allowed_button_ids 仅存在于此 schema，
 * 绝不下发给 client（见 AgentUiRequestPayloadV1）。
 */
export const AgentUiRequestCommandV1 = Type.Object(
  {
    request_id: agentUiIdV1(),
    target_player: agentUiIdV1(),
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
    request_id: agentUiIdV1(),
    target_player: agentUiIdV1(),
    xml: Type.String({ maxLength: 8192 }),
    timeout_ticks: Type.Integer({ minimum: 20, maximum: 2400 }),
  },
  { additionalProperties: false },
);
export type AgentUiRequestPayloadV1 = Static<typeof AgentUiRequestPayloadV1>;

// ─── Schema 3：Client → Server CustomPayload + Server → Agent Redis ───────────

/**
 * error action 的 reason 字面量联合（文档化 + TypeScript 类型安全）。
 *
 * - realm_gate_rejected: 玩家境界不足，server 拒绝面板请求
 * - invalid_button_id:   玩家点击了 allowed_button_ids 之外的按钮
 * - player_offline:      目标玩家已离线，server 无法路由
 * - invalid_command:     agent→server 命令结构非法，server 拒绝处理
 * - xml_sanitize_failed: server 清洗 XML 失败，面板不会下发
 *
 * 通过 AgentUiErrorReasonV1 可在调用方做完整 switch 校验。
 * params 字段仍保持 Record<string, string>（可扩展），此联合仅用于 TS 静态检查。
 */
export type AgentUiErrorReasonV1 =
  | "realm_gate_rejected"
  | "invalid_button_id"
  | "player_offline"
  | "invalid_command"
  | "xml_sanitize_failed";

/**
 * 玩家面板交互响应（client→server CustomPayload）。
 *
 * params 为 Record<string, string> 以保留可扩展性：
 *   button_click → params.button_id = "<id>"
 *   error        → params.reason: AgentUiErrorReasonV1（见上方联合类型）
 *                  realm_gate_rejected 还有 params.player_realm / params.required_realm
 */
export const AgentUiClientResponsePayloadV1 = Type.Object(
  {
    request_id: agentUiIdV1(),
    action: AgentUiActionType,
    params: Type.Record(Type.String(), Type.String()),
  },
  { additionalProperties: false },
);
export type AgentUiClientResponsePayloadV1 = Static<
  typeof AgentUiClientResponsePayloadV1
>;

/**
 * server 转发给 agent 的面板响应（Redis bong:agent_ui_response）。
 *
 * 只有这条权威 server→agent 边界可回填 target_player；真实 Fabric C2S
 * 从不发送该字段，server 依已认证的连接实体确定玩家。
 */
export const AgentUiResponsePayloadV1 = Type.Object(
  {
    ...AgentUiClientResponsePayloadV1.properties,
    target_player: Type.Optional(agentUiIdV1()),
  },
  { additionalProperties: false },
);
export type AgentUiResponsePayloadV1 = Static<typeof AgentUiResponsePayloadV1>;

// ─── Schema 4：Server → Client close 信号（bong:server_data 变体 agent_ui_close）──

export const AgentUiCloseReasonV1 = Type.Union([
  Type.Literal("invalid_button_id"),
  Type.Literal("session_expired"),
]);
export type AgentUiCloseReasonV1 = Static<typeof AgentUiCloseReasonV1>;

/**
 * server 向 client 发送关闭信号。
 *
 * reason 为空 = Replaced（client 关闭不发任何 response）；
 * reason = "invalid_button_id" | "session_expired" 时 client 显示提示后关闭。
 */
export const AgentUiClosePayloadV1 = Type.Object(
  {
    request_id: agentUiIdV1(),
    reason: Type.Optional(AgentUiCloseReasonV1),
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
