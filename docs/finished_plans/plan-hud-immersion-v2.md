# Bong · plan-hud-immersion-v2

HUD 感知增强——在 `plan-hud-polish-v1` 已落地的 HUD 基座上拓展。hud-polish-v1 已覆盖 **目标 HP/realm 条、区域切换提示、和平/战斗/修炼三模式、物品 inspect、forge/alchemy 进度、灵田 overlay、全 HUD 动画 polish、全屏事件特效**。本 plan **不重复**目标条、区域提示、inspect、进度条、基础隐显逻辑，而是在其之上新增 4 个独立 HUD widget：① 灵压雷达（mini 圆环显示周边灵气方向密度）② 方向罗盘 ③ 威胁感知指示器 ④ HUD layout preset 系统 + 极端环境 HUD 变体。

**世界观锚点**：`worldview.md §三` 凝脉+ 可感知区域灵气精确值 → 灵压雷达的 lore 依据 · `§五` 通灵境可感知天道注意力（危机预警）→ 威胁指示器的 lore 依据 · `§十一` 匿名系统（默认不显示名字）→ HUD 不应暴露他人太多信息 · `§八` 天道语调冷漠——HUD 警告不应甜腻

**library 锚点**：`cultivation-0001 六境要录`（各境界感知力描述）

**前置依赖**：
- `plan-hud-polish-v1` ✅ finished → **硬依赖已满足**（当前 repo 仍有同名 active 文档 drift；消费本 plan 时以 client 代码符号和 finished evidence 为准，不依赖该 active 路径）
- `plan-HUD-v1` ✅ → `BongHudOrchestrator` / `BongHudStateSnapshot` / ZoneHudRenderer
- `plan-spirit-eye-v1` ✅ → spiritual_sense 感知范围
- `plan-perception-v1.1` ✅ → PerceptionEdgeState 威胁感知
- `plan-realm-vision` (impl) ✅ → fog/tint 层
- `plan-zone-atmosphere-v2` ✅ finished → `ZoneAtmosphereProfileRegistry` / `ZoneAtmospherePlanner` 可作为极端环境 HUD 变体配色依据

**反向被依赖**：
- `plan-breakthrough-cinematic-v1` 🆕 → 突破 cinematic 中 HUD 自动切沉浸模式
- `plan-death-rebirth-cinematic-v1` 🆕 → 濒死/死亡 cinematic 中 HUD 特殊 layout

---

## 当前代码实地核验（2026-05-11）

- **HUD 基座已存在**：`client/src/main/java/com/bong/client/hud/BongHudOrchestrator.java` 已集中组装 HUD command；`BongHudStateSnapshot` 已封装 `ZoneState` / `NarrationState` / `VisualEffectState`；`HudImmersionMode` 已按战斗/修炼/和平过滤 HUD layer。
- **hud-polish-v1 关键符号已落地**：`TargetInfoHudPlanner`、`BongZoneHud`、`LingtianOverlayHudPlanner`、`HudAnimation` 等已在 client 代码和 finished evidence 中；区域切换由 `BongZoneHud` + `ZoneState.changedAtMillis()` / `dimensionTransition()` 承担，不存在独立区域切换 planner 文件。
- **本 plan 的新 widget 尚未存在**：当前未发现 `QiDensityRadarHudPlanner`、`DirectionalCompassHudPlanner`、`ThreatIndicatorHudPlanner`、`HudLayoutPreset`。
- **环境/感知数据源可复用**：`ZoneState` 已有 `spiritQiNormalized()` / `spiritQiRaw()` / `negativeSpiritQi()` / `collapsed()` / `dangerLevel()`；`EnvironmentEffectController` / `ZoneEnvironmentState` / `ZoneAtmosphereProfileRegistry` 已存在；`PerceptionEdgeState` 目前是 `entries()` + `SenseEntry(kind,x,y,z,intensity)`，没有 `locked_by_count` 字段；`TribulationStateStore` 有 active tribulation phase/wave state。
- **实现约束**：本 plan 默认 client-only，不新增 server protocol。只有当现有 store 无法表达 widget 必需状态时，才在独立后续 plan 里扩 server payload；本 plan 内优先从 `BongHudStateSnapshot`、`ZoneState`、`PlayerStateStore`、`PerceptionEdgeStateStore`、`TribulationStateStore` 和本地 client world query 衍生。

