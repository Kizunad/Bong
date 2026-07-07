# BugHunt: 垂死大能负压流失未进入 rift 账本

## 0. 结论

`server/src/fauna/dying_elder.rs` 的 `dying_elder_drain_system` 在 `Plea` / `Recovering` 状态下每 tick 从 `DyingElderBlackboard.qi_current` 扣除负压流失真元，但只调用 `WorldQiAccount::push_transfer_audit(QiTransfer { reason: RiftCollapse, ... })` 和发送 `QiTransfer` 事件，没有把 `actual_drain` 写入 `rift:<home_zone>`、zone 或 overflow 的真实余额。

这会让垂死大能等待玩家喂丹/拖延期间持续损失的真元从守恒快照中消失。世界观要求全服灵气总量零和（worldview.md §二 L18-L20；AGENTS.md §9），`docs/CLAUDE.md` 也明确警告“只 emit `QiTransfer` 事件却无 system 消费应用到 `WorldQiAccount`”是吞真元红旗。

## 1. 证据链

- `dying_elder_drain_system` 用 `compute_drain_per_tick` 计算 drain，并执行 `bb.qi_current = (bb.qi_current - drain).max(0.0)`；随后构造 `QiTransferReason::RiftCollapse`，但只 `account.push_transfer_audit(transfer.clone())` 与 `qi_transfer_events.send(transfer)`，没有 `set_balance` / `transfer` / 真实 rift 余额增加。
  - `server/src/fauna/dying_elder.rs:831`
  - `server/src/fauna/dying_elder.rs:884`
  - `server/src/fauna/dying_elder.rs:891`
  - `server/src/fauna/dying_elder.rs:893`
- `WorldQiAccount::push_transfer_audit` 的注释和实现明确是 audit-only：“仅将 transfer 追加到审计轨迹，不修改任何账户余额”。
  - `server/src/qi_physics/ledger.rs:463`
- `summarize_world_qi` 只汇总 `ZoneRegistry.zone.spirit_qi`、所有 `Cultivation.qi_current`、背包 qi、`WorldQiAccount::total()`；不查询 `DyingElderBlackboard.qi_current`。
  - `server/src/qi_physics/ledger.rs:641`
- 同类 TSY 负压 drain 的正确模式会先把 rift balance 增加，再 `push_transfer_audit`，说明 `RiftCollapse` 不能只靠事件或审计记录补账。
  - `server/src/world/tsy_drain.rs:90`

## 2. 非重复说明

不是 #1020「垂死大能死亡释放 overflow 蒸发」。#1020 关注 `dying_elder_death_system` 在死亡结算时 `qi_release_to_zone` 的 overflow 分支；本 bug 发生在死亡前的 `Plea` / `Recovering` 每 tick 负压流失路径，目标账户应是 `rift:<home_zone>`。

也不是 #1050 craft zone、#1056 NPC 回气、#1076 灵田 plot_qi、#1082 灵蝗潮推进扣 qi，路径和触发体验均不同。

## 3. 实际游玩体验影响

玩家遇到垂死大能后，拖延、围观、反复喂丹或等待其自然力竭时，会看到大能气息逐渐衰弱，最终可能死亡并掉落传承。但这段时间被坍缩渊抽走的真元没有进入 rift/zone/overflow 真实账本，天道“回收真元”的生态收益少记，长服会出现遭遇越多、守恒 telemetry 越漂的隐性经济偏差。

如果玩家用回元丹延长大能存活时间，问题更明显：给丹把大能黑板真元抬高，后续负压 tick 又把这部分真元 audit-only 抽走，玩家投入的丹药真元在死亡前可能持续蒸发，影响“拖延看他被负压耗死”和“喂丹赌传承/夺舍”的风险收益账。

## 4. 修复要求

1. 在 `dying_elder_drain_system` 中对 `actual_drain` 走真实余额落账，推荐对齐 `server/src/world/tsy_drain.rs` 的 `record_tsy_drain_transfer` 模式：`rift:<home_zone>` balance 增加 `actual_drain`，再 `push_transfer_audit`。
2. 明确 `DyingElderBlackboard.qi_current` 与 ECS `Cultivation.qi_current` 的权威关系。若继续使用 blackboard 作为权威池，所有进出 blackboard 的 `TradeDan`、`SoulSeize`、`RiftCollapse` 必须在守恒快照中有对应余额变化或专门账户。
3. 补饱和化测试：
   - Plea / Recovering tick：`bb.qi_current` 减少 N，`WorldQiAccount::balance(rift:<home_zone>)` 增加 N。
   - `push_transfer_audit` 仍只留痕，不作为余额变化替代。
   - `summarize_world_qi` 前后 `bb.qi_current` 损失必须由 `ledger_qi` 增量抵消。
   - 与死亡释放路径分开测试，避免只覆盖 #1020 的 `dying_elder_death_system`。

## 5. 对抗复核

- 第 1 轮 subagent 提出 heartbeat 伪灵脉候选；因与 #1000 同主题，按去重规则丢弃。
- 第 2 轮 subagent 专门反驳本候选，结论为“成立”：`bb.qi_current` 不进 `summarize_world_qi`，`push_transfer_audit` 不改余额，未发现消费者补 rift balance，且与 #1020 死亡释放 overflow 非重复。
