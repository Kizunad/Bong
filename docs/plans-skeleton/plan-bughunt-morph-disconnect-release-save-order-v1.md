# plan-bughunt-morph-disconnect-release-save-order-v1

> 骨架：来源 #1395。断线保存与易形释放使用不同调度时序，非法装备快照可先落盘。

## §0 摘要

`server/src/combat/lifecycle.rs:492-506` 通过 deferred command 调用 `release_morph_state`；断线处理随后立即执行持久化。若 command 尚未 flush，保存读取到仍带易形装备的 `PlayerInventory`，形成重登后的非法快照。

## §1 游玩影响

玩家断线瞬间可能把伪皮/易形装备保存成真实持有物，重连后绕过释放清理、重复获得物品或卡住下一次易形。

## §2 复现路径

1. 玩家处于 active morph 状态并马上断线。
2. 让 `release_morph_state` 仍在 deferred command 队列，先触发 persistence save。
3. 重启/重连加载快照，观察 morph 装备仍存在，随后运行期释放再与存档冲突。

## §3 今天 `origin/main` 根因证据

- `combat/lifecycle.rs:492-506` 释放通过 `commands.entity(...).add(...)`/deferred command 排队。
- 同一断线链路在 command flush 前读取并写玩家持久化快照，未等待释放完成。
- `body_plan/morph.rs:386-391` 释放会改 inventory，证明保存顺序必须在其之后。

## §4 非重复比对

`plan-body-plan-morph-v1` 定义易形装备模型，`plan-bughunt-morph-overflow-ledger-v1` 处理释放真元落点；本骨架只修断线事务顺序，不重复余额逻辑。

## §5 立项检查记录

本记录已按 `docs/worldview.md`、`docs/finished_plans/`、`docs/plan-*.md`、`docs/plans-skeleton/` 与 `docs/plans-skeleton/reminder.md` 逐项检索。

- **worldview**：查“易形、伪皮、断线、身份”；`worldview.md §四/§五` 要求形体状态可逆且不复制资产。
- **finished_plans**：查 `release_morph_state`、`save_player`、`Lifecycle`；未见断线 save barrier。
- **active plan**：查 morph lifecycle、persistence save；无同一顺序修复。
- **skeleton**：查 `morph disconnect`、`save order`；无同根因骨架。
- **reminder.md**：查 `morph`、`disconnect`、`persistence`；仓内无对应条目。

## §6 接入面与跨仓契约

- **Inputs**：断线事件、`MorphState`、`release_morph_state`、玩家持久化快照。
- **Outputs**：清理后的 `PlayerInventory`、保存记录、断线后的 server snapshot。
- **共享类型或事件**：复用 `Lifecycle`、`MorphState`、persistence save barrier；不另造断线装备副本。
- **server 符号**：`combat::lifecycle`、`body_plan::morph::release_morph_state`、`persistence::save_player`。
- **agent**：无变更；morph 装备未进入 agent schema。
- **client**：无变更；重连后仍收到既有 inventory snapshot。
- **worldview 锚点**：`docs/worldview.md §四` 身体形态与 §五`身份连续性`。

## 阶段总览

| 阶段 | 状态 | 交付物 |
| --- | --- | --- |
| P0 | ⬜ | 将 morph release 与 save 放入同一有序事务/flush barrier |
| P1 | ⬜ | 断线、进程退出、重复 release 与重连快照回归测试 |

## 来源 issue

- #1395 `[flash-review][major] 断线存档先于易形释放`
