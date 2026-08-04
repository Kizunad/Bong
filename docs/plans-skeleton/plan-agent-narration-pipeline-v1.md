# plan-agent-narration-pipeline-v1（骨架）

> **骨架（草案）**。一句话主题：把 Tiandao 的 narration 从“各 runtime 各自 drain、各自改状态、各自 Redis publish”收口为一条可重放、可去重、可校验、可路由、可确认的 agent 发布事务管道，处理 cluster 3 的 30 个 source issue ID：排除已 falsified 的 #1551 后保留 29 个 confirmed source，其中 4 个是 #1470 的重复证据，另有 6 个跨簇 follow-up 不计入本 plan 账本。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|---|---|---|---|
| P0 | 事件信封、owner、source issue 吸收清单与迁移边界 | plan_skeleton | ⬜ |
| P1 | ingest：可靠摄入、保留待处理事件、失败可重放 | fix_pr | ⬜ |
| P2 | dedupe：稳定幂等键、顺序/并发控制、提交前不推进游标 | fix_pr | ⬜ |
| P3 | validate：NarrationV1 合约、文本/样式/目标边界与 fallback | fix_pr | ⬜ |
| P4 | route：scope/target/dimension 规范化与隐私路由 | fix_pr | ⬜ |
| P5 | publish confirm：失败边界、确认后状态提交、指标/回归门禁 | fix_pr | ⬜ |

## 接入面

- **进料**：现有 Tiandao Redis channel、`RuntimeRedis` drain API、各 narration runtime 的事件 payload、`WorldStateV1`/server tick、schema validator。
- **出料**：唯一 `AGENT_NARRATE` 发布出口，供 server `narration_selector` 按 scope/target/dimension 投递；失败事件留在可重放的 pending 状态。
- **共享契约**：复用现有 NarrationV1、`AGENT_NARRATE`、`RuntimeRedis`、server 已有 target 解析规则；新增共享 pipeline envelope/idempotency/ack 语义前先核实现有类型，禁止为每个 runtime 再造一套。
- **跨仓库边界**：本草案主改 agent；若发现 server 端 target/dimension 合约无法表达现有需求，只提出最小 schema/bridge 接缝，不能顺手改 server gameplay 或 client UI。
- **世界观锚点**：本 plan 只处理天道叙事的可靠投递与作用域隐私，不新增世界观名词、境界、经济或真元物理。
- **真元边界**：本 plan 不移动、不生成、不衰减真元；任何涉及 qi 的 narration 仅传递已存在事件字段，不改变 `qi_physics` ledger。

## 0. 立项依据与职责

### 0.1 来源与实现 owner

- **来源**：2026-08-03 flash-review cluster 3；合并汇总将其归类为“agent 发布/叙事管道”。triage 原始 source ledger 有 30 个 issue ID；其中 #1551 已由 shard-2 判定为 falsified，故本草案只把其余 29 个作为 confirmed source；这 29 个中 #1475/#1509/#1527/#1538 是 #1470 的重复证据，并非 4 个独立根因。另有 6 个真实但属于其他 cluster 的编号，见 §4.4，不计入本 plan 的 source ledger。
- **唯一实现 owner**：`agent`（Tiandao runtime / schema / Redis IPC）；不归入 R1-R10 server/client 重构轨道。
- **实现形态**：先作为共享 agent 基础设施 plan，再由各 feature narration runtime 逐个迁移；不是把 30 个 issue 分成 30 个互相复制的修复出口。
- **依据的流程出口**：后续写入仓库时，必须遵循 `plan-refactor-master-v1.md §10.1.1` 的三步：cluster intake 列明 source issue、证据、唯一 owner 与 implementation owner；在同一提交树中读取根 `CLAUDE.md` 与 `docs/CLAUDE.md` §§五-六；skeleton 合入 `origin/main` 且 owner 入队后，才关闭 source issue 并关联证据。

### 0.2 目标接入面

