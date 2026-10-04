# plan-bughunt-alchemy-furnace-replaceable-target-v1

> 骨架：来源 #1473。放炉消耗物品后无条件把目标方块覆写为 Furnace。

## §0 摘要

`server/src/alchemy/mod.rs:497-503` 消耗炉物品后直接 `layer.set_block(..., BlockState::FURNACE)`，没有检查目标是否 air/可替换。任意玩家方块可能被覆盖而没有掉落或拒绝。

## §1 游玩影响

放置炼丹炉会破坏箱子、工作台或其他玩家建筑，造成资产损失和 grief 风险。

## §2 复现路径

1. 在非可替换方块上发送炼丹炉放置请求。
2. 观察请求先消费 item，随后 `set_block` 覆写目标。
3. 核对原方块无掉落且炉已出现。

## §3 今天 `origin/main` 根因证据

- `alchemy/mod.rs:497-503` 没有 `is_replaceable`/air 检查就写 Furnace。
- 物品消费与方块写入无失败回滚边界。
- 既有 block placement funnel 的保护未被该入口复用。

## §4 非重复比对

`plan-block-break-integration-v1` 统一破坏入口；本骨架只接入炼丹炉放置的目标可替换性与消费顺序，不改通用破坏语义。

## §5 立项检查记录

本记录已按 `docs/worldview.md`、`docs/finished_plans/`、`docs/plan-*.md`、`docs/plans-skeleton/` 与 `docs/plans-skeleton/reminder.md` 逐项检索。

- **worldview**：查“炼丹炉、建筑保护、放置”；`worldview.md §八` 的炼丹设施不能破坏既有遗迹。
- **finished_plans**：查 `set_block`、`FURNACE`、block place；未见该入口的 replaceable gate。
- **active plan**：查 alchemy furnace/placement；无同一写点修复。
- **skeleton**：查 `alchemy furnace`、`replaceable`、`set_block`；无重复骨架。
- **reminder.md**：查 `furnace`、`replaceable`；仓内无对应条目。

## §6 接入面与跨仓契约

- **Inputs**：炼丹炉放置请求、目标 layer/position、玩家 item instance。
- **Outputs**：成功时 block state 与库存扣除；失败时拒绝且原方块/库存不变。
- **共享类型或事件**：复用通用 block placement validation、`BlockState`、inventory consume result。
- **server 符号**：`alchemy::mod` 放置 handler、`world::block_place`、`consume_item_instance_once`。
- **agent**：无变更；方块放置不经 agent。
- **client**：无变更；仍消费既有 block update/失败反馈。
- **worldview 锚点**：`docs/worldview.md §八` 炼丹设施与建筑保护。

## 阶段总览

| 阶段 | 状态 | 交付物 |
| --- | --- | --- |
| P0 | ⬜ | 先校验目标可替换性，再以原子边界消费物品并写 Furnace |
| P1 | ⬜ | air、可替换植物、非替换容器、并发放置回归测试 |

## 来源 issue

- #1473 `[flash-review][major] 放炉无条件覆写任意方块`
