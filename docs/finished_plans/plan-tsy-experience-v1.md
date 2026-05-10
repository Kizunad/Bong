# Plan: TSY Experience v1（坍缩渊体验层补全）

> 坍缩渊(搜打撤)服务端逻辑已 100% 完成——入场/搜刮/坍缩/撤离/race-out 全链路跑通。本 plan 补齐**玩家体验层**：portal 模型与 VFX、坍缩级联视觉与音效、race-out 警报、上古遗物发光、容器搜刮动画、负压视觉反馈、narration 触发接线。目标：让 TSY 从"一个看起来像 vanilla MC 的维度"变成"末法残土最危险的地方"。

---

## 接入面 Checklist（防孤岛）

- **进料**：`tsy::RiftPortal` ✅ / `tsy::TsyCollapseStarted` ✅ / `tsy::ExtractProgressS2c` ✅ / `tsy::SearchProgressS2c` ✅ / `tsy::DroppedLootSyncHandler` ✅ / `tsy_hostile::FuyaAura` ✅ / `audio::SoundRecipePlayer` ✅ / `vfx::VfxRegistry` ✅
- **出料**：client 侧 VFX player 注册 → `VfxBootstrap` / audio recipe JSON → `server/assets/audio/recipes/` / 模型资产 → `client/src/main/resources/assets/bong/models/` / HUD overlay 增强 → `ExtractProgressHudPlanner`
- **共享类型/event**：复用 `VfxEventRequest` / `AudioTriggerS2c` / `TsyCollapseStarted` / `ExtractProgressS2c`，不新增 event
- **跨仓库契约**：server emit `VfxEventRequest(tsy_portal_idle/tsy_collapse_burst/tsy_fuya_aura)` → client `VfxRegistry` 消费；server emit `AudioTriggerS2c(tsy_*)` → client `SoundRecipePlayer` 播放
- **worldview 锚点**：§十六 活坍缩渊（负压/脆化/真元逸散）/ §十 搜打撤循环 / §十二 死亡惩罚

---

## 阶段总览

| 阶段 | 目标 | 状态 |
|------|------|------|
| P0 | Portal VFX + 坍缩视觉 + race-out 音效 | ⬜ |
| P1 | 负压视觉反馈 + Fuya 光环 + 上古遗物发光 | ⬜ |
| P2 | 容器搜刮动画 + narration 触发接线 + 高手死处 zone 补全 | ⬜ |

---

## P0 — Portal VFX + 坍缩视觉 + race-out 音效 ⬜

### 交付物

1. **Portal VFX Player**（`client/src/main/java/com/bong/client/visual/particle/TsyPortalVortexPlayer.java`）
   - MainRift：蓝紫涡旋粒子环（BongRibbonParticle 16帧环形路径 + BongSpriteParticle qi_aura 贴图 tint #6644AA）
   - DeepRift：深红涡旋（tint #AA2222，旋转速度 ×1.5）
   - CollapseTear：白闪 + 不稳定抖动（position jitter ±0.3 block，lifetime 随机 10-20 tick）
   - 注册：`VfxBootstrap` 添加 `tsy_portal_idle` / `tsy_portal_deep` / `tsy_portal_tear` 三个 event ID

2. **坍缩级联 VFX**（`TsyCollapseBurstPlayer.java`）
   - `TsyCollapseStarted` 触发：全屏红闪 `OverlayQuadRenderer` 0.5s fade（`0x44FF0000`）+ 地面裂缝 `BongGroundDecalParticle` rune_char 贴图 × 20 随机分布
   - 注册：`tsy_collapse_burst` event ID

3. **Race-out 音效 recipe**（`server/assets/audio/recipes/`）
   - `tsy_race_out_alarm.json`：层叠 `minecraft:block.note_block.pling`(pitch 0.5→2.0 每秒递增) + `minecraft:entity.warden.heartbeat`(volume 0.8, loop)
   - `tsy_collapse_rumble.json`：`minecraft:entity.ender_dragon.growl`(pitch 0.3, volume 0.6) + `minecraft:block.anvil.land`(delay 500ms)
   - `tsy_extract_success.json`：`minecraft:entity.player.levelup`(pitch 1.5, volume 0.4)

4. **Server VFX/Audio emit 接线**
   - `server/src/tsy/extract_system.rs`：portal spawn 时 emit `VfxEventRequest::new("tsy_portal_idle", portal_pos)`
   - `server/src/tsy/lifecycle.rs`：`TsyCollapseStarted` handler 追加 emit `VfxEventRequest::new("tsy_collapse_burst", zone_center)` + `AudioTriggerS2c::new("tsy_race_out_alarm", zone_broadcast)`
   - `server/src/tsy/extract_system.rs`：extract 成功时 emit `AudioTriggerS2c::new("tsy_extract_success", player_local)`

### 验收抓手

