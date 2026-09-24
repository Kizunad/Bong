# plan-bughunt-botany-disconnect-session

> 阶段总览：P0 ✅ 2026-07-11 · P1 ✅ 2026-07-11 · P2 ✅ 2026-07-11

> 一句话主题：botany 野外采集 session 断线后仍按旧 `client_entity` 继续计时；到完成 tick 时先清掉 `HarvestSessionStore` 里的 session，再因旧实体缺 `Client` 查不到 `PlayerInventory` 而失败，日志说可重试但进度已丢。玩家重连前旧 session 会挡住新实体继续采集，重连后若已被完成路径清掉，则只能从头再采。

> 立项动机：这是持久化 / 重连 / session lifecycle 交界处的真实玩家可达 bug。它不是 #990 普通世界容器断线锁、#894 craft 重连 session、#876 矿脉移动打断，也不同于 `plan-botany-harvest-mode-request-misroute-v1` 的模式请求错接。本 plan 只固化 skeleton，不改代码。

## Bug 摘要

`HarvestSessionStore` 以稳定 `player_id` 保存采集 session，但每个 `HarvestSession` 内仍保存启动时的 ECS `client_entity`。断线后，`player::despawn_disconnected_clients` 会持久化玩家切片并给旧实体插 `Despawned`，但不清理或迁移 `HarvestSessionStore`。

随后 botany 的 Update 链仍会运行 `tick_harvest_sessions`。完成时 `complete_harvest_for_player` 第一行就 `remove_session(player_id)`，然后才用 `Query<&mut PlayerInventory, With<Client>>.get_mut(session.client_entity)` 取库存。旧实体已经没有 `Client`，查询失败，函数返回 Err；调用方只打日志 `"session cleared, plant left un-harvested for retry"`。实际结果是：植物没有收获、产物没有给玩家、terminal 帧不发、session 也已经没了。

## 对实际游玩体验的影响

玩家在野外采药时掉线或崩客户端，会遇到两个坏状态：

1. 若重连发生在 server 计时完成前，服务端仍保留旧 session。新客户端实体尝试重新开始采集时，`start_or_resume_harvest` 看到同一个 `player_id` 已有 session 就直接 return，但 active progress 仍发给旧 `client_entity`，新客户端看不到进度，也无法真正 resume。
2. 若 server 在玩家离线期间把 session 计到完成，完成路径会清掉 session 后因旧实体缺 `Client` 失败。玩家回来后植物还在，但之前等待的采集进度被静默抹掉，只能从头再采；客户端断线时也已经清了本地采集 store，不会有可靠的恢复提示。

体感上这是“断线吃掉采集进度 / 重连后采集卡住或回到起点”，高频发生在药草采集中途网络抖动、客户端崩溃、切服重连等场景。

## 证据定位

- `server/src/botany/components.rs:95`：`HarvestSession` 保存 `player_id` 和启动时 `client_entity`。
- `server/src/botany/components.rs:175`：`HarvestSessionStore` 以 `sessions_by_player: HashMap<String, HarvestSession>` 保存 session。
- `server/src/botany/harvest.rs:60`：`start_or_resume_harvest` 对同 `player_id` 已存在 session 直接 return，不更新为新 `client_entity`。
- `server/src/botany/harvest.rs:89`：`request_harvest_mode` 会拒绝不同 `client_entity` 的模式请求，重连新实体无法操作旧 session。
- `server/src/botany/harvest.rs:134`：`complete_harvest_for_player` 先 `remove_session(player_id)`。
- `server/src/botany/harvest.rs:146`：随后才查 `PlayerInventory + With<Client>`；断线旧实体命中失败。
- `server/src/botany/harvest.rs:548`：`tick_harvest_sessions` 只按 `progress_at(now) >= 1.0` 完成，不要求玩家在线。
- `server/src/botany/harvest.rs:597`：Err 分支日志称 `session cleared, plant left un-harvested for retry`，但没有把 session 放回 store。
- `server/src/botany/harvest.rs:1310`：现有测试固定了“缺 inventory 时植物不收获且 session 被清掉”的行为，缺少断线语义保护。
- `server/src/player/mod.rs:318`：断线清理参数不含 `HarvestSessionStore`，只持久化玩家切片 / 清 coffin / 标记 `Despawned`。
- `client/src/main/java/com/bong/client/botany/BotanyHudBootstrap.java:30`：客户端断线时清空本地 botany session store，无法靠 client resume token 恢复。

## 触发路径

1. 玩家开始野外 botany 采集，服务端 `HarvestSessionStore` 记录 `player_id = offline:<name>` 与当前 `client_entity`。
2. 玩家在采集未完成时断线。客户端清本地采集状态；服务端断线清理持久化玩家，但不清 botany session。
3. `enforce_harvest_session_constraints` 对缺 `Client` 的旧实体移动判定为 false，不产生 interrupted terminal。
4. 若玩家很快重连，新实体触发采集时被旧 `player_id` session 挡住；旧 session 的进度帧仍路由到旧实体。
5. 若离线期间 tick 达到完成，`tick_harvest_sessions` 调 `complete_harvest_for_player`，先删 session，再因旧实体无 `Client` 查询不到库存而失败。
6. 玩家回来后看不到完成反馈，没拿到药草，原等待进度丢失；植物仍可采，但必须从头开始。

