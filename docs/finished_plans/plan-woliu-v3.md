# Bong · plan-woliu-v3 · 涡流流五招完整包

涡流功法五招完整包：动画 / 粒子 / 音效 / 伤害 / 真元消耗 / HUD / 全流程。承接 `plan-woliu-v1` ✅ finished（P0 基础真空吸入已实装）与 `plan-woliu-v2` ✅ finished（持涡 / 瞬涡 / 涡口 / 涡引 / 涡心已归档）—— v3 引入**真空场物理**（半径 r 内负压区 → 目标被拉向施法者 → 真元从目标向施法者逆流）+ **涡流共振**（多目标时涡流叠加增益）+ **紊流爆发**（蓄力后释放真空场碎裂为物理冲击波），五招完整规格。**严守 worldview §五 涡流"以空制有"哲学**。

**世界观锚点**：`worldview.md §五:440-455 涡流核心`（展掌开涡 / 真空吸扯 / 以空制有 / 紊流窒息）· `§四:500 零距离贴脸施法`（涡流的"吸"让距离主动缩短）· `§五:460 涡流真空吸音`（施法时周围声音被吸走——音效设计依据）· `§P 异体排斥`（涡流 ρ 中等，靠真空负压而非注入突破防御）

**library 锚点**：`cultivation-0004 涡流散人手札`（真空场与灵气压差的关系）

**前置依赖**：
- `plan-skill-v1` ✅ + `plan-hotbar-modify-v1` ✅ → SkillRegistry / Casting / cooldown
- `plan-combat-no_ui` ✅ + `plan-combat-ui_impl` ✅ → 战斗事件垫
- `plan-multi-style-v1` ✅ → QiColor / PracticeLog / StyleAttack trait
- `plan-qi-physics-v1` ✅ → qi_collision / field / constants
- `plan-vfx-v1` ✅ → 粒子基类 / 屏幕效果
- `plan-particle-system-v1` ✅ → BongSpriteParticle / BongLineParticle
- `plan-audio-v1` ✅ → SoundRecipePlayer / AudioTriggerS2c
- `plan-HUD-v1` ✅ → BongHudOrchestrator
- `plan-cultivation-v1` ✅ → Realm / Cultivation / Meridian
- `plan-meridian-severed-v1` ✅ → SkillMeridianDependencies

**反向被依赖**：
- `plan-style-balance-v1` → 5 招数值进平衡矩阵
- `plan-audio-implementation-v1` → 涡流流 5 条专属音效 recipe

---

## 接入面 Checklist

- **进料**：`cultivation::Cultivation` / `qi_physics::field` / `SkillRegistry` / `SkillSet` / `Casting` / `PracticeLog` / `Realm` / `combat::CombatState`
- **出料**：5 招 `WoliuSkillId` enum 注册到 SkillRegistry / server `combat::woliu_v2::*` 模块 / client 5 动画 + `VortexSpiralPlayer` 5 路粒子 + 5 音效 recipe + `woliu_state_emit` HUD overlay / `VortexCastEvent` / `TurbulenceFieldSpawned` / `EntityDisplacedByVortexPull` / `QiTransfer` / `ApplyStatusEffectIntent` / `CombatEvent`
- **跨仓库契约**：server `combat::woliu_v2::*` → client `bong:woliu_v2/cast` + `bong:vfx_event` + `bong:audio/play` / agent `tiandao::woliu_v2_runtime` narration
- **qi_physics 锚点**：涡流 ρ=0.35（中等排斥——不靠注入而是靠负压）沿用 `combat::woliu_v2::physics::{stir_99_1,pull_displacement_blocks}`、`TurbulenceField`、`TurbulenceExposure` 与 `QiTransfer` ledger；不在 plan 内另写独立 physics 子系统。
- **经脉依赖声明**：`SkillMeridianDependencies::declare(woliu_*, vec![手太阴肺经, 手少阴心经])` — 涡流依赖呼吸+心脉控制真空

---

## §0 设计轴心

- [x] **以空制有**：涡流不"打出去"——它"吸进来"。五招均以 `combat::woliu_v2::VortexCastEvent` / `TurbulenceField` 的负压中心、吸扯位移、真元逆流或紊流外爆表达。
- [x] **反向 shake**：server 已输出 caster/center/particle contract，client 通过 `VortexSpiralPlayer` 对五招使用 inward spiral / burst wave 视觉；更细的 camera shake 留给后续 client polish。
- [x] **真空吸音**：五招均注册 AMBIENT 类 `SoundRecipe`，由现有 `SoundRecipePlayer` 的 combat ambient ducking 与专属低频 recipe 表达真空吸音。
- [x] **淡紫色调**：`vfx_animation_trigger::color_for_woliu_skill` + 五招粒子 id 统一走涡流淡紫视觉路径。
- [x] **每招差异化视听**：5 招均有独立 skill id、animation id、particle id、sound recipe id、HUD state overlay / icon texture contract。

---

## 五招规格

