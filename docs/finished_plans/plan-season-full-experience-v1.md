# Bong · plan-season-full-experience-v1 · 骨架

季节完整体验——在 `plan-audio-world-v1`（节律音差 P2）+ `plan-botany-visual-v1`（灵材视觉）+ `plan-vfx-wiring-v1`（环境 VFX 接线）+ `plan-zone-atmosphere-v2`（区域 fog/sky 季节覆盖）✅ active / skeleton 基础上拓展。上述 plan 各自处理了季节的一个切面：audio-world-v1 P2 已做节律音差（炎汐 pitch ×1.1 / 凝汐 ×0.9）；botany-visual-v1 已做植物生长阶段渲染；vfx-wiring-v1 已做状态效果 VFX；zone-atmosphere-v2 P4 做季节 fog/sky 覆盖。但这些**分散在各 plan 中的季节碎片没有统一编排**——玩家仍然感知不到"现在是什么季节"。本 plan 做**季节体验层的统一编排者**：① 季节视觉控制器（整合各 plan 的季节切面）② 季节专属粒子 ③ 灵草季节生态可视化 ④ 兽潮大迁徙视觉事件 ⑤ 突破与季节联动的电影化表现。

**世界观锚点**：`worldview.md §十七` 末法节律（夏=炎汐 / 冬=凝汐 / 过渡=汐转）+ 地形响应 5 类 + 修炼节奏/渡劫/灵物 shelf life 与季节共振 · `§三` 突破与季节（夏渡劫命中率高 / 冬突破爆发不足 / 汐转高风险）· `§七` 生物生态大迁徙（区域灵气被吸干→兽潮）· `§九` 骨币半衰 + 物资保存（夏快冬慢）

**library 锚点**：`world-0002 末法纪略`（末法无四季只有散聚两态的描述）

**前置依赖**：
- `plan-audio-world-v1` 🆕 active → **节律音差已做**（P2 炎汐/凝汐 ambient pitch 偏移 + 火/雪追加音）——本 plan 不重复 audio，仅在 `SeasonVisualController` 中引用其 ambient 切换作为视听协同
- `plan-botany-visual-v1` 🆕 active → **植物生长阶段渲染已做**——本 plan 不重复基础渲染，仅叠加季节导致的视觉状态变化（冻结光泽/蒸散微粒）
- `plan-vfx-wiring-v1` 🆕 active → **状态效果 VFX 已做**——本 plan 不碰状态 VFX，仅新增季节专属粒子
- `plan-zone-atmosphere-v2` 🆕 skeleton → **季节 fog/sky 覆盖（P4）已做**——本 plan 不重复 fog/sky lerp，仅做上层编排 + 粒子 + gameplay 层
- `plan-jiezeq-v1` ✅ → `SeasonState` / `SeasonClock` / 跨系统 hook
- `plan-botany-v2` ✅ → 植物 season 依赖
- `plan-particle-system-v1` ✅ → 季节粒子基类

**反向被依赖**：
- `plan-breakthrough-cinematic-v1` 🆕 → 突破视觉受季节影响（本 plan P4 定义）

---

## 与各 active plan 的边界

| 维度 | 哪个 plan 已做 | 本 plan 拓展 |
|------|--------------|-------------|
| 季节 ambient 音差 | audio-world-v1 P2 | 不碰。仅引用 |
| 季节 fog/sky | zone-atmosphere-v2 P4 | 不碰 fog/sky 参数。仅在 `SeasonVisualController` 中发信号触发其切换 |
| 植物生长渲染 | botany-visual-v1 | 叠加季节视觉：冻结光泽 / 蒸散微粒 / 霜结物种可见性 |
| 状态效果 VFX | vfx-wiring-v1 P2 | 不碰。仅新增季节独有粒子 |
| 季节专属粒子 | 无 | 热浪扭曲 / 冰晶闪烁 / 紊流灵气 / 劫气标记 |
| 兽潮 | 无 | 大迁徙视觉事件（万兽奔腾粒子 + 地面震动 + 音效） |
| 突破季节联动 | 无 | 季节影响突破 cinematic 的视觉叠加 |
| 季节 HUD | 无 | HUD 角落微妙季节图标（小叶/雪花/紊线）——不写文字 |