- **进料**：现有 Tiandao Redis channel、`RuntimeRedis` drain API、各 narration runtime 的事件 payload、`WorldStateV1`/server tick、schema validator。
- **出料**：唯一 `AGENT_NARRATE` 发布出口，供 server `narration_selector` 按 scope/target/dimension 投递；失败事件留在可重放的 pending 状态。
- **共享契约**：复用现有 NarrationV1、`AGENT_NARRATE`、`RuntimeRedis`、server 已有 target 解析规则；新增共享 pipeline envelope/idempotency/ack 语义前先核实现有类型，禁止为每个 runtime 再造一套。
- **跨仓库边界**：本草案主改 agent；若发现 server 端 target/dimension 合约无法表达现有需求，只提出最小 schema/bridge 接缝，不能顺手改 server gameplay 或 client UI。
- **世界观锚点**：本 plan 只处理天道叙事的可靠投递与作用域隐私，不新增世界观名词、境界、经济或真元物理。
- **真元边界**：本 plan 不移动、不生成、不衰减真元；任何涉及 qi 的 narration 仅传递已存在事件字段，不改变 `qi_physics` ledger。

## 1. Goals

1. 让所有 narration 事件都经过统一的 `ingest → dedupe → validate → route → publish confirm` 管道，不再允许 runtime 直接 `publish` 绕过保护边界。
2. 在 Redis publish 失败、LLM 解析失败、进程重启、同 tick 重试和并发到达时，保证事件不会静默丢失、无限重复或乱序污染后续状态。
3. 将 scope、target、dimension、隐私和文本合约变成发布前可验证的边界；非法事件拒绝或产生明确的 bounded fallback，而不是把错误 payload 发进 server。
4. 把“状态已推进/冷却已记录/队列已 drain”与“叙事已被发布确认”绑定，形成可观测的提交语义。
5. 为现有和未来 narration runtime 提供单一接入点，减少 feature plan 只写 renderer/模板却没有生产消费或回流确认的孤岛。

## 2. Non-goals

- 不在本 plan 内补齐每一个 feature event 的业务 renderer、世界观文案或新的叙事内容；已有 `poi novice`、`anqi charged` 等 feature skeleton 仍由各自 owner 实现，只需迁移到共享管道。
- 不修改 server gameplay、战斗、修炼、qi ledger、NPC 行为、worldgen 或 client HUD/VFX；它们只是 narration 的生产端或消费端，不是本 plan 的实现域。
- 不追求跨 Redis/进程的数学意义“exactly once”幻觉；先明确并实现稳定幂等键 + 可重放的 at-least-once 传输，避免依赖无法提供的外部事务。
- 不以兼容层长期保留旧的每 runtime 直发路径；迁移完成后旧出口应删除或在集中注册表中 fail-fast，而不是双发兜底。
- 不把失败吞掉后仅写日志视为可靠性；日志、指标和 dead-letter/pending 语义必须能支持重放或人工处置。
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
| #1738 | `scattered-cultivator-narration.ts::renderNpcIntrusionNarration` 广播 SpiritNiche 坐标，违反 revealed/revealed_by 隐私。 | route 必须先执行 recipient/privacy policy，再计算 publish fan-out。 |

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
| #1738 | narration recipient scope 泄露隐私。 | 幂等记录不能绕过 recipient policy；同一事件不同 recipient 的 key/审计需明确。 |
| #1746 | `world-model.ts::cloneZoneSnapshot` 丢失 context/persistence 使用的 `ZoneSnapshot.status`。 | snapshot clone 是 pipeline 输入的一部分，字段丢失必须在 ingress/context contract pin。 |

> 注：#1616、#1619、#1738 等可同时落入多个阶段；上表按“需要由管道解决的边界”重复引用，但 source issue 只计一次。正式 source ledger 是 30 个原始 issue ID：排除已 falsified 的 #1551 后剩 29 个 confirmed source，其中 #1475/#1509/#1527/#1538 是 #1470 的重复证据，剩余 confirmed source 无 falsified 项。六个跨簇 follow-up 见 §4.4，不计入本 plan source ledger。

## 4. Source issue 清单与唯一聚类

### 4.1 publish-error-boundary（5 个 source ID；其中 4 个为 #1470 重复证据）

`#1470 #1475 #1509 #1527 #1538`

### 4.2 narration-routing-and-delivery（14 个 confirmed source ID）

`#1481 #1486 #1495 #1510 #1518 #1525 #1528 #1530 #1532 #1567 #1581 #1587 #1590 #1607`

`#1551` 不在此清单：shard-2 判定其为 falsified，仅保留在 §3.2 的 regression note。

### 4.3 agent-publish-idempotency（10 个 confirmed source ID）

`#1616 #1619 #1654 #1659 #1673 #1674 #1675 #1679 #1738 #1746`