**结论**：可升 active。前置已满足，新 widget 不存在且边界清晰；需要先接入现有 HUD command/layer 体系，再补 planner/unit test。

---

## 与 hud-polish-v1 的边界

| 维度 | hud-polish-v1 已做 | 本 plan 拓展 |
|------|-------------------|-------------|
| 目标信息 | `TargetInfoHudPlanner`（锁定目标 HP/realm 条） | 不碰 |
| 区域切换 | `BongZoneHud` + `ZoneState.changedAtMillis()` / `dimensionTransition()`（切换时中央文字 / 维度 blackout） | 不碰。罗盘上附带 zone 名是增量 |
| 隐显模式 | 和平/战斗/修炼三模式 + crossfade 0.3s | 增强：沉浸模式 fade 效果 + 被攻击临时恢复 + Alt peek + 自动进入时机 |
| inspect | `ItemInspectScreen`（长按物品全屏查看） | 不碰 |
| 进度条 | forge/alchemy 进度增强 + smooth lerp | 不碰 |
| 全屏事件 | 突破金框/天劫紫电/死亡灰化 | 不碰 |
| 灵压雷达 | 无 | 新增 `QiDensityRadarHudPlanner`（凝脉+） |
| 方向罗盘 | 无 | 新增 `DirectionalCompassHudPlanner`（顶栏） |
| 威胁感知 | 无 | 新增 `ThreatIndicatorHudPlanner`（通灵+） |
| layout preset | 无 | 新增 `HudLayoutPreset` 系统 |
| 负灵域/死域 HUD | 无 | 新增环境特殊 HUD 变体 |

---

## 接入面 Checklist

- **进料**：`BongHudOrchestrator` / `BongHudStateSnapshot` / `ZoneState` / `PlayerStateStore` / `PerceptionEdgeStateStore` / `TribulationStateStore` / `EnvironmentEffectController` / `ZoneEnvironmentState` / `ZoneAtmosphereProfileRegistry` / `SeasonStateStore`
- **出料**：`QiDensityRadarHudPlanner`（圆形 mini 灵压雷达）+ `DirectionalCompassHudPlanner`（顶栏方向 + zone 名）+ `ThreatIndicatorHudPlanner`（通灵+ 危机预警闪烁）+ `HudLayoutPreset`（战斗/探索/修炼三套 layout 对应 widget 集合）+ 负灵域/死域 HUD 变体渲染
- **跨仓库契约**：纯 client 侧——消费已有 server state 与本地 client world query；不新增网络 schema

---

## §0 设计轴心

- [ ] **信息可见但克制**：不摊数据面板——灵压用方向箭头密度表示、威胁用边缘 pulse 频率表示
- [ ] **境界决定 HUD 可见信息**：醒灵/引气无灵压雷达 / 凝脉出雷达+罗盘 / 通灵出威胁指示器
- [ ] **不暴露匿名系统**：雷达/威胁不显示其他玩家具体信息（只显示"有修士气息"方向标记）
- [ ] **hud-polish-v1 的沉浸模式是基座**：本 plan 的 layout preset 建立在其三模式之上，不替换而是细化

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|----|------|----|
| P0 | 灵压雷达 + 方向罗盘 | ✅ 2026-05-11 |
| P1 | 威胁感知指示器 | ✅ 2026-05-11 |
| P2 | HUD layout preset 系统 | ✅ 2026-05-11 |
| P3 | 负灵域/死域 HUD 变体 | ✅ 2026-05-11 |
| P4 | 沉浸模式增强（fade/peek/auto） | ✅ 2026-05-11 |
| P5 | 全矩阵压测（layout × 境界 × 模式 × 环境） | ✅ 2026-05-11 |

---

## P0 — 灵压雷达 + 方向罗盘 ✅ 2026-05-11

### 交付物

