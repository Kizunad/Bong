# Bong · plan-knockback-physics-v1

**击退物理系统**——统一的力学模型取代全项目 ad-hoc 硬编击退距离。牛顿力学类比（F/m=a→v→d），真元池比作为质量的一部分（真元越满越难推），碰撞物理（撞墙二次伤害 + 方块破坏 + 撞人连锁）。本 plan 是**物理基建**，所有涉及击退的 plan 统一向此取值。

**世界观锚点**：
- `worldview.md §四:332-340` 距离衰减/拼刺刀——近战频繁、击退距离短但每击都有
- `worldview.md §四:351-358` 过载撕裂——爆脉瞬间 20 点真元 = 巨大冲击力
- `worldview.md §四:360-389` 全力一击——"化虚老怪一击轰塌山门"= 击退力 + 碰撞破坏的极端案例
- `worldview.md §四:155-168` 真元是极度致密的高维流体——真元满的修士物理上更"重"
- `worldview.md §五:399-401` 体修/爆脉——零距离强灌 = 天然最高击退效率
- `worldview.md §五:432-435` 截脉/震爆——皮下过载震爆 = 反向击退力来源
- `worldview.md §四:237-258` 16 部位伤口——撞墙产生的二次伤害应落在对应体表部位

**交叉引用**：
- `plan-qi-physics-v1`（finished）：本 plan 在 qi_physics 下新增 `knockback` 子模块
- `plan-combat-no_ui`（finished）：`AttackIntent` / `CombatEvent` / `DerivedAttrs` / resolve.rs 挂钩
- `plan-armor-v1`（finished）：护甲重量 → `BodyMass.armor_mass`；护甲减伤 → 碰撞二次伤害
- `plan-weapon-v1`（finished）：`WeaponKind` → 不同武器击退效率
- `plan-sword-basics-v1`（active）：劈/刺/格各有不同 knockback_efficiency
- `plan-movement-overhaul-v1`（skeleton）：玩家击退管线前置

**前置依赖**：
- `DerivedAttrs` ✅（`server/src/combat/components.rs:266`）
- `Stamina` / `StaminaState::Exhausted` ✅（`components.rs:84`）
- `DashState` / `ActiveOverride::Knockback` ✅（`npc/movement.rs:128`）
- `PendingKnockback` ✅（`movement.rs:287`）
- `apply_default_block_break()` ✅（`world/block_break.rs:32`）
- sweep 碰撞检测 ✅（`movement.rs:501-560`）
- `qi_physics` 模块 ✅

**反向被依赖**：所有涉及击退的当前和未来 plan

---

## 接入面 Checklist

- **进料**：`AttackIntent` / `CombatEvent` / `Weapon` / `DerivedAttrs` / `Stamina` / `Cultivation` / `MundaneArmor` / `PendingKnockback` / `DashState`
- **出料**：`qi_physics::knockback::KnockbackResult` / `CollisionResult` / `CombatEvent.collision_damage` / `block_break` / `PendingKnockback`（扩展） / `KnockbackEvent`
- **共享类型**：新增 `BodyMass` component / `Stance` enum / `qi_physics::knockback` 模块 / `KnockbackEvent` / 改动 `PendingKnockback` 扩展字段 / 废弃固定 `KNOCKBACK_DISTANCE`
- **跨仓库契约**：server: `qi_physics::knockback` + `BodyMass` + `Stance` + `KnockbackEvent` / client: `KnockbackSyncV1` payload / agent: 无
- **worldview 锚点**：§四 全力一击/过载撕裂/16 部位
- **qi_physics 锚点**：新增 `qi_physics::knockback` 子模块

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | 力学模型 + qi_physics::knockback + BodyMass + Stance | ⬜ |
| P1 | 战斗集成（resolve.rs 挂钩 + 武器/招式效率表 + NPC 统一） | ⬜ |
| P2 | 碰撞物理（撞墙二次伤害 + 方块破坏 + 撞人连锁） | ⬜ |
| P3 | 饱和测试 | ⬜ |

---

## P0 — 力学模型

### 核心公式

```
击退距离 = F / R × DISTANCE_SCALE
  F = (physical_damage × 1.0 + qi_invest × 2.0 + attacker_mass_bonus) × knockback_efficiency
  R = body_mass_total × stance_factor × (1.0 + qi_fill_ratio × 0.5)
  DISTANCE_SCALE = 0.05（全局调参）
```

### BodyMass Component

```rust
// server/src/combat/body_mass.rs
pub struct BodyMass {
    pub base_mass: f64,      // 人类默认 70.0
    pub armor_mass: f64,     // 护甲重量求和
    pub inventory_mass: f64, // item_count × 0.5，上限 30.0
}
```

NPC 质量：骨煞 30.0 / 异变兽 120-500 / 道伥 60.0。

### Stance 系统

```rust
pub enum Stance {
    Rooted    => 2.5,  // 主动扎根
    Braced    => 1.5,  // 防御姿态（格挡中）
    Standing  => 1.0,
    Moving    => 0.85,
    Casting   => 0.7,
    Sprinting => 0.5,
    Exhausted => 0.4,
    Airborne  => 0.2,
}
```

从现有状态（Stamina / MovementMode / CastPhase / parrying）自动推导。

### 常数

```rust
pub const PHYSICAL_FORCE_RATIO: f64 = 1.0;
pub const QI_FORCE_RATIO: f64 = 2.0;
pub const ATTACKER_MASS_TRANSFER_RATIO: f64 = 0.1;
pub const DISTANCE_SCALE: f64 = 0.05;
pub const QI_ANCHORING_COEFFICIENT: f64 = 0.5;
pub const MAX_KNOCKBACK_DISTANCE: f64 = 30.0;
```

