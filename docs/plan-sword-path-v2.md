# Bong · plan-sword-path-v2

**剑道 v1 遗留的全部 ECS 接线 + 运行时系统 + 残卷解锁 + BOSS AI + VFX 资产 + agent 屏蔽 + 集成测试**。v1 落地了纯函数/数据结构/常数/HUD planner，本 plan 把这些接入 Bevy system 让整条剑道链路真正跑起来。

**v1 → v2 关系**：v1 = 骨架（struct + fn + test），v2 = 肉（system + event handler + asset + e2e）。

**世界观锚点**：同 v1（§三 六境界 + §四 近战 + §五 器修 + §六 凝实/锋锐色 + §八 天道感应）

**前置依赖**：
- `plan-sword-path-v1` ✅ — 全部纯函数、数据模型、87 server 测试、11 client 测试
- `plan-combat-no_ui` ✅ — `AttackIntent` / `CombatEvent` / `SkillRegistry`
- `plan-skill-v1` ✅ — `SkillRegistry::register` / `TechniqueDefinition` / `cast_technique`
- `plan-meridian-severed-v1` ✅ — `SkillMeridianDependencies::declare()`
- `plan-vfx-v1` ✅ — VfxEventRouter / VfxPlayer / BongParticles 管线
- `plan-audio-world-v1` ✅ — `PlaySoundRecipeRequest` 音效管线
- `plan-entity-model-v1` ✅ — `BongEntityModelKind` / `BongVisualState`（黑武士 fauna 模型已注册）
- `plan-npc-ai-v1` ✅ — big-brain Utility AI 框架（Scorer / Action）

**反向被依赖**：
- `plan-supply-coffin-v1` skeleton — 物资棺 loot 表包含 `scroll_sword_path`，需本 plan 补全残卷的 `technique_scroll_spec`

---

## 接入面 Checklist

- **进料**（v1 已有，v2 接线）：
  - `sword_path::bond::SwordBondComponent` — 绑定数据（v1 定义）
  - `sword_path::grade::SwordGrade` — 品阶数据
  - `sword_path::techniques::*` — 五招常数
  - `sword_path::shatter::*` — 碎裂/化虚纯函数
  - `sword_path::heaven_gate::*` — 天道盲区 Registry
  - `combat::weapon::Weapon { weapon_kind: Sword }` — 持剑判定
  - `cultivation::components::Cultivation` — 真元池 + 境界
  - `cultivation::known_techniques::KnownTechniques` — 招式 proficiency
  - `cultivation::meridian::MeridianSystem` — 经脉状态
  - `cultivation::technique_scroll::read_combat_technique_scroll` — 残卷阅读
- **出料**（v2 新增 system 产出）：
  - `combat::events::CombatEvent` — 剑道招式命中结算
  - `combat::events::AttackSource::Sword*` 4 变体 — v2 实际接入 combat pipeline
  - `network::VfxEventPayloadV1` — 粒子/音效/动画触发
  - `network::agent_bridge::publish_world_state_to_redis` — 天道盲区过滤
  - `cultivation::technique_scroll::TechniqueScrollReadEvent` — 残卷读取事件
- **共享类型/event**：
  - **复用** v1 全部 struct/event（不新增数据结构）
  - **新增** Bevy systems：`sword_bond_tracking_system` / `sword_technique_cast_system` / `sword_shatter_system` / `heaven_gate_cast_system` / `tiandao_blind_zone_tick_system` / `tiandao_blind_zone_filter_system`
- **跨仓库契约**：
  - server: `sword_path/systems.rs`（新建）— 全部 Bevy Update systems
  - client: VfxPlayer 类 × 8 + audio_recipe JSON × 10 + PlayerAnimator JSON × 4 + `SwordBondHudStateStore` network handler
  - agent: `publish_world_state_to_redis` 过滤 blind zone 内玩家 + `bong:agent_cmd` blind zone blocked 响应
- **qi_physics 锚点**：同 v1（`QiTransfer` 注剑/碎裂/化虚 + `container_intake` 灵剑衰减 + `release_to_zone` 化虚释放 + `attenuation` 剑气衰减 0.03/格）

---

## 边界：本 plan 做什么 & 不做什么

| 维度 | 范围 | 不做 |
|------|------|------|
| ECS wiring | 绑定触发、招式 cast、碎裂反噬、化虚结算、盲区 tick 全部接 Bevy system | 新数据模型（v1 已完成） |
| 残卷 | `scroll_sword_path` 补 `technique_scroll_spec`，5 招各一卷 | 新物品 |
| BOSS AI | 黑武士 big-brain Scorer/Action 3 阶段 | 新怪物 |
| VFX | 全部粒子贴图 + audio_recipe + PlayerAnimator JSON + VfxPlayer 类 | 新粒子基类 |
| Agent | blind zone 过滤 `world_state` 推送 | 新 agent 功能 |
| 测试 | v1 留空的全部 `[ ]` 测试项 + e2e 集成 | 已通过的 v1 测试 |

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | 残卷解锁（scroll_sword_path → technique_scroll_spec 5 招）+ 残卷读取 system | ⬜ |
| P1 | ECS 接线——绑定触发 + 注入 + 碎裂 + 招式 cast + 经脉拦截 | ⬜ |
| P2 | 化虚·一剑开天门 runtime + 天道盲区 tick/过滤 + agent 屏蔽 | ⬜ |
| P3 | 黑武士 BOSS AI（big-brain 3 阶段 + spawn + 掉落 runtime）| ⬜ |
| P4 | VFX 资产全包（贴图 + audio_recipe + 动画 + VfxPlayer）+ 视听联调 | ⬜ |
| P5 | v1 遗留测试补全 + e2e 集成测试 + InspectScreen 扩展 | ⬜ |

