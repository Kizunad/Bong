# plan-bughunt-debug-combat-production-gate-v1

> 严重级别：Blocker（生产授权绕过）。来源 #1867。`/bong combat` 直接生成带 `debug_command` 的 AttackIntent，没有生产权限门。

## §0 摘要

`/bong combat` 是公开命令入口，但 `server/src/player/gameplay.rs:216-219` 在调用 `bridge_debug_combat_action` 前没有用现有 `DevCommandPermissions::is_operator` 校验。`bridge_debug_combat_action`（`:255-274`）随后把参数放进 `AttackIntent.debug_command=Some(action)`；该 intent 会进入 resolver 的 debug 分支，普通玩家可以绕过视野、距离和冷却。它是生产授权绕过，应按 blocker 处理。

## §1 游玩影响

任何普通玩家都可使用公开命令制造不应存在的战斗结果，跳过正常 combat 门禁，构成生产安全问题并破坏 PvE/PvP、公平性和审计可信度。`/bong gather` 与 `/bong breakthrough` 的公开语义不应被一并误伤；授权门只保护 debug combat 分支。

## §2 复现路径

1. 以普通玩家身份发送公开的 `/bong combat <target> <qi_invest>`（`server/src/cmd/gameplay/mod.rs:24-38`）。
2. `handle_bong_gameplay`（`:70-83`）照常把 combat action 入队，`apply_queued_gameplay_actions`（`player/gameplay.rs:216-219`）直接调用 bridge。
3. bridge 生成 `debug_command=Some` 的 intent；`combat/resolve.rs:472-489,595-622,2502-2519` 对 debug intent 跳过 cooldown/reach 反作弊并按 debug target 解析。
4. 修复后的对照应是：普通玩家在 bridge 调用前被现有 operator/dev 授权拒绝，不产生任何 `AttackIntent`；operator 仍可走原调试路径。

## §3 今天 `origin/main` 根因证据

- `server/src/cmd/gameplay/mod.rs:24-38,70-83` 将 `/bong combat` 注册为公开 `BongCmd` 并无条件入 `GameplayActionQueue`；批次 4 的 dev-root gate 不覆盖这个 public root。
- `server/src/player/gameplay.rs:216-219,255-274` 在 bridge 调用前没有读取 `DevCommandPermissions::is_operator`，直接构造 `debug_command=Some` 的 `AttackIntent`。
- `server/src/combat/resolve.rs:472-489` 对 debug intent 不记 cooldown/qi 反作弊，`:595-622` 对 reach 失败也不记违规，`:2502-2519` 走 debug target；因此普通玩家确实可到达绕过门禁的 resolver 分支。
- 现有授权实现位于 `server/src/cmd/dev/mod.rs:115-117,193-274`，问题是该检查没有接在 bridge 调用前，而不是缺少可复用的授权 API。

## §4 非重复比对

批次 4 已修复 dev-root 命令 gate（PR #1900），但 `/bong` 是公开 gameplay root，且其 combat bridge 没有调用同一 `DevCommandPermissions::is_operator`；本骨架补的是生产安全 gate，不重复 dev-root 注册逻辑。

## §5 立项检查记录

本记录已按 `docs/worldview.md`、`docs/finished_plans/`、`docs/plan-*.md`、`docs/plans-skeleton/` 与 `docs/plans-skeleton/reminder.md` 逐项检索。

- **worldview**：查“战斗、公平、天道命令、调试”；`worldview.md §七` 不允许玩家把调试命令当生产招式。
- **finished_plans**：查 dev command authorization、`debug_command`、`AttackIntent`；未见 gameplay bridge gate。
- **active plan**：查 `cmd/dev` 与 player gameplay；无同一生产 gate 修复。
- **skeleton**：查 `debug combat`、`production gate`、`AttackIntent`；无重复骨架。
- **reminder.md**：查 `debug`、`combat`、`operator`；仓内无对应条目。

## §6 接入面与跨仓契约

- **Inputs**：公开 `BongCmd::Combat`、已认证 `Username`、`DevCommandPermissions`、`CombatAction`。
- **Outputs**：仅 operator 授权成功时生成 debug `AttackIntent`；未授权时在调用 bridge 前拒绝、反馈并保证零 `AttackIntent`/零 combat event。
- **共享类型或事件**：复用 `cmd::dev::DevCommandPermissions::is_operator`、`AttackIntent`、`AttackSource`、既有 chat feedback；不新增客户端 debug payload，也不把普通 gather/breakthrough 改成 dev-only。
- **server 符号**：`cmd::gameplay::handle_bong_gameplay`、`player::gameplay::{apply_queued_gameplay_actions,bridge_debug_combat_action}`、`cmd::dev::DevCommandPermissions`、`combat::resolve`。
- **agent**：无变更；命令授权和 combat intent 不经 agent。
- **client**：无变更；玩家输入仍走 server 命令树。
- **worldview 锚点**：`docs/worldview.md §七` 战斗秩序与天道权限。

## 阶段总览

| 阶段 | 状态 | 交付物 |
| --- | --- | --- |
| P0 | ⬜ | 在 `bridge_debug_combat_action` 调用前调用现有 `DevCommandPermissions::is_operator`；未授权不发任何 `AttackIntent`，并保持 gather/breakthrough 公开语义 |
| P1 | ⬜ | 普通玩家零 intent、operator 保留 debug intent、离线未授权 fail-closed、重复命令和审计回归测试 |

## 来源 issue

- #1867 `[flash-review][major] /bong combat 将 debug 攻击路径暴露给生产玩家`