---

## 接入面 Checklist

- **进料**：`SeasonState { phase, progress, days_in_phase }`（plan-jiezeq-v1 ✅）/ `SeasonClock` / `ZoneEnvironment` / `botany::PlantSeasonalState` / `cultivation::BreakthroughRequest` / `npc::NpcState`
- **出料**：`SeasonVisualController`（统一编排季节视觉切面 → 向各 plan 发信号）+ `SeasonParticleEmitter`（3 套季节专属粒子）+ `SeasonGameplayHints`（NPC 气泡/agent narration/HUD icon）+ `MigrationVisualEvent`（兽潮视觉）+ `SeasonBreakthroughOverlay`（突破季节叠加）
- **跨仓库契约**：server `SeasonState` → client `SeasonVisualController` / agent `seasonal_narration` template（天道叙事中提及季节但不显式——"天地间灵气似乎躁动起来"不说"现在是炎汐期"）

---

## §0 设计轴心

- [x] **无显式 tag**：季节不在 HUD 写"当前：夏"——通过天空/粒子/NPC 旁白/灵草状态间接表现（严守 worldview §K 红线）
- [x] **可感知的游戏性差异**：玩家应能凭经验判断"现在是渡劫的好时候吗？"
- [x] **季节改变世界**：同一坐标夏天和冬天应该是**不同的世界**（worldview §十七原文）
- [x] **死域不受季节影响**：死域/余烬死地永远灰白（由 zone-atmosphere-v2 P4 保证，本 plan 不需重复）
- [x] **本 plan 是编排者不是实现者**：fog/sky 由 zone-atmosphere-v2 改、ambient 由 audio-world-v1 改、植物渲染由 botany-visual 改——本 plan 提供 `SeasonVisualController` 作为统一触发器，向各 plan 的系统发信号

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|----|------|----|
| P0 | SeasonVisualController 统一编排骨架 + 季节 HUD icon + 季节专属粒子 | ✅ |
| P1 | 灵草季节生态可视化 | ✅ |
| P2 | NPC 季节行为 + agent narration 季节模板 | ✅ |
| P3 | 兽潮大迁徙视觉事件 | ✅ |
| P4 | 突破与季节联动的电影化表现 | ✅ |
| P5 | 完整季节循环 e2e（一个 game-year 全链路走查） | ✅ |

---

## P0 — SeasonVisualController + HUD icon + 专属粒子 ✅

### 交付物

1. **`SeasonVisualController`**（`client/src/main/java/com/bong/client/season/SeasonVisualController.java`）
   - 注册到 `ClientTickEvents.END_CLIENT_TICK`
   - 每 tick 读取 `SeasonState.phase` + `SeasonState.progress`（0.0-1.0 在当前 phase 内的进度）
   - 向各系统发信号：
     - → `ZoneAtmosphereRenderer`（zone-atmosphere-v2）：`setSeasonOverride(phase, progress)`
     - → `MusicStateMachine`（audio-world-v1）：`setSeasonModifier(phase, progress)`
     - → `SeasonParticleEmitter`（本 plan）：`updateSeason(phase, progress)`
   - phase 切换时 emit `SeasonTransitionEvent`（client 侧 event，被各 HUD/VFX 消费）
   - 统一入口，避免各系统各自轮询 `SeasonState` 导致不同步

2. **季节 HUD 微妙图标**（`SeasonHintHudPlanner.java`）
   - 位置：屏幕右上角，时间/天气信息旁（如有）
   - 图标极小（8×8px）+ alpha 0.4（不抢眼）：
     - 炎汐：小火焰（暖橙色）
     - 凝汐：小雪花（冷白色）
     - 汐转：小紊乱线（灰紫色 + 微抖动 per-tick）
   - **不写任何文字**（严守 §K 红线——玩家应该能靠环境判断而非靠 HUD 读字）
   - 新玩家 hint：首次见到图标时 toast "你感到天地间灵气在变化"（仅一次，永不重复）

