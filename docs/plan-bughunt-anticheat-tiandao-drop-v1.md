# plan-bughunt-anticheat-tiandao-drop-v1

## 一句话

`bong:anticheat` 已被 server 发布并在 agent schema 标成 `Server -> Agent` 通道，但 Tiandao runtime 没有订阅、校验、缓冲或消费该事件；玩家触发 reach/cooldown/qi_invest 异常后，事件只到 Redis 与 server log，agent 侧完全看不见。

## 高置信证据

1. server 会真实生产事件：`server/src/combat/anticheat.rs` 在阈值命中后构造 `AntiCheatViolationEvent`（`emit_anticheat_threshold_reports`），`server/src/network/anticheat_bridge.rs:15` 把它转成 `RedisOutbound::AntiCheatReport`。
2. Redis bridge 会发布到 `bong:anticheat`：`server/src/network/redis_bridge.rs:628-633` 将 `RedisOutbound::AntiCheatReport` 序列化并 publish 到 `CH_ANTICHEAT`。
3. schema 明确把该通道暴露给 agent：`agent/packages/schema/src/channels.ts:172-173` 标注 `ANTICHEAT: "bong:anticheat"`，且 `agent/packages/schema/src/anticheat.ts` 提供 `AntiCheatReportV1` 与 `validateAntiCheatReportV1Contract`。
4. Tiandao 运行时没有接收路径：`agent/packages/tiandao/src/redis-ipc.ts:156-193` 的 `CROSS_SYSTEM_EVENT_CHANNELS` 不包含 `ANTICHEAT`；`connect()` 只订阅 world/tsy/npc/alchemy/poi/rat 与 `CROSS_SYSTEM_EVENT_CHANNELS`（`redis-ipc.ts:751-761`）；`agent/packages/tiandao/src` 全量搜索未发现 `ANTICHEAT` 或 `AntiCheatReport` runtime consumer。
5. 现有测试只证明 schema 与 server publish，不证明 Tiandao 消费：server 有 `publishes_anticheat_report_on_correct_channel`，schema 有 contract pin，但 Tiandao 没有订阅/解析/回归测试。

## 实际游玩体验影响

正常玩家遇到异常攻击距离、异常攻击冷却或异常真元投入时，server 会记录并发布反作弊报告，但 Tiandao 不会获得该上下文。结果是天道不会把异常战斗纳入 world model、不会形成观察/告警/裁决策略，也不会让管理员或叙事层看到“此人反复触发异常战斗”的信号。对玩家体感就是：异常战斗行为可能已经影响了 PvP 公平性，但世界层和 agent 层仍像无事发生。

这不是纯 server/client 实现 bug；断点在 server -> agent schema/Redis 契约已经存在，而 agent runtime 没有消费。

## 去重说明

- 不重复 #1054：#1054 聚焦 NPC combat/relic schema parity，本 plan 聚焦 `bong:anticheat` Tiandao 事件无人消费。
- 不重复 #1061：#1061 聚焦 generated schema/freshness gate，本 plan 不讨论 generated 产物新鲜度，而是运行时订阅缺口。
- 不照搬旧 r03 `niche_guardian_tiandao_drop`：本 plan 独立验证的是 anticheat 通道，生产者、schema、runtime 缺口均不同。
- `docs/finished_plans/plan-anticheat-v1.md` 曾写“运维侧消费，非玩家可见”，但当前 `channels.ts` 已把 `ANTICHEAT` 纳入统一 Redis v1 channel 并标为 `Server -> Agent`。若设计仍是“非 Tiandao 消费”，则也应由 agent 侧显式登记/测试为 ignored；当前状态是声明给 agent，却没有任何订阅或 ignored guard，容易被误判为闭环。

## 两轮对抗结论

第一轮 explore 独立找到 territory narration、shield_block_hit、schema dist 风险；主线程另行发现 `bong:anticheat` 无 Tiandao 消费。

第二轮反证逐项比较：

- `TERRITORY_NARRATION_REQUEST`：server 注释和 finished plan 明确说明“留契约供未来 agent runtime”，且 server fallback 已保证玩家可见，像显式遗留，不适合作为 BugHunt 主候选。
- `shield_block_hit`：`ServerDataType` 与 `ServerDataV1` union 确有闭包漂移，但主 union 校验和实际 client feedback 大概率不坏，影响偏 tooling。
- `bong:anticheat`：server 生产、schema 声明、Tiandao 无订阅三点同时成立，且没有已有 runtime 或 ignored guard 覆盖。推荐作为本轮单一候选。

## 建议修复方向

1. 在 `RedisIpc` 或独立 `AnticheatRuntime` 中订阅 `CHANNELS.ANTICHEAT`。
2. 使用 `validateAntiCheatReportV1Contract` 校验 payload，拒绝坏包并计数/日志。
3. 明确消费语义二选一：
   - 进入 Tiandao cross-system event buffer/world model，供后续 agent tick 使用；
   - 或作为运维事件单独缓冲并发布管理员/日志 narration。
4. 补 Tiandao 测试：publish `bong:anticheat` 后能被订阅、校验、drain 或显式 ignored；测试名必须防止“channel 已在 schema 但 runtime 没接”的回归。

## 验收

- `cd agent && npm run build -w @bong/schema`
- `cd agent/packages/schema && npm test`
- `cd agent/packages/tiandao && npm test`
- 如改动 server channel 对齐，再补 `cd server && cargo test anticheat`
