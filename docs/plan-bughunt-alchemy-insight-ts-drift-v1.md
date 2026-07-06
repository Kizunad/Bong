# BugHunt: 丹心识别洞察事件 `ts` 合约漂移

状态：Skeleton Plan  
线程：BugHunt E3 / agent-schema 第三轮  
范围：agent TypeScript schema、tiandao Redis runtime bridge、server 事件合约证据

## Bug 摘要

`bong:alchemy_insight` 的 server payload 带 `ts` 字段，但 `agent/packages/schema/src/alchemy.ts` 中 `AlchemyInsightV1` 没有声明 `ts`，且 TypeBox schema 设置了 `additionalProperties: false`。Tiandao `RedisIpc` 收到真实 server-shaped payload 后会调用 `validateAlchemyInsightV1Contract`，把含 `ts` 的事件判定为 invalid 并直接丢弃。

这不是 #970 的暗器充能叙事断链，也不是 #979 的聊天时间戳丢失；本 bug 只覆盖丹心识别洞察事件的 server -> agent 合约漂移。

## 实际游玩体验影响

`plan-alchemy-v2` 已把“丹心识别精度 >= 80% 时触发 `bong:alchemy_insight` -> agent narration”列为完成验收。当前链路下，server 真的发出的高精度丹心识别事件会被 Tiandao schema 校验拒绝，玩家即使获得 `RecipeHint` 物品，也不会看到已承诺的天道叙事反馈，调试时也会误以为 agent 已观察到该炼丹洞察。

反方审查要求降级表述：目前没有证明 `DanxinIdentifyIntent` 已有完整 live 玩家入口，所以本 plan 不主张“炼丹核心玩法不可用”，只主张已完成文档承诺的 agent 叙事桥接会拒收真实 payload，导致体验反馈与运行时观测失真。

## 证据定位

- `server/src/schema/alchemy.rs:153` 定义 `AlchemyInsightV1`，字段包含 `ts: u64`，并使用 `#[serde(deny_unknown_fields)]`。
- `server/src/network/alchemy_bridge.rs:37` 的 `publish_alchemy_insight_events` 构造 payload 时写入 `ts: current_unix_millis()`，再发送 `RedisOutbound::AlchemyInsight`。
- `server/src/alchemy/danxin.rs:151` 在 `accuracy >= 0.80` 时发送 `AlchemyInsightEvent`。
- `agent/packages/schema/src/alchemy.ts:161` 的 `AlchemyInsightV1` 缺少 `ts`，同时 `additionalProperties: false`。
- `agent/packages/schema/samples/alchemy-insight.sample.json` 缺少 `ts`；`agent/packages/schema/generated/alchemy-insight-v1.json` 也未生成 `ts` 字段。
- `agent/packages/tiandao/src/redis-ipc.ts:500` 在 `ALCHEMY_INSIGHT` channel 上调用 `validateAlchemyInsightV1Contract(data)`；校验失败后 `return`，不会 `recordAlchemyRuntimeEvent`。
- `agent/packages/tiandao/tests/redis-ipc.test.ts:524` 的 “observes alchemy insight events” mock payload 未带 `ts`，因此没有覆盖真实 server payload。
- `docs/finished_plans/plan-alchemy-v2.md:28`、`:54`、`:161`、`:222` 将 agent 丹心识别 narration 和 alchemy-insight IPC 作为已落地跨仓库契约。

## 触发路径

1. server 处理丹心识别，`accuracy >= 0.80`。
2. `server/src/alchemy/danxin.rs` 发送 `AlchemyInsightEvent`。
3. `server/src/network/alchemy_bridge.rs` 转成 `AlchemyInsightV1`，payload 包含 `ts`。
4. Redis 发布到 `bong:alchemy_insight`。
5. Tiandao `RedisIpc.handleAlchemyRuntimeEventMessage` 收到消息。
6. `validateAlchemyInsightV1Contract` 因额外字段 `ts` 拒绝 payload。
7. `recordAlchemyRuntimeEvent` 不执行，后续 agent narration 或调试观察不到该事件。

## 反方审查记录

### Round 1

反方结论：需要降级。理由是当前 main 上未证明 `DanxinIdentifyIntent` 有真实玩家入口，且 Tiandao 目前没有完整 alchemy insight narration runtime 消费者；不能把问题描述为炼丹主玩法失效。

采纳结果：保留 bug，但改为“已承诺的 server -> agent 叙事/观测桥接拒收真实 payload”。证据链仍成立：server 会生产含 `ts` 的事件，agent schema 会拒绝它，现有 mock 避开了真实字段。

### Round 2

反方结论：降级通过。#974 丹方残卷 PR 主题不同；#970/#979 不重复；server/agent `ts` 漂移是真实合约 bug。风险边界是：schema-only 修复只能让 Tiandao 接收事件，若要恢复“agent narration 触发”的完成承诺，还必须补足或接通实际 narration 消费链路。

## Skeleton Fix Plan

1. 对齐 TypeScript schema：在 `agent/packages/schema/src/alchemy.ts` 的 `AlchemyInsightV1` 增加 `ts: Type.Integer({ minimum: 0, maximum: JS_SAFE_INTEGER_MAX })`。
2. 同步样例和生成物：更新 `agent/packages/schema/samples/alchemy-insight.sample.json`，重新生成 `agent/packages/schema/generated/alchemy-insight-v1.json` 和 `dist/`。
3. 补 Tiandao server-shaped 回归：把 `agent/packages/tiandao/tests/redis-ipc.test.ts` 的 alchemy insight payload 改为包含 `ts`，并断言 callback 与 latest buffer 均能接收真实形状。
4. 补叙事消费验证：明确 `AlchemyRuntimeEventV1` 中 `AlchemyInsightV1` 的消费位置。若当前只入 buffer，需要增加或接通 alchemy insight narration runtime，使 `plan-alchemy-v2` 的“agent narration 触发”可观察。
5. 增加负例测试：缺少必填字段、`ts` 非整数、`accuracy` 越界时仍拒绝，避免放宽 schema 变成 silent accept。

## 验收测试计划

- `cd agent/packages/schema && npm test`
- `cd agent/packages/tiandao && npm test`
- `cd agent && npm run build`
- 手工或集成验证：向 `bong:alchemy_insight` 发布包含 `ts` 的 server-shaped payload，确认 Tiandao 不再打印 invalid alchemy event，且对应 narration 或可观察 runtime 事件被触发。

## 风险

- 当前 evidence 未证明 live 客户端已能触发 `DanxinIdentifyIntent`，修复时不要把玩家入口问题混入本 plan。
- 若只补 `ts` schema，不补 narration consumer，最多恢复事件接收和观测，不能完成 `plan-alchemy-v2` 的叙事体验承诺。
- schema 改动后必须重建 `@bong/schema` 的 `dist/`，否则 `packages/tiandao` 仍可能引用旧构建产物。
