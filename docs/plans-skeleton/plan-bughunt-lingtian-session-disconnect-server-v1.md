# plan-bughunt-lingtian-session-disconnect-server-v1

## §0 摘要

灵田六类 session（`ActiveLingtianSessions`）不挂断线清理：actor 实体断线消失后 session 仍照常 tick 完成，种植免费吃种子/收获产物静默蒸发。`ActiveLingtianSessions.by_actor` 的 `tick_all()` 只对存进 HashMap 的 `ActiveSession` 枚举内部字段自增，完全不查询 actor 是否仍是活着的 ECS 实体；玩家断线后完成结算命中"NPC 自带种子"分支（`inventories.get_mut(actor)` 查询失败），种植侧免费复制种子，收获侧作物/掉落静默丢弃但 `plot.crop = None`/`harvest_count++` 仍无条件执行。

**置信度说明**：本条 finding 来自 bughunt 20260726-r2 主题轮的 `overflow_full` 队列——因 severity 排序在本轮截断，未进入 skeptic 对抗对峙轮，没有独立 verdict。本骨架的证据链仅为 finder 读码 + finding 自带的去重比对，未经过第二轮对抗验证，实施前建议优先做一次独立复核。

本 plan 仅是 BugHunt Skeleton Plan，不包含实际修复。

## §1 实际游玩体验影响

- 玩家发起种植/采集后短窗口内断线（网络抖动、Alt+F4、客户端崩溃均可，不需要脚本化精确到帧），保持离线直到该 tick 在服务端走完即可复现异常结算。
- 种植侧：`apply_planting_completion` 命中"caster 无 `PlayerInventory`"分支时会把这段代码当作"NPC 自带种子"处理，作物照常种下但种子从未被扣——对于刚好在这个窗口断线的玩家，等价于一次免费复制。
- 采集侧：`apply_harvest_completion` 命中同款 `None` 分支时把收获物/种子掉落静默丢弃（debug 日志"NPC 消费 offscreen"），但 `plot.crop = None; harvest_count++` 仍无条件执行——成熟作物凭空蒸发且不可恢复，窗口比种植更宽（HARVEST_AUTO_TICKS/REPLENISH Zone 更长），断线命中概率现实中更高。
- 均不需要任何 dev 命令，是断线这一现实网络条件下的常规游玩场景。

## §2 复现路径

1. 玩家持种子 + 可种植空 plot，发送 `lingtian_start_planting`。
2. 在约 1 秒窗口内（`PLANTING_TICKS=20`，`server/src/lingtian/session.rs:32`）断线（网络抖动/客户端崩溃/主动退出均可）。
3. `player::despawn_disconnected_clients` 给旧实体 `insert(Despawned)`（`player/mod.rs:478`），随后该实体在同 tick 内被 valence 真正移出 World；`ActiveLingtianSessions.tick_all()`（`systems.rs:249-253`）不查询 actor 是否仍是活着的 ECS 实体，session 照常 tick 完成。
4. 现状预期：`apply_planting_completion`（`systems.rs:918-963`）在完成结算时 `inventories.get_mut(actor)` 查询失败（实体已不存在），命中"NPC 自带种子"分支——crop 照常种下，但对应种子从未被扣，是一次免费复制。
5. 对称地，采集侧：玩家发起采集后同样窗口内断线，`apply_harvest_completion`（`systems.rs:969-1069`）命中同款 `None` 分支，收获物/种子掉落静默丢弃，但 `plot.crop = None; harvest_count++`（`:1069-1070`）仍无条件执行，成熟作物凭空蒸发且不可恢复；采集窗口更宽（约 7~8 秒），现实断线概率更高。
6. 修复后预期：断线当帧按 actor Entity 从 `ActiveLingtianSessions` 中 clear 掉对应 session（取消而非放任完成），不触发 `apply_*_completion`。

## §3 根因证据

- `server/src/lingtian/systems.rs:195-197` `ActiveLingtianSessions.by_actor: HashMap<Entity, ActiveSession>`，`tick_all` 只对 enum 内部计时，不查询任何 ECS 组件。
- `server/src/lingtian/systems.rs:936-957`（函数体在 918-963 区间）`apply_planting_completion`：`inventories.get_mut(actor).ok()` 为 `None` 时跳过 `consume_one_seed`，注释"NPC 自带种子"，未区分"真 NPC"与"断线玩家"。
- `server/src/lingtian/systems.rs:1013-1030`（函数体在 969-1069 区间）`apply_harvest_completion` 同款 `None` 分支静默丢弃收获物，`plot.crop = None; plot.harvest_count = plot.harvest_count.saturating_add(1)`（确认精确行号 `:1069-1070`）仍照常清空+计数。
- `server/src/lingtian/session.rs:32` `PLANTING_TICKS=20`（1 秒完成窗口），采集侧 `HARVEST_AUTO_TICKS`/`REPLENISH` Zone 窗口更宽（约 140/160 tick，7~8 秒）。
- `handle_start_planting` 只在起手时校验背包里有种子（`systems.rs:391-397` `player_has_seed_for`），真正的 `consume_one_seed` 扣减发生在 1 秒后的 `apply_planting_completion` 完成结算里——起手校验和完成扣减之间存在时间窗口。
- 对照修法：`server/src/botany/harvest.rs:559,574` `release_disconnected_harvest_sessions` 消费 `RemovedComponents<Client>`（valence 在客户端连接丢失时移除该组件），是同一套断线场景的正确修法，唯独没被搬到 lingtian 模块。全仓排查 `RemovedComponents<Client>` 使用点，lingtian 模块没有注册任何清理系统去 `sessions.clear(actor)`。

