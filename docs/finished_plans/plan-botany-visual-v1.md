# Plan: Botany Visual v1（灵材视觉生态）

> 植物系统后端完整（22 v1 + 17 v2 植物、灵田 6 动作、采集/种植/补灵/收获全链路），植物图标也已生成（39 张 PNG）。但**玩家看到的是 vanilla 蕨类/蘑菇方块**，采收没有粒子，生长没有阶段渲染，灵田是光秃秃的泥地。本 plan 让灵材有灵气、灵田有生命。

---

## 接入面 Checklist（防孤岛）

- **进料**：`botany::PlantRegistry` ✅ / `botany::HarvestProgress` ✅ / `lingtian::LingtianPlot` ✅ / `lingtian::LingtianSessionState` ✅ / `BotanyPlantEntityRenderer` ✅ / `BotanyHudPlanner` ✅ / `vfx::VfxRegistry` ✅ / `audio::SoundRecipePlayer` ✅
- **出料**：client 植物渲染增强 → `BotanyPlantEntityRenderer` / VFX player → `VfxBootstrap` / audio recipe → `server/assets/audio/recipes/` / 灵田地块自定义渲染 → `LingtianPlotRenderer`
- **共享类型/event**：复用 `VfxEventRequest` / `AudioTriggerS2c` / `HarvestProgress` / `LingtianSessionState`，不新增 event
- **跨仓库契约**：server emit `VfxEventRequest(botany_harvest/botany_aura/lingtian_till)` → client VfxRegistry 消费
- **worldview 锚点**：§九 自给经济（灵田）/ §七 生态（植物与灵气共生）

---

## 阶段总览

| 阶段 | 目标 | 状态 |
|------|------|------|
| P0 | 灵草灵光 aura + 采收粒子 + 灵田动作音效接线 | ✅ |
| P1 | 植物生长阶段渲染 + 灵田地块自定义外观 | ✅ |
| P2 | 灵材品质视觉区分 + 灵田状态 overlay + 灵材放置展示 | ✅ |

---

## P0 — 灵草灵光 + 采收粒子 + 音效接线 ✅

### 交付物

1. **灵草 aura VFX**（`BotanyAuraPlayer.java`）
   - 灵气浓度 ≥ 0.5 的植物实体周围：4 颗 `BongSpriteParticle`（`qi_aura` 贴图，tint 按 `spirit_quality` 映射：0.3→淡绿 #88CC88 / 0.7→翠绿 #22FF44 / 1.0→金绿 #FFDD22）
   - 粒子缓慢上升 + 微弱左右飘动（sin 轨迹），lifetime 40-80 tick
   - server emit：`botany::growth_system` 每 200 tick 对 mature 植物 emit `VfxEventRequest::new("botany_aura", plant_pos)`

2. **采收粒子 VFX**（`BotanyHarvestBurstPlayer.java`）
   - 采收瞬间：叶片碎片粒子 × 12（`BongSpriteParticle` `enlightenment_dust` 贴图 tint 绿系，向上扩散 + 重力下落）
   - 稀有植物（epic+）额外金色光柱（复用 `BreakthroughPillarPlayer` 缩小版，高度 3 block，持续 1s）
   - server emit：`botany::harvest_system` 采收成功时 emit `VfxEventRequest::new("botany_harvest", plant_pos)` + rarity 参数

3. **灵田动作音效接线**
   - `harvest_pluck.json` / `till_soil.json` / `plot_replenish.json` 已存在但未接线
   - `server/src/lingtian/action_system.rs`：每个 action 完成时 emit `AudioTriggerS2c::new("{recipe_id}", player_local)`
   - 新增 `lingtian_plant_seed.json`（`minecraft:block.grass.place` pitch 1.2）/ `lingtian_drain.json`（`minecraft:block.pointed_dripstone.drip_lava` pitch 0.8 loop）

### 验收抓手

- 测试：`server::botany::tests::mature_plant_emits_aura_vfx` / `server::lingtian::tests::harvest_emits_audio`
- 手动：走到灵草旁 → 看到绿色浮光 → 采收 → 叶片碎裂粒子 + 采收音效

---

## P1 — 植物生长阶段渲染 + 灵田自定义外观 ✅

### 交付物

1. **生长阶段渲染**（增强 `BotanyPlantEntityRenderer`）
   - 3 个视觉阶段：seedling（quad 缩放 0.3 + 半透明 0.5）/ growing（缩放 0.7 + 微摆动 sin 动画）/ mature（缩放 1.0 + aura 粒子 P0 已有）
   - 阶段由 server 下发 `PlantGrowthStage { Seedling | Growing | Mature | Wilted }` 字段驱动
   - Wilted 状态：quad tint 灰化（saturation ×0.3）+ 无 aura

2. **灵田地块自定义外观**（`LingtianPlotRenderer.java`）
   - 灵田已开垦地块：用 `BongGroundDecalParticle` 在 farmland 表面绘制发光灵纹（`rune_char` 贴图 tint 淡青 #44CCCC，lifetime 永驻直到地块状态变化）
   - 地块状态视觉：空置=无灵纹 / 已种植=淡青灵纹 / 成熟=亮绿灵纹 + 上方 aura / 枯竭=灰色灵纹 + 裂缝 decal
   - 灵田补灵时：地面灵纹亮度脉动 2s（alpha 0.3→0.8→0.3）

