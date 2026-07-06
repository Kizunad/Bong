# plan-craft-close-pause-loss-v1（骨架）

> **骨架（草案）**。一句话主题：修复 `craft / workbench` 主闭环里“**关闭界面 = 直接取消**”的语义错配。当前 `CraftScreen` / `WorkbenchScreen` 在 `removed()` 时只要 session 仍 active 就发送 `sendCraftCancel()`，导致玩家按 `Esc` / `C` 关屏、被别的界面顶掉、或只是想临时离开配方页时，进行中的手搓会被当成**主动取消**处理；而 `cancel` 路径按 `CANCEL_REFUND_RATIO=0.7` 只返还 70% 材料，形成“**本应暂停，实际吞 30% 材料**”的实机体验 bug。该行为与 `plan-craft-v1` 明定的“inventory 关闭后任务暂停，重新打开继续”相矛盾。

> **玩家体验影响**：这是正常游玩可达、且代价明确的主路径 bug。玩家做手搓/制作台配方时，只要习惯性按一次 `Esc` 关界面，或被其它 UI 流程打断，就会平白损失 30% 已投入材料；不是做错配方，不是失败结算，而是**关界面这个中性操作被错误映射成了带损耗的取消操作**。对长时配方、稀缺材料、批量制作尤为伤。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 统一 pause / cancel 语义，补最小可证据化状态模型 | fix_pr | ⬜ |
| P1 | client 关闭界面不再隐式 cancel，改显式取消入口 | fix_pr | ⬜ |
| P2 | server 只在 craft UI 处于“打开/继续”态时推进 tick | fix_pr | ⬜ |
| P3 | 回归测试：Esc/关屏/重开/显式取消/批量制作 | fix_pr | ⬜ |

## P0 — 问题钉死

- **设计与实现直接冲突**：
  - `docs/finished_plans/plan-craft-v1.md:134` 明写：`inventory 关闭后任务暂停（in-game 时间不推进）；重新打开继续`
  - `server/src/craft/session.rs:5-6` 也把 `inventory 关闭` 定义为“调用方不调用 tick，自动暂停”
- **client 当前把“关屏”硬映射为“取消”**：
  - `client/.../CraftScreen.java:100-104`：`removed()` 时若 `CraftStore.sessionState().active()` → `ClientRequestSender.sendCraftCancel()`
  - `client/.../WorkbenchScreen.java:108-112`：同样逻辑
  - `client/.../CraftScreen.java:133-136`：按 `C` 或 `Esc` 会直接 `close()`
  - `client/.../WorkbenchScreen.java:141-144`：按 `Esc` 会直接 `close()`
- **取消不是无害操作，而是带税**：
  - `server/src/craft/session.rs:27`：`CANCEL_REFUND_RATIO = 0.7`
  - `server/src/craft/session.rs:458-468`：`cancel_craft()` 只返还未完成部分材料的 70%
  - `server/src/network/craft_emit.rs:284-317`：收到 `CraftCancelIntent` 就走 `PlayerCancelled` 路径，真实加回材料并移除 `CraftSession`

## P1 — 当前实现为什么一定伤玩家

- 玩家并没有点击“取消制作”按钮。`CraftActionBar` 当前只有“开始制作”，并无独立 cancel 按钮；因此**关闭界面本不该携带损耗语义**。
- `CraftScreen`/`WorkbenchScreen` 关闭是高频中性动作：看完材料、临时切屏、按习惯键退出，都可触发。
- 该损耗是**稳定可复现**的，不依赖 race：
  1. 起一个耗时配方
  2. 在 session active 时按 `Esc`（或手搓页按 `C`）
  3. `removed()` 立即发 `craft_cancel`
  4. server 按 70% refund 结算，玩家白损 30% 材料

## P2 — 根因不只一半：pause 机制本身也没落地

- `server/src/network/craft_emit.rs:323-337` 的 `tick_craft_sessions()` 目前只按 `With<Client>` 查询在线玩家 session，并没有“craft UI 正在打开”的 gate。
- 这意味着即便只删掉 client 端的隐式 cancel，session 也会在玩家关掉界面后继续推进，依旧不符合“关闭后暂停”的设计。
- 因此修复不能停在“去掉 sendCraftCancel()”；必须补一个**显式 pause/active gate**，让：
  - `关闭界面` = pause
  - `显式取消` = 70% refund
  - `重开界面` = resume

## P3 — 修复方向

- client：
  - 去掉 `removed()` 中的隐式 `sendCraftCancel()`
  - 增加显式 cancel 入口（按钮或明确热键），只让它触发 `CraftCancelIntent`
  - 打开 craft/workbench 界面时向 server 声明“session 可推进”，关闭时声明“session 暂停”
- server：
  - 给 `CraftSession` 或玩家态增加“UI open / paused”状态
  - `tick_craft_sessions()` 只推进 active 且未 paused 的 session
  - 保证 `CraftSessionStateV1` / 重开 hydration 能恢复同一 recipe、已耗时、批量计数

## P4 — 验收

- `Esc` / `C` / 关闭 `WorkbenchScreen` 不再吃材料，不再发 `PlayerCancelled`
- 关闭后等待任意时长，重开界面时进度保持不变
- 显式 cancel 仍返还 70% 材料，语义不回归
- 批量制作中途关屏再重开，`completed_count` / `remaining_ticks` 连续
- 新增回归测试至少覆盖：
  - `close_screen_pauses_without_emitting_cancel`
  - `explicit_cancel_still_refunds_70_percent`
  - `paused_session_does_not_tick_until_reopened`
  - `workbench_screen_escape_does_not_destroy_session`

## 审计来源

bughunt L2（craft/forge/inventory 主路径，fresh worktree）。候选对比后保留此题，原因：它是**已接线、正常玩家必经、一次按键即可触发材料损失**的主路径 bug；相较于 forge 路径若干“功能未接线”候选，这个题目的可达性、玩家伤害和修复边界都更清晰。
