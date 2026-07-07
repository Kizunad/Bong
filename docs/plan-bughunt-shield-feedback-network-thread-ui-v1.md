# plan-bughunt-shield-feedback-network-thread-ui-v1

Skeleton Plan：`shield_broken` / `shield_block_hit` 两个盾牌反馈 payload 在 `bong:server_data` raw receiver 的网络线程内直接落地音效、粒子和瞬态 HUD，需改成主线程落地。

## Bug 摘要

`BongNetworkHandler.registerServerDataChannel()` 使用 Fabric raw `ClientPlayNetworking.registerGlobalReceiver(Identifier, PlayChannelHandler)` 接收 `bong:server_data`，并在 callback 内同步执行 `ROUTER.route(...)`。Fabric API 1.3.12 sources 明确该 raw handler 运行在 network thread，读完 buffer 后访问 game state 必须通过 `client.execute(...)` 切到 render thread。

但 `ServerDataRouter` 注册的 `shield_broken` 与 `shield_block_hit` handler 在 `handler.handle(...)` 阶段就执行了生产副作用：

- `ShieldBrokenHandler` 清本地盾槽后直接 `SoundRecipePlayer.instance().play(...)`，并通过 `BongVfxParticleBridge.spawnParticle(...)` 生成破盾粒子。
- `ShieldBlockHitHandler` 直接播放格挡音效、生成格挡粒子，并写入 `ZhenmaiHudStateStore.flashShieldBlock(...)`。
- 只有 handler 返回的 `alertToast` / `visualEffectState` 会在之后进入 `client.execute(() -> applyDispatch(...))`，上述音效 / 粒子 / 盾弧 HUD 已经提前发生在网络线程。

## 实际游玩体验影响

玩家举盾成功格挡或盾牌耐久归零时，服务端会推送 `shield_block_hit` / `shield_broken`。当前客户端可能在网络线程里修改音频播放队列、访问 `MinecraftClient` / 粒子管理相关对象并写瞬态 HUD。实际游玩中表现为盾格挡或破盾的木 / 骨差异化音效、粒子、准星盾弧偶发丢失、顺序错乱，或在高延迟 / 高频战斗包下出现难复现的客户端竞态异常。此 plan 不声称已有稳定 crash log，主张的是生产路径违反 Fabric raw receiver 线程契约。

## 证据定位

- `client/src/main/java/com/bong/client/BongNetworkHandler.java:244`-`:299`：`bong:server_data` raw receiver 在 callback 内同步 `ROUTER.route(jsonPayload, readableBytes)`；只有 dispatch 携带 chat / narration / player / zone / visual / toast / ui 等字段后才 `client.execute(applyDispatch)`。
- `client/src/main/java/com/bong/client/network/ServerDataRouter.java:89`-`:90`、`:173`-`:176`、`:314`-`:315`：创建并注册 `ShieldBrokenHandler` / `ShieldBlockHitHandler`，route 时直接调用 `handler.handle(envelope)`。
- `client/src/main/java/com/bong/client/network/ShieldBrokenHandler.java:94`-`:99`、`:127`-`:165`、`:172`-`:183`：破盾 handler 在 dispatch 返回前清盾槽、播放音效、spawn 粒子并读取 `MinecraftClient.getInstance()` / player 坐标。
- `client/src/main/java/com/bong/client/network/ShieldBlockHitHandler.java:95`-`:100`、`:114`-`:160`：格挡命中 handler 在 dispatch 返回前播放音效、spawn 粒子并写 `ZhenmaiHudStateStore.flashShieldBlock(...)`。
- `client/src/main/java/com/bong/client/visual/particle/BongVfxParticleBridge.java:12`-`:18`、`:34`-`:42`：bridge 注释明确调用方需保证主线程，`MinecraftClient#particleManager` 不能在网络线程调用；实现会取 `MinecraftClient` 后调用 particle player。
- `client/src/main/java/com/bong/client/audio/SoundRecipePlayer.java:40`-`:41`、`:67`-`:88`、`:144`-`:150`：音频播放器用普通 `LinkedHashMap` / `ArrayList` 存 `loops` / `pending`，`play()` 入队，`ClientTickEvents.END_CLIENT_TICK` 再 drain，没有同步队列语义。
- `server/src/network/mod.rs:984`-`:998`：生产服务端注册 `emit_shield_broken_payloads` / `emit_shield_block_hit_payloads`。
- `server/src/combat/resolve.rs:1184`-`:1195`、`:1267`-`:1274`：盾耐久归零与成功格挡会发 `ShieldBroken` / `ShieldBlockHit`。
- 本地 Gradle cache 的 `fabric-networking-api-v1-1.3.12+13a40c6677-sources.jar` 中 `ClientPlayNetworking.registerGlobalReceiver(Identifier, PlayChannelHandler)` Javadoc：raw handler runs on the network thread；访问 game state 必须调用 `execute(Runnable)` 切 render thread。当前 client 使用 `fabric_version=0.92.3+1.20.1`。

## 触发路径