### 招式一：吸涡掌（Vacuum Palm）
- **定位**：基础攻击，单目标近距吸引+真元逆流
- **机制**：展掌 → 8 格内单目标被拉向施法者 2 block/s × 1.5s → 接触时真元逆流（从目标吸取 qi 15 点 → 施法者回复）
- **真元消耗**：20
- **冷却**：3s
- **动画**：单掌前推 → 掌心朝目标 → 手指微张（`woliu_vacuum_palm.json`，UPPER_BODY 6 tick）
- **粒子**：目标→施法者 淡紫色螺旋线（`BongLineParticle` 螺旋轨迹 × 4，从目标向掌心汇聚）
- **音效**：`woliu_vacuum_palm.json`（`minecraft:entity.enderman.teleport`(pitch 0.5, volume 0.3) + 周围环境音 ducking 0.5s）
- **HUD**：目标被吸时其位置出现淡紫箭头指向施法者（2s）

### 招式二：涡流护体（Vortex Shield）
- **定位**：防御技，身周真空层偏转来袭攻击
- **机制**：开启后 5s 内身周 2 格真空场 → 远程投射物偏转（命中率 -60%）+ 近战攻击者被微推 1 block → 持续消耗真元 5/s
- **真元消耗**：25 初始 + 5/s 持续
- **冷却**：12s
- **动画**：双掌环抱 → 缓慢旋转（`woliu_vortex_shield.json`，FULL_BODY loop）
- **粒子**：身周 2 格半透明淡紫球面（`BongSpriteParticle` 球面分布 × 16 持续旋转 + 偶发偏转闪光）
- **音效**：`woliu_vortex_shield.json`（`minecraft:block.portal.ambient`(pitch 2.0, volume 0.08) loop — 低频嗡鸣持续）
- **HUD**：真元条旁出现紫色"护体"小 icon + 持续时间倒计时弧线（auto-hide 5s 后消失）

### 招式三：真空锁（Vacuum Lock）
- **定位**：控制技，锁定目标移动
- **机制**：指定 12 格内目标 → 目标周围形成真空笼（3s 内移速 -80% + 无法跳跃）→ 被锁目标真元逸散加速（qi_drain ×2）
- **真元消耗**：35
- **冷却**：15s
- **动画**：双手合十 → 猛然张开（`woliu_vacuum_lock.json`，UPPER_BODY 8 tick）
- **粒子**：目标位置出现淡紫色笼状线框（`BongLineParticle` 球形线框 radius 1.5 block × 12 线条，lifetime 60 tick loop）
- **音效**：`woliu_vacuum_lock.json`（`minecraft:entity.generic.drink`(pitch 0.5, volume 0.4) — 真空吸力声 + 目标处环境音压低 3s）
- **HUD**：被锁目标头顶出现紫色锁链 icon（对双方可见）+ 施法者 HUD 显示剩余锁定时间

### 招式四：涡流共振（Vortex Resonance）
- **定位**：群体技，多目标涡流叠加
- **机制**：以施法者为中心 6 格球形 → 区域内所有敌对目标同时被轻微拉向中心（1 block/s）+ 每增加 1 个目标涡流强度 +20%（3 目标时 pull 1.6 block/s）→ 持续 4s
- **真元消耗**：50
- **冷却**：20s
- **动画**：双臂缓慢展开 → 掌心朝天 → 身体微浮 0.2 block（`woliu_vortex_resonance.json`，FULL_BODY 80 tick loop）
- **粒子**：施法者为中心的淡紫色涡旋平面（`VortexSpiralPlayer` 的 `bong:woliu_vortex_resonance_field` 路由，按 turbulence radius 输出向内收缩粒子）
- **音效**：`woliu_vortex_resonance.json`（`minecraft:entity.warden.sonic_boom` + `minecraft:block.portal.ambient` — 低频共振+嗡鸣叠加）
- **HUD**：施法者脚下出现紫色涡旋 indicator（范围 6 格圆）+ 每个被拉目标出现紫色←箭头

### 招式五：紊流爆发（Turbulence Burst）
- **定位**：终结技，真空场碎裂为物理冲击波
- **机制**：需要先 charge 2s（期间移速 -50% + 身周形成真空场）→ 释放：6 格球形冲击波（伤害 60 + 击退 4 block + 被击中目标 1s 眩晕）+ 施法者自身后退 2 block（反冲力）
- **真元消耗**：80
- **冷却**：30s
- **动画**：charge 阶段双掌合拢收紧 → release 双掌猛然推出 + 身体后仰（`woliu_turbulence_burst.json`，FULL_BODY 40 tick）
- **粒子**：charge 期间周围粒子向内收缩（环境粒子被"吸入"） → release 瞬间淡紫色球形冲击波向外扩散（`BongSpriteParticle` 球面 × 32 快速向外 + `BongLineParticle` 径向线条 × 8）
- **音效**：`woliu_turbulence_burst.json`（charge 低频聚拢 + release `minecraft:entity.generic.explode`(pitch 1.5, volume 0.5) — "真空碎裂的爆响"）
- **HUD**：charge 期间屏幕边缘微收紧（vignette 淡紫）+ release 时全屏白闪 0.1s + camera 向后 shake

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|----|------|----|
| P0 | server `combat::woliu_v2` 模块骨架 + SkillRegistry 注册 5 招 + 吸涡掌 server 全实装 + qi_physics 真空场 API 接入 + 经脉依赖声明 + 30 单测 | ✅ 2026-05-11 |
| P1 | 涡流护体 + 真空锁 server 实装 + client 前 3 招动画/粒子/音效/HUD 全流程 | ✅ 2026-05-11 |
| P2 | 涡流共振 + 紊流爆发 server 实装 + client 后 2 招动画/粒子/音效/HUD 全流程 | ✅ 2026-05-11 |
| P3 | 环境音 ducking 系统（涡流施法时周围声音被吸走）+ HUD auto-hide 策略 + 全 5 招视听 polish | ✅ 2026-05-11 |
| P4 | agent narration 接线（天道对涡流的评价模板）+ 5 招 × 全境界 × 全距离 饱和化测试 | ✅ 2026-05-11 |

