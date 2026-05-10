# Plan: VFX Wiring v1（修炼/战斗/产出 VFX 全系统接线）

> 粒子引擎已完整（7 基础几何类 + 18 VFX player + 30 张粒子贴图 + server VFX emit 管线）。但**大量游戏事件没有触发 VFX**——forge 锤击没有火星、alchemy 炼制没有蒸汽、修炼吸灵没有灵气流、战斗命中没有方向指示、阵法激活没有符文光。本 plan 把现有粒子引擎接到每个游戏系统的关键事件上。

---

## 接入面 Checklist（防孤岛）

- **进料**：`vfx::VfxEventRequest` ✅ / `vfx::VfxRegistry` ✅ / 18 existing VFX players ✅ / `forge::ForgeSessionState` ✅ / `alchemy::BrewSessionState` ✅ / `cultivation::BreakthroughRequest` ✅ / `combat::DamageEvent` ✅ / `zhenfa::FormationActivateEvent` ✅ / `lingtian::LingtianActionEvent` ✅
- **出料**：server 各模块追加 `VfxEventRequest` emit 调用 / 新增 VFX player → `VfxBootstrap` 注册 / 新增粒子贴图（如需）→ `client/src/main/resources/assets/bong/textures/particle/`
- **共享类型/event**：不新增 event。仅在现有事件 handler 中追加 VFX emit
- **跨仓库契约**：server 各模块 emit `VfxEventRequest(event_id, pos, params)` → client `VfxRegistry` 消费
- **worldview 锚点**：§五 流派视觉 / §三 修炼过程 / §八 阵法

---

## 阶段总览

| 阶段 | 目标 | 状态 |
|------|------|------|
| P0 | 修炼 VFX（吸灵/经脉/突破）+ 战斗 VFX（命中/格挡） | ✅ 2026-05-10 |
| P1 | 产出 VFX（forge/alchemy/lingtian）+ 阵法 VFX | ✅ 2026-05-10 |
| P2 | 社交 VFX + 死亡 VFX + 状态效果 VFX | ✅ 2026-05-10 |

---

## P0 — 修炼 VFX + 战斗 VFX ✅ 2026-05-10

### 交付物

1. **修炼吸灵 VFX**（`CultivationAbsorbPlayer.java`）
   - 打坐时：周围 8 格内灵气粒子（`BongSpriteParticle` `lingqi_ripple` tint 按 zone 灵气浓度映射）向玩家汇聚
   - 粒子密度 = 灵气浓度 × 10（高浓度区粒子多）
   - server emit：`cultivation::meditate_system` 每 40 tick emit `VfxEventRequest::new("cultivation_absorb", player_pos)` with `spirit_qi` param

2. **经脉打通 VFX**（`MeridianOpenFlashPlayer.java`）
   - 经脉打通瞬间：玩家身体对应位置闪光线（`BongLineParticle` 从打通经脉对应的身体部位 → 丹田方向，#22FFAA，lifetime 20 tick）
   - 多条经脉同时打通时线条叠加
   - server emit：`cultivation::meridian_system` 经脉打通时 emit `VfxEventRequest::new("meridian_open", player_pos)` with `meridian_id` param
   - 已有 `MeridianOpenHudPlanner` → VFX 与 HUD 同步触发

3. **突破 VFX 增强**
   - `BreakthroughPillarPlayer` 已存在 → 确认 server breakthrough 成功时 emit
   - 追加突破失败 VFX：红色碎裂粒子环（`BongSpriteParticle` × 16 向外扩散 + 地面裂缝 decal 3s 消散）
   - 新增 `breakthrough_fail` VFX event ID

