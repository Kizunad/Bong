# plan-bughunt-alchemy-insight-ts-drift-v1

> **Active plan（由 skeleton promotion，2026-07-11）**。一句话主题：修复 `bong:alchemy_insight` 的 server-shaped payload 含权威 `ts`，但 TypeScript `AlchemyInsightV1` 拒收该字段而导致 Tiandao 运行时丢弃事件的契约漂移。

## 阶段总览

| 阶段 | 主题 | 状态 | 验收日期 |
|---|---|---|---|
| P0 | 第一性原理 RED：证明真实 server-shaped payload 被 schema 拒收 | ⏳ | — |
| P1 | 最小 schema/sample/generated/dist 修复与饱和测试 | ⬜ | — |
| P2 | 完整 agent/schema 门禁、主线复验、对抗验证与归档 | ⬜ | — |

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
- 重建并提交 `agent/packages/schema/dist/` 中受影响产物，避免 Tiandao 引用旧 dist。
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
