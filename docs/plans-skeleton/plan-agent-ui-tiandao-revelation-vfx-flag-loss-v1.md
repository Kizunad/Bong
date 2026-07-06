# plan-agent-ui-tiandao-revelation-vfx-flag-loss-v1（骨架）

> **骨架（草案）**。一句话主题：`agent_ui` 的 `tiandao_revelation` 生产链路在 `agent → server → client` 传输中丢失了“这是天道启示面板”的语义位，导致 client 端为该面板专门实现的暗蓝 vignette + 轻微 shake 在正常游玩里**永远不会触发**。已排除你指定不碰的 `tsy discovery target fallback`、`agent runtime locust warning duration drift`、`combat_event juice bridge gap`。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|---|---|---|---|
| P0 | 天道启示面板 VFX 语义位在 S2C payload 丢失 | plan_skeleton | ⬜ |
| P1 | S2C schema / server emit / client payload-path 回归测试补齐 | fix_pr | ⬜ |

## 接入面

- **进料**：agent `UiRenderer.renderUi({ scenario: "tiandao_revelation", ... })`（`agent/packages/tiandao/src/ui/uiRenderer.ts`）发布 `AgentUiRequestCommandV1`；该 command 按模板规则把天道启示设为 `realm_gate=5`（`docs/finished_plans/plan-agent-ui-data-v1.md:266-290`）。
- **中转**：server `process_agent_ui_cmd`（`server/src/network/agent_ui.rs:344-515`）校验 `realm_gate` / XML 后，构造 `AgentUiRequestPayloadV1` 下发 `bong:agent_ui_request`。
- **出料**：client `AgentUiPayloadHandler.handleRawRequest`（`client/src/main/java/com/bong/client/network/AgentUiPayloadHandler.java:126-140`）解析裸 JSON，调用 `AgentUiScreen.create(requestId, xml, timeoutTicks, currentTick)` 打开面板；`AgentUiScreen.init()` 再把 `isTiandaoRevelation` 写入 `AgentUiVfxState`，供 `AgentUiVfxPlanner` 决定是否画天道专属 vignette / shake（`client/src/main/java/com/bong/client/agentui/AgentUiScreen.java:183-189`，`client/src/main/java/com/bong/client/agentui/AgentUiVfxState.java:57-69`）。
- **共享类型 / bridge 面**：
  - `agent/packages/schema/src/payloads/agent-ui.ts:38-77`：command 含 `realm_gate`，但 S2C `AgentUiRequestPayloadV1` 只有 `request_id/target_player/xml/timeout_ticks`。
  - `server/src/network/agent_ui.rs:487-492`：server 真下发的 payload 也只有这四个字段。
  - `client/src/main/java/com/bong/client/network/AgentUiPayloadHandler.java:128-131`：client 侧按这四字段解析。

## P0 — 天道启示面板 VFX 语义位在 S2C payload 丢失

- **候选 bug（major）**：P3 规格明确要求“天道启示面板特有”暗蓝 vignette + 轻微 shake（`docs/finished_plans/plan-agent-ui-data-v1.md:288-290`），client 也确实做了 `isTiandaoRevelation` 开关和专属 VFX 分支（`AgentUiScreen.java:86-89,127-147,187-188`；`AgentUiVfxState.java:11-12,57-69`）。但生产链路中，agent 的 `realm_gate=5` 只存在于 `AgentUiRequestCommandV1`，server 下发给 client 的 `AgentUiRequestPayloadV1` 不携带任何 `panel_kind` / `vfx_profile` / `is_tiandao_revelation` 等替代语义位（`agent-ui.ts:63-77`；`agent_ui.rs:487-492`）；client payload handler 又始终走四参重载 `AgentUiScreen.create(...)`，该重载硬编码把 `isTiandaoRevelation` 设为 `false`（`AgentUiPayloadHandler.java:42-57`；`AgentUiScreen.java:117-124`）。结果是：**生产路径永远不可能把 `AgentUiVfxState.isTiandaoRevelation` 置 true，天道启示专属 VFX 代码整段不可达。**

