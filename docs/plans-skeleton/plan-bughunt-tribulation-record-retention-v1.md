# plan-bughunt-tribulation-record-retention-v1

> **来源 issue**：#1355。
> 一句话主题：天劫焦土记录资源只追加、不消费或裁剪，长期运行会无界占用内存。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | `TribulationScorchRecords` 的生产消费与有界保留策略 | ⬜ |
| P1 | 结算、持久化交接和长期运行容量回归 | ⬜ |

## §0 摘要

`record_tribulation_scorch_system` 在 `server/src/tribulation/scorch_record.rs:40-70` 为每个焦土 zone 的 `TribulationSettled` 调 `records.push`。`TribulationScorchRecords` 在 `:25-38` 只有 `push` 和只读 `records()`；今天主线除测试外没有生产消费者、清空或容量上限。该资源由 `cultivation::register` 初始化并在 `mod.rs:460-470` 接入生产 schedule，长时间渡劫会让 `Vec<ScorchRecord>` 持续增长。

## §1 立项检查记录

- **worldview**：查 `docs/worldview.md` 的渡劫、焦土标记和世界长期运行关键词；保留 `glass_fulgurite` 事件语义，不擅自删除仍待 world persistence 消费的标记。
- **finished_plans**：查 `plan-terrain-tribulation-scorch-v1` 及 terrain/world persistence 归档；确认地理标记模型已有，但本运行时 buffer 没有生产交接/保留策略。
- **active plan**：查 `docs/plan-*.md` 的 `ScorchRecord`、`TribulationSettled`、world persistence 和 terrain marker；没有一份拥有该 Vec 的生命周期。
- **skeleton**：查 fauna/world cache、event outbox 和 terrain cleanup 骨架；它们不消费天劫焦土记录，不能直接复用其容量语义。
- **reminder.md**：查 `docs/plans-skeleton/reminder.md` 的 `TribulationScorchRecords`、`glass_fulgurite`、`record_tribulation_scorch_system`；仓内有该文件，但没有本资源的 retention 登记。

## §2 接入面与跨仓契约

- **Inputs**：`TribulationSettled`、`ZoneRegistry`、`Position`/`CurrentDimension`、`CombatClock` 和 zone active events。
- **Outputs**：焦土记录交给明确的 server persistence/terrain 消费者并从内存队列删除，或按可验证 TTL/容量有界保留；未命中焦土 zone 的事件仍不产生记录。
- **共享类型/事件**：复用 `ScorchRecord`、`TribulationScorchRecords`、`TRIBULATION_SCORCH_EVENT`、`GLASS_FULGURITE_MARKER_ID`；若新增消费事件，沿用 server 内部事件，不改变 client payload。
- **三端契约符号**：server `tribulation::scorch_record::{record_tribulation_scorch_system,ScorchRecord}` 与 `cultivation::register`；client **无变更**，当前记录没有下发 wire schema；agent **无变更**，渡劫焦土记录不经 Redis IPC。
- **worldview/qi**：这是生命周期/持久化问题，不转移真元，不引入 qi ledger 账户；若记录消费伴随地形副作用，应保持现有事件幂等而非用真元事件替代。

## §3 游玩影响与复现

在 `north_waste_east_scorch` 等 zone 连续结算大量渡劫，观察 `TribulationScorchRecords.records().len()` 只增不减；服务器长跑后资源占用随历史渡劫次数增长，即使焦土标记已经被下游处理也不会释放内存。

## §4 `origin/main` 根因证据

- `server/src/tribulation/scorch_record.rs:25-38` 的 resource 没有 drain、ack、TTL 或容量 API。
- `server/src/tribulation/scorch_record.rs:40-70` 每次命中焦土 zone 都无条件 `records.push(record)`。
- `server/src/cultivation/mod.rs:267` 初始化 resource，`:460-470` 把记录系统接入生产 schedule；全仓 `records()` 调用仅在该文件测试，未形成生产消费者。

## §5 非重复比对

`plan-terrain-tribulation-scorch-v1` 是焦土标记的领域计划，不等于当前 Vec 的消费已落地；world/entity cache cleanup 骨架不拥有 `TribulationScorchRecords`。本骨架只补记录交接和 retention，不改地形生成器或 BongWorldGen。

## §6 修复计划骨架

- **P0**：确定一个 server 内部的持久化/地形消费入口，以批量 drain + ack（或显式 bounded queue）消费 `ScorchRecord`；消费成功后再移除，失败保留可重试记录。
- **P1**：为重复结算、非焦土 zone、缺 actor、下游失败、重启/长跑容量增加回归；验收记录数量有上限或成功 ack 后归零，不能只测试 `push`。

## §7 验证计划

实现后只跑 server 栈 fmt、clippy、cargo test，重点 `tribulation::scorch_record`、cultivation schedule 和对应 persistence consumer；agent/client/worldgen 无代码改动，不跑其门禁。
