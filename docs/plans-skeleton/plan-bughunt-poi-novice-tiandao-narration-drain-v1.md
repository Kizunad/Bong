# BugHunt: 新手 POI 天道叙事事件无人消费

## Bug 摘要

`plan-poi-novice-v1` 已把 `PoiSpawnedEventV1` / `TrespassEventV1` 定义为 agent narration 触发事件，但 Tiandao 生产 runtime 只在 `RedisIpc` 中订阅、解析、缓存 `bong:poi_novice/event`，没有 drain/独立 runtime 把事件渲染后发布到 `AGENT_NARRATE`。

结果是 `renderPoiSpawnedNarration` / `renderTrespassNarration` 只在单测中可达，真实服务器发出的新手 POI 发现提示、残卷发现提示、屠村警告叙事不会进入游戏。

本 bug 与开放 PR #935 不重复：#935 记录的是散修聚居点屠村后一周拒交易的 server gameplay 主链断开；本 plan 只记录 agent-schema/tiandao runtime 层的 narration 发布断链，不把交易门禁未生效作为本轮问题。

## 实际游玩体验影响

- 玩家进入初醒原后，新手 POI 被 worldgen/server 动态加载或散落生成，天道侧本应给出炼器、炼丹、采药、交游、搏杀入口的 zone perception 提示；实际没有任何 `AGENT_NARRATE` 输出，玩家只能靠偶然碰撞或外部调试信息发现这些教学入口。
- 残卷藏匿点本应使用独立的 `poi_novice.scroll_found` 文案提示“残页可学之法”，实际只被 RedisIpc 缓存，不会播报，削弱残卷作为早期技能入口的可发现性。
- 散修聚点 trespass 事件本应至少播出“一周拒交易”的系统警告；实际 agent 侧不发布该警告。注意：拒交易 gameplay 是否真正生效由 #935 覆盖，本 plan 仅覆盖警告叙事不出现。
- 调试体验也受影响：测试里能看到 `onPoiNoviceEvent` 回调和 renderer 通过，生产运行却没有消费路径，容易误判 POI narration 已接通。

## 证据定位

- 设计/完工证据明确承诺 agent narration 触发：
  - `docs/finished_plans/plan-poi-novice-v1.md:49`：`PoiSpawnedEventV1` / `TrespassEventV1` 标注为 agent narration 触发。
  - `docs/finished_plans/plan-poi-novice-v1.md:148`：P2 写明 `agent narration: PoiSpawnedEventV1 / TrespassEventV1 触发`。
  - `docs/finished_plans/plan-poi-novice-v1.md:249`：模块表列出 `Agent narration`，目标文件为 `agent/packages/tiandao/src/poi-narration.ts`。
  - `docs/finished_plans/plan-poi-novice-v1.md:304`、`:328`：Finish Evidence 只落到 `RedisIpc.onPoiNoviceEvent` 和两个 renderer，未出现生产 publish 路径。
- server 侧确实产生并发布事件：
  - `server/src/world/poi_novice.rs:243-255`：`PoiNoviceLoader` 对加载出的站点发送 `PoiSpawned`。
  - `server/src/world/poi_novice.rs:623-682`：散落遗缴生成后也发送 `PoiSpawned`。
  - `server/src/network/poi_novice_bridge.rs:13-35`：`PoiSpawned` 被转成 `RedisOutbound::PoiSpawned`。
  - `server/src/network/poi_novice_bridge.rs:40-56`：`TrespassEvent` 被转成 `RedisOutbound::PoiTrespass`。
  - `server/src/network/redis_bridge.rs:1143-1159`：两类 outbound 都发布到 `CH_POI_NOVICE_EVENT`。
- Tiandao 侧只缓存，不消费：
  - `agent/packages/tiandao/src/redis-ipc.ts:297-299`：收到 `POI_NOVICE_EVENT` 后进入 `handlePoiNoviceEventMessage`。
  - `agent/packages/tiandao/src/redis-ipc.ts:424-448`：按 `poi_spawned` / `trespass` 验证并 `recordPoiNoviceEvent`。
  - `agent/packages/tiandao/src/redis-ipc.ts:454-460`：事件只进入 `latestPoiNoviceEvents` 并触发 callback。
  - `agent/packages/tiandao/src/redis-ipc.ts:758`：RedisIpc 订阅 `POI_NOVICE_EVENT`。
  - `agent/packages/tiandao/src/redis-ipc.ts:823-829`：只暴露 `getLatestPoiNoviceEvents` / `onPoiNoviceEvent`，没有 `drainPoiNoviceEvents`。
  - `agent/packages/tiandao/src/runtime.ts:145-159`：`RuntimeRedis` drain 接口列出 rat、npc death、TSY UI、price、weather、ecology、chat，没有 POI novice。
  - `agent/packages/tiandao/src/runtime.ts:1269-1420`：主循环 drain chat/npc death/ecology/economy/weather/locust/UI/qi-color/seasonal，没有 POI novice 处理。
  - `agent/packages/tiandao/src/main.ts:170-328`：独立 narration runtime 启动清单没有 POI novice runtime。
  - `agent/packages/tiandao/src/redis-ipc.ts:928-939`：`publishNarrations` 可以发布到 `AGENT_NARRATE`，但 POI novice 事件从未进入这个出口。
- renderer 和测试停在非生产路径：
  - `agent/packages/tiandao/src/narration/templates.ts:59-83`：模板和 renderer 存在。
  - `agent/packages/tiandao/tests/poi-novice-narration.test.ts:18-54`：只测 renderer 输出。
  - `agent/packages/tiandao/tests/redis-ipc.test.ts:970-1004`：只测 RedisIpc 能观察/缓存 POI novice 事件，没有断言 runtime 发布 narration。
  - `grep -R "getLatestPoiNoviceEvents|onPoiNoviceEvent|renderPoiSpawnedNarration|renderTrespassNarration" agent/packages/tiandao/src` 只命中定义与模板，无生产调用。

