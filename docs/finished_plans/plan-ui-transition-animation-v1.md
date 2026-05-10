# Bong · plan-ui-transition-animation-v1 · 已完成

UI 过渡动画与屏幕切换流畅性——在 `plan-hud-polish-v1`（HUD 动画 polish）+ `plan-entity-model-v1`（实体模型/screen）+ `plan-item-visual-v1`（物品视觉）✅ active 基础上拓展。hud-polish-v1 P2 已做 HUD 元素级动画（toast slide-in / 进度条 smooth lerp / 状态条 flash / 事件流 typewriter）——但这些是 **HUD 组件内部动画**，不涉及全屏 screen 切换。当前打开背包/锻造台/丹炉/inspect/cultivation screen 是瞬间切换——缺乏过渡让每步操作有**体感重量**。本 plan 定义一套 UI 过渡语言 + 给关键屏幕切换加上动画 + loading 状态可视化 + Redis 连接状态指示。不碰 UI 内容/功能——纯过渡层。

**世界观锚点**：`worldview.md §八` 天道运维博弈——灵物操作磨损（取物品有代价）→ 打开背包应该有"磨损"的微沉重感（非负反馈，是体感）· `§九` 交易不安全 → 交易 UI 打开/关闭应比普通背包更"锐利" · `§十` 资源匮乏 → UI 不应华丽花哨，过渡应简洁克制

**library 锚点**：无直接引用——纯 UI/UX 工程

**前置依赖**：
- `plan-hud-polish-v1` 🆕 active → **HUD 动画基座**（toast slide / progress lerp / state bar flash 的 animation utility 可复用。本 plan 做 screen 级过渡，不做 HUD 组件级动画）
- `plan-entity-model-v1` 🆕 active → 实体模型 screen（锻造台/丹炉/灵龛 screen 需等其 BlockEntity 渲染完成后才有意义做过渡）
- `plan-item-visual-v1` 🆕 active → 物品视觉体系（inspect 过渡配合物品图标/稀有度 glow 才有完整体验）
- `plan-HUD-v1` ✅ → `BongHudOrchestrator`（过渡层可插入）
- `plan-inventory-v2` ✅ → 背包 screen
- `plan-cultivation-v1` ✅ → CultivationScreen
- `plan-input-binding-v1` ✅ → keybind 触发 screen
- `plan-npc-engagement-v1` 🆕 active → NPC 交互 screen（NpcDialogueScreen / NpcTradeScreen / NpcInspectScreen）

**反向被依赖**：所有 screen 类 plan 可调用过渡 API（`ScreenTransition.open(screen, animation)`）

---

## 与 hud-polish-v1 的边界

| 维度 | hud-polish-v1 已做 | 本 plan 拓展 |
|------|-------------------|-------------|
| toast 动画 | slide-in from right 0.2s + slide-out 0.3s | 不碰 |
| 进度条 | smooth lerp 填充 | 不碰 |
| 状态条变化 | 增减 flash（绿/红 0.3s）| 不碰 |
| 事件流 | typewriter 效果（每字 30ms）| 不碰 |
| 全屏 screen 切换 | 无（瞬切）| 新增：4 种过渡动画 + 8 screen 映射 |
| loading 状态 | 无 | 新增：LoadingOverlay（跨 screen 异步加载）|
| 连接状态 | 无 | 新增：ConnectionStatusIndicator（Redis 绿/黄/红）|

---

## 接入面 Checklist

- **进料**：`MinecraftClient.setScreen()` 调用点 / `BongHudOrchestrator` / Redis IPC 连接状态（`IpcManager.isConnected()`）/ 各 screen 类（`InventoryScreen` / `ForgeScreen` / `AlchemyScreen` / `CultivationScreen` / `NpcDialogueScreen` / `NpcTradeScreen` / `ItemInspectScreen` / `DeathScreen`）
- **出料**：`ScreenTransition` 动画引擎（4 种动画 + timing + easing）+ `TransitionConfig`（per-screen 配置）+ `ScreenTransitionRegistry`（screen 类 → 默认动画映射）+ `LoadingOverlay`（跨 screen 异步加载过渡）+ `ConnectionStatusIndicator`（Redis 状态 HUD 小圆点）
- **跨仓库契约**：纯 client 侧——不涉及 server

---

## §0 设计轴心

