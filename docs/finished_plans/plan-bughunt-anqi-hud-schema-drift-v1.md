# plan-bughunt-anqi-hud-schema-drift-v1

状态：✅ 2026-07-12
分区：BugHunt worker / agent-schema / r12

## 1. 一句话 bug

`anqi_hud` 已是 server 与 client 正式接通的 `bong:server_data` S2C payload，但 `agent/packages/schema/src/server-data.ts` 的 TypeScript `ServerDataType` / `ServerDataV1` 镜像完全没有 `anqi_hud` 分支，导致 agent/schema 对暗器 HUD 协议漂移假绿。

## 2. 实际游玩体验影响

玩家使用暗器分身、封元蓄力、容器磨损等招式时，server 会下发 `anqi_hud`，client 依赖它更新暗器 HUD 的 echo / charge / abrasion 状态。当前真实游戏主链路仍可能工作，因为 Rust schema、protobuf bridge 与 Java router 已接通。

风险在回归保护：agent/schema 作为 server_data 契约镜像不认识 `anqi_hud`，bot/playtest/schema 校验无法把暗器 HUD 当作正式 payload 约束。后续如果 server 改字段、client bridge 漏字段、proto oneof 或 JSON type 漂移，schema gate 仍可能绿色，玩家实际看到的就是暗器 HUD 不闪、不显示蓄力进度、容器磨损反馈丢失，而自动化只证明“有某个 server_data”，没有证明暗器 HUD 协议仍对齐。

## 3. 复现路径

1. 查看 `agent/packages/schema/src/server-data.ts:167-266`，`ServerDataType` 字面量列表没有 `Type.Literal("anqi_hud")`。
2. 查看 `agent/packages/schema/src/server-data.ts:1806-1912`，`ServerDataV1` union 没有 `ServerDataAnqiHudV1` 或等价 wrapper。
3. 在 agent schema 目录搜索 `anqi_hud` / `AnqiHud`，没有对应 TypeBox schema、sample 或 generated artifact。
4. 对照 server/client：Rust 会构造并发送 `ServerDataPayloadV1::AnqiHud`，client proto bridge 与 router 都注册了 `anqi_hud`。

## 4. 根因证据

- `server/src/schema/server_data.rs:556-558`：`ServerDataPayloadV1` 已有 `AnqiHud(AnqiHudV1)` 变体。
- `server/src/schema/server_data.rs:769-783`：`AnqiHudV1` 定义了 `kind`、`echo_count`、`aim_progress`、`charge_progress`、`abrasion_container`、`abrasion_qi_payload`、`tick`。
- `server/src/schema/server_data.rs:5739-5764`：server 单测锁定 `payload_type_label(ServerDataType::AnqiHud) == "anqi_hud"`，并断言 wrapper 序列化出的 wire `type` 是 `anqi_hud`。
- `server/src/network/anqi_hud_emit.rs:38-68`：`emit_anqi_hud_payloads` 从暗器事件构造 `ServerDataV1::new(ServerDataPayloadV1::AnqiHud(...))` 并发送给 client。
- `client/src/main/java/com/bong/client/network/ProtoServerDataBridge.java:169-170`：protobuf `ANQI_HUD` oneof 映射为 legacy JSON type `anqi_hud`。
- `client/src/main/java/com/bong/client/network/ServerDataRouter.java:249-252`：client `ServerDataRouter` 注册 `handlers.put("anqi_hud", ...)`。
- `agent/packages/schema/src/server-data.ts:167-266`：TS `ServerDataType` 没有 `anqi_hud`。
- `agent/packages/schema/src/server-data.ts:1806-1912`：TS `ServerDataV1` union 没有 `anqi_hud` wrapper。

## 5. 不重复性

- 不重复 #1059 “bot 战斗 server_data 类型断言假阳性”：#1059 是 bot helper 对 protobuf 类型匹配过宽，本题是 TS schema 镜像缺少一个已上线 payload。
- 不重复 #1061 “agent schema 生成物覆盖假绿”：#1061 聚焦已被 Tiandao runtime 消费的 server -> agent Redis payload 缺 generated coverage；本题是 server -> client `bong:server_data` 的 `anqi_hud` 根本没进入 `ServerDataV1`。
- 不重复 #1093 “Tiandao schema dist 启动断链”：#1093 是 dist/export 启动问题，本题是源 schema union 漏分支。
- 不重复 #1111 “proto breaking gate 浅拉假跳过”：#1111 是 proto breaking check 的拉取深度，本题是具体 payload 的 TypeBox 镜像缺失。
- 不重复 #1068-#1072：这些 PR 分别覆盖 bot playtest 修复批、worldgen 预算、濒死 UX、bot 产用场景等，不是 `anqi_hud` schema drift。

## 6. 修复计划骨架