3. **gen_plant_growth_stages.py**（`scripts/images/`）
   - 基于现有 39 张植物图标，批量生成 seedling/growing 两个阶段变体（缩放 + 透明度 + 色调偏移）
   - 输出到 `client/src/main/resources/assets/bong-client/textures/gui/botany/stages/`

### 验收抓手

- 测试：`client::botany::tests::growth_stage_renders_correctly` / `client::lingtian::tests::plot_rune_decal_updates`
- 手动：种下种子 → 看到小芽 → 逐渐长大 → 成熟发光 → 收获后灵纹变灰

---

## P2 — 品质视觉 + 灵田状态 overlay + 灵材放置 ✅

### 交付物

1. **灵材品质视觉区分**
   - 物品栏内植物图标按 `spirit_quality` 添加边框光晕：common 无 / uncommon 绿边 / rare 蓝边 / epic 紫边
   - tooltip 追加品质条（0.0-1.0 渐变色条）

2. **灵田状态 overlay HUD**
   - 靠近灵田时（5 格内）在 crosshair 旁显示迷你面板：地块状态 icon + 植物名 + 生长进度 % + 染污度
   - 复用 `BotanyHudPlanner` 已有框架，补齐 TODO 标注的圆角/植物图标

3. **灵材放置展示**
   - 灵材物品可右键放置在平面上作展示（类似 MC 物品展示框但使用植物 quad 渲染）
   - 放置后的灵材保留 aura 粒子（P0 复用）

### 验收抓手

- 测试：`client::inventory::tests::spirit_quality_border_renders` / `client::botany::tests::proximity_overlay_shows`
- 手动：打开背包 → rare 灵草有蓝色边框 → 靠近灵田 → 看到迷你面板 → 右键放置灵草 → 展示发光

---

## 前置依赖

| 依赖 plan | 状态 | 用到什么 |
|-----------|------|---------|
| plan-botany-v1 | ✅ finished | PlantRegistry / 22 v1 植物 / HarvestProgress |
| plan-botany-v2 | ✅ finished | 17 v2 植物 / render profile / SeasonRequired |
| plan-lingtian-v1 | ✅ finished | LingtianPlot / 6 动作 / LingtianSessionState |
| plan-vfx-v1 | ✅ finished | VfxRegistry / OverlayQuadRenderer |
| plan-particle-system-v1 | ✅ finished | BongSpriteParticle / BongGroundDecalParticle |
| plan-audio-v1 | ✅ finished | SoundRecipePlayer / recipe JSON schema |
| plan-HUD-v1 | ✅ finished | BongHudOrchestrator / BotanyHudPlanner |

**全部依赖已 finished，无阻塞。**

## Finish Evidence

- 实现提交：
  - `38868d5c8` `botany-visual: 接通灵草与灵田音画事件`
  - `a89e1b3cd` `botany-visual: 增强植物阶段渲染与粒子资产`
  - `2ea9d957f` `botany-visual: 补齐灵材品质与灵田状态 HUD`
- P0：
  - server 在 `botany::lifecycle` 每 200 tick 为高灵气成熟植物 emit `bong:botany_aura`。
  - server 在 `network::vfx_animation_trigger` 从 `HarvestTerminalEvent` emit `bong:botany_harvest`。
  - 灵田完成事件接入 `till_soil` / `lingtian_plant_seed` / `harvest_pluck` / `plot_replenish` / `lingtian_drain`，并新增对应 recipe JSON。
  - client `VfxBootstrap` 注册 `BotanyAuraPlayer`、`BotanyHarvestBurstPlayer`、`LingtianPlotRunePlayer`。
- P1：
  - `BotanyPlantV2Entity` 下发/持久化 `GrowthStage`，`BotanyPlantEntityRenderer` 按 seedling/growing/mature/wilted 调整 scale、alpha、sway、tint 和 emissive overlay。
  - `scripts/images/gen_plant_growth_stages.py` 基于 39 张植物图标生成 78 张 seedling/growing 阶段资产到 `client/src/main/resources/assets/bong-client/textures/gui/botany/stages/`。
  - 灵田动作地块灵纹通过 `BongGroundDecalParticle` 渲染，开垦/种植/补灵/收获/吸灵使用不同颜色与强度。
- P2：
  - `GridSlotComponent` 为灵材按 `spirit_quality` 绘制 uncommon/rare/epic 边框光晕。
  - `ItemTooltipPanel` 为灵材追加品质百分比与品质条。
  - `LingtianSessionHud` 在 active session 时于准星侧显示地块 mini panel，包含动作 icon、植物 id、进度与染污度；植物展示渲染复用 `BotanyPlantV2Entity` 的 quad/stage/aura 路径。
- 验证：
  - `python3 scripts/images/gen_plant_growth_stages.py --check`
  - `cd server && cargo test mature_plant_emits_aura_vfx_on_cadence`
  - `cd server && cargo test lingtian_actions_emit_dedicated_recipes`
  - `cd server && cargo test completed_botany_harvest_emits_leaf_burst_particle`
  - `cd server && cargo test lingtian_completion_events_emit_plot_rune_particles`
  - `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
  - `cd client && JAVA_HOME="/usr/lib/jvm/java-17-openjdk-amd64" ./gradlew test --tests '*BotanyPlantVisualStateTest' --tests '*BotanySpiritQualityVisualsTest' --tests '*LingtianPlotVisualStateTest' --tests '*VfxRegistryTest'`
  - `cd client && JAVA_HOME="/usr/lib/jvm/java-17-openjdk-amd64" ./gradlew test build`
