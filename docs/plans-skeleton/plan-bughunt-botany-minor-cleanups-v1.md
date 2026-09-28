# plan-bughunt-botany-minor-cleanups-v1

> **来源 issue**：#1783、#1779、#1599、#1594、#1523、#1427、#1425。
> 一句话主题：野外 botany 的死亡生成、徒手伤害、采集会话、负灵域门控、跨维度踩踏和扩散占位共用一份小型边界清扫；不涉及 `lingtian` 灵田重构。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 生成位置、伤害、会话模式、zone/维度身份和扩散占位的权威边界 | ⬜ |
| P1 | botany 生产路径回归与守恒/伤情断言 | ⬜ |

## §0 摘要

七条 minor 都能从正常采集、负灵域或死亡事件到达：植物可能在墙内/虚空生成，徒手 hazard 只写 `Wound` 不扣血，模式切换沿用旧起点，`FuChenCao` 的 `NegativeField` 声明与 refresh 门矛盾，噬灵藓使用旧 zone 名做门控而按当前位置入账，扩散循环不即时占位，踩踏检测不带维度。修复应复用现有 botany session、`ZoneRegistry`、`CurrentDimension` 和 qi release 入口，不在本骨架新增植物物理常数。

## §1 立项检查记录

- **worldview**：查 `docs/worldview.md` 的野生灵材、负灵域、伤残和维度语义；本骨架不改灵田/世界观正典。
- **finished_plans**：查 `plan-botany-v1`、`plan-botany-harvest-mode-request-misroute-v1`、`plan-neg-domain-fauna-v1` 与 botany disconnect/harvest 计划，确认本批是生产边界补洞。
- **active plan**：查 `docs/plan-*.md` 中 botany、lingtian、qi ledger、dimension scope；没有一份同时覆盖这七个入口。
- **skeleton**：查 `Plant`、`HarvestSessionStore`、`ShiLingXianDrainTag`、`CurrentDimension`、`occupied`；与 growth-cost ledger、trample race 骨架分别去重。
- **reminder.md**：已查 `docs/plans-skeleton/reminder.md` 的 botany/灵田/负灵域条目；仓内有该文件，但没有这七条的既有登记。

## §2 接入面与跨仓契约

- **Inputs**：`BotanyPlantKind`/`Plant`、死亡事件与 `Position`、`HarvestSessionStore`、`ZoneRegistry`、`CurrentDimension`、`Cultivation`/`Wounds`、噬灵藓 `ShiLingXianDrainTag`。
- **Outputs**：合法地表锚定的植物、真实伤口与血量变化、切换后新的采集起点、按当前 zone/维度的 qi release、去重后的新植物实体。
- **共享类型/事件**：复用 `Plant`、`HarvestSession`、`HarvestTerminalEvent`、`Wound`、`QiFlowOutcome`、`CurrentDimension`、`qi_physics::release::qi_release_to_zone`；不新造第二套 botany session 或植物占位表。
- **三端契约符号**：server `botany::{events::spawn_event_plants,hazard::apply_completion_hazards,harvest::request_harvest_mode,shiling_xian::{moss_step_on_system,moss_drain_system,moss_spread_system}}`；agent **无变更**，这些是 server gameplay 与 qi ledger 内部路径；client **无 wire 变更**，沿用现有 botany progress/plant snapshot，若拒绝原因已有字段则只复用。
- **worldview/qi**：锚定 `docs/worldview.md` §二、§四、§十和维度边界；#1523 的扣除/释放必须走 `qi_release_to_zone`（最终生成 `QiTransfer { from, to, amount, reason: QiTransferReason::ReleaseToZone }`），不要二次调用 overflow；守恒测试引用 `SPIRIT_QI_TOTAL` 与 `qi_physics::ledger::assert_conservation`。

## §3 游玩影响与复现

1. 在墙内、空中或虚空位置触发死亡事件，观察 `KongShouHen` 按死亡坐标生成；徒手采集带 `WoundOnBareHand` 时只出现伤口条目而血量不降。
2. 采集期间走远再把 Auto 切 Manual，旧 `origin_position` 使下一 tick 立即中断；在负灵域 refresh `FuChenCao`，其 `NegativeField` 分支被跳过或随即凋零。
3. 踩藓后移动到另一 zone，旧 tag 仍按 A 门控、释放却按当前位置 B 入账；两个相邻藓源同 tick 命中同一格会生成重复 Plant；异维同坐标藓会误触发踩踏。

## §4 `origin/main` 根因证据

- `server/src/botany/events.rs:70-97` 只查 Overworld zone，随后直接用死亡 `Position` spawn，未调用地表/环境/Y 校验（#1783）。
- `server/src/botany/hazard.rs:185-206` 写入 `Wounds.entries`，没有同步扣 `health_current`（#1779）。
- `server/src/botany/harvest.rs:100-122` 切换模式重置 tick/时长但保留 `origin_position`（#1599）。
- `server/src/botany/registry.rs:970-979` 给 `FuChenCao` 同时声明 `NegativeField` 与 `survive_threshold=0.0`；`lifecycle.rs:480-481` 对非正 zone 跳过 v1 refresh（#1594）。
- `server/src/botany/shiling_xian.rs:206-232` 以 `tag.zone_name` 门控，释放入口按玩家当前位置寻找 zone；`qi_release_to_zone` 的 outcome 只能审计，不能再二次入账（#1523）。
- `server/src/botany/shiling_xian.rs:333-358` 检查 `occupied` 后只 push `new_plants`，未立即插回占位（#1427）；`:87-115` 的位置匹配没有 `CurrentDimension`（#1425）。

## §5 非重复比对

`plan-bughunt-botany-growth-cost-harvest-ledger-v1` 只处理 growth cost 与 item qi 守恒；`plan-bughunt-botany-trample-harvest-race-v1` 只处理会话归属的踩踏竞态；`plan-bughunt-botany-disconnect-session` 已归档且只处理断线 session。本文不改 `lingtian` 灵田整体重构，也不重复已有 qi ledger/死亡链计划。

## §6 修复计划骨架

- **P0**：事件生成复用 `v2_candidate_position`/`check_env_locks` 等真实位置 helper；徒手 hazard 通过统一伤害入口扣血；模式切换把当前权威位置写入 `origin_position`；负灵域 tag 每次按当前位置与维度刷新或失效；moss spread 在接受候选时立刻占位；所有空间判断带 `CurrentDimension`。
- **P1**：为七条路径分别加最小回归：无效死亡坐标不生成、Wound 与 health 同步、模式切换不被旧位置打断、FuChenCao 的 zone 语义可达、跨 zone 释放落在唯一稳定 zone、同 tick 同格只生成一个 Plant、异维同坐标不触发。
- **qi 守恒门**：玩家 `Cultivation.qi_current` 是外部权威；释放只调用 `qi_release_to_zone`/`release_qi_amount_to_zone` 的现有封装并检查 `QiFlowOutcome`，ledger 账户之间才用 `ledger.transfer(QiTransfer { ... })`。断言使用 `SPIRIT_QI_TOTAL`，不写 `100.0` 字面量。

## §7 验证计划

实现后只跑 server 栈 fmt、clippy、cargo test；重点过滤 `botany::`、`shiling_xian`、`harvest` 和 qi ledger 守恒测试，再跑完整 server 门禁。agent/client 无代码改动，不跑其门禁。
