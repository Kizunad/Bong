# BugHunt: dormant Redis HashReplace 失败后 dirty 预清导致快照持久化回滚

## Bug 摘要

`NpcDormantStore` 的 Redis HASH 持久化在 `publish_world_state_to_redis` 中先 `take_dirty()` 清掉 dirty，再把 `NpcDormantHash` 丢给 Redis bridge。`HashReplace` 走后台 fire-and-forget，失败只 `warn!`，没有 ACK/失败回传给 ECS 重新 `mark_dirty`。

结果是：一次后台 Redis `HashReplace` 超时或失败后，失败本身不会触发重试；只有后续 dormant 真实变更才可能再次全量写 HASH。若失败后马上重启，或 store 已空且无后续变更，`bong:npc/dormant` 会保留旧快照，导致离线 NPC 消失、复活或状态回滚。

## 实际游玩体验影响

玩家远离 NPC 后，NPC 会从 live entity 脱水成 dormant snapshot。如果该次 Redis HASH 写失败且服务器随后重启，启动只会从旧 `bong:npc/dormant` 读取，刚脱水的 NPC 不存在，玩家回到原区域会发现 NPC 像被重启吞掉。

反向路径也会影响游玩：玩家靠近 dormant NPC 后，服务器从 store 移除 snapshot 并水化成 live entity；若 store 变空时本应删除 Redis HASH 的后台写失败，dirty 已清且空 store 不会再自动变脏。重启后旧 dormant snapshot 又被读回，玩家会看到已水化/已移除的 NPC 复活或回滚。

## 证据定位

- `server/src/npc/hydrate/mod.rs:464` 附近：`dehydrate_far_npcs_system` 对远离玩家的 live NPC 调 `store.insert(snapshot)`，随后 `commands.entity(entity).insert(Despawned)`。
- `server/src/npc/dormant/mod.rs:386` 附近：`NpcDormantStore` 的 dirty 注释写明变更后依赖 Redis publish；`take_dirty()` 在 `server/src/npc/dormant/mod.rs:438` 附近会读出并清掉 dirty。
- `server/src/network/mod.rs:1206` 附近：`publish_world_state_to_redis` 先发 world state；`server/src/network/mod.rs:1214` 调 `dormant_store.take_dirty()` 后才 `to_redis_hash_payloads()`；`server/src/network/mod.rs:1221` 附近忽略 `tx_outbound.send(RedisOutbound::NpcDormantHash(entries))` 的结果。
- `server/src/network/redis_bridge.rs:1762` 附近：后台命令 `tokio::spawn` 后立即返回 `Ok(())`；失败只在 `server/src/network/redis_bridge.rs:1769` 附近 `warn!`，没有回传 ECS。
- `server/src/network/redis_bridge.rs:1781` 附近：`RedisIoCommand::HashReplace { .. }` 全部走 background fire-and-forget。
- `server/src/network/redis_bridge.rs:1843` 附近：`execute_hash_replace` 明确可能因 Redis 错误或超时返回 Err，但 background 分支只记录日志。
- `server/src/npc/dormant/mod.rs:606` 附近：启动从 Redis `HGETALL bong:npc/dormant` 恢复；`server/src/npc/dormant/mod.rs:705` 附近把 Redis payload 直接塞回 store 并 rebuild index。
- `server/src/network/mod.rs` 的 `dormant_publish_skipped_when_clean` 测试已锁住 clean 周期不发 `NpcDormantHash`，所以失败后不会靠普通 publish 周期全量补写。

## 触发路径

1. dormant store 发生真实变更：脱水新增快照、hydrate 移除快照、离屏 tick 推进位置/寿命/真元、pending-release 状态变化等。
2. 下一次 `publish_world_state_to_redis` 命中 200 tick 周期，`take_dirty()` 把 dirty 清掉。
3. `NpcDormantHash` 被翻译成 Redis `HashReplace`，并走后台连接。
4. Redis `DEL/HSET/RENAME` 超时或失败，后台任务只记录 warn。
5. ECS 中的 `NpcDormantStore` 已是 clean，没有 ACK/失败事件重新置 dirty。
6. 无后续真实变更时，后续 publish 只发 world state，不再发 dormant HASH。
7. 服务器重启后 `load_dormant_store_from_redis_system` 从旧 HASH 恢复，出现 NPC 消失、复活或状态回滚。

