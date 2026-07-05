# plan-bughunt-quickslot-breakthrough-pill-noop-v1（骨架）

> **骨架（草案）**。一句话主题：`BreakthroughBonus` 丹药在 **`take_pill` 路径已实装、在 quick-slot/cast 路径却被硬编码成 no-op**，导致玩家把 `固元丹` / `开脉丹` 绑进快捷栏后会**正常起手、正常扣药、正常进冷却，但完全拿不到突破加成**；同一物品两条入口行为分叉，属于 combat runtime 共享施法侧路的真实断链。

> 去重说明：本题**不属于**你排除的 `combat_event juice bridge`、`war emergent reputation gap`、`movement dash HUD`、`combat-ui` 旧题；也**不重复**既有 bughunt skeleton。它聚焦的是 **quick-slot cast runtime 与 alchemy take_pill runtime 的分叉**，根因位于 server 共享施效路径。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 🔴 quick-slot 使用突破丹时药效被吞（扣药/冷却成立，`BreakthroughBoost` 缺失） | fix_pr | ⬜ |

## P0 — 🔴 quick-slot 使用突破丹时药效被吞

- **复现路径**
  - 1. 新角色默认背包就带 `guyuan_pill`（[server/assets/inventory/loadouts/default.toml](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-bz/server/assets/inventory/loadouts/default.toml:54)），其 effect 是 `breakthrough_bonus`（[server/assets/items/pills.toml](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-bz/server/assets/items/pills.toml:1)）。
  - 2. 把 `固元丹` 或 `开脉丹` 绑定到 quick slot；`handle_use_quick_slot` 对绑定物品**不看 category、不看 effect kind**，只要 slot 里绑着实例就会统一走 `Casting` 路径（[server/src/network/client_request_handler.rs](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-bz/server/src/network/client_request_handler.rs:9243)）。
  - 3. 等 cast 自然完成后，quick-slot 路径会正常扣 1 颗药并推完成态；但状态栏不会出现 `破境助力`，随后发起突破时 `material_bonus` 仍为 0。
  - 4. 对照组：同一颗药若走 `AlchemyTakePill`，`handle_alchemy_take_pill` 会明确发 `ApplyStatusEffectIntent { kind: BreakthroughBoost }`（[server/src/network/client_request_handler.rs](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-bz/server/src/network/client_request_handler.rs:12336)），突破系统也会把该 buff 计入成功率（[server/src/cultivation/breakthrough.rs](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-bz/server/src/cultivation/breakthrough.rs:677)）。

- **根因链路**
  - `pills.toml` 已把 `guyuan_pill` / `kaimai_dan` 注册为 `effect = { kind = "breakthrough_bonus", ... }`（[pills.toml](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-bz/server/assets/items/pills.toml:11)）。
  - quick-slot 入口 `handle_use_quick_slot` 会为任意绑定物品插入 `Casting { source: QuickSlot, bound_instance_id: Some(instance_id) }`，没有把 pill redirect 到 `take_pill` 专用分支（[client_request_handler.rs](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-bz/server/src/network/client_request_handler.rs:9318)）。
  - cast 完成后 `cast_emit::apply_item_effect` 对 `ItemEffect::BreakthroughBonus` 不是挂 buff，而是直接写死 `no-op (buff state TODO)`（[server/src/network/cast_emit.rs](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-bz/server/src/network/cast_emit.rs:565)）。
  - 与之相对，`take_pill` 路径对同一个 `ItemEffect::BreakthroughBonus` 会发送 `StatusEffectKind::BreakthroughBoost`，持续 `BREAKTHROUGH_BOOST_DURATION_TICKS`（[client_request_handler.rs](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-bz/server/src/network/client_request_handler.rs:12336)）。
  - 突破系统只认 `StatusEffects` 里的 `BreakthroughBoost` 聚合值：`sum_breakthrough_boost` 汇总活跃 buff（[server/src/combat/status.rs](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-bz/server/src/combat/status.rs:92)），`breakthrough_system` 再把它并入 `material_bonus`（[breakthrough.rs](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-bz/server/src/cultivation/breakthrough.rs:677)）。quick-slot 路径既然没挂 buff，下游自然读不到。