3. **季节专属粒子**（`SeasonParticleEmitter.java`）
   - 炎汐粒子：
     - 热浪扭曲：地面 y+0.5 位置水平波纹粒子（`BongSpriteParticle` `lingqi_ripple` tint #FFD700 alpha 0.15, density 0.5 per 10 tick, lifetime 30 tick）
     - 远雷闪光：天空随机位置白色闪光（`BongSpriteParticle` `tribulation_spark` tint #FFFFFF scale 8.0, density 0.1 per 120 tick, lifetime 2 tick——瞬闪）
     - 灵草蒸散微粒：`BotanyPlant` entity 附近微金粒子上升（density 0.3, lifetime 20 tick——灵气蒸发）
   - 凝汐粒子：
     - 飘雪：全域白色小粒子缓慢下落（`BongSpriteParticle` `cloud256_dust` tint #FFFFFF density 1.5, drift (random_x × 0.01, -0.03, random_z × 0.01)）
     - 冰晶闪烁：偶发微小棱形闪光（`BongSpriteParticle` `enlightenment_dust` tint #C0E0FF density 0.2 per 60 tick, lifetime 15 tick）
     - 灵物冻结光泽：低温植物表面微蓝冰层反光
   - 汐转粒子：
     - 紊乱灵气流：大尺度 `BongLineParticle` 随机方向（长度 2-5 block, tint #9966CC alpha 0.2, density 0.3 per 40 tick, lifetime 10 tick——空中可见灵气在乱窜）
     - 劫气标记微粒：被标记玩家头顶微红闪烁（`BongSpriteParticle` `tribulation_spark` tint #FF4444 density 0.5 per 20 tick——暗示"你被劫气标记了"但不明确告知）

### 验收抓手

- 测试：`client::season::tests::controller_syncs_all_systems` / `client::season::tests::hud_icon_no_text` / `client::season::tests::summer_heat_wave_particles` / `client::season::tests::winter_snow_drift_speed` / `client::season::tests::transition_chaos_particles`
- 手动：炎汐 → 地面热浪+远雷+微金蒸散 → 凝汐 → 飘雪+冰晶+植物冻结 → 汐转 → 紫色灵气乱窜+劫气微红

---

## P1 — 灵草季节生态可视化 ✅

### 交付物

1. **季节导致的植物视觉状态**（叠加到 botany-visual-v1 已有的生长阶段渲染之上）
   - 炎汐：
     - 耐热植物：生长加速视觉 → 叶片微金 tint + 粒子蒸散加倍
     - 不耐热植物：表层枯萎视觉 → 叶片 desaturation + 尖端棕色 tint + 偶发掉叶粒子
   - 凝汐：
     - 耐寒植物：冬季特殊视觉 → 表面微蓝冰层 + 内里微光脉动（真元在内部缓慢流动——"它还活着"）
     - 不耐寒植物：冻结视觉 → 全体白色 frost overlay + 静止（无生长动画）
     - 霜结物种（仅冬季可见）：从无到有 fade in 5s（平时隐形——worldview §十七 霜结灵物）
   - 汐转：
     - 所有植物：生长速度视觉波动（animation speed 正弦波 0.5×-1.5× per 60 tick）
     - 偶发紫色灵气脉冲从植物向外扩散（灵气紊乱导致植物"放电"）

2. **灵草采集时机提示**
   - NPC 经过成熟植物时偶发气泡（P0 NPC 交互系统支持后）：
     - 凝汐将尽 + 霜结物种成熟："（看向雪魄莲）…快了。"
     - 炎汐 + 耐热灵草过熟："再不采就蒸干了。"
   - agent narration 季节性提示（不直接说季节名）：
     - "天地间灵气如沸油——灵物生长如疯。"（=炎汐）
     - "万物凝滞，唯冰下有光。"（=凝汐 + 霜结物种暗示）

3. **灵田季节响应可视化**（叠加到 hud-polish-v1 P2 灵田 overlay 之上）
   - 灵田 overlay 追加季节指标：
     - 炎汐：进度条旁小火焰 icon（生长 ×1.2）
     - 凝汐：进度条旁小雪花 icon（生长 ×0.6）
     - 汐转：进度条旁紊乱 icon（生长速度波动）
   - 不写数字——图标+颜色变化暗示