## 触发路径

1. server 启动或地表遗缴散落生成 `PoiSpawned`。
2. `server/src/network/poi_novice_bridge.rs` 构造 `PoiSpawnedEventV1`，经 `RedisOutbound::PoiSpawned` 发布到 `bong:poi_novice/event`。
3. Tiandao `RedisIpc` 收到消息，验证为 `poi_spawned`，写入 `latestPoiNoviceEvents`。
4. `runRuntime` 没有 `drainPoiNoviceEvents`，`main.ts` 也没有独立 POI runtime。
5. `renderPoiSpawnedNarration` 不会被调用，`publishNarrations` 不会发 `AGENT_NARRATE`。

`trespass` 同理：server bridge 发布 `TrespassEventV1`，Tiandao 缓存后无人消费，`renderTrespassNarration` 不会进入 `AGENT_NARRATE`。

## 反方审查记录

### Round 1

反方任务：尽力证明候选不是 bug，重点查生产消费、server 兜底、finished plan 是否只承诺观察、开放 PR 去重。

结论：反方未通过。反方确认 `RedisIpc` 订阅并缓存 POI novice；`onPoiNoviceEvent` / `getLatestPoiNoviceEvents` / renderer 在生产代码无调用；`RuntimeRedis` 与 `runRuntime` 没有 POI drain；`main.ts` 没有独立 POI runtime；server 侧没有 `PendingGameplayNarrations` 兜底；开放 PR 只找到 #935，主题不同。

### Round 2

反方任务：收窄后继续挑战是否与 #935 完全重复、是否只是未承诺发布、是否 `main.ts` 有绕过 `runRuntime` 的独立订阅 runtime。

结论：反方未通过。反方认为 #935 是 server gameplay 拒交易断链，本候选是 agent runtime narration 断链；finished plan 原文写明 agent narration 触发，不只是 observer/template；`main.ts` 启动清单无 POI novice runtime；`publishNarrations` 出口存在但没有 POI novice 输入。

## Skeleton Fix Plan

### P0：补齐 RedisIpc drain 契约

- 在 `agent/packages/tiandao/src/runtime.ts` 的 `RuntimeRedis` 增加 `drainPoiNoviceEvents?(): PoiNoviceRuntimeEventV1[]`。
- 在 `agent/packages/tiandao/src/redis-ipc.ts` 实现 `drainPoiNoviceEvents()`，返回当前 `latestPoiNoviceEvents` 并清空队列。
- 保留 `getLatestPoiNoviceEvents` / `onPoiNoviceEvent` 作为观察 API，避免破坏已有测试。

### P1：补生产消费与发布

- 在 `agent/packages/tiandao/src/runtime.ts` 增加 `processPoiNoviceEvents`：
  - drain `poi_spawned` / `trespass`。
  - 分别调用 `renderPoiSpawnedNarration` / `renderTrespassNarration`。
  - 通过 `redis.publishNarrations` 发布到 `AGENT_NARRATE`。
- `sourceTick` 口径优先使用当前 fresh `WorldStateV1.tick`；若事件在 state 前到达，等下一次 fresh state 一并 drain，避免无 tick metadata。
- correlationId 建议使用 `poi-novice:${state.tick}`，日志中输出事件数量与 kind 分布。

### P2：避免刷屏与重复

- 启动加载可能一次性发多个 `PoiSpawned`。修复时需要定义合并/限流策略：
  - 同一 tick、同一 zone 的普通 POI 可合并为一条 zone perception。
  - `scroll_hidden` 保留独立残卷文案，避免教学入口被合并吞掉。
  - `trespass` 不与普通 spawned 合并，优先发布 system warning。
- 如果未来新增独立 `PoiNoviceNarrationRuntime`，必须删除主循环 drain 或加去重，禁止双发。

### P3：测试补洞

- `redis-ipc.test.ts`：新增 `drainPoiNoviceEvents` 清空队列测试，覆盖 `poi_spawned` 与 `trespass` 顺序。
- `runtime.test.ts` 或新增专门测试：构造 fake `RuntimeRedis` 返回 POI novice 事件，断言 `processPoiNoviceEvents` 调用 `publishNarrations`，且 renderer 文案与 `POI_NOVICE_NARRATION_TEMPLATES` 对齐。
- 增加“不消费不重复发布”测试：第二次 drain 为空时不得再次 publish。
- 补一个批量 spawned 测试，锁定限流/合并策略，防止 startup 全量 POI 刷屏。

## 验收测试计划

- `cd agent/packages/tiandao && npm test`
- 如改到 `agent/packages/schema/src/*` 或 export：`cd agent/packages/schema && npm test`
- `cd agent && npm run build`
- 手动 Redis 验证：
  - 向 `bong:poi_novice/event` publish 一条 `poi_spawned` sample。
  - 订阅 `AGENT_NARRATE`，确认收到 `scope=zone`、`style=perception`、`target=<zone>` 的 narration。
  - 再 publish 一条 `trespass` sample，确认收到 `style=system_warning` 且文案包含“一周”。

## 风险

- POI 加载是批量事件，直接逐条发布会让玩家上线瞬间刷屏，需要合并/限流。
- `PoiSpawnedEventV1` 当前没有 tick 字段，metadata 必须绑定到消费时的 world state tick，不能伪造事件时间。
- #935 可能后续修复拒交易 gameplay；本修复只负责叙事发布，不能把拒交易门禁逻辑混入 agent。
- 如果后续添加独立 runtime，与主循环 drain 同时存在会双发 narration，必须二选一或做事件 id 去重。
