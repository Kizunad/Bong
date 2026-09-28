# plan-bughunt-hybrid-fusion-drained-qi-v1

> 骨架：来源 #1450。融合 despawn 组件兽时未消费 RatBlackboard 的 drained_qi。

## §0 摘要

`server/src/fauna/hybrid_beast.rs:243-256,321-328` 只从 `Cultivation.qi_current` 读取并随后 despawn；RatBlackboard 中已经记录的 `drained_qi` 没有合并到释放总额，导致被抽取真元成为孤儿余额。

## §1 游玩影响

融合失败或组件兽死亡时，先前吸取的真元不回到 zone，玩家观察到灵气总量下降且审计无法解释。

## §2 复现路径

1. 让 hybrid beast 吸取真元，确认 `RatBlackboard.drained_qi > 0`。
2. 触发 fusion despawn/失败路径。
3. 对比 despawn 前后 zone、ledger 与事件，发现仅释放 cultivation 当前值。

## §3 今天 `origin/main` 根因证据

- `hybrid_beast.rs:243-256` 构造融合/死亡结算时只读取 `Cultivation.qi_current`。
- `:321-328` despawn 组件兽，未把 RatBlackboard `drained_qi` 转入释放函数。
- `drained_qi` 没有后续消费者或清零审计，形成不可追踪余额。

## §4 非重复比对

`plan-bughunt-tsy-death-extra-hand-protection-v1` 处理死亡掉落槽位；`plan-bughunt-qi-ledger-asymmetry-v1` 处理通用 overflow。这里的独立根因是 hybrid blackboard 与 despawn 释放合并遗漏。

## §5 立项检查记录

本记录已按 `docs/worldview.md`、`docs/finished_plans/`、`docs/plan-*.md`、`docs/plans-skeleton/` 与 `docs/plans-skeleton/reminder.md` 逐项检索。

- **worldview**：查“异兽、融合、吸灵、灵气回流”；`worldview.md §七` 要求异兽状态不创造/吞噬世界灵气。
- **finished_plans**：查 `HybridBeast`、`RatBlackboard`、`drained_qi`、release helpers；未见融合终结清账。
- **active plan**：查 hybrid/fusion despawn；无同一释放修复。
- **skeleton**：查 `drained_qi`、`hybrid beast`、`fusion`；无重复骨架。
- **reminder.md**：查 `hybrid`、`drained_qi`；仓内无对应条目。

## §6 接入面与跨仓契约

- **Inputs**：`RatBlackboard.drained_qi`、`Cultivation.qi_current`、所在 zone、despawn outcome。
- **Outputs**：一次性释放到 zone/overflow 的 ledger transfer、清零 blackboard、despawn 事件。
- **共享类型或事件**：复用 `QiTransfer { from, to, amount, reason }`、`QiTransferReason::ReleaseToZone`、`Cultivation::release_to_zone`/`transfer_external_qi_to_ledger`；用 `qi_physics::ledger::assert_conservation`，测试引用 `SPIRIT_QI_TOTAL`。
- **server 符号**：`fauna::hybrid_beast`、`RatBlackboard`、`qi_physics::ledger::{transfer_external_qi_to_ledger,assert_conservation}`。
- **agent**：无变更；异兽 blackboard 不在 agent schema。
- **client**：无变更；despawn/VFX 已有事件，不改 payload。
- **worldview 锚点**：`docs/worldview.md §七` 异兽与灵气生态。

## 阶段总览

| 阶段 | 状态 | 交付物 |
| --- | --- | --- |
| P0 | ⬜ | 合并 cultivation 与 blackboard drained qi，使用真实 ledger release/overflow |
| P1 | ⬜ | 融合成功、失败、重复 despawn 与守恒断言测试 |

## 来源 issue

- #1450 `[flash-review][major] 融合 despawn 鼠时未归还 drained_qi`
