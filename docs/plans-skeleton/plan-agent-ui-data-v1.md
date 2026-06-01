# Bong · plan-agent-ui-data-v1 · 骨架

**UI-as-Data：天道驱动的动态交互面板**——天道 Agent 根据当前世界状态生成 OwoUI XML 布局，Valence 以 `bong:agent_ui_request` CustomPayload 下发给指定玩家，Fabric 微端用 OwoUIAdapter 实时渲染动态面板；玩家的交互（按钮点击 / 选择）经 `bong:agent_ui_response` 回传 Agent 驱动后续推演。适用场景：活坍缩渊秘境发现界面、垂死大能最后传承对话、天道"气运审判"启示面板。

**来源**：`docs/scribble.md` §"结合你的 Agent 架构的最佳实践" → "大模型推演 UI... OwoUIAdapter.createFromXML"

**交叉引用**：
- `plan-agent-v2.md` ✅ — Agent 三 Agent 并发 + Arbiter 已实装，本 plan 在此基础上扩展 UI 下发管道
- `plan-ipc-schema-v1.md` ✅ — TypeBox schema source of truth，新增 `AgentUiRequestPayloadV1` / `AgentUiResponsePayloadV1`
- `plan-client.md` ✅ — Fabric CustomPayload 框架 + HudRenderLayer 基础设施
- `plan-dying-elder-v1.md` ⬜ skeleton — 垂死大能对话树是本 plan 最高优先场景之一
- `plan-daozhan-v1.md` ⬜ skeleton — 道伥遭遇界面可复用本 plan 渲染管道
- `plan-tsy-zone-v1.md` ✅ — 活坍缩渊秘境（活死坍缩渊）是本 plan 核心 use case 之一

**worldview 锚点**：
- **§八 天道行为准则**："天道利用玩家之间的贪欲来平衡"、"暗示某个修士气运将尽"——天道的 UI 绝不是客服弹窗，而是间接的命运提示
- **§五 境界感知**：通灵境玩家"能感知天道注意力（危机预警）"——UI 应在通灵+ 境界才会出现"天道审判"类面板；低境界只能看到模糊的感应残影
- **§十六 活坍缩渊（秘境）**：秘境的发现、进入许可、内部分层信息，适合用动态面板呈现

**qi_physics 锚点**：不涉及真元计算。面板内若展示灵气浓度数据，数值只读取 `zone.spirit_qi`，不触发任何 `QiTransfer`。

**前置依赖**：
- `plan-agent-v2.md` ✅ — Agent 推演能力 + Redis IPC
- `plan-client.md` ✅ — CustomPayload 双向管道
- `plan-ipc-schema-v1.md` ✅ — schema 注册流程

---

## 接入面 Checklist

- **进料**：
  - Agent Arbiter 输出的世界推演结果（已有 `WorldModel` Redis 持久化）
  - `zone.spirit_qi` / `player.realm` / `player.tiandao_attention`（server ECS query）
  - OwoUI XML 规范（`client/resourcepack/owo-ui/` 已有若干静态 layout，本 plan 扩展为动态生成）
- **出料**：
  - `bong:agent_ui_request` CustomPayload：`{ request_id: UUID, target_player: UUID, xml: String, timeout_ticks: u32 }`
  - `bong:agent_ui_response` CustomPayload：`{ request_id: UUID, action: String, params: Record<string, string> }`
  - server-side `AgentUiSessionStore`（内存 HashMap, TTL = timeout_ticks）
  - agent-side `UiResponseHandler` 消费响应并注入下一轮推演
- **共享类型**：新增 `AgentUiRequestPayloadV1` / `AgentUiResponsePayloadV1`（TypeBox schema + Rust serde）
- **跨仓库契约**：

| 层 | symbol |
|----|--------|
| server | `network/agent_ui.rs` — `AgentUiSessionStore` + CustomPayload 发送 / 接收 handler |
| server | `network/agent_ui.rs` — `AgentUiResponseEvent` → EventWriter，system 消费后转 Redis |
| client | `AgentUiPayloadHandler.java` — CustomPayload 解析 + OwoUIAdapter.createFromXML |
| client | `AgentUiScreen.java` — 动态 OwoUI 屏幕，按钮 ID 绑定 → 发 `bong:agent_ui_response` |
| agent | `packages/tiandao/src/ui/uiRenderer.ts` — 模板 → XML 生成；`uiResponseConsumer.ts` — Redis 消费响应 |
| schema | `AgentUiRequestPayloadV1` / `AgentUiResponsePayloadV1` TypeBox + JSON sample |