---

## P0 — 残卷解锁

### P0.1 scroll_sword_path 拆分为 5 卷

`server/assets/items/sword_materials.toml` 现有 `scroll_sword_path` 改为通用卷轴壳，新增 5 个具体卷：

| id | name | technique_id | required_realm |
|----|------|-------------|---------------|
| `scroll_sword_condense` | 剑道残卷·凝锋心法 | `sword_path.condense_edge` | 引气 |
| `scroll_sword_qi_slash` | 剑道残卷·剑气斩诀 | `sword_path.qi_slash` | 凝脉 |
| `scroll_sword_resonance` | 剑道残卷·共鸣要义 | `sword_path.resonance` | 固元 |
| `scroll_sword_manifest` | 剑道残卷·归一秘录 | `sword_path.manifest` | 通灵 |
| `scroll_sword_heaven_gate` | 剑道残卷·天门禁忌 | `sword_path.heaven_gate` | 化虚 |

每个卷轴定义 `technique_scroll_spec`：

```toml
[technique_scroll_spec]
kind = "combat_technique"
technique_id = "sword_path.condense_edge"
required_realm = "induce"
meridian_dependencies = ["large_intestine", "small_intestine"]
```

原 `scroll_sword_path` 保留为"通用卷轴"，用于 loot 表的 placeholder——运行时开箱时按权重随机替换为 5 卷中的 1 个（低阶权重高）。

### P0.2 残卷读取接入

`server/src/sword_path/scroll.rs`（新建）

- 监听 `ClientRequestPayloadV1::UseItem { template_id: "scroll_sword_*" }`
- 调用 `read_combat_technique_scroll(known, cultivation, meridians, severed, template)` 
- 成功 → 消耗卷轴 + 发 `TechniqueLearnedEvent` + 发 narration
- 失败 → 发失败原因 toast

**视听——残卷阅读成功**：
- **粒子**：`BongLineParticle` × 6 从卷轴位置向玩家头部汇聚，lifetime 20 tick，颜色 `#AABBCC`，spawn burst，贴图复用 `bong:sword_qi_trail`，VfxPlayer `ScrollAbsorbVfxPlayer`，事件 ID `bong:sword_scroll_read`
- **音效**：`{ "layers": [{ "sound": "entity.experience_orb.pickup", "pitch": 1.2, "volume": 0.7, "delay_ticks": 0 }, { "sound": "block.enchantment_table.use", "pitch": 1.0, "volume": 0.5, "delay_ticks": 5 }] }`
- **HUD**：`HudRenderLayer.POPUP` toast「习得 {technique_display_name}」，颜色 `#C8D8E8`，持续 80 tick，fade-out 20 tick
- **narration**：scope: player, style: perception — `"残卷上的文字如蝌蚪般游动，涌入识海。{technique_display_name}的心法口诀在脑海中渐渐清晰。"` / `"归元剑宗的剑意残留在字里行间，你仿佛看到了千年前宗门弟子在练功场挥剑的身影。"`

---

## P1 — ECS 接线

### P1.1 绑定触发 system

`server/src/sword_path/systems.rs`（新建）

```rust
/// 追踪玩家连续使用剑术次数 → 达到 BOND_TRIGGER_USES 时挂载 SwordBondComponent
fn sword_bond_tracking_system(
    mut commands: Commands,
    mut combat_events: EventReader<CombatEvent>,
    mut players: Query<(Entity, &Weapon, Option<&SwordBondComponent>, Option<&mut SwordBondProgress>)>,
    mut bond_events: EventWriter<SwordBondFormedEvent>,
)
```

逻辑：
- 每次 `CombatEvent` 命中且 `attacker` 持 `WeaponKind::Sword` → 对应 `SwordBondProgress.consecutive_uses += 1`
- 换剑 / 不持剑 → `consecutive_uses = 0`
- `consecutive_uses >= BOND_TRIGGER_USES` 且无 `SwordBondComponent` → 挂载 `SwordBondComponent::new(weapon_entity)` + 发 `SwordBondFormedEvent`
- 已有 `SwordBondComponent` → 拒绝第二绑定

### P1.2 真元自动注入 system

```rust
/// 每次剑道招式 cast 后自动注入真元到灵剑
fn sword_qi_inject_system(
    mut technique_events: EventReader<TechniqueCastEvent>,
    mut players: Query<(&mut SwordBondComponent, &mut Cultivation)>,
)
```

逻辑：
- `TechniqueCastEvent.technique_id` 以 `sword_path.` 开头 → `bond.try_inject_qi(qi_cost)`
- 走 `QiTransfer { from: player, to: sword }` 守恒

### P1.3 碎裂反噬 system

