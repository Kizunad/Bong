# Plan: HUD Polish v1（HUD/UI 沉浸化打磨）

> 28 个 HUD planner 已实装，核心状态条/事件流/快捷栏全在。但**缺少大量上下文感知 UI**：看不到目标 NPC/怪物的 HP、区域切换没有提示、灵田/forge/alchemy 进度条缺乏反馈、inspect 系统不完整、HUD 元素在和平/战斗间不做隐显切换。本 plan 把 HUD 从"功能够用"磨到"沉浸且极简"。

---

## 接入面 Checklist（防孤岛）

- **进料**：`BongHudOrchestrator` ✅ / 28 existing HUD planners ✅ / `CombatState` ✅ / `CurrentZone` ✅ / `NpcMetadataS2c`（plan-npc-engagement-v1 P0 新增）/ `LingtianSessionState` ✅ / `forge::ForgeSessionState` ✅ / `alchemy::BrewSessionState` ✅
- **出料**：新 HUD planner → `client/src/main/java/com/bong/client/hud/` / inspect screens → `client/src/main/java/com/bong/client/inspect/`
- **共享类型/event**：复用所有现有 S2c packet，不新增
- **跨仓库契约**：纯 client 侧 HUD 增强，server 零改动
- **worldview 锚点**：memory「HUD 沉浸式极简」/ memory「未解锁不常驻 HUD」

---

## 阶段总览

| 阶段 | 目标 | 状态 |
|------|------|------|
| P0 | 目标 HP/realm 条 + 区域切换提示 + HUD 沉浸隐显 | ✅ 2026-05-10 |
| P1 | 物品 inspect 详情 + forge/alchemy 进度增强 | ✅ 2026-05-10 |
| P2 | 灵田状态 overlay + 全 HUD 动画 polish | ✅ 2026-05-10 |

---

## P0 — 目标条 + 区域提示 + 沉浸隐显 ✅ 2026-05-10

### 交付物

1. **目标 HP/Realm 条**（`TargetInfoHudPlanner.java`）
   - 锁定目标（右键/攻击后 5s 保持）：屏幕顶部居中显示目标信息条
   - NPC 目标：名称 + realm + HP 条 + 真元条（如果玩家境界 ≥ 目标则显示，否则 "???"）
   - 生物目标：名称 + HP 条（无 realm/真元）
   - 玩家目标：名称 + realm（HP/真元隐藏——PVP 信息战）
   - 5s 无交互后 fade out

2. **区域切换提示**（`ZoneTransitionHudPlanner.java`）
   - 玩家 `CurrentZone` 变化时：屏幕中央偏下淡入区域名（中文，半透明白色，2s 显示 + 1s fade out）
   - 示例："青云断峰" / "灵泉湿地" / "裂谷"
   - 危险区域（spirit_qi < 0）：红色文字 + 追加 "⚠ 负灵域"
   - TSY 维度切换：全屏黑屏 0.5s → 区域名 → 回到正常（配合维度转换动画）

3. **HUD 沉浸隐显**（增强 `BongHudOrchestrator`）
   - **和平模式**（无战斗 10s+）：仅显示两层快捷栏 + 迷你状态条 + 事件流
   - **战斗模式**（CombatState::InCombat）：展开全部战斗 HUD（stamina/damage floater/target info/status effects）
   - **修炼模式**（is_meditating）：隐藏快捷栏，仅显示经脉图 + 真元条 + 事件流
   - 切换 crossfade 0.3s（不突兀）
   - 遵循 memory「HUD 沉浸式极简」：常驻仅两层快捷栏+三状态条+事件流

### 验收抓手

- 测试：`./gradlew test --tests "com.bong.client.hud.TargetInfoHudPlannerTest" --tests "com.bong.client.hud.BongZoneHudTest" --tests "com.bong.client.hud.BongHudOrchestratorTest" --tests "com.bong.client.hud.HudImmersionModeTest"`
- 手动：攻击 NPC → 顶部出现 HP 条 → 5s 后消失 → 走到新区域 → 中央出现区域名 → 进入和平 10s → HUD 简化 → 遇敌 → HUD 展开

