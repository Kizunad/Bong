# Bong · plan-baomai-v2 · 完成

**越级原则 + 全力一击 / 全战斗系统层**。把 worldview §四 越级原则与全力一击（commit d5e528aa 已正典）实装为：① 池子差距矩阵 const + `realm_gap_multiplier()` 函数（30 对组合）② 全力一击双 skill（charge / release）走 hotbar-modify-v1 SkillBar 现成框架 ③ Exhausted status effect（按 qi_committed 比例的虚脱时长）④ 完整专属 client UI（蓄力 / 释放 / 虚脱 三态）。

**⚠️ plan 名澄清**：journey-v1 §G 用 `plan-baomai-v2` 命名（暗示爆脉流 v2），实际 worldview §四 越级 + 全力一击是**全战斗系统机制**（不绑流派任何流派都可触发"全力一击"）。本 plan 保留 journey 已用名 `plan-baomai-v2` 但**范围跨流派**，baomai-v1 P1+ 的贴山靠 / 血崩步 / 逆脉护体 / 燃命 4 招实装留 plan-baomai-v3。

**世界观锚点**：
- `worldview.md §四 越级原则与全力一击` (commit d5e528aa **本 plan 全部物理根基**——池子差距矩阵 6 境界 + 越级 4 档可行性 + 全力一击三特征 charge/虚脱/不可日常)
- `worldview.md §三 进入境界时的池子大小（最低门槛路径）` (池子差距矩阵的数值依据)
- `worldview.md §四 距离衰减` (全力一击与近战 / 远程的伤害互动)
- `worldview.md §四 异体排斥` (qi 注入排斥系数与全力一击伤害穿透关系)

**library 锚点**：待写 `cultivation-XXXX 一击录`（化虚老怪 / 通灵渡劫救场亲历者关于"全力一击"的实战手记 + 极少数越级偷一波成功的偷袭案例集）

**交叉引用**：
- `plan-baomai-v1`（✅ finished，**强前置**）—— `cultivation::skill_registry::SkillRegistry` + `combat::components::SkillBarBindings` + `burst_meridian` 模板；本 plan 沿用同框架注册新 skill `bao_mai.full_power_charge` + `bao_mai.full_power_release`
- `plan-hotbar-modify-v1`（✅ finished，**强前置**）—— SkillBar 拖入绑定机制；本 plan 双 skill 玩家拖入任意两槽位即可
- `plan-cultivation-v1`（✅ finished）—— `Cultivation::qi_current` / `qi_max` / `Realm` 全用
- `plan-combat-no_ui`（✅ finished）—— `AttackIntent` / `DamageEvent` 路径；本 plan 加 `FullPowerAttackIntent` 走相同结算链路
- `plan-combat-ui_impl`（✅ finished）—— 客户端伤害 VFX 框架，本 plan 加专属释放雷光
- `plan-armor-v1`（✅ finished）—— Exhausted status effect 接 armor mitigation `defense_modifier` 槽位
- `plan-skill-v1`（✅ finished）—— `Casting` 状态机；ChargingState 与 Casting 是不同 Component（Casting = 短瞬法术，Charging = 长蓄力）
- `plan-tribulation-v1`（active ⏳）—— "P3-P5 渡劫救场" 留 hook，由 tribulation-v1 后续接入（"渡劫第三波必须自渡 = 不能用全力一击外援"——但自身渡劫者可以全力一击对抗劫雷）
- `plan-narrative-political-v1`（active 2026-05-04）—— 全力一击成功击杀化虚级 / 高 Renown 目标 → 触发 `high_renown_milestone` event（"以下犯上一击毙命"江湖传闻）
- `plan-multi-style-v1`（active）—— 本 plan **不接 PracticeLog**（"全力一击"是全战斗机制不归任何流派 practice）

**阶段总览**：

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | 池子差距矩阵 6×6 const + `realm_gap_multiplier(attacker, defender) -> f32` 公共函数 + 30 对组合 boundary 单测 | ✅ |
| P1 | `FullPowerStrike` 系统：`ChargingState` Component + `bao_mai.full_power_charge` / `bao_mai.full_power_release` 双 skill 注册 + 双键 hotbar 走 SkillBar 既有框架 + `FullPowerAttackIntent` 走 combat 结算 + 打断逻辑（30% qi loss）+ 战后虚脱挂载 | ✅ |
| P2 | `Exhausted` status effect 实装：时长 = `qi_committed × 0.1 秒` + qi_recovery -50% modifier + 防御 -50% modifier + expire system | ✅ |
| P3 | 测试 + tribulation hook + narrative-political `high_renown_milestone` hook | ✅ |
| P4 | client UI：`ChargingProgressBarHud` + `ChargingOrbVfx`（caster 周围蓄力球粒子）+ `ReleaseLightningVfx`（释放瞬间雷光）+ `ExhaustedGreyOverlay`（HUD 灰晕 + 玩家身上灰雾粒子） | ✅ |

---

## 接入面 checklist（防孤岛）

