# plan-bughunt-debug-combat-production-gate-v1

> 骨架：来源 #1867。`/bong combat` 直接生成带 `debug_command` 的 AttackIntent，没有生产权限门。

## §0 摘要

`server/src/player/gameplay.rs:255-274` 的 `bridge_debug_combat_action` 将命令参数直接放入 `AttackIntent.debug_command=Some(action)`，没有 operator/dev scope 校验。生产玩家可按名触发绕过视野、距离和冷却的 debug 攻击。

## §1 游玩影响

任何玩家都可使用调试命令制造不应存在的战斗结果，破坏 PvE/PvP、公平性和审计可信度。

## §2 复现路径

1. 以普通玩家身份发送 `/bong combat`。
2. 观察 gameplay bridge 生成 debug intent 并进入普通 resolver。
3. 对比正常攻击门禁，debug intent 缺少视野/冷却/距离检查。

## §3 今天 `origin/main` 根因证据

- `gameplay.rs:255-274` 构造 `AttackIntent` 并将 `debug_command` 设为 `Some`。
- 周边 `GameplayAction` 分支未先调用 operator/dev authorization gate。
- resolver 以 debug 标记改变正常校验语义，生产路径可达。

## §4 非重复比对

批次 4 已修复部分 dev 命令统一 gate（PR #1900），但其范围不含 `player/gameplay.rs` 的 combat bridge；本骨架补该入口。

## §5 立项检查记录

本记录已按 `docs/worldview.md`、`docs/finished_plans/`、`docs/plan-*.md`、`docs/plans-skeleton/` 与 `docs/plans-skeleton/reminder.md` 逐项检索。

- **worldview**：查“战斗、公平、天道命令、调试”；`worldview.md §七` 不允许玩家把调试命令当生产招式。
- **finished_plans**：查 dev command authorization、`debug_command`、`AttackIntent`；未见 gameplay bridge gate。
- **active plan**：查 `cmd/dev` 与 player gameplay；无同一生产 gate 修复。
- **skeleton**：查 `debug combat`、`production gate`、`AttackIntent`；无重复骨架。
- **reminder.md**：查 `debug`、`combat`、`operator`；仓内无对应条目。

## §6 接入面与跨仓契约

- **Inputs**：命令来源、player identity/operator scope、`CombatAction`。
- **Outputs**：授权时 debug `AttackIntent`；未授权时拒绝反馈且零 combat event。
- **共享类型或事件**：复用现有 dev authorization、`AttackIntent`、`AttackSource`；不新增客户端 debug payload。
- **server 符号**：`player::gameplay::bridge_debug_combat_action`、`cmd::dev` authorization、`combat::resolve`。
- **agent**：无变更；命令授权和 combat intent 不经 agent。
- **client**：无变更；玩家输入仍走 server 命令树。
- **worldview 锚点**：`docs/worldview.md §七` 战斗秩序与天道权限。

## 阶段总览

| 阶段 | 状态 | 交付物 |
| --- | --- | --- |
| P0 | ⬜ | 在 debug combat bridge 前接入统一 operator/dev gate，失败零副作用 |
| P1 | ⬜ | 普通玩家、operator、离线命令、重放和审计回归测试 |

## 来源 issue

- #1867 `[flash-review][major] /bong combat 将 debug 攻击路径暴露给生产玩家`
