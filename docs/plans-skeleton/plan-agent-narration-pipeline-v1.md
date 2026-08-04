# plan-agent-narration-pipeline-v1（骨架）

> **骨架（草案）**。一句话主题：把 Tiandao 的 narration 从“各 runtime 各自 drain、各自改状态、各自 Redis publish”收口为一条可去重、可校验、可路由、可记录 publisher-side resolve，且支持 pending store 写入后的内部恢复/重试的 agent 发布事务管道；处理 cluster 3 排除已 falsified #1551 后的 29 个 confirmed source，并把与 bounded concurrency 交付物同根的跨簇 #1702 纳入本 plan，共 30 个 confirmed source，其中 4 个是 #1470 的重复证据，另有 5 个跨簇 follow-up 不计入本 plan 账本。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|---|---|---|---|
| P0 | 事件信封、owner、source issue 吸收清单与迁移边界 | plan_skeleton | ⬜ |
| P1 | ingest：bounded pending 摄入、写入后的内部恢复 | fix_pr | ⬜ |
| P2 | dedupe：稳定幂等键、顺序/并发控制、提交前不推进游标 | fix_pr | ⬜ |
| P3 | validate：source payload → render → NarrationV1 output | fix_pr | ⬜ |
| P4 | route：现有 scope/target 规范化与匿名化 | fix_pr | ⬜ |
| P5 | publish confirm：失败边界、确认后状态提交、指标/回归门禁 | fix_pr | ⬜ |

## 接入面

- **进料**：现有 Tiandao Redis channel、`RuntimeRedis` drain API、各 narration runtime 的事件 payload、`WorldStateV1`/server tick、schema validator。
- **出料**：唯一 `AGENT_NARRATE` 发布出口；现有 `NarrationV1`/server `narration_selector` 只支持 scope/target，dimension-aware 与 SpiritNiche recipient authorization 因 wire/authority 数据缺口不在本 plan P 阶段锁内，见 §8。
- **共享契约**：复用现有 NarrationV1、`AGENT_NARRATE`、`RuntimeRedis`、server 已有 target 解析规则；新增共享 pipeline envelope/idempotency/publish 状态语义前先核实现有类型，禁止为每个 runtime 再造一套。
- **跨仓库边界**：本 plan 主改 agent；现有 server contract 无 dimension 字段、SpiritNiche authorization 数据或 consumed receipt producer，故本 plan 只按现有 scope/target selector 发布，并把三项跨仓 contract 扩展保留在 §8 开放问题。
- **世界观锚点**：本 plan 只处理天道叙事的可靠投递与作用域隐私，不新增世界观名词、境界、经济或真元物理。
- **真元边界**：本 plan 不移动、不生成、不衰减真元；任何涉及 qi 的 narration 仅传递已存在事件字段，不改变 `qi_physics` ledger。

## 0. 立项依据与职责

### 0.1 来源与实现 owner

- **来源**：2026-08-03 flash-review cluster 3；合并汇总将其归类为“agent 发布/叙事管道”。triage 原始 source ledger 有 30 个 issue ID；其中 #1551 已由 shard-2 判定为 falsified，故其余 29 个为 cluster 3 confirmed source；这 29 个中 #1475/#1509/#1527/#1538 是 #1470 的重复证据，并非 4 个独立根因。跨簇 #1702 与本 plan 的 bounded concurrency 交付物完全重合，故一并纳入，共 30 个 confirmed source。其余 5 个真实但属于其他 cluster 的编号见 §4.4，不计入本 plan 账本。
- **唯一实现 owner**：`agent`（Tiandao runtime / schema / Redis IPC）；不归入 R1-R10 server/client 重构轨道。
- **实现形态**：先作为共享 agent 基础设施 plan，再由各 feature narration runtime 逐个迁移；不是把 30 个 issue 分成 30 个互相复制的修复出口。
- **流程前置已核对（2026-08-04）**：本 skeleton 创建提交树内的根 `CLAUDE.md`「Plan 工作流」与 `docs/CLAUDE.md` §§五-六 均存在且已读取；头部接入面与本文 §10 按其强制结构补齐。

### 0.2 目标接入面

- **进料**：现有 Tiandao Redis channel、`RuntimeRedis` drain API、各 narration runtime 的事件 payload、`WorldStateV1`/server tick、schema validator。
- **出料**：唯一 `AGENT_NARRATE` 发布出口；现有 `NarrationV1`/server `narration_selector` 只支持 scope/target，dimension-aware 与 SpiritNiche recipient authorization 因 wire/authority 数据缺口不在本 plan P 阶段锁内，见 §8。
- **共享契约**：复用现有 NarrationV1、`AGENT_NARRATE`、`RuntimeRedis`、server 已有 target 解析规则；新增共享 pipeline envelope/idempotency/publish 状态语义前先核实现有类型，禁止为每个 runtime 再造一套。
- **跨仓库边界**：本 plan 主改 agent；现有 server contract 无 dimension 字段、SpiritNiche authorization 数据或 consumed receipt producer，故本 plan 只按现有 scope/target selector 发布，并把三项跨仓 contract 扩展保留在 §8 开放问题。
- **世界观锚点**：本 plan 只处理天道叙事的可靠投递与作用域隐私，不新增世界观名词、境界、经济或真元物理。
- **真元边界**：本 plan 不移动、不生成、不衰减真元；任何涉及 qi 的 narration 仅传递已存在事件字段，不改变 `qi_physics` ledger。

## 1. Goals

1. 让所有 narration 事件都经过统一的 `ingest → dedupe → validate → route → publish confirm` 管道，不再允许 runtime 直接 `publish` 绕过保护边界。
2. 在 Redis publish 失败、LLM 解析失败、同 tick 重试和并发到达时，明确已写入 pending 的事件的恢复/重试边界，避免静默丢失、无界重试或乱序污染后续状态；普通 Redis Pub/Sub 的 ingress 是 at-most-once，pending 写入前崩溃以及 publisher resolve 后的 subscriber 投递均不在本 plan 的端到端保证内。
3. 将现有 wire 可表达的 scope、target 和文本合约变成发布前可验证的边界；dimension 与 SpiritNiche recipient authorization 作为明确 contract gap 留待 §8 决策，不在本 plan P 阶段伪锁定。
4. 把“状态已推进/冷却已记录/队列已删除”与“publisher 已完成 best-effort resolve”绑定；其中队列删除只是本地 terminal bookkeeping，可能在零 subscriber 或 resolve 后 crash 场景下发生，因此不代表消息已送达或可见副作用只发生一次。未来若引入带 receipt/dedupe 的 server contract，再扩展可选 `consumed`。
5. 为现有和未来 narration runtime 提供单一接入点，减少 feature plan 只写 renderer/模板却没有生产消费或回流确认的孤岛。

