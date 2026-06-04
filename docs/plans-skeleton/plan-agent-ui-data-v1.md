# Bong · plan-agent-ui-data-v1 · 骨架

**UI-as-Data：天道驱动的动态交互面板**——天道 Agent 根据当前世界状态生成 OwoUI XML 布局，经 Redis IPC `bong:agent_ui_cmd` → Valence server 验证/门控 → `bong:agent_ui_request` CustomPayload 下发给指定玩家，Fabric 微端用 OwoUIAdapter 实时渲染动态面板；玩家的交互（按钮点击 / 关闭）经 `bong:agent_ui_response` CustomPayload 回传 server，再经 `bong:agent_ui_response` Redis channel 回传 Agent 驱动后续推演。适用场景：活坍缩渊秘境发现界面、垂死大能最后传承对话、天道"气运审判"启示面板（通灵+专属）。

**来源**：`docs/scribble.md` §"结合你的 Agent 架构的最佳实践" → "大模型推演 UI... OwoUIAdapter.createFromXML"

**交叉引用**：
- `plan-agent-v2.md` ✅ — Agent 三 Agent 并发 + Arbiter 已实装，本 plan 在此基础上扩展 UI 下发管道
- `plan-ipc-schema-v1.md` ✅ — TypeBox schema source of truth，新增三个 schema：`AgentUiRequestCommandV1` / `AgentUiRequestPayloadV1` / `AgentUiResponsePayloadV1`
- `plan-client.md` ✅ — Fabric CustomPayload 框架 + HudRenderLayer 基础设施
- `plan-dying-elder-v1.md` ⬜ skeleton — 垂死大能对话树是本 plan 最高优先场景；传承按钮点击触发的 `QiTransfer` 由 `plan-dying-elder-v1` 的 `legacy_accept_system` 处理，本 plan UI 只展示耗费数值
- `plan-daozhan-v1.md` ⬜ skeleton — 道伥遭遇界面可复用本 plan 渲染管道
- `plan-tsy-zone-v1.md` ✅ — 活坍缩渊秘境是本 plan 核心 use case 之一
- `plan-qi-physics-v1.md` ✅ — 传承/耗费真元场景的 QiTransfer 守恒由下游 plan 实现，本 plan 不产生任何 QiTransfer

**worldview 锚点**：
- **§八 天道行为准则**："天道利用玩家之间的贪欲来平衡"、"暗示某个修士气运将尽"——天道的 UI 绝不是客服弹窗，而是间接的命运提示
- **§五 境界感知**：通灵境玩家"能感知天道注意力（危机预警）"——**天道启示类**面板（`realm_gate=5`）服务端强制仅限通灵+，低境界玩家收到 realm_gate 拒绝事件；低境界看到的"模糊感应"是 Agent 生成不同 XML（UX），不是安全机制
- **§十六 活坍缩渊（秘境）**：秘境发现/进入许可面板；realm_gate=3（凝脉境）

**qi_physics 锚点**：
- 本 plan **不涉及任何 QiTransfer**；面板内展示的灵气浓度 / 真元耗费均为只读显示值
- "接受传承"按钮点击后，server 向 `plan-dying-elder-v1` 的 `legacy_accept_system` 转发事件，由该 system 发起 `QiTransfer { from: player_qi_pool, to: legacy_sink, amount }`；本 plan 的 `AgentUiSession` 达到 Completed 终态后即 compare-and-remove，不持有任何 qi 数据
- **任何 QiTransfer 均在本 plan 范围之外**

**前置依赖**：
- `plan-agent-v2.md` ✅ — Agent 推演能力 + Redis IPC
- `plan-client.md` ✅ — CustomPayload 双向管道
- `plan-ipc-schema-v1.md` ✅ — schema 注册流程

---

## §0 设计约束（技术边界）

