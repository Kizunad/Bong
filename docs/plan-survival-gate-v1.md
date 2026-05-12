# Bong · plan-survival-gate-v1 · 骨架

**Survival 模式门控 + 原版 HUD 清理**——非 Survival 玩家免疫所有伤害；砍掉原版红心/饥饿/经验/护甲条，Bong 自有 HUD 已完全覆盖。

**世界观锚点**：
- `worldview.md §四:237-258` 16 部位伤口系统——仅 Survival 下生效
- 无 qi_physics 锚点（纯基建 plan）

**交叉引用**：
- `plan-combat-no_ui`（finished）：`Wounds` / `AttackIntent` / `CombatEvent` / `resolve_attack_intents()`
- `plan-knockback-physics-v1`（finished）：`KnockbackEvent` / collision damage
- `plan-death-lifecycle-v1`（finished）：`DeathEvent` / `wound_bleed_tick()`
- `plan-weapon-v1`（finished）：`MixinInGameHud.renderHotbar()` cancel 已有先例

**前置依赖**：
- `GameMode` component ✅（Valence，`server/src/player/mod.rs:32`，默认 `Creative`）
- `/gm` 命令 ✅（`cmd/dev/gm.rs`，`/gm s` 切 Survival）
- `resolve_attack_intents()` ✅（`combat/resolve.rs:212`）
- `wound_bleed_tick()` ✅（`combat/lifecycle.rs:154`）
- `queue_collision_wound()` ✅（`npc/movement.rs:782`）
- `MixinInGameHud` ✅（`client/.../mixin/MixinInGameHud.java`，已 cancel `renderHotbar`）

**反向被依赖**：所有涉及玩家受伤的 plan（受益于 GameMode 门控）

---

## 接入面 Checklist

- **进料**：`GameMode` component（Valence 内置，每个 Client entity 自带）
- **出料**：无新类型——门控逻辑内联在已有系统中
- **共享类型**：新增 `is_damageable(Entity, &Query<&GameMode>) -> bool` 工具函数
- **跨仓库契约**：server: `combat/` 各系统加门控 / client: mixin 追加 cancel
- **worldview 锚点**：§四 伤口系统（仅 Survival 生效）
- **qi_physics 锚点**：无

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | Server GameMode 伤害门控 | ⬜ |
| P1 | Client 砍原版 HUD | ⬜ |
| P2 | 饱和测试 | ⬜ |

---

## P0 — Server GameMode 伤害门控

### 工具函数

```rust
// server/src/combat/mod.rs
pub fn is_damageable(entity: Entity, game_modes: &Query<&GameMode>) -> bool {
    game_modes.get(entity).map_or(true, |gm| *gm == GameMode::Survival)
}
```

NPC 无 `GameMode` component → `map_or(true)` → 始终可受伤。玩家只在 Survival 受伤。

### 门控点

| # | 文件 | 函数 | 行为 |
|---|------|------|------|
| 1 | `combat/resolve.rs:212` | `resolve_attack_intents()` | 加 `Query<&GameMode>` 参数；target 非 Survival → skip 整个 intent（无伤害、无击退、无 CombatEvent） |
| 2 | `combat/lifecycle.rs:154` | `wound_bleed_tick()` | 加 `Query<&GameMode>` 参数；非 Survival entity → skip 流血扣血 |
| 3 | `npc/movement.rs:782` | `queue_collision_wound()` | 内部查 `GameMode`；非 Survival target → 不创建伤口 |
| 4 | `combat/carrier.rs:810` | 投掷物命中扣血处 | 加 GameMode 检查 → 非 Survival 免伤 |
| 5 | `combat/zhenmai_v2.rs:874` | 截脉 backfire 自伤 | 加 GameMode 检查 → Creative 测招无自伤 |

### 不门控

- `combat/debug.rs:79`（`/health set` dev 命令）— 调试用，应无视 GameMode
- `alchemy/pill.rs:432`（服丹回血）— 正面效果
- `combat/lifecycle.rs:1335`（复活回血）— 正面效果

---

## P1 — Client 砍原版 HUD

修改 `client/src/main/java/com/bong/client/mixin/MixinInGameHud.java`，追加 2 个 `@Inject` cancel：

```java
// 砍红心 + 饥饿条 + 护甲条 + 氧气条
@Inject(method = "renderStatusBars", at = @At("HEAD"), cancellable = true)
private void bong$hideStatusBars(DrawContext context, CallbackInfo ci) {
    ci.cancel();
}

// 砍经验条（含等级数字）
@Inject(method = "renderExperienceBar", at = @At("HEAD"), cancellable = true)
private void bong$hideExperienceBar(DrawContext context, int x, CallbackInfo ci) {
    ci.cancel();
}
```

格式与已有 `bong$replaceHotbar` 一致。MC 1.20.1 yarn 方法签名：
- `renderStatusBars(DrawContext)` — 绘制红心 + 饥饿 + 护甲 + 氧气泡
- `renderExperienceBar(DrawContext, int)` — 绘制经验条 + 等级数字

---

## P2 — 饱和测试

### Server 门控（1-13）
1. `is_damageable` 无 GameMode（NPC）→ true
2. `is_damageable` Survival → true
3. `is_damageable` Creative → false
4. `is_damageable` Adventure → false
5. `is_damageable` Spectator → false
6. `resolve_attack_intents` 目标 Creative 玩家 → Wounds 不变 + 无 CombatEvent + 无 KnockbackEvent
7. `resolve_attack_intents` 目标 Survival 玩家 → 正常扣血 + 事件发出
8. `wound_bleed_tick` Creative 玩家有伤口 → health 不降
9. `wound_bleed_tick` Survival 玩家有伤口 → health 降
10. `queue_collision_wound` Creative target → 不创建伤口
11. Creative 玩家被攻击 → 无 KnockbackEvent
12. `/gm s` 后被攻击 → 正常受伤（模式切换即时生效）
13. `/gm c` 后残留伤口 → 流血不再扣血

### Client HUD（14-15）
14. `MixinInGameHud` 注册 3 个 cancel（hotbar + statusBars + experienceBar）
15. 进游戏确认无红心 / 饥饿条 / 经验条 / 护甲图标 / Bong 自有 HUD 正常