## 2. Non-goals

- 不在本 plan 内补齐每一个 feature event 的业务 renderer、世界观文案或新的叙事内容；已有 `poi novice`、`anqi charged` 等 feature skeleton 仍由各自 owner 实现，只需迁移到共享管道。
- 不修改 server gameplay、战斗、修炼、qi ledger、NPC 行为、worldgen 或 client HUD/VFX；它们只是 narration 的生产端或消费端，不是本 plan 的实现域。
- 不追求跨 Redis/进程的数学意义“exactly once”幻觉；普通 Redis Pub/Sub 没有 subscriber delivery ack、replay 或端到端 dedupe，本 plan 只把 ingress 视为 at-most-once，并对 pending store 已成功写入的事件提供内部恢复/重试语义。Redis `PUBLISH` resolve 只表示 Redis client-side 调用完成，不表示消费者收到；若要覆盖 pending 写入前 crash、消费者确认或单一可见副作用，必须另行迁移到 Redis Streams/consumer group 或等价 durable claim/ack transport，并补齐 wire-level correlation/receipt/dedupe contract，见 §8。
- 不以兼容层长期保留旧的每 runtime 直发路径；迁移完成后旧出口应删除或在集中注册表中 fail-fast，而不是双发兜底。
- 不把失败吞掉后仅写日志视为可靠性；日志、指标和 bounded dead-letter/pending 语义必须能支持有限重放、明确丢弃或人工处置。
- 不将未经授权的 broadcast 作为“路由失败时的安全默认值”；隐藏/玩家/zone 事件必须 fail-closed。

## 3. Current-state findings

### 3.1 发布失败边界（P1/P5）

五个 issue 指向同一根因：各 runtime 在 `void` 消息 handler 中启动异步处理，Redis `publish` 位于 LLM `try/catch` 外，拒绝会逃逸且没有统一 pending/ack 状态。

| Issue | 具体证据 | 管道含义 |
|---:|---|---|
| #1470 | `agent/packages/tiandao/src/dugu_v2_runtime.ts:171` 的 publish rejection 逃出 LLM `try/catch`。 | publish 必须进入统一 failure boundary。 |
| #1475 | `mutation-narration-runtime.ts:152` 重复同样的 uncaught publish 形状。 | 不能按 runtime 逐个打补丁。 |
| #1509 | `woliu_v2_runtime.ts:183` 重复 uncaught Redis publish。 | 共享出口必须覆盖所有 runtime。 |
| #1527 | `poison-trait-runtime.ts:139` 重复同样的失败边界。 | publish error 不能只靠调用方自觉 catch。 |
| #1538 | `breakthrough-cinematic-narration.ts:229` 重复同样的 Redis publish failure。 | 视觉/突破叙事也必须走同一确认语义。 |

### 3.2 摄入、去重、顺序与状态提交（P1/P2）

这些 issue 表明当前事件可能在处理前被移除、同一实体事件并发完成、或业务状态在 publish 成功前先推进。

| Issue | 具体证据 | 管道含义 |
|---:|---|---|
| #1481 | `neg-domain-escape.ts:68-71` 只门控 avoidance counter，`buildLostLockNarration` 却对每个 pending attack 推送。 | dedupe key 必须覆盖最终 narration，而不是只覆盖局部计数器。 |
| #1486 | `anqi-narration.ts:175-177` 对每条消息 `void handlePayload(...)`，无队列/并发限制。 | ingest 后必须有有界队列和 per-key 顺序。 |
| #1495 | `locust-swarm-narration.ts:64-68` 在 downstream publish 成功前记录 cooldown。 | cooldown/state 只能在 confirm 后提交。 |
| #1510 | `runtime.ts:1298-1331` 先 drain NPC deaths，再做 stale-state short-circuit；注入只发生在非 stale 分支。 | stale/拒绝路径也要有明确 ack 或 requeue，不能 drain 即丢。 |
| #1518 | `redis-ipc.ts:1013` 在 LLM 处理前原子 trim chat；运行时错误没有 requeue。 | ingest 必须先保留 pending，再在成功/明确拒绝后确认消费。 |
| #1567 | `neg-domain-escape.ts:104-105` 退出时发 relock narration，却不消费/重置 pending tribulation command。 | narration 与 command 的状态提交需要显式关联，避免重复或悬挂。 |
| #1581 | `named-faction-narration.ts:133-137` 在 narration publish side effect 前推进 `lastStatusByFaction`。 | 业务 cursor/state 只能在 publish confirm 后推进。 |
| #1619 | `void_erosion_runtime.ts::onMessage` 并发启动 `handlePayload`，同一实体 phase narration 可乱序。 | 至少按 entity/event stream key 串行化。 |

**Falsified regression note（不计入 confirmed source，也不进入 P 阶段锁）**：#1551 曾被归入 cooldown cluster，但 shard-2 已判定为 falsified；`ecology-analyzer.ts:251-259` 在 cooldown 分支返回 false，且只在 accepted path 更新 `lastNarrationTickByKey`，原 issue 所称的 cooldown 自刷新前提不成立。后续可保留一条回归测试，证明 rejected cooldown path 不会刷新 accepted tick；不能据此增加生产修复范围。

### 3.3 合约校验与输入边界（P3）

| Issue | 具体证据 | 管道含义 |
|---:|---|---|
| #1530 | `void_erosion_runtime.ts:61-66` 接受任意 style 字符串和无界文本。 | 所有 narration 在 publish 前必须经过 NarrationV1 + bounded content validation。 |
| #1616 | `void_erosion_runtime.ts::parseNarration` 未做 NarrationV1 contract validation 就 publish。 | renderer 返回对象不能被视为已校验对象。 |
| #1654 | `schema/src/social.ts` 有 intrusion validator，却没有同 channel 的 NicheGuardianFatigueV1/BrokenV1 validator。 | validator 覆盖要按 channel/payload 注册表审计，不能只覆盖一个变体。 |
| #1673 | `query-player.ts` 检查 `player_join`，但 server recent_events 没有该 producer。 | ingest/validate 需要 event producer 与 consumer 的契约对拍，死事件名应拒绝或修正来源。 |
| #1674 | `query-player.ts` 无 maxItems/limit 地映射全部 player.social.relationships。 | 工具/LLM 输入必须有结果边界，避免单条事件膨胀 pipeline。 |