| 维度 | 内容 |
|------|------|
| **进料** | `Cultivation { qi_current, qi_max, realm }` (cultivation-v1) · `SkillBarBindings` (combat-no_ui) · `SkillRegistry` (cultivation skill_registry) · `DamageEvent` (combat-no_ui) · `Realm` enum (cultivation-v1) · 玩家 raycast target lookup（baomai-v1 崩拳现成模式） |
| **出料** | `realm_gap_multiplier()` 公共函数（grep 抓手）· `ChargingState` Component（玩家蓄力中标记）· `FullPowerAttackIntent` Bevy event · `Exhausted` status effect Component · `FullPowerStrikeKilledEvent`（化虚级目标被击杀时 emit，给 narrative-political 消费） |
| **共享 event** | 复用 `AttackIntent` / `DamageEvent`（combat-no_ui 既有）；新增 `ChargeStartedEvent` / `ChargeInterruptedEvent` / `FullPowerReleasedEvent` / `ExhaustedExpiredEvent`（仅本 plan 内部 + agent / client 推送） |
| **跨仓库契约** | **server**：`REALM_GAP_MATRIX` const / `realm_gap_multiplier` fn / `FullPowerStrikeKilledEvent` / `ChargingState` / `Exhausted` Components / `bao_mai.full_power_charge` + `bao_mai.full_power_release` SkillRegistry 注册 / `charge_tick_system` / `charge_interrupt_system` / `release_full_power_fn` / `exhausted_expire_system` / `Exhausted` recovery + defense hooks<br>**agent**：本 plan **无 agent narration 主动产出**；新增 `full_power_charging_state` / `full_power_release` / `full_power_exhausted_state` server_data schema，击杀高境目标通过既有 `SocialRenownDeltaEvent` fame_delta 链路接 narrative-political，不新增孤立 Redis 频道<br>**client**：`ChargingProgressBarHud` Java HUD / `ChargingOrbVfx` particle hook / `ReleaseLightningVfx` particle hook / `ExhaustedGreyOverlay` HUD shader / `full_power_charging_state` server_data（caster 自身 HUD）+ `bong:charging_orb` VFX（周围玩家可见）/ `full_power_release` server_data + `bong:release_lightning` VFX（释放瞬间）/ `full_power_exhausted_state` server_data + `bong:exhausted_grey_mist` VFX |
| **worldview 锚点** | §四 越级原则 + 全力一击（全节）+ §三 池子差距数值依据 + §四 距离衰减（hook）+ §四 异体排斥（hook） |
| **红旗自查** | ❌ 自产自消（接 cultivation / combat / armor / skill / hotbar / tribulation / narrative-political） · ❌ 近义重名（沿用 SkillRegistry / SkillBarBindings / Realm / DamageEvent，新增 ChargingState / Exhausted / REALM_GAP_MATRIX 都是新概念） · ❌ 无 worldview 锚（§四 §三 双锚） · ⚠️ skeleton 同主题：plan-baomai-v1 ✅ 已 finished（本 plan 是它的"v2 数值实装" + "全战斗系统扩展"，非另起） · ❌ 跨仓库缺面（server + client 必涉及；agent 沿用既有 narrative-political 链路） |

---

## §0 设计轴心

- [x] **plan 范围跨流派**（Q1 A）—— 本 plan 是"全力一击"全战斗机制层，不绑爆脉流；任何流派玩家在 hotbar 拖入双 skill 都可用。baomai-v1 P1+ 4 招（贴山靠 / 血崩步 / 逆脉护体 / 燃命）留 plan-baomai-v3
- [x] **双 skill 双槽 hotbar 模式**（Q2 B）—— 沿用 plan-hotbar-modify-v1 SkillBar 框架，注册 `bao_mai.full_power_charge` 和 `bao_mai.full_power_release` 两 skill；玩家拖入任意两槽位（如 1=charge / 2=release），按 1 蓄力，按 2 释放
- [x] **充能可被打断 30% qi loss**（Q3 A）—— 任何 DamageEvent 命中 caster → 强制取消 charge → 退还 qi_committed × 60%（损失 30%；额外 10% 是"已转化无法回收"的物理代价）
- [x] **虚脱时长 = qi_committed × 0.1 秒**（Q4 B）—— `release` 时 `Exhausted::recovery_at_tick = now + qi_committed × 2 ticks`（vanilla 20 tps，0.1 秒/qi → 50 qi 虚脱 5 秒，500 qi 虚脱 50 秒，2000 qi 虚脱 200 秒 ≈ 3 分钟）—— 匹配 worldview "数十秒到数分钟"
- [x] **越级数值仅"全力一击"用**（Q5 A）—— 常规 AttackIntent / DamageEvent 不动公式，保留流派 trade-off matrix 不变；仅 `FullPowerAttackIntent` 走 `realm_gap_multiplier()` 换算伤害（worldview "唯一例外：全力一击"完全一致）
- [x] **NPC AI 不实装**（Q6 A）—— 本 plan 仅玩家可触发；NPC AI 全力一击留 plan-baomai-v3（worldview "高境强者很少全力出手"，NPC 早期没必要 AI 决策这个）
- [x] **完整专属 UI**（Q7 A）—— 蓄力 progress bar + 蓄力球粒子（caster 周围越蓄越亮）+ 释放雷光 + 虚脱灰晕——worldview "化虚老怪一掌轰塌山门"需要仪式感视觉；周围玩家也应看到 caster 蓄力（PVP 反制窗口的视觉提示）
- [x] **不动伤害公式 / 不动 PracticeLog** —— 全力一击的伤害是"qi_released × realm_gap_multiplier"独立公式，不走流派伤害链路；不写 baomai PracticeLog（这不是 baomai 流派）

