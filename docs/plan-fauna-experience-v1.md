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
| P0 | 噬元鼠 + 负压灵模型/VFX/音效（最高频遭遇） | ⬜ |
| P1 | 灰烬蛛 + 缝合兽 + 负压畸变体模型/VFX/音效 | ⬜ |
| P2 | 道伥/执念/守卫 TSY hostile 模型 + 全生物 spawn/death 通用 VFX | ⬜ |

---

## P0 — 噬元鼠 + 负压灵 ⬜

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

## P1 — 灰烬蛛 + 缝合兽 + 负压畸变体 ⬜

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

## P2 — TSY hostile 模型 + 通用 spawn/death VFX ⬜

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
