# plan-bughunt-identity-cooldown-restart-tick-v1

> Skeleton plan。只读审计产物，来源 issue #1480。

## §0 摘要

身份切换冷却的权威字段 `PlayerIdentities.last_switch_tick` 被持久化为服务器运行时的 tick 值。重启后 `GameTick` 从零开始，而加载逻辑原样恢复旧 tick；`identity_panel_emit` 直接把两个不同运行周期的 tick 相减，导致重启后面板在长时间内按刷新间隔反复下发错误的“仍在冷却”状态。

## §1 游玩影响

玩家重启或重连后看到的冷却倒计时与实际身份切换资格不一致：面板可能持续显示满冷却，且每 20 tick 重发一次。若命令门禁也读同一个错位值，玩家会被错误拒绝；即使命令最终允许，UI 也会误导玩家。

## §2 复现路径

1. 在运行周期 `GameTick≈100` 切换身份，使 `last_switch_tick=100` 并保存到 SQLite。
2. 重启服务端，`GameTick` 回到 0，加载 `last_switch_tick=100`。
3. `should_emit_identity_panel_state` 在 `now_tick=0,20,40...` 计算 `last_switch_tick + IDENTITY_SWITCH_COOLDOWN_TICKS`，直到新的运行 tick 追上旧值前持续认为处于冷却刷新窗口。
4. 观察 `IdentityPanelStateV1.cooldown_remaining_ticks` 与玩家本应经历的 wall-clock 冷却不一致。

## §3 今天 `origin/main` 证据

- `server/src/persistence/identity.rs:7-16`：SQLite 表持久化字段名就是 `last_switch_tick`，而非跨重启的 wall-clock/epoch。
- `server/src/persistence/identity.rs:98-147`：`load_player_identities` 直接读取并恢复 `last_switch_tick`，没有与当前 `GameTick` 建立重启基线。
- `server/src/identity/mod.rs:40-44,60-66`：冷却常量为运行 tick，`PlayerIdentities` 只保存该 tick。
- `server/src/network/identity_panel_emit.rs:16-30,43-62`：发包系统读取 `GameTick`，用持久化 tick 加冷却常量计算刷新 deadline；`now_tick=0` 仍满足 20 tick 刷新节奏。
- `server/src/schema/identity.rs:64-71`：`IdentityPanelStateV1` 将 `last_switch_tick` 与 `cooldown_remaining_ticks` 作为 server→client 合约字段。

## §4 非重复比对

- `docs/finished_plans/plan-identity-v1.md` 已定义身份切换和面板 payload，但没有重启后运行时 tick 基线的验收。
- `plan-bughunt-identity-persist-key-mismatch-v1.md` 处理身份持久化主键/角色绑定，不处理 tick 时钟域。
- 当前代码没有把旧 tick 与 `last_updated_wall` 换算的实现；本问题是单独的时间域错位。

## §5 立项检查记录

- **worldview**：查 `身份切换`、`冷却`、`game-day`、`身份与信誉`；命中 `docs/worldview.md §十一.身份与信誉` 的一日冷却设定，未规定重启清空冷却。
- **finished_plans**：查 `IdentityPanelStateV1`、`last_switch_tick`、`persistence/identity`；找到身份功能完成 plan，未找到跨重启时钟转换。
- **active plan**：查 `identity_panel_emit`、`cooldown_remaining_ticks`、`GameTick`；无 active plan 处理该时间域。
- **skeleton**：查 `identity`、`restart`、`tick`、`cooldown`；只有持久化 key/UI 相关骨架，没有本根因。
- **reminder.md**：查 `identity`、`冷却`、`重启`、`tick`；仓内无对应条目。

## §6 接入面与跨仓契约

- **Inputs**：SQLite `player_identities.last_switch_tick`、当前 `GameTick`、`IDENTITY_SWITCH_COOLDOWN_TICKS`、`PlayerIdentities::cooldown_remaining`。
- **Outputs**：`ServerDataPayloadV1::IdentityPanelState(IdentityPanelStateV1)`，经 `bong:server_data` 下发。
- **共享类型或事件**：复用 `PlayerIdentities`、`IdentityPanelStateV1`、`ServerDataV1`、agent schema 的 `IdentityPanelStateV1`；字段不新增，只修时间基线。
- **server 符号**：`persistence::{save_player_identities,load_player_identities}`、`identity::PlayerIdentities`、`network::identity_panel_emit::{should_emit_identity_panel_state,build_identity_panel_state}`。
- **agent**：无变更；身份面板 payload 是 server→client，agent 不消费也不生成该冷却字段。
- **client**：无变更；`IdentityPanelStateHandler`/`IdentityPanelStateStore` 继续消费同一字段，修复应保持 wire 结构不变。
- **worldview 锚点**：`docs/worldview.md §十一` 的身份切换一 game-day 冷却。

## 阶段总览

| 阶段 | 状态 | 交付物 |
| --- | --- | --- |
| P0 | ⬜ | 选定并实现跨重启的冷却时间基线（wall-clock 或持久化 epoch），统一命令/UI 读取 |
| P1 | ⬜ | 迁移旧存档、边界保护和 server→client payload 回归测试 |

## P0：统一时钟域

- 以现有 `last_updated_wall`/wall-clock 或显式 boot epoch 建立可比较的冷却截止点；不得继续把旧进程 tick 当新进程 tick。
- `PlayerIdentities::cooldown_passed`、`cooldown_remaining`、`should_emit_identity_panel_state` 必须共享同一权威计算，避免命令与面板分叉。

## P1：兼容与回归

- 旧表只有 `last_switch_tick` 时采用明确的保守迁移策略，不能因解析失败无限放行或无限拒绝。
- 保持 `IdentityPanelStateV1` 字段类型/名称不变，补服务端序列化与客户端 handler 的现有样本回归。

## 验收测试计划

- 保存后同一进程的冷却行为保持不变。
- 重启后在冷却内、刚过冷却、从未切换三种状态的命令判定和 `cooldown_remaining_ticks` 一致。
- 重启后不再仅因 `now_tick` 是 20 的倍数而重复发送错误的满冷却面板；合法刷新仍按既有节流间隔工作。

## 来源 issue

- #1480 `[flash-review][major] 身份切换冷却跨重启 tick 域错位，重启后满冷却持续刷新`