1. **`QiDensityRadarHudPlanner`**（`client/src/main/java/com/bong/client/hud/QiDensityRadarHudPlanner.java`）
   - **仅凝脉+ 玩家可见**（醒灵/引气 → 不显示）
   - 位置：屏幕左下角，迷你人体剪影右侧（不遮挡三状态条）
   - 外观：圆形 mini-map（半径 24px），8 方向标记
   - 每方向标记 = 该方向 32 格内灵气浓度 → 标记长度/亮度/颜色：
     - 高浓度（馈赠区）：长标记 + 金色
     - 中浓度：中标记 + 青白
     - 低浓度（死域附近）：短标记 + 灰色
     - 负浓度（负灵域）：标记反转内指 + 紫色（灵气被抽入方向）
   - "有修士气息"标记：8 格内有其他玩家/NPC → 对应方向出现小白点（不暴露 identity/realm/HP——匿名系统）
   - 数据源：首版用 `BongHudStateSnapshot.zoneState().spiritQiNormalized()` / `spiritQiRaw()` 给 zone 级基线，再用本地 client world/entity query 计算方向采样；若没有细粒度灵气栅格，不新增 server 通信，先以 zone 级雷达 + 本地实体方向完成闭环

2. **`DirectionalCompassHudPlanner`**（`client/src/main/java/com/bong/client/hud/DirectionalCompassHudPlanner.java`）
   - 位置：屏幕顶栏正中（不遮挡 `TargetInfoHudPlanner`——它在顶部偏下）
   - 外观：横向刻度条（240px 宽），标注 N/S/E/W + 角度刻度每 30°
   - 当前朝向在正中央，两侧显示 ±90° 范围
   - 刻度条下方小字：当前 zone 名称（中文，alpha 0.6——不抢眼）
   - zone 切换时名称 flash 1s（配合 hud-polish-v1 的 `BongZoneHud` 中央大字）
   - 灵龛方向标记：如果玩家已设灵龛 → 罗盘上对应方向小蓝三角（死亡后重生方向提示）
   - 坍缩渊入口标记：如果当前 zone 有 TSY portal → 罗盘上紫色菱形标记

3. **雷达/罗盘与沉浸模式联动**
   - hud-polish-v1 沉浸模式（和平 10s+）→ 雷达 alpha 0.3（不消失，保持环境感知）/ 罗盘正常
   - 修炼模式 → 雷达高亮（修炼时灵气方向更重要）/ 罗盘隐藏
   - 战斗模式 → 雷达正常 / 罗盘正常

### 验收抓手

- 测试：`client::hud::tests::radar_hidden_below_ningmai` / `client::hud::tests::radar_negative_qi_invert_marker` / `client::hud::tests::compass_zone_name_flash` / `client::hud::tests::niche_marker_on_compass`
- 手动：凝脉角色 → 左下角出现圆形雷达 → 走向高灵气区域 → 对应方向标记变长变金 → 转身 → 罗盘跟转 → zone 切换 → 罗盘下方 zone 名 flash

---

## P1 — 威胁感知指示器 ✅ 2026-05-11

### 交付物

1. **`ThreatIndicatorHudPlanner`**（`client/src/main/java/com/bong/client/hud/ThreatIndicatorHudPlanner.java`）
   - **仅通灵+ 玩家可见**（化虚额外增强）
   - 外观：屏幕四边缘 subtle pulse glow（不挡视野——边缘 2px 宽 gradient）
   - 消费 `PerceptionEdgeState`（已有 plan-perception-v1.1）→ 4 方向威胁等级
   - 颜色：低威胁=微绿(alpha 0.1) / 中=黄(alpha 0.2) / 高=红(alpha 0.3) / 极高=红+脉动(0.5Hz 闪烁)
   - pulse 频率 = 威胁距离的函数：远 → 慢闪(1s 周期) / 近 → 快闪(0.3s 周期) / 极近 → 常亮

2. **被锁定预警**
   - 其他玩家对你使用 inspect/锁定 → 若后续已有 lock/inspect store 则消费；当前 `PerceptionEdgeState` 无 `locked_by_count`，首版只从高强度 hostile `SenseEntry` 衍生边缘预警，不新增 schema
   - 预警 pulse：全屏边缘红色闪烁 1s（不透露谁锁定了你——匿名系统）
   - 接 worldview §六 顿悟 "敌人锁定你时有 1 秒预警"