- [ ] **克制**：修仙 UI 过渡不应花哨（无弹性 bounce、无闪光特效）—— slide 和 fade 为主
- [ ] **有重量**：不同 screen 的过渡速度不同——背包(沉)比 ESC menu(快)慢，暗示"灵物操作有代价"
- [ ] **过渡 ≠ 延迟**：过渡期间 screen 已在初始化（并行 preload），不做无意义等待
- [ ] **loading = 不可见就不焦虑**：跨 screen 异步数据加载时显示 `LoadingOverlay`（灰墨晕染 + "凝神中…"），超时 3s 后显示 retry
- [ ] **Redis 状态可见**：右下角小圆点——玩家应该知道 agent 是否在线
- [ ] **与 hud-polish-v1 动画 utility 复用**：easing function / alpha lerp / color flash 等底层工具复用，不重写

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|----|------|----|
| P0 | ScreenTransition 引擎 + ScreenTransitionRegistry + hook `setScreen()` | ✅ 2026-05-10 |
| P1 | 8 screen 过渡配置落实 | ✅ 2026-05-10 |
| P2 | LoadingOverlay + 异步 preload | ✅ 2026-05-10 |
| P3 | ConnectionStatusIndicator + 断线重连 UX | ✅ 2026-05-10 |
| P4 | NPC 交互 screen 过渡 + 特殊死亡/重生过渡 | ✅ 2026-05-10 |
| P5 | 性能 + 快速切换压测 + 低配 fallback | ✅ 2026-05-10 |

---

## P0 — ScreenTransition 引擎 + Registry ✅ 2026-05-10

### 交付物

1. **`ScreenTransition`**（`client/src/main/java/com/bong/client/ui/ScreenTransition.java`）
   - 4 种过渡动画枚举：
     - `SLIDE_UP`：新 screen 从底部上滑（y offset 从 screenHeight → 0，easing ease-out-cubic）
     - `FADE`：旧 screen alpha 1→0 + 新 screen alpha 0→1（交叉 fade）
     - `SCALE_UP`：新 screen 从中心点 0.8× scale 放大到 1.0×（+ alpha 0→1，easing ease-out-quad）
     - `NONE`：瞬切（fallback / debug）
   - 通用参数：`duration_ms`（100-600ms）/ `easing`（ease-out-cubic / ease-out-quad / linear）
   - API：`ScreenTransition.play(oldScreen, newScreen, type, duration, easing, callback)`

2. **`TransitionConfig`**（`client/src/main/java/com/bong/client/ui/TransitionConfig.java`）
   - per-screen 配置：`{ screen_class, open_transition, open_duration_ms, close_transition, close_duration_ms }`
   - open 和 close 可用不同动画（背包 open=SLIDE_UP / close=SLIDE_DOWN(reverse)）

3. **`ScreenTransitionRegistry`**（`client/src/main/java/com/bong/client/ui/ScreenTransitionRegistry.java`）
   - 8 screen → 默认 TransitionConfig 映射
   - `register(screenClass, config)` / `get(screenClass)` / `getOrDefault(screenClass, DEFAULT_FADE_200MS)`
   - 未注册 screen 默认 FADE 200ms（不阻塞新 screen 开发）

4. **hook `MinecraftClient.setScreen()`**
   - Mixin `ScreenSetMixin`：截获 `setScreen(screen)` 调用
   - 如果 registry 有配置 → 创建 `ScreenTransitionOverlay`（DrawContext 层，在旧 screen 上绘制过渡动画）→ 动画完成后 → 真正 setScreen
   - 如果 screen == null（关闭 screen）→ 使用 close transition
   - 快速连续调用保护：新 transition 取消正在播放的旧 transition（不 queue 无限过渡）

### 验收抓手

- 测试：`client::ui::tests::transition_slide_up_position` / `client::ui::tests::transition_fade_alpha` / `client::ui::tests::rapid_switch_cancels_previous` / `client::ui::tests::unregistered_screen_uses_default`
- 手动：打开背包 → 底部滑入 → 关闭 → 滑出 → 快速连按两次 → 不卡

---

## P1 — 8 screen 过渡配置 ✅ 2026-05-10

### 交付物

每个 screen 的过渡配置 + 设计意图：