```rust
/// SwordShatterEvent → 扣减 Cultivation qi_current/qi_max + 释放真元回 zone
fn sword_shatter_system(
    mut shatter_events: EventReader<SwordShatterEvent>,
    mut players: Query<(&mut Cultivation, &Position)>,
    mut zone_qi: ResMut<ZoneQiManager>,
)
```

逻辑：
- `cultivation.qi_current -= backlash_qi_current()`
- `cultivation.qi_max -= backlash_qi_max()`（永久衰减）
- `release_to_zone(stored_qi, zone)` 守恒释放
- 10% roll → spawn `broken_sword_soul`

### P1.4 招式 cast 接入 SkillRegistry

`server/src/sword_path/skill_register.rs`（新建）

将 5 个 `SwordTechniqueDef` 转换为 `TechniqueDefinition` 注册到 `SkillRegistry`：

```rust
pub fn register_sword_techniques(registry: &mut SkillRegistry) {
    for def in ALL_SWORD_TECHNIQUES {
        registry.register(TechniqueDefinition {
            id: def.id,
            display_name: def.display_name,
            required_realm: def.required_realm,
            qi_cost: def.qi_cost,
            stamina_cost: def.stamina_cost,
            cast_ticks: def.cast_ticks,
            cooldown_ticks: def.cooldown_ticks,
            range: def.range,
        });
    }
}
```

### P1.5 经脉依赖声明

`SkillMeridianDependencies::declare()` 注册：

| technique_id | 依赖经脉 |
|-------------|---------|
| `sword_path.condense_edge` | `LargeIntestine` + `SmallIntestine` |
| `sword_path.qi_slash` | `LargeIntestine` + `SmallIntestine` + `TripleEnergizer` |
| `sword_path.resonance` | `LargeIntestine` + `SmallIntestine` + `TripleEnergizer` |
| `sword_path.manifest` | `LargeIntestine` + `SmallIntestine` + `TripleEnergizer` |
| `sword_path.heaven_gate` | `LargeIntestine` + `SmallIntestine` + `TripleEnergizer` + `Du` |

经脉 SEVERED → `check_meridian_dependencies` 拦截 → cast 失败 + toast 提示。

### P1.6 招式战斗效果 system

```rust
fn sword_technique_effect_system(
    mut technique_events: EventReader<TechniqueCastEvent>,
    mut combat_events: EventWriter<CombatEvent>,
    players: Query<(&Position, &SwordBondComponent, &Cultivation)>,
    targets: Query<(Entity, &Position)>,
)
```

| 招式 | 效果 |
|------|------|
| `condense_edge` | 挂 buff component → 下次攻击 ×1.8 + 穿甲 30%，5s / 1 次命中消散 |
| `qi_slash` | 8 格直线 raycast → 命中所有 entity → `base_attack × grade_mult × attenuation(0.03/格, dist)` |
| `resonance` | 6 格 AoE → 敌方 cast 打断 + `StatusEffectKind::Slow { duration: 3-5s }` |
| `manifest` | spawn 剑意实体 → 5s 自动追踪最近敌方 → `base_attack × 2.0` → 结束后 `bond_strength -= 0.1` |
| `heaven_gate` | → P2 |

**视听**：每个招式的 cast/命中触发对应 `VfxEventPayloadV1`（v1 plan 已完整定义规格）。

---

## P2 — 化虚·一剑开天门 runtime + 天道盲区

### P2.1 化虚 cast system

```rust
fn heaven_gate_cast_system(
    mut cast_events: EventReader<HeavenGateCastEvent>,
    mut players: Query<(&mut Cultivation, &mut SwordBondComponent, &Position, &MeridianSystem)>,
    mut blind_registry: ResMut<TiandaoBlindZoneRegistry>,
    mut zone_qi: ResMut<ZoneQiManager>,
    mut shatter_events: EventWriter<SwordShatterEvent>,
    mut combat_events: EventWriter<CombatEvent>,
    clock: Res<CombatClock>,
    targets: Query<(Entity, &Position)>,
)
```

完整 4 阶段流程（v1 §P4.1）：
1. 蓄力 0-60 tick → 逐 tick 抽取 qi_current + stored_qi 到 staging_buffer
2. 临界点 60 tick → zone warning
3. 释放 60-80 tick → 100 格内 AoE 伤害（`staging_buffer × attenuation × 0.5 穿透`）
4. aftermath → `TiandaoBlindZone` 注册 + `qi_current = 0` + `qi_max *= 0.1` + 境界跌至固元 + `SwordShatterEvent`

### P2.2 天道盲区 tick system

```rust
fn tiandao_blind_zone_tick_system(
    mut registry: ResMut<TiandaoBlindZoneRegistry>,
    clock: Res<CombatClock>,
)
```

每 tick 检查过期 → 移除。

### P2.3 Agent 屏蔽

`server/src/network/redis_bridge.rs` — `publish_world_state_to_redis` 新增过滤：

```rust
if blind_registry.is_player_hidden(player_pos) {
    continue; // 不推送该玩家 snapshot
}
```

`bong:agent_cmd` handler：目标 zone 有 blind zone → 响应 `{ "status": "blocked", "reason": "tiandao_blind_zone" }`。