3. **天劫预警**
   - 天劫临近（当前优先消费 `TribulationStateStore` 的 active phase/wave state；若只有已开始状态，则只做进行中预警，不虚构 countdown 字段）→ 全屏边缘紫电纹路 pulse
   - 与 hud-polish-v1 P2 的"全屏紫电纹路 2s"不冲突：hud-polish 是渡劫**开始**时的一次性特效，本 plan 是渡劫**临近**时的持续预警 pulse

4. **化虚增强**
   - 化虚境玩家：ThreatIndicator 追加"天道注意力条"
   - 外观：屏幕右下角微型竖条 3×20px
   - 天道注意力高 → 条满 + 红色 → 暗示"你太引人注目了"
   - 数据源：若已有 client store 暴露 `tiandao_attention` 则消费；当前未在 client/server grep 到稳定字段，首版不得新增协议，可先从 `TribulationStateStore` / `PerceptionEdgeState` 强度衍生或延后到独立契约 plan

### 验收抓手

- 测试：`client::hud::tests::threat_indicator_hidden_below_tongling` / `client::hud::tests::pulse_frequency_by_distance` / `client::hud::tests::lock_warning_1s`
- 手动：通灵角色 → 被高威胁 NPC 靠近 → 屏幕边缘红色 pulse 逐渐加快 → 被其他玩家 inspect → 全屏红闪 1s → 天劫临近 → 紫电 pulse

---

## P2 — HUD layout preset 系统 ✅ 2026-05-11

### 交付物

1. **`HudLayoutPreset`**（`client/src/main/java/com/bong/client/hud/HudLayoutPreset.java`）
   - 3 套 preset：

   | preset | 显示 widget | 隐藏 widget |
   |--------|-----------|-----------|
   | **战斗** | 雷达 + 罗盘 + 威胁 + MiniBody + 真元/stamina 条 + 目标条 + 事件流 | 灵田 overlay / 物品 inspect |
   | **探索** | 罗盘 + 雷达 + zone 名 + 三状态条 + 事件流 | 目标条 / 威胁 / MiniBody |
   | **修炼** | 真元条 + 经脉打通进度 + 雷达(高亮) + 事件流 | 快捷栏 / 罗盘 / 目标条 / MiniBody |

   - 自动切换：由 hud-polish-v1 P0 的三模式（和平/战斗/修炼）驱动，本 plan 只定义每个模式对应的 widget 集合
   - 手动覆盖：设置界面可自定义每个 preset 显示哪些 widget（owo-lib checkbox list）

2. **preset 切换动画**
   - 切换时：隐藏 widget alpha → 0（0.2s）/ 显示 widget alpha 0 → 1（0.3s）
   - 错开时序：先隐藏旧的，延迟 0.1s 再显示新的（避免重叠闪烁）
   - 复用 hud-polish-v1 P2 的 smooth lerp 基建

3. **HUD density 选项**
   - 设置界面新增 `HUD Density` 滑块：
     - **最小**：仅三状态条 + 事件流（适合截图/录制）
     - **标准**：preset 定义的全部 widget
     - **最大**：所有 widget 常驻（debug/竞技用）
   - density 覆盖 preset：最小模式下不管 preset 是什么都只显示最少 widget

### 验收抓手

- 测试：`client::hud::tests::preset_switches_on_combat_state` / `client::hud::tests::preset_animation_stagger` / `client::hud::tests::density_overrides_preset`
- 手动：和平时探索 preset → 遇敌 → widget 渐切到战斗 preset → 打坐 → 修炼 preset → 设置 density 最小 → 仅剩三状态条

---

## P3 — 负灵域/死域 HUD 变体 ✅ 2026-05-11

### 交付物

1. **负灵域 HUD 变体**
   - 进入负灵域（`ZoneEnvironment.spirit_qi < 0`）→ 全部 HUD 元素色调偏紫（hue shift +30°）
   - 真元条追加"被抽吸"动画：条从满端开始出现紫色腐蚀纹路，每 tick 向空端扩展（速率 = 负灵压强度）
   - 事件流文字颜色偏紫（#9966CC）
   - 雷达标记全部反转内指 + 紫色脉动（强化"灵气被抽入"的视觉信号）

