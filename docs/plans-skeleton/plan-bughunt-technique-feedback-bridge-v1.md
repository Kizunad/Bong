# BugHunt: 战斗功法学会/精通 deferred 叙事桥未闭环

> 来源：BugHunt 线程 E9（agent-schema 第九轮）。本文件是 Skeleton Fix Plan，只记录高置信 bug 与修复验收方案；不消费、不归档任何 plan。

## Bug 摘要

`plan-woliu-v4` 已把战斗功法学会/精通定义为跨端反馈事件，并明确承诺 `tiandao` 订阅 `bong:technique/learned` 生成 narration；当前实现只在 server 内 emit `TechniqueLearnedEvent` / `TechniqueMasteredEvent`，没有 RedisOutbound 生产者、没有 agent schema payload、`REDIS_V1_CHANNELS` 未包含 technique 四通道，也没有 Tiandao technique narration runtime。

这不是功法学习 gameplay 本体失效：玩家仍会学会招式、熟练度仍会增长。问题是承诺的 agent/schema/runtime feedback bridge 没闭环，导致实际游玩中“学会/练满战斗功法”没有天道观察和叙事反馈。

## 对实际游玩体验的影响

- 玩家读涡流残卷、首次受击自学闪避、观摩/传功学招后，状态已更新，但不会出现计划承诺的“某人于此处悟得涡流之法”类天道叙事。
- 涡流招式练满时 `TechniqueMasteredEvent` 会发出，但没有 agent 或 HUD/成就 reader 消费，玩家只感到数值暗中变化，缺少关键成就反馈。
- 调试时更容易误判：schema 已有 `bong:technique/*` channel 常量，finished plan 也宣称 agent 已覆盖，但线上 Redis/Tiandao 实际无消息链路。

## 证据定位

- `docs/finished_plans/plan-woliu-v4.md:66-75`：出料列出 `TechniqueLearnedEvent` / `TechniqueMasteredEvent`、`bong:technique/learned` / `scroll_read` / `mastered`，并承诺 agent `tiandao` 订阅 `bong:technique/learned` 生成 narration。
- `docs/finished_plans/plan-woliu-v4.md:721-722`：跨仓库核验只写了 `CH_TECHNIQUE_*` 与 schema surface，但没有真实 runtime 证据。
- `agent/packages/schema/src/channels.ts:298-308`：定义 `TECHNIQUE_SCROLL_READ` / `TECHNIQUE_LEARNED` / `TECHNIQUE_MASTERED` / `TECHNIQUE_PROFICIENCY_UP`。
- `agent/packages/schema/src/channels.ts:470-484`：`REDIS_V1_CHANNELS` 从 `SKILL_SCROLL_USED` 直接跳到 `SPIRIT_EYE_*`，未包含 technique 四通道。
- `server/src/network/redis_bridge.rs:194-197` 与 `:965-976`：RedisOutbound 与序列化 arm 只覆盖 `SkillXpGain` / `SkillLvUp` / `SkillCapChanged` / `SkillScrollUsed`，后面直接进入 NPC arm；没有 Technique arm。
- `server/src/cultivation/technique_scroll.rs:19-23`、`server/src/network/client_request_handler.rs:3055-3062`：残卷学习成功会 emit `TechniqueLearnedEvent`。
- `server/src/cultivation/first_hit_dash.rs:84`：首次受击自学路径也会 emit `TechniqueLearnedEvent`。
- `server/src/cultivation/technique_proficiency.rs:171-174`：练满时 emit `TechniqueMasteredEvent`。
- `agent/packages/tiandao/src/main.ts:162-288`：auxiliary runtime 启动清单没有 technique narration runtime；全 `agent/packages/tiandao/src` 搜 `TECHNIQUE_` / `bong:technique` 无命中。
- `server/src/test_coverage_guards.rs:118-126`：仓库已把 `TechniqueLearnedEvent` / `TechniqueMasteredEvent` 标为 `DeferredFollowUp`，理由正是当前没有成就/叙事 reader。该 guard 只登记缺口，没有修复 plan。

## 触发路径