- 测试：`client::visual::tests::tsy_portal_vortex_registers` / `server::tsy::tests::collapse_emits_vfx_and_audio`
- 手动：进入 TSY → 看到 portal 涡旋粒子 → 触发坍缩 → 红闪 + 地裂 + 警报音 → 成功撤离 → 听到 extract_success

---

## P1 — 负压视觉反馈 + Fuya 光环 + 上古遗物发光 ⬜

### 交付物

1. **负压视觉反馈**（`client/src/main/java/com/bong/client/visual/TsyPressureOverlay.java`）
   - 根据 `spirit_qi` 负值强度渐变 screen vignette：-0.4 淡紫边缘 → -1.1 深紫压迫 + 视野微缩（FOV ×0.95）
   - 接入 `BongHudOrchestrator` 作为 VisualEffect layer

2. **Fuya 光环 VFX**（`TsyFuyaAuraPlayer.java`）
   - 8 格半径球形扰动粒子（BongSpriteParticle `qi_aura` tint #220044，向中心吸引轨迹）
   - Fuya 移动时粒子跟随；EnrageMarker 激活后颜色切换 #FF0044 + 粒子加速
   - server emit：`server/src/npc/tsy_hostile.rs` Fuya tick 时 emit `VfxEventRequest::new("tsy_fuya_aura", fuya_pos)`

3. **上古遗物发光**（`client/src/main/java/com/bong/client/inventory/AncientRelicGlowRenderer.java`）
   - 物品栏中 `ItemRarity::Ancient` 物品：tooltip 边框脉动发光（#FFD700 → #FF8800 呼吸循环 2s）
   - 3D 掉落物：附加 `BongSpriteParticle` 反光粒子环（6 颗粒子绕轴旋转）
   - 充能次数显示：tooltip 底部 `⚡ ×3` 格式

### 验收抓手

- 测试：`client::visual::tests::pressure_overlay_scales_with_qi` / `client::inventory::tests::ancient_relic_glow_renders`
- 手动：进入 TSY deep tier → 屏幕边缘变紫 → 靠近 Fuya 看到吸引粒子 → 拾取上古遗物 → 物品栏闪金光

---

## P2 — 容器搜刮动画 + narration 接线 + 高手死处补全 ⬜

### 交付物

1. **容器搜刮 VFX**
   - 搜刮开始：容器位置冒 dust 粒子（`BongSpriteParticle` cloud256_dust × 8）
   - 搜刮完成：物品弹出动画（粒子从容器 pos → 玩家 pos 抛物线轨迹）
   - 音效：`tsy_search_scrape.json`（`minecraft:block.gravel.break` + `minecraft:item.armor.equip_chain`）

2. **Narration 触发接线**
   - `TsyCollapseStarted` → tiandao agent `calamity.md` race-out prompt 触发（新增 `ZoneStatusV1::RaceOut` variant → ContextAssembler 消费）
   - 首次入场 → tiandao `insight.md` TSY 首次进入 prompt

3. **高手死处 zone 补全**
   - `zones.tsy.json` 新增 `tsy_gaoshou_01` family（shallow/mid/deep 三层）
   - 补充 POI：2 loot_container + 1 npc_anchor + 1 relic_core_slot

### 验收抓手

- 测试：`server::tsy::tests::gaoshou_zone_loads` / `agent::tests::raceout_narration_triggers`
- E2E：完整 TSY 跑一轮（入场 → 搜刮 → 坍缩 → race-out / 死亡两路径），全程视觉/音效/narration 无缺失

---

## 前置依赖

| 依赖 plan | 状态 | 用到什么 |
|-----------|------|---------|
| plan-tsy-v1 | ✅ finished | RiftPortal / TsyPresence / extract_system |
| plan-tsy-worldgen-v1 | ✅ finished | 4 terrain profile / POI consumer / dimension infra |
| plan-tsy-extract-v1 | ✅ finished | ExtractProgressS2c / SearchProgressS2c |
| plan-tsy-container-v1 | ✅ finished | RelicExtracted event / container archetypes |
| plan-tsy-lifecycle-v1 | ✅ finished | TsyCollapseStarted / collapse flow |
| plan-tsy-hostile-v1 | ✅ finished | FuyaAura / Daoxiang / Zhinian / Sentinel |
| plan-tsy-raceout-v1 | ✅ finished | CollapseTear portal / 3s extract / race-out HUD |
| plan-tsy-loot-v1 | ✅ finished | DroppedLootSyncHandler / ItemRarity::Ancient |
| plan-vfx-v1 | ✅ finished | VfxRegistry / VfxEventRequest / VfxPlayer trait |
| plan-particle-system-v1 | ✅ finished | BongSpriteParticle / BongRibbonParticle / BongGroundDecalParticle |
| plan-audio-v1 | ✅ finished | SoundRecipePlayer / AudioTriggerS2c / recipe JSON schema |

**全部依赖已 finished，无阻塞。**
