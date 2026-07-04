# plan-shield-block-combat-event-feedback-v1（骨架）

> **骨架（草案）**。一句话主题：盾格挡命中时，server 已把 `combat_event.kind` 发成 `shield_block`，但 client `CombatEventHandler` 仍把它落到默认 `HIT` 分支，导致玩家正常举盾格挡后的**数值飘字反馈按普通受击红字显示**，与专用盾格挡反馈链脱节。

> 去重说明：**不重复** `plan-bughunt-r10-findings-v1` 的“破盾后 `ShieldBlock` / `ShieldBlocking` 残留”题。那一题是 **server ECS 状态泄漏**；本题是 **server→client `combat_event` kind 分类漏接**，即使盾未破也可稳定复现。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | `shield_block` 飘字分类断链 | fix_pr | ⬜ |

## P0 — `shield_block` 飘字分类断链

- **#1 major（fix_pr）**：`server/src/network/combat_event_emit.rs:48-53` 明确把 `DefenseKind::ShieldBlock` 编成 `kind="shield_block"`；`server/src/combat/resolve.rs:8945-9060` 的前方举盾 happy-path 测试又证明这条分支在**正常游玩可达**，且格挡后 `physical_damage` 仍可保留残值（不是“永远 0 伤所以看不到飘字”的死路）。
- 但 `client/src/main/java/com/bong/client/combat/handler/CombatEventHandler.java:88-109` 的 `parseKind()` / `defaultColorFor()` **只识别** `crit/block/heal/qi_damage`；`shield_block` 落入默认 `HIT`，因此最终写入 `DamageFloaterStore.Kind.HIT`，颜色也走普通受击红。
- 同一个 handler 的 `toJuiceEvent()`（同文件 `157-175`）却**显式**把 `shield_block` 映射到 `CombatJuiceEvent.Kind.SHIELD_BLOCK`。这说明 client 端契约事实上已经承认 `shield_block` 是独立 kind，但数值飘字这半条链漏了同步。
- `client/src/main/java/com/bong/client/network/ShieldBlockHitHandler.java:19-109` 的专用 `shield_block_hit` payload 只补**粒子 / 音效 / toast / HUD 瞬态盾弧**，不携带伤害数值，不能替代 `combat_event` 的“本次被挡后实际掉了多少”的数值反馈。
- **实际影响**：玩家正常举盾、正面吃到一击并成功触发盾格挡时，屏幕上仍会出现与普通挨打几乎同构的红色伤害飘字；玩家无法从数值飘字层分辨“这次是盾格挡后的残余伤害”还是“完全没挡到的普通受击”，主战斗反馈被误导。

## 修法草案

- 最小修：`CombatEventHandler.parseKind()` / `defaultColorFor()` 把 `shield_block` 归到格挡语义（可先复用 `BLOCK` 的灰色飘字与分类）。
- 若希望 A/V 进一步差异化：给 `DamageFloaterStore.Kind` 增 `SHIELD_BLOCK`，由 `DamageFloaterHudPlanner` 单独决定前缀 / 颜色，而不是复用 `BLOCK`。
- 无论选哪条修法，都应补 pin 测试：`kind="shield_block"` 不能再落默认 `HIT`。

## 开放问题

1. `shield_block` 飘字是否直接复用现有 `BLOCK` 视觉语义，还是要在 `DamageFloaterStore.Kind` 新增独立 `SHIELD_BLOCK`？
2. 是否要顺手补一条“server `wire_kind("shield_block")` → client `CombatEventHandler` 不得 default HIT”的跨端对拍测试，防以后再漏新 `kind`？

## 审计来源

bughunt loop 20260704-i。两轮怀疑式证伪后保留：

1. 反证一：这不是“已有 `shield_block_hit` 专用通道，所以 `combat_event` 怎么分都无所谓”——专用通道只补视听，不补数值飘字。
2. 反证二：这不是不可达死码——`resolve.rs` happy-path 测试已锁住正常前方举盾会真实产出 `DefenseKind::ShieldBlock`，且仍可能存在残余 `physical_damage`，因此玩家常规战斗可见。