## 反方审查记录

### Round 0：灵木候选被推翻

最初怀疑 `spiritwood` 灵木采伐也会离线完成后吞掉落。反方指出灵木 `start_spiritwood_sessions` 必须保存 `Some(tool_instance_id)`，断线后 `enforce_spiritwood_session_constraints` 的库存查询缺失会让 `tool_switched = None != Some(tool_id)`，并且该 enforce 在 completion 前 `.chain()` 执行。因此正常灵木 session 会被取消，不采用该候选。

### Round 1：botany 候选确认

反方未能推翻 botany 问题：

- botany Update 链持续运行，`tick_harvest_sessions` 只看 `GameplayTick` 与 session 进度。
- 断线清理没有 `HarvestSessionStore` 参数，也没有 `RemovedComponents<Client>` 专门清理 botany session。
- `HarvestSessionStore` 按 `player_id` 保存，但 session 内旧 `client_entity` 不会在重连时迁移。
- 完成函数先删 session 再查库存，旧实体缺 `Client` 会失败，且日志里的 retry 没有实际 session 可 retry。

### Round 2：强反方通过

第二轮专门攻击“这只是可接受的断线取消 / 已有设计 / 可自动恢复”。结论仍通过：

- 这不是干净取消：服务端不会发 interrupted terminal，也不会把 session 明确取消给客户端。
- 重连不能 resume：客户端断线已清 store；服务端 active progress 发给旧实体；新实体 start 被旧 player_id session 挡住。
- 现有测试只覆盖结构性失败后植物保持可重收，没有定义断线 session 的取消或恢复语义。
- 开放 PR / active plan 未覆盖同问题；`plan-botany-harvest-mode-request-misroute-v1` 是模式请求错接，不是断线旧实体 lifecycle。

## Skeleton Fix Plan

### P0 ✅ 2026-07-11 - 明确断线语义：取消或迁移，二选一落地（选定方案 A：断线即取消）

- 方案 A：断线即取消 botany session。
  - 在 player disconnect cleanup 之前或其中接入 `HarvestSessionStore`，按 `canonical_player_id` 找 session，移除并发 `HarvestTerminalEvent { interrupted: true, detail: "断线打断" }`。
  - 保证植物不收获、进度不继续计时、重连后可正常重新开始。
- 方案 B：断线 session 暂停并支持重连迁移。
  - `HarvestSession` 增加在线/暂停状态或把 `client_entity` 变成可重绑定字段。
  - 新实体 join 后按 `player_id` 迁移 session 的 `client_entity`，并重新推送 progress。
  - 完成路径必须先确认 online inventory 可用；离线期间不得先 `remove_session` 后失败。

建议 P0 先选方案 A。它更符合当前客户端断线清 store 的行为，修复面小，且避免离线期间自动完成采集带来的库存/掉落/危险结算歧义。

### P1 ✅ 2026-07-11 - 修正完成路径原子性

- `complete_harvest_for_player` 不应在所有结构性前置校验成功前 `remove_session`。
- 对缺 `Client` / 缺 `PlayerInventory` 的活跃 session，要么保留 session 等待明确取消 / 迁移，要么走统一断线取消路径。
- 日志文案要与实际状态一致：如果 session 已清，就不要写 `for retry`；如果要 retry，就必须保留或恢复 session。

### P2 ✅ 2026-07-11 - 清理测试语义

- 改写 `harvest_completion_missing_player_inventory_leaves_plant_unharvested`：缺 inventory 不应把断线场景固定为“session 清掉”。
- 增加专门的 disconnect lifecycle 测试，不再用“系统装配缺陷”间接覆盖断线。

## 验收测试计划

- server 单测：玩家开始 botany 手动采集后移除 `Client`，下一 Update 应按选定语义取消 session 或暂停 session；不得静默继续到完成后清 session。
- server 单测：离线期间 tick 超过 `duration_ticks`，不得调用旧实体库存 grant，不得清 session 后只打 retry 日志。
- server 单测：断线取消方案下，`HarvestTerminalEvent.interrupted=true` 且 detail 明确为断线；植物 `harvested=false`，重连后同 `player_id` 可重新开始采集。
- server 单测：若采用迁移方案，新实体 join 后同 `player_id` 的 session `client_entity` 被重绑定，progress payload 发给新实体，模式请求不因旧 entity 被拒。
- client/server 集成或 bot e2e：采集中途断线重连，HUD 不应卡在无进度状态；玩家可重新采或继续采，且不会丢药草、不会白给完成、不会出现旧 session 挡住新 session。
- 回归：`plan-botany-harvest-mode-request-misroute-v1` 的 session mode 请求 pin 仍然通过，避免把“模式请求”和“断线恢复”修成互相覆盖。

