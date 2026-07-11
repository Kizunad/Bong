# BugHunt: 普通世界容器断线后 `opened_by` 残留导致永久软锁

> **状态**：✅ 2026-07-11 修复完成（真 bug 确认 → 双层修复 + 7 项饱和测试 → 无上下文 opus validator PASS）

## Bug 摘要

普通可放置世界容器（`StorageCrate` / `DeadDrop`）打开后，如果打开者断线、客户端崩溃或网络中断，客户端不会可靠发送 `external_container_close`。服务端断线清理只把玩家旧 entity 标为 `Despawned`，不会清理该容器上的 `ExternalContainer.opened_by = Some(old_entity)`。

之后任何玩家（包括原玩家重连后的新 entity）再次打开同一容器，`handle_container_open` 会把旧 entity 当作仍在占用，返回“有人正在翻找”，导致货箱 / 草药箱 / 死信箱被永久软锁，除非破坏容器。

本 bug 不重复 #973 / #981 的维度或距离门禁案例：核心不是跨维或走远后继续操作，而是普通世界容器状态机在断线路径缺少 `opened_by` 清理。

## 实际游玩体验影响

玩家在基地里打开货箱整理物品时，如果掉线、崩服前断开、客户端崩溃，箱子会继续显示为“有人正在翻找”。原玩家重连后也可能无法重新打开，因为重连通常是新 client entity，和旧 `opened_by` 不相等。

多人场景下，队友会看到箱子像被一个不存在的人占用；死信箱更糟，非 owner 破坏会触发阵法惩罚，玩家不能把“砸掉再放回”当作无损修复。最终体感是普通仓储和匿名交易容器会因为一次异常断线变成坏箱子。

## 证据定位

- `server/src/world/container_open.rs:104`：只要 `ext.opened_by` 是 `Some` 且不等于当前 `ev.client`，直接拒绝并提示“有人正在翻找”，没有校验旧 entity 是否仍有 `Client` 或是否已 `Despawned`。
- `server/src/player/mod.rs:318`：断线系统消费 `RemovedComponents<Client>`；`server/src/player/mod.rs:450` 只清 `coffin_registry`，随后 `server/src/player/mod.rs:455` 给旧 entity 插入 `Despawned`，没有遍历 `ExternalContainer.opened_by`。
- `server/src/supply_coffin/lifecycle.rs:86`：物资棺有断线释放锁逻辑，但 `server/src/supply_coffin/lifecycle.rs:123` 限定只管理 `ExternalContainerKind::SupplyCoffin`。
- `server/src/supply_coffin/lifecycle.rs:252`：测试明确 pin 住 `StorageCrate` / `DeadDrop` 不归 supply coffin lifecycle 管。
- `server/src/network/client_request_handler.rs:14192` 与 `server/src/network/client_request_handler.rs:14213`：只有收到 owner 的 `external_container_close` 才清 `opened_by`；断线/崩溃不能保证该 C2S 到达。
- `client/src/main/java/com/bong/client/inventory/LootContainerScreenBootstrap.java:29`：断线回调只清客户端本地 `LootContainerStateStore`，不可能补发可靠的服务端 close。
- `server/src/world/container_block.rs:235`、`:251`、`:275`：普通容器被破坏时才会 close/remove session；这不是断线恢复路径。

## 触发路径

1. 玩家 A 放置或找到 `trade_crate` / `herb_crate_placed` / `dead_drop_box`。
2. A 右键打开容器，server 在 `handle_container_open` 设置 `ext.opened_by = Some(A_entity)`。
3. A 客户端崩溃、网络断开或进程被杀，没有发送 `external_container_close`。
4. server 的 `despawn_disconnected_clients` 持久化玩家并把 `A_entity` 标为 `Despawned`。
5. 玩家 B 或 A 重连后尝试打开同一容器。
6. `handle_container_open` 看到 `opened_by = Some(A_old_entity)` 且不等于当前 entity，拒绝打开并提示“有人正在翻找”。

## 反方审查记录

Round 1：反方先审更宽的“普通容器 move 阶段缺持续门禁”候选，结论 PASS；同时指出该表述与距离/维度门禁类问题相近，真实可达性需区分正常客户端能否开屏走远。

Round 2：主线程把候选收窄为断线后 `opened_by` 残留。反方最终结论 PASS：没有找到普通 `StorageCrate` / `DeadDrop` 的断线 cleanup；供给棺释放锁路径被源码和测试明确限制在 `SupplyCoffin`；重连复用旧 entity 也没有证据，旧 entity 已被 `Despawned`。

## Skeleton Fix Plan

- [x] 新增普通外部容器占用清理系统：消费 `RemovedComponents<Client>` 或在现有 `despawn_disconnected_clients` 附近统一扫描 `Query<(Entity, &mut ExternalContainer)>`，当 `opened_by == disconnected_entity` 时置空。
- [x] 不把该逻辑塞进 `supply_coffin::lifecycle` 的专属供给棺超时/碎裂流程；应放在通用 `ExternalContainer` 层或 `container_block` 层，覆盖 `StorageCrate` / `DeadDrop`，并保持 `SupplyCoffin` 现有行为不回退。
- [x] `handle_container_open` 可防御性检查 stale owner：若 `opened_by` 指向的 entity 已无 `Client` 或已 `Despawned`，先释放锁再继续当前 open。
- [x] 对 DeadDrop 保持破坏惩罚语义不变；修复目标是异常断线释放占用，不改变非 owner 破阵规则。

