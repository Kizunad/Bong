# plan-agent-ui-close-reason-drop-v1

> **活跃 BugFix plan**。一句话主题：`agent_ui_close` 的 `reason` 字段虽然由 server 专门下发给 client，用来区分 `Replaced` 与 `invalid_button_id/session_expired` 错误关闭，但在 `AgentUiPayloadHandler -> AgentUiStore -> AgentUiScreen` 链路里被完全吞掉，导致错误关闭与正常替换在玩家视角都变成“静默收屏”。

> 立项动机：本轮只看 `agent-ui / client bridge / panel runtime`，重点筛 `screen open path / panel state / overlay scope / fallback route / payload 字段`。已避开已知重复题：realm gate 广播泄漏、`button_click` 回流天道推演丢 `player_uuid/scenario`、agent_ui 覆层被 screen gate 提前吞掉、`tiandao_revelation` VFX 语义位丢失；也未与 `#931`/`#927` 重复。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|---|---|---|---|
| P0 | `agent_ui_close.reason` 客户端分流丢失 | bugfix | ⬜ |

## P0 — `agent_ui_close.reason` 客户端分流丢失

- **候选 bug（major，待第一性原理验证）**：`server` 明确把 `agent_ui_close.reason` 设计成客户端 runtime 分流字段，但 `client` 收到后只按 `request_id` 关面板，**完全不消费 `reason`**，导致 `invalid_button_id` 与 `session_expired` 都退化成与 `Replaced` 完全同形的“静默关闭”。

### 复现路径

1. 触发一个 `agent_ui` 面板，让 client 进入 `AgentUiScreen` 活跃态（`client/.../AgentUiPayloadHandler.java:139-148`）。
2. 让 server 走错误关闭分支之一：
   - `invalid_button_id`：`server/src/network/agent_ui.rs:671-684` 先向 Agent 发 `{ action:"error", params.reason:"invalid_button_id" }`，并按设计继续向 client 发 close；
   - `session_expired`：`server/src/network/agent_ui.rs:617-641` 在 stale response/无活跃 session 时向 client 发 `AgentUiClose(session_expired)` 防 UI 悬空。
3. client 经 `BongNetworkHandler.registerAgentUiChannels()` 收到 `bong:agent_ui_close` 裸 JSON（`client/src/main/java/com/bong/client/BongNetworkHandler.java:1053-1066`）。
4. `AgentUiPayloadHandler.handleRawClose()` 解析出 `reason` 后，仅调用 `AgentUiStore.receiveClose(requestId, reason)`（`client/.../AgentUiPayloadHandler.java:223-231`）。
5. `AgentUiStore.receiveClose()` **完全不看 `reason`**，只做 request_id 匹配并调用 `screen.receiveCloseSignal()`（`client/.../AgentUiStore.java:43-48`）。
6. `AgentUiScreen.receiveCloseSignal()` 又只是 `closeWithoutResponse()`，继续静默清屏（`client/.../AgentUiScreen.java:203-205,247-257`）。
7. 结果：玩家既看不到计划要求的“天道拒绝了这次操作”提示，也看不到“这次点击已过期/面板已失效”之类反馈，体验上与 `Replaced` 无法区分。

### 根因链路

- **设计契约已写死需要分流**：
  - `docs/finished_plans/plan-agent-ui-data-v1.md:52`：`invalid_button_id` close 信号到 client 后，要求“关闭面板并显示‘天道拒绝了这次操作’提示”。
  - `docs/finished_plans/plan-agent-ui-data-v1.md:210`：`reason=""` 代表 `Replaced`，`reason="invalid_button_id"` 等代表 `Error`，要求 client 依据 reason 分流提示。
  - schema 也把这一层语义保留到了 S2C：`agent/packages/schema/src/payloads/agent-ui.ts:126-139`，注释写明 `reason="invalid_button_id" | "session_expired" 时 client 显示提示后关闭`。
- **server 确实在生产这些 reason**：
  - `invalid_button_id` / `session_expired` 均通过 `send_agent_ui_close_to_client(..., Some(reason))` 下发（`server/src/network/agent_ui.rs:523-541,617-641,671-684`）。
- **client bridge 把 reason 读出来却不使用**：
  - `AgentUiPayloadHandler.handleRawClose()` 读出 `reason`（`client/.../AgentUiPayloadHandler.java:229`）；
  - `AgentUiStore.receiveClose(requestId, reason)` 签名保留了 `reason`，实现却只按 request_id 清屏（`client/.../AgentUiStore.java:43-48`）；
  - `AgentUiScreen.receiveCloseSignal()` 也没有 reason 参数，自然无法弹提示（`client/.../AgentUiScreen.java:203-205`）。
- **测试把退化行为 pin 成了“正常”**：
  - `client/.../AgentUiPayloadHandlerTest.java:85-101,145-160` 对带 `reason` 的 close 只断言“清掉 active screen”；
  - `client/.../AgentUiScreenProtocolTest.java:350-364` 只要求 `receiveCloseSignal()` “不发任何包 + 清 store”，没有任何错误提示断言。

### 影响面

