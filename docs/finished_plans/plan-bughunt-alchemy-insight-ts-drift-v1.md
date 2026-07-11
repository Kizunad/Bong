# plan-bughunt-alchemy-insight-ts-drift-v1

> **Active plan（由 skeleton promotion，2026-07-11）**。一句话主题：修复 `bong:alchemy_insight` 的 server-shaped payload 含权威 `ts`，但 TypeScript `AlchemyInsightV1` 拒收该字段而导致 Tiandao 运行时丢弃事件的契约漂移。

## 阶段总览

| 阶段 | 主题 | 状态 | 验收日期 |
|---|---|---|---|
| P0 | 第一性原理 RED：证明真实 server-shaped payload 被 schema 拒收 | ✅ | 2026-07-11 |
| P1 | 最小 schema/sample/generated/dist 修复与饱和测试 | ✅ | 2026-07-11 |
| P2 | 完整 agent/schema 门禁、主线复验、对抗验证与归档 | ✅ | 2026-07-11 |

## 接入面与范围

- **进料**：server 已通过 Redis channel `bong:alchemy_insight` 发布 `AlchemyInsightV1`，其中 `ts` 来自 `current_unix_millis()`；本 PR 不修改 server。
- **出料**：`agent/packages/tiandao/src/redis-ipc.ts` 通过 `validateAlchemyInsightV1Contract` 接收事件并写入 alchemy runtime event callback/latest buffer。
- **共享契约**：复用 `agent/packages/schema/src/alchemy.ts::AlchemyInsightV1`、`validateAlchemyInsightV1Contract`、sample 与 generated JSON Schema，不新建近义协议。
- **跨仓库契约**：server 证据锚点为 `server/src/schema/alchemy.rs::AlchemyInsightV1.ts` 与 `server/src/network/alchemy_bridge.rs::publish_alchemy_insight_events`；实际改动限定在 agent/schema 与 Tiandao 测试。
- **worldview / qi_physics**：纯 wire schema 对齐，不引入世界观命名、数值、真元流动或视觉资产。
- **非目标**：不补造新的 alchemy narration consumer，不改 live 玩家入口，不改 server/client，不改依赖或生产配置。现有 runtime callback/latest buffer 是本 bug 的可观察接收边界。

## P0 — 第一性原理 RED：真实 payload 被拒收

### 目标

先假设 skeleton 可能误报，以真实 server 结构核对生产链路，并加入修复前必红的契约测试。

### 可核验交付物

- 核对 `server/src/schema/alchemy.rs::AlchemyInsightV1` 与 `agent/packages/schema/src/alchemy.ts::AlchemyInsightV1` 字段 parity。
- 在 schema 测试中加入含 `ts` 的 server-shaped 正例，以及缺失、负数、非整数、超过 `Number.MAX_SAFE_INTEGER` 的反例。
- 在 `agent/packages/tiandao/tests/redis-ipc.test.ts` 加入含 `ts` 的 Redis payload，断言修复前 callback/latest buffer 均未接收，从而证明拒收发生在 validator gate。
- RED 证据必须记录具体失败测试和错误，不以静态阅读代替复现。

## P1 — 最小 schema/sample/generated/dist 修复与饱和测试

### 目标

只补齐既有 wire 字段，保持 `additionalProperties: false` 与现有业务语义不变。

### 可核验交付物

- `agent/packages/schema/src/alchemy.ts::AlchemyInsightV1` 增加 `ts: Type.Integer({ minimum: 0, maximum: JS_SAFE_INTEGER_MAX })`。
- 更新 `agent/packages/schema/samples/alchemy-insight.sample.json` 与 `agent/packages/schema/generated/alchemy-insight-v1.json`。
- 重建 `agent/packages/schema/dist/` 中受影响产物并由 Tiandao typecheck/test 消费验证；`dist/` 是 ignored 构建产物，不纳入 commit。
- Tiandao 回归使用真实 server-shaped payload，断言 callback 与 `getLatestAlchemyEvents()` 均保留 `ts`。
- 饱和测试覆盖：`0`、`Number.MAX_SAFE_INTEGER` 可接收；负数、小数、超上界、缺失 `ts`、其它未知字段均拒绝；`accuracy` 既有边界不放宽。

## P2 — 完整门禁、同步、验证与归档

### 验收矩阵

- `cd agent && npm run build -w @bong/schema`
- `cd agent/packages/schema && npm test`
- `cd agent/packages/tiandao && npm test`
- `cd agent && npm run build`
- 删除/篡改 generated schema 时 freshness gate 必须失败；正常树上 `generate:check` 通过。
- 当前干净 HEAD 经全新无上下文、read-only `gpt-5.6-sol` xhigh validator 输出 `PASS <sha>`。
- 同步最新 `origin/main` 后，对最终 HEAD 重跑受影响门禁并再次取得 validator PASS。

