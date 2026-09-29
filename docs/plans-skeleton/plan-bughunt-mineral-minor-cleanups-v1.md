# plan-bughunt-mineral-minor-cleanups-v1

> **来源 issue**：#1591、#1503、#1468、#1458。矿物掉落、耗尽日志和劫气坐标的四个独立 minor 共享 mineral/gathering 入口，但分别保持物品身份、关服持久化、采集完成门和玩家位置语义。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 掉落合并、采集完成、karma 坐标和日志 flush 的权威边界 | ⬜ |
| P1 | freshness/qi、重复 Stop、重启和劫气落点回归 | ⬜ |

## §0 摘要

今天主线仍有四条可达缺口：`find_mergeable_stack` 只按 template/数量合并而丢 freshness 与 initial qi；`ExhaustedMineralsLog` 只有每 600 tick 刷盘、没有 `AppExit` hook；`SurvivalDrop` 先发 `MineralDropEvent` 再检查 `GatheringSession` 完成度；`mark_player` 把矿块坐标当成玩家 last position。修复只复用现有 inventory/gathering/persistence/karma API。

## §1 立项检查记录

- **worldview**：查 `docs/worldview.md` §六/§八/§十的矿物、骨币与真元携带语义；不新增货币或矿物物理规则。
- **finished_plans**：查 `plan-mineral-v1`、shelflife、persistence slices、gathering tool/session 计划；没有同时覆盖四个入口的实现。
- **active plan**：查矿脉 respawn、persistence flush、gathering session 和 inventory filter active plans，确认各项责任边界。
- **skeleton**：查 `plan-bughunt-mineral-respawn-block-writeback-v1`、`plan-gathering-mineral-origin-position-break-v1` 与 freshness 计划；没有重复的掉落合并/Stop 前置门组合。
- **reminder.md**：查 mineral、freshness、shutdown、gathering、karma；仓内有该文件，但没有四条 issue 的登记。

## §2 接入面与跨仓契约

- **Inputs**：`MineralDropEvent`、`ItemInstance::freshness`/`initial_qi`、`GatheringSessionStore`、`DiggingEvent`、`AppExit`、`KarmaFlagIntent`、玩家 `Position`/`CurrentDimension`。
- **Outputs**：只合并 identity 相容的矿物堆、关服前 durable exhausted log、完成会话后一次掉落、劫气 `last_position` 为玩家位置。
- **共享类型/事件**：复用 `stack_identity_matches`/`Freshness`、`GatheringSession`、`ExhaustedMineralsLog::flush`、`KarmaFlagIntent`；不新增矿物掉落事件或另一套采集计时。
- **三端契约符号**：server `mineral::{inventory_grant,break_handler,persistence,bridge}`、`gathering::GatheringSessionStore`；agent 继续消费既有 `GameEvent::MineralKarmaFlag`，**无 schema 变更**；client **无变更**，仍接收既有 inventory/mining progress payload。
- **worldview/qi**：物品 `Freshness.initial_qi` 属可核算 item qi；涉及 qi 的合并测试用 `SPIRIT_QI_TOTAL`、`qi_physics::ledger::assert_conservation`，不凭空合并不同 freshness 的真元。其余三条不新增 `QiTransfer`。

## §3 游玩影响与复现

1. 背包已有陈旧灵石堆，再挖一块高 freshness 灵石，当前路径把两者数量相加并丢新物品 qi/寿命。
2. 在 600 tick flush 窗口内关服，重启后永久耗尽矿点不在 exhausted 集合中；或伪造/重复发送同一帧 Stop，先收到掉落再进入缺失/未完成 session 分支。
3. 在矿块处触发劫气后立即离开，天罚按矿块位置而不是玩家当前位置落下。

## §4 `origin/main` 根因证据

- `server/src/mineral/inventory_grant.rs:67-80` 用 `find_mergeable_stack` 后只 `stack_count += DEFAULT_DROP_STACK_COUNT`；该 helper (`inventory/mod.rs:2259` 附近) 不比较 freshness/initial qi（#1591）。
- `server/src/mineral/persistence.rs:203-231` 只在 `flush_clock >= flush_interval_ticks` 时刷盘；`ExhaustedMineralsLog::flush` 虽存在于 `:154-188`，没有矿物模块的 `AppExit`/`Last` hook（#1503）。
- `server/src/mineral/break_handler.rs:328-388` 在确认 `GatheringSession` 完成前于 `:356-360` 发 `MineralDropEvent`；`mining_completion` 之后才读取/移除 session（#1468）。
- `server/src/mineral/bridge.rs:45-65` 从客户端读取玩家 `position`，却把 `intent.position` 传给 `weights.mark_player`（#1458）。

## §5 非重复比对

respawn skeleton 只处理耗尽点重生，tool durability session plan 只处理工具 instance 绑定，shelflife clock plan 只处理时钟重基准；本骨架不改它们的职责。#1458 是 karma 落点，不是矿脉坐标持久化。

## §6 修复计划骨架

- **P0**：掉落合并调用现有 identity matcher；不相容时开新 stack。将 `AppExit` flush 接入 `Last`，并保持写失败可观察。`SurvivalDrop` 先取得并校验真实 session/完成 tick，再发一次 drop；无 session 直接拒绝。karma 标记传玩家当前位置，矿块坐标仍仅用于 heatmap/事件内容。
- **P1**：identity 不相容与相容 freshness/qi 两组测试；flush 前后重启测试；重复 Stop、早 Stop、无 session 零掉落测试；玩家离开矿块后的 karma last position 与天罚落点测试。
- **qi 门**：若堆叠合并保留 item qi，必须以 `ItemInstance` 的真实 initial/current qi 计算并通过 ledger 断言，不能将 freshness 差异静默折叠；测试引用 `SPIRIT_QI_TOTAL`。

## §7 验证计划

实现后跑 server fmt/clippy/cargo test，重点 `mineral::*`、`gathering::*`、persistence shutdown tests；不跑 agent/client 门禁。