1. **realm_gate 服务端强制**：`AgentUiRequestCommandV1.realm_gate` 由 Agent 按面板类型设置（天道启示=5，秘境发现=3，一般面板=0）；server 收到请求后：
   - 若 `target_player` 对应的 ECS Entity 不存在（玩家离线）→ 立即拒绝，向 `bong:agent_ui_response` Redis 发布 `{ error: "player_offline", request_id }`，不创建 session
   - 若玩家在线但 `player.realm < realm_gate` → 拒绝，向 `bong:agent_ui_response` Redis 发布 `{ type: "realm_gate_rejected", request_id, player_realm, required_realm }`
   - 两种情况 realm_gate 字段均不下发给 client，防止客户端感知门控阈值

2. **XML 安全（全部在 server sanitize 阶段执行）**：
   - 白名单标签：`<label>` / `<button>` / `<flow-layout>` / `<grid-layout>` / `<texture>`；其余标签 strip（宽容解析，不整体拒绝）
   - DTD / 外部实体：检测到 `<!DOCTYPE` 或 `<!ENTITY` → 拒绝整个 payload，返回错误
   - 大小上限：8192 字节（超出拒绝，不截断）
   - 深度上限：6 层（超出 strip 至第 6 层）
   - 节点数上限：64 节点（超出拒绝）
   - Agent 模板参数插值前必须 XML-escape（`&→&amp;`, `<→&lt;`, `>→&gt;`, `"→&quot;`, `'→&apos;`）；server sanitize 时再次验证无裸 `<` / `&`

3. **动作白名单**：`AgentUiRequestCommandV1.allowed_button_ids: string[]`（Agent 提供，最多 16 条）；server `AgentUiSession` 存储；client 发 `button_click` 时，server 校验 `params["button_id"] ∈ allowed_button_ids`，不在则：
   - session 进入 `Error` 终态
   - 向 Agent 发布 `bong:agent_ui_response` Redis `{ action: "error", params: { "reason": "invalid_button_id" }, request_id }`
   - **同时**向 client 发送 `#[close_channel]`（见 §8 Q4）信号，携带 `{ request_id, reason: "invalid_button_id" }`，client 据此关闭面板并显示"天道拒绝了这次操作"提示
   - allowed_button_ids 不下发给 client，防止客户端感知白名单

4. **Session 状态机**：

   ```text
   Open → Completed   (client button_click，server 校验通过)
   Open → Dismissed   (client ESC，client 主动发 dismissed)
   Open → TimedOut    (server ticker：elapsed_ticks ≥ timeout_ticks)
   Open → Replaced    (同一玩家新 request 到达，旧 session 被替换)
   Open → Error       (allowed_button_ids 校验失败 / 响应格式非法 / client parse_error 回传)

   终态（Completed/Dismissed/TimedOut/Replaced/Error）：
   - compare-and-remove：HashMap::remove(player_id) 幂等
   - 重复终态事件静默丢弃（request_id 已不存在于 HashMap）
   - 所有终态均向 bong:agent_ui_response Redis 发布对应事件
   - Redis 发布失败：重试 3 次（50ms/100ms/200ms backoff），全部失败则记录错误日志并保留 TimedOut 终态（防止 agent 等待孤儿 session）
   - PlayerDisconnect：server 监听 `PlayerDisconnect` event → 若该玩家有 Open session → 转为 Dismissed 终态 + Redis 发布 dismissed event（清理孤儿 session）
   ```

5. **无状态 XML**：XML 不允许嵌入脚本或持久 state——所有交互状态在 Agent 侧管理，客户端只负责渲染和收发

6. **单面板互斥**：同一玩家同一时刻最多一个 `agent_ui` 面板（以 player_id 为 key 存储 session）；新请求自动将旧 session 转为 Replaced 终态并通知 client 关闭

7. **本 plan 不产生 QiTransfer**：传承耗费真元等下游逻辑委托给 `plan-dying-elder-v1`；UI 模板内展示的真元数值为显示用途，server 不在本 plan 的 system 内触发 QiTransfer

