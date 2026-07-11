# plan-agent-ui-close-reason-drop-v1

> **活跃 BugFix plan（外部 review gate 阻塞）**。一句话主题：`agent_ui_close.reason` 的错误关闭语义同时存在 server 生产断点与 client 消费断点，导致 `invalid_button_id/session_expired` 在玩家视角退化为“静默收屏”。

> 立项动机：本轮只看 `agent-ui / client bridge / panel runtime`，重点筛 `screen open path / panel state / overlay scope / fallback route / payload 字段`。已避开已知重复题：realm gate 广播泄漏、`button_click` 回流天道推演丢 `player_uuid/scenario`、agent_ui 覆层被 screen gate 提前吞掉、`tiandao_revelation` VFX 语义位丢失；也未与 `#931`/`#927` 重复。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|---|---|---|---|
| P0 | `agent_ui_close.reason` 生产/消费断链 | bugfix | ⏳ |

## P0 — `agent_ui_close.reason` 生产/消费断链

- **已确认 bug（major）**：既有契约要求 `invalid_button_id/session_expired` 通过带 reason 的 close 给玩家错误反馈；实际 `invalid_button_id` 在 server 端没有生成 close，`session_expired` 虽能到达 client，却与所有其他 reason 一样被 runtime 丢弃。

### 复现路径

1. 触发一个 `agent_ui` 面板，让 client 进入 `AgentUiScreen` 活跃态（`client/.../AgentUiPayloadHandler.java:139-148`）。
2. 让 server 走错误关闭分支之一：
   - `invalid_button_id`：修复前只向 Agent 发 `{ action:"error", params.reason:"invalid_button_id" }`，session 仍保持 `Open`，没有任何 `AgentUiClose` S2C；
   - `session_expired`：在 stale response/无活跃 session 时会向 client 发 `AgentUiClose(session_expired)` 防 UI 悬空。
3. client 经 `BongNetworkHandler.registerAgentUiChannels()` 收到 `bong:agent_ui_close` 裸 JSON（`client/src/main/java/com/bong/client/BongNetworkHandler.java:1053-1066`）。
4. `AgentUiPayloadHandler.handleRawClose()` 解析出 `reason` 后，仅调用 `AgentUiStore.receiveClose(requestId, reason)`（`client/.../AgentUiPayloadHandler.java:223-231`）。
5. 修复前 `AgentUiStore.receiveClose()` **完全不看 `reason`**，只做 request_id 匹配并调用无参 `screen.receiveCloseSignal()`。
6. `AgentUiScreen` 的按钮点击会先发送 response 并本地关屏；因此 server 错误 close 到达时 `AgentUiStore` 往往已经为空，不能只修“活跃 screen 匹配”分支。
7. 修复前 `AgentUiScreen.receiveCloseSignal()` 只是 `closeWithoutResponse()`，继续静默清屏。
8. 结果：玩家既看不到“天道拒绝了这次操作”，也看不到“面板已过期”，体验上与 `Replaced` 无法区分。

### 根因链路

- **设计契约已写死需要分流**：
  - `docs/finished_plans/plan-agent-ui-data-v1.md:52`：`invalid_button_id` close 信号到 client 后，要求“关闭面板并显示‘天道拒绝了这次操作’提示”。
  - `docs/finished_plans/plan-agent-ui-data-v1.md:210`：`reason=""` 代表 `Replaced`，`reason="invalid_button_id"` 等代表 `Error`，要求 client 依据 reason 分流提示。
  - schema 也把这一层语义保留到了 S2C：`agent/packages/schema/src/payloads/agent-ui.ts:126-139`，注释写明 `reason="invalid_button_id" | "session_expired" 时 client 显示提示后关闭`。
- **server 生产端存在一处断链**：
  - `session_expired` 已通过 `send_agent_ui_close_to_client(..., Some("session_expired"))` 下发；
  - `invalid_button_id` 修复前只 emit Redis Error，不移除 Open session，也不发送 client close，与 finished plan 的 Error 终态契约直接冲突。
