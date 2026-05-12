# Bong · plan-movement-overhaul-v1

**移动系统重构**——统一玩家/NPC 移动管线、腿伤影响移速（worldview 正典闭环）、护甲重量影响移速、Dash 加 proficiency、砍掉 Slide 和双重跳。本 plan 是移动层基建。

**世界观锚点**：
- `worldview.md §四:254-256` 腿部损伤→移速硬规则：正常 1.0 / 跛 0.7 / 残 0.4 / 动不了 0.0
- `worldview.md §四:332-340` 距离衰减/拼刺刀——dash 是核心战术
- `worldview.md §三:133-153` 境界越高移速越快（已实装 +5%/级）
- `worldview.md §四:155-168` 真元极度致密流体——Slide 无物理依据
- `worldview.md §四:351-358` 过载撕裂——双重跳需灵脉瞬间过载支撑体重，末法灵脉承受不起

**交叉引用**：
- `plan-movement-v1`（finished）：当前实装——**本 plan 是 v2 重构**
- `plan-combat-no_ui`（finished）：`DerivedAttrs` / `Wounds` / `StatusEffectKind::LegStrain`
- `plan-armor-v1`（finished）：`MundaneArmor` + 护甲重量
- `plan-knockback-physics-v1`（finished）：已实装 `qi_physics::knockback` / `BodyMass` / `KnockbackSyncV1` / `KnockbackEvent`——本 plan P3 接入玩家侧 Valence velocity 派发
- `plan-sword-basics-v1`（finished）：`KnownTechnique.proficiency` 框架——本 plan P4 Dash proficiency 同框架

**前置依赖**：
- `MovementState` ✅（`server/src/movement/mod.rs:127`）
- `MovementController` / `Navigator` ✅（`npc/movement.rs` + `navigator.rs`）
- `Wounds` / `BodyPart::LegL|LegR` ✅（`combat/components.rs`）
- `StatusEffectKind::LegStrain` ✅（`combat/events.rs:125`——存在但从未被创建）
- `MundaneArmor` ✅（`armor/mundane.rs`）
- `DerivedAttrs.move_speed_multiplier` ✅（`combat/components.rs:270`）
- `KnownTechnique.proficiency` ✅
- `qi_physics::knockback::compute_knockback()` ✅（`qi_physics/knockback.rs`）
- `BodyMass` ✅（`combat/body_mass.rs`）
- `KnockbackSyncV1` ✅（`schema/server_data.rs:467`）
- `KnockbackEvent` ✅（`combat/knockback.rs`）

**反向被依赖**：所有涉及移动限制的未来 plan

---

## 接入面 Checklist

- **进料**：`Wounds`（腿伤）/ `MundaneArmor`（重量）/ `Stamina` / `Cultivation.realm` / `KnownTechnique`（Dash prof）/ `PendingKnockback` / `KnockbackEvent` / `BodyMass`
- **出料**：`MovementState`（更新）/ `MovementStateV1`（精简 schema）/ `LegStrain` StatusEffect / `ActivePlayerKnockback` component / client HUD 更新
- **共享类型**：改动 `MovementAction` enum（删 Sliding/DoubleJumping）/ 改动 `MovementStateV1`（删 slide/jump 字段）/ 新增 `leg_wound_to_speed()` / 新增 `armor_weight_to_speed()` / 新增 `ActivePlayerKnockback` / 复用 `PendingKnockback`+`KnockbackEvent`（已由 knockback-physics-v1 实装）
- **跨仓库契约**：server: `movement/` 重构 + `leg_wound.rs` + `armor_weight.rs` + `player_knockback.rs` / client: 删 B 键 + Space 双重跳 + HUD 精简 + 删 Slide/DoubleJump VFX / agent: 删 `MovementStateV1` 旧字段
- **worldview 锚点**：§四:254-256 腿伤移速 + §四:332 拼刺刀 + §三:133 境界移速
- **qi_physics 锚点**：无新增（复用 `qi_physics::knockback`）

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | 砍 Slide/DoubleJump（server + client + schema + 清理视听资产） | ✅ 2026-05-13 |
| P1 | 腿伤→移速闭环（Wound → LegStrain → speed） | ✅ 2026-05-13 |
| P2 | 护甲重量→移速 | ✅ 2026-05-13 |
| P3 | 玩家击退管线（PendingKnockback → Valence velocity） | ✅ 2026-05-13 |
| P4 | Dash proficiency（KnownTechnique 注册 + 缩放曲线） | ✅ 2026-05-13 |
| P5 | 饱和测试 | ✅ 2026-05-13 |

---

## P0 — 砍 Slide / DoubleJump