### 3.4 作用域、目标与隐私路由（P4）

| Issue | 具体证据 | 管道含义 |
|---:|---|---|
| #1525 | `narration-eval.ts:200` 先移除 exposed names，再检查 unexposed names，重叠姓名可逃过禁用检查。 | 匿名化/隐私校验必须先于 route，并按最长/禁止集合确定性处理。 |
| #1528 | `meridian-severed-narration.ts:79-81` 给 player target 加 `meridian_severed:` 前缀，非 server 认可的 player target。 | route 阶段必须规范化 target，禁止 renderer 自造 wire 格式。 |
| #1532 | `zhenfa-v2-runtime.ts:41-46` event 没 zone 时默认 broadcast，包括隐藏 zhenfa deployment。 | 缺目标不能自动升级为 broadcast，必须 fail-closed 或显式 policy。 |
| #1587 | `mutation-narration-runtime.ts:57-60` 对 player/zone 都使用 `mutation:<entity_id>`。 | scope 与 target 的格式需由集中 route policy 生成。 |
| #1590 | `halfstep-rechallenge-narration.ts:167-169` 直接转发 raw `payload.char_id`，不是 normalized player target。 | player target 必须以 canonical identity/格式生成。 |
| #1607 | `query-zone-history.ts:123-127` 将 bounded `localDelta` 与 full-history trend delta 并列展示。 | route/context 输入必须携带同一时间窗口语义，避免 narration 误读状态。 |
| #1738 | `scattered-cultivator-narration.ts::renderNpcIntrusionNarration` 广播 SpiritNiche 坐标，违反 revealed/revealed_by 隐私。 | 当前输入与 selector 无法执行该策略；作为 §8.9 的 authorization producer/schema/enforcing-consumer contract gap 保持 open。 |

### 3.5 发布幂等、游标与重启（P2/P5）

| Issue | 具体证据 | 管道含义 |
|---:|---|---|
| #1616 | 同时属于 contract validation：`parseNarration` 可绕过统一校验直接 publish。 | confirm 必须以“验证通过的 envelope”作为前置。 |
| #1619 | 同时属于 event serialization：同实体 phase 事件可并发乱序。 | idempotency key 不能替代 ordering key，二者都要有。 |
| #1654 | 同时暴露 schema validator 覆盖缺口。 | validator registry 要有完整性检查。 |
| #1659 | `economy-analyzer.ts::canNarrate` 用重启后 server tick 对比陈旧 `lastNarrationTick`，导致 narration 被错误抑制。 | 幂等/冷却 key 必须包含 tick epoch 或 monotonic event identity，不能裸比跨进程 tick。 |
| #1673 | consumer 检查 server 没有 producer 的 `player_join`。 | event identity/producer registry 必须可审计。 |
| #1674 | 无界 social relationships 结果。 | envelope/context 必须有大小上限。 |
| #1675 | `runtime.ts` 只有 command 和 narration 都 publish 成功才推进 `lastProcessedStateCursor`。 | 需要定义每个 side effect 的 ack 关系，避免一项成功、一项失败时整批重复。 |
| #1679 | publish 失败后以新 batch id 重发 command，cursor 不变。 | 重试必须复用 stable idempotency key，不得每次生成新 batch id。 |
| #1738 | narration recipient scope 泄露隐私。 | 当前幂等/route 无 authorization 输入或 enforcing consumer；不得在本 plan 内伪造 recipient key，等待 §8.9 完整契约。 |
| #1746 | `world-model.ts::cloneZoneSnapshot` 丢失 context/persistence 使用的 `ZoneSnapshot.status`。 | snapshot clone 是 pipeline 输入的一部分，字段丢失必须在 ingress/context contract pin。 |

> 注：#1616、#1619、#1738 等可同时落入多个调查分类；但 #1738 因缺 authorization 数据和 enforcing consumer，不进入本 plan P 阶段锁，只保留为 §8 contract gap。正式账本为 cluster 3 排除 falsified #1551 后的 29 个 confirmed source，加上由 bounded concurrency 交付物直接吸收的跨簇 #1702，共 30 个 confirmed source；其中 #1475/#1509/#1527/#1538 是 #1470 的重复证据。其余 5 个跨簇 follow-up 见 §4.4，不计入本 plan source ledger。

## 4. Source issue 清单与唯一聚类

### 4.1 publish-error-boundary（5 个 source ID；其中 4 个为 #1470 重复证据）

`#1470 #1475 #1509 #1527 #1538`

### 4.2 narration-routing-and-delivery（15 个 confirmed source ID）

`#1481 #1486 #1495 #1510 #1518 #1525 #1528 #1530 #1532 #1567 #1581 #1587 #1590 #1607 #1702`

`#1551` 不在此清单：shard-2 判定其为 falsified，仅保留在 §3.2 的 regression note。

### 4.3 agent-publish-idempotency（10 个 confirmed source ID）

`#1616 #1619 #1654 #1659 #1673 #1674 #1675 #1679 #1738 #1746`

**正式 source ledger 计数：cluster 3 有 30 个原始 issue ID，其中 #1551 已 falsified，余 29 个 confirmed source ID；另纳入与 bounded concurrency 交付物直接重合的跨簇 #1702，因此本 plan 共 30 个 confirmed source ID。30 个中 4 个是 #1470 的重复证据，26 个是非重复 confirmed source。** 不计入本 plan 的其余 5 个跨簇 follow-up 见 §4.4；不在 skeleton 阶段关闭或宣称任何 source 已修复。

### 4.4 跨簇 follow-up（5 个；不属于本 plan source ledger，也不进入 P 阶段锁）

以下编号在 shard-3 中是真实的 agent/narration 相关 finding，但不属于本次 plan 正式 source 清单。它们只作为后续 owner 的输入，不计入本 plan 的 30 个 confirmed source 或 acceptance 的“全部覆盖”统计：

| Issue | shard cluster | follow-up 边界 |
|---:|---|---|
| #1688 | agent-zone-narration-routing | 由 route/bridge owner 另行收口；本 plan 不锁定其修复。 |
| #1692 | agent-zone-narration-routing | 由 server bridge/schema owner 另行收口；本 plan 不锁定其修复。 |
| #1701 | agent-llm-fallback | 由 chat/LLM fallback owner 另行收口；本 plan 不锁定其修复。 |
| #1728 | npc-archetype-schema | 由 schema/registry owner 另行收口；本 plan 不锁定其修复。 |
| #1734 | nullable-option-schema | 由 schema wire-contract owner 另行收口；本 plan 不锁定其修复。 |