2. **死域 HUD 变体**
   - 进入死域（`ZoneEnvironment.spirit_qi == 0`）→ 全部 HUD 元素饱和度 -60%（desaturation filter on HUD layer）
   - 灵压雷达灰掉（所有方向标记灰色短线——"这里什么都没有"）
   - 事件流文字颜色灰化
   - 罗盘颜色灰化 + zone 名显示"死域·[区域名]"（红色）

3. **坍缩渊 HUD 变体**
   - 进入 TSY 维度 → HUD 元素抖动（每 120 tick 微偏移 ±1px，暗示空间不稳）
   - 雷达标记不可靠（每 200 tick 随机方向出现假标记 0.5s——"你的感知在这里不可靠"）
   - 坍缩倒计时期间：全部 HUD 边缘加红色 vignette 逐渐收紧
   - race-out 阶段：罗盘上出口方向大绿箭头常亮（唯一可靠信息）

4. **HUD 变体切换过渡**
   - 正常 → 负灵域/死域/TSY：1s lerp（颜色/饱和度/位置）
   - zone 边界 150 格过渡带内：按距离 lerp 混合正常和变体（与 zone-atmosphere-v2 的 fog 过渡同步）

### 验收抓手

- 测试：`client::hud::tests::negative_qi_hue_shift` / `client::hud::tests::dead_zone_desaturation` / `client::hud::tests::tsy_radar_fake_markers` / `client::hud::tests::zone_boundary_hud_lerp`
- 手动：从灵泉湿地走向死域 → HUD 逐渐灰化 → 进入负灵域 → 紫色调 + 真元腐蚀 → 进入 TSY → HUD 微抖 + 雷达出现假标记

---

## P4 — 沉浸模式增强 ✅ 2026-05-11

### 交付物

1. **沉浸模式 fade 效果**
   - hud-polish-v1 P0 已建 `ImmersiveModeToggle`（默认 keybind F6）→ 本 plan 增强其 fade：
     - ON：所有 HUD 元素 0.5s fade out（不瞬消——worldview §八 克制感）
     - OFF：0.3s fade in
     - 关键警告仍显示：真元 < 20% / HP < 10% / 天劫临近 / 被玩家锁定 → 仅以屏幕边缘 pulse 形式出现

2. **Alt peek**
   - 沉浸模式 ON 时按住 Alt → 临时显示全部 HUD（alpha 0.6，不完全不透明——"瞥一眼"感觉）
   - 松开 Alt → 0.3s fade 回沉浸
   - 按住 > 3s 不松手 → 自动退出沉浸模式（用户想看 HUD）

3. **被攻击临时恢复**
   - 沉浸模式 ON 时被攻击 → 临时恢复战斗 HUD preset 5s
   - 5s 后如果已脱战 → 自动 fade 回沉浸
   - 如果仍在战斗 → 保持战斗 HUD 直到脱战 + 5s

4. **修炼自动进入**
   - 进入打坐（`is_meditating`）→ 3s 后自动切换沉浸模式（不需手动 F6）
   - 结束打坐 → 自动退出沉浸模式
   - 可在设置中关闭此行为

### 验收抓手

- 测试：`client::hud::tests::immersive_fade_duration` / `client::hud::tests::alt_peek_temporary` / `client::hud::tests::alt_peek_3s_exit` / `client::hud::tests::combat_temporary_restore` / `client::hud::tests::meditate_auto_immersive`
- 手动：F6 → HUD fade out 0.5s → Alt → 瞥见全部 HUD → 松开 → fade 回 → 被攻击 → 战斗 HUD 出现 → 脱战 5s → 又 fade → 打坐 → 3s 后自动沉浸

---

## P5 — 全矩阵压测 ✅ 2026-05-11

### 交付物

1. **矩阵覆盖**
   - 3 layout × 6 境界 × 2 沉浸模式 × 4 环境（正常/负灵域/死域/TSY）= 144 组合
   - 每组合：HUD 元素不重叠 / 不遮挡关键信息 / 颜色正确

2. **性能压测**
   - 30fps 基线下全部 HUD widget 同时渲染 < 1.5ms
   - 雷达 client 侧 spatial query 开销 < 0.5ms（32 格内 entity scan）

3. **分辨率适配**
   - 1920×1080 / 1366×768 / 2560×1440 三种分辨率下 HUD 不越界