---

## P3 — 黑武士 BOSS AI（成长型 + 动画配套）

### P3.0 设计轴心——成长型 BOSS

黑武士不是静态数值怪。它是一个会**随战斗推进变快**的 BOSS：
- 每个技能有独立冷却（CD tick）
- 每经过一个**成长周期**（`GROWTH_CYCLE_TICKS = 600` = 30s），所有技能 CD 缩短 15%（`CD_DECAY_PER_CYCLE = 0.85`）
- 成长上限：CD 最低不低于原始值的 40%（`CD_FLOOR_RATIO = 0.40`）——即打到 ~3 个周期后 CD 趋于稳定
- 意味着拖延越久 BOSS 越凶猛：Phase 1 慢吞吞的弹幕到 Phase 2 末期变成连续轰炸

```rust
#[derive(Component, Debug, Clone)]
pub struct HeiwushiState {
    pub phase: HeiwushiPhase,
    pub health_max: f32,       // 2100.0
    pub health: f32,
    pub base_attack: f32,      // 35.0
    pub defense: f32,          // 8.0
    pub move_speed: f64,       // 4.8 blocks/s
    pub growth_cycles: u32,    // 已经历的成长周期数
    pub last_cycle_tick: u64,  // 上次成长周期的 tick
    pub skill_cooldowns: HeiwushiCooldowns,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeiwushiPhase {
    Phase1,  // 100%-60% HP：单剑持握，稳健节奏
    Phase2,  // 60%-25% HP：加速模式，旋涡频出
    Phase3,  // <25% HP：暗影化身，双持狂暴
}

#[derive(Debug, Clone)]
pub struct HeiwushiCooldowns {
    /// 每技能的剩余冷却 tick（0 = 可用）
    pub melee_slash: u32,
    pub dark_barrage: u32,
    pub dark_vortex: u32,
    pub shadow_transform: u32,  // 一次性，触发后永久 0
    /// 基础 CD 值（成长衰减前）
    pub base_melee_slash: u32,
    pub base_dark_barrage: u32,
    pub base_dark_vortex: u32,
}

impl HeiwushiCooldowns {
    /// 返回当前成长周期下的实际 CD
    pub fn effective_cd(&self, base: u32, cycles: u32) -> u32 {
        let factor = CD_DECAY_PER_CYCLE.powi(cycles as i32).max(CD_FLOOR_RATIO);
        (base as f64 * factor).round() as u32
    }
}
```

### P3.1 动作表——6 个 Action 配套动画

每个 Action 严格绑定一个动画 ID + 粒子 + 音效。

---

#### 1. 待机巡逻 `HeiwushiIdleAction`

**触发**：无玩家进入 20 格检测范围时。

**行为**：在铸剑古殿地下室 20×20 范围内三点巡逻，到达巡逻点后面向大殿方向行礼。

**动画**：
- 移动中：`heiwushi.walk`（需补写）
  - endTick 20（1s 循环），`rightLeg.pitch` / `leftLeg.pitch` 交替 ±0.6rad，`rightArm.pitch` / `leftArm.pitch` 交替 ±0.2rad（持剑摆臂幅度小），`body.pitch = +0.05rad`（微前倾）
- 到达巡逻点：`heiwushi.idle`（需补写）
  - endTick 80（4s 循环），`body.yaw` ±0.05rad 正弦摆（左右微晃），`beiSword2.roll` / `beiSword3.roll` ±0.03rad 反向摆（背剑微颤），`rightArm.pitch = -0.15rad`（持剑微抬），easing sinInOut
- **音效**：无主动音效；环境走 `ambient_sword_sea`

---

#### 2. 近战斩击 `HeiwushiMeleeSlashAction`

**触发**：全阶段可用。玩家进入 3 格 → `HeiwushiAggroScorer` > 0.5。

**冷却**：base 40 tick（2s）→ 成长衰减 → 最低 16 tick（0.8s）

**行为**：面向目标 → 前冲 0.5 格 → 单臂横斩。

**动画**：`heiwushi.melee_slash`（需补写）
- endTick 16（0.8s）
- 0-4 tick：`rightArm.pitch = -0.3rad`（举刀），`body.yaw = -0.2rad`（侧身蓄力），easing cubicOut
- 4-10 tick：`rightArm.pitch = +0.6rad`（下劈），`rightArm.yaw = +0.8rad`（横扫），`body.yaw = +0.4rad`（转体跟随），`body.z += 0.5`（前冲），easing cubicIn
- 10-16 tick：回正，`rightArm.pitch = -0.15rad`，`body.yaw = 0`，easing linear

**伤害**：`base_attack × phase_mult`（Phase 1: ×1.0 / Phase 2: ×1.3 / Phase 3: ×2.0）

**粒子**：`BongLineParticle` × 3 沿斩击弧线，lifetime 8 tick，颜色 `#334455` alpha 180，spawn burst at tick 6，贴图 `bong:sword_qi_trail`，VfxPlayer `HeiwushiSlashVfxPlayer`，事件 ID `bong:heiwushi_melee_slash`

