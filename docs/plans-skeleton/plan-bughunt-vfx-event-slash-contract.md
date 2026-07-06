# BugHunt: vfx_event slash event_id 契约漂移

## Bug 摘要

`bong:vfx_event` 的 `spawn_particle.event_id` 合约在三栈之间漂移：TS schema / client envelope 只接受 `namespace:path_with_underscores`，但 server 与 client VFX registry 已使用 `bong:vfx/...` 这类带 `/` 的 event_id。结果是服务端已发出的粒子包在客户端 `VfxEventEnvelope` 解析阶段直接 parse error，根本到不了 `VfxRegistry`。

主体非重复证据是拟态灰烬蛛暴起：`server/src/npc/brain_spider.rs:37` 使用 `bong:vfx/spider_ambush`，`client/src/main/java/com/bong/client/visual/particle/SpiderAmbushVfxPlayer.java:25` 也注册同一 slash ID，但 `client/src/main/java/com/bong/client/network/VfxEventEnvelope.java:54` 的正则会拒绝它。

## 实际游玩体验影响

玩家遭遇拟态灰烬蛛时，服务端从伪装态切到暴起态并发送音效/粒子，但客户端会丢掉 `bong:vfx/spider_ambush` 粒子。实际体验是地面伪装突然变成蜘蛛并追击，却缺少 400ms 径向灰烬 burst，玩家更难从远处或混战中辨认“这是暴起起手”，战斗读招反馈不完整。

融合兽也受同一合约漂移影响：`bong:vfx/hybrid_formation` / `bong:vfx/hybrid_rage` 已有客户端注册，但在 parse 阶段被 slash 正则拦下，融合仪式汇聚与低血量狂暴粒子仍会消失。该部分只作为补充证据；r5 旧记录是“client 零注册”，当前问题是“注册后仍被 envelope 拦截”。

## 证据定位

- TS schema 禁 slash：`agent/packages/schema/src/vfx-event.ts:36` `ANIM_ID_PATTERN = "^[a-z0-9_]+:[a-z0-9_]+$"`，`agent/packages/schema/src/vfx-event.ts:141` 用于 `spawn_particle.event_id`。
- client envelope 禁 slash：`client/src/main/java/com/bong/client/network/VfxEventEnvelope.java:54` 同正则；`client/src/main/java/com/bong/client/network/VfxEventEnvelope.java:197` 解析 `event_id`；`client/src/main/java/com/bong/client/network/VfxEventEnvelope.java:381` 注释承认 `Identifier` 允许 `/`，但先按 schema 正则过滤。
- client vfx_event 入口无旁路：`client/src/main/java/com/bong/client/BongNetworkHandler.java:321` 注册 `bong:vfx_event`，`:333` 调 `VFX_ROUTER.route()`；parse error 在 `:334-336` 直接返回。
- 服务端发送 slash ID：`server/src/npc/brain_spider.rs:37` 常量 `bong:vfx/spider_ambush`，`:221-229` 发送 `spawn_particle`；`server/src/fauna/experience.rs:156-175` 构造 `VfxEventPayloadV1::SpawnParticle`。
- client 已注册同 slash ID：`client/src/main/java/com/bong/client/visual/particle/SpiderAmbushVfxPlayer.java:25`，`client/src/main/java/com/bong/client/visual/particle/VfxBootstrap.java:140-141`。
- hybrid 补充证据：`server/src/fauna/hybrid_beast.rs:477-486`、`:800-824` 发送 slash ID；`client/src/main/java/com/bong/client/visual/particle/NpcParticleVfxPlayer.java:18-19` 和 `VfxBootstrap.java:150-153` 已注册。

## 触发路径

1. 拟态灰烬蛛处于 Disguised，`SpiderAmbushScorer` 判定玩家真元超过阈值。
2. `SpiderAmbushAction` 进入 Requested，切换 `SpiderDisguiseState::Ambush`，发送 `VfxEventRequest`，event_id 为 `bong:vfx/spider_ambush`。
3. `emit_vfx_event_payloads` 将 payload 序列化为 JSON 并通过 `bong:vfx_event` 发给附近客户端。
4. 客户端 `BongNetworkHandler` 调 `VfxEventRouter.route()`。
5. `VfxEventEnvelope.parse()` 对 `event_id` 套 `^[a-z0-9_]+:[a-z0-9_]+$`，slash ID 解析失败。
6. router 返回 parse error，`BongVfxParticleBridge` 与 `VfxRegistry` 永远不会收到该粒子。

## 反方审查记录

第一轮反方结论：通过候选。确认服务端确实走 `bong:vfx_event` JSON 通道，蛛暴起和融合兽都发 `SpawnParticle.event_id`，客户端无专用旁路，现有测试只覆盖 `bong:sword_qi_slash` / `bong:lingqi_ripple` 这类无 slash ID。开放 PR 中未发现 `VfxEventEnvelope` / slash / `spider_ambush` / `hybrid_formation` 同类修复。

第二轮反方结论：仍通过，但收窄定性。更准确的 bug 名称是“`bong:vfx_event` slash event_id 违反 schema/envelope 契约”，不是单点“client parser 太严格”。修复必须统一 server/client/schema 三栈，不能只改 Java parser。

## Skeleton Fix Plan

1. 先定合约方向：
   - 保守方案：维持 schema 禁 slash，将 `bong:vfx/spider_ambush`、`bong:vfx/hybrid_formation`、`bong:vfx/hybrid_rage` 迁移为无 slash ID，并同步 server 常量、client `Identifier`、VfxBootstrap 注册、测试与文档。
   - 放宽方案：允许 MC Identifier path 中的 `/`，同步更新 TS schema、client `VfxEventEnvelope`、server/schema 样例与跨端测试。
2. 对选定方案补跨端 pin 测试：server serialization sample、client `VfxEventEnvelopeTest`、`VfxEventRouterTest`、VfxRegistry lookup。
3. 加回归用例覆盖蛛暴起 slash/renamed event_id，证明 `route()` 成功进入 particle bridge，不再 parse error。
4. hybrid 只做同类回归覆盖，不重复 r5 的“零注册”修复主题。

## 验收测试计划

- client：在 `client/` 下按 JDK 17 约定跑 `./gradlew test build`。
- targeted client tests：`VfxEventEnvelopeTest` 新增带目录路径或迁移后 ID 的 positive case；`VfxEventRouterTest` 证明 payload 能分发到 particle bridge。
- server：若改 server event_id，跑相关 Rust 单测，至少覆盖 `brain_spider` VFX pin、`hybrid_beast` VFX emit、`vfx_event` serialization。
- 手动/联调：触发拟态灰烬蛛暴起，确认能同时看到伪装解除、音效、灰烬径向 burst；触发融合兽形成/半血狂暴，确认粒子可见。

## 风险

- 若选择放宽 slash，必须同步 TS schema、client parser 与样例，避免 agent/schema 仍判 payload 非法。
- 若选择 rename，必须保留 server/client 字面值逐字符一致，避免再次出现注册表与发包 ID 漂移。
- VFX event_id 可能已被历史文档引用；改名时要检查 finished plan 注释与测试描述，但不要回写 `docs/library/`。
