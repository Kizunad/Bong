# plan-bughunt-dev-give-mineral-count-cap-v1

> Skeleton plan。只读审计产物，来源 issue #1513。

## §0 摘要

`/give <mineral> <count>` 的矿物分支把 `u32 count` 当作循环次数，只拒绝零值，没有服务端上限。一次 operator 调用即可向 `Events<MineralDropEvent>` 排入数十亿事件，生产消费系统会在同一 tick 继续分配实例和写入背包，造成事件洪泛、延迟或 OOM。统一 dev command operator scope 已在 PR #1900 修好，和本骨架的数量边界是两个独立问题。

## §1 游玩影响

即使调用者已经是 operator，超大 count 仍会让服务端在一个命令中排队海量矿物事件；其他玩家的 tick、聊天和正常掉落会被拖慢，极端值可能耗尽内存。该命令是 dev-only，但 dev 工具也必须 fail closed，不能把未经限制的输入直接转成事件数量。

## §2 复现路径

1. 在 `BONG_DEV_MODE` 下以 operator 执行 `/give za_gang 4294967295`。
2. `GiveCmd::Item` 成功解析 `u32`，`handle_give` 只在 `count == 0` 时返回。
3. 矿物分支执行 `for _ in 0..*count`，每次向 `Events<MineralDropEvent>` 发送一条事件。
4. 观察单 tick 事件队列和后续库存消费量无任何上限。

## §3 今天 `origin/main` 证据

- `server/src/cmd/dev/give.rs:14-17`：命令模型使用 `count: u32`。
- `server/src/cmd/dev/give.rs:32-39`：解析器接受完整 `u32` 范围，没有上限 parser。
- `server/src/cmd/dev/give.rs:51-70`：`handle_give` 只拒绝 `count == 0`。
- `server/src/cmd/dev/give.rs:72-96`：矿物 fallback 对 `0..*count` 每项发送 `MineralDropEvent`，数量与输入线性相等。
- `server/src/cmd/dev/mod.rs:193-274`：现有 operator scope/gate 只解决权限，不限制 `give` 的 count；因此 #1513 仍是独立真问题。

## §4 非重复比对

- #1452/#1549/#1573/#1638 等权限 issue 已由 `6e6d936f6`/PR #1900 的统一 operator gate 修复，本骨架明确不回退该门。
- `plan-gathering-mineral-origin-position-break-v1` 处理矿脉来源与破坏位置，不限制 dev `/give` 事件数量。
- 现有 inventory stack 上限不能替代命令入口的事件上限：当前循环在库存消费前就已经构造海量事件。

## §5 立项检查记录

- **worldview**：查 `矿石`、`资源种类`、`灵气是零和的`；命中 `docs/worldview.md §十.资源与匮乏`，没有“GM 可无限瞬时洪泛”的玩法契约。
- **finished_plans**：查 `dev give`、`MineralDropEvent`、`count cap`；找到权限门 PR 证据，未找到数量上限 plan。
- **active plan**：查 `server/src/cmd/dev/give.rs`、`MineralDropEvent`、`u32 count`；无 active plan 修改该循环。
- **skeleton**：查 `give.rs`、`count == 0`、`MineralDropEvent`；无同一入口的 cap 骨架。
- **reminder.md**：查 `give`、`矿物`、`事件洪泛`、`OOM`；仓内无对应条目。

## §6 接入面与跨仓契约

- **Inputs**：operator 的 `GiveCmd::Item { id, count }`、`MineralRegistry`、当前玩家 entity/游戏 tick。
- **Outputs**：`Events<MineralDropEvent>`，由 `consume_mineral_drops_into_inventory` 等生产链路消费；普通 item 分支继续走 `add_item_to_player_inventory`。
- **共享类型或事件**：复用 `GiveCmd`、`MineralDropEvent`、`MineralRegistry`、`InventoryInstanceIdAllocator`；上限应是 server 常量/配置，不新增 IPC schema。
- **server 符号**：`cmd::dev::give::{GiveCmd,handle_give}`、`mineral::MineralDropEvent`、矿物事件消费系统。
- **agent**：无变更；dev command 不由 agent 发起，Redis IPC 不承载该命令。
- **client**：无变更；命令树由 server 下发，结果仍是聊天/库存更新，数量限制在 server 权威入口完成。
- **worldview 锚点**：`docs/worldview.md §十.资源与匮乏` 的有限资源循环。

## 阶段总览

| 阶段 | 状态 | 交付物 |
| --- | --- | --- |
| P0 | ⬜ | 在解析/执行入口建立单一、有文档的矿物 count 上限，超限零副作用拒绝 |
| P1 | ⬜ | 事件数量与 inventory 消费回归，证明 operator 权限门仍有效 |

## P0：输入边界

- 先校验 id 是否为矿物、再校验 `count <= MAX_DEV_GIVE_MINERAL_COUNT`；超限不能先排任何事件。
- 上限只约束矿物事件分支；普通物品继续遵守模板/库存 stack 语义并保留既有错误反馈。

## P1：事件洪泛回归

- 覆盖 0、1、上限、上限+1、`u32::MAX`，断言拒绝路径的事件队列与库存均不变。
- 以 operator/non-operator 矩阵验证权限门和数量门都在，不能由另一条命令 root 绕过。

## 验收测试计划

- 合法上限值恰好产生预期数量的 `MineralDropEvent`。
- 超限请求返回明确聊天错误且事件数为零。
- 非 operator 无论 count 多小都仍被 `gate_dev_commands` 拒绝；operator 的普通 item 分支不受矿物 cap 误伤。

## 来源 issue

- #1513 `[flash-review][major] /give 矿物 count 无上限，单 tick 海量事件致 OOM`