| Screen | open | close | duration | 设计意图 |
|--------|------|-------|----------|---------|
| 背包 `InventoryScreen` | SLIDE_UP | SLIDE_DOWN | 300ms | 沉重——灵物操作有代价 |
| 锻造台 `ForgeScreen` | SCALE_UP | SCALE_DOWN | 400ms | 专注——从世界聚焦到工作台 |
| 丹炉 `AlchemyScreen` | SCALE_UP + 微 fog overlay | FADE | 400ms | 专注 + 蒸汽感（fog overlay alpha 0.05） |
| 修炼 `CultivationScreen` | FADE + vignette | FADE | 600ms | 最慢——进入内心世界（不急） |
| 物品 inspect `ItemInspectScreen` | SLIDE_RIGHT | SLIDE_LEFT | 200ms | 快——快速查看物品，不打断操作节奏 |
| ESC menu | FADE | FADE | 150ms | 最快——逃跑/暂停时秒开 |
| 死亡 screen | handled by death-cinematic | — | — | 由 death-rebirth-cinematic-v1 管 |
| NPC 对话 `NpcDialogueScreen` | FADE | FADE | 250ms | 中等——对话节奏不急不慢 |

注意：
- NPC 交易 `NpcTradeScreen`：从 `NpcDialogueScreen` 切换过来时用 SLIDE_RIGHT 200ms（从对话进入交易，视觉上"向右展开"）
- 灵龛重生 screen：由 death-rebirth-cinematic-v1 管理，不走本 plan 的过渡系统

### 验收抓手

- 测试：`client::ui::tests::inventory_uses_slide_up` / `client::ui::tests::esc_menu_fastest` / `client::ui::tests::cultivation_slowest`
- 手动：逐个打开 8 screen → 确认过渡类型 + 速度符合设计意图 → 背包沉重 / ESC 秒开

---

## P2 — LoadingOverlay + 异步 preload ✅ 2026-05-10

### 交付物

1. **`LoadingOverlay`**（`client/src/main/java/com/bong/client/ui/LoadingOverlay.java`）
   - 当 screen 切换需要等待 server 数据（如 forge session state / NPC trade inventory）时显示
   - 视觉：灰色半透明背景（alpha 0.7）+ 中央灰墨晕染动画（DrawContext 圆形 gradient 缓慢扩散 + 收缩 loop）+ 文字 "凝神中…"（墨色 #C0B090）
   - 超时 3s：文字变 "灵脉堵塞，稍后再试" + 追加 retry 按钮
   - 超时 10s：文字变 "天道失联" + 追加 "返回主世界" 按钮

2. **异步 preload 机制**
   - screen 切换前发起 server 数据请求（如 `ForgeSessionRequestC2s`）
   - 数据到达前显示 LoadingOverlay
   - 数据到达后 LoadingOverlay fade out 0.2s → 真正的 screen 出现（带过渡动画）
   - preload 与 transition 并行：transition 动画播放期间同时在等数据——如果数据先到，transition 结束后直接显示 screen（无 loading）

3. **loading 墨滴粒子**
   - LoadingOverlay 中心：墨滴粒子（DrawContext 小圆点从中心向外随机方向缓慢扩散 × 5，lifetime 60 tick，颜色 #2A2A2A alpha 0.3——极微妙的"墨在晕染"视觉）
   - 纯 DrawContext 实现（不走 BongSpriteParticle——loading 是 2D UI 层不是 3D 世界）

### 验收抓手

- 测试：`client::ui::tests::loading_overlay_shows_on_async` / `client::ui::tests::loading_timeout_3s_retry` / `client::ui::tests::preload_parallel_with_transition`
- 手动：打开锻造台 → server 响应慢 → 灰墨晕染 "凝神中…" → 数据到 → 锻造台 scale-up 出现 → 断网 → 3s → "灵脉堵塞"

---

## P3 — ConnectionStatusIndicator ✅ 2026-05-10

### 交付物

1. **`ConnectionStatusIndicator`**（`client/src/main/java/com/bong/client/ui/ConnectionStatusIndicator.java`）
   - 注册到 `BongHudOrchestrator`（HUD 组件）
   - 位置：屏幕右下角（快捷栏右侧下方），半透明小圆点 6×6px
   - 3 状态颜色：
     - 绿 #44AA44：Redis 连接正常 + agent 在线
     - 黄 #CCAA44：Redis 重连中 / agent 无响应 > 5s
     - 红 #AA4444：Redis 断开 > 10s
   - hover（crosshair 指向）：显示 tooltip "天道连接 · 延迟 42ms" / "天道失联 · 断开 15s"
   - alpha 0.4 常驻（不抢眼——只有出问题时才注意到）

2. **状态变化反馈**
   - 绿→黄：圆点颜色 lerp 1s（不突变）
   - 黄→红：圆点颜色 lerp 1s + toast "与天道失联"（使用 hud-polish-v1 toast 系统）
   - 红→绿（重连成功）：圆点颜色 lerp 0.5s + toast "天道重注"
   - 不做弹窗/阻断——连接状态是后台信息，不应打断操作

