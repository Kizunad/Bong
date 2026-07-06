# plan-bughunt-tiandao-agent-ui-click-context-loss-v1（骨架）

> **骨架（草案）**。一句话主题：tiandao / agent-ui / sidepaths 存在一条高置信协议断链 bug：`button_click` 从 UI 返回到天道推演时丢失 `player_uuid` 与 `scenario`，上下文只剩 `request_id + button_id`，导致多玩家/多面板侧路径无法把“谁点了什么、点的是哪个面板”可靠映射回具体后续决策。

## 结论

- **结论**：这是一个 **real bug**，不是注释漂移。
- **核心断点**：
  - `AgentUiResponsePayloadV1` 设计上只有 `{ request_id, action, params }`，没有 `player_uuid` / `target_player` / `scenario` 字段（`agent/packages/schema/src/payloads/agent-ui.ts:100-115`）。
  - server 收到点击后只把同一瘦身 payload 转发给 agent；session 在 compare-and-remove 后被消费掉，不再保留可反查玩家绑定（`server/src/network/agent_ui.rs:632-686`）。
  - `AgentUiRuntime` 只把原始 response 入队，不维护 `request_id -> { player, scenario }` 映射（`agent/packages/tiandao/src/ui/agentUiRuntime.ts:90-145`）。
  - `buttonClickBlock` 最终只把 `request_id` 和 `button_id` 喂给 LLM（`agent/packages/tiandao/src/context.ts:280-301`）。

## 复现路径

1. 生产侧 `processTsyZoneActivatedForUi()` 触发 UI 时，确实知道 `targetPlayer.uuid` 和 `scenario="tsy_discovery"`，并把二者与 `request_id` 一起写进日志；但这里只 log，不落任何运行态映射表（`agent/packages/tiandao/src/runtime.ts:1069-1084`）。
2. client 点击按钮后，协议只回传 `sendAgentUiResponse(requestId, "button_click", {"button_id":"..."})`；设计稿也明确这一点（`docs/finished_plans/plan-agent-ui-data-v1.md:206`）。
3. server 校验 `request_id + allowed_button_ids` 后，直接把 `{ request_id, action, params }` 转发到 `bong:agent_ui_response`；没有把 `target_player` / `scenario` 补回 response（`server/src/network/agent_ui.rs:668-686`）。
4. agent 侧 consumer/runtime 只缓存原始 response；队列元素里仍只有 `request_id + button_id`（`agent/packages/tiandao/src/ui/agentUiRuntime.ts:90-145`）。
5. `buttonClickBlock` 渲染出的上下文形如 `- request_id=... button_id=dismiss`；LLM 看不到玩家身份，也看不到面板类型（`agent/packages/tiandao/src/context.ts:293-300`）。

## 直接证据

- **协议缺字段**：`AgentUiResponsePayloadV1` 只有 `request_id / action / params`，注释也只声明 `button_click -> params.button_id`（`agent/packages/schema/src/payloads/agent-ui.ts:101-115`）。
- **服务端不补回玩家/场景**：`receive_agent_ui_response_system` 在 `take_if_match` 后直接 forward `request_id/action/params`，没有写任何 target/scope 元数据（`server/src/network/agent_ui.rs:681-686`）。
- **运行态不建索引**：`triggerUi` 时虽知道 `player=${targetPlayer.uuid} request_id=${result.requestId}`，但没有任何 `Map`/registry 保存这层关联（`agent/packages/tiandao/src/runtime.ts:1080-1084`）。
- **LLM 输入已不可逆**：`buttonClickBlock` 最终只拼 `request_id` 和 `button_id`（`agent/packages/tiandao/src/context.ts:293-300`）。
- **按钮 ID 冲突真实存在**：四种模板都复用 `dismiss`，即便单玩家也无法从 `button_id=dismiss` 反推是 TSY、elder、revelation 还是 blur 面板（`agent/packages/tiandao/src/ui/xmlTemplates.ts:99-101,125-127,174,190`）。
- **验收与实现已分叉**：既有 finished plan 的 P2 验收写的是“玩家点击‘进入’ → agent 收到 Completed → emit agent_cmd 解锁秘境”，但现实现只把 `button_click` 作为 LLM 文本线索注入，并未提供可定向执行该 follow-up 的身份上下文（`docs/finished_plans/plan-agent-ui-data-v1.md:123,220`）。

## 根因链路