---

## P1 — 战斗集成

### 武器击退效率表

| WeaponKind | efficiency |
|---|---|
| Staff | 1.2 |
| Saber | 0.9 |
| Fist | 0.8 |
| Sword | 0.6 |
| Spear | 0.4 |
| Dagger | 0.3 |
| Bow | 0.1 |

### 招式修正

| AttackSource | modifier |
|---|---|
| Melee | 1.0 |
| SwordCleave | 1.5 |
| SwordThrust | 0.4 |
| BurstMeridian | 2.5 |
| FullPower | 5.0 |
| QiNeedle | 0.05 |

### 替换 NPC 固定击退

废弃 `KNOCKBACK_DISTANCE = 4.0` / `KNOCKBACK_DURATION_TICKS = 5`，`PendingKnockback` 扩展为动态 distance/velocity/duration。

---

## P2 — 碰撞物理

### 撞墙

```
kinetic_energy = 0.5 × mass × velocity²
entity_damage = kinetic_energy × 0.3 × (1 - armor_defense)
block_stress = kinetic_energy × 0.5
block_broken = block_stress > block.hardness
```

方块硬度：草/土 1.0 / 木 2.0 / 石 5.0 / 铁 15.0 / 黑曜石 50.0 / 残灰 0.5 / 灵木 8.0。连续穿透上限 3 个方块。

### 撞人（保龄球）

简化弹性碰撞：动量守恒，轻的飞更远。双方各受 `kinetic_energy × mass_ratio × 0.2` 伤害。被撞者获得二次击退。连锁上限 3 级，每级动量 ×0.5 衰减。

碰撞伤害走 `WoundKind::Blunt` + 护甲减伤。

---

## P3 — 饱和测试

1-11. P0 力学公式正确性
12-19. P1 战斗集成（剑劈 1-3 格 / 爆拳 5-8 格 / 全力一击→30 上限 / 气针无击退 / Braced 减 33% / Exhausted 增 150%）
20-31. P2 碰撞（土墙碎/石墙不碎/穿透 3 上限/撞人动量交换/连锁 3 级/护甲减伤）
32-36. 守恒安全（碰撞不产真元/方块走 block_break/连锁上限/MAX clamp/NPC 玩家统一）
37-40. 回归（baomai≈3格 / skull-fiend≈6格 / zhenfa≈2格 / NPC≈4格）

## Finish Evidence

### 落地清单

- P0 力学模型：`server/src/qi_physics/knockback.rs` 提供 `KnockbackInput` / `KnockbackResult` / `compute_knockback()` / `wall_collision()` / `entity_collision()`；`server/src/combat/body_mass.rs` 提供 `BodyMass`、`Stance`、装备重量同步、NPC archetype 质量表。
- P1 战斗集成：`server/src/combat/knockback.rs` 提供武器/攻击来源击退映射；`server/src/combat/resolve.rs` 在命中后插入动态 `PendingKnockback` 并发出 `KnockbackEvent`；`server/src/npc/skull_fiend.rs` 切到动态击退入口。
- P2 碰撞物理：`server/src/npc/movement.rs` 扩展 `PendingKnockback` / `ActiveOverride::Knockback`，处理撞墙钝伤、软块破坏、撞人连锁、动能衰减和链深上限。
- P3 饱和测试：server 覆盖 body mass、姿态、公式、武器/来源映射、撞墙、撞人、战斗 resolve、骨煞回归；agent/client 覆盖 `knockback_sync` schema、router 注册和 `hit_pushback` 视觉复用。
- 跨仓库契约：server `server/src/schema/server_data.rs` + `server/src/network/knockback_sync_emit.rs`；agent `agent/packages/schema/src/server-data.ts` + generated schema；client `client/src/main/java/com/bong/client/network/KnockbackSyncHandler.java`。

### 关键 commit

- `efb697456` · 2026-05-12 · `feat(knockback): 建立统一击退物理模型`
- `7dc6d2145` · 2026-05-12 · `feat(knockback): 接入战斗击退与碰撞链`
- `d7ecc5b43` · 2026-05-12 · `feat(knockback): 同步击退客户端契约`
- `dfec33de2` · 2026-05-12 · `test(client): 同步鲸实体 raw id 断言`（client 基线测试漂移修正，运行时未改）

### 测试结果

- `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` → 4629 passed。
- `cd server && cargo test body_mass --all-targets && cargo test knockback --all-targets && cargo test skull_fiend --all-targets` → 5 + 14 + 8 passed。
- `cd agent && npm run build && npm test --workspace @bong/schema` → schema 20 files / 384 tests passed。
- `cd client && JAVA_HOME=$HOME/.sdkman/candidates/java/17.0.18-amzn PATH=$HOME/.sdkman/candidates/java/17.0.18-amzn/bin:$PATH ./gradlew test build` → 1389 tests passed，build successful。

### 跨仓库核验

- server：`qi_physics::knockback`、`BodyMass`、`Stance`、`KnockbackEvent`、`PendingKnockback::from_result()`、`KnockbackSyncV1`。
- agent：`ServerDataKnockbackSyncV1`、`server-data-knockback-sync-v1.json`、`ServerDataV1` union。
- client：`knockback_sync` router type、`KnockbackSyncHandler`、`VisualEffectState.EffectType.HIT_PUSHBACK`。

### 遗留 / 后续

- 玩家物理位移仍等待 `plan-movement-overhaul-v1` 接入 server-authoritative player movement；本 plan 已同步客户端击退视觉并保持 NPC 动态击退生效。