---

## Finish Evidence

- **落地清单**：
  - server：`server/src/combat/woliu_v2/events.rs` / `skills.rs` / `tests.rs` 注册 `woliu.vacuum_palm`、`woliu.vortex_shield`、`woliu.vacuum_lock`、`woliu.vortex_resonance`、`woliu.turbulence_burst`；`VacuumPalm` 目标吸扯+15 qi siphon；`VortexShield` 输出 `DamageReduction`；`VacuumLock` 目标减速+真元抽离；`VortexResonance` 多目标按目标数增强吸扯；`TurbulenceBurst` 造成 60 点震荡伤害、击退和 1s stun。
  - server 契约：`server/src/schema/woliu_v2.rs`、`server/src/network/woliu_event_bridge.rs`、`server/src/network/vfx_animation_trigger.rs`、`server/src/audio/mod.rs`、`server/src/cultivation/known_techniques.rs` 已接入 v3 skill enum、Redis payload、VFX/audio trigger、5 个音效 recipe、Lung+Heart 经脉依赖。
  - agent：`agent/packages/schema/src/woliu_v2.ts` / `tests/woliu_v2.test.ts` 和 `agent/packages/tiandao/src/woliu_v2_runtime.ts` / `tests/woliu_v2_runtime.test.ts` 已识别五个 v3 skill id 并生成 narration label。
  - client：`client/src/main/java/com/bong/client/visual/particle/VortexSpiralPlayer.java`、`VfxBootstrap.java`、`VfxRegistryTest.java` 注册五个 v3 particle id；`client/src/main/resources/assets/bong/player_animation/woliu_*.json` 提供五个动画；`server/assets/audio/recipes/woliu_*.json` 提供五个音效。
- **关键 commit**：
  - `7e3eec26c`（2026-05-11）`plan-woliu-v3: 接入涡流真空五招`
  - `442e4a90d`（2026-05-11）`plan-woliu-v3: 补齐真空五招运行效果`
- **测试结果**：
  - server：`cargo fmt --check` ✅；`CARGO_BUILD_JOBS=1 cargo clippy --all-targets -- -D warnings` ✅；`CARGO_BUILD_JOBS=1 cargo test -j 1` ✅ 4023 passed。
  - server targeted：`CARGO_BUILD_JOBS=1 cargo test -j 1 combat::woliu_v2` ✅ 144 passed；`cargo test loads_default_audio_recipes` ✅ 1 passed；`cargo test woliu_v3_techniques_require_breath_and_heart_meridians` ✅ 1 passed。
  - agent：`npm run build` ✅；`npm run generate:check -w @bong/schema` ✅ 338 generated schema files fresh；`npm test -w @bong/schema` ✅ 358 passed；`npm test -w @bong/tiandao -- woliu_v2_runtime` ✅ 3 passed。
  - client：`JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64 ./gradlew test build` ✅ BUILD SUCCESSFUL。
  - animation render：`/tmp/woliu_v3_anim_vacuum_palm/woliu_vacuum_palm_grid.png`、`/tmp/woliu_v3_anim_vortex_shield/woliu_vortex_shield_grid.png`、`/tmp/woliu_v3_anim_vacuum_lock/woliu_vacuum_lock_grid.png`、`/tmp/woliu_v3_anim_vortex_resonance/woliu_vortex_resonance_grid.png`、`/tmp/woliu_v3_anim_turbulence_burst/woliu_turbulence_burst_grid.png`。
- **跨仓库核验**：server `WoliuSkillId::{VacuumPalm,VortexShield,VacuumLock,VortexResonance,TurbulenceBurst}` ↔ agent `WoliuSkillCastV1["skill"]` literal set ↔ client `VortexSpiralPlayer.{VACUUM_PALM,VORTEX_SHIELD,VACUUM_LOCK,VORTEX_RESONANCE,TURBULENCE_BURST}`。
- **遗留 / 后续**：涡流与其他流派组合（吸+毒、吸+爆等）进入 `style-balance-v1`；本 plan 复用现有涡流 icon texture，后续视觉 polish 可替换为五招专属图标。
