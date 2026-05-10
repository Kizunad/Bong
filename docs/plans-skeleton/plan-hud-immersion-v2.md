# Bong · plan-hud-immersion-v2 · 骨架

沉浸式 HUD 增强。当前 `BongHudOrchestrator` 已编排 20+ HUD 组件（Zone / Toast / VisualEffect / 各 HudPlanner）——但 HUD 在"信息密度"和"沉浸感"之间还有空间。本 plan 做三件事：① 灵压雷达（mini 圆环显示周边灵气浓度方向）② 沉浸模式（一键隐藏所有 HUD，仅保留关键警告）③ 方向罗盘 + 威胁感知指示器。不替换现有 HUD，在现有编排层之上增强。

**世界观锚点**：`worldview.md §三` 凝脉+ 可感知区域灵气精确值 → 灵压雷达的 lore 依据 · `§五` 通灵境可感知天道注意力（危机预警）→ 威胁指示器的 lore 依据 · `§十一` 匿名系统（默认不显示名字）→ HUD 不应暴露他人太多信息 · `§八` 天道语调冷漠——HUD 警告也不应甜腻

**library 锚点**：`cultivation-0001 六境要录`（各境界感知力描述）

**前置依赖**：
- `plan-HUD-v1` ✅ → `BongHudOrchestrator` / `BongHudStateSnapshot` / ZoneHudRenderer
- `plan-spirit-eye-v1` ✅ → spiritual_sense 感知范围
- `plan-perception-v1.1` ✅ → PerceptionEdgeState 威胁感知
- `plan-realm-vision` (impl) ✅ → fog/tint 层
- `plan-zone-atmosphere-v2` 🆕 skeleton → zone profile（雷达配色依据）

**反向被依赖**：
- `plan-breakthrough-cinematic-v1` 🆕 → 突破 cinematic 中 HUD 自动切沉浸模式
- `plan-death-rebirth-cinematic-v1` 🆕 → 濒死/死亡 cinematic 中 HUD 特殊 layout

---

## 接入面 Checklist

- **进料**：`BongHudOrchestrator` / `BongHudStateSnapshot` / `ZoneHudRenderer` / `spirit_eye::SpiritualSense` / `perception::PerceptionEdgeState` / `cultivation::Realm`
- **出料**：`QiDensityRadarHudPlanner`（圆形 mini 灵压雷达）+ `DirectionalCompassHudPlanner`（顶栏方向 + zone 名）+ `ThreatIndicatorHudPlanner`（通灵+ 危机预警闪烁）+ `ImmersiveModeToggle`（一键隐藏/显示 HUD 层级）+ `HudLayoutPreset`（战斗/探索/修炼三套 layout）
- **跨仓库契约**：纯 client 侧——消费已有 server state（`SpiritualSense` / `ZoneEnvironment` / `PerceptionEdgeState`）

---

## §0 设计轴心

- [ ] **信息可见但克制**：不摊数据面板，用视觉隐喻（圆形灵压 → 方向箭头密度=灵气浓度 / 威胁指示器 → 边缘 pulse 的频率=威胁距离）
- [ ] **沉浸模式**：一键隐藏所有 HUD → 仅保留关键警告（真元 < 20% / HP < 10% / 天劫临近 / 被玩家锁定）
- [ ] **境界决定 HUD 可见信息**：醒灵无灵压雷达 / 凝脉出方向 zone 名 / 通灵出威胁指示器
- [ ] **不暴露匿名系统**：雷达/威胁不显示其他玩家具体信息（只显示"有修士气息"沿方向）

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|----|------|----|
| P0 | `QiDensityRadarHudPlanner`（圆形 mini-map，显示 8 方向灵气浓度微粒密度）+ `DirectionalCompassHudPlanner`（顶栏：当前朝向 + zone 名称 + 坐标）+ `ImmersiveModeToggle`（默认 keybind F6）+ `HudLayoutPreset` 数据结构 | ⬜ |
| P1 | `ThreatIndicatorHudPlanner`：通灵+ 激活。consum `PerceptionEdgeState` → 4 方向边缘 pulse（绿 = 低威胁 / 黄 = 中 / 红 = 高）+ 被其他玩家锁定时 1s 预警 pulse（接 worldview §六 顿悟 "敌人锁定你时有 1 秒预警"）+ 天劫临近全屏红光 pulse | ⬜ |
| P2 | 三套 HUD layout：战斗 layout（雷达 + compass + 威胁 + mini body + 真元/stamina 条）/ 探索 layout（compass + 雷达 + zone 名）/ 修炼 layout（仅真元条 + 经脉打通进度 + immersive mode auto-on 打坐时）| ⬜ |
| P3 | 负灵域 HUD 变体：进入负灵域 → 所有 UI 颜色偏紫 + 真元条加"被抽吸"动画（条从满向下腐蚀）+ 死域 HUD 变体（饱和度降低 + 灵气雷达灰掉） | ⬜ |
| P4 | 沉浸模式增强：immersive mode ON → HUD 元素渐隐（0.5s fade）+ 仅关键警告 overlay 出现（屏幕边缘 pulse）+ 被攻击时临时恢复战斗 HUD 5s 后再次渐隐 + 可在沉浸模式中按 Alt 临时 peek 全部 HUD | ⬜ |
| P5 | 饱和化测试：3 layout × 6 境界 × 沉浸/正常模式 × 所有 HUD 叠加无 layout 冲突 + 低配（30fps）压测 | ⬜ |

---

## Finish Evidence（待填）

- **落地清单**：`QiDensityRadarHudPlanner` / `DirectionalCompassHudPlanner` / `ThreatIndicatorHudPlanner` / `ImmersiveModeToggle` / 3 layout / 负灵域死域 HUD 变体
- **关键 commit**：P0-P5 各自 hash
- **遗留 / 后续**：minimap 大地图（M 键，需 `plan-worldmap-v1`）