## 验收测试计划

- [x] server 单测：构造 `ExternalContainerKind::StorageCrate`，设置 `opened_by = Some(player)`；模拟 player 断线/removed client 后运行 cleanup，断言 `opened_by == None`。
- [x] server 单测：`ExternalContainerKind::DeadDrop` 同样释放 stale `opened_by`，但不触发破阵、掉落或 session remove。
- [x] 回归单测：`SupplyCoffin` 现有断线释放锁、距离关闭、timeout despawn 行为保持不变。
- [x] 集成测试：A 打开普通货箱后断线，B 在范围内打开成功；A 重连后也能重新打开。
- [x] 负向测试：A 正常在线且打开容器时，B 仍被“有人正在翻找”拒绝。

## 风险

- 如果 cleanup 只按 `Despawned` 扫描，需确认 system ordering 在普通容器 open 前执行，否则同 tick 重连/打开可能仍遇到一次 stale reject。
- 不应误清仍在线玩家的锁；必须以断线 entity 或无 `Client`/`Despawned` 为条件。
- 不应让 DeadDrop 的阵法防砸被绕过；释放的是 UI 占用锁，不是 owner 或 ward 状态。

## Finish Evidence

### 落地清单

- **验真结论**：真 bug。origin/main 上 `despawn_disconnected_clients`（`server/src/player/mod.rs:318`）断线清理只清 `coffin_registry` 后插 `Despawned`，全程不触碰 `ExternalContainer.opened_by`；`supply_coffin/lifecycle.rs:56` 以 `is_supply_coffin_lifecycle_managed` 过滤显式不管 `StorageCrate`/`DeadDrop`；`handle_container_open`（`server/src/world/container_open.rs`）对 `opened_by != ev.client` 无条件拒绝。断线后残留旧 entity → 容器对所有人（含重连后的原玩家——重连是新 entity）永久「有人正在翻找」。
- **修复（双层，均在 `server/src/world/container_open.rs`）**：
  1. 新系统 `release_disconnected_container_locks`——消费 `RemovedComponents<Client>`，释放所有 `ExternalContainer` 上指向断线 entity 的 `opened_by`（不区分 `source_kind`；对 `SupplyCoffin` 与其专属 lifecycle 幂等共存，timeout/despawn/冷却行为不变）；注册进 `container_open::register`。
  2. `handle_container_open` 防御性 stale-owner 检查——新增 `live_clients: Query<(), With<Client>>`，`opened_by` 指向的 entity 已无 `Client` 时直接放行当前 open，兜住同 tick 竞态、`RemovedComponents` 双帧缓冲语义与其他遗漏路径。
- **DeadDrop 语义保持**：只清 UI 占用锁，不触碰 `ContainerBlock`/`DeadDropWard`/掉落/session 注册，破坏惩罚（破阵）语义不变。

### 关键 commit

- `99f73132` 2026-07-11 修复：外部容器断线后 opened_by 残留导致永久软锁（双层修复 + 7 测试）
- `8df80151` 2026-07-11 测试：断言 chat 拒绝消息前显式 flush mock client 包缓冲（修 flaky）
- `512217dd` 2026-07-11 merge origin/main（a422fbc8，干净无冲突）

### 测试结果

- `cargo test --lib world::container_open::` → 9/9 全绿（2 个既有 payload pin + 7 个新增：StorageCrate 断线释放 / DeadDrop 释放且无副作用（items 不变 + entity 不 despawn）/ 三 kind 全覆盖 / 在线持有者不被误清 / A 断线→B 可开→原主重连新 entity 可开全链路 / 防御路径不挂清理系统也生效 / 在线占用仍拒绝且 B 收到「有人正在翻找」）
- `cargo fmt --check` ✅；clippy 本 PR 触碰文件 0 命中（本地 rustc 1.96 在未触碰文件有 ~69 处 pre-existing 噪声）
- 全量 `cargo test`（合并 a422fbc8 后）：10960 passed / 0 failed（另 2 个 wall-clock 预算测试 `world::poi_novice::…blankets_the_aabb`（10s 预算，单跑 7.03s）与 `fauna::migration::…five_ms_budget` 在共享机多 agent 编译抢核时轮换超时，均与本 PR 无关——diff 仅 container_open.rs——两者单独跑均绿，已分区验证覆盖全部测试）

### 跨仓库核验

- server：`release_disconnected_container_locks` / `live_clients` stale 检查 / `ExternalContainer.opened_by`（本 PR 全部落点）
- client：无需改动——`LootContainerScreenBootstrap` 断线只清本地 store 的行为不变，修复全部在 server 权威侧
- agent/schema：不涉及（无 payload/schema 变更）

### 对抗验证

- 无上下文 read-only validator（opus）对 HEAD `8df80151` 判 **PASS**：A bug 真实 / B 修复正确完整（防御检查方向正确、不误清在线锁、SupplyCoffin 幂等、DeadDrop ward 不变）/ C 测试真锁契约非同义反复 / D 无重大遗漏（防御路径兜住 RemovedComponents 双帧与系统注册顺序问题）

### 遗留 / 后续

- 两个 wall-clock 预算测试（poi_novice 10s / fauna 5ms）在共享负载机器上易轮换 flake，属测试基建问题，不在本 plan 范围
- `external_container_close` C2S 仍是唯一的"正常关闭"路径；本修复只兜异常断线，正常 close 流程未动