- **client bridge 把 reason 读出来却不使用**：
  - `AgentUiPayloadHandler.handleRawClose()` 读出 `reason`（`client/.../AgentUiPayloadHandler.java:229`）；
  - `AgentUiStore.receiveClose(requestId, reason)` 签名保留了 `reason`，实现却只按 request_id 清屏（`client/.../AgentUiStore.java:43-48`）；
  - `AgentUiScreen.receiveCloseSignal()` 也没有 reason 参数，自然无法弹提示（`client/.../AgentUiScreen.java:203-205`）。
- **测试把退化行为 pin 成了“正常”**：
  - `client/.../AgentUiPayloadHandlerTest.java:85-101,145-160` 对带 `reason` 的 close 只断言“清掉 active screen”；
  - `client/.../AgentUiScreenProtocolTest.java:350-364` 只要求 `receiveCloseSignal()` “不发任何包 + 清 store”，没有任何错误提示断言。
  - `server/src/network/agent_ui.rs` 的旧测试反而断言 `invalid_button_id` 后 session 保持 `Open`，把与设计契约相反的状态锁成了绿色。

### 影响面

- **这个 bug 对实际游玩体验的影响**：
  - 当玩家点击了已失效/被 server 判无效的按钮时，面板会直接消失，没有“这次操作被天道拒绝”反馈；玩家只会体感成 UI 自己没了，难以分辨是自己误操作、时序过期，还是网络抖动。
  - 当旧面板响应迟到命中 `session_expired` 时，也会静默收屏；玩家看不到“你当前看到的是过期面板”的信号，容易继续把问题归因到随机卡顿或面板 bug。
  - `Replaced` 原本就该静默，而 `invalid_button_id/session_expired` 理应有提示；现在三者在 client 上被压平成同一种关屏结果，等于把 close payload 的语义位白送掉。
- **涉及范围**：
  - server：`invalid_button_id` 缺 Error 终态与 close S2C；`session_expired` 生产正常；
  - client bridge/runtime：`AgentUiPayloadHandler` 已正确解析并转交 reason，真正的消费断点在 `AgentUiStore` / `AgentUiScreen`；
  - tests：server/client 都有与现行契约不完整或相反的断言，导致缺口长期漏检。

### 修复建议

1. 给 client close 链路补上 reason 语义传递：
   - 保持 `AgentUiPayloadHandler.handleRawClose()` 现有解析/转交行为，让 runtime 真正消费已传入的 `reason`；
   - `AgentUiStore.receiveClose(...)` / `AgentUiScreen.receiveCloseSignal(...)` 需要能区分 `null`（Replaced）与 `invalid_button_id/session_expired`。
2. 在 `reason != null` 的错误关闭路径上追加玩家可见反馈：
   - `invalid_button_id`：至少落一条本地 toast/actionbar/chat 提示，对齐 plan 的“天道拒绝了这次操作”；
   - `session_expired`：至少提示“面板已过期/当前操作已失效”，避免和 Replaced 混淆。
3. 补 client 测试：
   - `handleRawClose(reason="invalid_button_id")` 不仅要清屏，还要断言提示被触发；
   - `session_expired` 同理；
   - `Replaced` 继续保持静默，防止把正常替换路径也改得刷提示。
4. 修正 server 的 `invalid_button_id` 分支：emit Redis Error 后移除 session，并发送 `{ request_id, reason:"invalid_button_id" }` close S2C。

### 验收抓手

1. 手工/集成：
   - 构造 `invalid_button_id` close：client 看到明确错误提示，随后面板关闭；
   - 构造 `session_expired` close：client 看到“已过期”类提示，随后面板关闭；
   - 构造 `Replaced` close（无 reason）：client 静默切到新面板，不出现错误提示。
2. 单测：
   - `AgentUiPayloadHandlerTest` 新增 “带 `reason` close 会触发对应反馈” 断言；
   - `AgentUiScreenProtocolTest` 锁定 `Replaced` 静默、`Error close` 非静默的分叉行为；
   - 回归 pin：`reason` 仍可缺省，且不会回流多余 `agent_ui_response`。
   - server 锁定 `invalid_button_id` 同时产生 Redis Error、close S2C，并终止 Open session。

## 反方裁决（退化记录）

> 初始 bughunt 未启用额外 subagent；本 BugFix 闭环在完成后另启无上下文、严格只读 validator，并在 PR 中记录裁决结果。

