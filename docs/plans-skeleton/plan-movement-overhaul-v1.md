# Bong · plan-movement-overhaul-v1 · 骨架

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
- `plan-knockback-physics-v1`（skeleton）：依赖本 plan 的玩家击退管线
- `plan-sword-basics-v1`（active）：Dash proficiency 同框架

**前置依赖**：
- `MovementState` ✅（`server/src/movement/mod.rs:127`）
- `MovementController` / `Navigator` ✅（`npc/movement.rs` + `navigator.rs`）
- `Wounds` / `BodyPart::LegL|LegR` ✅（`combat/components.rs`）
- `StatusEffectKind::LegStrain` ✅（`status.rs:166`——存在但从未被创建）
- `MundaneArmor` ✅（`armor/mundane.rs`）
- `DerivedAttrs.move_speed_multiplier` ✅（`components.rs:270`）
- `KnownTechnique.proficiency` ✅

**反向被依赖**：`plan-knockback-physics-v1` / `plan-sword-basics-v1` / 所有涉及移动限制的未来 plan

---

## 接入面 Checklist

- **进料**：`Wounds`（腿伤）/ `MundaneArmor`（重量）/ `Stamina` / `Cultivation.realm` / `KnownTechnique`（Dash prof）/ `PendingKnockback`
- **出料**：`MovementState`（更新）/ `MovementStateV1`（精简 schema）/ `LegStrain` StatusEffect / client HUD 更新
- **共享类型**：改动 `MovementAction` enum（删 Sliding/DoubleJumping）/ 改动 `MovementStateV1`（删 slide/jump 字段）/ 新增 `leg_wound_to_speed()` / 新增 `armor_weight_to_speed()` / 改动 `PendingKnockback` 对玩家生效
- **跨仓库契约**：server: `movement/` 重构 + `leg_wound.rs` + `armor_weight.rs` + `player_knockback.rs` / client: 删 B 键 + Space 双重跳 + HUD 精简 / agent: 无
- **worldview 锚点**：§四:254-256 腿伤移速 + §四:332 拼刺刀 + §三:133 境界移速
- **qi_physics 锚点**：无

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | 砍 Slide/DoubleJump（server + client + schema） | ⬜ |
| P1 | 腿伤→移速闭环（Wound → LegStrain → speed） | ⬜ |
| P2 | 护甲重量→移速 | ⬜ |
| P3 | 玩家击退管线（PendingKnockback → Valence velocity） | ⬜ |
| P4 | Dash proficiency | ⬜ |
| P5 | 饱和测试 | ⬜ |

---

## P0 — 砍 Slide / DoubleJump

### Server（`movement/mod.rs`）

删除：`MovementAction::Sliding` / `DoubleJumping` 变体 + 所有 Slide/DoubleJump 常数/逻辑/hitbox 缩小/接触伤害/空中连跳/转向限制。

保留：`MovementAction::Dashing` + 全部 Dash 逻辑 + 基础速度公式。

### Schema

`MovementStateV1` 删除 `slide_cooldown_remaining_ticks` / `double_jump_charges_*` / `hitbox_height_blocks`。

### Client

删除 B 键 Slide / Space 双重跳 / Slide cooldown arc / DoubleJump charge dots。保留 V 键 Dash + cooldown arc + Stamina bar。

---

## P1 — 腿伤→移速闭环

### worldview 正典映射

```rust
pub fn leg_wound_to_speed(worst_wound: WoundGrade) -> f64 {
    match worst_wound {
        Intact | Bruise  => 1.0,   // 正常
        Abrasion         => 0.9,   // 轻微跛
        Laceration       => 0.7,   // 跛（正典）
        Fracture         => 0.4,   // 残（正典）
        Severed          => 0.0,   // 动不了（正典）
    }
}

// 双腿综合 = min(左腿最严重, 右腿最严重)
```

### LegStrain 公式扩展

现有 `LegStrain` 公式 clamp 上限从 0.6 → 1.0（允许 SEVERED = 100% 减速）。`magnitude = (1.0 - leg_factor) / 0.15`。

### Dash 与腿伤

- SEVERED (factor=0.0) → Dash 禁止
- FRACTURE (factor≤0.4) → Dash 距离 ×0.5 + CD ×1.5
- LACERATION (factor≤0.7) → Dash 距离 ×0.8

---

## P2 — 护甲重量→移速

```rust
pub fn armor_weight_to_speed(total_weight: f64) -> f64 {
    if total_weight < 5.0 { 1.0 }
    else if total_weight <= 15.0 { 1.0 - (total_weight - 5.0) * 0.015 }
    else { (0.85 - (total_weight - 15.0) * 0.01).max(0.65) }
}
```

速度公式整合：`speed = BASE × realm × zone × stamina × leg_wound × armor_weight`

---

## P3 — 玩家击退管线

当前 `PendingKnockback` 对无 `MovementController` 的玩家无效。新增：

```rust
// movement/player_knockback.rs
fn apply_player_knockback_system(...) {
    // PendingKnockback → Valence set_velocity() 速度脉冲
    // + ActivePlayerKnockback 持续状态（碰撞检测用）
    // 击退中禁 Dash/攻击，允许格挡
    // 结束后 5 tick 恢复窗口（移速 ×0.5）
}
```

新增 `KnockbackSyncV1` payload 下发 client。

---

## P4 — Dash Proficiency

```rust
TechniqueDefinition {
    id: "movement.dash",
    display_name: "闪避",
    grade: "common",
    required_realm: 0,  // 出生即有
    stamina_cost: 15.0,
    cooldown_ticks: 40,
}
```

| 属性 | prof 0 | prof 50 | prof 100 |
|------|--------|---------|----------|
| stamina_cost | 15 | 12 | 9 |
| cooldown_ticks | 40 | 30 | 20 |
| distance | 2.8m | 3.2m | 3.8m |

获取：每次使用 +0.5（prof<50），战斗中额外 +0.3，i-frame 躲避额外 +0.5。

### 视听

动画 `bong:movement_dash`（endTick 4，身体前倾+双臂摆动）。粒子：起跳点灰色地面印 + 身后浅灰尘土。音效：风声 + 脚踏地声。

---

## P5 — 饱和测试

1-5. P0：slide/jump 拒绝 + Dash 不受影响 + schema 无旧字段 + HUD 精简
6-13. P1：双腿 INTACT=1.0 / LACERATION=0.7 / FRACTURE=0.4 / SEVERED=0.0+Dash 禁止 / BRUISE=1.0 / 治愈恢复 / 混合伤 / 双腿取最差
14-19. P2：无甲=1.0 / 布甲=1.0 / 板甲≈0.895 / 极限=0.65 / 穿脱即变 / 叠加腿伤
20-24. P3：NPC 命中→玩家飞 / 击退中禁 Dash / 结束清除 / 方向正确 / 零力不触发
25-31. P4：prof 缩放 / 每次 +0.5 / 战斗额外 / 残腿 Dash ×0.5 / 断腿禁 Dash
32-35. 回归：速度公式各因子独立 / 纯净状态=重构前 / 境界 bonus 不变 / zone mod 不变