---

## §1 第一性原理（worldview §四 推导）

- **池子差距是物理事实**（§三 + §四）—— 6 境界进入时 qi_max 比例 1 : 4 : 15 : 54 : 210 : 1070；这是基础矩阵
- **常规战斗不直接换算**（§四 line 380-391）—— "决定胜负的是流派对位、技巧、天时地利"；流派 trade-off matrix（§五 §六 既有）已实装
- **全力一击是唯一例外** —— "把整个池子或大半池子一次性灌出去 → 池子差距才直接换算为伤害"；这是数学事实但仅在此机制下生效
- **charge 窗口 = 反制时机** —— worldview "半秒到数秒，低境者反制时机（埋阵法 / 扔暗器 / 弹反）"；本 plan 充能可被打断 = 实装这个反制窗口
- **战后虚脱 = 物理代价** —— 真元池骤空 → 经脉空虚 → "回复速率减半 + 防御 -50%" worldview 直接给出修正系数
- **不可日常的设计意图** —— "化虚老怪一击轰塌山门后，自己也得调息半个时辰"——本 plan 虚脱时长按 qi_committed 比例做 = 化虚老怪一击约 2000 qi → 200 秒 ≈ 3 分钟虚脱（worldview "半个时辰"压缩到游戏感知尺度）

---

## §2 P0 — 池子差距矩阵 const + 公共函数

### 数据 const（`server/src/combat/realm_gap.rs` 新文件）

```rust
//! 越级原则 - 池子差距矩阵 (worldview §四 line 364-372)
//!
//! 行 = 攻击者境界，列 = 防御者境界
//! 数值 = "攻击者 qi_max / 防御者 qi_max" 比率，源自 §三 进入境界时 qi_max 表

use crate::cultivation::components::Realm;

/// 6×6 池子差距矩阵。索引顺序与 Realm enum 一致：
/// 0 = Awaken, 1 = Induce, 2 = Condense, 3 = Solidify, 4 = Spirit, 5 = Void
pub const REALM_GAP_MATRIX: [[f32; 6]; 6] = [
    /*                   def: Awaken  Induce  Condense Solidify  Spirit    Void  */
    /* atk: Awaken */   [1.0,    0.25,    0.067,   0.019,    0.0048,   0.00093],
    /* atk: Induce */   [4.0,    1.0,     0.267,   0.074,    0.019,    0.0037],
    /* atk: Condense */ [15.0,   3.75,    1.0,     0.278,    0.071,    0.014],
    /* atk: Solidify */ [54.0,   13.5,    3.6,     1.0,      0.257,    0.051],
    /* atk: Spirit */   [210.0,  52.0,    14.0,    3.89,     1.0,      0.196],
    /* atk: Void */     [1070.0, 268.0,   71.0,    19.8,     5.1,      1.0],
];

pub fn realm_gap_multiplier(attacker: Realm, defender: Realm) -> f32 {
    REALM_GAP_MATRIX[attacker as usize][defender as usize]
}

/// 越级可行性分类（worldview §四 line 374-378）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RealmGapTier {
    Equal,        // ratio ~ 1.0
    OneStepUp,    // 3.6 - 5.1（可咬一口）
    TwoStepUp,    // 13 - 71（绝望）
    ThreeStepUp,  // 52+（没有战斗，只有踩死）
    Lower,        // < 1.0（防御者高于攻击者）
}

pub fn classify_gap(ratio: f32) -> RealmGapTier {
    match ratio {
        r if r < 0.95 => RealmGapTier::Lower,
        r if r < 1.5 => RealmGapTier::Equal,
        r if r < 6.0 => RealmGapTier::OneStepUp,
        r if r < 100.0 => RealmGapTier::TwoStepUp,
        _ => RealmGapTier::ThreeStepUp,
    }
}
```

### 测试（饱和化）

- [x] `realm_gap_matrix_diagonal_is_one`（同境界 ratio = 1.0）
- [x] `realm_gap_matrix_inverse_relation`（如 `multiplier(Spirit, Awaken) × multiplier(Awaken, Spirit) ≈ 1.0`）
- [x] **30 对组合 boundary case**：每对 (atk, def) 都验证 `multiplier()` 返回 worldview §四 表中数值（误差 ±0.05）
- [x] `classify_gap_one_step_returns_one_step_up`（boundary 3.6 - 5.1）
- [x] `classify_gap_two_step_returns_two_step_up`（boundary 13 - 71）
- [x] `classify_gap_three_step_returns_three_step_up`（boundary 52+）
- [x] `classify_gap_lower_returns_lower`（防御者高于攻击者）
- [x] `classify_gap_equal_returns_equal`（同境界）