## §4 非重复比对

- `plan-bughunt-lingtian-session-disconnect-ui-v1`（已存在于 `docs/`）明确对象是 client 侧 `LingtianSessionStore.java`（static volatile HUD 快照），该 plan 去重段自述"不重复 c2s 门禁"，完全不涉及 server 侧 `ActiveLingtianSessions` 的完成结算逻辑——本 finding 是 server 端权威状态机缺口，不重复。
- `docs/plan-bughunt-lingtian-plot-qi-ledger-gap-v1.md`：专注 `plot_qi` 与 `WorldQiAccount` 的守恒记账缺口，与种子/收获物这类"玩家背包物品"的 session 完成结算完全不同层面，不重复。
- `docs/finished_plans/plan-bughunt-botany-disconnect-session.md`（已归档修复）：修的是 `botany::harvest.rs` 的 `HarvestSessionStore`（野外采集，不同模块不同数据结构），并未触及 `lingtian::systems.rs` 的 `ActiveLingtianSessions`——两个模块各自独立实现同一类会话，botany 那条已经修好，lingtian 这条被漏掉了，是同一 bug class 的姊妹案例而非重复。

## §5 修复计划骨架

### P0 断线清理系统

- 仿照 `botany::harvest.rs` 的 `release_disconnected_harvest_sessions`，为 lingtian 新增消费 `RemovedComponents<Client>` 的断线清理系统：断线当帧按 actor Entity 从 `ActiveLingtianSessions` 中 clear 掉对应 session（取消而非放任完成），不触发 `apply_*_completion`。
- 若要保留"NPC 散修"复用同一函数的设计，需要在完成结算处区分"caster 从未有 `PlayerInventory` 组件（真 NPC）"与"caster 实体已被断线清理彻底移除（曾是玩家）"两种语义，而不是用同一个 `Query::get_mut` 失败分支笼统吞掉。

### P1 测试

- 补单测：玩家在 planting 窗口内断线（模拟 despawn），session 被清理不完成，种子未被消耗、plot 未被种下。
- 补单测：玩家在 harvest 窗口内断线，session 被清理不完成，plot 保留成熟状态、`harvest_count` 不递增。
- 补单测：真 NPC（从未持有 `PlayerInventory`）路径行为保持不变（回归保护，验证"区分真 NPC 与断线玩家"逻辑正确）。

## §6 验证计划

- `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
- 手工/bot 复现矩阵：种植窗口内断线（验证种子不复制）、采集窗口内断线（验证作物不丢失）、真 NPC 路径不受影响。

## §7 接入面与守恒说明

- 进料：`RemovedComponents<Client>`、`ActiveLingtianSessions.by_actor`、`PlayerInventory`。
- 出料：session 清理（取消而非完成结算）。
- 跨端契约：本问题是纯 server 内部会话状态机问题，不涉及 client/agent 契约变更；若采纳"区分真 NPC/断线玩家"方案，可能需要在 session 数据结构上新增标记字段（server 内部，非跨端 payload）。
- qi_physics：种子/收获物属于库存物品范畴，不涉及真元/灵气转移，不新增 qi 常数或 ledger 流。

## §8 对抗复核结论

- **未经 skeptic 对峙轮**：本条 finding 属于 bughunt 20260726-r2 的 `overflow_full` 队列，因 severity 排序截断未进入本轮对抗对峙，没有独立 `verdict`（`is_real`/`reachable` 均未经第二方核验）。
- 候选证据（finder 自证，未经对抗）：`ActiveLingtianSessions.tick_all()` 确认不查询 ECS 实体存活性；`apply_planting_completion`/`apply_harvest_completion` 的 `None` 分支确认存在且行为如描述；`botany::harvest.rs::release_disconnected_harvest_sessions` 确认是同类场景的已有正确修法，lingtian 模块确认没有对应清理系统；起手校验（`player_has_seed_for`）与完成扣减（`consume_one_seed`）之间确认存在约 1 秒（种植）/7~8 秒（采集）的时间窗口。
- 去重比对（finder 自述，未经对抗）：`plan-bughunt-lingtian-session-disconnect-ui-v1` 明确限定 client store，不覆盖 server 端；`plan-bughunt-lingtian-plot-qi-ledger-gap-v1` 是记账缺口，不同层面；`plan-bughunt-botany-disconnect-session`（已归档修复）是不同模块的姊妹案例。
- **实施前建议**：优先对本条 finding 做一次独立第一性原理复核（对照本骨架 §3 的 file:line，实地验证断线窗口内的完成结算行为），确认后再进入正式修复流程；置信度低于本轮其余 6 条已过 skeptic 对峙的 finding。