---

## 接入面 Checklist

- **进料**：
  - Agent Arbiter 输出的世界推演结果（已有 `WorldModel` Redis 持久化）
  - `player.realm`（server ECS query，用于 realm_gate 校验）
  - `zone.spirit_qi` / `player.tiandao_attention`（server ECS query，只读展示数值）
  - OwoUI XML 规范（`client/resourcepack/owo-ui/` 已有若干静态 layout，本 plan 扩展为动态生成）
  - `bong:agent_ui_cmd` Redis channel（agent 发布 `AgentUiRequestCommandV1`，server 订阅）
- **出料**：
  - `bong:agent_ui_request` CustomPayload（server → client）：`AgentUiRequestPayloadV1 { request_id, target_player, xml, timeout_ticks }`（不含 realm_gate / allowed_button_ids）
  - `bong:agent_ui_response` CustomPayload（client → server）：`AgentUiResponsePayloadV1 { request_id, action: literal union, params }`
  - `bong:agent_ui_response` Redis channel（server 发布，agent 订阅）：同上结构 + `server_timestamp`
  - server-side `AgentUiSessionStore` Resource（`HashMap<PlayerId, AgentUiSession>`，按玩家索引）
  - agent-side `uiResponseConsumer.ts` 消费响应并注入 Arbiter 下一轮推演
- **共享类型**（三个独立 schema，TypeBox + Rust serde，字段不得混用）：

  | schema | 用途 | 含 realm_gate | 含 allowed_button_ids |
  |--------|------|:---:|:---:|
  | `AgentUiRequestCommandV1` | agent → server（Redis） | ✅ | ✅ |
  | `AgentUiRequestPayloadV1` | server → client（CustomPayload） | ❌ | ❌ |
  | `AgentUiResponsePayloadV1` | client → server（CustomPayload）/ server → agent（Redis） | ❌ | ❌ |

- **跨仓库契约**：

  | 层 | symbol |
  |----|--------|
  | agent | 发布 `bong:agent_ui_cmd` Redis channel（`AgentUiRequestCommandV1`） |
  | server | `network/agent_ui.rs`：`AgentUiSessionStore` + realm_gate 校验 + XML sanitize + allowed_button_ids 校验 + Session 状态机 |
  | server | 发布 `bong:agent_ui_response` Redis channel（agent 消费） |
  | client | `AgentUiPayloadHandler.java`：CustomPayload 解析 + OwoUIAdapter.createFromXML |
  | client | `AgentUiScreen.java`：动态 OwoUI 屏幕，按钮 ID → 发 `bong:agent_ui_response` CustomPayload |
  | agent | `packages/tiandao/src/ui/`：XML 模板生成 + Redis 响应消费 |
  | schema | 三个 schema TypeBox + JSON sample（各自独立文件） |

---

## 阶段总览

| 阶段 | 内容 | 状态 | 验收标准 |
|------|------|------|---------|
| **P0** | schema 定稿 + agent→server Redis IPC + server→client CustomPayload 链路 | ⬜ | 单测：schema roundtrip（三个）/ realm_gate 拒绝低境界 / DTD 拒绝 / 非法标签 strip / 单面板互斥 + compare-and-remove 幂等 / allowed_button_ids 校验 / timeout TimedOut 终态 / XML 超限拒绝 |
| **P1** | client OwoUI XML 动态渲染 + 按钮响应 → `bong:agent_ui_response` CustomPayload | ⬜ | 集成测试：server 下发含 2 个按钮的 XML → client 渲染 → 点击按钮 → server 收到正确 action；ESC → dismissed；Replaced 信号 → 关闭不发包 |
| **P2** | agent 侧 XML 生成（xmlEscape）+ Redis `bong:agent_ui_response` 消费 | ⬜ | mock 推演：agent 生成活坍缩渊面板 → 玩家点击"进入" → agent 收到 Completed → emit `agent_cmd` 解锁秘境；realm_gate_rejected → emit narration 替代面板 |
| **P3** | 3 种标准面板模板（秘境发现 / 垂死传承 / 天道启示）+ 视听规格 | ⬜ | 每种模板 XML 通过 OwoUI 渲染测试；通灵-以下收天道启示被拒绝；传承面板不触发 QiTransfer（由 plan-dying-elder-v1 处理）；timeout dismiss 闭环 |