---

## P1 — 物品 inspect + 进度增强 ✅ 2026-05-10

### 交付物

1. **物品 inspect 详情屏**（`client/src/main/java/com/bong/client/inspect/ItemInspectScreen.java`）
   - 长按物品 1s → 打开全屏 inspect：
     - 大图标居中 + 旋转展示动画
     - 属性面板：名称 / 稀有度 / 品质 / 重量 / 格子尺寸 / 保质期 / 充能次数 / 描述
     - 灵材追加：产地 / 适配丹方提示
     - 法器追加：灵核等级 / 铭文槽 / 当前附着
   - ESC 关闭

2. **Forge 进度增强**（增强现有 forge HUD）
   - 4 步状态机每步：进度条 + 当前步骤名（"淬火中..." / "铭文刻划..."）
   - 步骤切换时 screen flash（`OverlayQuadRenderer` 白闪 0.1s）
   - 完成时：产物名 toast + 稀有度颜色

3. **Alchemy 进度增强**（增强现有 alchemy HUD）
   - 火候阶段显示：温度条（蓝→绿→红渐变）
   - 炼制中鼎口冒蒸汽提示（配合 plan-entity-model-v1 P1 丹炉模型）
   - 成功/失败 toast + 丹药品质条

### 验收抓手

- 测试：`client::inspect::tests::item_inspect_opens_on_long_press` / `client::hud::tests::forge_step_progress_shows`
- 手动：长按物品 → 全屏 inspect → 看到完整属性 → 炼器 → 每步有进度条 + 步骤名 → 完成 toast

---

## P2 — 灵田 overlay + 动画 polish ✅ 2026-05-10

### 交付物

1. **灵田状态 overlay**
   - 靠近灵田 5 格：crosshair 右下方迷你面板
   - 内容：地块状态 icon（空/种/熟/枯）+ 植物名 + 生长进度 % + 染污度条
   - 复用 `BotanyHudPlanner` 框架，补齐 TODO

2. **全 HUD 动画 polish**
   - 所有 toast 弹出：slide-in from right 0.2s + slide-out 0.3s（不再瞬出瞬消）
   - 进度条填充：smooth lerp（不再逐帧跳变）
   - 状态条变化：增减值 flash（增=绿闪 / 减=红闪 0.3s）
   - 事件流消息：typewriter 效果（每字 30ms 间隔）

3. **全屏事件特效**（大事件增强）
   - 突破成功：全屏金色边框 flash 1s + toast "突破成功 · 引气期"
   - 化虚渡劫开始：全屏紫电纹路 2s
   - 死亡：全屏灰化 1s → 遗念文字渐出

### 验收抓手

- 测试：`client::hud::tests::toast_slide_animation` / `client::hud::tests::progress_bar_smooth_lerp`
- E2E：完整 30min gameplay → HUD 始终保持极简沉浸 / 战斗时自动展开 / toast 动画流畅 / 区域切换有提示

---

## 前置依赖

| 依赖 plan | 状态 | 用到什么 |
|-----------|------|---------|
| plan-HUD-v1 | ✅ finished | BongHudOrchestrator / 28 planners / ToastHudRenderer |
| plan-combat-no_ui | ✅ finished | CombatState |
| plan-vfx-v1 | ✅ finished | OverlayQuadRenderer / EdgeDecalRenderer |
| plan-forge-v1 | ✅ finished | ForgeSessionState / 4 步状态机 |
| plan-alchemy-v1 | ✅ finished | BrewSessionState |
| plan-lingtian-v1 | ✅ finished | LingtianPlot / LingtianSessionState |
| plan-botany-v1 | ✅ finished | BotanyHudPlanner |
| plan-death-lifecycle-v1 | ✅ finished | 遗念生成 / DeathInsight |

**全部依赖已 finished，无阻塞。**

## Finish Evidence

### 落地清单

