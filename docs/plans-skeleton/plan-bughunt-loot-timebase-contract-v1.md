# BugHunt：搜刮面板倒计时混用两端墙钟

> 来源 Issue：#1597。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 为 loot timeout 采用单一时间基准 | ⬜ |

## §0 摘要

server supply_coffin::interact 发送绝对 timeout_wall_secs，client LootContainerPanel 用自己的 System.currentTimeMillis() 相减。两端时钟偏移会让 UI 提前卸载或显示虚假的长剩余时间。

## §1 游玩影响

客户端可能在 server 仍允许搜刮时提前关闭面板，或在 server 已关闭时继续显示倒计时；server 最终仍以 session/close 权威结算，问题限于操作窗口与提示。

## §2 复现路径

把客户端墙钟调快/调慢后打开 supply coffin，观察 LootContainerPanel.tickTimer 与 server loot_timeout_secs 倒计时不同步。

## §3 今天 origin/main 的根因证据

- server/src/supply_coffin/interact.rs:211 写 timeout_wall_secs = now + grade.loot_timeout_secs()。
- client/src/main/java/com/bong/client/inventory/LootContainerPanel.java:194-205 用本地 System.currentTimeMillis()/1000 与 server epoch 相减，InspectScreen.tick 随 tickTimer() 结果卸载面板。
- server/src/network 的 loot session/close payload 是唯一权威；没有 client clock offset 字段。

## §4 非重复比对与立项检查记录

- worldview：查搜打撤、棺材搜刮时间窗与物品所有权；不改掉落/真元规则。
- finished_plans：查 plan-supply-coffin-*、plan-inventory-v1；确认 server session gate 已有，缺口是 UI 时间基准。
- active plan：查 LootContainerPanel、timeout_wall_secs、LootContainerStateStore；未发现同一修法。
- skeleton：查 loot timeout、clock offset；无同主题骨架。
- reminder.md：仓内无 docs/plans-skeleton/reminder.md，无相关条目。

## §5 修复骨架

优先让 server 下发相对 remaining_secs/单调 tick 截止值，或下发 server-now 与 timeout 并在 client 建立一次 offset；禁止直接拿两端未校准 epoch 相减。server 关闭事件到达后仍强制收口，client 只做显示。

## §6 接入面与跨仓契约

- Inputs：supply coffin open/session payload 的 timeout 与 client 单调时间。
- Outputs：LootContainerPanel 的剩余秒数、自动卸载；server Closed/inventory snapshot 仍权威。
- 共享类型或事件：复用 SupplyCoffinSession、LootContainerStateStore、现有 close 事件；字段变更需同步 proto/schema。
- server 符号：supply_coffin::interact、loot session emit/close；若改字段需保持服务端时间计算唯一。
- agent：无变更；搜刮 session 不经 agent IPC。
- client：LootContainerPanel.tickTimer/render 和 state adapter 使用统一 timebase。
- worldview / qi_physics：搜刮时间窗对应世界观搜打撤；不涉及真元物理。