---

## §1 P0：Schema + agent→server IPC + CustomPayload 链路

### Schema 定义

```typescript
// agent/packages/schema/src/payloads/agent_ui.ts

// Agent → Server（Redis bong:agent_ui_cmd）
const AgentUiRequestCommandV1 = Type.Object({
  request_id: Type.String({ format: 'uuid' }),
  target_player: Type.String({ format: 'uuid' }),
  xml: Type.String({ maxLength: 8192 }),
  timeout_ticks: Type.Integer({ minimum: 20, maximum: 2400, default: 600 }),
  realm_gate: Type.Integer({ minimum: 0, maximum: 5, default: 0 }),
  allowed_button_ids: Type.Array(Type.String(), { maxItems: 16 }),
})

// Server → Client（CustomPayload bong:agent_ui_request）
// realm_gate / allowed_button_ids 不下发给 client
const AgentUiRequestPayloadV1 = Type.Object({
  request_id: Type.String({ format: 'uuid' }),
  target_player: Type.String({ format: 'uuid' }),
  xml: Type.String({ maxLength: 8192 }),
  timeout_ticks: Type.Integer({ minimum: 20, maximum: 2400 }),
})

// Client → Server CustomPayload + Server → Agent Redis（同一 schema）
const AgentUiActionType = Type.Union([
  Type.Literal('button_click'),
  Type.Literal('dismissed'),
  Type.Literal('timeout'),
  Type.Literal('replaced'),
  Type.Literal('error'),
  Type.Literal('parse_error'),  // client OwoUI 解析 XML 失败时回传
])
const AgentUiResponsePayloadV1 = Type.Object({
  request_id: Type.String({ format: 'uuid' }),
  action: AgentUiActionType,
  params: Type.Record(Type.String(), Type.String()),
})
```

### 交付物

- [ ] TypeBox 三个 schema（`agent/packages/schema/src/payloads/agent_ui.ts`）+ JSON samples（`samples/agent_ui_request_command.json` / `agent_ui_request_payload.json` / `agent_ui_response_button_click.json` / `agent_ui_response_dismissed.json`）
- [ ] `server/src/network/agent_ui.rs`：
  - `AgentUiSessionState` enum：`Open / Completed / Dismissed / TimedOut / Replaced / Error`
  - `AgentUiSession { request_id, player_id, allowed_button_ids, timeout_ticks, elapsed_ticks, state }`
  - `AgentUiSessionStore` Resource（`HashMap<PlayerId, AgentUiSession>`，按玩家索引）
  - `receive_agent_ui_cmd_system`：订阅 `bong:agent_ui_cmd` → realm_gate 校验 → XML sanitize → 若旧 session Open → Replaced 终态 + 发 close CustomPayload → 创建新 Open session → 推送 `bong:agent_ui_request` CustomPayload
  - `agent_ui_tick_system`：每 tick 推进 `elapsed_ticks`，超时 → TimedOut 终态 + 发 close CustomPayload + 发布 Redis 响应
  - `receive_agent_ui_response_system`：接收 client CustomPayload → 校验 `request_id + player binding + allowed_button_ids` → 更新 session 终态 → compare-and-remove → 发布 `bong:agent_ui_response` Redis
  - `xml_sanitize(xml: &str) -> Result<String, XmlSanitizeError>`：DTD 拒绝 / 标签白名单 strip / 深度限制 / 节点数限制 / 大小限制
  - `receive_player_disconnect_system`：监听 `PlayerDisconnect` event → 若该玩家有 Open session → Dismissed 终态 + Redis 发布 dismissed event（清理孤儿 session）
  - AgentUiSessionStore 并发安全：Bevy `ResMut<AgentUiSessionStore>` 由 ECS 调度器在 system 内保证互斥访问；三个 system（receive_cmd / tick / receive_response）通过 `SystemSet` 顺序约束（`receive_cmd.before(tick).before(receive_response)`），无需额外 Mutex
  - Redis 发布失败重试：终态事件发布失败时，重试 3 次（50ms/100ms/200ms backoff）；全部失败则记录错误日志，session 已处于终态不阻塞新请求
