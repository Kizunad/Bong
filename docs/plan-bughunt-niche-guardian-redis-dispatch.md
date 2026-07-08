# plan-bughunt-niche-guardian-redis-dispatch

## Bug 摘要

`bong:social/niche_intrusion` Redis channel 被 server 复用承载三种 payload：

1. `NicheIntrusionEventV1`
2. `NicheGuardianFatigueV1`
3. `NicheGuardianBrokenV1`

但 Tiandao 侧两个订阅该 channel 的 runtime 都只按 `NicheIntrusionEventV1` 解析。结果是灵龛守护疲劳 / 破碎事件虽已从 server 发布到 agent channel，却在 agent 入口被当作 invalid intrusion payload 拒绝，无法进入政治叙事或散修叙事。

## 实际游玩体验影响

对实际游玩体验的影响：灵龛被入侵时，参与者仍会收到局部 `server_data`、HUD 和音效反馈，所以这不是核心玩法完全不可见；严重度偏 P2。但“守护消耗到几次”“守护被打碎”这两类更关键的防线变化不会进入天道叙事层。

玩家实际会感到灵龛防御变化只在当事人侧短促发生，世界没有对应反应：附近散修不会形成“守护被磨损 / 击破”的旁白，政治叙事也不会把破防事件纳入声名、侵扰、报复氛围。对长期据点玩法而言，灵龛守护从“有社会后果的防线”退化成局部状态提示，削弱入侵风险的可感知性和追责感。同时，agent runtime 会把正常生产事件记为 contract rejected，污染日志和监控统计。

## 证据定位

- `server/src/social/mod.rs:682-732` 会把 `NicheIntrusion` / `NicheGuardianFatigue` / `NicheGuardianBroken` 三类事件都转换为 `server_data` 发给参与者。
- `server/src/social/mod.rs:3294-3398` 同时把三类事件都 publish 到 Redis outbound，其中 fatigue 和 broken 分别走 `RedisOutbound::NicheGuardianFatigue` / `RedisOutbound::NicheGuardianBroken`。
- `server/src/network/redis_bridge.rs:1200-1241` 将 `NicheIntrusionEventV1`、`NicheGuardianFatigueV1`、`NicheGuardianBrokenV1` 全部发布到同一个 `CH_SOCIAL_NICHE_INTRUSION`。
- `agent/packages/schema/src/social.ts:201-231` 已定义三种 TS schema：`NicheIntrusionEventV1`、`NicheGuardianFatigueV1`、`NicheGuardianBrokenV1`，说明 guardian payload 不是 server 私有结构。
- `agent/packages/tiandao/src/political-narration.ts:303-308` 收到 `SOCIAL_NICHE_INTRUSION` 后只调用 `parseNicheIntrusion`。
- `agent/packages/tiandao/src/political-narration.ts:355-358` `parseNicheIntrusion` 只用 `validateNicheIntrusionEventV1Contract` 校验；guardian payload 缺 `niche_pos/items_taken/taint_delta`，会被拒绝。
- `agent/packages/tiandao/src/scattered-cultivator-narration.ts:64-68` 也订阅 `SOCIAL_NICHE_INTRUSION`。
- `agent/packages/tiandao/src/scattered-cultivator-narration.ts:131-140` 同样只按 `NicheIntrusionEventV1` 校验，不识别 guardian fatigue/broken。

## 触发路径

1. 玩家或 NPC 入侵灵龛，触发守护疲劳或守护破碎事件。
2. server 在 `publish_social_events` 中把事件写入 `RedisOutbound::NicheGuardianFatigue` 或 `RedisOutbound::NicheGuardianBroken`。
3. Redis bridge 将 payload 发布到 `bong:social/niche_intrusion`。
4. `PoliticalNarrationRuntime` 和 `ScatteredCultivatorNarrationRuntime` 都收到该 channel 消息。
5. 两个 runtime 都用 `NicheIntrusionEventV1` schema 校验 guardian payload。
6. 校验失败后事件被记为 contract rejected，不产生 `AGENT_NARRATE`。