这些 follow-up 可在后续实现中复用本 plan 的 pipeline 接口，但必须拥有自己的 source-to-deliverable、owner 和关闭证据。

## 5. Proposed pipeline

### 5.1 统一 envelope（P0 前置契约）

每一条进入共享管道的事件都先包装为内部 envelope；字段名最终以现有 schema/代码核查为准，草案只锁定语义：

- `event_id`：来自 producer 的稳定事件身份；没有时由 producer identity + source sequence/tick + entity key 生成，禁止每次重试重新生成。
- `source` / `kind` / `source_tick` / `tick_epoch`：用于 producer registry、重启后的时间域区分和取证。
- `payload`：原始事件或 renderer 输入，进入 validate 前不可直接 publish。
- `dedupe_key` / `ordering_key`：分别解决重复消费与同一实体/zone 的顺序，不混为一个字段。
- `attempt` / `state`：`pending`、`validated`、`routed`、`published`、`rejected`、`dead_letter` 等本 plan 必选状态；`published` 是 publisher-side terminal in-scope state，不是 subscriber delivery 或 visible-side-effect confirmation。未来 server receipt contract 落地后才可增加可选 `consumed`。
- `scope` / `target`：route 阶段按现有 `NarrationV1` 可表达字段生成的显式结果，缺少必要信息不得隐式 broadcast。`dimension` / `recipient_policy` 仅是未来 wire 扩展候选，不得写入当前 envelope-to-wire 锁。
- `correlation_id`：关联同一输入事件的 command/narration side effects，重试复用。

### 5.2 Stage 1 — ingest（摄入与保留）

1. 所有 Redis channel 进入统一 ingress adapter；adapter 只做 channel/kind 识别、最小反序列化和 envelope 建立，不在 callback 内直接调用 LLM 或 publish。
2. 普通 Redis Pub/Sub callback 没有 ack/replay：消息抵达进程后，adapter 尽快写入 bounded pending queue/store，再启动 renderer/LLM；只有 pending 写入成功后的事件才具备重启恢复。进程在 pending 写入前崩溃时事件可能丢失，这是当前 transport 的明确边界，不能宣称源端重投。
   **[DRAFT-DECISION F07]** pending 写入成功后以同一 `event_id` 恢复/去重；写入成功、worker 处理前或处理中崩溃均从 pending store 恢复。故障注入测试只锁定 post-ingest recovery；覆盖 pre-ingest crash 的强保证必须先迁移 producer+consumer 到 Redis Streams/consumer group 等 durable claim/ack transport，作为 §8 开放选项，不在本 plan P 阶段实现。
3. 为每个 producer/channel 建显式注册表，记录 payload validator、source identity、ordering key、最大 payload/context 大小和失败策略。
4. handler 异常、post-ingest 进程重启和 worker 暂时不可用时，仍留在 pending store 的 item 可按同一 key 重试；这不等于 Redis Pub/Sub 的 subscriber delivery replay，也不保证 publisher resolve 后消费者一定收到。queue capacity、global concurrency、优先级、满队列 terminal policy 以及 transition telemetry schema 必须在 P0 按 §8.12 冻结，不能以无界 fallback 代替。

**本阶段锁定**：#1510、#1518、#1673、#1746，并为 #1486/#1619 提供顺序入口。

### 5.3 Stage 2 — dedupe（去重、顺序与并发）

1. 以稳定 `dedupe_key` 做 pipeline 内的 same-event retry claim；Redis publish 失败重试不得换新 batch/event id，但这不等于当前 Pub/Sub receiver 已实现可见副作用去重。
2. 以 `ordering_key` 对同一 player/entity/zone 的事件串行化；不同 key 可 bounded 并发，设置全局并发上限和队列上限。
3. 将 feature cooldown、`lastStatusByFaction`、`lastNarrationTickByKey` 等业务状态变成 confirm 后 commit 的 side effect；pipeline 在事件成功前不调用不可逆状态推进。
4. 明确 stale event、重复 event、跨 restart tick epoch 的处理：可安全忽略的事件写入已处置原因，不可安全忽略的事件 requeue 或 dead-letter。
5. 对 command + narration 两类 side effect 使用同一个 correlation/idempotency 语义，避免一边成功、另一边失败后生成全新 batch 重发。

**本阶段锁定**：#1481、#1486、#1495、#1567、#1581、#1619、#1659、#1675、#1679、#1702。

### 5.4 Stage 3 — validate（schema、内容与输入边界）

1. ingress 先按 channel/kind 运行对应 source payload validator；payload 合法后才调用 renderer。renderer 构造 `{ v, narrations }` 后，再对输出运行 `NarrationV1` contract validator，只有输出验证通过才能进入 route/publish；不能把输入 payload 当作 NarrationV1，也不能把“TypeScript 类型通过”当成 wire validation。
2. 集中约束 style enum、text 长度、metadata 字段、target 格式、optional/null 语义、numeric finite 值和上下文大小。
3. validator registry 必须覆盖每个已注册 channel/kind/variant；缺 validator 是启动/测试失败，而不是运行时放行。
4. 畸形输入按 item 隔离：可生成的 unknown/fallback 逐条生成 bounded fallback；不可恢复的 item 进入 rejected/dead-letter 并保留原因，不能使整批已摄入消息消失。
5. 对 producer/consumer event 名称做契约 pin，发现 server 无 producer 的死事件或 schema 漂移时 fail-fast。

**本阶段锁定**：#1530、#1616、#1654、#1673、#1674、#1746。

### 5.5 Stage 4 — route（现有 scope/target 合约）

1. route policy 根据已验证 payload 和当前 world state 生成现有 `NarrationV1` 可表达的 canonical `(scope, target)`；renderer 不得自行拼 wire target。
2. 对 `player`、`zone`、`broadcast`、内部/system scope 建显式 allowed transition；缺 zone、无法解析 player 或隐藏事件不能自动升级为 broadcast。
3. publish 前对拍现有 server selector：zone target 必须是可命中的真实 zone，player target 必须是 normalized identity。
4. 匿名化先于 route，按最长/禁止集合确定性处理；context/window 字段保持同一时间范围和 tick epoch。
5. 当前 `NarrationV1`/Rust mirror/server selector 均无 `dimension` 或 recipient authorization contract；P4 不声称 dimension gate，也不承接 SpiritNiche `revealed/revealed_by`。#1738 保留在 source ledger，但实现锁与关闭证据必须等待 §8 的 schema + producer authz data + enforcing consumer 决议。