3. **数据源**
   - 消费 `IpcManager`（已有 Redis 连接管理）→ `isConnected()` / `getLatencyMs()` / `getDisconnectedDurationMs()`
   - 每 60 tick 检查一次（不需要 per-tick）

### 验收抓手

- 测试：`client::ui::tests::connection_indicator_green_on_connect` / `client::ui::tests::connection_indicator_yellow_on_delay` / `client::ui::tests::connection_indicator_red_on_disconnect` / `client::ui::tests::disconnect_toast_once`
- 手动：正常游戏 → 右下角绿点 → 断 Redis → 黄点 → 5s → 红点 + toast "与天道失联" → 重连 → 绿点 + toast "天道重注"

---

## P4 — NPC 交互 screen 过渡 + 死亡/重生 ✅ 2026-05-10

### 交付物

1. **NPC 交互 screen 链过渡**
   - 右键 NPC → `NpcDialogueScreen`（FADE 250ms）
   - 对话中选"交易" → `NpcTradeScreen`（SLIDE_RIGHT 200ms——视觉"展开"）
   - 对话中选"查看" → `NpcInspectScreen`（SLIDE_RIGHT 200ms）
   - 关闭任何 NPC screen → FADE 200ms 回到世界
   - NPC 交易过程中 NPC 走远 → 强制关闭 screen（FADE 100ms 快速）+ toast "对方离开了"

2. **特殊 screen 过渡**
   - 灵龛设置 `SpiritNicheScreen`：SCALE_UP 400ms（与锻造台同风格——专注操作）
   - 阵法核心 `FormationCoreScreen`：FADE 500ms + 微紫 tint overlay（暗示灵力操作）
   - 灵田管理（如果有 screen）：FADE 300ms

3. **与 death-rebirth-cinematic-v1 的协调**
   - 死亡 screen 过渡由 death-cinematic 管（本 plan 不介入）
   - 但：death-cinematic 结束后 rebirth 阶段 → 如果玩家在重生后立即打开背包 → 本 plan 的 SLIDE_UP 过渡正常工作（两系统独立，不冲突）
   - 死亡 cinematic 期间如果收到 screen 切换请求（不应该有，但防御性）→ queue 到 cinematic 结束后

4. **过渡期间输入锁定**
   - 过渡动画播放期间（100-600ms）：屏蔽键盘/鼠标输入（防止过渡中误操作）
   - 例外：ESC 键始终可用（过渡中按 ESC → 取消过渡 + 关闭 screen）

### 验收抓手

- 测试：`client::ui::tests::npc_dialogue_to_trade_slide` / `client::ui::tests::npc_walk_away_force_close` / `client::ui::tests::input_locked_during_transition` / `client::ui::tests::esc_cancels_transition`
- 手动：右键 NPC → 对话 fade in → 选交易 → 向右展开 → NPC 走远 → 强制关闭 → 死亡后重生 → 立即开背包 → 正常滑入

---

## P5 — 性能 + 压测 + 低配 fallback ✅ 2026-05-10

### 交付物

1. **性能要求**
   - 所有 transition ≤ 16ms GPU time per frame（不掉帧）
   - SCALE_UP 的 DrawContext scale 计算 < 0.5ms
   - LoadingOverlay 墨滴粒子 < 0.3ms

2. **快速切换压测**
   - 连续快速切换 10 screen（背包→关→背包→关...）→ transition queue 正确取消 → 不崩溃 / 不残留 overlay
   - 切换过程中收到 server packet → 不丢包 / 不 desync
   - screen 初始化抛异常 → LoadingOverlay 捕获 → 显示 "灵脉堵塞" → 不白屏

3. **低配 fallback**
   - 设置界面 `UI Transition` 开关：ON（默认）/ OFF
   - OFF：所有 transition duration = 0（瞬切）+ LoadingOverlay 简化（无墨滴粒子，纯文字）
   - 帧率 < 30fps 时自动建议关闭过渡（toast 一次性提示）

4. **分辨率适配**
   - SLIDE_UP offset 按 screenHeight 百分比（不是固定 px）
   - SCALE_UP center point 按 screen center
   - LoadingOverlay 居中

### 验收抓手

- 压测脚本：`scripts/ui_transition_stress.sh`（10 次快速切换 + 帧率日志）
- 低配测试：设置 OFF → 确认瞬切 + 无残留
- 断网 + 快速切换：不白屏不崩溃

---

## Finish Evidence