**正式 source ledger 计数：30 个原始 issue ID；其中 #1551 已 falsified，因此 29 个 confirmed source ID；29 个中 4 个是 #1470 的重复证据，25 个是非重复 confirmed source。** 不计入本 plan 的跨簇 follow-up 见 §4.4；不在 skeleton 阶段关闭或宣称任何 source 已修复。

### 4.4 跨簇 follow-up（不属于本 plan source ledger，也不进入 P 阶段锁）

以下编号在 shard-3 中是真实的 agent/narration 相关 finding，但不属于本次 cluster 3 三组正式 source 清单。它们只作为后续 owner 的输入，不计入本 plan 的 30 个原始 ID、29 个 confirmed source 或本 plan acceptance 的“全部覆盖”统计：

| Issue | shard cluster | follow-up 边界 |
|---:|---|---|
| #1688 | agent-zone-narration-routing | 由 route/bridge owner 另行收口；本 plan 不锁定其修复。 |
| #1692 | agent-zone-narration-routing | 由 server bridge/schema owner 另行收口；本 plan 不锁定其修复。 |
| #1701 | agent-llm-fallback | 由 chat/LLM fallback owner 另行收口；本 plan 不锁定其修复。 |
| #1702 | agent-unbounded-narration-concurrency | 由 runtime concurrency owner 另行收口；本 plan 不锁定其修复。 |
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
- `attempt` / `state`：`pending`、`validated`、`routed`、`published`、`consumed`、`rejected`、`dead_letter` 等状态；状态转换必须有单向规则和原因。
- `scope` / `target` / `dimension` / `recipient_policy`：route 阶段的显式结果，缺少必要信息不得隐式 broadcast。
- `correlation_id`：关联同一输入事件的 command/narration side effects，重试复用。

### 5.2 Stage 1 — ingest（摄入与保留）

1. 所有 Redis channel 进入统一 ingress adapter；adapter 只做 channel/kind 识别、最小反序列化和 envelope 建立，不在 callback 内直接调用 LLM 或 publish。
2. 收到消息先写入 bounded pending queue/store，再返回 Redis callback；不能复用“先 trim/drain、后处理”的不可逆顺序。
   **[DRAFT-DECISION F07]** callback ack 的前置条件是 pending 写入成功返回；pending 写失败时不得 ack、不得推进业务状态，源端必须可重投。进程在“写入前”崩溃时依赖源端重投，在“写入成功、callback ack 前”崩溃时按同一 `event_id` 去重，在“callback ack 后、worker 处理前”崩溃时从 pending store 恢复；这三段都必须有故障注入测试。
3. 为每个 producer/channel 建显式注册表，记录 payload validator、source identity、ordering key、最大 payload/context 大小和失败策略。
4. handler 异常、进程重启和消费者暂时不可用时，pending item 可重放；queue capacity、global concurrency、优先级、满队列 terminal policy 以及 transition telemetry schema 必须在 P0 按 §8.9 冻结，不能以无界 fallback 代替。

**本阶段锁定**：#1510、#1518、#1673、#1746，并为 #1486/#1619 提供顺序入口。

### 5.3 Stage 2 — dedupe（去重、顺序与并发）

1. 以稳定 `dedupe_key` 做 same-event retry claim；Redis publish 失败重试不得换新 batch/event id。
2. 以 `ordering_key` 对同一 player/entity/zone 的事件串行化；不同 key 可 bounded 并发，设置全局并发上限和队列上限。
3. 将 feature cooldown、`lastStatusByFaction`、`lastNarrationTickByKey` 等业务状态变成 confirm 后 commit 的 side effect；pipeline 在事件成功前不调用不可逆状态推进。
4. 明确 stale event、重复 event、跨 restart tick epoch 的处理：可安全忽略的事件写入已处置原因，不可安全忽略的事件 requeue 或 dead-letter。
5. 对 command + narration 两类 side effect 使用同一个 correlation/idempotency 语义，避免一边成功、另一边失败后生成全新 batch 重发。

**本阶段锁定**：#1481、#1486、#1495、#1567、#1581、#1619、#1659、#1675、#1679。

### 5.4 Stage 3 — validate（schema、内容与输入边界）