1. 玩家通过残卷研读、首次受击自学、观摩或 NPC 传功学会战斗功法。
2. server 写入 `KnownTechniques`，并 emit `TechniqueLearnedEvent`。
3. 由于没有 EventReader 将该 Bevy event 转成 `RedisOutbound::TechniqueLearned`，`bong:technique/learned` 不会发布。
4. Tiandao 没有订阅该通道的 runtime，即使补上 Redis 消息也不会生成 narration。
5. 玩家只看到本地状态变化，缺少承诺的天道叙事/反馈。

练满路径同理：战斗施放让熟练度到达满级后 emit `TechniqueMasteredEvent`，但没有 Redis/agent 消费链路。

## 反方审查记录

- Round 1 PASS：反方确认没有现有消费者，不重复 #970/#979/#995/#1006/#1011/#1017/#1023/#1031，也未在 `docs/plan-*.md` / `docs/plans-skeleton` 找到同题 plan。最强削弱点是 `test_coverage_guards.rs` 已将其标为 `DeferredFollowUp`。
- Round 2 PASS：反方确认它仍符合本轮 agent/schema/runtime bridge 漂移范围；`plan-test-coverage-guards-v1` 只是 triage 白名单，不是可执行修复计划。结论要求降级表述：这是已知 deferred bridge gap 的具体化，不是核心功法系统阻断。

## Skeleton Fix Plan

### P0 - schema 契约补齐

- 新增 `TechniqueLearnedPayloadV1`、`TechniqueMasteredPayloadV1`，必要时补 `TechniqueScrollReadPayloadV1`。
- 将 `CHANNELS.TECHNIQUE_SCROLL_READ` / `TECHNIQUE_LEARNED` / `TECHNIQUE_MASTERED` 加入 `REDIS_V1_CHANNELS`。
- 为 payload 加正反 sample、generated schema、schema pin 测试。

### P1 - server Redis bridge

- 增加 `RedisOutbound::TechniqueLearned` / `TechniqueMastered`。
- 增加 EventReader system，把 `TechniqueLearnedEvent` / `TechniqueMasteredEvent` 转为 Redis payload。
- 对残卷、首次受击自学、观摩、传功、练满路径补最小集成测试：event 发出后 RedisOutbound 队列有对应 technique payload。

### P2 - Tiandao narration runtime

- 新增 `TechniqueNarrationRuntime`，订阅 `bong:technique/learned` 与 `bong:technique/mastered`。
- 输出 scope 应优先 zone/player 可路由；没有 zone 的源头需要明确 fallback，避免 narration 静默丢弃。
- 在 `main.ts` 启动 runtime，补 connect/parse/reject/publish 测试。

### P3 - 防回归

- `REDIS_V1_CHANNELS` 对 technique 通道加 pin 测试。
- `test_coverage_guards.rs` 中移除或改写 `TechniqueLearnedEvent` / `TechniqueMasteredEvent` 的 deferred 白名单，要求真实 reader 存在。
- Tiandao 测试覆盖 invalid payload 不发布、valid learned/mastered payload 发布 narration。

## 验收测试计划

- `agent/packages/schema`：`npm test`，并覆盖 technique payload sample 与 `REDIS_V1_CHANNELS` pin。
- `agent/packages/tiandao`：`npm test`，覆盖 runtime 订阅、合法 payload narration、非法 payload reject。
- `server/`：`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`，至少包含 technique event -> RedisOutbound 的专项测试。
- 联调：玩家读一张涡流残卷学会新招后，Redis 出现 `bong:technique/learned`，Tiandao 发布一条可见 narration；涡流招式练满后同理出现 mastered narration。

## 风险

- 不要把修复范围扩大到功法学习数值、残卷掉落、client HUD 重做；本 plan 只补 agent/schema/runtime feedback bridge。
- `TechniqueLearnedEvent` 当前携带 Bevy `Entity`，转 Redis 时必须映射到稳定 player/char id，并决定 zone 来源；缺 zone 时需要明确 fallback。
- 精通事件频率可能受高频战斗影响，需要 dedupe/throttle，避免练满边界重复播报。
- `TechniqueScrollReadEvent` 包含失败分支，若 P0 一并接入 scroll_read，需要避免把失败尝试都当作“学会”叙事。