---

## §0 设计约束（技术边界）

1. **XML 安全**：Agent 生成的 XML 必须通过 server 侧白名单过滤（允许 `<label>` / `<button>` / `<flow-layout>` / `<grid-layout>` / `<texture>` 等，禁止 `<script>` 或自定义标签）。server 在转发前 sanitize。
2. **面板生命周期**：每个 `request_id` 有 `timeout_ticks`（默认 600t = 30s），超时 server 自动关闭面板并通知 agent。玩家强制关闭（ESC）也发回 `{ action: "dismissed" }`。
3. **无状态 XML**：XML 不允许嵌入脚本或持久 state——所有交互状态在 Agent 侧管理，客户端只负责渲染和收发。
4. **单面板互斥**：同一玩家同一时刻最多一个 `agent_ui` 面板，新请求自动关闭旧面板。
5. **境界门控**：醒灵 / 引气境玩家只能看"模糊残影"版本（XML 简化，无可点击元素）；凝脉+ 才能看完整面板。详细规则由 Agent 在 XML 生成时根据 `player.realm` 决策，不在 server 强制（server 只做 sanitize）。

---

## 阶段总览

| 阶段 | 内容 | 状态 | 验收标准 |
|------|------|------|---------|
| **P0** | schema 定稿 + server → client CustomPayload 链路 | ⬜ | 单测：server 发 `AgentUiRequestPayloadV1` → client handler 收到并 parse XML；双端 schema sample roundtrip |
| **P1** | client OwoUI XML 动态渲染 + 按钮响应 → `bong:agent_ui_response` | ⬜ | 集成测试：server 下发含 2 个按钮的 XML → client 渲染 → 点击按钮 → server 收到正确 `action` |
| **P2** | agent 侧 XML 生成 + Redis `bong:agent_ui_response` 消费 | ⬜ | mock 推演：agent 生成活坍缩渊秘境面板 → 玩家点击"进入" → agent 收到响应 → emit `agent_cmd` 解锁秘境 |
| **P3** | 3 种标准面板模板（秘境发现 / 垂死传承 / 天道启示）| ⬜ | 每种模板 XML 通过 OwoUI 渲染测试；境界门控验证（低境看模糊版）；timeout dismiss 闭环 |

---

## §1 P0：Schema + CustomPayload 链路

- [ ] TypeBox `AgentUiRequestPayloadV1`（`agent/packages/schema/`）：`request_id: UUID`, `target_player: UUID`, `xml: String`, `timeout_ticks: u32`, `realm_gate: number`（最低可见境界，0 = 全境界可见）
- [ ] TypeBox `AgentUiResponsePayloadV1`：`request_id: UUID`, `action: String`（"button_click" / "dismissed" / "timeout"）, `params: Record<string, string>`
- [ ] `server/src/network/agent_ui.rs`：`AgentUiSessionStore` Resource（HashMap<UUID, AgentUiSession>）+ `send_agent_ui_system` + XML sanitize（allowlist tag 过滤）
- [ ] `client/.../AgentUiPayloadHandler.java`：接收 `bong:agent_ui_request` → parse → 触发屏幕渲染
- [ ] ≥ 8 单测：schema roundtrip / sanitize 拦截非法标签 / timeout 触发关闭 / 单面板互斥

---

## §2 P1：Client OwoUI 动态渲染

- [ ] `AgentUiScreen.java`：extends `BaseOwoScreen`，从 `AgentUiRequestPayloadV1.xml` 调用 `OwoUIAdapter.createFromXML` 构建组件树
- [ ] 按钮 ID 约定：`<button id="enter_secret_realm">进入秘境</button>` → 点击触发 `sendAgentUiResponse(requestId, "button_click", Map.of("button_id", "enter_secret_realm"))`
- [ ] 自定义 CSS-style 样式扩展：`<label style="color: #FF4444">` 支持 16 进制颜色（通过 OwoUI theme 机制）
- [ ] 面板背景：使用 `BongSpriteParticle` vignette + 半透明黑底，贴合末法残土暗色调（`#1A0D0D` vignette）
- [ ] ≥ 10 单测：XML parse 各合法标签 / 非法标签被过滤 / 按钮点击发包 / dismiss 发包 / timeout 发包

---

## §3 P2：Agent 侧 XML 生成 + 响应消费