1. 玩家进入战斗并装备木盾或骨盾。
2. 成功举盾格挡一次，或盾牌耐久被打到 0。
3. server combat resolve 发 `ShieldBlockHit` 或 `ShieldBroken` event。
4. server network emit 系统通过 `bong:server_data` 推 `shield_block_hit` / `shield_broken`。
5. client raw receiver 在网络线程同步 route 到对应 handler。
6. handler 直接播放音效、生成粒子、写瞬态盾弧 HUD；toast / flash dispatch 才在之后进主线程。

## 重复避让

- 不重复 #1049 / `plan-bughunt-mineral-probe-result-network-thread-ui-v1`：该 plan 只覆盖 `mineral_probe_result`，并明确把 `ShieldBrokenHandler` / `ShieldBlockHitHandler` 列为后续审计、不纳入范围。本 plan 是同类线程根因下的独立盾牌 payload。
- 不重复 #1030 `craft_outcome` 网络线程完成反馈。
- 不重复 #1016 `cast_sync` 网络线程关闭功法配置浮窗。
- 不重复 `plan-shield-block-combat-event-feedback-v1`：该 finished plan 修的是 `combat_event.kind=shield_block` 飘字分类，不处理 `shield_block_hit` / `shield_broken` 的 network-thread 落点。
- 不重复 r10 破盾服务端状态泄漏：该项修 server `ShieldBlock` / `ShieldBlocking` 生命周期；本 plan 只处理 client feedback 线程边界。

## Adversarial 审查记录

### Round 1 反方

质疑点：

- 这可能只是 #1049 的重复，因为根因同为 `server_data` route 在网络线程执行 handler。
- 盾牌反馈偏 combat，不一定属于 client-ui 分区。
- audio / particle bridge 可能自身线程安全，未必是 bug。
- 需要证明生产可达，而不是测试孤岛。

回应：

- #1049 明确只覆盖 `mineral_probe_result`，并把盾牌 handlers 排除为后续审计。
- 本案标题和范围收窄为 client-ui/network thread boundary 影响盾牌反馈，不主张修战斗数值。
- `BongVfxParticleBridge` 自身注释要求主线程；`SoundRecipePlayer` 是普通集合 + client tick drain，不是线程安全队列。
- server emit、router 注册、raw receiver 同步 route 均闭合。

### Round 2 反方

最终结论：通过。不能推翻候选；这是 valid BugHunt candidate，不降级为 `NO_CANDIDATE`。必须收窄为 `shield_broken` / `shield_block_hit` 两个 payload，避免宣称首次发现 `server_data` route 根因；风险表述写成“违反 Fabric raw receiver 线程契约，可能导致音效 / 粒子丢失、竞态或难复现客户端异常”，不夸大为稳定 crash。

## Skeleton Fix Plan

1. 让 `ShieldBrokenHandler` / `ShieldBlockHitHandler` 只解析 payload 并产出结构化 feedback spec，不在 `handle(...)` 内触发 `SoundRecipePlayer`、`BongVfxParticleBridge`、`MinecraftClient` 或瞬态 HUD。
2. 扩展 `ServerDataDispatch`，增加盾牌 feedback 所需的本地 sound spec、particle spec、shield HUD flash spec，或抽象成通用 client feedback spec。
3. 在 `BongNetworkHandler.registerServerDataChannel()` 的主线程 `applyDispatch(...)` 路径统一落地盾牌音效、粒子、HUD 盾弧、toast / flash。
4. 保留木盾 / 骨盾差异化规格，不改 server combat resolve、不改 shield schema、不改粒子 / 音频资产。
5. 可选后续：审计其他 `server_data` handler 是否仍在 route 阶段触达 MC UI / sound / particle；本 plan 不扩大到全量重构。

## 验收测试计划

- client 单测：`ShieldBrokenHandler.handle(...)` 与 `ShieldBlockHitHandler.handle(...)` 不再调用注入的 audio / particle bridge，而是返回 dispatch spec。
- client 单测：主线程 applier 对 `shield_broken` spec 正确播放木 / 骨破盾音效、spawn 对应粒子、显示「盾已碎裂」toast / flash。
- client 单测：主线程 applier 对 `shield_block_hit` spec 正确播放木 / 骨格挡音效、spawn 对应粒子、写盾弧 HUD。
- router 线程契约测试：`ServerDataRouter.route(...)` 对这两个 payload 不产生 UI / audio / particle 副作用。
- 手动验证：JDK 17 下 `cd client && ./gradlew runClient`，进服后用木盾 / 骨盾分别触发成功格挡与破盾，确认音效、粒子、盾弧、toast 均出现，且日志无网络线程访问 UI 报错。

## 风险与边界

- 不应把所有 `ROUTER.route(...)` 粗暴包进 `client.execute` 后立即结束；部分 handler 当前只更新线程安全 store 或纯解析，统一迁移需要单独审计。
- 不要顺手改盾牌数值、耐久、server 状态机或资源规格。
- `ZhenmaiHudStateStore` 写入本身是 `AtomicReference`，不是主证据；主修复动机是 Sound / VFX / MC game state 访问必须在主线程。
