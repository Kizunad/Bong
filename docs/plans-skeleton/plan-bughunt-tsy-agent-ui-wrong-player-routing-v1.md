# plan-bughunt-tsy-agent-ui-wrong-player-routing-v1（骨架）

> **骨架（草案）**。一句话主题：TSY 首次激活的 agent-ui 面板在 `player_id` 失配时，会从“发给触发玩家”退化成“发给 `state.players[0]` 的首个在线玩家”。这不是广播泄漏，而是 **target_player 路由串台**：别的在线玩家会收到不属于自己的“活坍缩渊发现”面板，真正触发者反而看不到，后续点击意图也会沿错人上下文继续流。

> 立项动机：当前实现把这条退化路径写进了生产代码和单测，且 server bridge 也明确允许 `player_id` 降级成 `entity:<debug>`。因此只要触发玩家在 bridge/agent 两段之间掉出当前 snapshot，这个 UI channel 就会稳定误投给别的在线玩家，属于 agent runtime / protocol / bridge 边界上的真实错路由。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | TSY agent-ui target_player 失配后误投首个在线玩家 | fix_pr | ⬜ |

## P0 — TSY agent-ui target_player 失配后误投首个在线玩家

- **现象**：`agent/packages/tiandao/src/runtime.ts:1049-1050` 在处理 `tsy_zone_activated` 时使用 `state.players.find((p) => p.uuid === event.player_id) ?? state.players[0]`。也就是说，只要 `event.player_id` 不在当前 `world_state.players` 里，agent 就不会放弃本次面板，而是把 `triggerUi({ scenario: "tsy_discovery" })` 发给首个在线玩家。对应回归测试还把这条行为锁成了预期：`agent/packages/tiandao/tests/runtime.test.ts:2319-2337` 直接断言 “falls back to first online player when player_id not found in state”。
- **复现路径**：
  1. 服务器侧首次入场触发 `TsyZoneActivated`（`server/src/inventory/tsy_loot_spawn.rs:133-145`）。
  2. bridge 层把 `triggering_player_entity` 解析成 `player_id`；若该 entity 此时查不到 `Username`+`Client`，就降级成 `entity:<debug>`（`server/src/network/tsy_event_bridge.rs:149-163`）。
  3. agent runtime 下一轮消费 `bong:tsy_event` 时，`event.player_id` 无法命中 `state.players[].uuid`，于是走 `?? state.players[0]`。
  4. `UiRenderer.renderUi()` 据此把 `AgentUiRequestCommandV1.target_player` 写成错误玩家 UUID，错误玩家收到 TSY 发现面板，真实触发者没有面板。
- **根因链路**：这是一个双边退化组合 bug，不是单点偶发。server bridge 先把“找不到触发玩家身份”编码成可过 schema 的 `entity:*` 字符串；agent runtime 再把“找不到目标玩家”解释为“随便挑一个在线玩家继续送达”。两边都没有把“身份失配”当成 hard stop，导致 request-response 的目标玩家语义被破坏。
- **为什么正常游玩可达**：`publish_tsy_zone_activated_events` 自己的注释与测试已承认 `entity 无 Username（entity 已 despawn / 非 Client）` 会发生，并以 `entity:*` 作为生产 fallback（`server/src/network/tsy_event_bridge.rs:146-163,404-431`）。这意味着触发玩家只要在 Redis 事件发出前后经历断线、实体替换、或 bridge 读取窗口里短暂失去 `Client` 身份，agent 端就会收到一个永远无法命中 `state.players[].uuid` 的 `player_id`，随后误投给别的在线玩家。
- **对实际游玩体验的影响**：多人在线时，A 触发的 TSY 首次发现面板可能直接弹到 B 身上。轻则 B 被迫看到一块与自己无关的秘境面板、A 完全错过发现提示；重则 B 的按钮点击会继续沿错误 request_id/target_player 上下文流动，后续“进入秘境 / 仅观察 / 关闭”的意图被系统理解成 B 的，而不是原触发者 A 的。realm gate 只会把“误投的清晰面板”降成“误投的模糊面板”，并不能修复错人。
- **影响面**：
  - `server/src/network/tsy_event_bridge.rs`：把缺失身份降级成 `entity:*`，没有终止该 event。
  - `agent/packages/tiandao/src/redis-ipc.ts`：无额外校验，照单缓存该 `tsy_zone_activated`。
  - `agent/packages/tiandao/src/runtime.ts`：把 miss 当成“发给首个在线玩家”。
  - `agent/packages/tiandao/src/ui/uiRenderer.ts`：把错误 `targetPlayer.uuid` 写入 `AgentUiRequestCommandV1.target_player`，把串台路由固化到 server session。