---

## §3 P1 — FullPowerStrike + 双 skill 注册 + 打断逻辑

### Component / Event 定义（`server/src/cultivation/full_power_strike.rs` 新文件）

```rust
#[derive(Debug, Clone, Component)]
pub struct ChargingState {
    pub started_at_tick: u64,
    pub qi_committed: u32,
    pub target_qi: u32,         // 玩家目标蓄力（默认 = qi_max，封顶不超过 qi_current 起手值）
}

#[derive(Debug, Clone, Event)]
pub struct ChargeStartedEvent {
    pub caster: Entity,
    pub started_at_tick: u64,
    pub initial_qi: u32,
}

#[derive(Debug, Clone, Event)]
pub struct ChargeInterruptedEvent {
    pub caster: Entity,
    pub qi_lost: u32,            // 30% loss
    pub qi_refunded: u32,        // 60% refund
    pub trigger: InterruptTrigger,  // ByDamage / ByMovement / ByPlayer
    pub at_tick: u64,
}

#[derive(Debug, Clone, Event)]
pub struct FullPowerAttackIntent {
    pub caster: Entity,
    pub target: Option<Entity>,   // raycast 找到 / None = 空射
    pub qi_released: u32,
    pub at_tick: u64,
}

#[derive(Debug, Clone, Event)]
pub struct FullPowerStrikeKilledEvent {
    pub caster: Entity,
    pub target: Entity,
    pub target_realm: Realm,
    pub at_tick: u64,
    // narrative-political-v1 P1 high_renown_milestone consumer 用此事件触发"以下犯上"江湖传闻
}

pub const FULL_POWER_CHARGE_RATE_PER_TICK: u32 = 50;     // 每 tick 转化 50 qi
pub const FULL_POWER_MIN_QI_TO_START: u32 = 100;          // 起步阈值
pub const EXHAUST_TICKS_PER_QI_COMMITTED: u64 = 2;        // 0.1 秒/qi（@ 20 tps）
```

### 双 skill SkillRegistry 注册

```rust
// 在 cultivation::skill_registry::init_registry() 中加入：
registry.register("bao_mai.full_power_charge", start_charge_fn);
registry.register("bao_mai.full_power_release", release_full_power_fn);
```

### `start_charge_fn`（charge skill cast handler）

```
检查序列（任一失败 → CastResult::Reject）：
  1. caster 已有 ChargingState → Reject(AlreadyCharging)
  2. caster 已有 Exhausted → Reject(StillExhausted)
  3. caster.qi_current < FULL_POWER_MIN_QI_TO_START → Reject(InsufficientQi)
  4. caster.realm == Realm::None → Reject(NotEligible)

通过 → 添加 ChargingState { started_at_tick: now, qi_committed: 0, target_qi: caster.qi_current }
emit ChargeStartedEvent
```

### `charge_tick_system`（每 tick 推进充能）

```
对每个 ChargingState entity:
  to_consume = min(FULL_POWER_CHARGE_RATE_PER_TICK, caster.qi_current, target_qi - qi_committed)
  if to_consume == 0:
    # 蓄力满 → 自动停止（玩家可继续按 release）但不再消耗 qi
    continue
  caster.qi_current -= to_consume
  state.qi_committed += to_consume
```

### `release_full_power_fn`（release skill cast handler）

```
检查：
  1. caster 必须有 ChargingState → 否则 Reject(NotCharging)
  2. state.qi_committed < FULL_POWER_MIN_QI_TO_START → Reject(ChargedTooLittle)

执行：
  1. raycast 找最近 entity within 8 格（与 baomai-v1 崩拳同模式）
  2. emit FullPowerAttackIntent { caster, target: raycast_result, qi_released: state.qi_committed }
  3. 添加 Exhausted { recovery_at_tick: now + state.qi_committed * EXHAUST_TICKS_PER_QI_COMMITTED, ... }
  4. 移除 ChargingState
```

### `apply_full_power_attack_intent_system`（伤害结算）

```
for intent in FullPowerAttackIntent:
  let multiplier = realm_gap_multiplier(caster.realm, target.realm);
  let raw_damage = intent.qi_released as f32 * multiplier;
  
  # 应用流派加成 - 不应用！全力一击不走流派 trade-off
  # 应用距离衰减 - 复用 combat::distance_atten（如果用 ranged）
  # 应用异体排斥 - 复用 combat::xeno_rejection（attacker 流派 ρ）
  
  let final_damage = raw_damage * (1.0 - rho_attacker);
  
  emit DamageEvent { source: FullPower, target, damage: final_damage, ... }
  
  # 如果击杀且 target 是高境玩家 → emit FullPowerStrikeKilledEvent
  if target.is_killed() && target.realm >= Realm::Spirit {
    emit FullPowerStrikeKilledEvent { caster, target, target_realm }
  }
```

### `charge_interrupt_system`（打断逻辑）