**本阶段锁定**：#1525、#1528、#1532、#1587、#1590、#1607。#1738 仅登记 contract gap，不进入 P 阶段锁。

### 5.6 Stage 5 — publish confirm（唯一出口、确认与取证）

1. 所有通过 route 的 narration 只能从集中 publisher 发往 `AGENT_NARRATE`；publisher 自己包住 Redis publish rejection、超时和 transport error。
2. `published` 表示 publisher/client 侧 Redis 调用 resolve，是本 plan 的 terminal in-scope state；它只表示 Redis 接受了该调用，不表示 subscriber 收到、server 已消费、recipient 已选择或客户端已展示。现有 Pub/Sub 返回的 subscriber count 不能作为 delivery receipt，零 subscriber 时 resolve 也不构成端到端成功。只有未来 correlated receipt + consumer dedupe contract 落地后，才可增加可选 `consumed`，本 plan 不要求、也不等待不存在的 producer。
3. publish failure、连接断开或 resolve 后的 agent crash 可能分别造成 pending 保留、重复 publish 或 subscriber 未收到；因此本 plan 不声称 lossless delivery、at-least-once Pub/Sub 投递或 single visible side effect。失败/重启重试保留同一 `event_id`/`dedupe_key`/`correlation_id`，但当前 `NarrationV1`/server consumer 没有足够的 wire-level idempotency contract，重复可见副作用必须作为已知边界记录，而不是由本 plan 伪造消除。超过有限尝试次数进入 bounded dead-letter，不伪造成功；测试覆盖 client-side resolve、publish rejection、超时、断线、零 subscriber 和 resolve 后 crash，不注入虚构的 server consumed receipt。
4. **[DECISION-NEEDED F06]** command+narration 的部分确认不能由单一 event-level ack 推断。P0 必须冻结四格状态矩阵：
   - command 成功 + narration 成功：两个 side effect 各自 terminal success，correlation cursor 是否前进必须由选定矩阵规定。
   - command 成功 + narration 失败：command 不应因 narration 重试而重复；narration 复用原 key 重试或 dead-letter。
   - command 失败 + narration 成功：narration 不应因 command 重试而重复；command 复用原 key 重试或 dead-letter。
   - command 失败 + narration 失败：两者各自按 retry/dead-letter policy 处理，不能提交完整 cursor。
   P0 必须从“每个 side effect 独立 terminal、correlation cursor 仅在两者 terminal success 时前进”与“允许 partial terminal、cursor 拆成两个可查询子游标”中选择一个，并为四种组合固定 cursor、retry、dedupe、dead-letter 结果；四种组合都要有集成测试。
5. 每次转换记录结构化原因（accepted/retried/rejected/routed/published/dead-letter）；指标至少能按 source/kind、失败阶段、队列深度、重试次数、重复命中和 publish latency 聚合。未来可选 `consumed` 不计入本 plan 必选状态链。
6. 旧 runtime 直发路径迁移完成后删除；测试或启动扫描发现绕过集中 publisher 的调用即失败。

**本阶段锁定**：#1470、#1475、#1509、#1527、#1538、#1495、#1581、#1675、#1679。

## 6. Phased delivery list

### P0：收口契约与吸收边界

- [ ] 复核本 skeleton 创建提交树内根 `CLAUDE.md`、`docs/CLAUDE.md` §§五-六 的读取证据，并确认 implementation owner 为 `agent`。
- [ ] 建立 30 个 confirmed source issue 的唯一映射表，标明 shared infrastructure owner；其中 4 个是 #1470 的重复证据，#1702 由 bounded concurrency 交付物直接吸收；另将 §4.4 的 5 个跨簇 follow-up 单独登记为外部 owner，不纳入本 plan acceptance。
- [ ] 盘点所有 Tiandao narration Redis channel、`RuntimeRedis` drain、直接 `publishNarrations` callsite、业务状态/cooldown/cursor 写点。
- [ ] 定义 envelope、event_id、dedupe_key、ordering_key、tick_epoch、ack/state transition 及 pending/dead-letter 语义。
- [ ] 定义现有 server selector 可接受的 canonical scope/target contract；dimension、SpiritNiche recipient authorization 和 consumed receipt 作为 §8 明示 contract gap，不进入当前 P 阶段实现锁。
- [ ] 收口 F06/F08/F09；F04/F07 按本次保守修订的现有 producer/consumer 与 post-ingest recovery 边界收口，F05 仍为 deferred/open gate：每条 decision-needed 必须落为具体协议、选项和测试矩阵后才能进入依赖它的后续阶段。不得把 §8.9 authorization data + enforcing consumer contract 尚未落地的隐私/fan-out 决策标为已收口。

### P1：有界 ingest 与 pending store

- [ ] 统一 Redis ingress adapter，禁止 callback 内直接 drain+LLM+publish。
- [ ] 为 chat/NPC/feature narration channel 接入 pending queue/store，支持 pending 写入后的内部重启恢复、失败 requeue 和 bounded backpressure；普通 Pub/Sub ingress 是 at-most-once，pre-ingest crash、subscriber delivery 和 publisher resolve 后的重放不在保证内。
- [ ] 为每个 channel/kind 注册 source、validator、ordering key、最大 payload/context 和失败策略。
- [ ] 覆盖 #1510、#1518、#1673 的测试：处理失败不能使已摄入事件静默消失，单条 fallback 不影响同批其他事件。

### P2：幂等、顺序与确认前状态提交

- [ ] 实现稳定 event/dedupe/correlation key；retry 不生成新 batch id。
- [ ] 实现 per-player/entity/zone ordering 与全局 bounded concurrency；禁止无界 `void handlePayload` 旁路，并以 #1486/#1619/#1702 共用的容量/并发测试锁住。
- [ ] 将 cooldown、last status、state cursor、pending command rearm 等副作用迁入 confirm 后提交阶段。
- [ ] 覆盖 #1481、#1486、#1495、#1567、#1581、#1619、#1659、#1675、#1679。

### P3：NarrationV1 validate 与 schema drift 关口