### 验收抓手

- 测试：`client::season::tests::plant_heat_tolerance_visual` / `client::season::tests::frost_species_fade_in_winter` / `client::season::tests::lingtian_overlay_season_icon`
- 手动：炎汐 → 灵草叶片微金+蒸散 / 不耐热枯萎 → 凝汐 → 霜结物种从无到有 → 灵田进度条旁雪花 icon

---

## P2 — NPC 季节行为 + agent narration ✅

### 交付物

1. **NPC 季节行为可视化**
   - 炎汐：散修活跃度 +30%（移动速度视觉加快 + 对话气泡频率增加）/ 凡人寻找阴凉（趋向树荫/建筑）
   - 凝汐：散修龟缩（移动范围缩小 50% + 偶发瑟缩动画 → 微抖动 entity position ±0.02）/ 凡人室内行为（进入建筑后不出来直到汐转）
   - 汐转：NPC 焦虑行为（移动路径不确定 + 偶发左右看 → entity yaw 快速转动 ±30°）/ 高境 NPC 打坐准备渡劫

2. **NPC 季节对话模板**
   - 散修 + 炎汐："天热，灵草长得倒快。"
   - 散修 + 凝汐："…太冷了。骨币也缩水了。"（worldview §九 冬季半衰慢——但买卖少所以缩水）
   - 散修 + 汐转："你也感觉到了？…天地间，不太对。"
   - 凡人 + 凝汐："大仙，小人家里没柴了…"
   - 守墓人（全季节）："…"（守墓人不受季节影响——他们已经超脱了）

3. **Agent narration 季节模板**（`agent/packages/tiandao/src/templates/seasonal.ts`）
   - 天道在适当时机（季节切换后 100-300 tick 内随机）发一条 narration：
     - 炎汐开始："灵气如沸。天地间有什么在骚动。"
     - 凝汐开始："万物沉寂。灵气凝滞如死水。"
     - 汐转开始："节律紊乱——天道也看不清接下来的走向。"
   - **绝不**直接说"炎汐期开始"——天道也用隐喻

### 验收抓手

- 测试：`server::npc::tests::npc_activity_by_season` / `agent::tiandao::tests::seasonal_narration_no_explicit_name`
- 手动：炎汐 → NPC 活跃+灵草对话 → 凝汐 → NPC 龟缩+凡人躲室内 → 汐转 → NPC 焦虑+天道 narration"节律紊乱"

---

## P3 — 兽潮大迁徙视觉事件 ✅

### 交付物

1. **兽潮触发条件**
   - 某区域 `ZoneEnvironment.spirit_qi` 连续下降 → 低于阈值（即将化死域）→ 触发 `MigrationEvent`
   - server 侧 `fauna_migration_system`：扫描 zone 灵气 → emit `MigrationEvent { zone_id, direction, duration }`
   - 持续 5-10min（按 zone 面积缩放）

2. **大迁徙视觉**
   - 所有野生 NPC/生物朝正数灵气方向狂奔：
     - entity velocity ×2（server 驱动，client 只负责视觉）
     - 奔跑烟尘粒子：`BongSpriteParticle` `cloud256_dust` tint 棕色 × 8 per entity per 5 tick
     - 大量 entity 同向奔跑的"群体视觉"——camera 微震 intensity 0.05 持续（万兽奔腾的地面震感）
   - 天空变化：兽潮方向天色微暗（dust cloud 遮天——fog_density +0.1 toward direction）
   - 落后的老弱 NPC 掉队 → 站着喘气 → 被即将到来的死域吞噬 → 灰化消失（与死域扩张同步）

3. **兽潮音效**（与 audio-world-v1 协同）
   - `migration_rumble.json`：`minecraft:entity.warden.roar`(pitch 0.2, volume 0.1, loop 3s) + `minecraft:entity.horse.gallop`(pitch 0.3, volume 0.05, loop 1s)（远距离大地震动 + 马蹄般的群体声）
   - 方向性：audio attenuation WORLD profile，从兽潮方向传来

