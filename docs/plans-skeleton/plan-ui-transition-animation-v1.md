# Bong · plan-ui-transition-animation-v1 · 骨架

UI 过渡动画与屏幕切换流畅性。当前打开背包/锻造台/丹炉/inspect/cultivation screen 是瞬间切换——缺乏过渡让每步操作有**体感重量**。本 plan 定义一套 UI 过渡语言（slide/fade/scale）+ 给 8 个关键屏幕切换加上动画 + loading 状态可视化 + Redis 连接状态指示。不碰 UI 内容/功能——纯过渡层。

**世界观锚点**：`worldview.md §八` 天道运维博弈——灵物操作磨损（取物品有代价）→ 打开背包应该有"磨损"的微沉重感（非负反馈，是体感）· `§九` 交易不安全 → 交易 UI 打开/关闭应比普通背包更"锐利" · `§十` 资源匮乏 → UI 不应华丽花哨，过渡应简洁克制

**library 锚点**：无直接引用——纯 UI/UX 工程

**前置依赖**：
- `plan-HUD-v1` ✅ → `BongHudOrchestrator`（过渡层可插入）
- `plan-inventory-v2` ✅ → 背包 screen
- `plan-forge-leftovers-v1` ⏳ active → 锻造台 screen
- `plan-alchemy-client-v1` ⏳ active → 丹炉 screen
- `plan-cultivation-v1` ✅ → CultivationScreen
- `plan-input-binding-v1` ✅ → keybind 触发 screen

**反向被依赖**：所有 screen 类 plan 可调用过渡 API（`ScreenTransition.open(screen, animation)`）

---

## 接入面 Checklist

- **进料**：`MinecraftClient.setScreen()` 调用点（背包/锻造/丹炉/inspect/cultivation/死亡/respawn/esc menu）/ `BongHudOrchestrator` / Redis IPC 连接状态
- **出料**：`ScreenTransition` 动画引擎（4 种: slide-left / fade / scale-up / none）+ `TransitionConfig`（per-screen 配置）+ `LoadingOverlay`（跨 screen 异步加载时的过渡动画）+ `ConnectionStatusIndicator`（Redis 连接状态 HUD 小圆点: 绿/黄/红）+ `ScreenTransitionRegistry`（screen 类 → 默认过渡动画映射）
- **跨仓库契约**：纯 client 侧——不涉及 server

---

## §0 设计轴心

- [ ] **克制**：修仙 UI 过渡不应花哨（无弹性 bounce、无闪光特效）—— slide 和 fade 为主
- [ ] **有重量**：背包打开 = 沉重 slide-up（暗示"每次触摸灵物都有代价"）/ 锻造台 = scale-up 从中心展开（暗示"专注"）/ 死亡 = 全黑 fade 1.5s
- [ ] **loading = 不可见就不焦虑**：跨 screen 异步数据加载时显示 `LoadingOverlay`（灰墨晕染 + "凝神中…"）+ 超时 3s 后显示"灵脉堵塞，稍后再试"
- [ ] **Redis 状态可见**：右下角小圆点（绿 = 已连接 / 黄 = 重连中 / 红 = 断开）+ hover 显示延迟 ms

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|----|------|----|
| P0 | `ScreenTransition` 动画引擎（4 种 transition）+ `TransitionConfig` + `ScreenTransitionRegistry`（8 screen → 默认动画映射）+ MinecraftClient.setScreen() hook（截获 screen 切换 → 播过渡动画 → 再切 screen）| ⬜ |
| P1 | 8 screen 过渡落实：背包（slide-up 0.3s）/ 锻造台（scale-up 0.4s）/ 丹炉（scale-up + 微 fog 0.4s）/ CultivationScreen（fade + 微 vignette 0.6s）/ inspect（slide-right 0.2s）/ 死亡 screen（fade-to-black 1.5s）+ 灵龛重生（fade-from-black 3s）/ ESC menu（fade 0.15s，最快——逃跑时秒开） | ⬜ |
| P2 | `LoadingOverlay`：跨 screen 异步加载时（如从背包切到锻造台需等 forge session state）→ 中间显示灰墨晕染 overlay + "凝神中…"文字 + 微粒子（墨滴下落）+ 超时 3s 显示"灵脉堵塞，稍后再试" retry button | ⬜ |
| P3 | `ConnectionStatusIndicator`：Redis 连接状态圆点（右下角，半透明，hover 显示 ms 延迟）+ 断开时 toast "与天道失联" + 重连成功 toast "天道重注" + 状态变化过渡动画（颜色渐变 1s，不 blink） | ⬜ |
| P4 | 过渡性能：所有 transition 必须 ≤ 16ms GPU time（60fps）+ preload 下一 screen 资源（异步）+ 低配 fallback（transition duration = 0 即时切换） | ⬜ |
| P5 | 饱和化测试：连续快速切换 10 screen（验证 transition queue 不崩溃）+ 低配 30fps 测试 + 断网/重连时 transition 不卡死 | ⬜ |

---

## Finish Evidence（待填）

- **落地清单**：`ScreenTransition` 引擎 / `TransitionConfig` / 8 screen 过渡 / `LoadingOverlay` / `ConnectionStatusIndicator`
- **关键 commit**：P0-P5 各自 hash
- **遗留 / 后续**：未来新 screen 只需加一行 registry map