**音效** `heiwushi_melee_slash`：
```json
{ "layers": [
  { "sound": "entity.player.attack.sweep", "pitch": 0.8, "volume": 0.9, "delay_ticks": 3 },
  { "sound": "entity.iron_golem.attack", "pitch": 1.2, "volume": 0.5, "delay_ticks": 4 }
]}
```

---

#### 3. 暗影弹幕 `HeiwushiDarkBarrageAction`（= 动画 `黑暗弹幕` / `skill1`）

**触发**：Phase 1 + Phase 2。玩家 4-8 格内 → score 0.7。

**冷却**：base 60 tick（3s）→ 成长衰减 → 最低 24 tick（1.2s）

**行为**：双臂展开 → 双剑从手中射出暗影弹 → 弹道直线 8 格。

**动画**：`heiwushi.skill1`（已有，0.76s = 15.2 tick）
- 实际关键帧解读：
  - 0s：双臂高举（`rightArm.pitch = -80°`，`leftArm.pitch = -77.5°`），身体前倾 10°
  - 0.24s：双臂降至攻击位（`rightArm.pitch = -12.5°`），准备释放
  - 0.52s：左右持剑 scale → 0（剑消失 = 射出），弹幕释放帧
  - 0.76s：保持姿态（余韵）

**伤害**：`base_attack × 1.2`，8 格直线 raycast，穿透第一个目标（对后排 50% 伤害）

**弹道粒子**：`BongSpriteParticle` × 8 深紫色暗影团，lifetime 20 tick，速度 8 m/s 直线，颜色 `#1A0022`→`#330044` fade，spawn burst at tick 10（释放帧），贴图 `bong:dark_barrage_bolt`（新增 8×8 暗紫球），VfxPlayer `HeiwushiBarrageVfxPlayer`，事件 ID `bong:heiwushi_dark_barrage`

**音效** `heiwushi_dark_barrage`：
```json
{ "layers": [
  { "sound": "entity.wither.shoot", "pitch": 1.4, "volume": 0.8, "delay_ticks": 0 },
  { "sound": "entity.breeze.shoot", "pitch": 0.8, "volume": 0.5, "delay_ticks": 5 }
]}
```

---

#### 4. 暗黑旋涡 `HeiwushiDarkVortexAction`（= 动画 `黑暗旋涡` / `skill2`）

**触发**：Phase 2 + Phase 3。玩家 6 格内 → score 0.8。

**冷却**：base 80 tick（4s）→ 成长衰减 → 最低 32 tick（1.6s）

**行为**：身体前倾蓄力 → 双臂展开旋转 → 6 格 AoE 旋涡，吸引范围内目标向中心 + 伤害。

**动画**：`heiwushi.skill2`（已有，1.04s = 20.8 tick）
- 实际关键帧解读：
  - 0s：起始站立
  - 0.24s：身体前倾 15°（`body.pitch = +15°`），双臂举起（`rightArm/leftArm.pitch = -55°`）—— 蓄力
  - 0.52s：双臂展开（`rightArm.yaw = +57.5°`，`leftArm.yaw = -60°`），身体后仰（`body.pitch = -7.5°`）—— 旋涡释放
  - 0.76s：双臂继续展开至最大（`rightArm.yaw = +75°`，`leftArm.yaw = -75°`）—— 旋涡全开
  - 全程腿部微动（`rightLeg/leftLeg.pitch` ±5-7.5°）—— 脚下踩地稳固

**伤害**：`base_attack × 1.5`，6 格 AoE 圆形。附加 knockback 吸引（方向朝向 BOSS 中心，力度 0.4）。

**粒子**：
- `BongGroundDecalParticle` × 1 扩散环，半径 0→6 格，lifetime 30 tick，颜色 `#110022` alpha 150，贴图 `bong:dark_vortex_ring`（新增 16×16 暗紫环），spawn burst at tick 10
- `BongSpriteParticle` × 16 螺旋向内收束，lifetime 20 tick，颜色 `#220033`→`#440066`，spawn continuous tick 10-20

VfxPlayer `HeiwushiVortexVfxPlayer`，事件 ID `bong:heiwushi_dark_vortex`

**音效** `heiwushi_dark_vortex`：
```json
{ "layers": [
  { "sound": "entity.warden.sonic_charge", "pitch": 0.8, "volume": 1.0, "delay_ticks": 0 },
  { "sound": "entity.ender_dragon.growl", "pitch": 1.5, "volume": 0.5, "delay_ticks": 5 },
  { "sound": "entity.breeze.wind_burst", "pitch": 0.6, "volume": 0.6, "delay_ticks": 10 }
]}
```

---

#### 5. 暗影化身 `HeiwushiShadowTransformAction`（= 动画 `黑暗化身` / `skill3`）

**触发**：HP 首次跌破 25% → **一次性**触发，不可重复。

**行为**：背剑展开 → 拔入双手 → 模式永久切换为双持。Phase 3 开始。

**动画**：`heiwushi.skill3`（已有，0.8s = 16 tick）
- 实际关键帧解读：
  - 0s：起始
  - 0.12s：背剑微动（`beiSword2.roll = -5°`，`beiSword3.roll = +5°`）—— 震颤前兆
  - 0.28s：双臂伸向背后（`rightArm.pitch = -55°, yaw = -27.5°`，`leftArm.pitch = -55°, yaw = +25°`），头部偏转（`bone8.pitch = -15°, yaw = -25°, roll = -65°`）—— 拔剑中
  - 0.36s：背剑展开至极限（`beiSword2.roll = -5°`→蓄力）
  - 0.52s：背剑最终展开（`beiSword2.roll = +50°`，`beiSword3.roll = -50°`）—— 剑入双手
  - 0.8s：双持姿态定格