- **影响面**
  - 直接命中的物品：`固元丹`（+0.12）与 `开脉丹`（+0.18）两种突破辅助 pill。
  - 直接命中的入口：所有通过 quick slot / cast runtime 消耗这两种丹药的玩法。
  - 不受影响的入口：`AlchemyTakePill` 专用服丹路径。
  - 表现层症状：药会消失、cast/cooldown 会成立，但 `StatusSnapshot` 不会出现 `BreakthroughBoost`，突破成功率也不会提升。

- **这个 bug 对实际游玩体验的影响**
  - 玩家最直观的体感是：**我明明把突破丹吃掉了，UI 也给了施放/冷却反馈，但冲关时收益完全没上身。**
  - 这会把稀有 pill 变成“看起来可快捷使用、实际会白白吞掉”的陷阱，尤其对新角色默认背包自带 `固元丹` 的场景伤害最大。
  - 因为 `take_pill` 与 quick slot 对同一物品给出不同结果，玩家几乎不可能从规则层面自洽判断“这颗药到底该怎么吃才有效”，只会感知成系统不稳定。

- **修复建议**
  - 方案 A：在 `cast_emit::apply_item_effect` 为 `BreakthroughBonus` 补上与 `handle_alchemy_take_pill` 等价的 `ApplyStatusEffectIntent(BreakthroughBoost)`，让 quick-slot 路径与服丹路径收敛到同一 runtime 语义。
  - 方案 B：更严格地把 `category = pill && effect = BreakthroughBonus` 的物品从 quick-slot 入口前置拒绝或重定向到 `take_pill`，避免“同物双语义”。
  - 回归测试至少要补两条：
    - quick-slot 消耗 `guyuan_pill` 后应出现 `BreakthroughBoost` status。
    - 同一 pill 的 quick-slot 与 `take_pill` 两条路径应产出一致的突破加成语义。

## 反方裁决

> 退化说明：当前会话**没有可用 subagent / delegate 工具**，无法再开独立审稿代理。本节按要求做 **两轮主会话自举反方裁决**，显式记录反方论点与驳回理由。

### Round 1

- **反方论点**：突破丹可能本来就设计成“只能 `take_pill`，不能 quick-slot”；quick-slot 吃到 no-op 不算 bug。
- **驳回理由**：
  - `handle_use_quick_slot` 对绑定实例没有 `category == pill` 的拒绝，也没有把 pill 转发到 `take_pill`；代码语义是“任何已绑定实例都能走 cast runtime”。
  - 默认 loadout 直接发 `guyuan_pill`，而 quick-slot 正是共享消耗品入口；若设计上真要禁用，入口应前置拒绝，而不是**允许起手、允许扣药、最后 silently no-op**。
  - 同一 effect 在 `take_pill` 路径已明确定义为 `BreakthroughBoost`，说明产品语义不是“这药本来没效果”，而是“quick-slot 这条 sidepath 没接上”。

### Round 2

- **反方论点**：即使 quick-slot 分支不挂 buff，玩家仍可改走 `take_pill`，属于使用方式问题，不算高优先级 bug。
- **驳回理由**：
  - 这不是“存在替代入口”的 UX 选择题，而是**同一份资源在合法入口 A 正常、入口 B 吞收益**。只要入口 B 没被禁用，它就是功能错误。
  - 问题后果是实打实的资源损失：pill 被消耗、冷却被写入、但 `BreakthroughBoost` 缺席；这是 server runtime 的状态缺失，不是文案歧义。
  - quick-slot 是战斗/移动中的高频操作习惯入口。把突破丹放进快捷栏并不是异常玩法，系统却给出假阳性成功反馈，优先级不能按“有绕路方案”降级。

## 审计来源

- bughunt round：2026-07-05 本轮，聚焦 combat runtime / combat sidepaths，显式避开 `combat_event juice bridge`、`war emergent reputation gap`、`movement dash HUD`、`combat-ui` 旧题。
- 证据类型：**纯代码闭环证据**，包含物品定义、入口分流、quick-slot 完成分支、take_pill 对照分支、突破系统消费端五段链路，足以支撑 real bug 判定。