4. **战斗命中 VFX**（`CombatHitDirectionPlayer.java`）
   - 攻击命中时：被攻击方向红色弧线（`BongLineParticle` arc 90° 扇面，lifetime 10 tick）
   - 格挡成功：蓝色火花（`BongSpriteParticle` `tribulation_spark` tint #4488FF × 6 向外飞散）
   - server emit：`combat::resolve` 命中时 emit `VfxEventRequest::new("combat_hit", target_pos)` with `direction` param；格挡时 emit `"combat_parry"`

5. **战斗命中音效接线确认**
   - `parry_clang.json` 已存在 → 确认 server combat parry 时 emit audio
   - `cast_interrupt.json` 已存在 → 确认中断时 emit

### 验收抓手

- 测试：`server::cultivation::tests::meditate_emits_absorb_vfx` / `server::combat::tests::hit_emits_direction_vfx` / `server::cultivation::tests::breakthrough_fail_emits_vfx`
- 手动：打坐 → 灵气粒子向身体汇聚 → 经脉打通 → 身体光线 → 突破成功 → 光柱 / 失败 → 红碎裂 → 战斗 → 命中红弧 → 格挡蓝火花

---

## P1 — 产出 VFX + 阵法 VFX ✅ 2026-05-10

### 交付物

1. **Forge 锤击 VFX**（`ForgeHammerStrikePlayer.java`）
   - 淬火步骤：锤击时橙色火星（`BongSpriteParticle` `tribulation_spark` tint #FF8800 × 8 向上飞溅）
   - 铭文步骤：蓝色灵纹闪（`BongGroundDecalParticle` `rune_char` tint #4488FF 在工作台表面 0.5s）
   - 开光步骤：白色光爆（`BreakthroughPillarPlayer` 微缩版，高度 2 block，0.5s）
   - server emit：`forge::session_system` 每步 state 变化时 emit VFX

2. **Alchemy 炼制 VFX**（`AlchemyBrewVaporPlayer.java`）
   - 炼制中：彩色蒸汽上升（`BongSpriteParticle` `cloud256_dust` tint 按丹药类型变色，缓慢上升 + 左右飘动）
   - 火候过高：红色火焰粒子从丹炉底部喷出
   - 完成：金色光球从鼎口浮出（`BongSpriteParticle` `enlightenment_dust` tint #FFD700 单颗大粒子 lifetime 40 tick）
   - server emit：`alchemy::session_system` tick 时 emit 蒸汽 / 完成时 emit 光球

3. **灵田动作 VFX**
   - 开垦：土块翻起（`BongSpriteParticle` `cloud256_dust` tint 棕色 × 6 向上抛）
   - 种植：种子落地绿色脉动（`BongGroundDecalParticle` `lingqi_ripple` tint 绿 1s 消散）
   - 补灵：灵气从上方注入地块（`BongSpriteParticle` `qi_aura` 从 y+2 → y+0 下落轨迹 × 4）
   - server emit：`lingtian::action_system` 各 action 完成时 emit VFX

4. **阵法激活 VFX**（增强已有 `FormationActivatePlayer`）
   - 诡雷激活：地面红色符文闪 → 爆炸粒子环
   - 警戒场激活：蓝色半透明球面扩散（`BongSpriteParticle` 球面分布 × 20，lifetime 60 tick）
   - 阵法耗竭：灰色碎裂 + 符文消散
   - server emit：`zhenfa::activation_system` 各事件 emit VFX

### 验收抓手

- 测试：`server::forge::tests::hammer_step_emits_vfx` / `server::alchemy::tests::brew_emits_vapor` / `server::lingtian::tests::till_emits_vfx` / `server::zhenfa::tests::activate_emits_vfx`
- 手动：炼器 → 锤击火星 → 铭文蓝纹 → 开光白爆 → 炼丹 → 彩色蒸汽 → 成品金球 → 灵田开垦 → 土飞 → 阵法 → 符文光

---

## P2 — 社交 VFX + 死亡 VFX + 状态效果 VFX ✅ 2026-05-10

### 交付物