**状态切换**：
- `state.phase = Phase3`
- `state.base_attack *= 2.0`
- `state.defense *= 0.5`
- `state.move_speed = 7.2`（从 4.8 提速 50%）
- 强制重置所有技能 CD 为 0（变身后立即可攻击）

**粒子**：`BongSpriteParticle` × 32 暗紫爆散（v1 已定义），lifetime 25 tick，速度 2.0 m/s，颜色 `#2A0033`→`#550066`，spawn burst，贴图 `bong:dark_transform`，VfxPlayer `HeiwushiTransformVfxPlayer`，事件 ID `bong:heiwushi_transform`

**音效** `heiwushi_transform`（v1 已定义）：
```json
{ "layers": [
  { "sound": "entity.warden.emerge", "pitch": 1.2, "volume": 1.0, "delay_ticks": 0 },
  { "sound": "entity.wither.spawn", "pitch": 1.5, "volume": 0.6, "delay_ticks": 5 },
  { "sound": "entity.lightning_bolt.impact", "pitch": 0.8, "volume": 0.4, "delay_ticks": 8 }
]}
```

**HUD**（nearby 玩家）：`VisualEffectProfile.BOSS_PHASE_SHIFT`（v1 已定义），edgeVignette `#1A0022` maxAlpha 150，duration 1500ms + screenShake amplitude 3px 频率 25Hz 800ms

**narration**：scope: zone, style: narrative — `"铸剑古殿深处传来一声嘶哑的低吼：'师兄……是贼人……我来护宗！'暗紫色的真元从人偶体内涌出，两柄背剑被扯入双手。"`

---

#### 6. 死亡 `HeiwushiDeathAction`

**触发**：HP ≤ 0。

**行为**：停止所有攻击 → 播放死亡动画 → 掉落物品 → 灵智核心消散 → 延迟 despawn。

**动画**：`heiwushi.death`（需补写）
- endTick 30（1.5s）
- 0-10 tick：前倾（`body.pitch = +0.8rad`），持剑手松开（`rightArm.pitch = +1.0rad` 下垂），easing cubicOut
- 10-20 tick：双膝跪地（`rightLeg.pitch = -1.2rad`，`leftLeg.pitch = -1.2rad`），`body.pitch = +1.4rad`（深前倾），easing linear
- 20-30 tick：完全倒地（`body.pitch = +1.57rad`），`body.y -= 0.8`（高度下降），双臂自然散开，easing linear

**粒子**（v1 已定义）：`BongSpriteParticle` × 48 缓慢上升（drift Y +0.3），lifetime 60 tick，颜色 `#334455`→透明，spawn continuous 3s，贴图 `bong:sword_soul_mote`

**音效** `heiwushi_death`（v1 已定义）：
```json
{ "layers": [
  { "sound": "entity.allay.death", "pitch": 0.6, "volume": 0.8, "delay_ticks": 0 },
  { "sound": "block.amethyst_block.break", "pitch": 0.4, "volume": 0.5, "delay_ticks": 10 }
]}
```

**narration**（v1 已定义）：scope: zone, style: narrative — `"人偶缓缓跪倒，双手仍紧握剑柄。嘶哑的声音最后一次响起：'掌门……小师弟……练完了……'灵智核心的微光熄灭，千年执念终于散去。"`

---

### P3.2 成长周期 system

```rust
fn heiwushi_growth_tick_system(
    clock: Res<CombatClock>,
    mut bosses: Query<&mut HeiwushiState, With<HeiwushiMarker>>,
) {
    for mut state in &mut bosses {
        if clock.tick - state.last_cycle_tick >= GROWTH_CYCLE_TICKS {
            state.growth_cycles += 1;
            state.last_cycle_tick = clock.tick;
            // CD 衰减在 effective_cd() 中按 cycles 实时计算，不需要改 base
        }
    }
}
```

**成长节奏表**（base CD → 实际 CD）：

| 周期 | 衰减因子 | 近战 (base 40) | 弹幕 (base 60) | 旋涡 (base 80) |
|------|---------|---------------|----------------|----------------|
| 0 | 1.00 | 40 (2.0s) | 60 (3.0s) | 80 (4.0s) |
| 1 | 0.85 | 34 (1.7s) | 51 (2.55s) | 68 (3.4s) |
| 2 | 0.72 | 29 (1.45s) | 43 (2.15s) | 58 (2.9s) |
| 3 | 0.61 | 24 (1.2s) | 37 (1.85s) | 49 (2.45s) |
| 4+ | 0.52→0.40 floor | 21→16 | 31→24 | 42→32 |

**设计意图**：前 90s（3 周期）手感明显加速，之后趋稳。鼓励玩家在 Phase 1-2 窗口期速战速决，拖入 Phase 3 + 高成长 = 地狱难度。