### 归档条件

- P0/P1/P2 全部标记 `✅ 2026-07-11`。
- 填写严格命名的 `## Finish Evidence`，列出落地文件、关键 commits、测试结果、跨栈核验与遗留项。
- 运行 `bash scripts/plan-finish.sh plan-bughunt-alchemy-insight-ts-drift-v1`，以独立中文 commit 归档。

## 风险边界

- 当前证据不证明 live client 已有完整丹心识别入口；本修复只恢复已经存在的 server → Redis → Tiandao 接收/观测契约。
- 不把“允许 `ts`”实现成放宽未知字段；`additionalProperties: false` 必须保持。
- schema source 改动后必须重建 dist，否则 Tiandao 测试可能继续加载旧产物而产生假红或假绿。

## Finish Evidence

### 落地清单

- `agent/packages/schema/src/alchemy.ts`：`AlchemyInsightV1` 新增必填 `ts` 安全整数契约。
- `agent/packages/schema/samples/alchemy-insight.sample.json`：共享正样本补齐真实 server-shaped `ts`。
- `agent/packages/schema/generated/alchemy-insight-v1.json`：required/properties 与 TypeBox source 对齐。
- `agent/packages/schema/tests/schema.test.ts`：新增 server wire parity 饱和边界矩阵。
- `agent/packages/tiandao/tests/redis-ipc.test.ts`：真实 Redis payload 验证 callback/latest buffer 保留 `ts`。

### 关键 commit

- `11c593a41e1eb02a366baf59608d49e313e59e52`（2026-07-11）：升格 skeleton 并收口为纯 agent/schema 契约修复。
- `8d1e18c7d54242db6187bdb38a06a999f35f6bbf`（2026-07-11）：提交 RED，schema 3 项失败且 Tiandao callback 为 0。
- `f8a67b0219947ce8acaeb6f1d1f0c836d3fa1378`（2026-07-11）：补齐 `ts` source/sample/generated，保持严格未知字段拒绝。
- `025db31b834030ece64bdeef134c00f5567734db`（2026-07-11）：按 CodeRabbit 意见补齐三类拒绝断言的实际 `ok/errors` 诊断。

### 测试结果

- RED：schema server-wire 套件因 `/ts: Unexpected property` 与缺失 `ts` 被误接收共 3 项失败；Tiandao Redis 用例记录 `/ts: Unexpected property`，callback 0。
- 定向 GREEN：schema `10 passed`；Tiandao Redis `1 passed`。
- `cd agent/packages/schema && npm test`：28 files / 799 tests，全部通过。
- `cd agent/packages/tiandao && npm test`：72 files / 825 tests，全部通过。
- `cd agent && npm run build`：`@bong/schema` 与 `@bong/tiandao` TypeScript build 全部通过，schema dist 已重建。
- `cd agent && npm run generate:check -w @bong/schema`：392 个 generated schema 全部 fresh。
- CodeRabbit 返工后定向 `alchemy insight server wire parity`：9 tests 全部通过；schema 全量仍为 28 files / 799 tests，workspace build 仍通过。
- 主线同步：`origin/main=3c8bf9253680795136f152f5504f6f709c5e16cb`，`merge-base` 相同，分类 `already-up-to-date`。
- 无上下文 read-only validator：`PASS f8a67b0219947ce8acaeb6f1d1f0c836d3fa1378`（修复后）与同步后复验再次 PASS。

### 跨仓库核验

- server（只读证据）：`server/src/schema/alchemy.rs::AlchemyInsightV1.ts`、`server/src/network/alchemy_bridge.rs::publish_alchemy_insight_events`、Redis channel `bong:alchemy_insight`。
- schema：`AlchemyInsightV1`、`validateAlchemyInsightV1Contract`、`alchemy-insight-v1.json` 三者字段一致。
- agent：`RedisIpc.handleAlchemyRuntimeEventMessage` 校验后进入 callback 与 `getLatestAlchemyEvents()`，`ts` 不再被拒收或丢失。
- client：本 plan 不涉及 client wire 或 UI。

### 遗留 / 后续

- 不在本 plan 内补 live client 的丹心识别入口，也不新增 alchemy narration consumer；本修复边界是恢复既有 Redis 接收与运行时观测契约。
- `agent/packages/schema/dist/` 为 ignored 构建产物，已按门禁重建并由 Tiandao 全量 typecheck/test 消费验证，不提交到 Git。
