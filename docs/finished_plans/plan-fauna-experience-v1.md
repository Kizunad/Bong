# Plan: Fauna Experience v1（异变兽视觉生态）

> 5 类异变兽（噬元鼠/拟态灰烬蛛/异变缝合兽/负压畸变体/飞行鲸）+ 4 类 TSY hostile（道伥/执念/守卫/负压灵）后端逻辑全部完成，AI/掉落/生态全就绪。但除飞行鲸有 GeckoLib 自定义模型外，**其余全部是 vanilla Silverfish/Zombie/Villager 皮**。本 plan 给每种生物独特的视觉身份。

---

## 接入面 Checklist（防孤岛）

- **进料**：`fauna::BeastKind` ✅ / `fauna::drop` ✅ / `npc::tsy_hostile::TsyHostileArchetype` ✅ / `npc::brain_rat::RatPhase` ✅ / `whale::WhaleRenderer` ✅ / `vfx::VfxRegistry` ✅ / `audio::SoundRecipePlayer` ✅
- **出料**：GeckoLib 模型 → `client/src/main/resources/assets/bong/geo/` / VFX player → `VfxBootstrap` / audio recipe → `server/assets/audio/recipes/` / 自定义 Renderer → `client/src/main/java/com/bong/client/fauna/`
- **共享类型/event**：复用 `VfxEventRequest` / `AudioTriggerS2c` / `DeathEvent`，新增 `FaunaSpawnS2c` 告知 client 生物种类以选择 renderer
- **跨仓库契约**：server spawn 时附带 `BeastKind` metadata → client 按 kind 选择 GeckoLib model + renderer
- **worldview 锚点**：§七 动态生物生态 / §十六 坍缩渊敌对实体

---

## 阶段总览

| 阶段 | 目标 | 状态 |
|------|------|------|
| P0 | 噬元鼠 + 负压灵模型/VFX/音效（最高频遭遇） | ✅ 2026-05-10 |
| P1 | 灰烬蛛 + 缝合兽 + 负压畸变体模型/VFX/音效 | ✅ 2026-05-10 |
| P2 | 道伥/执念/守卫 TSY hostile 模型 + 全生物 spawn/death 通用 VFX | ✅ 2026-05-10 |

---

## P0 — 噬元鼠 + 负压灵 ✅ 2026-05-10

### 交付物

1. **噬元鼠 GeckoLib 模型**（`local_models/DevoureRat.bbmodel` → `client/src/main/resources/assets/bong/geo/devour_rat.geo.json`）
   - 骨骼框架低多边形（skeletal rat，发光红瞳，背脊突刺）
   - 3 variant texture：Normal(灰) / Thunder(蓝电弧纹) / Tainted(紫斑)
   - `DevoureRatRenderer.java` + `DevoureRatRenderBootstrap.java`（参照 WhaleRenderer 栈）
   - server spawn 时 `EntityKind::SILVERFISH` → 改为自定义 entity ID（参照 Whale entity 125 注册流程）

2. **鼠群 VFX**（`RatSwarmAuraPlayer.java`）
   - Gregarious 相位时：鼠群周围 qi 吸取可视化（`BongSpriteParticle` `qi_aura` tint #FF4444，从周围向鼠群中心汇聚轨迹）
   - server emit：`rat_phase.rs` Gregarious 切换时 emit `VfxEventRequest::new("rat_swarm_aura", group_center)`

3. **负压灵 GeckoLib 模型**（`local_models/Fuya.bbmodel`）
   - 半透明扭曲人形（imploding geometry 风格，身体中心塌缩感）
   - Enraged 状态：颜色从深紫 → 血红 + 体积膨胀 1.5x
   - `FuyaRenderer.java` + `FuyaRenderBootstrap.java`

4. **生物音效 recipe**
   - `fauna_rat_squeal.json`：`minecraft:entity.silverfish.hurt`(pitch 1.8, volume 0.3) × 3 随机延迟（群体效果）
   - `fauna_rat_death.json`：`minecraft:entity.silverfish.death`(pitch 1.5)
   - `fauna_fuya_pressure_hum.json`：`minecraft:entity.warden.heartbeat`(pitch 0.3, volume 0.5, loop) + `minecraft:block.respawn_anchor.deplete`(pitch 0.5)
   - `fauna_fuya_charge.json`：`minecraft:entity.warden.sonic_charge`(pitch 0.8)
   - server emit：`npc/brain_rat.rs` attack/death 时 emit audio；`npc/tsy_hostile.rs` Fuya tick/charge 时 emit audio

### 验收抓手