- [ ] ≥ 15 单测：
  - schema roundtrip（三个 schema，正样本 + 缺字段 / 超限负样本）
  - realm_gate=5 拒绝 realm=4（通灵-）玩家 → Redis error event
  - realm_gate=0 允许所有境界
  - `<!DOCTYPE` 拒绝 → payload 整体拒绝
  - 非法标签 `<script>` strip → 正常返回剩余内容
  - XML 超 8192 字节 → 拒绝
  - 节点数超 64 → 拒绝
  - 单面板互斥：新 request 到达 → 旧 session Replaced 终态 + compare-and-remove
  - compare-and-remove 幂等：重复 dismissed 事件 → 第二次静默丢弃
  - allowed_button_ids 校验：非法 button_id → Error 终态 + Redis error event
  - timeout → TimedOut 终态 → Redis `{ action: "timeout" }` 发布
  - allowed_button_ids 最多 16 条（第 17 条 → 拒绝 command）
  - PlayerDisconnect → Open session → Dismissed 终态 + Redis dismissed
  - client parse_error 回传 → Error 终态 + Redis `{ action: "parse_error" }` 发布
  - Redis 发布失败 3 次重试后日志记录，不阻塞下一 session

---

## §2 P1：Client OwoUI 动态渲染

- [ ] `AgentUiPayloadHandler.java`：监听 `bong:agent_ui_request` CustomPayload → parse `AgentUiRequestPayloadV1` → 触发 `AgentUiScreen.open(requestId, xml, timeoutTicks)`
- [ ] `AgentUiScreen.java`（extends `BaseOwoScreen`）：从 payload xml 调用 `OwoUIAdapter.createFromXML` 构建组件树；按钮 ID 约定：`<button id="enter_realm">踏入探寻</button>` → 点击触发 `sendAgentUiResponse(requestId, "button_click", Map.of("button_id", "enter_realm"))`
- [ ] 颜色样式：`<label style="color: #C8A060">` 支持 16 进制颜色（通过 OwoUI theme 机制）
- [ ] 面板背景：`BongSpriteParticle` vignette + 半透明黑底 `#1A0D0D`
- [ ] 倒计时：client 维护本地倒计时（`timeout_ticks`），到期后自动关闭面板并发 `dismissed`（server 为 timeout 权威来源，client 不发 timeout，server ticker 负责 TimedOut 终态）
- [ ] Replaced / Error close 信号（server → client，channel 格式待 §8 Q4 P0 决策，暂记为 `#[close_channel] { request_id, reason?: String }`）：client 收到后关闭当前面板；reason 为空表示 Replaced（不发 response），reason="invalid_button_id" 等表示 Error（显示"天道拒绝了这次操作"提示）；不发任何 bong:agent_ui_response（server 已在终态时完成 Redis 发布）
- [ ] **parse_error 降级**：`OwoUIAdapter.createFromXML` 抛出异常时 → 关闭动态面板 → 显示静态 fallback 面板（"天道信号紊乱，法则碎片无法解析"+ error_code）→ 发 `parse_error` action；server 收到 parse_error → Error 终态 → Redis 发布 parse_error event → Agent 可选简化模板重试（最多 1 次）
- [ ] ≥ 12 单测：各合法标签 parse / 非法标签被过滤 / 按钮点击发包正确 action+params / ESC 发 dismissed / 倒计时到期发 dismissed（不发 timeout）/ Replaced 信号 → 关闭不发包 / malformed XML → parse_error 回传 / 空 XML → parse_error / fallback 面板静态内容正确