- **建议修复范围 / 模块**：优先收口 `server/src/network/tsy_event_bridge.rs` 与 `agent/packages/tiandao/src/runtime.ts`。高置信方向有两条：
  1. bridge 侧：`player_id` 解析失败时不要发可消费的 `tsy_zone_activated`，直接 drop 并记 warn/metric。
  2. agent 侧：`event.player_id` 命不中当前在线玩家时直接 skip，不允许 fallback 到 `state.players[0]`。
  两边至少要落一边 hard stop；最差也必须把 `entity:*` 视为不可路由的失效身份，而不是继续发 UI。
- **验收抓手**：
  1. 新增 server/agent 双端 pin：`player_id` 为 `entity:*` 或任意 miss 时，`processTsyZoneActivatedForUi` 不得调用 `triggerUi`。
  2. 两人在线场景下，A 触发 TSY、B 在线旁观；无论 A 断线还是快照 miss，B 都不应收到 A 的面板。
  3. happy path 仍要保住：`player_id="offline:A"` 命中时，只给 A 发 `agent_ui_request`。
  4. 监控日志需可观测：出现 drop/skip 时要带 `family_id`、原始 `player_id`、tick，便于排查时序问题。

## 反方裁决摘要

1. **Round 1（退化处理：当前会话未启用 subagent，由主代理执行反方）**：反方主张“这是故意的 UX 兜底，不算 bug；总比没人收到面板好”。驳回理由：这里承载的是玩家专属 `target_player` 语义，不是广播提示；把 A 的事件交给 B 会直接篡改 request-response 目标对象，比“没人收到”更坏，因为它制造了错误交互而非缺交互。
2. **Round 2（退化处理：继续由主代理做第二轮反方）**：反方主张“server realm_gate 会兜底，最多是高阶玩家看到一个模糊面板”。驳回理由：realm_gate 只能决定清晰/模糊，不能决定“该不该发给这个人”。而且高阶旁观者会拿到完整面板，低阶旁观者也会拿到错误的模糊面板；真正触发者仍然没有面板，因此这不是安全兜底，是错路由。

## 开放问题

1. `player_id` 解析失败时应在 bridge 侧直接 drop，还是允许 agent 侧统一 skip 并打 telemetry？两边都能修，但最好只保留一处权威闸门，避免未来再出现别的 runtime 复用到 `entity:*`。
2. 除 TSY 外，仓库里凡是“事件带 player_id、下游再据此挑 target_player”的 agent runtime 都值得顺手扫一遍，确认没有第二处 `?? state.players[0]` 同型退化。

## 审计来源

bughunt 线程 CH，限定 agent runtime / protocol / bridge 侧，优先查 request-response、scenario/player 上下文与 UI channel 路由。已显式避开既有题：realm gate 广播泄漏、button_click 回流丢 `player_uuid/scenario`、以及近期 agent/schema 已提题。当前结论为 **report-only**：只立 skeleton，不改代码；两轮反方裁决因当前会话不可再开 subagent，按用户要求在此如实记录退化处理、反方论点与驳回理由。