- [ ] 对每个 channel/kind 先运行 source payload validator；renderer 构造 `{ v, narrations }` 后再集中运行 NarrationV1 output validator，并补齐 channel/variant validator 注册表。
- [ ] 锁定 style/text/metadata/nullable/numeric/context bounds；禁止任意 style、无界文本和无界 social relationship 结果进入发布。
- [ ] 对 malformed batch 做逐条 fallback/reject；对 server producer 与 agent consumer 做 event-name/schema pin。
- [ ] 覆盖 #1530、#1616、#1654、#1673、#1674、#1746。

### P4：canonical scope/target route

- [ ] 统一生成 player/zone/broadcast target，删除 runtime 自拼 prefix/raw id 的路径。
- [ ] route 前置 scope policy 与匿名化；缺 zone/target fail-closed。
- [ ] 加现有 server selector compatibility tests，确保 zone 与 player-target narration 真能命中消费者。
- [ ] 覆盖 #1525、#1528、#1532、#1587、#1590、#1607；#1738 等待 §8 authorization contract 决议，不得以本阶段完成为由关闭。

### P5：publish confirm、迁移清理与集成验收

- [ ] 将所有 agent narration runtime 迁移到唯一 publisher；集中处理 rejection、timeout、retry、dead-letter 和 metrics。
- [ ] 验证 publisher-side `published` 状态：只有 Redis client 调用 resolve 后才提交明确绑定为“publisher 已完成”的 cooldown/cursor；不得把该 resolve 当作 subscriber delivery ack、server consumed 或 single visible side effect 的证明。publish failure、断线、零 subscriber 和 resolve 后 crash 的有限重试/重复边界必须被记录；超过上限进入 bounded dead-letter。未来可选 `consumed` 不得成为当前状态提交前置。
- [ ] 冻结 dead-letter 生命周期：设置最大 item 数或字节数、每条记录的 retention/TTL、清理或归档 owner、达到容量上限后的丢弃/覆盖/暂停策略，并为 overflow 记录 telemetry；不得让 malformed input、publish exhaustion 或 queue overflow 无限追加 terminal records。
- [ ] 跑 agent 单测、构建、Redis integration/e2e，并用真实 server selector 验证现有 scope/target；dimension/SpiritNiche/consumed contract gap 不得伪造测试。其余 source issue 完成后才由 triage 关联/关闭，#1738 保持 open 直到后续 contract owner 落地。


## 7. Acceptance criteria

### 7.1 功能与可靠性

- 任意已注册 narration event 都能在 telemetry 中看到完整状态链：`ingest → dedupe → validate payload → render → validate NarrationV1 → route → published`（或明确的 rejected/dead-letter）；不存在只在单测可达、生产无 consumer 的路径。
- Redis publish 被人为拒绝、超时或断线时：调用不会产生未处理 promise rejection；事件保持 pending/retry/dead-letter 之一；不会提前推进 cooldown、cursor、last status 或删除 pending。这里的 resolve 只验证 publisher/client 侧调用完成，不验证 subscriber 收到或 server/client 可见。
- 同一 `event_id` 在 pipeline 内重试时复用原 key、不得生成新 batch id，并对重复 claim 产生审计记录；不把当前 Redis Pub/Sub 或 `NarrationV1`/server consumer 解释为可证明的 exactly-once 或 single visible side effect。若 publisher 在 resolve 后、terminal 状态落盘前崩溃，恢复可能再次 publish；若 subscriber 未连接，resolve 也可能没有任何消费者收到。需要 single visible side effect 时，必须由未来的 correlated receipt + receiver dedupe contract 提供证据。
- 同一 ordering key 的事件按输入顺序发布；不同 key 仍能在明确上限内并发；队列满和处理超时有可观察结果，不会无界增长。
- 进程重启后，已写入 pending 且未到 terminal 的事件可恢复；普通 Pub/Sub 在 pending 写入前崩溃不承诺重投。server tick 重置不会错误抑制合法新事件，也不会让旧事件无限重放。

### 7.2 合约、路由与隐私

- 每条 source payload 先通过对应 payload validator；renderer 构造的发布对象再通过 NarrationV1 validator。非法 style、超长文本、非法 nullable、未知 variant、无界上下文均在 publish 前被拒绝或转 bounded fallback。

- `scope=zone` 的 target 必须是 server selector 可命中的 canonical zone；`scope=player` 必须是 normalized player identity；隐藏/无 target 事件不能隐式变成 broadcast。
- 本 plan 只验收现有 `scope/target` route；当前 contract 无 dimension/recipient policy 字段，且无 SpiritNiche authorization data/enforcing consumer，因此不宣称 dimension gate 或 #1738 privacy closure。两项保持 §8 contract gap 与 open source issue。

- agent producer/consumer schema 注册表与 server event producer 对拍通过；缺 validator、死 event name、Rust `Option`/JSON `null` 漂移在测试或启动关口暴露。

### 7.3 测试与运维门禁

- `cd agent/packages/tiandao && npm test` 通过，覆盖每个 pipeline state、错误分支、重试、重复、乱序、空队列、满队列、重启恢复和 route fail-closed。
- `cd agent && npm run build` 通过；若 schema source 发生改动，先重建 `@bong/schema` dist，再验证 tiandao import。
- Redis 集成测试至少覆盖：publish failure/retry、同 key pipeline dedupe、批量 malformed item isolation、tick epoch restart、canonical zone/player target、零 subscriber 的 resolve 语义、resolve 后 crash 的重复边界，以及 pending 写入后的内部重启恢复；测试明确不把 Pub/Sub pre-ingest replay、subscriber delivery ack、single visible side effect、dimension routing、SpiritNiche recipient authz 或 server consumed receipt 伪装成现有能力。
- dead-letter boundedness 测试验证：malformed/publish-exhaustion/queue-overflow 终态不会超过配置的最大 item/byte 容量；TTL/retention 到期会清理或归档；容量已满时执行冻结的 overflow policy 并发出可查询 telemetry，不能静默无限增长。
- 生产代码中不存在绕过集中 publisher 的 direct `publishNarrations` 或等价 Redis publish 调用；扫描结果与集中注册表一致。
- 30 个 confirmed source issue 都有唯一映射：#1475/#1509/#1527/#1538 作为 #1470 的重复证据共享根因，#1702 由 bounded concurrency 交付物覆盖；可实现的 source 需有修复 commit/测试证据。#1738 因明确 contract gap 保持 open，不得以 plan 归档代替实现关闭。§4.4 的 5 个跨簇 follow-up 不计入本 plan acceptance，由各自 owner 提供独立证据。