---

## §3 P2：Agent 侧 XML 生成 + 响应消费

- [ ] `agent/packages/tiandao/src/ui/xmlTemplates.ts`：模板字符串 + `xmlEscape(s: string): string`（转义 `& < > " '`，null/undefined 输入返回空字符串）+ 参数插值（所有 `{{ param }}` 替换点必须先经 `xmlEscape`）
- [ ] `agent/packages/tiandao/src/ui/uiRenderer.ts`：根据 world state + player.realm 选择模板 + 填充；按面板类型设定 `realm_gate`（天道启示=5，秘境=3，一般=0）；player.realm < realm_gate 时生成"模糊感应版"XML（纯文字，无 button，供低境界玩家体验降级渲染，server 仍独立验证 realm_gate）；发布 `AgentUiRequestCommandV1` 到 `bong:agent_ui_cmd`
- [ ] `agent/packages/tiandao/src/ui/uiResponseConsumer.ts`：Redis `bong:agent_ui_response` 订阅；消费 Completed（button_click）→ 注入 Arbiter 下一轮推演；消费 realm_gate_rejected → emit narration 替代面板；消费 TimedOut / Dismissed → 标记当前面板上下文结束
- [ ] Agent 不直接向 LLM 请求 XML 字符串（幻觉风险）；LLM 只决策"选哪个模板 + 填什么参数值"；模板库负责结构安全
- [ ] **Agent 数据获取路径**：`player.realm` 和 `zone.spirit_qi` 等数据从 WorldModel Redis 中读取（`plan-agent-v2` 已实装 `player:{uuid}` / `zone:{id}` hash keys，每 tick 由 server 写入）；agent 在生成 XML 前 `await worldModel.getPlayer(targetPlayer)` 获取最新境界，无需单独 IPC 请求
- [ ] **模板参数默认值**：所有 `{{ param }}` 插值若对应值为 null/undefined → 替换为 `"?"`（防止原样输出 `{{ }}`）；必填参数（`zone_name`, `elder_title`, `tiandao_message`）若缺失 → `uiRenderer.ts` 抛出 Error，禁止下发含缺失必填字段的 XML；可选参数（`question_0`）缺失 → 整行 `<button>` 省略
- [ ] ≥ 10 单测：xmlEscape 边界（null / 含 `<script>` / 超长字符串截断）/ 模板渲染正确 XML-escape 参数 / player.realm < realm_gate → 生成模糊版 + realm_gate 仍正确 / Redis 响应消费 → Arbiter event 注入 / realm_gate_rejected → narration fallback / 必填参数缺失 → 抛出 Error / 可选参数缺失 → 对应 button 省略 / parse_error event → Agent 简化模板重试（最多 1 次）/ Redis 消费失败 → session 标记"状态未知"并记录日志

---

## §4 P3：标准面板模板

### 秘境发现面板（活坍缩渊，realm_gate=3 凝脉境）

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

`allowed_button_ids: ["enter_realm", "observe_only", "dismiss"]`，realm_gate=3

### 垂死大能传承面板（配合 plan-dying-elder-v1，realm_gate=3 凝脉境）

**注意**：`{{ qi_cost }}` 为只读展示数值；`accept_legacy` 按钮触发后，server 向 `plan-dying-elder-v1` 的 `legacy_accept_system` 转发事件，由该 system 发起 `QiTransfer { from: player_qi_pool, to: legacy_sink, amount: qi_cost }`。本 plan 自身不发起任何 QiTransfer，AgentUiSession 在 Completed 终态后即 compare-and-remove。