4. **兽潮 HUD 提示**
   - hud-polish-v1 事件流追加："[天道] 灵气正在枯竭——万物正在逃离。"
   - 罗盘（hud-immersion-v2 如已建）上标注兽潮方向红色箭头

### 验收抓手

- 测试：`server::fauna::tests::migration_triggers_on_qi_drop` / `client::season::tests::migration_dust_particles` / `client::season::tests::migration_camera_shake`
- 手动：在 zone 灵气缺少的区域等 → 兽潮开始 → NPC/生物朝一个方向奔跑 → 烟尘遮天 → 地面微震 → 落后者灰化 → 5min 后结束

---

## P4 — 突破与季节联动 ✅

### 交付物

1. **季节叠加突破 cinematic**（叠加到 breakthrough-cinematic-v1 的视觉之上）
   - 夏季（炎汐）突破：
     - 天劫叠加额外雷暴粒子（比正常渡劫多 50% 闪电粒子）
     - 突破光柱颜色偏金（tint +金色 20%）
     - 成功后天空短暂清亮 1s（fog_density → 0 瞬间清澈——"天地认可了你"）
   - 冬季（凝汐）突破：
     - 天地光柱叠加冰晶折射粒子（`BongSpriteParticle` 棱形 × 12 围绕光柱旋转）
     - 光柱颜色偏冷白蓝
     - 成功后周围 16 格雪粒停滞 0.5s（时间仿佛凝固——"你的突破震动了凝滞的天地"）
   - 汐转突破：
     - 紊乱扭曲效果叠加（屏幕 noise displacement 强度 ×2）
     - 光柱颜色在金/白/紫之间闪烁不定
     - 额外劫气标记风险提示：HUD 边缘紫色 pulse + 事件流 "节律紊乱，天地注意力聚焦于你"（不直接说"汐转风险高"——用后果暗示）

2. **突破失败季节差异**
   - 夏季失败：失败碎裂粒子（vfx-wiring-v1 已有）颜色偏金红（热力反噬）
   - 冬季失败：失败粒子偏冷蓝 + 冰裂声（`minecraft:block.glass.break`(pitch 0.5)——冻裂感）
   - 汐转失败：失败粒子紫色乱射 + 额外伤害视觉（vignette 紫红 pulse 2s——紊乱反噬更严重）

3. **修炼效率季节可感知**
   - 打坐时吸灵粒子（vfx-wiring-v1 P0 `CultivationAbsorbPlayer` 已做）→ 季节修正：
     - 炎汐：粒子密度 ×1.2 + 粒子速度 ×1.3（灵气活跃，吸得快）
     - 凝汐：粒子密度 ×0.6 + 粒子速度 ×0.5（灵气凝滞，吸得慢）
     - 汐转：粒子密度波动 + 偶发反向弹射（灵气不稳定——吸到一半被弹回去）

### 验收抓手

- 测试：`client::season::tests::summer_breakthrough_extra_lightning` / `client::season::tests::winter_breakthrough_frost_refraction` / `client::season::tests::transition_breakthrough_flicker` / `client::season::tests::meditation_absorb_density_by_season`
- 手动：夏季突破 → 金色光柱+额外雷暴 → 冬季突破 → 冷白光柱+冰晶旋转+雪粒停滞 → 汐转突破 → 光柱闪烁+紫色 pulse

---

## P5 — 完整季节循环 e2e ✅

### 交付物

1. **一个 game-year 全循环走查**
   - 加速时间跑完 炎汐→汐转→凝汐→汐转→炎汐 一个完整循环
   - 每个 phase 内在 6 个 zone 各停留 30s → 视觉/音效/粒子正确
   - 切换时 crossfade/lerp 过渡无断裂

2. **季节 × 各系统联动矩阵**
   - 3 季节 × 6 zone × 灵草(3 类) × NPC(2 类) × 修炼(2 状态) × 突破(2 结果) = 216 组合
   - 自动化截图覆盖关键组合（3×6×3 = 54 截图最少）

3. **兽潮 e2e**
   - 人工触发 zone 灵气下降 → 兽潮开始 → 视觉/音效 → 持续 5min → 结束 → zone 变死域