### 第一轮反方

- **反方论点**：`reason` 只是文档/注释语义，运行时静默关闭不一定算 bug；server 已经把 `invalid_button_id` 发给 Agent，核心状态机未必损坏。
- **驳回理由**：
  - 这不是“未来可选增强”，而是已写进 finished plan 与 schema 注释的现行客户端契约（`plan-agent-ui-data-v1.md:52,210`；`agent-ui.ts:129-130`）。
  - `session_expired` 字段从 server 发到 client、client 也解析出来了，但最终完全不消费；`invalid_button_id` 甚至没有跨过 server→client，这不是纯 UX 愿望单。
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

bughunt 线程 CN（worktree: `bughunt-loop-20260705-cn`，分支：`bughunt-loop-20260705-cn-agent-ui3`）。本轮第一性原理复核后更正原 skeleton 结论：**schema 契约完整；server 只生产 `session_expired` close、漏产 `invalid_button_id` close；client 对已收到的 reason 全部丢弃**。这是 server 终态与 client runtime 共同构成的真实协议断链。

## Finish Evidence

### 第一性原理验证

- client 红测：Java 17 下定向运行 `AgentUiPayloadHandlerTest`，新增契约断言得到 **27 tests / 3 failed**；失败精确对应 `invalid_button_id` 无提示、`session_expired` 无提示、screen 已本地关闭后迟到 reason 仍静默。
- server 红测：`system_invalid_button_id_emits_error_response_and_close_s2c` 修复前失败，实际 close payload 数为 `0`，证明原 skeleton 对 server 已下发 close 的判断不实。
- 时序补充：`AgentUiScreen.onButtonClicked()` 会先本地清空 `AgentUiStore`，所以错误 close 的消费逻辑必须覆盖“无 active screen”分支，同时在存在不同 request_id 的新 screen 时抑制旧提示。

### 落地清单

- `server/src/network/agent_ui.rs`
  - `invalid_button_id` 现在 emit Redis Error 后终止 session，并经 `bong:agent_ui_close` 下发同名 reason。
  - system 测试同时断言 Redis response、close S2C payload 与 session 移除。
- `client/src/main/java/com/bong/client/agentui/AgentUiCloseFeedback.java`
  - `invalid_button_id` / `session_expired` 独立中文文案；未知非空 reason 有可见兜底；空 reason 保持 Replaced 静默。
  - 复用现有 `BongToast` HUD 活链路，警示色展示 3 秒。
- `AgentUiStore` / `AgentUiScreen`
  - reason 传入 screen close 语义；按钮响应先登记 pending request，再本地关屏。
  - pending 只允许匹配 request_id、TTL 内的错误 close 消费一次；重复、未知、跨 request、新生命周期、TTL 边界与单调时钟回拨均 fail-closed。
- client tests
  - `AgentUiCloseFeedbackTest`、`AgentUiPayloadHandlerTest`、`AgentUiScreenProtocolTest` 覆盖映射、状态转换、静默 Replaced、迟到 close 与不回包契约。
  - `AgentUiCloseChannelIntegrationTest` 从共享 fixture 原始 bytes 经生产 dispatch/main-thread 入口直到 `BongToast` HUD command，并覆盖畸形 payload 隔离。
- `agent/packages/schema/samples/agent-ui-close.channel-wire.sample.json`
  - 共享 fixture 锁定专属 channel 与 Replaced / `session_expired` / `invalid_button_id` 三种生产 wire；server encoder、schema 与 client receiver 三端对拍。

### 关键 commit（2026-07-10）

- `fd25b1be` — 提升 agent UI 关闭原因修复 plan
- `9c514cf4` — 补齐 agent UI 关闭原因提示映射
- `de3d4488` — 传递 agent UI 关闭原因到面板状态
- `9ed1b4a4` — 锁定 agent UI 关闭原因全链路回归
- `a015d0e0` — 收束非法按钮为 agent UI 错误关闭
- `53addd8e` / `6965cdf2` — 关联并锁定迟到 close 的完整 request 生命周期
- `4f5725d1` / `a42bd573` — 复用生产线程调度入口并补原始 bytes 集成验收
- `24f4a43f` / `504b8d5e` — 共享 wire fixture 与 server 生产 encoder 对拍
- `1e52e097` / `f2428f57` — 收紧单调 TTL、非法输入与时钟边界

