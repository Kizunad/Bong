# plan-bughunt-movement-knockback-unit-v1

> 骨架：来源 #1809。玩家击退值标注为 blocks/tick，却直接传给 Valence 的 m/s API。

## §0 摘要

`server/src/movement/player_knockback.rs:47-65` 将 `velocity_blocks_per_tick` 直接传给 `Client::set_velocity`；Valence `client.rs:366` 明确该 API 单位为 m/s。没有 tick→second 转换，击退约弱 20 倍。

## §1 游玩影响

格挡、技能和陷阱的击退几乎不可见，玩家被挤在攻击者身边，战斗距离与世界观中的冲击反馈不一致。

## §2 复现路径

1. 触发带明确 blocks/tick 参数的击退。
2. 记录 server 计算值与 `Client::set_velocity` 入参。
3. 对比应有的 20 ticks/s 转换，观察实际位移显著偏小。

## §3 今天 `origin/main` 根因证据

- `player_knockback.rs:47-65` 字段命名是 `velocity_blocks_per_tick`，直接组装 Vec3 传给 client。
- Valence 源码 `/home/serverkizuna/.cargo/git/checkouts/valence-861f4ff4e4b2dee0/8eff652/crates/valence_server/src/client.rs:366` 标注 velocity 单位为 m/s。
- 代码没有统一单位常量或转换 helper。

## §4 非重复比对

`plan-combat-no_ui` 和技能 plan 只提供冲量数值；本骨架负责 server movement→Valence 边界单位，不修改任何技能平衡常量。

## §5 立项检查记录

本记录已按 `docs/worldview.md`、`docs/finished_plans/`、`docs/plan-*.md`、`docs/plans-skeleton/` 与 `docs/plans-skeleton/reminder.md` 逐项检索。

- **worldview**：查“击退、冲击、距离、战斗”；`worldview.md §四/§七` 要求物理反馈可感知。
- **finished_plans**：查 `set_velocity`、`blocks_per_tick`、knockback；未见单位适配契约。
- **active plan**：查 movement/player knockback；无同一转换修复。
- **skeleton**：查 `knockback unit`、`m/s`、`blocks/tick`；无重复骨架。
- **reminder.md**：查 `knockback`、`velocity`；仓内无对应条目。

## §6 接入面与跨仓契约

- **Inputs**：技能/战斗 knockback impulse、`GameTick`、玩家 client velocity API。
- **Outputs**：m/s 速度向量、位置更新与既有 combat feedback。
- **共享类型或事件**：复用 `Client::set_velocity`、knockback event/`Position`；新增单位常量需集中在 movement，不在客户端另算。
- **server 符号**：`movement::player_knockback`、`valence::Client::set_velocity`、combat knockback producers。
- **agent**：无变更；击退不是 agent IPC。
- **client**：无变更；客户端按 MC 物理消费 server velocity。
- **worldview 锚点**：`docs/worldview.md §四` 身体物理与 §七`战斗反馈`。

## 阶段总览

| 阶段 | 状态 | 交付物 |
| --- | --- | --- |
| P0 | ⬜ | 在 server/Valence 边界明确 blocks/tick→m/s 转换，统一所有击退入口 |
| P1 | ⬜ | 方向、零值、极限速度和多 tick 位移回归测试 |

## 来源 issue

- #1809 `[flash-review][major] 击退速度单位错配`
