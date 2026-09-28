# plan-bughunt-dev-command-execution-gate-v1

> **来源 issue**：#1469。`/tpzone` 虽在 dev command root 下，`gate_dev_commands` 未阻断 `CommandResultEvent`，普通玩家仍可能进入 handler 直接改写 `Position`。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 命令执行层 fail-closed operator/dev gate | ⬜ |
| P1 | tpzone 未授权零副作用与 operator 正常路径回归 | ⬜ |

## §0 摘要

`server/src/cmd/dev/mod.rs:193-274` 通过 command scope 给客户端树提示并在 `gate_dev_commands` 中发聊天提示，但 gate 没有消耗/取消对应 `CommandResultEvent`。`tpzone::handle_tpzone`（`tpzone.rs:38-63`）仍直接给任意 executor 设置目标 zone 中心位置。若未经授权的命令事件进入 Update，玩家可绕过探索/境界限制。

## §1 立项检查记录

- **worldview**：查 `docs/worldview.md` §十三区域表与 dev-only 命令约束；本骨架不改变 zone 定义，只收紧执行授权。
- **finished_plans**：查 PR #1900 的 operator gate、dev command registry、movement commit；确认现有 scope 解决客户端树可见性但没有消费层 fail-closed 断言。
- **active plan**：查 `cmd/dev`、movement commit、command executor；无统一 `CommandResultEvent` 拒绝/短路实现。
- **skeleton**：查 `plan-bughunt-debug-combat-production-gate-v1`、give/zone_qi 权限骨架；它们分别处理 debug combat 或单命令权限，不覆盖 tpzone handler 消费。
- **reminder.md**：查 dev、operator、tpzone、command gate；仓内有该文件，无本 issue 登记。

## §2 接入面与跨仓契约

- **Inputs**：`CommandExecutionEvent`/`CommandResultEvent<TpzoneCmd>`、`DevCommandPermissions`、executor `Username`、`Position`、`ZoneRegistry`。
- **Outputs**：未授权命令在 handler 前被拒绝且 position、聊天成功消息、movement side effect 均为零；operator 仍可正常传送。
- **共享类型/事件**：复用 `DevCommandScope`、`CommandScopes`、`CommandResultEvent` 与 `AuthoritativePositionCommitSet`；不新增 client-only gate。
- **三端契约符号**：server `cmd::dev::{scope_dev_command_roots,sync_operator_scope,gate_dev_commands}`、`tpzone::handle_tpzone`；agent **无变更**，dev command 不走 IPC；client **无 wire 变更**，原版 command tree 继续自动补全且只显示授权 scope。
- **worldview/qi**：dev 命令明确是测试绕过入口；本骨架不新增 gameplay qi，禁止复用到生产路径。

## §3 游玩影响与复现

普通玩家直接发送 `/tpzone <zone>` 或构造执行事件；预期应被拒绝，现状 gate 只发提示但 handler 仍可能把玩家传送到 zone 中心上方。

## §4 `origin/main` 根因证据

- `server/src/cmd/dev/mod.rs:193-274` 的 `gate_dev_commands` 对未授权请求只 `send_chat_message`，没有阻断/消费 command result。
- `server/src/cmd/dev/tpzone.rs:38-63` 不读取 `DevCommandPermissions`，命中 zone 后直接 `position.set`。
- `server/src/cmd/dev/mod.rs:137-180` 仍无条件注册 `tpzone`，而 `PUBLIC_COMMAND_ROOTS` 不包含它；scope 是提示层而不是 handler 的权威授权结果。

## §5 非重复比对

PR #1900 的其他 dev command 已有 operator 入口核验；`plan-bughunt-debug-combat-production-gate-v1` 只处理 `/bong combat` 的 AttackIntent，不应复用为 tpzone 的权限实现。本骨架收口通用执行门后再逐项核对其他 dev handler。

## §6 修复计划骨架

- **P0**：在 `CommandResultEvent` 消费前由统一 gate 真实丢弃未授权结果，或使 event 进入 handler 前带有不可伪造的 authorized marker；仅发提示不能算拒绝。`tpzone` 保留 `AuthoritativePositionCommitSet` 顺序。
- **P1**：未授权 tpzone、未知 zone、operator 合法传送、离线模式未授权四组测试，断言 position/registry/movement event 零副作用。

## §7 验证计划

实现后跑 server fmt/clippy/cargo test，重点 `cmd::dev` 和 `tpzone`；不跑 agent/client。
