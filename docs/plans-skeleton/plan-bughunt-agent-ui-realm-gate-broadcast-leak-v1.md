# plan-bughunt-agent-ui-realm-gate-broadcast-leak-v1（骨架）

> **骨架（草案）**。一句话主题：`agent-ui` 的 `realm_gate_rejected` 降级提示从 schema 到 consumer 都丢了目标玩家标识，`UiResponseConsumer` 只能把本应 `scope:"player"` 的“境界未至”文案硬编码成 `scope:"broadcast"`；一旦 server 权威拒绝某个玩家的 gated 面板，这条私人失败提示就会被发到全服聊天流。

> 立项动机：这不是抽象“文档不一致”。`agent/packages/tiandao/src/ui/uiResponseConsumer.ts` 注释明确写着理想形态是 `scope="player"`，但实现因为拿不到 `player_uuid` 直接退化成 `broadcast`；`server/src/network/mod.rs` 对 `broadcast` 又是无条件全服路由。`docs/finished_plans/plan-agent-ui-data-v1.md` 已把它记成遗留 follow-up，但当前仓库里仍无独立 skeleton 收口复现、影响面、修复面与验收抓手。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | `realm_gate_rejected` 私人提示误广播全服 | fix_pr | ⬜ |

## P0 — `realm_gate_rejected` 私人提示误广播全服

- **现象**：`agent/packages/schema/src/payloads/agent-ui.ts:100-115` 把 `AgentUiResponsePayloadV1` 固定成 `{ request_id, action, params: Record<string,string> }`，没有任何顶层 `target_player`/`player_uuid` 字段；`realm_gate_rejected` 也只约定 `params.player_realm` / `params.required_realm`（`:103-106`）。`server/src/network/agent_ui.rs:393-408` 在 realm gate 拒绝时实际发出的 error payload 也只有 `reason/player_realm/required_realm` 三项，没有把触发玩家 id 带回 agent。
- **根因链路**：`agent/packages/tiandao/src/ui/uiResponseConsumer.ts:17-19` 的注释写明期望是 `scope="player"`、`target=player_uuid`，但实现位于 `:274-280`，因为 `AgentUiResponsePayloadV1` 里没有玩家标识，只能构造 `{ scope: "broadcast", target: "world", style: "system_warning" }`。随后 `server/src/network/mod.rs:3121-3137` 的 `narration_selector` 对 `NarrationScope::Broadcast` 直接选 `RecipientSelector::Broadcast`，于是该提示稳定路由到全服，而不是只给当事玩家。
- **复现路径**：
  1. 准备两名在线玩家 A / B。
  2. 触发任意一条带 `realm_gate>0` 的 agent-ui 请求，让 server 权威侧对 A 返回 `realm_gate_rejected`。当前可用三类入口：一是 agent world-state 境界快照滞后，误把低境界 A 当成可见清晰版面板；二是后续会复用这条降级路径的 `dying-elder` / `tiandao revelation` 生产触发源；三是直接向 `bong:agent_ui_cmd` 发布一个 `realm_gate` 高于 A 实际境界的 command 做最小复现。
  3. server 按 `server/src/network/agent_ui.rs:393-408` 发布不带玩家 id 的 `AgentUiResponsePayloadV1{action:"error",reason:"realm_gate_rejected"}`。
  4. agent 按 `agent/packages/tiandao/src/ui/uiResponseConsumer.ts:274-280` 生成 `scope:"broadcast"` narration 并发到 `bong:agent_narrate`。
  5. server 按 `server/src/network/mod.rs:3125` 走 `RecipientSelector::Broadcast`，A 与 B 都会看到“天道的注意力掠过，境界未至...”这条原本只该属于 A 的提示。
- **为什么这是 bug，不是设计**：`agent/packages/tiandao/src/ui/uiResponseConsumer.ts:17-19` 已把设计口径写成“优先 `scope=player`，拿不到玩家 id 才退化”；`docs/finished_plans/plan-agent-ui-data-v1.md:361` 也明确承认当前 broadcast scope “偏离 §0.1 指定的 `scope=player`”。也就是说，现状不是产品选择，而是已知未收口的协议缺口。
- **测试侧证据**：`agent/packages/tiandao/src/ui/agent-ui.test.ts:714-730` 与 `:985-1009` 只断言“会往 `AGENT_NARRATE` 发一条 narration，且文案/style 正确”，没有任何 `scope/target` pin；这意味着当前错误路由不仅存在，而且已被测试空档默许。