### Server（`server/src/movement/mod.rs`）

**删除**：
- `MovementAction::Sliding` / `DoubleJumping` 变体（`mod.rs:64-65`）
- `From<MovementAction>` / `From<MovementActionRequestV1>` 对应分支（`mod.rs:79-80`, `mod.rs:89-90`）
- `double_jump_charges_remaining` / `double_jump_charges_max` 字段（`MovementState mod.rs:136-137`）
- `double_jump_charges_by_realm()` 函数及其调用（`mod.rs:249`）
- 所有 Slide 逻辑：hitbox 缩小 / 接触伤害 / 摩擦减速 / 冷却
- 所有 DoubleJump 逻辑：空中连跳 / 转向限制 / charge 恢复
- Slide 接触伤害 `inflicted_by: Some("movement_slide")` 分支（`mod.rs:848`）
- VFX 常量 `MOVEMENT_SLIDE` / `MOVEMENT_DOUBLE_JUMP`（`gameplay_vfx.rs:30-31`）
- `emit_action_effects()` 内 Sliding/DoubleJumping 分支（`mod.rs:657-672`）
- 音效 recipe 引用 `"movement_slide"` / `"movement_double_jump"`
- 音效测试断言 `movement_slide` / `movement_double_jump`（`audio/mod.rs:251-252`）

**保留**：`MovementAction::Dashing` + 全部 Dash 逻辑 + 基础速度公式。

### Schema

**Server**（`server/src/schema/movement.rs`）：
- 删除 `MovementActionRequestV1::Slide` / `DoubleJump`（`schema/movement.rs:8,16-17`）
- 删除 `MovementStateV1` 的 `slide_cooldown_remaining_ticks` / `double_jump_charges_remaining` / `double_jump_charges_max`（`schema/movement.rs:38-39` + `server_data.rs:3313-3314`）

**Agent**（`agent/packages/schema/src/movement.ts`）：
- 删除 `slide_cooldown_remaining_ticks` / `double_jump_charges_remaining` / `double_jump_charges_max`（`movement.ts:33-35`）
- 删除 `MovementActionRequestV1` 的 Slide / DoubleJump literal

### Client

**删除**：
- B 键 Slide 绑定（`MovementKeybindings.java:17,21,33-34,48,53`）
- Space 双重跳路由（`MovementKeyRouter.java:15-16`——`SLIDE` 分支）
- `MovementState.slideCooldownRemainingTicks` / `doubleJumpChargesRemaining` / `doubleJumpChargesMax` 字段（`MovementState.java:11` 等）
- `MovementStateHandler` 内解析 `slide_cooldown_remaining_ticks` / `double_jump_charges_remaining` / `double_jump_charges_max`（`MovementStateHandler.java:49-51`）
- `MovementHudPlanner.appendCooldown(... SLIDE_COLOR ...)` Slide cooldown arc（`MovementHudPlanner.java:64`）
- `MovementHudPlanner.appendDoubleJumpDots()` charge dots（`MovementHudPlanner.java:69,152-161`）
- `MovementVfxPlayer.Kind.SLIDE` / `DOUBLE_JUMP` 分支 + `playSlide()` / `playDoubleJump()` 方法（`MovementVfxPlayer.java:12-13,22-24,48-49,78-133,139-140,147-148,155-156`）
- `VfxBootstrap` 注册 `SLIDE` / `DOUBLE_JUMP`（`VfxBootstrap.java:177-178`）
- `BongAnimations.DOUBLE_JUMP` 引用（`BongAnimations.java:97,162`）
- `MovementState.Action.DOUBLE_JUMPING` 变体（`MovementState.java:129`）
- `MovementStateHandler` 内 `"double_jump"` action 解析（`MovementStateHandler.java:180`）

**删除资产文件**：
- `client/src/main/resources/assets/bong/player_animation/slide_low.json`
- `client/src/main/resources/assets/bong/player_animation/double_jump.json`

**保留**：V 键 Dash + `dash_forward.json` + Dash cooldown arc + Stamina bar。

---

## P1 — 腿伤→移速闭环

### worldview 正典映射

```rust
// server/src/movement/leg_wound.rs
pub fn leg_wound_to_speed(worst_wound: WoundGrade) -> f64 {
    match worst_wound {
        Intact | Bruise  => 1.0,   // 正常
        Abrasion         => 0.9,   // 轻微跛
        Laceration       => 0.7,   // 跛（worldview §四:254）
        Fracture         => 0.4,   // 残（worldview §四:255）
        Severed          => 0.0,   // 动不了（worldview §四:256）
    }
}

// 双腿综合 = min(左腿最严重, 右腿最严重)
pub fn combined_leg_factor(wounds: &Wounds) -> f64 {
    let left = worst_wound_grade(wounds, BodyPart::LegL);
    let right = worst_wound_grade(wounds, BodyPart::LegR);
    leg_wound_to_speed(left).min(leg_wound_to_speed(right))
}
```