- [x] 在 `agent/packages/schema/src/server-data.ts` 增加 `AnqiHudV1` / `ServerDataAnqiHudV1` TypeBox schema，字段与 Rust `AnqiHudV1` 对齐。
- [x] 将 `Type.Literal("anqi_hud")` 加入 `ServerDataType`，并将 `ServerDataAnqiHudV1` 加入 `ServerDataV1` union。
- [x] 在 `SCHEMA_REGISTRY` / `GENERATED_SCHEMA_FILES` 登记 `server-data-anqi-hud-v1.json`，生成对应 artifact。
- [x] 增加正反 sample 与 schema 测试：`kind="echo"` / `kind="charge"` / `kind="abrasion"` 正例，缺字段、额外字段、非法数值反例。
- [x] 增加 Rust/TS parity pin：至少覆盖 Rust wire shape 中的 `type:"anqi_hud"` 与所有必需字段。

## 7. 验证计划

- [x] `cd agent/packages/schema && npm test`
- [x] `cd agent/packages/schema && npm run generate:check`
- [x] `cd agent && npm run build -w @bong/schema`
- [x] 抽查 `agent/packages/schema/generated/server-data-v1.json` 包含 `anqi_hud` 分支。
- [x] 抽查 `agent/packages/schema/generated/server-data-anqi-hud-v1.json` 存在，且删除后 freshness gate 会红。

## 8. 对抗复核记录

第一轮候选是 `ServerDataVortexStateV1` 单体 generated artifact 缺失。反方指出 `server-data-v1.json` 已覆盖 `vortex_state`，生产 S2C 走 protobuf + client router，影响偏契约审计且接近 #1061；该候选降级并丢弃。

第二轮收窄为 `anqi_hud`：两名 adversarial reviewer 独立确认 server 已发、client 已路由、TS `ServerDataType` / `ServerDataV1` 无 `anqi_hud`，且不重复 #1059/#1061/#1093/#1111/#1068-#1072。

Adversarial conclusion：PASS。`anqi_hud` 是正式 server_data payload，但 agent/schema 完全漏镜像；实际游玩主链路未必立即断，核心 bug 是暗器 HUD 回归与协议漂移保护缺口。

## Finish Evidence

### 落地清单

- TypeBox 与 generated：`agent/packages/schema/src/server-data.ts`、`schema-registry.ts`、`generated/{anqi-hud-v1,server-data-anqi-hud-v1,server-data-v1}.json`。
- 共享 wire 语料：`agent/packages/schema/samples/server-data.anqi-hud.*.json`，由 TypeScript、Rust、Java 三端共同执行。
- Rust 真实 wire：`server/src/schema/server_data.rs` 锁定 serde/kind/边界，`server/src/schema/proto_gen.rs` 锁定全部 kind 与非零 aim，`server/src/network/anqi_hud_emit.rs` 通过生产 protobuf serializer 验证五路 emitter。
- Java 消费边界：`AnqiHudServerDataHandler` 严格校验完整字段集合、数值上限和 canonical 容器；`AnqiHudServerDataHandlerTest` 覆盖 Proto→bridge→router→store 及共享 corpus。

### 关键 commit

- `0ef81bd9`（2026-07-09）：补齐暗器 HUD TypeBox 契约与注册表。
- `8cf4817f` / `b0baa846`（2026-07-10）：补齐共享 wire 语料并锁定 Rust 对拍。
- `856137a2` / `35bee849` / `b11753e2`（2026-07-10）：统一跨端数值边界、Rust kind 与 Proto 消费边界。
- `793d20ec` / `90cad8f5` / `61903b34`（2026-07-12）：接通 Java aim，拒绝窄化/形状漂移，并执行共享 corpus。
- `7a006604` / `c1f758ab`（2026-07-12）：锁定真实 emitter、非零 aim Proto 与生产 protobuf 出站。

### 测试结果

- `cd agent/packages/schema && npm test`：29 files / 816 tests PASS。
- `npm run generate:check`：397 个 generated artifacts fresh；删除独立 artifact 的 freshness 负分支 PASS。
- `npm run build`（`@bong/schema`）：PASS；ignored `dist/` 已在本地重建。
- `cargo test anqi_hud`：25/25 PASS；`s2c_all_proto_variants_encode_without_panic` PASS。
- `cargo test`：目标代码全绿；全仓并发运行 11,288 PASS，两个最新主线的时序/耗时用例抖动，随后各自 `--exact` 单独 PASS。
- Java 17 `./gradlew test build`：PASS；`AnqiHudServerDataHandlerTest` 含共享 corpus 与 Proto 闭环。
- PR #1147 旧 CI run `29060370791`：schema、agent、client、server、smoke 全部 PASS；bot e2e 21/22，唯一失败为本 PR 未触碰的 `cultivation_pill_consume`。

### 跨仓库核验

- server：`AnqiHudKindV1::ALL`、`ServerDataPayloadV1::AnqiHud`、`serialize_server_data_payload_proto`。
- schema：`AnqiHudV1`、`ServerDataAnqiHudV1`、`server-data.anqi-hud.wire-corpus.json`。
- client：`ProtoServerDataBridge` → `AnqiHudServerDataHandler` → `AnqiHudStateStore`。

### 遗留 / 后续

- 本 plan 无未完成代码项；PR gate 仍以最终 HEAD 的 CI 与 `/review` / CodeRabbit 闭环结果为准。
- Follow-up（仓库级 blocker，非本 PR 新增）：Rust 1.96 全仓 clippy 受 `origin/main` 既有 66 个新 lint 阻塞；未在本 PR 扩大范围修整。