```
监听 DamageEvent → 如果 target 有 ChargingState:
  qi_committed = state.qi_committed
  qi_refunded = (qi_committed as f32 * 0.6) as u32  // 60% refund
  qi_lost = qi_committed - qi_refunded               // 30% loss + 10% transformation cost
  caster.qi_current += qi_refunded
  emit ChargeInterruptedEvent { caster, qi_lost, qi_refunded, trigger: ByDamage }
  remove ChargingState
```

### 测试（饱和化）

- [x] **Happy path**：
  - `start_charge_adds_charging_state_when_qi_sufficient`
  - `charge_tick_consumes_qi_and_increases_committed`
  - `release_full_power_emits_attack_intent_with_committed_qi`
  - `release_full_power_adds_exhausted_state`
  - `full_power_attack_applies_realm_gap_multiplier`
- [x] **边界**：
  - `start_charge_rejected_when_qi_below_threshold`
  - `release_rejected_when_charged_too_little`
  - `start_charge_rejected_when_already_exhausted`
  - `start_charge_rejected_when_already_charging`
  - `charge_tick_caps_at_target_qi`（蓄力满后不再消耗）
  - `release_with_no_target_still_consumes_qi_and_exhausts`（空射代价）
- [x] **错误分支**：
  - `release_without_charging_state_rejected`
  - `start_charge_no_realm_rejected`
- [x] **状态转换**：
  - `charge_interrupted_by_damage_refunds_60_percent_qi`
  - `interrupted_charge_removes_charging_state`
  - `interrupted_charge_does_not_add_exhausted`（被打断不虚脱，仅损 qi）
  - `release_to_exhausted_to_normal_state_transition`
- [x] **击杀事件**：
  - `full_power_kill_high_realm_emits_killed_event`
  - `full_power_kill_low_realm_does_not_emit_killed_event`（低境击杀不触发"以下犯上"）

---

## §4 P2 — Exhausted status effect

### Component + system

```rust
#[derive(Debug, Clone, Component)]
pub struct Exhausted {
    pub started_at_tick: u64,
    pub recovery_at_tick: u64,
    pub qi_recovery_modifier: f32,  // 0.5
    pub defense_modifier: f32,       // 0.5
}

pub const EXHAUSTED_QI_RECOVERY_MODIFIER: f32 = 0.5;
pub const EXHAUSTED_DEFENSE_MODIFIER: f32 = 0.5;
```

### Hook 接入

- [x] `cultivation::tick::qi_recovery_tick` 系统加 query：`if Exhausted exists → recovery × 0.5`
- [x] `armor::resolve` 系统加 query：`if Exhausted exists → defense_modifier × 0.5`
- [x] `exhausted_expire_system`：每 tick 检查 `recovery_at_tick <= now` → 移除 Exhausted Component + emit `ExhaustedExpiredEvent`

### 测试

- [x] `exhausted_qi_recovery_is_halved`
- [x] `exhausted_defense_modifier_is_halved`
- [x] `exhausted_expires_after_correct_tick_count_per_qi_committed`
- [x] `exhausted_expire_emits_event`
- [x] `exhausted_50_qi_lasts_5_seconds`（boundary）
- [x] `exhausted_500_qi_lasts_50_seconds`
- [x] `exhausted_2000_qi_lasts_200_seconds`（化虚级老怪 boundary）
- [x] `exhausted_during_active_does_not_re_apply_modifier`（不重复 stack）

---

## §5 P3 — 测试 e2e + tribulation hook + narrative-political milestone

### e2e 集成测试

- [x] **完整 happy path**：玩家 charge → 满 qi → release → 击杀低境 NPC → Exhausted 50 秒 → 期间 qi_recovery -50% + 被打防御 -50% → expire → 恢复正常
- [x] **被打断 path**：charge 中被另一玩家攻击命中 → 30% qi loss + 不虚脱 → 立即可重新 charge（无冷却）
- [x] **越级偷一波 path**：凝脉玩家 (qi_max ~600) charge 全部 qi → 释放对固元玩家 (qi_max ~2000) → multiplier ~0.278 → 实际伤害 ~167 qi 等量穿透 → 凝脉玩家虚脱 60 秒
- [x] **化虚老怪 path**：化虚 NPC（dev spawn）charge 2000 qi → 释放对醒灵新人 → multiplier ×1070 → 一击秒杀 + emit FullPowerStrikeKilledEvent

### tribulation hook（不实装，仅留 docs）

- [x] `server/src/cultivation/full_power_strike.rs` 顶部注释 / docs/plans-skeleton 留 `tribulation-v1 vN+1 接入说明`：
  > 渡虚劫第三波 worldview §三 "无外援"——这意味着玩家"全力一击"自渡是核心策略；plan-tribulation-v1 后续 P5 应允许玩家在渡劫期间使用 FullPowerStrike 对劫雷计算结算（target = 劫雷实体，复用 multiplier）

### narrative-political milestone hook

- [x] `FullPowerStrikeKilledEvent` server 端保留为内部事件；不新增孤立 `bong:full_power_killed` Redis 频道，改走既有 `SocialRenownDeltaEvent` fame_delta / tag 链路给 narrative-political 消费
- [x] **本 plan 不动 narrative-political**——narrative-political-v1 P1 既有"高 Renown 出名"链路继续消费 renown delta。本 plan 确保击杀高境目标后 emit fame_delta（高境击杀 = +大 fame）
- [x] 测试：`full_power_kill_high_realm_emits_killed_and_renown_events`