## 反方审查记录

第一轮质疑：

- “完全不会重试”表述过强；后续真实变更会重新 dirty。
- `Sender::send` 失败通常意味着 bridge receiver 断开，作为主证据较弱。
- 序列化失败当前缺少现实触发样例，建议只列次要风险。
- 需确认没有周期性全量重发、没有 Redis 写失败 ACK、没有开放 PR 覆盖。

补证与让步：

- 标题和主轴改为“后台 Redis `HashReplace` 失败后 dirty 预清且无 ACK 补偿”。
- 补齐最小时序：dirty 变更 -> publish 清 dirty -> 后台 `HashReplace` 失败 -> clean publish 不发 HASH -> 重启 HGETALL 旧 HASH。
- `send`/序列化失败只作为次要风险，不作为立案核心。
- 玩法例子聚焦脱水新增快照丢失、hydrate/remove 后旧快照复活。

最终裁决：

- 反方结论：通过，足够作为新 skeleton plan。
- 关键理由：这不是普通 dirty gate 设计取舍，而是 dirty 在持久化确认前被消费，后台失败无 ACK/失败回传，证伪了已完成 `plan-dormant-persistence-bridge-fix-v1` 中“失败静默，靠下周期重发 + P1 dirty 保证最终一致”的假设。

## Skeleton Fix Plan

- [ ] P0：为 dormant HASH 写入建立“待确认”状态，不在 `HashReplace` 进入后台队列时永久清 dirty；至少保证失败后会重试。
- [ ] P0：选择并落地一种 ACK/失败补偿方案：
  - 方案 A：后台 Redis 写失败通过 channel 回 ECS，重新 `mark_dirty`。
  - 方案 B：`take_dirty` 改为成功确认后 clear，失败保留 dirty。
  - 方案 C：为 dormant HASH 加 per-key single-flight pending/retry 状态，避免失败丢重试且避免并发写重叠。
- [ ] P0：空 store 删除 HASH 的路径必须同样可靠；`entries.is_empty()` 触发 `DEL bong:npc/dormant` 失败后不能永久 clean。
- [ ] P1：更新 `plan-dormant-persistence-bridge-fix-v1` 相关实现假设对应的注释，明确“失败本身会驱动重试”的真实契约。
- [ ] P1：保留后台 fire-and-forget 不阻塞主 Redis outbound 的性质，不能回退到无限 pin `pending_command` 的旧问题。

## 验收测试计划

- [ ] server 单测：构造 dirty `NpcDormantStore`，模拟 `NpcDormantHash` 后台写失败，断言失败会重新 dirty 或留下 pending retry。
- [ ] server 单测：store 变空后发空 entries，模拟 Redis `DEL` 失败，断言下一 publish 仍会尝试删除旧 HASH。
- [ ] server 单测：clean 周期仍不发 dormant HASH；只有 pending/失败状态或新变更才发，避免回到每 10 秒全量重写。
- [ ] server 集成测试：脱水新增 snapshot 后模拟 Redis 写失败 + 重启恢复，断言不会丢失该 dormant NPC。
- [ ] server 集成测试：hydrate/remove snapshot 后模拟 Redis 删除失败 + 重启恢复，断言旧 snapshot 不会复活。
- [ ] 回归测试：Redis bridge `HashReplace` 仍不占用 `pending_command`，world_state/combat/chat 出站不被 dormant 写失败饿死。

## 风险

- Redis 写失败重试如果设计成 inline pending，可能复活 `plan-dormant-persistence-bridge-fix-v1` 已修过的 outbound 饥饿问题。
- 背景写入若使用固定 `{key}:tmp` 且允许并发重叠，可能出现后一轮前导 `DEL` 干扰前一轮的问题；需要 single-flight 或唯一 temp key 策略。
- 过度频繁重试会重新制造大 HASH 写入压力；需要节流、退避或 pending 合并。
- dormant snapshot 含真元状态，不能用“失败后 drop”处理；否则可能造成离线 NPC 真元账与实际世界状态长期漂移。