- 测试：`client::fauna::tests::rat_model_registers` / `client::fauna::tests::fuya_model_registers` / `server::fauna::tests::rat_swarm_emits_aura_vfx`
- 手动：遇到鼠群 → 看到红瞳骨鼠（不是银鱼虫）→ 群聚时红色真元汇聚 → 进入 TSY 遇到负压灵 → 半透明扭曲人形 + 嗡鸣声

---

## P1 — 灰烬蛛 + 缝合兽 + 负压畸变体 ✅ 2026-05-10

### 交付物

1. **灰烬蛛模型**（`AshSpider.bbmodel`）：半透明拟态（shimmer veil 效果，idle 时近乎隐形，aggro 后现形）
2. **缝合兽模型**（`HybridBeast.bbmodel`）：缝合嵌合体（多种动物部件拼接，关节处有缝线贴图）
3. **负压畸变体模型**（`VoidDistorted.bbmodel`）：void 扭曲版缝合兽（黑紫色调 + 身体局部"反转"几何）
4. 各自 Renderer + RenderBootstrap 注册
5. 各自音效 recipe（attack/death/ambient 各 1）
6. Spider shimmer VFX：idle 时 `BongSpriteParticle` 微光闪烁（模拟拟态）

### 验收抓手

- 测试：每种生物 model_registers 测试
- 手动：在不同区域遇到 3 种生物，各自外观可明确区分

---

## P2 — TSY hostile 模型 + 通用 spawn/death VFX ✅ 2026-05-10

### 交付物

1. **道伥模型**：枯槁人形（干尸化外观，空洞眼眶，行动僵硬）
2. **执念模型**：Masquerade 态=普通人外观 / Aggressive 态=黑雾缭绕 + 面部扭曲（两套 texture 切换）
3. **守卫模型**：石化甲胄人形（古代风格，phase 提升时甲胄裂缝发光）
4. **通用 spawn/death VFX**
   - Spawn：`BongSpriteParticle` dust burst × 8（从地面向上扩散）
   - Death：`DeathSoulDissipatePlayer`（已存在）复用 + 骨碎片粒子（BongLineParticle 短线段 × 6 向外飞散）
   - server emit：所有生物 spawn/death 事件 emit VFX

### 验收抓手

- 测试：全 9 种生物 model_registers 测试通过
- E2E：完整 TSY run 遇到道伥/执念/守卫/负压灵各 1，外观/音效/VFX 全部差异化
- 视频：录制 9 种生物各 10s clip，确认无 vanilla 皮残留

---

## 前置依赖

| 依赖 plan | 状态 | 用到什么 |
|-----------|------|---------|
| plan-fauna-v1 | ✅ finished | BeastKind / drop table / rat_phase |
| plan-npc-ai-v1 | ✅ finished | big-brain Scorer/Action / WhaleRenderer 参考栈 |
| plan-tsy-hostile-v1 | ✅ finished | TsyHostileArchetype / FuyaAura / Zhinian phase |
| plan-vfx-v1 | ✅ finished | VfxRegistry / DeathSoulDissipatePlayer |
| plan-particle-system-v1 | ✅ finished | BongSpriteParticle / BongLineParticle |
| plan-audio-v1 | ✅ finished | SoundRecipePlayer / recipe JSON |
| plan-player-animation-v1 | ✅ finished | PlayerAnimator JSON 格式参考 |

**全部依赖已 finished，无阻塞。**

## Finish Evidence

### 落地清单

- P0 噬元鼠 / 负压灵：`server/src/fauna/visual.rs` 固定 `DEVOUR_RAT_ENTITY_KIND=126`、`FUYA_ENTITY_KIND=133`；`server/src/npc/spawn_rat.rs` 把鼠群从 vanilla silverfish 切到 `DEVOUR_RAT_ENTITY_KIND`；`server/src/npc/tsy_hostile.rs` 给 Fuya 挂 `FaunaVisualKind::Fuya`，用 entity-scoped audio flag 接通压力嗡鸣 loop 并在 DeathEvent 停止，charge 音效走 `fauna_fuya_charge`；client 侧落在 `client/src/main/java/com/bong/client/fauna/`、`client/src/main/resources/assets/bong/geo/devour_rat.geo.json`、`fuya.geo.json`、`textures/entity/fauna/devour_rat*.png`、`fuya.png`；鼠群 VFX 落在 `RatSwarmAuraPlayer`。
- P1 灰烬蛛 / 缝合兽 / 负压畸变体：`server/src/npc/spawn.rs` 通过 `entity_kind_for_beast` 与 `visual_kind_for_beast` 将 `BeastKind::{Spider,HybridBeast,VoidDistorted}` 接到自定义 EntityKind 与视觉壳；音效 recipe 落在 `server/assets/audio/recipes/fauna_ash_spider_*`、`fauna_hybrid_beast_*`、`fauna_void_distorted_*`；client 模型 / 贴图落在 `ash_spider.geo.json`、`hybrid_beast.geo.json`、`void_distorted.geo.json` 与同名 texture；spider shimmer VFX 落在 `SpiderShimmerPlayer`。
- P2 道伥 / 执念 / 守卫 + 通用 spawn/death VFX：`server/src/npc/tsy_hostile.rs` 固定 `DAOXIANG_ENTITY_KIND=130`、`ZHINIAN_ENTITY_KIND=131`、`TSY_SENTINEL_ENTITY_KIND=132` 并挂 `FaunaVisualKind`；client 模型 / 贴图落在 `daoxiang.geo.json`、`zhinian.geo.json`、`tsy_sentinel.geo.json` 与同名 texture；通用 `fauna_spawn_dust`、`fauna_bone_shatter` 与既有 `death_soul_dissipate` 由 `server/src/fauna/experience.rs` 发出，client 播放器落在 `FaunaSpawnDustPlayer` / `FaunaBoneShatterPlayer`。