1. 在任何 renderer 或 publish 前运行对应 `NarrationV1` contract validator；不能把“TypeScript 类型通过”当成 wire validation。
2. 集中约束 style enum、text 长度、metadata 字段、target 格式、optional/null 语义、numeric finite 值和上下文大小。
3. validator registry 必须覆盖每个已注册 channel/kind/variant；缺 validator 是启动/测试失败，而不是运行时放行。
4. 畸形输入按 item 隔离：可生成的 unknown/fallback 逐条生成 bounded fallback；不可恢复的 item 进入 rejected/dead-letter 并保留原因，不能使整批已摄入消息消失。
5. 对 producer/consumer event 名称做契约 pin，发现 server 无 producer 的死事件或 schema 漂移时 fail-fast。

**本阶段锁定**：#1530、#1616、#1654、#1673、#1674、#1746。

### 5.5 Stage 4 — route（scope、target、dimension、隐私）

1. route policy 根据已验证 payload、当前 world state 和 recipient policy 生成 canonical `(scope, target, dimension)`；renderer 不得自行拼 wire target。
2. 对 `player`、`zone`、`broadcast`、内部/system scope 建显式 allowed transition；缺 zone、无法解析 player 或隐藏事件不能自动升级为 broadcast。
3. 在 publish 前调用 server-compatible selector contract/check：zone target 必须是可命中的真实 zone；player target 必须是 normalized identity；dimension 必须与 recipient 可达域一致。
4. 先做匿名化和隐私过滤，再计算 fan-out；SpiritNiche 等 `revealed/revealed_by` 规则不能被“叙事易懂”覆盖。
5. context/window 字段保持同一时间范围和 tick epoch，避免局部 delta 与全历史 delta 混排。

**[DECISION-NEEDED F05] 多 recipient fan-out 的终态尚不能由现有 shard 证据唯一推出。P0 必须在以下两种协议中择一并写入 envelope/测试：**
- **A — per-recipient durable（保守候选）**：为每个 `(event_id, recipient_id, envelope_variant)` 建独立 dedupe/ack/retry 状态；A 成功、B 失败时保留 B pending，只对 A 记 terminal success，event-level pending 直到全部 recipient terminal；失败 recipient 超过上限进入 dead-letter。
- **B — event-level atomic**：所有 recipient 成功才提交 event-level ack；任一失败则整事件重试，并依赖 recipient dedupe 防止已成功 recipient 重复可见 side effect。
- **禁止未决的 partial silent success**：不能删除 event-level pending 却只记录部分 recipient 成功，也不能把未授权 recipient 的失败改成 broadcast fallback。
验收必须覆盖 A 成功/B 超时、重试后 A 不重复且 B 最终成功、以及 recipient privacy filter 将 recipient 排除的情况。

**本阶段锁定**：#1525、#1528、#1532、#1587、#1590、#1607、#1738。

### 5.6 Stage 5 — publish confirm（唯一出口、确认与取证）

1. 所有通过 route 的 narration 只能从集中 publisher 发往 `AGENT_NARRATE`；publisher 自己包住 Redis publish rejection、超时和 transport error。
2. **[DRAFT-DECISION F04]** 分开记录 `published` 与 `consumed`：前者只表示 publisher 收到 Redis client resolve，后者只表示 server 提供显式消费回执；没有 server ack 时，`published` 不等于 `consumed`，必须保留“已发布、未确认消费”的可查询状态。publish-level 只提交 transport attempt/dedupe claim；cooldown、cursor、pending 删除以及依赖 server 可见性的状态必须等 `consumed`，若没有该回执则保持 pending 或进入明确的待确认状态。
3. publish 失败保留同一 `event_id`/`dedupe_key`/`correlation_id`，按退避和最大尝试次数重试；超过上限进入可观测 dead-letter，不伪造成功。成功/失败回执必须可注入测试，覆盖 client resolve、server consumed、超时和断线的区别。
4. **[DECISION-NEEDED F06]** command+narration 的部分确认不能由单一 event-level ack 推断。P0 必须冻结四格状态矩阵：
   - command 成功 + narration 成功：两个 side effect 各自 terminal success，correlation cursor 是否前进必须由选定矩阵规定。
   - command 成功 + narration 失败：command 不应因 narration 重试而重复；narration 复用原 key 重试或 dead-letter。
   - command 失败 + narration 成功：narration 不应因 command 重试而重复；command 复用原 key 重试或 dead-letter。
   - command 失败 + narration 失败：两者各自按 retry/dead-letter policy 处理，不能提交完整 cursor。
   P0 必须从“每个 side effect 独立 terminal、correlation cursor 仅在两者 terminal success 时前进”与“允许 partial terminal、cursor 拆成两个可查询子游标”中选择一个，并为四种组合固定 cursor、retry、dedupe、dead-letter 结果；四种组合都要有集成测试。