### LegStrain 公式扩展

现有 `LegStrain`（`combat/status.rs:166`）的 clamp 上限从 0.6 → 1.0（允许 SEVERED = 100% 减速）。

```rust
// 触发 LegStrain StatusEffect
// magnitude = (1.0 - leg_factor) / 0.15
// 例：Laceration → (1.0 - 0.7) / 0.15 = 2.0 → clamp(0.0, 1.0) = 1.0 → 减速 15%
// 实际减速直接走 leg_wound_to_speed()，LegStrain effect 仅用于 HUD 显示和状态查询
```

### 速度公式整合

```rust
// server/src/movement/speed.rs
pub fn compute_move_speed(
    base: f64,
    realm_bonus: f64,        // 已有：+5%/级
    zone_modifier: f64,      // 已有：区域加减速
    stamina_factor: f64,     // 已有：体力低时减速
    leg_wound_factor: f64,   // P1 新增
    armor_weight_factor: f64,// P2 新增（本阶段传 1.0）
) -> f64 {
    base * realm_bonus * zone_modifier * stamina_factor * leg_wound_factor * armor_weight_factor
}
```

### Dash 与腿伤

- SEVERED (factor=0.0) → Dash 禁止，`MovementActionRejected` + reason "leg_severed"
- FRACTURE (factor≤0.4) → Dash 距离 ×0.5 + CD ×1.5
- LACERATION (factor≤0.7) → Dash 距离 ×0.8

### 视听

P1 不新增视听资产——复用已有系统：
- **跛行动画**：已有 `limp_left.json` / `limp_right.json`——根据哪条腿受伤播放对应动画
- **LegStrain HUD**：复用已有 `StatusEffectKind::LegStrain` 的状态条显示（`status_snapshot_emit.rs:100` "腿部应力伤"）
- **Dash 拒绝反馈**：复用 `MovementActionRejected` 已有客户端处理路径

---

## P2 — 护甲重量→移速

```rust
// server/src/movement/armor_weight.rs
pub fn armor_weight_to_speed(total_weight: f64) -> f64 {
    if total_weight < 5.0 { 1.0 }
    else if total_weight <= 15.0 { 1.0 - (total_weight - 5.0) * 0.015 }
    else { (0.85 - (total_weight - 15.0) * 0.01).max(0.65) }
}
// 布甲 ~3.0 → 1.0（无减速）
// 铁甲 ~10.0 → 0.925
// 板甲 ~12.0 → 0.895
// 全重甲 ~20.0 → 0.80
// 极限堆甲 → 下限 0.65
```

`total_weight` 从 `MundaneArmor` 已有的 `weight()` 方法求和（`mundane.rs:178`），穿戴变更时重算。

### 速度公式最终形态

```rust
speed = BASE × realm_bonus × zone_modifier × stamina_factor × leg_wound_factor × armor_weight_factor
```

各因子独立相乘，无顺序依赖。

### 视听

P2 不新增视听资产——负重减速是数值层变化，无独立感知行为。玩家通过已有的 `MovementStateV1.move_speed_multiplier` 下发感知速度变化。

---

## P3 — 玩家击退管线

> **前置**：`plan-knockback-physics-v1`（finished）已实装 `qi_physics::knockback`、`BodyMass`、`KnockbackEvent`、`KnockbackSyncV1`、客户端 `KnockbackSyncHandler` + `HIT_PUSHBACK` 视觉效果。当前 `PendingKnockback` 对无 `MovementController` 的玩家实体被 `remove` 丢弃（`npc/movement.rs:441-443`）。

### 新增：`movement/player_knockback.rs`

```rust
// server/src/movement/player_knockback.rs

/// 玩家击退持续状态——附加到玩家 Entity 上，每 tick 更新直到过期
#[derive(Component)]
pub struct ActivePlayerKnockback {
    pub velocity: DVec3,           // 初始速度（qi_physics::knockback 输出）
    pub remaining_ticks: u32,      // 剩余 tick
    pub recovery_ticks: u32,       // 恢复窗口 tick（结束后 5 tick 移速 ×0.5）
    pub source_entity: Option<Entity>,
}

fn apply_player_knockback_system(
    mut commands: Commands,
    mut players: Query<(Entity, &mut Position, &PendingKnockback, &Client), Without<MovementController>>,
    mut active: Query<(Entity, &mut ActivePlayerKnockback, &mut Position, &Client)>,
    ...
) {
    // 1. PendingKnockback → ActivePlayerKnockback + Valence set_velocity() 初始脉冲
    // 2. 每 tick 更新 velocity（地面摩擦衰减 0.85 / 空中 0.95）
    // 3. remaining_ticks 归零 → 进入 recovery（5 tick，移速 ×0.5）
    // 4. recovery 结束 → remove ActivePlayerKnockback
}
```