1. **社交 VFX**
   - 灵龛建立：石质光环从地面升起（`BongGroundDecalParticle` `lingqi_ripple` × 3 圈逐次扩散）
   - 结契仪式：双方之间灵丝连接（`BongRibbonParticle` 从 A → B，lifetime 60 tick，双向）
   - 仇人相见（feud）：双方头顶红色感叹号粒子 0.5s

2. **死亡 VFX 增强**
   - 玩家死亡：已有 `DeathSoulDissipatePlayer` → 追加灰化全屏 overlay（`OverlayQuadRenderer` #444444 → #000000 fade 2s）
   - 遗念生成：灰雾中浮出半透明文字粒子（需新增 `BongTextParticle` 或用 rune_char 变体）

3. **状态效果 VFX**
   - 中毒（Contamination > 0.3）：身周绿色烟雾持续（`BongSpriteParticle` `cloud256_dust` tint #44AA44 × 2 每 60 tick）
   - 虚弱（Exhausted）：已有 `ExhaustedGreyMistVfx` → 确认接线
   - 经脉裂痕：受损经脉对应身体部位偶尔闪红线（每 100 tick 闪一次，lifetime 5 tick）

### 验收抓手

- 测试：`server::social::tests::niche_establish_emits_vfx` / `server::combat::tests::death_emits_enhanced_vfx`
- E2E：全系统 VFX 走查——修炼 / 战斗 / 炼器 / 炼丹 / 灵田 / 阵法 / 社交 / 死亡，每个环节都有粒子反馈

---

## 前置依赖

| 依赖 plan | 状态 | 用到什么 |
|-----------|------|---------|
| plan-vfx-v1 | ✅ finished | VfxRegistry / 18 VFX players / VfxEventRequest |
| plan-particle-system-v1 | ✅ finished | 7 几何类 / 30 贴图 |
| plan-combat-no_ui | ✅ finished | DamageEvent / CombatState / parry |
| plan-cultivation-v1 | ✅ finished | meditate / meridian / breakthrough |
| plan-forge-v1 | ✅ finished | ForgeSessionState / 4 步状态机 |
| plan-alchemy-v1 | ✅ finished | BrewSessionState |
| plan-lingtian-v1 | ✅ finished | LingtianActionEvent |
| plan-zhenfa-v1 | ✅ finished | FormationActivateEvent |
| plan-social-v1 | ✅ finished | SpiritNiche / feud / pact |
| plan-death-lifecycle-v1 | ✅ finished | DeathEvent / DeathInsight |

**全部依赖已 finished，无阻塞。**

## Finish Evidence

### 落地清单

- P0 修炼/战斗：`server/src/cultivation/tick.rs`、`server/src/cultivation/meridian_open.rs`、`server/src/cultivation/breakthrough.rs`、`server/src/combat/resolve.rs` 发出 `bong:cultivation_absorb` / `bong:meridian_open` / `bong:breakthrough_fail` / `bong:combat_hit` / `bong:combat_parry`；客户端新增 `CultivationAbsorbPlayer`、`MeridianOpenFlashPlayer`、`BreakthroughFailPlayer`、`CombatHitDirectionPlayer`。
- P1 产出/阵法：`server/src/forge/mod.rs`、`server/src/network/client_request_handler.rs`、`server/src/lingtian/systems.rs`、`server/src/zhenfa/mod.rs` 接线 forge / alchemy / lingtian / zhenfa VFX；客户端新增 `ForgeHammerStrikePlayer`、`AlchemyBrewVaporPlayer`、`LingtianActionVfxPlayer`、`ZhenfaActionVfxPlayer`。
- P2 社交/死亡/状态：`server/src/social/mod.rs` 接线灵龛、结契、仇怨 VFX；`server/src/cultivation/dugu.rs` 接线 `bong:poison_mist`；死亡 `bong:death_soul_dissipate` 与虚弱 `bong:exhausted_grey_mist` 已在既有 lifecycle/full_power emit 路径接线，本 plan 确认并保持客户端注册。
- 共享 helper/注册：`server/src/network/gameplay_vfx.rs` 统一封装 `SpawnParticle` request；`client/src/main/java/com/bong/client/visual/particle/VfxBootstrap.java` 与 `VfxRegistryTest.java` 覆盖所有新增 route。