5. 每次转换记录结构化原因（accepted/retried/rejected/routed/published/consumed/dead-letter），指标至少能按 source/kind、失败阶段、队列深度、重试次数、重复命中和 publish latency 聚合。
6. 旧 runtime 直发路径迁移完成后删除；测试或启动扫描发现绕过集中 publisher 的调用即失败。

**本阶段锁定**：#1470、#1475、#1509、#1527、#1538、#1495、#1581、#1675、#1679。

## 6. Phased delivery list

### P0：收口契约与吸收边界

- [ ] 在 draft 转仓库 skeleton 前，复读根 `CLAUDE.md`、`docs/CLAUDE.md` §§五-六并确认 implementation owner 为 `agent`。
- [ ] 建立 29 个 confirmed source issue 的唯一映射表，标明 shared infrastructure owner；其中 4 个是 #1470 的重复证据，另将 §4.4 的 6 个跨簇 follow-up 单独登记为外部 owner，不纳入本 plan acceptance。
- [ ] 盘点所有 Tiandao narration Redis channel、`RuntimeRedis` drain、直接 `publishNarrations` callsite、业务状态/cooldown/cursor 写点。
- [ ] 定义 envelope、event_id、dedupe_key、ordering_key、tick_epoch、ack/state transition 及 pending/dead-letter 语义。
- [ ] 定义 server selector 能接受的 canonical scope/target/dimension contract；若无法复用现有类型，列出最小跨仓库接缝而不直接实现。
- [ ] 收口 F04/F05/F06/F07/F08/F09：每条 decision-needed 必须落为具体协议、选项和测试矩阵后才能进入 P1。

### P1：可靠 ingest 与 pending queue

- [ ] 统一 Redis ingress adapter，禁止 callback 内直接 drain+LLM+publish。
- [ ] 为 chat/NPC/feature narration channel 接入 pending queue/store，支持重启恢复、失败 requeue 和 bounded backpressure。
- [ ] 为每个 channel/kind 注册 source、validator、ordering key、最大 payload/context 和失败策略。
- [ ] 覆盖 #1510、#1518、#1673 的测试：处理失败不能使已摄入事件静默消失，单条 fallback 不影响同批其他事件。

### P2：幂等、顺序与确认前状态提交

- [ ] 实现稳定 event/dedupe/correlation key；retry 不生成新 batch id。
- [ ] 实现 per-player/entity/zone ordering 与全局 bounded concurrency；禁止无界 `void handlePayload` 旁路。
- [ ] 将 cooldown、last status、state cursor、pending command rearm 等副作用迁入 confirm 后提交阶段。
- [ ] 覆盖 #1481、#1486、#1495、#1567、#1581、#1619、#1659、#1675、#1679。

### P3：NarrationV1 validate 与 schema drift 关口

- [ ] 集中运行 NarrationV1 与各 payload validator；补齐 channel/variant validator 注册表。
- [ ] 锁定 style/text/metadata/nullable/numeric/context bounds；禁止任意 style、无界文本和无界 social relationship 结果进入发布。
- [ ] 对 malformed batch 做逐条 fallback/reject；对 server producer 与 agent consumer 做 event-name/schema pin。
- [ ] 覆盖 #1530、#1616、#1654、#1673、#1674、#1746。

### P4：canonical route 与隐私门

- [ ] 统一生成 player/zone/broadcast target，删除 runtime 自拼 prefix/raw id 的路径。
- [ ] route 前置 scope policy、dimension gate、匿名化和 SpiritNiche recipient privacy；缺 zone/target fail-closed。
- [ ] 加 server selector compatibility tests，确保低境界 zone narration、zhenfa deploy 和 player-target narration 真能命中消费者。
- [ ] 覆盖 #1525、#1528、#1532、#1587、#1590、#1607、#1738。

### P5：publish confirm、迁移清理与集成验收

- [ ] 将所有 agent narration runtime 迁移到唯一 publisher；集中处理 rejection、timeout、retry、dead-letter 和 metrics。
- [ ] 验证 `published` 与 `consumed` 分离：Redis client resolve 只推进 publish-level audit/dedupe claim；cooldown、cursor、pending 删除和依赖 server 可见性的状态只有在 `consumed` 回执后推进；无回执时保持待确认状态。失败后相同 key 重放至成功不会重复 side effect。
- [ ] 删除/禁用旧直发路径，添加生产代码扫描或注册表 pin 防回归。
- [ ] 跑 agent 单测、构建、Redis integration/e2e，并用真实 server selector 验证 scope/target/dimension；完成后才由 triage 关联/关闭 source issue。