## 8. Open questions（进入 P0 前必须收口）

1. **durable transport 选项**：本 plan 可用内存或持久 pending store，但只能恢复已写入 pending 的事件。普通 Redis Pub/Sub ingress 是 at-most-once；`PUBLISH` resolve 只表示 Redis client-side 调用完成，不能证明 subscriber delivery。若未来要求覆盖 pending 写入前的 crash、consumer ack 或 receiver-side replay，必须将对应 server producer 与 agent consumer 一并迁移到 Redis Streams/consumer group（或等价 durable claim/ack transport），并确定 retention、redelivery、claim timeout、consumer group 与清理责任；不能仅换 pending store 就宣称 pre-ingest replay 或 lossless delivery。
2. **可选 consumed receipt 与可见副作用去重**：本 plan 以 publisher-side best-effort resolve 的 `published` 为 terminal in-scope state，不把它当作 server consumed、recipient selected 或 client displayed。若未来需要证明 server 已消费并保证同一事件只有一个可见 narration side effect，必须新增 correlated receipt wire contract（event/correlation id）、server receipt producer、receiver dedupe、agent receipt consumer、超时/重复语义及跨栈集成测试；在该完整生产链落地前，`consumed` 不得成为必选状态或 pending 删除前置。
3. **dead-letter 生命周期与容量**：所有 publish exhaustion、malformed input 和 queue-overflow 进入 dead-letter 的路径必须共用一个有界策略。P0 必须冻结最大 item 数或字节数、单条记录最大 payload、retention/TTL、清理/归档 owner、容量达到上限时的 overflow policy（reject-and-metric、覆盖最旧记录或暂停新写入，三者择一并说明取舍）以及 `dead_letter_overflow`/eviction telemetry；达到上限后不得无限追加、静默丢弃或回到无界 pending。测试必须持续注入 terminal failure，证明 dead-letter 总量受上限约束、TTL 可清理/归档且 overflow 行为可观测。
4. **命名与 owner**：统一 pipeline 应落在 `agent/packages/tiandao/src/` 哪个模块/类名；`RuntimeRedis` 现有 API 是适配层还是应被替换？需要避免新建第二套 runtime abstraction。
5. **幂等键来源**：哪些 server/agent event 已有稳定 event id；缺失者由哪个 producer 生成？`source_tick + entity` 在同 tick 多事件时如何避免碰撞；是否需要跨重启 `tick_epoch` 持久化。
6. **顺序粒度**：默认按 player、entity、zone 还是 source channel 串行？同一事件同时产生 command 和 narration 时，二者的 ordering/ack 是否共用 key。
7. **LLM failure 终态与责任边界 [DECISION-NEEDED F08]**：四类 failure 必须逐类选择责任主体和固定协议，不能共用模糊 fallback：
   - timeout：选“有限重试后 bounded fallback 发布”或“有限重试后 rejected/dead-letter”；
   - transport rejection/disconnect：选“按相同 key 重试”或“达到上限后 dead-letter”，不得把 client resolve 当成功；
   - malformed JSON：选“validator 生成固定上限 unknown fallback”或“直接 rejected”；
   - schema-invalid response：选“逐项 fallback”或“逐项 rejected/dead-letter”。
   P0 必须为每类写明最大 retry、退避、fallback 最大长度/字段、最终 state 和 pending 是否保留，并用 fake LLM/断线集成测试锁定。
8. **dimension-aware route contract gap**：当前 TypeBox `NarrationV1`、Rust mirror 与 server selector 均没有 dimension 字段。未来若启用必须同步扩展 schema source/generated artifacts、Rust mirror、agent producer 和 server enforcing selector，并加跨维负向测试；在此之前只按现有 scope/target 路由。
9. **SpiritNiche authorization contract gap（#1738）**：当前 `NicheIntrusionEventV1`/`WorldStateV1` 不含 owner、revealed 或 authorized viewers，server selector 也不能消费 recipient policy。后续方案必须明确 authorization data source（server canonical niche owner + reveal registry）、把所需字段送入 agent 或由 server 自行判权，并指定 enforcing consumer（建议 server selector fail-closed）；producer、schema、consumer 与隐私 e2e 未一并落地前，#1738 保持 open 且不进入 P4 锁。
10. **隐私与 fan-out [DEFERRED F05]**：F05 仍是未收口的 privacy/fan-out decision，不能在本 plan 的 P0 里标记为 closed，也不能让后续 P1/P4/P5 假设 recipient policy 已存在。只有 §8.9 的 authorization data + enforcing consumer contract 落地后，才能在 per-recipient durable 与 event-level atomic 中选择，并固定 recipient key、retry、dead-letter 和 pending 删除条件；在此前，本 plan 只允许现有 scope/target route，且不得以 mock authorization、测试专用 receipt 或未消费字段代替生产链。
11. **command+narration partial confirm [DECISION-NEEDED F06]**：必须从“每个 side effect 独立 terminal、correlation cursor 仅在两者 terminal success 时前进”与“允许 partial terminal、cursor 拆成两个可查询子游标”中选择一个，固定四种成功/失败组合的 cursor、retry、dedupe、dead-letter 结果；不能以单一 event-level ack 掩盖部分确认。
12. **backpressure 与优先级 [DECISION-NEEDED F09]**：现有证据不足以推出数值，不能伪造默认值。P0 必须从“固定 capacity + global concurrency + priority tiers + 满队列 reject/dead-letter”与“固定 capacity + 低优先级合并/淘汰但保留隐私/教学事件”中选择一套，冻结具体配置字段和值、优先级、terminal policy，以及 transition telemetry 的唯一 key、字段（event_id、state、reason、attempt、queue_depth、timestamp/tick_epoch、source/kind）和查询方式；同时必须落实下列 dead-letter policy row，且所有容量/TTL 字段在 P0 退出前必须为有限、已配置值，否则阻塞 P1/P5：

   | terminal sink | size bound | retention / cleanup owner | overflow policy | required evidence |
   |---|---|---|---|---|
   | `dead_letter` | finite `max_items` + `max_bytes`；每条记录有 `max_payload_bytes` | finite `retention_ttl`；由 agent Tiandao pipeline cleanup/archive job 到期清理或归档 | conservative default：reject new terminal record + emit `dead_letter_overflow`，不重新塞回 pending；若 P0 选择覆盖/暂停，必须记录取舍并保持有界 | 持续注入 malformed、publish-exhaustion、queue-overflow，验证 item/byte/TTL 上限及 overflow telemetry |

   验收同时断言 pending 与 dead-letter 均不超过各自上限，满队列和 dead-letter overflow 均产生可查询 telemetry。
