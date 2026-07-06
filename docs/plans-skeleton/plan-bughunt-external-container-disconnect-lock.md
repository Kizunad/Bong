# BugHunt: 普通世界容器断线后 `opened_by` 残留导致永久软锁

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

- [ ] 新增普通外部容器占用清理系统：消费 `RemovedComponents<Client>` 或在现有 `despawn_disconnected_clients` 附近统一扫描 `Query<(Entity, &mut ExternalContainer)>`，当 `opened_by == disconnected_entity` 时置空。
- [ ] 不把该逻辑塞进 `supply_coffin::lifecycle` 的专属供给棺超时/碎裂流程；应放在通用 `ExternalContainer` 层或 `container_block` 层，覆盖 `StorageCrate` / `DeadDrop`，并保持 `SupplyCoffin` 现有行为不回退。
- [ ] `handle_container_open` 可防御性检查 stale owner：若 `opened_by` 指向的 entity 已无 `Client` 或已 `Despawned`，先释放锁再继续当前 open。
- [ ] 对 DeadDrop 保持破坏惩罚语义不变；修复目标是异常断线释放占用，不改变非 owner 破阵规则。

## 验收测试计划

- [ ] server 单测：构造 `ExternalContainerKind::StorageCrate`，设置 `opened_by = Some(player)`；模拟 player 断线/removed client 后运行 cleanup，断言 `opened_by == None`。
- [ ] server 单测：`ExternalContainerKind::DeadDrop` 同样释放 stale `opened_by`，但不触发破阵、掉落或 session remove。
- [ ] 回归单测：`SupplyCoffin` 现有断线释放锁、距离关闭、timeout despawn 行为保持不变。
- [ ] 集成测试：A 打开普通货箱后断线，B 在范围内打开成功；A 重连后也能重新打开。
- [ ] 负向测试：A 正常在线且打开容器时，B 仍被“有人正在翻找”拒绝。

## 风险

- 如果 cleanup 只按 `Despawned` 扫描，需确认 system ordering 在普通容器 open 前执行，否则同 tick 重连/打开可能仍遇到一次 stale reject。
- 不应误清仍在线玩家的锁；必须以断线 entity 或无 `Client`/`Despawned` 为条件。
- 不应让 DeadDrop 的阵法防砸被绕过；释放的是 UI 占用锁，不是 owner 或 ward 状态。