---

## §6 P4 — client UI

### `ChargingProgressBarHud` Java HUD

- [x] caster 自身 HUD 中央显示 progress bar（qi_committed / target_qi 百分比）
- [x] 颜色渐变：浅红（< 30%）→ 红（30-70%）→ 紫红（> 70%）→ 金紫（满）
- [x] 文字："蓄力中... XXX/XXX 真元"

### `ChargingOrbVfx`（粒子球，server-driven）

- [x] caster 周围生成蓄力球粒子（半径 1 格，越蓄越大越亮）
- [x] 周围 N 格内其他玩家可见（PVP 反制窗口的视觉提示——worldview "低境者反制时机"）
- [x] CustomPayload `bong:charging_state` 同步：`{ caster_uuid, qi_committed, target_qi, started_tick }`
- [x] 周围玩家可看到"某修士在蓄力大招"——可决策是否打断 / 逃跑 / 反制

### `ReleaseLightningVfx`（释放瞬间）

- [x] 释放时 caster → target 一道紫红雷光（参考 vanilla lightning 但染色）
- [x] 命中点爆炸粒子
- [x] CustomPayload `bong:full_power_release` 同步：`{ caster_uuid, target_uuid, qi_released, hit_position }`

### `ExhaustedGreyOverlay`（虚脱期）

- [x] HUD 角落灰晕 shader（caster 自己看到自己虚脱状态）
- [x] caster 身上"灰雾"粒子（其他玩家可见 caster 虚脱中——破绽信号）
- [x] 进度条显示剩余虚脱时间

### 测试

- [x] client automated smoke：`FullPowerStateHandlerTest` + `ChargingProgressBarHudTest` + `ExhaustedGreyOverlayTest` 覆盖完整充能 → 释放 → 虚脱状态链路；live `runClient` 观感留后续人工 QA
- [x] PVP 可见性接线：server-driven `bong:charging_orb` / `bong:release_lightning` / `bong:exhausted_grey_mist` VFX 事件 + `VfxBootstrap` 注册 + particle players 编译通过；live 多人观感留后续人工 QA

---

## §7 数据契约（下游 grep 抓手）

| 契约 | 位置 |
|---|---|
| `REALM_GAP_MATRIX` 6×6 const | `server/src/combat/realm_gap.rs`（新文件） |
| `realm_gap_multiplier(attacker, defender) -> f32` 公共函数 | `server/src/combat/realm_gap.rs` |
| `RealmGapTier` enum + `classify_gap(ratio)` | `server/src/combat/realm_gap.rs` |
| `ChargingState` Component | `server/src/cultivation/full_power_strike.rs`（新文件） |
| `Exhausted` Component | `server/src/cultivation/full_power_strike.rs` |
| `FullPowerAttackIntent` Bevy event | `server/src/cultivation/full_power_strike.rs` |
| `ChargeStartedEvent` / `ChargeInterruptedEvent` / `ExhaustedExpiredEvent` | `server/src/cultivation/full_power_strike.rs` |
| `FullPowerStrikeKilledEvent` | `server/src/cultivation/full_power_strike.rs` |
| `FULL_POWER_CHARGE_RATE_PER_TICK = 50` const | `server/src/cultivation/full_power_strike.rs` |
| `FULL_POWER_MIN_QI_TO_START = 100` const | `server/src/cultivation/full_power_strike.rs` |
| `EXHAUST_TICKS_PER_QI_COMMITTED = 2` const | `server/src/cultivation/full_power_strike.rs` |
| `start_charge_fn` / `release_full_power_fn` SkillRegistry handler | `server/src/cultivation/full_power_strike.rs` |
| `bao_mai.full_power_charge` / `bao_mai.full_power_release` skill IDs | `server/src/cultivation/skill_registry.rs`（注册段） |
| `charge_tick_system` / `charge_interrupt_system` / `release_full_power_fn` / `exhausted_expire_system` / `apply_full_power_attack_intent_system` systems | `server/src/cultivation/full_power_strike.rs` |
| qi_recovery hook 接 `Exhausted::qi_recovery_modifier` | `server/src/cultivation/tick.rs` |
| defense mitigation hook 接 `Exhausted::defense_modifier` | `server/src/combat/status.rs` |
| `full_power_charging_state` server_data + `bong:charging_orb` VFX | `agent/packages/schema/src/full-power.ts` + `server/src/network/full_power_emit.rs` |
| `full_power_release` server_data + `bong:release_lightning` VFX | `agent/packages/schema/src/full-power.ts` + `server/src/network/full_power_emit.rs` |
| `full_power_exhausted_state` server_data + `bong:exhausted_grey_mist` VFX | `agent/packages/schema/src/full-power.ts` + `server/src/network/full_power_emit.rs` |
| 高境击杀 renown hook | `server/src/cultivation/full_power_strike.rs` emits `FullPowerStrikeKilledEvent` + `SocialRenownDeltaEvent` |
| `ChargingProgressBarHud` Java HUD | `client/src/main/java/com/bong/client/hud/ChargingProgressBarHud.java`（新） |
| `ChargingOrbVfx` particle hook | `client/src/main/java/com/bong/client/visual/particle/ChargingOrbVfx.java`（新） |
| `ReleaseLightningVfx` particle hook | `client/src/main/java/com/bong/client/visual/particle/ReleaseLightningVfx.java`（新） |
| `ExhaustedGreyMistVfx` particle hook | `client/src/main/java/com/bong/client/visual/particle/ExhaustedGreyMistVfx.java`（新） |
| `ExhaustedGreyOverlay` HUD shader | `client/src/main/java/com/bong/client/hud/ExhaustedGreyOverlay.java`（新） |