```xml
<flow-layout direction="vertical" gap="4">
  <label style="color:#FF6666;font=bold">{{ elder_title }} 的残余神识</label>
  <label style="color:#CCCCCC">{{ elder_narration }}</label>
  <label style="color:#888888">接受传承需消耗 {{ qi_cost }} 真元（由天道见证，不可撤回）</label>
  <flow-layout direction="horizontal" gap="8">
    <button id="accept_legacy">接受传承</button>
    <button id="ask_question_0">{{ question_0 }}</button>
    <button id="dismiss">离开</button>
  </flow-layout>
</flow-layout>
```

`allowed_button_ids: ["accept_legacy", "ask_question_0", "dismiss"]`，realm_gate=3

### 天道启示面板（通灵+境界专属，realm_gate=5）

```xml
<flow-layout direction="vertical" gap="6">
  <label style="color:#8888FF;font=bold">天意</label>
  <label style="color:#CCCCCC">{{ tiandao_message }}</label>
  <label style="color:#666666;font=italic">（此感应无需回应——天道不等你）</label>
  <button id="dismiss" style="color:#AAAAAA">闭目冥思</button>
</flow-layout>
```

`allowed_button_ids: ["dismiss"]`，realm_gate=5（通灵境）；realm < 5 玩家收到 realm_gate_rejected event，Agent 降级发 narration

---

## §5 视听规格

**面板出现动画**（client side，`AgentUiScreen.init()`）：
- 粒子底噪：`BongSpriteParticle`，数量 8-12，lifetime 40t，颜色 `#4A2A2A`（暗红灵气底色），从面板四角向内聚拢，spawn 模式 burst
- 音效：`ambient.soul_speed_loop`（vanilla），pitch 0.6，volume 0.3，delay 0t
- fade-in：面板整体 opacity 0→1，duration 10t，easing linear

**天道启示面板特有**：
- HUD vignette overlay：颜色 `#1A0D40`（暗蓝），opacity 0.4，duration 200t，fade in 20t / fade out 40t
- 屏幕轻微 shake：amplitude 0.5px，frequency 0.1Hz，duration 40t

---

## §8 开放问题（P0 决策门前需收口）

1. **OwoUI 动态 XML 能力边界**：`OwoUIAdapter.createFromXML` 是否支持运行时传入字符串还是只支持静态 resource 路径？需 Explore agent 查阅 owo-lib 0.11.2+1.20 源码或 README，确认是否需要 fork/patch。如需 fork，计入本 plan P1 前置。
2. **多语言 / 字体**：OwoUI XML 内嵌中文字符串在 MC 1.20.1 资源包里可能有字体覆盖问题；建议 P1 阶段用资源包内嵌思源宋体 subset，或走 i18n key（`lang/zh_cn.json`）避免硬编码。P0 阶段暂用内嵌字符串，记为 tech debt。
3. **realm_gate_rejected UX**：当 server 拒绝 agent_ui_request（realm_gate 不满足）时，`uiResponseConsumer.ts` 消费 `realm_gate_rejected` event → emit 一条 narration 替代面板（v1 推荐）。是否还需要 client 侧提示（"你感知到一丝天意，但无法触及"）？v1 先只走 narration，client 无额外提示。
4. **`#[close_channel]` CustomPayload 格式（P0 必决）**：server 向 client 发 Replaced / Error 关闭信号的 channel 形式，候选方案：
   - A. 独立 channel `bong:agent_ui_close { request_id, reason?: String }`（推荐，职责清晰）
   - B. 复用 `bong:agent_ui_request` 带特殊 xml=""（节省注册一个 channel，但容易混淆）
   **P1 阶段所有 `#[close_channel]` 占位符必须替换为此处 P0 选定的方案**；在 P0 决策未落地前，§2 P1 均以 `#[close_channel]` 表示，实施时依决策替换。
