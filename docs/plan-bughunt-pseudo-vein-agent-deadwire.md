# plan-bughunt-pseudo-vein-agent-deadwire

## 摘要

伪灵脉生产 runtime 会在玩家高密度/高灵气消耗后生成灵潮窗口，并在阶段切换时通过 `PendingGameplayNarrations.push_zone` 给玩家本地提示；但为 agent-schema 侧预留的 `bong:pseudo_vein:active` / `bong:pseudo_vein:dissipate` 链路没有真实生产发送，也没有 Tiandao runtime 消费。结果是 `@bong/schema`、RedisBridge outbound arm、Tiandao 叙事模板和测试都存在，真实游玩中却永远不会进入 agent 的叙事/世界模型闭环。

## 代码证据

- `server/src/world/pseudo_vein_runtime.rs` 的 `pseudo_vein_runtime_tick_system` 在 `runtime.advance()` 后只做三件事：结算真元、发送 VFX、阶段变化时 `PendingGameplayNarrations.push_zone`。没有向 `RedisBridgeResource.tx_outbound` 发送 `RedisOutbound::PseudoVeinSnapshot` 或 `RedisOutbound::PseudoVeinDissipate`。
- `server/src/network/redis_bridge.rs` 已声明 `RedisOutbound::PseudoVeinSnapshot(PseudoVeinSnapshotV1)` 与 `RedisOutbound::PseudoVeinDissipate(PseudoVeinDissipateEventV1)`，并分别序列化到 `CH_PSEUDO_VEIN_ACTIVE` / `CH_PSEUDO_VEIN_DISSIPATE`，但生产 `grep` 没有发现对应 `tx_outbound.send(...)`。
- `agent/packages/tiandao/src/redis-ipc.ts` 把 `PSEUDO_VEIN_ACTIVE` / `PSEUDO_VEIN_DISSIPATE` 放入 `CROSS_SYSTEM_EVENT_CHANNELS` 后只进入 `latestCrossSystemEvents` 缓冲；`RuntimeRedis` 没有 drain 接口，`runtime.ts` 也不读取该缓冲。
- `agent/packages/tiandao/src/narration/templates.ts` 已有 `renderPseudoVeinSnapshotNarration` 与 `renderPseudoVeinDissipateNarration`，但除 `agent/packages/tiandao/tests/pseudo-vein-narration.test.ts` 外没有生产调用。

## 实际游玩体验影响

玩家仍能看到 server 兜底的两条本地提示，但 Tiandao 不会“感知”伪灵脉活跃/耗散：不会把灵潮诱导、警戒、耗散写入 agent 叙事流或世界模型上下文。多人围绕伪灵脉抢固元窗口时，天道层表现为沉默，后续基于 agent 事件的生态反馈、世界记忆、跨系统叙事都缺失。

## 非重复性

已检查近轮相关 PR：

- #1000 是 heartbeat 伪灵脉守恒缺口。
- #899 是伪灵脉 runtime zone 重启丢失。
- #741 是伪灵脉快照契约校验。
- #732/#733 是风暴锚点/持续时间校验。

这些都不是 `agent/packages/schema` / `agent/packages/tiandao` 的伪灵脉 Redis 事件无人消费和生产发送断链；也不重复 #1054、#1061、#1075、#1081、#1093。

## 修复建议

1. 在伪灵脉 Active/Warning/Dissipating/Dissipate 边沿构造 `PseudoVeinSnapshotV1` / `PseudoVeinDissipateEventV1` 并发送到 RedisBridge。
2. 在 Tiandao 侧为 `PSEUDO_VEIN_ACTIVE` / `PSEUDO_VEIN_DISSIPATE` 增加专用 validator + drain/runtime，调用现有 `renderPseudoVeinSnapshotNarration` / `renderPseudoVeinDissipateNarration`。
3. 增加 schema pin、Tiandao runtime 消费测试，以及 server 生产发送测试，防止只保留契约和模板的假闭环。

## 对抗结论

两轮对抗审查后放弃了 `territory_narration_request`（server 明确有 `push_zone` 真闭环且注释说明 agent runtime 暂缺）和炼丹 `session_start/intervention_result`（接近 #1023 旧主题）。伪灵脉候选更符合 agent-schema 分区：server 生产玩法存在、schema/Redis/Tiandao 模板存在，但 agent 消费闭环断开。