### 关键 commit

- `350980c89`（2026-05-10）`vfx-wiring-v1：接入服务端游戏事件 VFX`
- `5bfb5cbe5`（2026-05-10）`vfx-wiring-v1：注册客户端游戏 VFX players`
- `963e498f3`（2026-05-10）`vfx-wiring-v1：补齐社交关系 VFX 接线`
- `ddb010358`（2026-05-10）`归档 plan-vfx-wiring-v1：游戏事件 VFX 接线`
- `0d7468d89`（2026-05-10）`vfx-wiring-v1：处理 review VFX 细节`
- `2851c92e4`（2026-05-10）`vfx-wiring-v1：收敛 review 成功路径`

### 测试结果

- `cd server && cargo fmt --check` ✅
- `cd server && cargo check` ✅
- `cd server && cargo check --tests` ✅
- `cd server && CARGO_BUILD_JOBS=1 cargo clippy --all-targets -- -D warnings` ✅
- `cd server && CARGO_BUILD_JOBS=1 cargo test vfx -- --nocapture` ✅（rebase 前 92 passed）/ ⚠️（rebase 后两次在 test binary 链接阶段被 `rustc` `signal: 9, SIGKILL` 杀掉，无测试断言失败输出）
- `cd server && CARGO_BUILD_JOBS=1 cargo test poison_ambient_emits_mist_vfx_every_sixty_ticks -- --nocapture` ✅（rebase 前 1 passed）
- `cd server && CARGO_BUILD_JOBS=1 cargo test alchemy_flawed_take_back_grants_flawed_pill_residue -- --nocapture` ✅（rebase 前 1 passed）
- `cd server && CARGO_BUILD_JOBS=1 cargo test alchemy_explode_take_back_applies_damage_and_meridian_crack -- --nocapture` ✅（rebase 前 1 passed）
- `cd client && JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64 PATH=/usr/lib/jvm/java-17-openjdk-amd64/bin:$PATH ./gradlew test --tests com.bong.client.visual.particle.VfxRegistryTest` ✅
- `cd client && JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64 PATH=/usr/lib/jvm/java-17-openjdk-amd64/bin:$PATH ./gradlew test build` ✅
- `cd server && cargo test emits -- --nocapture` ⚠️ 前序本地两次在 test binary 链接阶段被 `rustc` `signal: 9, SIGKILL` 杀掉，无测试断言失败输出；已用 `cargo check --tests` + clippy 覆盖测试编译/lint。当前 PR 可见远端 check 为 E2E Redis Smoke，未单独暴露完整 `cargo test` job。

### 跨仓库核验

- server：`VfxEventRequest` / `gameplay_vfx::spawn_request` / `bong:cultivation_absorb` / `bong:forge_hammer_strike` / `bong:alchemy_brew_vapor` / `bong:zhenfa_trap` / `bong:social_pact_link` / `bong:poison_mist`
- client：`VfxRegistry` / `VfxBootstrap` / `CultivationAbsorbPlayer` / `AlchemyBrewVaporPlayer` / `SocialLinkVfxPlayer` / `PoisonMistPlayer`
- agent/schema：未新增 schema variant；沿用既有 `SpawnParticle` payload 字段。

### 遗留 / 后续

- 未新增贴图资产；全部复用既有 particle sprites/providers。
- 本地云环境对较大 server test binary 链接不稳定；rebase 前新增毒雾环境 VFX 单测已单独通过，rebase 后测试编译由 `cargo check --tests` 覆盖。建议后续 CI 明确拆出 server `cargo test` 可见 job。