4. **性能**
   - 季节粒子开销：最差情况（汐转 + 兽潮 + 突破同时）< 3ms per frame
   - SeasonVisualController tick 开销 < 0.1ms

### 验收抓手

- 自动化：`scripts/season_cycle_test.sh`（加速时间 + 截图）
- 手动：完整体验一个 game-year → 记录每个 phase 切换的视听感受 → 确认"不看 HUD 也知道季节变了"

---

## Finish Evidence

- 实现提交：
  - `0182d2013` `plan-season-full-experience-v1: 接入客户端季节体验`
  - `533c0f02b` `plan-season-full-experience-v1: 落地兽潮与NPC季节契约`
  - `8b08c5342` `plan-season-full-experience-v1: 增加天道季节旁白`
  - `e64716c94` `plan-season-full-experience-v1: 补季节循环验证脚本`
- P0：
  - `SeasonVisualController` 统一读取 `SeasonStateStore`，同步 `ZoneAtmosphereRenderer` / `MusicStateMachine` / `SeasonParticleEmitter`，phase 切换时产出 `SeasonTransitionEvent`。
  - `SeasonHintHudPlanner` 与 `LingtianOverlayHudPlanner` 仅绘制低 alpha 图标/色块，不写显式季节文字。
  - `SeasonParticleEmitter` 覆盖炎汐热浪/远雷/蒸散、凝汐飘雪/冰晶、汐转紊乱线/劫气标记。
- P1：
  - `SeasonPlantVisuals` 在既有 botany stage 渲染上叠加耐热/不耐热、耐寒/霜结、汐转脉冲视觉。
  - `BotanyPlantEntityRenderer` 应用季节 tint / alpha / sway；灵田 overlay 追加季节图标。
- P2：
  - server `npc::seasonal_behavior` 通过 `NpcSeasonRuntime` 写入 NPC tick，`NpcPatrol` 读取季节移动系数并同步收缩/放宽巡逻半径。
  - agent `templates/seasonal.ts` 与 `SeasonalNarrationTracker` 在季节切换后延迟发隐喻式 narration，并在模板加载时直接 lint 禁止显式季节名。
- P3：
  - server `fauna_migration_system` 按 zone 灵气骤降和低阈值触发 `MigrationEvent`，同时发 `bong:migration_visual` VFX payload。
  - client `MigrationVisualPlayer` 通过 `MigrationVisualPlanner` 消费兽潮 payload，产出烟尘/雾感视觉。
- P4：
  - `SeasonBreakthroughOverlay` 为突破成功/失败和打坐吸灵提供季节 tint、闪电倍率、冰晶折射、汐转 flicker、反噬强度、粒子密度/速度修正。
  - `SeasonBreakthroughOverlayHud`、`BreakthroughPillarPlayer`、`BreakthroughFailPlayer` / `CultivationAbsorbPlayer` 消费这些 profile，叠加到既有 cinematic/VFX。
- P5：
  - `SeasonFullExperienceTest.full_season_cycle_keeps_visual_signals_in_sync` 跑完炎汐→汐转→凝汐→汐转→炎汐，校验 atmosphere/music/HUD/particle 同步。
  - `scripts/season_cycle_test.sh` 作为跨栈 smoke gate，串起 client 季节体验、server 兽潮/NPC 契约、agent 季节 narration；真截图 / 性能预算留后续 plan。
- 验证：
  - `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
  - `cd client && JAVA_HOME="$HOME/.sdkman/candidates/java/17.0.18-amzn" PATH="$HOME/.sdkman/candidates/java/17.0.18-amzn/bin:$PATH" ./gradlew test build`
  - `cd agent && npm run build && npm test -w @bong/tiandao && npm test -w @bong/schema`
  - `bash scripts/season_cycle_test.sh`
  - `git diff --check`
- 遗留 / 后续：
  - 季节对 PVP meta 的数值影响仍留给 `plan-style-balance-v1` 联动。
  - 季节 NPC 贸易路线变化留给后续 `plan-economy-v2` 类任务；本 plan 只落体验层和契约面。
  - 完整截图编排与帧预算验证仍建议单独拆出 `plan-season-runtime-wire-v1` 一类后续 plan。