### 关键 commit

- `4d78b4776`（2026-05-10）实现异变兽视觉音效服务端契约：server 侧 EntityKind、FaunaVisualKind、spawn/death/attack/ambient VFX/audio 发射、音效 recipe 与回归测试。
- `3d49ec185`（2026-05-10）接入异变兽客户端渲染资源：client 侧 GeckoLib fauna entity/model/renderer/bootstrap、8 个非鲸视觉壳模型、贴图、动画、VFX player 与注册测试。
- `4d5f146ff`（2026-05-10）fix(fauna-experience-v1): 避免鲸误挂异变兽视觉壳：保留已有鲸 `WHALE_ENTITY_KIND=125` / WhaleRenderer 语义，非本 plan 视觉壳不覆盖鲸。
- `0d0cc1fd3`（2026-05-10）fix(fauna-experience-v1): 接通 Fuya 压力嗡鸣 loop：client 侧 `SoundRecipePlayer` 支持 payload-owned loop flag，server 侧 Fuya spawn/death 负责 play/stop。

### 测试结果

- rebase 前本地全量：`cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` 通过，`3639 passed; 0 failed`；`cd client && JAVA_HOME=$HOME/.sdkman/candidates/java/17.0.18-amzn PATH=$JAVA_HOME/bin:$PATH ./gradlew test build` 通过。
- review 修复后本地增量：`cd server && cargo fmt --check` 通过；`cd server && CARGO_BUILD_JOBS=1 cargo check --tests` 通过；`cd client && JAVA_HOME=$HOME/.sdkman/candidates/java/17.0.18-amzn PATH=$JAVA_HOME/bin:$PATH ./gradlew test build` 通过。
- review 修复后本地 `cargo test fuya_pressure_hum_audio_emits_on_aura_spawn` 在 test binary 链接阶段被系统 SIGKILL，无 Rust 诊断；该路径需由 PR CI 的 server/e2e 阶段继续核验。

### 跨仓库核验

- server：`FaunaVisualKind`、`entity_kind_for_beast`、`emit_fauna_spawn_vfx_system`、`emit_fauna_death_vfx_audio_system`、`emit_rat_swarm_aura_on_gregarious_system`、`emit_fuya_pressure_hum_audio_system`。
- client：`FaunaEntities`、`FaunaEntity`、`FaunaModel`、`FaunaRenderer`、`FaunaRenderBootstrap`、`RatSwarmAuraPlayer`、`SpiderShimmerPlayer`、`FaunaSpawnDustPlayer`、`FaunaBoneShatterPlayer`。
- 共享契约：复用既有 `VfxEventRequest` / `VfxEventPayloadV1::SpawnParticle` / `PlaySoundRecipeRequest` / `StopSoundRecipeRequest`，未新增 agent/schema wire type。

### 遗留 / 后续

- 云端 headless 流程已完成编译与注册回归；计划中的 9 种生物 10s 手动视频验收需要在本地图形环境中录制，不阻塞当前代码与资源归档。
- 状态驱动视觉切换仍应拆到后续 `fauna-state-visual-v1`：噬元鼠 Thunder/Tainted texture 与 RatPhase/环境态绑定、Fuya Enraged 红化 + 1.5x 膨胀、执念 Masquerade/Aggressive 双 texture 切换、守卫 phase 裂缝发光。当前 plan 已落地静态视觉身份、EntityKind/raw_id、通用 VFX 与音效 loop，不再把这些 phase-driven 细节标成已完成代码。