- [ ] `agent/packages/tiandao/src/ui/`：`xmlTemplates.ts`（模板字符串 + 参数插值），`uiRenderer.ts`（根据 world state 选择模板 + 填充），`uiResponseConsumer.ts`（Redis `bong:agent_ui_response` 订阅 → 注入 Arbiter 下一轮推演）
- [ ] 生成规则：Agent 不直接向 LLM 请求 XML 字符串（延迟 + 幻觉风险）；而是从预定义模板库按 `world_state` 字段填充——LLM 只决策"选哪个模板 + 填什么文字"
- [ ] narration 模板（scope: player, style: perception）：
  - 活坍缩渊发现："混沌的灵气涌动在你面前——这里曾经是什么？一股遥远的意志似乎在等你做决定。"
  - 垂死传承："一缕残存的神识在挣扎着，它等你很久了。"
  - 天道启示（通灵+）："你感知到一股难以名状的审视——不是警告，更像是...考核。"
- [ ] ≥ 8 单测：模板渲染 / 参数边界（null / 超长字符串截断）/ Redis 响应消费 → Arbiter event 注入

---

## §4 P3：标准面板模板

### 秘境发现面板（活坍缩渊）

```xml
<flow-layout direction="vertical" gap="4">
  <label style="color:#C8A060;font=bold">【活坍缩渊】{{ zone_name }}</label>
  <label style="color:#888888">残留灵压：{{ spirit_qi_display }} ／ 危险等级：{{ danger_tier }}</label>
  <label>{{ agent_narrative }}</label>
  <flow-layout direction="horizontal" gap="8">
    <button id="enter_realm" style="color:#FFD700">踏入探寻</button>
    <button id="observe_only" style="color:#AAAAAA">神识探查（不入）</button>
    <button id="dismiss">离开</button>
  </flow-layout>
</flow-layout>
```

### 垂死大能传承面板（配合 plan-dying-elder-v1）

```xml
<flow-layout direction="vertical" gap="4">
  <label style="color:#FF6666;font=bold">{{ elder_title }} 的残余神识</label>
  <label style="color:#CCCCCC">{{ elder_narration }}</label>
  <flow-layout direction="horizontal" gap="8">
    <button id="accept_legacy">接受传承（消耗 {{ qi_cost }} 真元）</button>
    <button id="ask_question_0">{{ question_0 }}</button>
    <button id="dismiss">离开</button>
  </flow-layout>
</flow-layout>
```

### 天道启示面板（通灵+境界专属）

```xml
<flow-layout direction="vertical" gap="6">
  <label style="color:#8888FF;font=bold">天意</label>
  <label style="color:#CCCCCC">{{ tiandao_message }}</label>
  <label style="color:#666666;font=italic">（此感应无需回应——天道不等你）</label>
  <button id="dismiss" style="color:#AAAAAA">闭目冥思</button>
</flow-layout>
```

---

## §5 视听规格

**面板出现动画**（client side, `AgentUiScreen.init()`）：
- 粒子底噪：`BongSpriteParticle`，数量 8-12，lifetime 40t，颜色 `#4A2A2A`（暗红灵气底色），从面板四角向内聚拢，spawn 模式 burst
- 音效：`ambient.soul_speed_loop`（vanilla），pitch 0.6，volume 0.3，delay 0t（面板打开时）
- fade-in：面板整体 opacity 0→1，duration 10t，easing linear

**天道启示面板特有**：
- HUD vignette overlay：颜色 `#1A0D40`（暗蓝），opacity 0.4，duration 200t，fade in 20t / fade out 40t
- 屏幕轻微 shake：amplitude 0.5px，frequency 0.1Hz，duration 40t（出现时）

---

## §8 开放问题（P0 决策门前需收口）

1. **OwoUI 动态 XML 能力边界**：`OwoUIAdapter.createFromXML` 是否支持运行时传入字符串还是只支持静态 resource 路径？需 Explore agent 查阅 owo-lib 0.11.2+1.20 源码或 README，确认是否需要 fork/patch。
2. **模板 vs 完全 LLM 生成**：让 LLM 输出完整 XML 有幻觉风险（非法标签、格式错误）；纯模板库限制了灵活性。决议前需统计当前 plan 所需面板类型数量，如果 ≤ 10 类型，模板库更安全。
3. **多语言 / 字体**：OwoUI XML 的文字是否走 Minecraft 的 i18n key 还是直接内嵌中文字符串？内嵌中文在 MC 1.20.1 资源包里可能有字体覆盖问题。
4. **境界门控强度**：醒灵/引气看"模糊版"是在 agent 端控制（生成不同 XML）还是 server 端根据 `player.realm` 动态修改 XML？建议 agent 端，server 不解析 XML 语义。