## 风险

- 若选择断线即取消，玩家短暂网络抖动会丢当前采集进度；但这是显式取消，至少不会卡住或假装 retry。
- 若选择暂停/迁移，必须明确离线期间 hazard、受击、移动、XP、库存 grant 的权威语义，修复面会扩大。
- `HarvestSessionStore` 同时保存技能 XP，改资源结构时不能误清 `skills_by_player`。
- disconnect 系统顺序要放在 `despawn_disconnected_clients` 之前或在仍可解析 username/player_id 的位置执行，否则会丢失 canonical player id。
- 修 `complete_harvest_for_player` 的 remove 时机时，要保护既有“植物未标 harvested 前结构性失败不吞产物”的回归意图。

## Finish Evidence

### 落地清单

- **P0 方案 A（断线即取消）**：`server/src/botany/harvest.rs` 新增系统 `release_disconnected_harvest_sessions`——消费 `RemovedComponents<Client>`（范式同 `world::container_open::release_disconnected_container_locks`），断线当帧按旧 `client_entity` 定位 session，移除并补发 `HarvestTerminalEvent { interrupted: true, detail: "断线打断" }`；`skills_by_player`（采集熟练度）有意保留。注册在 `server/src/botany/mod.rs` Update 链 `enforce_harvest_session_constraints` / `tick_harvest_sessions` 之前（整组 `.chain()` 锁序），保证断线当帧 session 恰到完成 tick 时取消路径胜出。重连后同 `player_id` 可立即用新实体重新开始采集。
- **P1（完成路径原子性/诚实语义）**：`complete_harvest_for_player` 两条结构性前置校验失败分支（缺 kind / 缺 `Client`+`PlayerInventory`）经 `send_structural_cancel_terminal` 补发 `interrupted=true, detail="结算异常打断"` 终结帧——session 在入口已移除，不发帧客户端 HUD 停在进度满格永远等不到收口。grant 阶段结构性失败的"无终结帧"语义系 plan-botany-harvest-full-inventory-loss-v1 §8.1 已 pin，不翻案。`tick_harvest_sessions` Err 日志文案改为与实际状态一致（session cancelled、需重新发起采集，不再谎称 for retry）。
- **P2（测试语义）**：`harvest_completion_missing_player_inventory_leaves_plant_unharvested` 不再间接覆盖断线场景（断线语义归 P0 专属测试组），改锁结构性失败显式取消契约（恰一条 interrupted 终结帧 + detail + session_id + client_entity）；`harvest_completion_missing_kind_registry_leaves_plant_unharvested` 同步补终结帧断言。grant 失败无终结帧 pin、无 session 断线 no-op pin 均保持。

### 关键 commit

- `e54874e8` 2026-07-11 — P0 方案 A：断线即取消系统 + 6 例饱和测试（Model: claude-sonnet-5）
- `b5016c13` / `9ff44a2c` 2026-07-11 — 测试导入修复 + 触碰文件 clippy 清零（Model: claude-sonnet-5）
- `08642f42` 2026-07-11 — P1+P2：结构性失败显式取消语义 + 测试改锁新契约（Model: claude-fable-5）

### 测试结果

- `cd server && cargo fmt --check && cargo test`：全绿（TEST_EXIT:0，含 full_app_startup smoke 与全部集成测试）
- 新增/改写测试：P0 断线取消 + 终结事件断言 / 断线当帧完成竞态（植物不标 harvested、无产出、仅一条 interrupt 事件）/ 重连新实体可重新开始 / 无 session 断线 no-op / 多玩家只取消断线者 / skill XP 保留（6 例）；P1/P2 结构性失败恰一帧断言（missing inventory / missing kind 各一）
- clippy：触碰文件 0 命中（全仓 `manual_is_multiple_of` 等为本机 rustc 1.96 pre-existing 噪声，CI pinned 版本不受影响）

### 跨仓库核验

- server：`release_disconnected_harvest_sessions` / `send_structural_cancel_terminal` / `HarvestTerminalEvent.interrupted`（`server/src/botany/harvest.rs`、`server/src/botany/mod.rs`）
- client：无需改动——`HarvestTerminalEvent` 走既有 `bong:server_data` 桥；下游消费者（audio/vfx/overflow）按 `!completed || interrupted` 跳过，对断线旧实体安全（validator 双轮核验确认）
- agent：不涉及

### 对抗验证

- 无上下文 Explore validator 两轮：`PASS dafaedff...`（P0+merge main）、`PASS 08642f42...`（P1+P2 增量，专项质疑双发帧/§8.1 pin 破坏/下游 panic 均排除）

### 遗留 / 后续

- 方案 B（断线暂停+重连迁移 session）未采用——与客户端断线清 store 行为不符且扩大离屏结算歧义，如未来需要"断线保进度"体验再立新 plan
- `botany/mod.rs` 注册顺序本身无专属 pin 测试（由竞态测试间接锁定），validator 判定可接受