## 7. Acceptance criteria

### 7.1 功能与可靠性

- 任意已注册 narration event 都能在 telemetry 中看到完整状态链：`ingest → dedupe → validate → route → publish confirm`；不存在只在单测可达、生产无 consumer 的路径。
- Redis publish 被人为拒绝、超时或断线时：调用不会产生未处理 promise rejection；事件保持 pending/retry/dead-letter 之一；不会提前推进 cooldown、cursor、last status 或删除 pending。
- 同一 `event_id` 在成功前重试多次只产生一个可见 narration side effect；retry 不生成新 batch id；成功后重复投递命中 dedupe 并有审计记录。
- 同一 ordering key 的事件按输入顺序发布；不同 key 仍能在明确上限内并发；队列满和处理超时有可观察结果，不会无界增长。
- 进程重启后，未确认事件可恢复；server tick 重置不会错误抑制合法新事件，也不会让旧事件无限重放。

### 7.2 合约、路由与隐私

- 任何发布对象先通过 NarrationV1 和对应 payload validator；非法 style、超长文本、非法 nullable、未知 variant、无界上下文均在 publish 前被拒绝或转 bounded fallback。
- `scope=zone` 的 target 必须是 server selector 可命中的 canonical zone；`scope=player` 必须是 normalized player identity；隐藏/无 target 事件不能隐式变成 broadcast。
- 维度/recipient policy 在 route 结果中可核验；SpiritNiche 未 reveal 的坐标不会广播；匿名化检查覆盖重叠姓名顺序问题。
- agent producer/consumer schema 注册表与 server event producer 对拍通过；缺 validator、死 event name、Rust `Option`/JSON `null` 漂移在测试或启动关口暴露。

### 7.3 测试与运维门禁

- `cd agent/packages/tiandao && npm test` 通过，覆盖每个 pipeline state、错误分支、重试、重复、乱序、空队列、满队列、重启恢复和 route fail-closed。
- `cd agent && npm run build` 通过；若 schema source 发生改动，先重建 `@bong/schema` dist，再验证 tiandao import。
- Redis 集成测试至少覆盖：publish failure/retry、同 key dedupe、批量 malformed item isolation、tick epoch restart、canonical zone/player target、隐私 recipient scope。
- 生产代码中不存在绕过集中 publisher 的 direct `publishNarrations` 或等价 Redis publish 调用；扫描结果与集中注册表一致。
- 29 个 confirmed source issue 都有对应的修复 commit/测试证据；其中 #1475/#1509/#1527/#1538 作为 #1470 的重复证据共享根因映射。§4.4 的 6 个跨簇 follow-up 不计入本 plan acceptance，必须由各自 owner 提供独立 source-to-deliverable 与关闭证据；没有以“已有 skeleton/日志可见”代替实现关闭。

## 8. Open questions（进入 P0 前必须收口）

1. **pending store 选型**：沿用当前 Redis Pub/Sub 周边的内存队列，还是新增 Redis Stream/list/持久 spool？若选择内存，重启恢复验收如何满足；若选择 Redis，需要确定 retention、consumer group 和清理责任。
2. **确认语义**：`AGENT_NARRATE` 的 publish “Redis client resolve”是否足够作为 confirm，还是需要 server 消费回执？F04 已冻结 `published`/`consumed` 两级记录，但 P0 仍必须根据 server 能否提供回执，明确没有回执时 pending/待确认状态的保留与终态。
3. **命名与 owner**：统一 pipeline 应落在 `agent/packages/tiandao/src/` 哪个模块/类名；`RuntimeRedis` 现有 API 是适配层还是应被替换？需要避免新建第二套 runtime abstraction。
4. **幂等键来源**：哪些 server/agent event 已有稳定 event id；缺失者由哪个 producer 生成？`source_tick + entity` 在同 tick 多事件时如何避免碰撞；是否需要跨重启 `tick_epoch` 持久化。
5. **顺序粒度**：默认按 player、entity、zone 还是 source channel 串行？同一事件同时产生 command 和 narration 时，二者的 ordering/ack 是否共用 key。
6. **LLM failure 终态与责任边界 [DECISION-NEEDED F08]**：四类 failure 必须逐类选择责任主体和固定协议，不能共用模糊 fallback：
   - timeout：选“有限重试后 bounded fallback 发布”或“有限重试后 rejected/dead-letter”；
   - transport rejection/disconnect：选“按相同 key 重试”或“达到上限后 dead-letter”，不得把 client resolve 当成功；
   - malformed JSON：选“validator 生成固定上限 unknown fallback”或“直接 rejected”；
   - schema-invalid response：选“逐项 fallback”或“逐项 rejected/dead-letter”。
   P0 必须为每类写明最大 retry、退避、fallback 最大长度/字段、最终 state 和 pending 是否保留，并用 fake LLM/断线集成测试锁定。
