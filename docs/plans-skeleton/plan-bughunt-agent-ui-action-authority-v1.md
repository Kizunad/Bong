# BugHunt：Agent UI C2S 动作与 server 权威终态共用联合

> 来源 Issue：#1657、#1647。`target_player` 私人路由已经由既有 plan 覆盖；本骨架只处理 client 可提交 `timeout/replaced/error` 等 server 权威动作的协议与入口隔离。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 分离 C2S client-owned action 与 S2A server-owned action | ⬜ |
| P1 | Rust ingress、TypeBox/generated schema、Fabric producer 与恶意动作回归 | ⬜ |

## §0 摘要

`AgentUiClientResponsePayloadV1` 与 server→agent `AgentUiResponsePayloadV1` 仍共用 `AgentUiActionType`，联合包含 `timeout/replaced/error`。`receive_agent_ui_response_system` 只对 `button_click` 做白名单校验，其余动作使用 client 原值转 Redis；持有 request id 的恶意 client 可以伪造 server 终态，触发 agent 的错误/会话处理。已有 finished plan 只补 `target_player` 和 realm gate 私人路由，没有收口动作来源权威。

## §1 游玩影响

恶意或错误 client payload 可提前结束 agent UI session、伪造 server error，造成玩家面板状态与 agent session 分叉；旧的 realm gate 广播问题已有 fail-closed 目标校验，本骨架不重复宣称它未修。

## §2 复现路径

1. 打开一个合法 UI session，保留其 `request_id`。
2. 通过 client CustomPayload 发送 `action=timeout`、`replaced` 或 `error`，或把 `button_click` 以外的 params 填成服务端终态。
3. `server/src/network/agent_ui.rs:661-706` 消费 session 并把动作写到 `bong:agent_ui_response`；agent `uiResponseConsumer.ts:183-247` 按动作分派，伪造动作进入真实处理路径。

## §3 今天 `origin/main` 的根因证据

- `agent/packages/schema/src/payloads/agent-ui.ts:17-30,105-129` 的 action union 同时服务 C2S 与 S2A；虽已有独立 payload 对象和 `target_player`，动作字面量仍未按来源拆分。
- `server/src/schema/agent_ui.rs:17-30,118-129` Rust mirror 仍复用 `AgentUiActionType`；`server/src/network/agent_ui.rs:661-706` 只在 `ButtonClick` 分支检查 allowed ids，随后把 `ev.action` 原样放入 Redis response。
- `agent/packages/tiandao/src/ui/uiResponseConsumer.ts:183-247` 对 `error/timeout/replaced` 有生产分支；`ClientRequestProtocol.java`/`AgentUiScreen.java` 是 client 的真实 C2S producer，不能把 server 权威终态当作可发送动作。

## §4 非重复比对与立项检查记录

- **worldview**：查 `docs/worldview.md` 的“天道面板”“权限”“玩家私密反馈”关键词；本骨架是协议来源隔离，不改变境界门规则。
- **finished_plans**：查 `plan-agent-ui-data-v1.md`、`plan-bughunt-agent-ui-realm-gate-broadcast-leak-v1.md` 及 UI payload 归档；确认它们处理字段/目标路由，未处理 C2S action authority。
- **active plan**：查 `docs/plan-*.md` 的 `AgentUiActionType`、`receive_agent_ui_response_system`、`uiResponseConsumer`；没有同一动作联合拆分修法。
- **skeleton**：查 `docs/plans-skeleton/` 的 agent-ui、click-context、target-player 关键词；现有 click-context 只处理按钮上下文，本骨架独立覆盖终态伪造。
- **reminder.md**：已查 `docs/plans-skeleton/reminder.md`；无 Agent UI action authority 条目。

## §5 修复骨架

- P0：为 client→server 定义仅含 client-owned action 的 schema/enum；server→agent 保留 timeout/replaced/error 等权威动作。Rust ingress 必须按 C2S schema 拒绝越权动作，再由 session 状态决定合法的 dismissed/button_click/parse_error。
- P1：同步 TypeBox、generated JSON、Rust mirror、Fabric protocol producer；覆盖每个合法动作、未知/权威动作伪造、stale request 与重复终态。`target_player` 缺失仍沿既有 fail-closed，不退回 broadcast。
- 验收：client 无法让 server 生成一个伪造的 server-owned response；合法按钮和关闭动作仍正常到达 agent；跨玩家 UI session 不互相影响。

## §6 接入面与跨仓契约

- **Inputs**：Fabric CustomPayload `agent_ui_response`、`AgentUiClientResponsePayloadV1`、session `request_id/allowed_button_ids`。
- **Outputs**：server→agent `bong:agent_ui_response`、`AgentUiResponsePayloadV1`，以及 agent `AGENT_NARRATE` 私人错误提示/会话队列。
- **共享类型或事件**：拆分 `AgentUiClientActionType` 与 `AgentUiServerActionType`，保留 `AgentUiRequestCommandV1`、`AgentUiResponsePayloadV1`；generated samples 与 Rust serde 必须同步。
- **server 符号**：`AgentUiActionType`、`AgentUiResponseEvent`、`receive_agent_ui_response_system`、`process_agent_ui_cmd`、`CH_AGENT_UI_RESPONSE`；server 是 client action 的最终授权者。
- **agent 符号**：`AgentUiClientResponsePayloadV1`、`AgentUiResponsePayloadV1`、`AgentUiRuntime`、`UiResponseConsumer.dispatch/handleError`。
- **client**：`ClientRequestProtocol.java`、`AgentUiScreen.java` 只发送允许的 client-owned union；需改动以跟新 schema 对齐，UI 文本与渲染无需改。
- **worldview / qi_physics**：对应玩家私密反馈与境界门语义；不触及真元 ledger 或 `SPIRIT_QI_TOTAL`。
