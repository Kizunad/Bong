# Bong · plan-season-full-experience-v1 · 骨架

季节完整体验——在 `plan-jiezeq-v1` 节律基础设施之上补全视觉/听觉/玩法层。当前 jiezeq-v1 已建 `SeasonState`（炎汐/凝汐/汐转 3 大阶段）+ 跨系统 hook（突破/shelflife/劫气/寿元）——但玩家**感受不到季节**。本 plan 让季节"可被看见、听到、玩到"：天空渐变、环境音切换、季节专属粒子、NPC 季节行为、灵草季节生态、突破时机选择。

**世界观锚点**：`worldview.md §十七` 末法节律（夏=炎汐 / 冬=凝汐 / 过渡=汐转）+ 地形响应 5 类 + 修炼节奏/渡劫/灵物 shelf life 与季节共振 · `§三` 突破与季节（夏渡劫命中率高 / 冬突破爆发不足 / 汐转高风险）· `§七` 生物生态大迁徙（区域灵气被吸干→兽潮）· `§九` 骨币半衰 + 物资保存（夏快冬慢）

**library 锚点**：`world-0002 末法纪略`（末法无四季只有散聚两态的描述）

**前置依赖**：
- `plan-jiezeq-v1` ✅ active → `SeasonState` / `SeasonClock` / 跨系统 hook
- `plan-lingtian-weather-v1` ⏳ active → 灵田季节响应（改为 jiezeq 消费者）
- `plan-botany-v2` ✅ → 植物 season 依赖
- `plan-zone-atmosphere-v2` 🆕 skeleton → 季节天空/fog 渐变
- `plan-audio-implementation-v1` 🆕 skeleton → 季节 ambient loop
- `plan-vfx-v1` ✅ → 屏幕叠加
- `plan-particle-system-v1` ✅ → 季节粒子
- `plan-terrain-ash-deadzone-v1` ⏳ active → 死域不受季节影响（恒 0）

**反向被依赖**：
- `plan-breakthrough-cinematic-v1` 🆕 → 突破视觉受季节影响
- `plan-hud-immersion-v2` 🆕 → HUD 季节指示

---

## 接入面 Checklist

- **进料**：`SeasonState { phase, progress, days_in_phase }` / `SeasonClock` / `ZoneEnvironment` / `LingtianPlot`（灵田季节响应）/ `botany::PlantSeasonalState` / `cultivation::BreakthroughRequest` / `alchemy::ShelfLife`
- **出料**：`SeasonVisualController`（天空色渐变 lerp + 雾色 lerp + 季节专属粒子发射器）+ `SeasonAudioController`（炎汐=虫鸣+雷暴 ambient / 凝汐=风声+雪粒 ambient / 汐转=紊乱低频嗡）+ `SeasonGameplayHints`（NPC 对话提及季节 / agent narration 提及节律 / HUD 角落微妙季节图标）+ `MigrationEvent`（兽潮 visual）
- **跨仓库契约**：server `SeasonState` → client `SeasonVisualController` + agent `seasonal_narration` template

---

## §0 设计轴心

- [ ] **无显式 tag**：季节不在 HUD 写"当前：夏"——通过天空颜色/环境音/粒子间接表现（严守 `plan-gameplay-journey-v1 §K` 红线）
- [ ] **可感知的游戏性差异**：玩家应能凭经验判断"现在是渡劫的好时候吗？"
- [ ] **季节改变世界**：同一坐标夏天和冬天应该是**不同的世界**（worldview §十七原文）
- [ ] **死域不受季节影响**：死域/余烬死地永远灰白（worldview §十七·5 类地形之首）

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|----|------|----|
| P0 | `SeasonVisualController` 骨架：按 `SeasonState.progress` lerp 天空 color（夏=微金橙 / 冬=灰白蓝 / 汐转=间歇紫灰）+ 雾色 lerp（夏=薄金雾 / 冬=浓白雾 / 汐转=波动雾密度）+ `SeasonAudioController`（3 套 ambient loop，按进度 crossfade）+ HUD 角落微妙季节图标（小叶/雪花/紊乱线——不写文字） | ⬜ |
| P1 | 季节专属粒子：炎汐=热浪扭曲粒子 + 远雷闪光粒子 + 灵草蒸散微粒子 / 凝汐=飘雪粒子 + 冰晶闪烁 + 灵物"冻结"光泽 / 汐转=紊乱灵气流粒子（随机方向）+ 劫气标记粒子（微红闪烁暗示"你被标记了"） | ⬜ |
| P2 | 灵草季节生态可视化：耐热/耐寒/霜结物种在不同季节的视觉状态（炎汐=茂盛但表层枯萎 / 凝汐=冻结但内里有缓慢流动真元微光 / 霜结物种冬季才可见——平时隐形）+ 灵草采集时机提示（NPC 旁白 "凝汐将尽，该采雪魄莲了"） | ⬜ |
| P3 | 兽潮（大迁徙）视觉事件：区域灵气被吸干即将化死域 → agent narration 警告 → 所有野生 NPC/生物朝正数灵气区狂奔 → client 侧大量 NPC 奔跑粒子 + 地面震动 + 音效（万兽奔腾 ambient）→ 持续时间 5-10min | ⬜ |
| P4 | 突破与季节联动视觉：夏季渡劫 → 天劫 cinematic 叠加雷暴粒子 / 冬季突破 → 天地光柱叠加冰晶折射 / 汐转突破 → 紊乱扭曲效果 + 额外劫气标记风险提示（HUD pulse "节律紊乱，此时突破风险倍增"） | ⬜ |
| P5 | 完整季节循环 e2e：一个 game-year（≈数十 game-day）走完 炎汐→汐转→凝汐→汐转→炎汐 全循环 × 6 zone × 所有季节联动系统验证 | ⬜ |

---

## Finish Evidence（待填）

- **落地清单**：`SeasonVisualController` / `SeasonAudioController` / 季节粒子 ×3 / 灵草季节 ecology vis / 兽潮 event / 突破季节联动 / HUD 季节图标
- **关键 commit**：P0-P5 各自 hash
- **遗留 / 后续**：季节对 PVP meta 的影响数值（`plan-style-balance-v1` 联动）