- **落地清单**：
  - P0：`client/src/main/java/com/bong/client/ui/ScreenTransition.java` / `TransitionConfig.java` / `ScreenTransitionRegistry.java` / `ScreenTransitionController.java` / `ScreenTransitionOverlay.java`；`client/src/main/java/com/bong/client/mixin/ScreenSetMixin.java` hook `MinecraftClient.setScreen()`；`MixinKeyboardSkillKeys.java` / `MixinMouse.java` / `MixinScreenInputLock.java` 锁定过渡期输入；`MixinScreenTransitionRender.java` 在 screen render 尾部绘制 overlay。
  - P1：`ScreenTransitionRegistry.bootstrapDefaults()` 注册 `InspectScreen` / `ForgeScreen` / `AlchemyScreen` / `CultivationScreen` / `ItemInspectScreen` / `GameMenuScreen` / `SparringInviteScreen` / `TradeOfferScreen` 等默认过渡，另覆盖 forge carrier / repair / zhenfa / lingtian / void action / identity / insight 等特殊 screen。vanilla `InventoryScreen` 仍由既有 `MixinMinecraftClient` 改路由到 Bong `InspectScreen`，背包 SLIDE_UP/DOWN 挂在实际展示的 `InspectScreen` 上。
  - P2：`client/src/main/java/com/bong/client/ui/LoadingOverlay.java` 提供 loading/retry/lost 三段状态、墨滴粒子、低配 fallback、preload 与 transition 并行判定。
  - P3：`client/src/main/java/com/bong/client/ui/ConnectionStatusIndicator.java` / `ClientConnectionStatusStore.java`；`BongNetworkHandler` 接入 JOIN/DISCONNECT 与 payload heartbeat；`BongHudOrchestrator` 输出右下角连接状态点；在线时从 `PlayerListEntry.getLatency()` 读取真实 MC network latency，未知时显示 `延迟 --`，不伪造常量；断线/重连 toast 复用 `BongToast`。
  - P4：NPC/交易链通过 `ScreenTransitionRegistry.registerChain(...)` 支持 dialogue→trade `SLIDE_RIGHT`；死亡 screen 配置为 `externalCinematic`/`NONE`，不抢 death cinematic；ESC 取消由 `MixinKeyboardSkillKeys`/`MixinScreenInputLock` 落地，鼠标输入锁定由 `MixinMouse` HEAD 拦截落地。
  - P5：`UiTransitionSettings.java` 提供 ON/OFF、低配 fallback、低 FPS 一次性建议；`scripts/ui_transition_stress.sh` 显式校验 JDK 17 并覆盖快速切换/overlay/loading/connection 的 focused test 入口；scale/slide 计算按分辨率比例。
- **关键 commit**：
  - `9382197a9`（2026-05-10）`feat(client): 增加 UI 过渡动画引擎`
  - `44100d48d`（2026-05-10）`feat(client): 增加 UI loading 过渡层`
  - `304431527`（2026-05-10）`feat(client): 接入天道连接状态指示`
  - `5ddf983ae`（2026-05-10）`fix(client): 收紧 UI 过渡 review 阻塞项`
  - `240597659`（2026-05-10）`fix(client): 收敛 UI transition 复审建议`
- **测试结果**：
  - `JAVA_HOME=/home/kiz/.sdkman/candidates/java/17.0.18-amzn ./gradlew test build`（client）✅ `BUILD SUCCESSFUL`
  - `JAVA_HOME=/home/kiz/.sdkman/candidates/java/17.0.18-amzn bash scripts/ui_transition_stress.sh` ✅ `BUILD SUCCESSFUL`
  - `JAVA_HOME=/home/kiz/.sdkman/candidates/java/17.0.18-amzn timeout 180s ./gradlew runClient`（client）✅ `BUILD SUCCESSFUL in 43s`；日志确认 `Bong Client bootstrap ready`、whale/fauna raw_id 对齐后正常进入资源加载与客户端运行态。
  - focused tests：`ScreenTransitionTest` / `LoadingOverlayTest` / `ConnectionStatusIndicatorTest` / `UiTransitionPerformanceTest` ✅
- **跨仓库核验**：纯 client 侧 plan；server / agent 无协议变更。client 命中 `ScreenTransition`、`ScreenTransitionRegistry`、`ScreenSetMixin`、`LoadingOverlay`、`ConnectionStatusIndicator`、`ClientConnectionStatusStore`、`BongHudOrchestrator`。
- **遗留 / 后续**：未来新 screen 只需 `ScreenTransitionRegistry.register(MyScreen.class, config)` 或 `registerChain(from, to, spec)`；本 plan 未新增 server 侧异步请求协议，只提供 client preload/overlay contract。