### P3.3 big-brain Thinker 组装

```rust
pub fn heiwushi_thinker() -> ThinkerBuilder {
    Thinker::build()
        .picker(FirstToScore { threshold: 0.3 })
        .when(HeiwushiDeathScorer, HeiwushiDeathAction)
        .when(HeiwushiTransformScorer, HeiwushiShadowTransformAction)
        .when(HeiwushiVortexScorer, HeiwushiDarkVortexAction)
        .when(HeiwushiBarrageScorer, HeiwushiDarkBarrageAction)
        .when(HeiwushiMeleeScorer, HeiwushiMeleeSlashAction)
        .otherwise(HeiwushiIdleAction)
}
```

**Scorer 优先级**（由高到低）：
1. `HeiwushiDeathScorer`：HP ≤ 0 → 1.0（最高优先）
2. `HeiwushiTransformScorer`：HP < 25% 且 Phase ≠ Phase3 → 0.95
3. `HeiwushiVortexScorer`：Phase 2/3 + 玩家 ≤ 6 格 + CD ready → 0.8
4. `HeiwushiBarrageScorer`：Phase 1/2 + 玩家 4-8 格 + CD ready → 0.7
5. `HeiwushiMeleeScorer`：玩家 ≤ 3 格 + CD ready → 0.6
6. `HeiwushiIdleAction`：otherwise fallback

### P3.4 spawn + 刷新

`server/src/npc/spawn_heiwushi.rs`

- zone `giant_sword_sea` 铸剑古殿地下室固定坐标 spawn
- 击杀后 real-time 72h 刷新（首杀后），之后 real-time 1h 刷新
- spawn 时 `HeiwushiState` 初始化（growth_cycles = 0，全 CD = 0 即可立即攻击）

### P3.5 掉落 runtime

v1 已定义 `HEIWUSHI_DROPS`（`server/src/fauna/drop.rs`），v2 接入实际 drop spawn：
- 必掉：`star_iron` ×2 + `sword_embryo_shard` ×2
- 首杀必掉：`broken_sword_soul` ×1（用 `PlayerState` 标记首杀）
- 30%：`ancient_sword_embryo` ×1
- 10%：`scroll_sword_manifest` ×1

### P3.6 需补写的动画汇总

| 动画 ID | endTick | 骨骼要点 | 状态 |
|---------|---------|---------|------|
| `heiwushi.idle` | 80 | body.yaw ±0.05rad 正弦，背剑微颤 | 需补写 |
| `heiwushi.walk` | 20 | 腿交替 ±0.6rad，持剑臂小幅摆 | 需补写 |
| `heiwushi.melee_slash` | 16 | 举刀→横斩→回正，body 转体跟随 | 需补写 |
| `heiwushi.death` | 30 | 前倾→跪地→倒地，松剑 | 需补写 |
| `heiwushi.skill1`（黑暗弹幕）| 15 | 双臂展开→剑射出 | ✅ 已有 |
| `heiwushi.skill2`（黑暗旋涡）| 21 | 前倾蓄力→双臂展开旋转 | ✅ 已有 |
| `heiwushi.skill3`（暗影化身）| 16 | 背剑展开→拔入双手 | ✅ 已有 |

---

## P4 — VFX 资产全包

### P4.1 粒子贴图（新增）

| 贴图 | 尺寸 | 用途 |
|------|------|------|
| `bong:sword_bond_line` | 8×8 | 绑定成功线条 |
| `bong:sword_shard` | 8×8 | 剑碎碎片 |
| `bong:sword_qi_trail` | 8×8 | 凝锋/剑意通用尾迹 |
| `bong:sword_qi_arc` | 32×8 | 剑气斩主弧 |
| `bong:sword_resonance_ring` | 16×16 | 剑鸣扩散环 |
| `bong:sword_manifest_aura` | 8×8 | 剑意化形光点 |
| `bong:heaven_gate_converge` | 8×8 | 化虚蓄力收束光点 |
| `bong:heaven_gate_flash` | 4×4 | 临界闪白 |
| `bong:heaven_gate_shockwave` | 32×32 | 冲击波环 |
| `bong:dust_cloud` | 8×8 | 化虚余尘 |
| `bong:dark_transform` | 8×8 | 黑武士变身碎片 |
| `bong:sword_soul_mote` | 4×4 | 黑武士死亡魂光 |
| `bong:coffin_debris` | 8×8 | 物资棺碎片（supply-coffin-v1 共用） |

### P4.2 audio_recipe JSON（新增）

| recipe_id | 用途 | layers 数 |
|-----------|------|----------|
| `sword_bond_form` | 绑定成功 | 2 |
| `sword_shatter` | 剑碎反噬 | 3 |
| `sword_condense_edge` | 凝锋 cast | 1 |
| `sword_condense_hit` | 凝锋命中消散 | 1 |
| `sword_qi_slash` | 剑气斩 cast | 2 |
| `sword_qi_slash_hit` | 剑气斩命中 | 1 |
| `sword_resonance` | 剑鸣 cast | 3 |
| `sword_manifest_summon` | 剑意化形召唤 | 2 |
| `sword_manifest_strike` | 剑意追踪命中 | 1 |
| `heaven_gate_charge_0s` | 化虚蓄力第一层 | 1 |
| `heaven_gate_charge_1s` | 化虚蓄力第二层 | 1 |
| `heaven_gate_charge_2s` | 化虚蓄力第三层 | 1 |
| `heaven_gate_flash` | 临界闪光 | 1 |
| `heaven_gate_release` | 冲击波释放 | 3 |
| `heiwushi_transform` | 黑武士 Phase 3 | 3 |
| `heiwushi_death` | 黑武士死亡 | 2 |
| `sword_scroll_read` | 残卷阅读 | 2 |

