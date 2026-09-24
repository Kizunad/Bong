# plan-bughunt-r9-playerstate-spirit-qi-max-v1

> **已完成（2026-07-09，归档审计确认）**。

一句话主题：从 `docs/plans-skeleton/plan-bughunt-r9-findings-v1.md` 拆出 r9 P0，锁住 `player_state.spirit_qi_max` 从 server active `bong:server_data` 到 client HUD 的真实真元上限链路，防止中高境界真元条分母回退为 100。

## 接入面

- **进料**：`server/src/player/state.rs` 从 ECS `Cultivation { qi_current, qi_max, realm }` 与 `PlayerState` 读取玩家状态，`server/src/schema/server_data.rs` 负责 JSON wire，`proto/bong/envelope.proto` 负责 proto mirror。
- **出料**：`ServerDataPayloadV1::PlayerState.spirit_qi_max` 经 `bong:server_data` 下发，client `PlayerStateHandler` 写入 `PlayerStateViewModel`，`CultivationScreen` / HUD 状态条用 `spiritQiMax()` 计算分母和百分比。
- **共享类型 / event**：复用既有 `ServerDataPlayerStateV1` / `ServerDataPayloadV1::PlayerState` / proto `PlayerState`，不新增 payload 类型。
- **跨仓库契约**：server `ServerDataPayloadV1::PlayerState.spirit_qi_max`；agent schema `ServerDataPlayerStateV1` 与 generated `server-data-v1.json`；client `com.bong.client.network.PlayerStateHandler` / `com.bong.client.state.PlayerStateViewModel`。
- **worldview 锚点**：六境界与真元上限表见 `worldview.md §三 L65-L72`，真元池是 HUD 多血条核心层见 `worldview.md §四 L215-L229`。
- **qi_physics 锚点**：本 plan 不新增真元流动、衰减或守恒公式，只下发已存在的 `Cultivation.qi_max` 只读值。

## 阶段总览

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | 验证 r9 P0 真实链路与当前基线残余缺口 | ✅ 2026-07-09 |
| P1 | 收紧 `spirit_qi_max` server/client 必填消费契约 | ✅ 2026-07-09 |
| P2 | 同步 schema/proto/samples/generated/dist/tests 并跑 targeted 验证 | ✅ 2026-07-09 |
| P3 | 无上下文 gpt-5.5 xhigh read-only validator + PR | ✅ 2026-07-09 |

## P0 — 真实链路与残余缺口

原 r9 skeleton 指出旧链路缺 `spirit_qi_max`：`ServerDataPayloadV1::PlayerState` 只下发当前真元，client 缺字段时 `PlayerStateViewModel.normalizeSpiritQiMax` 回退 `max(100,current)`，使固元及以上境界分母低估。当前 `origin/main` 已包含一部分修复：server enum、proto、TS schema、sample 和 `server_payload_with_social_and_local_pressure` 已有 `spirit_qi_max: cultivation.qi_max`。

本 plan 继续锁住残余风险：Rust wire 反序列化不应把缺失 `spirit_qi_max` 默认为 0；client active handler 不应接受缺字段 payload 后继续用 100 fallback。fallback 只能保留在 viewmodel 防御脏输入，不能作为 `player_state` wire 契约。

## P1 — 收紧必填契约

交付物：

- `server/src/schema/server_data.rs`：移除 `ServerDataPayloadWireV1::PlayerState.spirit_qi_max` 的 `serde(default)`，缺字段 JSON 必须反序列化失败。
- `client/src/main/java/com/bong/client/network/PlayerStateHandler.java`：把 `spirit_qi_max` 纳入 required field，构建 viewmodel 时使用真实 required max。
- `client/src/test/java/com/bong/client/network/PlayerStateHandlerTest.java`：新增/调整高境界用例，断言 `spirit_qi=78, spirit_qi_max=150` 的 fill ratio 为 0.52；缺 `spirit_qi_max` 返回 no-op。

## P2 — schema / proto / sample / generated / dist / tests

交付物：

- `agent/packages/schema/src/server-data.ts`、`proto/bong/envelope.proto`、`agent/packages/schema/generated/server-data-v1.json`、`agent/packages/schema/samples/server-data.player-state.sample.json` 当前已包含必填 `spirit_qi_max`；本阶段用 `generate:check` / schema tests 确认无漂移。
- 若 schema source 发生变化，必须运行 `cd agent && npm run build -w @bong/schema` 生成 `dist/`。本修复优先避免不必要 schema source 改动。
- Rust targeted 测试覆盖 `server_data` sample 反序列化、缺字段拒绝与 player state emit。
- Client targeted 测试覆盖 handler/viewmodel 使用真实分母。

## P3 — validator / PR

完成本地 targeted 测试后，启动无上下文 `gpt-5.5` `xhigh` read-only validator，要求它从第一性原理复核 bug 规则、因果链、最小修复和测试证据。validator PASS 后 push `fix-r9-playerstate-spirit-qi-max` 并开 PR；PR body 必须逐项说明：

1. bug 的真实规则 / 设计约束；
2. 当前代码因果链为什么违反；
3. 每个 commit / 文件改了什么，以及为什么是最小正确修复；
4. 验证命令和 validator PASS 证据；
5. 剩余风险 / 非本 PR 范围。

## Finish Evidence

### 落地清单

- **P1 server**：`server/src/schema/server_data.rs` 的 `player_state` 反序列化拒绝缺失/非正 `spirit_qi_max`（`return Err("player_state.spirit_qi_max must be positive")`），新增 pin 测试 `player_state_requires_spirit_qi_max`。
- **P1 client**：`client/src/main/java/com/bong/client/network/PlayerStateHandler.java` 用 `readRequiredPositiveDouble(payload, "spirit_qi_max")` 把该字段纳入必填校验，缺字段进入 `invalidFields` 拒绝路径；`PlayerStateHandlerTest` / `ProtoServerDataBridgeTest` 新增高境界分母用例。
- **P2 schema**：`agent/packages/schema/src/server-data.ts`、`generated/server-data-v1.json`、`tests/schema.test.ts`（新增 35 行用例）、`resources/.../valid-player-state.json` sample 均已同步必填 `spirit_qi_max`。

### 关键 commit

- `f4b3cd97`（2026-07-09）：修复 player_state 真元上限 HUD 分母，收紧 server/schema/client 三端必填契约。

### 测试结果

- 归档审计时未重跑，以 commit message 既有记录为准："GitHub e2e 通过；本地 schema build/generate:check/schema.test.ts、client PlayerStateHandlerTest/ProtoServerDataBridgeTest、server cargo fmt --check + cargo test player_state_ 通过"。审计时通过 grep 复核 `player_state_requires_spirit_qi_max`（server）、`readRequiredPositiveDouble`（client）均存在于 `origin/main`（`f4b3cd97` 已是其祖先）。

### 跨仓库核验

- **server**：`ServerDataPayloadV1::PlayerState.spirit_qi_max` 反序列化必填校验。
- **agent/schema**：`ServerDataPlayerStateV1` / `generated/server-data-v1.json` / sample 均含必填 `spirit_qi_max`。
- **client**：`PlayerStateHandler` / `PlayerStateViewModel` 使用真实分母，不再回退 `max(100,current)` 作为 wire 契约（viewmodel 侧防御性 fallback 保留，但不再是必填字段的替代）。

### 遗留 / 后续

- 无。本 plan 范围内 P0-P3 均已闭环，未发现遗留缺口。