### 测试结果

- client 既有全量：Java 17，`./gradlew test build` → **3716 passed / 0 failed**。
- client 最终聚焦：Java 17，四组 agent UI close 测试 → **73 passed / 0 failed**。
- schema 既有全量：`npm test` → **790 passed / 0 failed**；最终聚焦 `npm test -- --run tests/agent-ui.test.ts` → **45 passed / 0 failed**。
- server 最终聚焦：`cargo test network::agent_ui::tests` → **49 passed / 0 failed**；`cargo fmt --check` → 通过。
- server 全量：`cargo test` → **10930 passed / 1 timing failure / 1 ignored**；唯一失败为无关的 `world::poi_novice::scatter_surface_stashes_terminates_when_existing_poi_blankets_the_aabb`（并发负载下 11.8 秒越过耗时阈值），立即单独复跑 → **1 passed，6.65 秒**。
- server clippy：规定命令在 Rust 1.96 下被仓库基线 **69 个**无关 lint 拦截（集中于 botany/combat/cultivation/fauna/world 等）；本次改动文件 `server/src/network/agent_ui.rs` 没有 clippy diagnostic，未越界修改这些模块。
- GitHub e2e：run `29122770407` attempt 2 在同一 HEAD `f2428f57` → **success**；client、schema、agent、server、smoke 与 bot e2e 全部通过。attempt 1 唯一失败为无关 forge 场景超时，原样重跑恢复，未跨域修改 forge。
- CodeRabbit：唯一 inline thread 已 `resolved + outdated`，当前未解决 thread **0**。
- 最终无上下文只读 validator：`fork_context:false`、`gpt-5.6-sol` Ultra/priority → **VERDICT: PASS**，明确 blocker 为 **无**。

### 跨栈核验

- schema：`AgentUiCloseReasonV1 = "invalid_button_id" | "session_expired"` 保持不变。
- server：`receive_agent_ui_response_system` → `encode_agent_ui_close_payload` → `send_agent_ui_close_to_client`，生产 bytes 与共享 fixture 完全一致。
- client：`BongNetworkHandler.registerAgentUiChannels` → `AgentUiPayloadHandler.dispatchRawClose` → `AgentUiStore.receiveClose` → `AgentUiScreen.receiveCloseSignal(reason)` → `AgentUiCloseFeedback` → `BongToast`。

### 遗留 / 后续

- 本修复不修改 schema/wire 版本，不涉及 agent 推演逻辑、世界观或真元守恒。
- 仓库级 Rust 1.96 clippy 基线清理不纳入本单一 BugFix plan；本次最终闭环无 blocker。

### Review 返工（2026-07-10）

- PR #1159 首轮统一 review 有效指出：active screen 为空时不能无条件接受任意 reason close，必须关联“本地按钮响应已发出、仍待 server 确认”的 request_id。
- 返工状态机：待确认 request 只在 request_id 匹配且 TTL 未过期时消费一次；重复 close、未知 request、TTL 到期均忽略；新 request 开始时清除旧 pending，避免新生命周期结束后旧 close 误弹。
- 等价跨端集成、e2e/相关 checks、CodeRabbit 清零与最终 Ultra validator PASS 均已完成；统一 `/review` 仍是归档前置 gate。

### 最终 review gate 阻塞（2026-07-11）

- 按约 20 分钟节奏共触发三轮统一 `/review`：run `29138145455`、`29138676111`、`29139232405`。
- 三轮结果完全一致：每轮 4/4 `gpt-5.6-sol high` reviewer 各重试 3 次，均由外部 provider 返回 `503 No available channel for model gpt-5.6-sol`；所有 reviewer 置信度为 `0`、状态为 `unclear`，未产生任何 PR 代码 finding。
- 这是 review 基础设施容量阻塞，不修改 `.github/scripts/review.mjs`，也不越界改业务代码；三轮上限后停止重试。
- `[BLOCKED: 统一 /review 外部 provider 连续三轮 503；需 provider 恢复后重新评论 /review，得到真实 PASS 才可归档]`
