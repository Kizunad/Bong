# plan-agent-ui-invalid-button-error-terminal-v1

一句话：`agent_ui` 在 `invalid_button_id` 分支把 session 留在 `Open`，没有进入 `Error` 终态，也没有发 `agent_ui_close(reason=invalid_button_id)`；同一 `request_id` 之后还能再收到 `timeout` / `replaced`，形成冲突终态。

## 复现路径

1. server 建立一条带 `allowed_button_ids` 的 `AgentUiSession`，例如 `request_id=req-btn`（[server/src/network/agent_ui.rs](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-cr/server/src/network/agent_ui.rs:433)）。
2. client 点击一个不在白名单内的按钮 ID；客户端会立刻先发 `button_click` 再本地关屏（[client/src/main/java/com/bong/client/agentui/AgentUiScreen.java](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-cr/client/src/main/java/com/bong/client/agentui/AgentUiScreen.java:291)）。
3. server 在 `invalid_button_id` 分支只发 Redis `{action:"error", reason:"invalid_button_id"}`，随后 `continue`；既没有 `take_if_match` 清 session，也没有给 client 发 `agent_ui_close`（[server/src/network/agent_ui.rs](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-cr/server/src/network/agent_ui.rs:657)）。
4. 同一 session 仍留在 `Open`，后续要么被 ticker 再打成 `timeout`（[server/src/network/agent_ui.rs](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-cr/server/src/network/agent_ui.rs:580)），要么被下一次面板请求打成 `replaced`（[server/src/network/agent_ui.rs](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-cr/server/src/network/agent_ui.rs:442)）。
5. agent 侧 `uiResponseConsumer` 对 `invalid_button_id` 只记 warn，不把它当 session_end（[agent/packages/tiandao/src/ui/uiResponseConsumer.ts](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-cr/agent/packages/tiandao/src/ui/uiResponseConsumer.ts:242)），于是会先看到 `error`，之后又可能看到同 `request_id` 的 `timeout/replaced`。

## 根因链路

- 定稿要求是：`invalid_button_id` 必须进入 `Error` 终态，并同时向 client 发 close 信号（[docs/finished_plans/plan-agent-ui-data-v1.md](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-cr/docs/finished_plans/plan-agent-ui-data-v1.md:49), [docs/finished_plans/plan-agent-ui-data-v1.md](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-cr/docs/finished_plans/plan-agent-ui-data-v1.md:210)）。
- 现实现把 `invalid_button_id` 特判成“只报错、不消耗 session”，还被单测显式钉死为“session 保持 Open”（[server/src/network/agent_ui.rs](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-cr/server/src/network/agent_ui.rs:972)）。
- 结果是 server 的权威状态机和 agent/client 的终态语义分叉：client 体感已结束，server 却仍持有活 session，agent 还能收到第二个终态事件。

## 影响面

- 对实际游玩体验的影响：
  当 XML 按钮和白名单漂移、混版本 rollout、未来模板接线失配，或任何桥接层误发了非法 `button_id` 时，玩家一点击面板就会本地消失，但 server 仍把这条会话当活的。之后 agent 可能再收到同一 `request_id` 的 `timeout` / `replaced`，导致“同一次面板既报错又超时/被替换”的冲突叙事，后续面板互斥和日志排障也会被污染。
- 这不是“close reason 被吞掉”的重复题；这里是 `invalid_button_id` 根本没有进入应有的 terminal path，close 包也根本没发。

## 修复建议

1. `receive_agent_ui_response_system` 的 `invalid_button_id` 分支改为真正 `take_if_match`/移除 session，落 `Error` 终态，而不是 `continue` 留在 `Open`。
2. 同分支同步发送 `AgentUiClosePayloadV1 { request_id, reason: Some("invalid_button_id") }`，保持与定稿一致；即便当前点击路径已本地关屏，也要保证 stale/未来 client 的权威 close 行为一致。
3. 新增回归：`invalid_button_id` 后同 `request_id` 不得再 emit `timeout` / `replaced` / `dismissed` 第二终态。
4. 若希望 agent 侧也显式收口 panel context，可把 `invalid_button_id` 视为 session_end 或新增 dedicated terminal callback；这项可作为 follow-up，但不能替代 server 先修正 terminal state。

## 验收抓手

- server 单测：
  `system_invalid_button_id_emits_error_response` 改为断言 session 已移除，而不是 `Open`。
- server 集成回归：
  先打 `invalid_button_id`，再推进 `CurrentTickResource` 到 `expire_tick`，断言不会再发 `timeout`。
- server 集成回归：
  先打 `invalid_button_id`，再发新 `AgentUiCmdEvent`，断言旧 `request_id` 不会再收到 `replaced`。
- client/bridge 回归：
  `invalid_button_id` 分支必须能收到 `bong:agent_ui_close {reason:"invalid_button_id"}`。
- agent 回归：
  同一 `request_id` 最多出现一次 terminal 结果，不再出现 `error` 后又 `timeout/replaced` 的双终态日志。

## 两轮反方裁决（退化处理）

当前会话无 subagent 能力，未能拉起 `@oracle` / `@explore` 做外部裁决；以下为本地退化的两轮反方审查，PR 会如实记录。

### Round 1

- 反方论点：这可能是刻意设计成“非法按钮不消耗 session，允许玩家重试”，不一定是 bug。
- 驳回理由：client 正常点击路径已经先 `sendResponse("button_click")` 再 `closeWithoutResponse()`，玩家端没有“继续重试同一面板”的机会；而定稿也明确要求 `invalid_button_id` 进入 `Error` 终态并下发 close（[docs/finished_plans/plan-agent-ui-data-v1.md](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-cr/docs/finished_plans/plan-agent-ui-data-v1.md:49)）。

### Round 2

- 反方论点：这只会被恶意客户端打到，真实玩家不会遇到，题目价值不高。
- 驳回理由：本线程优先看的就是 payload 字段、panel state、bridge runtime；而 `invalid_button_id` 的自然触发源并不只剩恶意输入，按钮 XML/白名单漂移、混版本 rollout、未来模板可选按钮接线失配都能打到同一分支。更关键的是，仓内单测已把错误行为固化为“session 保持 Open”，说明这是高复发的桥接层设计偏差，不是纯理论攻击面。