- P0 目标条 / 区域提示 / 沉浸隐显：
  - `client/src/main/java/com/bong/client/hud/TargetInfoState.java`
  - `client/src/main/java/com/bong/client/hud/TargetInfoStateStore.java`
  - `client/src/main/java/com/bong/client/hud/TargetInfoHudPlanner.java`
  - `client/src/main/java/com/bong/client/hud/HudImmersionMode.java`
  - `client/src/main/java/com/bong/client/hud/BongZoneHud.java`
  - `client/src/main/java/com/bong/client/state/ZoneState.java`
  - `client/src/main/java/com/bong/client/mixin/MixinClientPlayerInteractionManagerAlchemy.java`
  - `client/src/main/java/com/bong/client/hud/BongHudOrchestrator.java`
- P1 物品 inspect / forge / alchemy 进度：
  - `client/src/main/java/com/bong/client/inspect/ItemInspectScreen.java`
  - `client/src/main/java/com/bong/client/inspect/ItemInspectLongPressTracker.java`
  - `client/src/main/java/com/bong/client/inventory/InspectScreen.java`
  - `client/src/main/java/com/bong/client/hud/ForgeProgressHudPlanner.java`
  - `client/src/main/java/com/bong/client/hud/AlchemyProgressHudPlanner.java`
- P2 灵田 overlay / HUD 动画 polish：
  - `client/src/main/java/com/bong/client/hud/LingtianOverlayHudPlanner.java`
  - `client/src/main/java/com/bong/client/hud/HudAnimation.java`
  - `client/src/main/java/com/bong/client/hud/EventStreamHudPlanner.java`
  - `client/src/main/java/com/bong/client/hud/BongToast.java`

### 关键 commit

- `1c9d138cb` (2026-05-10) `feat(hud): 打磨沉浸式 HUD 交互`
- `1ec82d74b` (2026-05-10) `test(hud): 覆盖 HUD polish 行为`
- `e67889c55` (2026-05-10) `fix(hud): 收紧 review 指出的 HUD 边界`
- `168e23251` (2026-05-10) `fix(hud): 补齐沉浸模式和灵田边界`
- `1e3b39d56` (2026-05-10) `fix(hud): 统一沉浸模式过滤空命令`
- `adc463b94` (2026-05-10) `fix(hud): 调整沉浸模式优先级`

### 测试结果

- `JAVA_HOME=/home/kiz/.sdkman/candidates/java/17.0.18-amzn ./gradlew test --tests "com.bong.client.hud.TargetInfoHudPlannerTest" --tests "com.bong.client.hud.BongZoneHudTest" --tests "com.bong.client.hud.BongHudOrchestratorTest" --tests "com.bong.client.hud.ProcessingHudPlannerTest" --tests "com.bong.client.hud.LingtianOverlayHudPlannerTest" --tests "com.bong.client.hud.HudAnimationTest" --tests "com.bong.client.inspect.ItemInspectScreenTest"` — PASS
- `JAVA_HOME=/home/kiz/.sdkman/candidates/java/17.0.18-amzn ./gradlew test build` — PASS
- `git diff --check` — PASS

### 跨仓库核验

- server：零改动；未新增 S2C packet 或协议字段。
- agent：零改动；未新增 schema。
- client：命中 `TargetInfoHudPlanner` / `ZoneState.dimensionTransition()` / `BongZoneHud.negativeZoneText()` / `HudImmersionMode` / `ItemInspectScreen` / `ForgeProgressHudPlanner` / `AlchemyProgressHudPlanner` / `LingtianOverlayHudPlanner` / `HudAnimation`。

### 遗留 / 后续

- 目标条当前只复用 client 可见 entity health 与本地 target store；`NpcMetadataS2c` 到位后可把 NPC realm / qi 由该 store 权威灌入，不需要改 HUD planner 契约。
- TSY 维度黑屏由 `dimension_transition` / `tsy_transition` zone 状态触发；服务端若要精确控制，需要在既有 `zone_info.active_events` 或 `status` 中发送该标记。
