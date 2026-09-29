# plan-bughunt-social-niche-guardian-persistence-v1

> 骨架：来源 #1696、#1698。守卫激活与 charge 消耗只改运行态，未稳定持久化且 reveal 会覆盖。

## §0 摘要

`server/src/social/niche_defense.rs:222-255` 激活后只插入运行态组件；`resolve_intrusion:161-193` 消耗 charges，但 handler 仅在 `items_taken` 非空时于 `:302-313` 持久化。材料已扣而激活/充能状态可能重登丢失，reveal 又能覆盖内存值。

## §1 游玩影响

玩家付出材料开启守卫后重登，守卫消失或 charge 回满；入侵消耗无法追踪，社交据点防御失去可信度。

## §2 复现路径

1. 激活 niche guardian，确认材料扣除和运行态 component。
2. 在无 `items_taken` 的入侵中消耗 charge，或立即重登/reveal。
3. 观察持久化记录未变，重新 hydrate 后回到旧值/无守卫。

## §3 今天 `origin/main` 根因证据

- `niche_defense.rs:222-255` 激活只写 ECS 运行态。
- `:161-193` `resolve_intrusion` 改 charges。
- `:302-313` 持久化被 `items_taken` 非空条件包住，charge-only 结果不落库。

## §4 非重复比对

`plan-social-v1` 定义据点/守卫玩法，`plan-niche-craft-fix-v1` 处理视觉音效；本骨架只收敛运行态、持久化和 reveal 的状态权威。

## §5 立项检查记录

本记录已按 `docs/worldview.md`、`docs/finished_plans/`、`docs/plan-*.md`、`docs/plans-skeleton/` 与 `docs/plans-skeleton/reminder.md` 逐项检索。

- **worldview**：查“守卫、据点、充能、材料”；`worldview.md §十一` 要求社交承诺和资源消耗持久一致。
- **finished_plans**：查 `NicheDefense`、`charges`、persistence/reveal；无 charge-only 保存契约。
- **active plan**：查 social niche defense；无同一持久化修复。
- **skeleton**：查 `niche guardian`、`charges`、`reveal`；无重复骨架。
- **reminder.md**：查 `niche`、`guardian`、`charge persistence`；仓内无对应条目。

## §6 接入面与跨仓契约

- **Inputs**：激活请求、材料扣除结果、`NicheDefense`/charge state、intrusion/reveal 事件。
- **Outputs**：持久化 guardian record、charge delta、social snapshot/narration。
- **共享类型或事件**：复用 `NicheDefense`、`NicheDefenseReaction`、persistence slice；不另造 charge 表。
- **server 符号**：`social::niche_defense::{activate,resolve_intrusion}`、persistence save/load、reveal handler。
- **agent**：无变更；守卫状态不在 agent schema。
- **client**：无变更；既有 niche VFX/快照继续读取 server 权威。
- **worldview 锚点**：`docs/worldview.md §十一` 社交据点与势力防御。

## 阶段总览

| 阶段 | 状态 | 交付物 |
| --- | --- | --- |
| P0 | ⬜ | 激活/charge/reveal 统一持久化事务和幂等版本字段 |
| P1 | ⬜ | 材料失败、charge-only、重登、reveal 覆盖回归测试 |

## 来源 issue

- #1696 `[flash-review][major] 守卫激活只改内存组件不落库`
- #1698 `[flash-review][major] 守卫充能消耗从不持久化`