### 复现路径

1. 让 agent 触发一块清晰版天道启示面板：`UiRenderer.renderUi({ scenario: "tiandao_revelation", targetPlayer.realm >= Spirit, ... })`。该路径按设计会生成 `realm_gate=5` command（`plan-agent-ui-data-v1.md:266-277`）。
2. server `process_agent_ui_cmd` 接收 command，校验通过后发送 `bong:agent_ui_request`，但 payload 只有 `request_id/target_player/xml/timeout_ticks`（`server/src/network/agent_ui.rs:487-492`）。
3. client `AgentUiPayloadHandler.handleRawRequest` 解析该 payload，固定调用 `AgentUiScreen.create(requestId, xml, safeTimeoutTicks, currentTick)`（`client/src/main/java/com/bong/client/network/AgentUiPayloadHandler.java:42-57`）。
4. 该四参重载直接落到 `create(..., false)`（`client/src/main/java/com/bong/client/agentui/AgentUiScreen.java:117-124`），`init()` 注册的 `AgentUiVfxState` 于是 `isTiandaoRevelation=false`（`AgentUiScreen.java:183-189`）。
5. `AgentUiVfxState.tiandaoVignetteActive()` / `tiandaoShakeActive()` 先看这个 flag，`false` 时恒返回 false（`client/src/main/java/com/bong/client/agentui/AgentUiVfxState.java:57-69`）。
6. 因此玩家实际打开任何“天道启示”面板时，只能得到通用 agent_ui 粒子/音效/fade-in，看不到计划规格中的天道专属压迫感 VFX。

### 根因链路

1. **语义源在 agent command 端**：天道启示和普通面板的区别，当前只体现在 `scenario` 与 `realm_gate=5` 上（`agent/packages/tiandao/src/ui/uiRenderer.ts`；计划规格 `plan-agent-ui-data-v1.md:266-290`）。
2. **S2C schema 把安全字段剥掉了，但没补显示字段**：`AgentUiRequestPayloadV1` 出于安全原因不下发 `realm_gate/allowed_button_ids`（`agent-ui.ts:63-77`），这是对的；问题在于它也**没有**补一个无安全含义的 `panel_kind` / `is_tiandao_revelation`。
3. **server emit 沿用了缺字段 schema**：`process_agent_ui_cmd` 发包时完全照 `AgentUiRequestPayloadV1` 组装，丢掉了区分天道启示所需的最后一个信号（`agent_ui.rs:487-492`）。
4. **client 生产入口只会走默认 false 分支**：`AgentUiPayloadHandler` 没有任何条件分支可把 payload 转成 `create(..., true)`，于是专门为 P3 写的 tiandao VFX 开关在实战中永远打不开（`AgentUiPayloadHandler.java:42-57`，`AgentUiScreen.java:117-147`）。
5. **现有测试只锁了“重载本身能接受 true/false”**：`AgentUiVfxPlannerTest` 会测 `AgentUiScreen.create(..., true)` 和默认 false，但没有一条测试覆盖真实 `bong:agent_ui_request` payload-path 能把天道启示语义带进来，所以这条断路没被现有测试网兜住（见 `client/src/test/java/com/bong/client/agentui/AgentUiVfxPlannerTest.java` 的 `create(..., true)` / 默认 false 用例）。

### 这个 bug 对实际游玩体验的影响

- 通灵境玩家实际触发“天道启示”时，看到的界面和普通秘境发现 / 传承面板相比，几乎只剩文案差异；计划里承诺的“暗蓝边缘压迫 + 轻微抖屏”的危险预警感完全丢失。
- 玩家会把“天道注意力掠过”误感知成一块普通弹窗，而不是高规格、不可忽视的世界级提示，削弱了面板的辨识度和情绪强度。
- 这不是“资产还没做”或“后补 polish”的问题；client 侧 VFX 实现已经存在，但 wire contract 不把语义送到终点，属于**协议/bridge 断路**。