## 反方审查记录

- Round 1：PASS（候选收敛）。第 1 轮 subagent 提出多个 agent/schema 漂移方向；主线排除了明确 future + server 兜底的 territory channel，以及过宽、易与 #1061 混淆的全量 C2S/S2C 漂移，将范围收敛到同一 Redis channel 内多 payload 分发缺口。
- Round 2：PASS，建议立案但降级为 P2。反方确认 server 合法生产 Redis payload 会被 `PoliticalNarrationRuntime` 与 `ScatteredCultivatorNarrationRuntime` 当作 `NicheIntrusionEventV1` 契约错误拒绝；同时指出 client 已有 `SocialServerDataHandler` / `NicheIntrusionAlertHandler` 处理本地 HUD、事件流与音效，所以不是核心玩法完全无反馈。反方还指出旧设计文档对 agent narration 更偏“追凶 / 抄家事件”，不明确要求 guardian fatigue/broken 必须生成旁白；但这不改变“合法生产 payload 被记为 contract error”的事实。

## Skeleton Fix Plan

- [ ] 在 agent schema 层补齐 shared channel 的联合解析语义：`SOCIAL_NICHE_INTRUSION` 不只承载 `NicheIntrusionEventV1`，还承载 `NicheGuardianFatigueV1` / `NicheGuardianBrokenV1`。
- [ ] 在 `PoliticalNarrationRuntime` 中按 payload 形状或显式 discriminant 分派三类 niche 事件；guardian fatigue/broken 不应再走 `parseNicheIntrusion`。若产品决策暂不播报 guardian，也必须显式 ignore，不能计入 `rejectedContract`。
- [ ] 在 `ScatteredCultivatorNarrationRuntime` 中同样分派 guardian payload；至少 broken 事件应产生可见的 zone/broadcast 叙事，fatigue 可按 `charges_remaining` 做降噪。若选择不叙事，也应作为已识别事件忽略。
- [ ] 评估是否需要给 guardian Redis payload 增加 `event` / `type` discriminant；若新增，必须同步 Rust schema、TS schema、samples、server_data 兼容策略和 Redis bridge 测试。
- [ ] 避免把 guardian payload 改发到全新 channel 作为首修，除非同时迁移两个 runtime 和 channel pin；当前 bug 的最小修复是让现有 channel 的消费者识别现有三形态。

## 验收测试计划

- `agent/packages/schema`：新增或扩展 social schema 测试，锁定 `NicheGuardianFatigueV1` / `NicheGuardianBrokenV1` 是 `SOCIAL_NICHE_INTRUSION` 的合法 Redis payload 之一。
- `agent/packages/tiandao`：给 `PoliticalNarrationRuntime` 增加 guardian fatigue/broken 正例，断言不会计入 `rejectedContract`，且 broken 事件会 publish `AGENT_NARRATE`。
- `agent/packages/tiandao`：给 `ScatteredCultivatorNarrationRuntime` 增加同 channel 多 payload 回归，确保 `NicheIntrusionEventV1` 老路径仍正常，guardian payload 不再被 `NicheIntrusionEventV1` schema 拒绝。
- `server/`：补 Redis bridge pin，明确 `RedisOutbound::NicheGuardianFatigue/Broken` 发布到 `CH_SOCIAL_NICHE_INTRUSION` 时下游 agent schema 有对应 positive sample。
- schema 改动后必须重建 dist：`cd agent && npm run build -w @bong/schema`。

## 风险

- `SOCIAL_NICHE_INTRUSION` 已被多个 runtime 订阅；修复时要避免为同一个 guardian event 生成重复旁白。
- 如果引入 discriminant 字段，旧 Redis payload 与 server_data payload 的形态差异要明确处理，避免把 client `type: niche_guardian_broken` 误套到 Redis schema 上。
- guardian fatigue 可能高频触发，应有节流或只在关键档位播报；guardian broken 则应无条件可见，因为它代表灵龛防线被击穿。
