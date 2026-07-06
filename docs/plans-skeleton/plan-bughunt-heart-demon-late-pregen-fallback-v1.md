# plan-bughunt-heart-demon-late-pregen-fallback-v1

## 一句话结论

心魔劫 `HeartDemonPregenRequestV1` 已提前发往 agent，但 client 侧 `heart_demon_offer` 只在进入 `HeartDemon` 阶段那一次 `TribulationWaveCleared` 上屏；若 agent 回包晚于这一次发包，server 会先下发默认 fallback，随后把晚到的 `PendingHeartDemonOffer` 静默插进 ECS，却**再也没有第二次补发/替换路径**，导致玩家整场心魔面板永久看不到 agent 生成的个性化选项。

## 复现路径

1. 玩家进入渡虚劫，触发 `TribulationAnnounce`；server 在 [server/src/cultivation/tribulation.rs](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-cq/server/src/cultivation/tribulation.rs:3906) `publish_heart_demon_pregen_requests()` 立刻发出 `HeartDemonPregenRequestV1`。
2. 人为制造 agent 侧慢回包（LLM 慢、Redis 堵塞、runtime 重连后首条慢、或直接延迟发布 `bong:heart_demon_offer`），让回包晚于玩家进入 `HeartDemon` 阶段。
3. server 进入 `HeartDemon` 时不会等待 pregen，就在 [server/src/cultivation/tribulation.rs](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-cq/server/src/cultivation/tribulation.rs:1302) `should_enter_heart_demon_phase()` 直接放行；随后 [server/src/network/tribulation_heart_demon_offer_emit.rs](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-cq/server/src/network/tribulation_heart_demon_offer_emit.rs:15) 仅消费这一次 `TribulationWaveCleared` 并调用 `heart_demon_offer_for_client()`。
4. 因晚到时 `PendingHeartDemonOffer` 还不存在，`heart_demon_offer_for_client()` 在同文件 [46-57](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-cq/server/src/network/tribulation_heart_demon_offer_emit.rs:46) 走 `default_heart_demon_offer()` fallback，把默认“守本心 / 斩执念 / 无解”面板发给玩家。
5. agent 回包之后，server 只会在 [server/src/network/mod.rs](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-cq/server/src/network/mod.rs:2463) `RedisInbound::HeartDemonOffer` 分支里把 payload 插成 `PendingHeartDemonOffer`；这里**没有**再触发任何 S2C emit / close+reopen / replace 事件。
6. 玩家最终整场都停留在默认心魔面板；晚到的 agent 生成内容完全不可见。

## 根因链路

1. **请求提前发出，但协议没有“迟到后补发”语义**：
   `publish_heart_demon_pregen_requests()` 只是单向请求；后续没有 request/response deadline 协议字段，也没有“若晚到则重新推面板”的 server 逻辑。
2. **进入心魔阶段不等待 pregen**：
   `should_enter_heart_demon_phase()` 在 `next_wave == DUXU_HEART_DEMON_WAVE` 时直接返回 true，不要求 `PendingHeartDemonOffer` 已就绪。
3. **client 发包点只有一次**：
   `emit_heart_demon_offer_payloads()` 的输入仅是 `EventReader<TribulationWaveCleared>`；这意味着只会在波次切换那次上屏，晚到 offer 不会触发第二次发送。
4. **late offer 只落缓存，不落客户端**：
   `RedisInbound::HeartDemonOffer` 只 `insert(PendingHeartDemonOffer)`，没有任何伴随 emit。
5. **现有测试只覆盖“早到 pregen 被采用”，未覆盖“晚到 pregen 永不补发”**：
   [server/src/network/tribulation_heart_demon_offer_emit.rs](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-cq/server/src/network/tribulation_heart_demon_offer_emit.rs:276) 仅测 `pregen_heart_demon_offer_is_used_when_trigger_matches()`，前提是 `PendingHeartDemonOffer` 在发包前已经存在。

## 这个 bug 对实际游玩体验的影响

- 玩家在最需要“心魔按自己经历/近传记/气色定制”的心魔劫界面里，经常只能看到通用默认文案，体感像 agent 系统没生效。
- 这不是“文案略差一点”，而是 **agent runtime 的整条 pregen 价值被时序吞掉**：LLM 输出、个性化 flavor、最近生平映射，全都不会抵达 client。
- 由于默认 fallback 合法且 UI 正常打开，这个问题很隐蔽：不会 crash、不会红 toast，只会长期表现为“心魔总是那三张模板卡”。

## 影响面

- 直接影响 `HeartDemonPregenRequestV1 -> HeartDemonOfferDraftV1 -> PendingHeartDemonOffer -> HeartDemonOfferV1(S2C)` 整条链。
- 触发条件宽：任何 agent/Redis/LLM 延迟超过“请求发出到进入 HeartDemon 的窗口”即可复现，不要求异常输入。
- 风险主要落在真实服、弱网、runtime 重连、模型排队、首条冷启动等时序波动环境；本地快机单测不容易看出来。

## 修复建议

1. 给 `RedisInbound::HeartDemonOffer` 增加“若目标实体当前已在 `TribulationPhase::HeartDemon`，则立即二次 emit/replace S2C”的补发路径。
2. 或者把 `emit_heart_demon_offer_payloads()` 的触发源从“仅 `TribulationWaveCleared`”改成“`TribulationWaveCleared` + late `PendingHeartDemonOffer` 到达”双入口。
3. 若产品上允许 fallback 先开屏，再被 agent 文案替换，则需要显式 close/update 协议；否则就要在进入 `HeartDemon` 前等待 pregen 或引入短超时闸门。

## 验收抓手

- 构造一条心魔 pregen 回包**晚于** `TribulationWaveCleared` 的 e2e：先让 client 收到默认 offer，再注入合法 `RedisInbound::HeartDemonOffer`，断言当前实现不会二次更新；修复后应能补发或替换。
- 保留现有“pregen 早到即采用”的测试，再新增“late pregen arrives after fallback emit”测试，确保不会回归成永久默认卡。
- 加一条日志/metric 抓手：统计 `heart_demon_offer` 的 fallback 发包次数、late pregen 命中次数、late pregen 成功补发次数。

## 两轮反方裁决（本会话无 subagent，退化为人工裁决并在 PR 如实披露）

### 第一轮反方

**反方论点**：`HeartDemonPregenRequestV1` 在 [tribulation.rs:3906-3942](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-cq/server/src/cultivation/tribulation.rs:3906) 已经足够早发出，正常情况下 agent 回包应该赶得上；这是性能/部署问题，不是代码 bug。

**驳回理由**：代码层没有任何“必须等回包”或“晚到后补发”的协议保障；`should_enter_heart_demon_phase()` 明确不等 pregen，`emit_heart_demon_offer_payloads()` 也只消费一次事件。只要存在可达的正常时序抖动，这条链就会稳定退化成永久 fallback，属于确定性的时序缺口，不是纯运维问题。

### 第二轮反方

**反方论点**：即便晚到，`PendingHeartDemonOffer` 也已插入 ECS，也许后续 tick / 下一 wave / 现有 UI 刷新会自动把 agent offer 展示出来。

**驳回理由**：现有代码没有任何基于 `PendingHeartDemonOffer` 变更的 emit 路径；`emit_heart_demon_offer_payloads()` 唯一输入是 `TribulationWaveCleared`，[network/mod.rs:2463-2489](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260705-cq/server/src/network/mod.rs:2463) 的 late insert 既不发 event，也不直接给 client 发包。现有测试同样只覆盖 pregen 早到场景，没有任何“晚到后自动刷新”证据。