### 验收抓手

- 自动化：`scripts/hud_matrix_test.sh`（遍历 144 组合 + 截图对比）
- 帧率测试：10min 连续游戏 + 全 widget 开启 → 帧率日志

---

## Finish Evidence

- **落地清单**：
  - P0：`client/src/main/java/com/bong/client/hud/QiDensityRadarHudPlanner.java`、`DirectionalCompassHudPlanner.java`、`HudRuntimeContext.java`；凝脉+ 可见，雷达消费 `ZoneState` + `PerceptionEdgeState`，罗盘消费 runtime yaw / zone label / TSY exit marker。
  - P1：`ThreatIndicatorHudPlanner.java`；通灵+ 消费 `PerceptionEdgeState`，天劫态消费 `TribulationStateStore`，化虚追加天道注意力竖条；未新增 server schema。
  - P2：`HudLayoutPreset.java`、`HudLayoutPreferenceStore.java`、`BongHudOrchestrator.java`；战斗 / 探索 / 修炼 preset、density override、preset stagger alpha 已接入现有 `HudImmersionMode` filter。
  - P3：`HudEnvironmentVariant.java`、`HudEnvironmentVariantPlanner.java`；负灵域紫 tint、死域灰化、TSY tint / jitter / 雷达假标记 / collapse edge 已接入。
  - P4：`HudImmersionMode.java`、`HudImmersionControls.java`、`BongHud.java`；F6 toggle、0.5s fade out、0.3s fade in、Alt peek、3s auto-exit、战斗恢复、打坐 3s auto immersive。
  - P5：`client/src/test/java/com/bong/client/hud/HudImmersionMatrixTest.java`、`scripts/hud_matrix_test.sh`；3 layout × 6 realm × 2 immersive × 4 environment × 3 resolution matrix。
- **关键 commit**：
  - `f4516451b`（2026-05-11）`docs(plan-hud-immersion-v2): 升级为 active plan`
  - `4eed43c11`（2026-05-11）`docs(plan-hud-immersion-v2): 补充实地核验边界`
  - `1305a3531`（2026-05-11）`feat(plan-hud-immersion-v2): 增加 HUD 沉浸感知组件`
  - `ec2137542`（2026-05-11）`test(plan-hud-immersion-v2): 覆盖 HUD 沉浸矩阵`
  - `1b8c5b18a`（2026-05-11）`docs(plan-hud-immersion-v2): finish evidence 并归档至 finished_plans/`
  - `fb2f90692`（2026-05-11）`fix(plan-hud-immersion-v2): 修正 HUD 方位与时间基准`
  - `71c4c408d`（2026-05-11）`fix(plan-hud-immersion-v2): 收紧沉浸 alpha 状态边界`
  - `a120e9954`（2026-05-11）`test(plan-hud-immersion-v2): 稳定 HUD 矩阵验证入口`
- **测试结果**：
  - `JAVA_HOME=$HOME/.sdkman/candidates/java/17.0.18-amzn ./gradlew test --tests "com.bong.client.hud.*"`：通过。
  - `scripts/hud_matrix_test.sh`：通过。
  - `JAVA_HOME=$HOME/.sdkman/candidates/java/17.0.18-amzn ./gradlew --no-daemon test build`：通过，`client/build/test-results/test` 汇总 1158 tests / 0 failure / 0 error / 0 skipped。
- **跨仓库核验**：本 plan 为纯 client 侧；消费既有 `ZoneState` / `PlayerStateStore` / `PerceptionEdgeStateStore` / `TribulationStateStore` / `ExtractStateStore`，没有新增 server payload、agent schema 或 Redis contract。
- **遗留 / 后续**：minimap 大地图仍留给 `plan-worldmap-v1`；HUD 自定义拖拽布局仍需独立 plan；真实灵气栅格采样、`locked_by_count`、`tiandao_attention` 若要做 server-authoritative 字段，应另开契约 plan。

## 进度日志

- 2026-05-11：实地核验 client HUD / environment / perception 代码，确认前置已满足、新 widget 未落地、且 `locked_by_count` / `tiandao_attention` 等旧文案字段当前不存在；按 client-only 边界升 active。