### 击退中行为限制

- 击退中（`ActivePlayerKnockback` 存在）：禁 Dash / 禁攻击 / 允许格挡
- 恢复窗口（`recovery_ticks > 0`）：移速 ×0.5 / 允许 Dash / 允许攻击

### 视听

P3 不新增视听资产——复用已有系统：
- **击退位移**：Valence `set_velocity()` 驱动客户端原生平滑移动
- **击退视觉**：`plan-knockback-physics-v1` 已实装 `KnockbackSyncV1` → client `KnockbackSyncHandler` → `HIT_PUSHBACK` 视觉效果（屏幕边缘红色 vignette + 轻微 shake）
- **恢复窗口**：复用 LegStrain 类的减速体感（移速 ×0.5），无需额外视觉提示

---

## P4 — Dash Proficiency

### TechniqueDefinition 注册

```rust
// server/src/movement/dash_proficiency.rs
TechniqueDefinition {
    id: "movement.dash",
    display_name: "闪避",
    grade: "common",
    required_realm: 0,  // 出生即有
    stamina_cost: 15.0,
    cooldown_ticks: 40,
}
```

注册到 `KnownTechniques`，复用 `plan-sword-basics-v1` 的 proficiency 框架。

### 经脉依赖声明

```rust
SkillMeridianDependencies::declare("movement.dash", vec![
    MeridianId::LegL,
    MeridianId::LegR,
]);
```

双腿经脉任一 SEVERED → Dash 被 `check_meridian_dependencies` 拦截。

### Proficiency 缩放

| 属性 | prof 0 | prof 50 | prof 100 |
|------|--------|---------|----------|
| stamina_cost | 15 | 12 | 9 |
| cooldown_ticks | 40 | 30 | 20 |
| distance | 2.8m | 3.2m | 3.8m |

```rust
pub fn dash_stamina_cost(prof: f64) -> f64 {
    15.0 - (prof / 100.0) * 6.0  // 15 → 9
}
pub fn dash_cooldown_ticks(prof: f64) -> u32 {
    (40.0 - (prof / 100.0) * 20.0) as u32  // 40 → 20
}
pub fn dash_distance(prof: f64) -> f64 {
    2.8 + (prof / 100.0) * 1.0  // 2.8 → 3.8
}
```

### Proficiency 获取

- 每次使用 Dash：+0.5（prof < 50 时），+0.25（prof ≥ 50 时 diminishing）
- 战斗中使用（周围 16 格内有敌对实体）：额外 +0.3
- i-frame 躲避成功（Dash 期间规避了命中判定）：额外 +0.5

### 视听

P4 不新增视听资产——**复用已有 Dash 完整管线**：
- **动画**：`bong:dash_forward`（`client/src/main/resources/assets/bong/player_animation/dash_forward.json`，endTick 4，torso pitch 0.436 rad INOUTSINE，双臂后摆 pitch 0.65）
- **VFX**：`bong:movement_dash` → `MovementVfxPlayer.Kind.DASH`（`flyingSwordTrailSprites` 线条 + `cloudDustSprites` 尘土，count 10，maxAge 10，color `#DDE6EE`）
- **音效**：`movement_dash` audio recipe（已注册，`server/src/movement/mod.rs:654`）

Proficiency 缩放仅影响 stamina_cost / cooldown / distance 数值，视听不变。

---

## P5 — 饱和测试

### P0 回归（1-5）
1. server 拒绝 `MovementActionRequestV1::Slide`（返回 `ActionRejected`）
2. server 拒绝 `MovementActionRequestV1::DoubleJump`
3. Dash 不受影响（仍正常触发 + VFX + 音效 + 动画）
4. `MovementStateV1` 序列化不含 `slide_cooldown_remaining_ticks` / `double_jump_charges_*`
5. client `MovementHudPlanner` 无 Slide arc / DoubleJump dots

