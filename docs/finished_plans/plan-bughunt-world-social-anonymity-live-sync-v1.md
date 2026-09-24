# plan-bughunt-world-social-anonymity-live-sync-v1

> 一句话主题：`social_anonymity` 只在玩家进服时下发一次；后续聊天 / 交易 / 死亡把 `Anonymity.exposed_to` 真正写到了 server 权威状态与持久化，但**没有任何 live 重发链路**把新的“此人已对我暴露”同步给见证者客户端，导致 witness 在当前会话里仍看不到对方名牌，往往要重连才恢复。影响是：**玩家明明当场见证了对方发言、交易或死亡暴露，头顶名牌却继续匿名，现场追踪、复仇、临时结盟与信息确认都会卡成“服务器知道，客户端不知道”**。

> 立项动机：这条断链位于 `server/src/social/` 与 client social state 主路径，且对匿名博弈的实际手感有直接影响。它不是“season stale client”旧问题：这里卡住的是 **social exposure → anonymity live refresh** 这条独立同步链。

## 阶段总览

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | social 暴露后匿名可见性不 live 刷新 | ✅ 2026-07-26 |

验收日期：P0 验收 2026-07-26（当前升 active：2026-07-06）。

## P0 — social 暴露后匿名可见性不 live 刷新

- **现象**：`server/src/social/mod.rs:323-341` 的 `emit_anonymity_payloads_for_joined_clients` 只对 `Added<Anonymity>` 的新入场 client 组装并发送 `ServerDataPayloadV1::SocialAnonymity`；`build_remote_identity_payloads`（`:345-379`）确实会按 `anonymity.is_exposed_to(viewer)` 计算可见性，但这条计算只发生在 join/hydrate 当下。
- **状态已变、同步没变**：`apply_social_exposures`（`server/src/social/mod.rs:508-600`）在聊天 / 死亡 / 交易后会把 actor 的 `anonymity.expose_to(witnesses)` 写进 ECS 和 SQLite；但它后半段只广播 `ServerDataPayloadV1::SocialExposure`，没有重建也没有补发新的 `SocialAnonymity` 给 actor 或 witnesses。
- **client 侧为何会卡住**：`client/.../SocialServerDataHandler.java:40-58` 只有 `handleAnonymity` 会调用 `SocialStateStore.replaceAnonymity(...)`；而 `handleExposure`（`:61-87`）只 `recordExposure(...)` + 发一条 social HUD 事件，不会改名牌可见性缓存。`SocialStateStore.shouldShowRemoteNameTag(...)`（`client/.../SocialStateStore.java:46-58`）因此继续读旧 snapshot。
- **可观察后果**：`client/.../mixin/MixinEntityRenderer.java:20-36` 在渲染远端玩家名牌时完全依赖 `SocialStateStore.shouldShowRemoteNameTag(...)`。所以 A 在 B 面前发言 / 与 B 交易 / 当场死亡后，B 虽然收到了 `social_exposure` 事件，但**这一局里 A 的头顶名牌仍会继续被 cancel 掉**。
- **为什么这是 bug，不是设计**：`docs/finished_plans/plan-social-v1.md:291-304` 明确把“server 下发 `AnonymityPayload` 给每个 client，只含对本人可见的远端玩家子集 → client 据此显示/隐藏 name tag”和“暴露管道”写成同一条 live 闭环。当前实现只落了“server 权威状态更新 + exposure 提示”，漏了“visibility live refresh”。
- **对实际游玩体验的影响**：匿名博弈里最关键的“我刚刚看见你是谁”没有即时落到画面上。玩家会遇到：对面刚在附近说话或完成交易，HUD 已提示暴露，但战斗/追踪现场仍是一堆无名人影；死亡见证者知道有人暴露，却不能立刻从人群里确认是谁。对 PvP 追击、报复、现场结盟、灵龛线索核对都是真实手感损伤。
- **建议修复范围 / 模块**：优先收口 `server/src/social/mod.rs` 与 client social state 路径。主修方向应是：暴露事件落地后，server 立刻向 actor+witnesses 定向补发新的 `SocialAnonymity` snapshot；client 端可选加一层 pin，防止未来再出现“只收 exposure、不刷新 anonymity cache”的回归。
- **验收抓手**：至少补 4 组回归。1) chat exposure 后 witness 无需重连即可看到 actor 名牌。2) trade / death 两条暴露链同样 live 生效。3) 非见证者仍保持匿名，不出现过度暴露。4) server/client pin 测要能明确区分 `social_exposure` 与 `social_anonymity` 的职责，防止未来再次只发前者。