## 这个 bug 对实际游玩体验的影响

- 当某个玩家因为境界不够、快照滞后或 follow-up 面板门槛更高而被 `realm_gate_rejected` 时，其他在线玩家会无端看到一条与自己无关的“天道未许”提示，聊天流被污染，且会误以为全服刚发生了某种公共天象。
- 对当事玩家，这条提示原本应是“只有你感知到的一次失败反馈”；现在却变成公开广播，等于把个人面板失败事实泄漏给旁观者，尤其不适合 `dying-elder` / `tiandao revelation` 这类本来就偏私人、偏沉浸的面板。
- 对后续实现，这条链已经被 `plan-agent-ui-data-v1` 留作 follow-up；如果不先补协议与路由，后面新增任何 gated panel producer 都会复用同一个错误广播口径，把私人提示继续做成世界公告。

## 建议修复范围 / 模块

- `agent/packages/schema/src/payloads/agent-ui.ts`：给 `realm_gate_rejected` 路径补可路由的玩家标识。优先方案是为 `AgentUiResponsePayloadV1` 增加顶层 `target_player`；次优是约定 `params.target_player`，但会继续把“协议关键字段”埋进弱类型 map。
- `server/src/network/agent_ui.rs`：在 `realm_gate_rejected` error response 中回填 canonical player id，而不只发 `player_realm/required_realm`。
- `agent/packages/tiandao/src/ui/uiResponseConsumer.ts`：优先构造 `scope:"player"`、`target=<canonical_player_id>`；仅对旧 payload 或坏 payload 保留 broadcast 退化，并显式打 warn。
- `agent/packages/tiandao/src/ui/agent-ui.test.ts`：把现有 `realm_gate_rejected` 两组测试补成强 pin，断言 `scope==="player"` 且 `target===offline:<name>`，避免以后再退回 broadcast。

## 验收抓手

1. `realm_gate_rejected` 经过完整 server→agent→server narration 链后，产出的 narration 必须是 `scope:"player"`，且 `target` 为 canonical player id。
2. 两名在线玩家的端到端回归里，A 被 gate 拒绝时只有 A 能收到该 system warning，B 聊天流保持干净。
3. 兼容旧/坏 payload 的退化路径仍可工作，但必须打 warn，并且测试要明确区分“正常 payload 走 player scope”和“遗留 payload 才允许 broadcast fallback”。

## 反方裁决摘要

1. **Round 1（退化：当前会话无可用 subagent / delegate 工具，改为主代理手工反方裁决）**：反方论点是“这只是 finished plan 已记录的 minor follow-up，不算新 bug”。驳回理由：`uiResponseConsumer.ts:274-280` 与 `server/src/network/mod.rs:3125` 组合出的全服广播是当前运行时代码的真实行为；“已知”不等于“已修”，更不等于“不是 bug”。
2. **Round 2（同样为手工反方裁决）**：反方论点是“`UiRenderer` 现有 blur 版本会绕开大多数 `realm_gate_rejected`，所以广播问题不影响真实游玩”。驳回理由：server 权威拒绝路径仍然保留且有测试、文档、follow-up producer 共同依赖；只要出现 world-state 境界滞后、后续生产触发源直接发 gated 清晰版，错误广播就会立刻外露。换言之，这是被真实入口共享的协议缺口，不是死代码。

## 开放问题

1. `target_player` 应放回 `AgentUiResponsePayloadV1` 顶层，还是只对 `action="error" && reason="realm_gate_rejected"` 做局部扩展？建议选前者，避免继续把路由关键字段藏进 `params`。
2. 既然 `docs/finished_plans/plan-agent-ui-data-v1.md:361` 已认定这是 follow-up，修复 PR 是否顺手把同文档里的 “TSY target_player fallback” 与本条一起复核，避免再次出现“目标玩家信息丢失但靠 broadcast 兜底”的同型错误？

## 审计来源

bug-hunt 定点轮（范围：`agent-ui` / panel surface / follow-up side path；排除 tiandao revelation vfx flag loss、button_click context loss、TSY discovery target fallback）。本轮只读搜索 `schema → server agent_ui → tiandao ui consumer → server narration route → 既有 finished plan` 证据链，结论为 **report-only**：当前 worktree 仅新增 skeleton，不改源码。