全部 layers 结构严格遵循 v1 plan 中已定义的 sound/pitch/volume/delay_ticks 规格。

### P4.3 PlayerAnimator JSON（新增/补写）

| 文件 | endTick | 用途 |
|------|---------|------|
| `sword_manifest_cast.animation.json` | 40 | 剑意化形举剑 |
| `sword_heaven_gate_charge.animation.json` | 60 | 化虚蓄力举剑 |
| `sword_heaven_gate_release.animation.json` | 20 | 化虚劈下释放 |
| `heiwushi_idle.animation.json` | 80 | 黑武士站立 |
| `heiwushi_walk.animation.json` | 20 | 黑武士行走 |
| `heiwushi_death.animation.json` | 30 | 黑武士倒地 |

骨骼姿态严格遵循 v1 plan 定义的 pitch/yaw/roll 弧度值。

### P4.4 VfxPlayer 类（client Java）

| 类名 | 事件 ID |
|------|--------|
| `SwordBondVfxPlayer` | `bong:sword_bond_form` |
| `SwordShatterVfxPlayer` | `bong:sword_shatter` |
| `SwordCondenseVfxPlayer` | `bong:sword_condense_edge` |
| `SwordQiSlashVfxPlayer` | `bong:sword_qi_slash_path` |
| `SwordResonanceVfxPlayer` | `bong:sword_resonance` |
| `SwordManifestVfxPlayer` | `bong:sword_manifest_summon` / `bong:sword_manifest_strike` |
| `HeavenGateShockwaveVfxPlayer` | `bong:heaven_gate_shockwave` |
| `HeiwushiTransformVfxPlayer` | `bong:heiwushi_transform` |
| `HeiwushiDeathVfxPlayer` | `bong:heiwushi_death` |

---

## P5 — 测试补全 + 集成 + InspectScreen

### P5.1 v1 遗留单测补全

**bond.rs**（5 tests）：
- 连续 20 次使用剑术 → `SwordBondComponent` 挂载成功
- 19 次使用 → 不挂载
- 已绑定时换剑 → 旧绑定不自动解除
- 解绑仪式 30s → bond_strength 降 50%
- 1 player 绑定 2 剑 → 拒绝第二绑定

**techniques.rs**（3 tests）：
- 经脉依赖 SEVERED → cast 拒绝
- 无剑持有 → cast 拒绝
- 招式战斗效果（命中/AoE/追踪）结算正确

**tiandao_blind.rs**（1 test）：
- blind zone 内玩家不出现在 `world_state` 推送中

### P5.2 集成测试

`server/tests/sword_path_e2e.rs` 或 `scripts/e2e/sword-path-lifecycle.sh`：

1. **剑道全流程**：拾取剑 → 20 次使用绑定 → 凝锋 buff → 剑气斩命中 → 剑鸣 AoE → 品阶升级 → 剑意化形追踪
2. **化虚终极**：化虚境界 + 六阶剑 → 一剑开天门 → 100 格 AoE → 天道盲区 5min → 修为归零 + 剑碎
3. **天道屏蔽**：化虚一击后 → agent `world_state` 不含盲区内玩家 → 5min 后恢复
4. **黑武士 BOSS**：Phase 1 → Phase 2 → Phase 3 → 击杀 → 掉落验证
5. **残卷解锁**：拾取 `scroll_sword_condense` → 右键使用 → `KnownTechniques` 新增 → 可 cast 凝锋
6. **VFX 管线**：全部 9 个 VfxPlayer 事件正常触发（不校验视觉，校验事件发送）

### P5.3 InspectScreen 灵剑信息扩展

`client/src/main/java/.../InspectScreen.java` 装备面板灵剑条目新增：
- 品阶行：`"品阶：三阶·凝"`
- 封存真元行：`"封存：42.5 / 75.0"`
- 绑定强度行：`"人剑合一：78%"`

### P5.4 Client network handler

`SwordBondHudStateStore` 接入 `ServerDataPayloadV1` 解析：
- 新增 `SwordBondStateV1` schema（in_bond / grade / stored_qi / bond_strength / qi_cap）
- handler 解析后写入 store → HudPlanner 自动读取

---

## 开放问题

1. **残卷分卷策略**：5 招各一卷 vs 保持通用卷随机解锁？→ 建议分卷（玩家有明确追求目标），通用卷保留给 loot 表随机 roll
2. **黑武士刷新位置**：固定 vs 随机？→ 建议固定（铸剑古殿地下室，叙事锚定）
3. **剑匣机制**：v1 开放问题 #1 提到多剑切换——v2 暂不做，留 v3

---

## Finish Evidence

（迁入 `finished_plans/` 前必填）