---

## §8 决议（立项时已闭环 7 项 + 1 plan 名澄清）

调研锚点：worldview §四 越级原则与全力一击 (commit d5e528aa) + §三 进入境界时 qi_max 表 + plan-baomai-v1 ✅（崩拳已实装走 SkillRegistry 框架）+ plan-hotbar-modify-v1 ✅（SkillBar 双行架构 + 双键拖入）+ plan-cultivation-v1 ✅（qi_current/qi_max/Realm）+ plan-combat-no_ui ✅（AttackIntent / DamageEvent）+ plan-armor-v1 ✅（defense modifier 槽位）+ plan-narrative-political-v1（active 2026-05-04，high_renown_milestone consumer 待接）。

| # | 问题 | 决议 | 落地点 |
|---|------|------|--------|
| **Q1** | plan 范围 + 命名 | ✅ A：仅"越级原则 + 全力一击"全战斗系统层；保 plan-baomai-v2 名（journey 已用）但头部澄清范围跨流派；baomai-v1 P1+ 4 招留 plan-baomai-v3 | 头部 + §0 设计轴心 |
| **Q2** | 充能 charge 键位 | ✅ B：双 skill 双槽 hotbar 模式（`bao_mai.full_power_charge` + `bao_mai.full_power_release`），玩家拖入任意两槽位（如 1=charge / 2=release）；沿用 plan-hotbar-modify-v1 SkillBar 现成框架 | §3 SkillRegistry 注册段 |
| **Q3** | 充能可被打断 | ✅ A：任何 DamageEvent 命中 caster → 强制取消 → 退还 60% qi（30% loss + 10% transformation cost） | §3 charge_interrupt_system |
| **Q4** | 战后虚脱时长 | ✅ B：`Exhausted::recovery_at_tick = now + qi_committed × 2 ticks`（0.1 秒/qi @ 20 tps）— 50 qi → 5s / 500 qi → 50s / 2000 qi → 200s ≈ 3 分钟（worldview "数十秒到数分钟" 完全对齐） | §4 全节 |
| **Q5** | 越级数值适用范围 | ✅ A：仅 `FullPowerAttackIntent` 走 `realm_gap_multiplier()` 换算；常规 AttackIntent / DamageEvent 不动公式（worldview "唯一例外：全力一击"完全一致） | §3 apply_full_power_attack_intent_system |
| **Q6** | NPC AI 全力一击 | ✅ A：本 plan 仅玩家可触发；NPC AI 决策留 plan-baomai-v3（worldview "高境强者很少全力出手" 早期 NPC 没必要 AI 决策） | §0 设计轴心 + §3 测试不含 NPC AI |
| **Q7** | UI 强度 | ✅ A：完整专属 UI——蓄力 progress bar + 蓄力球粒子 + 释放雷光 + 虚脱灰晕；周围玩家也能看到 caster 蓄力（PVP 反制窗口的视觉提示） | §6 全节 |
| **plan 名** | journey 用 plan-baomai-v2 是否合适 | ⚠️ 保留 journey 已用名但 **plan 头部明示范围跨流派** | 头部澄清段 |

> **本 plan 无未拍开放问题**——P0 可立刻起。P3 narrative-political milestone hook 验证依赖 narrative-political-v1 P1 的高 Renown consumer 是否真有 fame_delta 触发（如有歧义可在该 plan P1 落地时拍）。

---

## §9 进度日志

- **2026-05-05 立项**：骨架立项。来源：journey-v1 §G "DEF 三 plan 切入点 / plan-baomai-v2（派生）越级原则 + 全力一击战后虚脱实装（worldview §四 commit d5e528aa 已正典化但 baomai-v1 未实装数值）"。**关键发现**：(a) worldview §四 越级 / 全力一击是**全战斗系统层**机制，不限爆脉流——plan 命名误导但保留 journey 已用名 plan-baomai-v2 + 头部澄清；(b) plan-baomai-v1 已实装 SkillRegistry / SkillBarBindings 框架，本 plan 沿用注册新 skill 即可（双 skill 双槽走 hotbar-modify-v1 已有架构）；(c) Exhausted 时长按 qi_committed 比例做完美匹配 worldview "数十秒到数分钟"；(d) 高境击杀沿既有 `SocialRenownDeltaEvent` fame_delta/tag 链路接 narrative-political，不新增孤立 Redis 频道。7 决议（Q1-Q7）一次性闭环 + plan 名澄清。