1. 方案把 `AgentUiResponsePayloadV1` 设计成通用薄信封，只保留 `request_id/action/params`。
2. server 端把 `request_id` 当成一次性 session 校验键，而不是可长期追溯的上下文 key；校验通过后立即 compare-and-remove。
3. agent 端没有在 `triggerUi()` 时保存 `request_id -> { targetPlayer, scenario, template params }` 的 side table。
4. 最终注入 LLM 的上下文丢失了执行 follow-up 所需的最小充分信息。
5. 现有测试只验“button_id/request_id 进了 prompt”，没有验“玩家身份/场景可恢复”，因此这条断链一直绿灯（`agent/packages/tiandao/src/ui/agent-ui.test.ts:714-731`、`agent/packages/tiandao/tests/button-click-context.test.ts`）。

## 影响面

- **多玩家污染**：同一推演窗口内若两个玩家分别点击不同面板，LLM 只会看到一串无归属按钮事件，无法判断哪个选择属于哪个玩家。
- **单玩家也会歧义**：`dismiss` 在 TSY / elder / revelation / blur 四种模板复用；当前上下文无法区分“离开秘境”、“拒绝传承”、“关闭天道启示”还是“散去模糊感应”。
- **协议阻断后续功能**：任何要求 agent 产出“针对具体玩家的 follow-up command / narration / unlock”能力的 sidepath，都会被这个缺字段问题卡住；既有 plan 的“解锁秘境”验收口径在当前协议下并不自洽。
- **测试盲区会放大回归**：后续如果再加更多 UI 场景，只会继续复用这条缺少 identity/scenario 的按钮信号通道，歧义面会越来越大。

## 这个 bug 对实际游玩体验的影响

- 玩家点了天道面板后，天道后续反应可能“知道有人点了某个按钮”，但不知道**是哪名玩家**、也不知道**点的是哪种面板**。
- 结果体感会表现为：多人同时交互时 follow-up 跟错人，或者明明点的是“关闭启示”，后续推演却把它当成“放弃秘境/传承”的同类信号。
- 即便单人游玩，只要按钮是复用的 `dismiss`，天道对“你刚刚拒绝了什么”的理解也是不完整的，后续叙事和指令都可能失焦。

## 修复建议

- **协议补字段**：给 `AgentUiResponsePayloadV1` 增加 `player_uuid` 与 `scenario`，或在 `params` 里强制镜像这两个字段，并做双端 schema pin。
- **运行态补索引**：`AgentUiRuntime.triggerUi()` 建立 `request_id -> { targetPlayer, scenario, templateKind, templateParams }` registry；收到终态响应后读取并清理。
- **上下文改为强语义**：`buttonClickBlock` 不再只打印 `request_id/button_id`，至少输出 `player_uuid/scenario/button_id`。
- **回归测试**：
  - 两个不同玩家在同一窗口各点一次，断言注入上下文可区分 player。
  - 同一玩家对两个不同模板都点 `dismiss`，断言上下文仍可区分 scenario。
  - 若保留 request registry 方案，补 terminal cleanup 测试，防止 registry 泄漏。

## 两轮反方裁决（退化处理）

> 当前会话无可用 subagent / delegate 能力，未能外包给独立反方代理；以下为**同会话退化版双轮反方裁决**，仍显式记录反方论点与驳回理由。

### Round 1

- **反方论点**：`request_id` 已经足够；server 或 runtime 完全可以在别处反查到 player/scenario，这不一定是 bug。
- **驳回理由**：
  - 全仓检索未发现任何 `request_id -> player/scenario` 的持久映射或反查表。
  - 现存代码里 `request_id` 只被用于 session 校验、日志打印和上下文透传；校验通过后 session 被 `take_if_match` 消费，agent 侧再无反查来源（`server/src/network/agent_ui.rs:681-686`，`agent/packages/tiandao/src/runtime.ts:1080-1084`）。

### Round 2

- **反方论点**：当前也许默认单玩家、单面板顺序交互，`button_id` 模糊一点问题不大，属于未来扩展问题。
- **驳回理由**：
  - 这不是纯未来问题：今天就有四个模板复用 `dismiss`，单玩家单会话也会丢失“拒绝的是哪类面板”语义。
  - runtime 明确支持批量 drain 多个 `button_click` 事件进同一轮推演，说明设计上已接受“同窗口多事件”输入；而当前输入缺失最关键的归属信息，属于现时协议缺口，不是遥远扩展债。

## 建议路由

- `fix_pr`

## 审计来源

- bughunt 轮次：2026-07-05 tiandao / agent-ui / sidepaths 定向自检。
- 方法：只读搜索 + 协议链路追踪 + 现有测试盲区复核 + 双轮同会话反方裁决。