- **这个 bug 对实际游玩体验的影响**：
  - 当玩家点击了已失效/被 server 判无效的按钮时，面板会直接消失，没有“这次操作被天道拒绝”反馈；玩家只会体感成 UI 自己没了，难以分辨是自己误操作、时序过期，还是网络抖动。
  - 当旧面板响应迟到命中 `session_expired` 时，也会静默收屏；玩家看不到“你当前看到的是过期面板”的信号，容易继续把问题归因到随机卡顿或面板 bug。
  - `Replaced` 原本就该静默，而 `invalid_button_id/session_expired` 理应有提示；现在三者在 client 上被压平成同一种关屏结果，等于把 close payload 的语义位白送掉。
- **涉及范围**：
  - server：`agent_ui_close` reason 生产逻辑正常；
  - client bridge/runtime：`AgentUiPayloadHandler`、`AgentUiStore`、`AgentUiScreen` 三段链路共同丢语义；
  - tests：client 侧现有单测未覆盖“错误关闭提示”契约，导致该缺口长期漏检。

### 修复建议

1. 给 client close 链路补上 reason 语义传递：
   - `AgentUiPayloadHandler.handleRawClose()` 不仅传 `request_id`，也要把 `reason` 交给 runtime；
   - `AgentUiStore.receiveClose(...)` / `AgentUiScreen.receiveCloseSignal(...)` 需要能区分 `null`（Replaced）与 `invalid_button_id/session_expired`。
2. 在 `reason != null` 的错误关闭路径上追加玩家可见反馈：
   - `invalid_button_id`：至少落一条本地 toast/actionbar/chat 提示，对齐 plan 的“天道拒绝了这次操作”；
   - `session_expired`：至少提示“面板已过期/当前操作已失效”，避免和 Replaced 混淆。
3. 补 client 测试：
   - `handleRawClose(reason="invalid_button_id")` 不仅要清屏，还要断言提示被触发；
   - `session_expired` 同理；
   - `Replaced` 继续保持静默，防止把正常替换路径也改得刷提示。

### 验收抓手

1. 手工/集成：
   - 构造 `invalid_button_id` close：client 看到明确错误提示，随后面板关闭；
   - 构造 `session_expired` close：client 看到“已过期”类提示，随后面板关闭；
   - 构造 `Replaced` close（无 reason）：client 静默切到新面板，不出现错误提示。
2. 单测：
   - `AgentUiPayloadHandlerTest` 新增 “带 `reason` close 会触发对应反馈” 断言；
   - `AgentUiScreenProtocolTest` 锁定 `Replaced` 静默、`Error close` 非静默的分叉行为；
   - 回归 pin：`reason` 仍可缺省，且不会回流多余 `agent_ui_response`。

## 反方裁决（退化记录）

> 当前会话未启用额外 subagent 流程；本题按用户要求做 **两轮反方裁决**，由当前会话手工完成，并在 plan / PR 中如实记录退化处理。

### 第一轮反方

- **反方论点**：`reason` 只是文档/注释语义，运行时静默关闭不一定算 bug；server 已经把 `invalid_button_id` / `session_expired` 发给 Agent 了，核心状态机并未损坏。
- **驳回理由**：
  - 这不是“未来可选增强”，而是已写进 finished plan 与 schema 注释的现行客户端契约（`plan-agent-ui-data-v1.md:52,210`；`agent-ui.ts:129-130`）。
  - 字段从 server 发到 client、client 也解析出来了，但最终完全不消费，这就是典型的 bridge/payload 语义丢失，不是纯 UX 愿望单。
  - 玩家视角把 `Replaced` 与 `Error/Expired` 混成同一个静默结果，会直接影响排障与操作理解，属于 runtime 行为缺口。

### 第二轮反方

- **反方论点**：也许 `reason` 本来只是给未来预留；现有 client tests 全绿，说明“只清屏”是当前接受行为。
- **驳回理由**：
  - 现有 tests 只验证“收到 close 后能清屏”，没有验证 plan 明确要求的“错误 close 要提示”，属于测试缺口，不是功能正确性的证明。
  - `AgentUiStore.receiveClose(requestId, reason)` 故意保留了 `reason` 参数，说明实现者本就预期这条语义会被消费，只是后续 wiring 漏掉了。
  - 如果 `reason` 真是预留字段，schema/plan 不会把 `invalid_button_id` / `session_expired` 写成现行 client 行为，更不会把 `Replaced` 和 `Error close` 明确区分。

## 去重说明

- 已排除用户点名的 4 个已出题方向：
  - realm gate 广播泄漏：不是本题；
  - `button_click` 回流天道推演丢 `player_uuid/scenario`：不是本题；
  - agent_ui 覆层被 screen gate 提前吞掉：不是本题；
  - `tiandao_revelation` VFX 语义位丢失：不是本题。
- 已 grep `docs/plan-bughunt-r*.md`、`docs/plans-skeleton/plan-bughunt-r*.md`、`docs/finished_plans/plan-agent-ui-data-v1.md`，未发现现成题目把“`agent_ui_close.reason` client 侧完全未消费”单独立项。

## 审计来源

bughunt 线程 CN（worktree: `bughunt-loop-20260705-cn`，分支：`bughunt-loop-20260705-cn-agent-ui3`）。本轮限定 `agent-ui / client bridge / panel runtime`，按 `screen open path / panel state / overlay scope / fallback route / payload 字段` 读码；结论是 **server 已生产、schema 已保留、client 已解析但 runtime 丢弃 `agent_ui_close.reason`**，属于新的 client bridge / panel runtime 真 bug。