## Finish Evidence

### 落地清单

- **P0 境界差距矩阵**：`server/src/combat/realm_gap.rs` 新增 `REALM_GAP_MATRIX`、`realm_gap_multiplier()`、`RealmGapTier`、`classify_gap()`，覆盖 6 境界矩阵、逆关系和分类边界。
- **P1 全力一击服务端机制**：`server/src/cultivation/full_power_strike.rs` 新增 `ChargingState`、`Exhausted`、`FullPowerAttackIntent`、`FullPowerReleasedEvent`、`FullPowerStrikeKilledEvent`、charge/release handler、打断、释放、虚脱过期、击杀 renown hook；`server/src/cultivation/skill_registry.rs` / `known_techniques.rs` 注册 `bao_mai.full_power_charge` 与 `bao_mai.full_power_release`。
- **P2 虚脱修正**：`server/src/cultivation/tick.rs` 接 `Exhausted::qi_recovery_modifier`，`server/src/combat/status.rs` 接 `Exhausted::defense_modifier`；`server/src/combat/resolve.rs` 为 `AttackSource::FullPower` 避免重复扣 qi。
- **P3 契约与事件**：`server/src/schema/server_data.rs`、`server/src/network/full_power_emit.rs`、`server/src/network/mod.rs` 输出 `full_power_charging_state` / `full_power_release` / `full_power_exhausted_state` server_data（HUD payload 定向发送给 caster，VFX 仍广播给周围玩家），并派发 `bong:charging_orb` / `bong:release_lightning` / `bong:exhausted_grey_mist` VFX；高境击杀通过 `FullPowerStrikeKilledEvent` + `SocialRenownDeltaEvent` 进入既有 renown/narrative 链路。
- **P4 agent/client**：`agent/packages/schema/src/full-power.ts`、`server-data.ts`、`schema-registry.ts` 和 generated JSON schema 同步新增 full_power payload；client 新增 `FullPowerStateStore`、`FullPowerStateHandler`、`ChargingProgressBarHud`、`ExhaustedGreyOverlay`、`ChargingOrbVfx`、`ReleaseLightningVfx`、`ExhaustedGreyMistVfx`，并在 `ServerDataRouter`、`BongHudOrchestrator`、`VfxBootstrap` 接线。

### 关键 commits

- `3212807ff` · 2026-05-09 · 实现全力一击服务端机制
- `645851032` · 2026-05-09 · 接入全力一击跨端契约
- `80f225d53` · 2026-05-09 · 接入全力一击客户端反馈
- `295558e68` · 2026-05-09 · 补全全力一击边界测试
- `93d4dd17e` · 2026-05-09 · 修复全力一击 review 问题

### 测试结果

- `cargo fmt --check`（server）通过。
- `cargo clippy --all-targets -- -D warnings`（server）通过。
- `cargo test`（server，review 修复后）通过：`3084 passed; 0 failed`。
- `cargo test full_power`（server，review 修复后）通过：`15 passed; 0 failed`。
- `JAVA_HOME="/usr/lib/jvm/java-17-openjdk-amd64" PATH="/usr/lib/jvm/java-17-openjdk-amd64/bin:$PATH" ./gradlew test build`（client）通过。
- `npm run build`（agent）通过：`@bong/schema` + `@bong/tiandao` TypeScript build。
- `npm test -w @bong/schema` 通过：`327 passed; 0 failed`。
- `npm test -w @bong/tiandao` 通过：`281 passed; 0 failed`。
- `npm run generate:check -w @bong/schema` 通过：`generated schema artifacts are fresh (301 files)`。
- `git diff --check` 通过。

### 跨仓库核验

- **server**：`REALM_GAP_MATRIX` / `realm_gap_multiplier` / `FullPowerAttackIntent` / `Exhausted` / `FullPowerStrikeKilledEvent` / `AttackSource::FullPower` / `ServerDataPayloadV1::FullPower*` / `emit_full_power_*`。
- **agent/schema**：`FullPowerChargingStateV1` / `FullPowerReleaseV1` / `FullPowerExhaustedStateV1` / `ServerDataFullPower*` / `CombatAttackSourceV1 = "full_power"` / generated `full-power-*.json`。
- **client**：`FullPowerStateStore` / `FullPowerStateHandler` / `ChargingProgressBarHud` / `ExhaustedGreyOverlay` / `ChargingOrbVfx` / `ReleaseLightningVfx` / `ExhaustedGreyMistVfx`。

### 遗留 / 后续

- 未启动 live `./gradlew runClient` 或多人 PVP 手工观感测试；本 PR 以 JVM unit tests、VFX bootstrap 注册、server/client/schema build 覆盖代码接线，实际多人视觉手感留后续人工 QA。
- NPC AI 主动使用全力一击仍按原计划留给 `plan-baomai-v3`。
- 渡劫劫雷实体作为 `FullPowerAttackIntent` target 的实战接入仍留给 `plan-tribulation-v1` 后续 P5；本 plan 已在 `full_power_strike.rs` 留 hook 注释。