### 交付物

1. **`server/src/social/mod.rs`**：验证 `apply_social_exposures` 的实际权威写入和 payload 发射路径；若 bug 属实，在暴露落地后向 actor 与 witnesses 定向补发新的 `ServerDataPayloadV1::SocialAnonymity` snapshot，复用既有 `build_remote_identity_payloads` 可见性口径，避免另写一套匿名判定。
2. **`server/src/social/mod.rs` 测试**：新增或扩展 server 回归，锁住 chat / trade / death 任一 social exposure 触发后 witness 同 tick 收到新的 `SocialAnonymity`，同时非 witness 不收到过度暴露。
3. **`client/src/main/java/com/bong/client/social/` 测试或 pin**：确认 `SocialExposure` 仍只承担 HUD/事件记录职责，名牌显隐继续由 `SocialAnonymity` snapshot 更新；若现有 client 测试框架不足，至少用 server 端 payload 序列测试锁住跨端契约。

### 验收标准

- server social 相关单测通过，并能明确断言 `SocialExposure` 与 `SocialAnonymity` 都发出且职责不同。
- 涉及 client Java 改动时，使用 JDK 17 跑 `cd client && ./gradlew test`；若未改 client，可不跨栈运行。
- 最终由无上下文只读 validator 复核：bug 属实性、修复最小性、非见证者不过度暴露、测试覆盖是否足够。

## 反方裁决摘要

1. **Round 1 怀疑**：也许 `social_exposure` 本来就承担 live 解匿名，client 可从 exposure 事件自行推出名牌显示。人工复核 `SocialServerDataHandler.java:61-87` 后排除：该路径只记事件流，不写 `SocialStateStore.anonymity`。
2. **Round 2 怀疑**：也许 server 在别处会按 tick 或按 exposure 后自动重发 `SocialAnonymity`。全仓收敛 `ServerDataPayloadV1::SocialAnonymity` 构造点后，只剩 `emit_anonymity_payloads_for_joined_clients` 一处，且 query gate 明确是 `Added<Anonymity>`；`apply_social_exposures` 自身无任何补发逻辑，这条反证失效。
3. **人工终裁**：这不是“状态没写进去”，而是典型“server 状态更新成功，但 live client cache 永不刷新”。由于名牌渲染 mixin 直接吃这份 cache，故玩家可见影响成立，候选保留。

## 开放问题

1. 当前 `build_remote_identity_payloads` 暴露后给 client 的 `display_name` 仍取 `lifecycle.character_id`，不是 identity display name；修 live refresh 时要不要顺手把“露出的名字”语义一起订正，需要 fix PR 决策。
2. 除名牌外，是否还有其他 player-facing UI 也应该在 exposure 后立即消费同一份 live anonymity 刷新，可在 fix PR 一并做 grep 复核。

## 审计来源

bughunt 线程 AM，范围限定 `server/src/world/`、`server/src/social/`、`client/src/main/java/com/bong/client/social/`、`client/src/main/java/com/bong/client/state/` 与相邻网络接线。原 bughunt PR 只提交 skeleton；本 active plan 负责验证候选并完成最小正确修复或提交不属实结论。

## 验证结论（2026-07-26 整理审计追认）

commit 43771a45c（2026-07-07「修复 social 暴露后匿名 live 刷新断链」，含 1105b5c4a）修复了本 bug：`apply_social_exposures` 在暴露落地后补发 `ServerDataPayloadV1::SocialAnonymity`，`server/src/social/mod.rs:3975-4014` 的双测试断言了 chat/trade/death 暴露链路的 live 刷新在同 tick 生效，而非见证者不会过度暴露。

## Finish Evidence

- **落地清单**：`server/src/social/mod.rs`（`apply_social_exposures` 补发 `SocialAnonymity`）
- **关键 commit**：43771a45c（2026-07-07，「修复 social 暴露后匿名 live 刷新断链」），含 1105b5c4a
- **测试结果**：`server/src/social/mod.rs:3975-4014` 双测试断言 live refresh；2026-07-26 审计为只读核验（Read+grep+git log 对拍 origin/main），未重跑测试套件
- **跨仓库核验**：server `apply_social_exposures`/`ServerDataPayloadV1::SocialAnonymity`；client `SocialServerDataHandler.handleAnonymity`/`SocialStateStore` 消费路径未改动，契约不变
- **遗留 / 后续**：开放问题 #1（`display_name` 语义是否订正为 identity display name）与 #2（其他 player-facing UI 是否也应消费同一份 live anonymity 刷新）未在本次修复中收口，留待后续 PR