13. **已有 feature skeleton 的迁移顺序**：`poi novice`、`anqi charged`、各类 narration target-prefix/runtime bridge skeleton 是否在 P1/P2 作为试点迁移，还是先完成共享管道后统一迁移；必须避免主循环与独立 runtime 双发。
14. **跨簇 schema follow-up 的边界**：§4.4 的 #1728/#1734 不属于本 plan source ledger 或 P-phase lock；它们的 schema source/generated artifacts、owner 和 closure evidence 必须由对应 schema/registry follow-up 单独冻结。本 plan 只负责对未知/不兼容输入 fail-closed，不借此扩大实现范围。
15. **source issue 关闭门**：只有共享管道完成后再逐项复验，还是允许某些同根因 issue 在 feature follow-up PR 中先关闭？需要统一 triage 证据格式，避免“关联 PR”被误写成“已修复”；#1738 必须等待 §8.9 的完整跨栈 contract 落地。

## 9. 风险与迁移注意

- **双发风险**：主循环 drain、独立 narration runtime、共享 pipeline 同时订阅会产生重复发布；迁移每个 channel 时必须有唯一 consumer claim 和重复 pin。
- **批量启动刷屏**：ingest 恢复或 startup snapshot 可能一次性进入大量 POI/状态事件；优先级、合并和上限必须在 P2/P5 验收，不得用无限丢弃掩盖压力。
- **语义误路由**：将缺失 zone 默认成 broadcast 会把隐藏事件扩大泄露；route policy 应 fail-closed，测试必须覆盖缺字段和未知 target。
- **伪 exactly-once**：Redis publish resolve 只等于本 plan 的 publisher-side `published` 状态，不等于 subscriber delivery、server 已选择 recipient 或客户端已展示；未来 `consumed` receipt 只有在 §8.2 的完整生产链落地后才可增加。若需要 single visible side effect，必须同时提供 receiver dedupe 与 correlated receipt，不能由当前 Pub/Sub client resolve 推导。
- **跨轨文件冲突**：本 plan 主体限定 agent；dimension selector、SpiritNiche authz、server receipt、schema generated artifact 或 client handler 均是 §8 后续跨栈 contract，不在本 plan PR 中顺手扩 scope。
- **失败重试放大**：没有稳定 key 或退避上限时，publish failure 会制造重复 narration 和队列爆炸；P2/P5 必须先于大规模 runtime 迁移。

## 10. 实施工作流（scope = 6 PR，进入 active 前执行）

### 10.1 资产与跨栈边界

本 plan 是纯 agent/schema 逻辑基础设施，不含 NBT、layout、模型或视觉资产，`docs/CLAUDE.md §6.1-6.2` 的三轮资产打磨不适用。实现期间不得顺手扩展 server dimension selector、SpiritNiche authorization 或 consumed receipt；三项只能按 §8 另立跨栈 contract owner。

### 10.2 PR 序列（前一 PR merge 后才开下一 PR）

1. **PR-1 / P0 契约盘点**：注册表、source ledger、envelope/state、现有 scope/target selector pin；冻结 F06/F08/F09。
2. **PR-2 / P1 ingest**：统一 ingress 与 bounded pending store，只验收 pending 写入后的重启恢复，明确 Pub/Sub pre-ingest 不可恢复。
3. **PR-3 / P2 dedupe/concurrency**：稳定 key、per-key ordering、全局 capacity/concurrency，覆盖 #1486/#1619/#1702。
4. **PR-4 / P3 validate**：source payload validation → renderer → NarrationV1 output validation，补齐 validator registry。
5. **PR-5 / P4 route**：只实现现有 scope/target canonical route 与匿名化；不伪造 dimension 或 SpiritNiche recipient contract。
6. **PR-6 / P5 publish/migration**：publisher-side `published` state、retry/dead-letter/metrics、旧直发迁移与 agent/Redis 集成验收；不把 resolve 当作 subscriber ack 或 single visible side effect 证明；#1738 保持 open。

### 10.3 每 PR 的验证与文档更新

- agent 代码：`cd agent/packages/tiandao && npm test`；schema source 改动先 `cd agent && npm run build -w @bong/schema`，再跑 `cd agent && npm run build`。
- 每 PR 只完成对应阶段；测试证据写入当前 active plan 的阶段记录，最后一个 PR 才补 `## Finish Evidence` 并归档。
- 任一实现发现需要 server/schema/client 跨栈扩展时，停在 §8 contract gap，不以 mock receipt、测试专用 authorization 或未消费 dimension 字段代替生产链。

### 10.4 独立实施上下文

主线不亲自实施 PR；每个 PR 按 `docs/CLAUDE.md §6.4` 启动独立实现 subagent，prompt 必须限定该 PR 范围、必读本节、门禁与禁止扩展的 §8 contract gap，并以 `ultrathink` 结尾：

```text
Agent(
  subagent_type: "claude",
  model: "opus",
  prompt: "<当前 PR 范围、依赖、测试与禁止项>\n\nultrathink"
)
```

subagent 只负责实现、测试、push 与创建该 PR，不等待 review、不 merge；主线只接收 PR URL、HEAD 与简短门禁证据。执行模型、validator 与 reviewer 的真实模型 ID 必须按当时路由记录在 commit trailer/PR body，不能以模板占位名冒充实际上游模型。

### 10.5 Review 等待与返工

每个 PR 必须完成仓库当时有效的 review gate 后才能进入下一 PR。pending 时按 `docs/CLAUDE.md §6.5` 使用 `ScheduleWakeup delaySeconds=1200`，禁止 sleep/busy-poll，最多三轮无结果才停交人工；有修改意见时由新的独立返工上下文修复、重跑对应栈门禁并重新等待 review，不能自行判定“应该通过”。review 触发动作遵循用户当时指令，本 skeleton 不授权自动发送触发评论。

### 10.6 单次 consume-plan 收口

用户单次提交 `/consume-plan` 后，主线按 PR-1 → PR-6 串行完成实现、review 收敛与 merge，不要求用户逐 PR 接力。所有 in-scope source 有修复/测试证据、#1738 明确保留为 open contract gap、全部阶段标记完成并补齐 `## Finish Evidence` 后，才可迁入 `docs/finished_plans/`；不得以 skeleton/关联 PR/日志可见替代真实实现关闭。
