# plan-bughunt-inventory-bonecoin-termination-drop-v1

> 骨架：来源 #1342。只读核查显示非自然终结会清空骨币，而常规物品才有掉落容器。

## §0 摘要

`server/src/inventory/mod.rs:1059-1060` 将 `inventory.bone_coins` 置零；随后 `RemainsContainer` 只保存 `items` 与 `bone_coins`，但非自然 `should_drop_to_world` 分支（`:1124-1144`）只遍历物品。骨币因此没有可拾取落点。

## §1 游玩影响

玩家被环境、脚本或非自然终结时，骨币从钱包消失，死亡风险直接变成不可追回的货币损失。

## §2 复现路径

1. 持有骨币并触发 `should_drop_to_world` 的终结原因。
2. 观察 `bone_coins` 已清零，地面 `DroppedLootEntry` 没有货币记录。
3. 重连或搜寻死亡点，骨币没有任何恢复入口。

## §3 今天 `origin/main` 根因证据

- `server/src/inventory/mod.rs:1059-1060` 先无条件读取并清零 `bone_coins`。
- `:1062-1123` 的遗骸路径会保存 `RemainsContainer.bone_coins`，但 `:1124-1144` 的世界掉落路径没有对应字段或货币掉落事件。
- 代码没有把骨币转换为可拾取 `ItemInstance` 的补偿路径。

## §4 非重复比对

`plan-death-lifecycle-v1` 定义终结事件顺序，未定义非自然掉落的骨币载体；物品掉落与 TSY 骨币规则也不覆盖这条分支。本骨架只处理货币从钱包到死亡点的交付。

## §5 立项检查记录

本记录已按 `docs/worldview.md`、`docs/finished_plans/`、`docs/plan-*.md`、`docs/plans-skeleton/` 与 `docs/plans-skeleton/reminder.md` 逐项检索。

- **worldview**：查“骨币、货币、死亡、遗骸”；`docs/worldview.md` §九确认骨币是唯一货币，不能静默销毁。
- **finished_plans**：查 `bone_coins`、`RemainsContainer`、`death_drop`；只有终结/掉落流程，没有本分支补偿。
- **active plan**：查 `apply_death_drop_to_inventory`、`should_drop_to_world`；无正在修改非自然骨币交付的 plan。
- **skeleton**：查 `bonecoin`、`termination`、`currency drop`；无同根因骨架。
- **reminder.md**：查 `bone_coins`、死亡掉落；仓内无对应条目。

## §6 接入面与跨仓契约

- **Inputs**：`DeathDropEvent`、`PlayerInventory.bone_coins`、`should_drop_to_world`、`DroppedLootRegistry`。
- **Outputs**：遗骸或地面可拾取的骨币记录、`DroppedItemEvent`/inventory snapshot，且钱包只扣一次。
- **共享类型或事件**：复用 `RemainsContainer`、`DroppedLootEntry`、`DroppedItemEvent`；不另造货币余额。
- **server 符号**：`inventory::apply_death_drop_to_inventory`、`RemainsContainer`、`DroppedLootRegistry`、`network::inventory_snapshot`。
- **agent**：无变更；Redis 世界状态没有骨币掉落专用 payload，本修复只改变 server 掉落权威。
- **client**：无变更；客户端继续消费既有物品/遗骸快照，不新增 wire 字段。
- **worldview 锚点**：`docs/worldview.md §九` 骨币经济与损失可追溯性。

## 阶段总览

| 阶段 | 状态 | 交付物 |
| --- | --- | --- |
| P0 | ⬜ | 为非自然终结定义骨币掉落/遗骸载体并保持原子扣除 |
| P1 | ⬜ | 回归测试：不同终结原因、满容器、重连拾取均只结算一次 |

## 来源 issue

- #1342 `[flash-review][major] 角色非自然终结时骨币被清零且无处掉落`