### P1 腿伤→移速（6-13）
6. 双腿 INTACT → speed factor = 1.0
7. 单腿 LACERATION → factor = 0.7（worldview §四:254）
8. 单腿 FRACTURE → factor = 0.4（worldview §四:255）
9. 单腿 SEVERED → factor = 0.0 + Dash 禁止（worldview §四:256）
10. BRUISE → factor = 1.0（不影响）
11. 治愈（Wound 消除）→ factor 恢复到 1.0
12. 混合伤（左 LACERATION + 右 FRACTURE）→ factor = min(0.7, 0.4) = 0.4
13. 双腿取最差：左 INTACT + 右 SEVERED → factor = 0.0

### P2 护甲→移速（14-19）
14. 无甲 → armor_weight_factor = 1.0
15. 布甲（weight ~3.0）→ factor = 1.0
16. 铁甲（weight ~10.0）→ factor ≈ 0.925
17. 极限堆甲 → factor 下限 = 0.65
18. 穿脱即变（equip/unequip 触发重算）
19. 叠加腿伤：LACERATION 0.7 × 铁甲 0.925 → 综合 ≈ 0.648

### P3 玩家击退（20-24）
20. NPC 命中玩家 → 玩家获得 Valence velocity 位移
21. 击退中禁 Dash（`MovementActionRejected` reason "knockback"）
22. `ActivePlayerKnockback` remaining 归零 → 进入 recovery → 5 tick 后清除
23. 击退方向正确（从攻击者指向玩家）
24. 零力击退（PendingKnockback distance ≈ 0）不触发 ActivePlayerKnockback

### P4 Dash proficiency（25-31）
25. prof 0: cost=15, CD=40, distance=2.8
26. prof 50: cost=12, CD=30, distance=3.2
27. prof 100: cost=9, CD=20, distance=3.8
28. 每次使用 prof +0.5（<50）/ +0.25（≥50 diminishing）
29. 战斗中额外 +0.3
30. FRACTURE 腿 → Dash 距离 ×0.5 + CD ×1.5
31. SEVERED 腿 → Dash 禁止（经脉依赖 check）

### 回归（32-35）
32. 速度公式各因子独立相乘（改一个不影响其他）
33. 纯净状态（无伤/无甲/境界 0）= 重构前 base speed
34. 境界 bonus +5%/级不变
35. zone modifier 不变

## Finish Evidence

- 落地清单：
  - `server/src/movement/{mod.rs,leg_wound.rs,armor_weight.rs,dash_proficiency.rs,player_knockback.rs}`
  - `server/src/combat/{player_attack.rs,status.rs}`
  - `server/src/cultivation/{mod.rs,known_techniques.rs}`
  - `server/src/network/{client_request_handler.rs,gameplay_vfx.rs}`
  - `server/src/schema/{movement.rs,server_data.rs}`
  - `server/assets/audio/recipes/{movement_slide.json,movement_double_jump.json}`
  - `agent/packages/schema/src/movement.ts`
  - `client/src/main/java/com/bong/client/{animation/BongAnimations.java,hud/MovementHudPlanner.java,movement/{MovementKeyRouter.java,MovementKeybindings.java,MovementState.java},network/{ClientRequestProtocol.java,MovementStateHandler.java},visual/particle/{MovementVfxPlayer.java,VfxBootstrap.java}}`
- 关键 commit：
  - `483516884` 2026-05-13 `feat(movement): 重构服务端移动管线`
  - `b63a8df06` 2026-05-13 `feat(schema): 精简移动动作契约`
  - `4b188fb37` 2026-05-13 `feat(client): 移除滑铲和双跳入口`
- 测试结果：
  - `server`: `cargo fmt --check`; `cargo clippy --all-targets -- -D warnings`; `cargo test` (`4636 passed`)
  - `server` 定点：`cargo test movement:: --no-fail-fast` (`29 passed`); `cargo test audio::tests::loads_default_audio_recipes` (`1 passed`)
  - `agent`: `npm run build`; `npm test --workspace @bong/schema` (`384 passed`); `npm test --workspace @bong/tiandao` (`363 passed`); `npm run generate:check --workspace @bong/schema` (`369 files fresh`)
  - `client`: `JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64 PATH=/usr/lib/jvm/java-17-openjdk-amd64/bin:$PATH ./gradlew test build` (`build successful`, `1385` client tests)
- 跨仓库核验：
  - `server`: `MovementAction::Dashing`, `ActivePlayerKnockback`, `leg_wound_to_speed`, `armor_weight_to_speed`
  - `client`: `MovementKeyRouter`, `MovementHudPlanner`, `MovementVfxPlayer`, `MovementStateHandler`
  - `agent/schema`: `movement.dash`, `MovementStateV1`, `MovementActionRequestV1`
- 遗留 / 后续：
  - 无