7. **路由权威**：canonical target/dimension 是否完全复用 server `narration_selector` contract；若 server 当前无法回执“无人命中”，route 阶段如何在 agent 侧做兼容性 pin 而不复制一份 selector。
8. **隐私与 fan-out [DECISION-NEEDED F05]**：同一事件对不同 recipient 需要不同文案/target 时，P0 必须在 per-recipient durable 与 event-level atomic 中选择，并固定 recipient key、ack、retry、dead-letter、event pending 删除条件；验收覆盖部分成功、重试和 privacy filter。
9. **command+narration partial confirm [DECISION-NEEDED F06]**：必须从“每个 side effect 独立 terminal、correlation cursor 仅在两者 terminal success 时前进”与“允许 partial terminal、cursor 拆成两个可查询子游标”中选择一个，固定四种成功/失败组合的 cursor、retry、dedupe、dead-letter 结果；不能以单一 event-level ack 掩盖部分确认。
10. **backpressure 与优先级 [DECISION-NEEDED F09]**：现有证据不足以推出数值，不能伪造默认值。P0 必须从“固定 capacity + global concurrency + priority tiers + 满队列 reject/dead-letter”与“固定 capacity + 低优先级合并/淘汰但保留隐私/教学事件”中选择一套，冻结具体配置字段和值、优先级、terminal policy，以及 transition telemetry 的唯一 key、字段（event_id、state、reason、attempt、queue_depth、timestamp/tick_epoch、source/kind）和查询方式；验收断言深度不超过上限、满队列结果和指标值。
11. **已有 feature skeleton 的迁移顺序**：`poi novice`、`anqi charged`、各类 narration target-prefix/runtime bridge skeleton 是否在 P1/P2 作为试点迁移，还是先完成共享管道后统一迁移；必须避免主循环与独立 runtime 双发。
12. **跨簇 schema follow-up 的边界**：§4.4 的 #1728/#1734 不属于本 plan source ledger 或 P-phase lock；它们的 schema source/generated artifacts、owner 和 closure evidence 必须由对应 schema/registry follow-up 单独冻结。本 plan 只负责对未知/不兼容输入 fail-closed，不借此扩大实现范围。
13. **source issue 关闭门**：只有共享管道完成后再逐项复验，还是允许某些同根因 issue 在 feature follow-up PR 中先关闭？需要统一 triage 证据格式，避免“关联 PR”被误写成“已修复”。

## 9. 风险与迁移注意

- **双发风险**：主循环 drain、独立 narration runtime、共享 pipeline 同时订阅会产生重复发布；迁移每个 channel 时必须有唯一 consumer claim 和重复 pin。
- **批量启动刷屏**：ingest 恢复或 startup snapshot 可能一次性进入大量 POI/状态事件；优先级、合并和上限必须在 P2/P5 验收，不得用无限丢弃掩盖压力。
- **语义误路由**：将缺失 zone 默认成 broadcast 会把隐藏事件扩大泄露；route policy 应 fail-closed，测试必须覆盖缺字段和未知 target。
- **伪 exactly-once**：Redis publish resolve 不等同于 server 已展示；文档和指标必须区分 accepted、published、consumed/observed（若没有消费回执则明确缺口）。
- **跨轨文件冲突**：本 plan 主体限定 agent；若要改 server selector、schema generated artifact 或 client handler，应先列接缝并拆独立 PR，不能在共享 pipeline PR 中顺手扩 scope。
- **失败重试放大**：没有稳定 key 或退避上限时，publish failure 会制造重复 narration 和队列爆炸；P2/P5 必须先于大规模 runtime 迁移。