### 影响面

- `agent/packages/schema/src/payloads/agent-ui.ts`
- `server/src/network/agent_ui.rs`
- `client/src/main/java/com/bong/client/network/AgentUiPayloadHandler.java`
- `client/src/main/java/com/bong/client/agentui/AgentUiScreen.java`
- `client/src/main/java/com/bong/client/agentui/AgentUiVfxState.java`
- `client/src/test/java/com/bong/client/agentui/AgentUiVfxPlannerTest.java`

## P1 — 修复建议

### 建议方案（推荐）

1. **不要把 `realm_gate` 原样下发给 client**，继续保留它的 server-only 安全属性。
2. 在 `AgentUiRequestPayloadV1` 新增一个**纯显示语义**字段，例如：
   - `panel_kind: "generic" | "tsy_discovery" | "elder_legacy" | "tiandao_revelation"`，或
   - `vfx_profile: "default" | "tiandao_revelation"`，或
   - 最小补丁版 `is_tiandao_revelation: boolean`
3. server `process_agent_ui_cmd` 在通过 realm gate 后，根据 command/场景衍生出该显示字段并随 S2C payload 下发。
4. client `AgentUiPayloadHandler` 解析该字段，命中天道启示时走 `AgentUiScreen.create(..., true)`。
5. 补一条 payload-path 回归测试：喂一份真实 `bong:agent_ui_request` JSON，断言打开后的 `screen.isTiandaoRevelation()==true`，而不是只测重载本身。

### 明确拒绝的伪修法

- **拒绝让 client 从 XML 文案猜面板类型**：例如看标题是不是“天意”、按钮是不是只有 `dismiss`。这会把展示语义埋进模板文本，未来改文案就会静默回归。
- **拒绝把 `realm_gate` 当成显示信号直接透传**：`realm_gate` 是安全字段，plan 和现有 schema 都明确不该下发；应该单独补无害的显示字段。

## 反方裁决（本会话无 subagent 能力，退化为本人双轮反方裁决）

### 第一轮反方

- **反方论点**：`realm_gate` 本来就是 server-only 安全字段，S2C payload 刻意不带它是设计选择；既然安全字段不下发，client 丢掉“天道启示”语义不算 bug。
- **驳回理由**：本题要修的不是“把 `realm_gate` 透传给 client”，而是“补一个无安全含义的显示语义位”。计划文档已经把“天道启示面板特有 VFX”写成已交付规格（`plan-agent-ui-data-v1.md:288-290`），client 代码也已经实现了 `isTiandaoRevelation` 分支；生产链路却没有任何字段能把这条语义送到 client，属于 contract 漏洞，而不是安全设计本身。

### 第二轮反方

- **反方论点**：即便没有专属 vignette/shake，面板文本仍能正常显示，玩家功能上没有卡住，所以最多算 polish 漏项，不值得立 bug skeleton。
- **驳回理由**：一方面，计划把天道启示与普通面板区分开来的核心之一就是 §5 视听规格；这不是额外锦上添花，而是“高规格世界提示”的一部分。另一方面，这里不是“尚未实现”，而是**实现已存在但生产路径永远不可达**，典型 bridge 断路。只要功能规格已承诺、代码已落地而 wire 让它恒死，就应按 bug 处理。

## 审计来源

2026-07-05 bughunt 单轮，限定 scope：`agent_ui` 的 `agent/packages/schema` + `server/src/network/agent_ui.rs` + `client/agentui/network` 生产链路。已显式避开 `tsy discovery target fallback`、`agent runtime locust warning duration drift`、`combat_event juice bridge gap`；并对现有 `docs/plan-bughunt-r*.md`、`docs/plans-skeleton/*.md`、`docs/finished_plans/plan-agent-ui-data-v1.md` 去重后确认：本题未被现有 bughunt skeleton/active plan 单独记录。
