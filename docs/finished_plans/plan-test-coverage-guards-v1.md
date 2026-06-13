# Bong · plan-test-coverage-guards-v1 · active

测试集**系统性盲区守护**——堵住「真生产 bug 能逃过整个测试集」的结构性漏洞。源起：proto-panic 真生产 bug（`serialize_server_data_payload` 生产走 proto `unreachable!()` panic，e2e 跑 `cfg(test)`=JSON 路径所以一直没抓到）。5 维并行审计（2026-06-13）发现这是一类盲区的一个实例，本 plan 按优先级补穷举/编译期/接线守护。

## 阶段总览

| 阶段 | 内容 | 状态 | 验收 |
|------|------|------|------|
| **P0** | proto 序列化穷举守护（每变体必须 proto 或 JSON-bypass 白名单 + 生产路径 #[test] 遍历 + #[should_panic] pin） | ✅ 2026-06-13 | `s2c_all_proto_variants_encode_without_panic` / `c2s_*` 遍历全变体；新增无 proto 变体 → 测试红；HalfStepRechallenge `#[should_panic]` pin |
| **P1** | emit→consumer 接线守护 + 14 个孤岛事件 triage（wiring-assert 机制 + 白名单 by-design） | ✅ 2026-06-14 | `event_emitters_have_readers_or_triage_entries` 测试期校验；每个 `EventWriter<E>` 要么有 `EventReader<E>`，要么在 intentional triage 白名单 |
| **P2** | e2e CI 范围补全（client `./gradlew test` 进 CI + full-app startup smoke + `to_proto_bytes` oversize cap） | ✅ 2026-06-14 | e2e.yml 跑 client gradlew；`full_app_startup.rs` 断言核心 resource 就绪；proto 编码超限返 Err |
| **P3** | mock-masking 真 impl 契约测试（真 TiandaoAgent 注入 + assert_no_halfstep 改 proto decode） | ✅ 2026-06-14 | 真 TiandaoAgent setXxx→LLM prompt 含内容的集成测试；`assert_no_*_on_server_data_channel` 用 `prost::decode` 而非 `serde_json::from_slice` |

> 优先级：P0 最高杠杆（直接防 proto-panic 类复发，编译期+穷举双保险）。P1 涉真功能孤岛（与「bug+代码断连狩猎」重叠，本 plan 做守护机制，孤岛修复可交狩猎）。P2/P3 收口 CI 与 mock 盲区。

## 接入面 Checklist

- **进料**：5 维审计 findings（cfg-divergence / proto-json-wire / mock-masking / emit-no-consumer / e2e-harness-scope）。
- **出料**：守护测试（穷举/编译期/接线）入 `server/src/schema/proto_convert.rs` tests、`server/src/network/agent_bridge.rs` tests、各模块 `register()`；CI 步骤入 `.github/workflows/e2e.yml`。
- **共享类型/symbol**：`ServerDataPayloadV1`（125 变体）/ `ClientRequestV1` / `RedisOutbound` / `to_proto_bytes` / `server_data_to_proto_payload` / `serialize_server_data_payload` / `EventWriter`/`EventReader`。
- **跨仓库契约**：server proto 编码 ↔ client `ProtoServerDataBridge` 解码 ↔ agent TypeBox（Redis sample 对拍）。
- **worldview/qi 锚点**：无新机制（纯测试基建）；P1 触及 `QiTransfer`（审计事件，守恒由 `WorldQiAccount` 维护，**不是**孤岛 bug——白名单）。

## P0 — proto 序列化穷举守护（防 proto-panic 类复发）

**根因**：`serialize_server_data_payload`（`server/src/network/agent_bridge.rs:49-56`）`#[cfg(test)]`→JSON / `#[cfg(not(test))]`→proto。所有 70+ 集成测试走 JSON 路径，生产 proto 路径只被 proto_convert.rs 里约 13 个 roundtrip 测试直接覆盖（125 个 `ServerDataPayloadV1` 变体里约 112 个无 proto 编码测试）。新变体漏 proto arm → 编译过 + 测试绿 + 生产 `unreachable!()` panic。

### 交付物

- [x] `server/src/schema/proto_convert.rs` 测试：`fn s2c_all_proto_variants_encode_without_panic()` —— 遍历**每个** `ServerDataPayloadV1` 变体的最小 fixture（复用 `server_data.rs::hud_payload_wire_type_matches_label` 测试已有的变体样本列表），对每个调 `ServerDataV1::new(variant).to_proto_bytes()`，断言非空 / 不 panic。**JSON-bypass 3 变体（AgentUiRequest/AgentUiClose/HalfStepRechallenge）显式排除**（断言它们走 `#[should_panic]`，不在正常遍历内）。
- [x] 同款 `fn c2s_all_proto_variants_encode_without_panic()` 覆盖 `ClientRequestV1` 全变体（约 37 个无 roundtrip 测试）。
- [x] `HalfStepRechallenge` 补 `#[test] #[should_panic(expected="HalfStepRechallenge 经由 JSON CustomPayload 发送")]`（仿 proto_convert.rs:4842/4856 已有的 `agent_ui_request_panics_if_proto_path_is_used`）。
- [x] **编译期 JSON-bypass 白名单**：`ServerDataPayloadV1` 加 `const fn is_json_bypass(&self) -> bool`（match 穷举，3 个 bypass 变体返 true），守护测试用它区分「应有 proto」vs「应 panic」——新增 bypass 变体必须显式标注，新增普通变体必须有 proto arm 否则遍历测试红。
- [x] 修正 `serialize_server_data_payload` 的**误导性注释**（"proto_convert round-trip tests (106 variants)" 与实际不符）→ 规范性说明「每个新变体 MUST 加入穷举守护」。
- [x] ≥ 2 防回归：删一个变体的 proto arm（mutation）→ 遍历测试 panic 红；把一个 bypass 变体误判为非 bypass → 测试红。

### P0 验收
`cargo test proto_convert` 全绿；删任一普通变体 proto arm 或漏标 bypass → 遍历守护测试撞红（mutation 验证）；新增 `ServerDataPayloadV1`/`ClientRequestV1` 变体不分类（proto arm 或 bypass 白名单）→ 编译/测试不过。

## P1 — emit→consumer 接线守护 + 孤岛 triage

审计扫出 **14 个 `EventWriter<E>` 但全仓零 `EventReader<E>`** 的 Bevy 事件（emit 后 2-tick 静默丢弃，测试直读 `Events<E>` 队列掩盖孤岛）：`SwordBondFormedEvent` / `TechniqueLearnedEvent` / `TechniqueMasteredEvent` / `InfluenceChangedEvent` / `IdentityCreatedEvent` / `IdentitySwitchedEvent` / `BeastHordeEvent` / `FlowFieldPrototype` / `YidaoCastCompleteEvent` / `NpcScheduleChangedEvent` / `QiNeedleChargedEvent` / `ZoneEnvironmentLifecycleEvent` / `TurbulenceFieldDecayed` / `TsySpawnResult`。`QiTransfer` 例外（审计日志，守恒由 `WorldQiAccount` 维护，**by-design 入白名单**）。

### 交付物（P1）
- [x] `wiring_assert` 机制：测试或启动期校验每个 `EventWriter<E>` 要么有注册 `EventReader<E>` 系统、要么在 `INTENTIONAL_UNCONSUMED_EVENTS` 白名单（含 QiTransfer + 注明理由）。
- [x] 14 个孤岛逐个 triage：真孤岛（功能不触发）→ 标 follow-up / 交「代码断连狩猎」修；by-design → 入白名单注明。

## P2 — e2e CI 范围补全

- [x] `.github/workflows/e2e.yml` 加 **client `./gradlew test`** 步骤（当前 CI **从不**跑 client 测试，`ProtoServerDataBridgeTest` 只本地 smoke-test.sh 跑）。
- [x] `server/tests/full_app_startup.rs`：构建完整 `run_server()` App 一帧，断言核心 resource（`RedisBridgeResource` 等）就绪 + 注册无 panic（当前无任何测试实例化完整 App）。
- [x] `to_proto_bytes()` 加 `MAX_PAYLOAD_BYTES` 超限校验（当前生产 proto 编码无 size cap，oversize 守护只在 JSON test 路径）。

## P3 — mock-masking 真 impl 契约测试

- [x] 真 `TiandaoAgent`（非 `ButtonClickAwareAgent`/`ChatAwareFakeAgent`/`DeathAwareFakeAgent` 子类）+ mock LlmClient spy prompt 的集成测试：`setButtonClickEvents`/`setChatSignals`/`setNpcDeathEvents` 注入 → 断言 LLM user message 真含内容。
- [x] `assert_no_halfstep_on_server_data_channel`（及同类）改用 `prost::Message::decode` 解 proto 而非 `serde_json::from_slice`（后者只在 test/JSON 有效，生产 proto 测不到路由回归）。
- [x] client `handleRawRequest_wire_*` 用 stub MinecraftClient（非 NPE-as-proxy）测真 happy-path。

## §8 开放问题

无未决设计门——纯测试基建，守护机制照既有 proto_convert/register 模式。P1 的孤岛白名单 vs 修复边界：by-design（QiTransfer 审计）入白名单，真功能孤岛交「代码断连狩猎」或后续 plan。

---

## P0 落地记录（多 PR plan 阶段进展，非归档 Finish Evidence）

proto 序列化**穷举编译期守护**落地，彻底防 proto-panic 类（新变体漏 proto arm → 编译过+JSON 测试绿+生产 `unreachable!()` panic）复发。

### 落地清单

- `server/src/schema/server_data.rs:3532` `ServerDataPayloadV1::is_json_bypass(&self) -> bool`：**对 &self 穷举 match（125 arm，无 catch-all `_`）**，3 个 JSON-bypass 变体（AgentUiRequest/AgentUiClose/HalfStepRechallenge）返 true，其余逐一 false。**新增变体不在此 match → rustc E0004 非穷举编译失败**（编译期强制分类）。
- `server/src/schema/proto_convert.rs:6231` `s2c_all_proto_variants_encode_without_panic()`：遍历全部 122 个非-bypass `ServerDataPayloadV1` 变体调 `to_proto_bytes()` 断言非空/不 panic + **fixture discriminant 集合 == 全 125 变体集合**的覆盖完整性断言（新变体无 fixture → 断言红）。
- `server/src/schema/proto_convert.rs:6857` `c2s_all_proto_variants_encode_without_panic()`：同款覆盖全部 97 个非-bypass `ClientRequestV1`（共 98，1 个 AgentUiResponse bypass）。
- `server/src/schema/proto_convert.rs:4895` `HalfStepRechallenge` 补 `#[should_panic(expected="HalfStepRechallenge 经由 JSON CustomPayload 发送")]` pin（此前仅 AgentUiRequest/Close/Response 有）。
- `agent_bridge.rs:48` 误导性注释（"round-trip tests (106 variants)" 与实际 125 不符）修正为规范性说明。

### 关键 commit

`7630bc469` 立 plan · `aa7c21eac` P0 穷举守护（4 files / +2272，含 122+97 变体 fixtures）

### 测试 + mutation 验证

`cargo test proto_convert` → **53 passed / 0 failed**（reverify 3 维全 PASS）。mutation 验证：删任一普通变体 proto arm → 遍历守护测试红；HalfStepRechallenge 走 proto → should_panic；is_json_bypass 漏标新变体 → E0004 编译不过；漏写 fixture → 覆盖断言红。

### 后续（P3 已完成）

- 无。P3 已完成，本 plan 全阶段收口。

---

## P1 落地记录（多 PR plan 阶段进展，非归档 Finish Evidence）

emit→consumer **接线守护 + 孤岛 triage** 落地，防止 Bevy `EventWriter<E>` 写出后无 `EventReader<E>` 消费、测试只 drain `Events<E>` 队列而掩盖运行时断连。

### 落地清单

- `server/src/main.rs`：以 `#[cfg(test)] mod test_coverage_guards;` 挂载测试期守护模块，不进入生产运行路径。
- `server/src/test_coverage_guards.rs`：新增 `event_emitters_have_readers_or_triage_entries()`，扫描 `server/src/**/*.rs` 的 `EventWriter<E>` / `EventReader<E>`，要求 writer-only 事件必须进入 `INTENTIONAL_UNCONSUMED_EVENTS`。
- `server/src/test_coverage_guards.rs`：`INTENTIONAL_UNCONSUMED_EVENTS` 为当前 writer-only 事件逐条写明 `reason` + `follow_up`；`QiTransfer` 标为 `DirectResourceConsumer`，原因是守恒余额由 `WorldQiAccount` / qi ledger 调用点直接 apply，不依赖 EventReader。
- `server/src/test_coverage_guards.rs`：新增 stale / orphan triage 守护；白名单事件一旦出现真实 `EventReader`，或代码中不再存在对应 `EventWriter`，测试会红，要求移出或更新 triage，避免白名单永久堆积。
- `server/src/test_coverage_guards.rs`：补 scanner 边界回归，覆盖 nested generic、字符串字面量干扰、lifetime 归一化与长 location 截断。
- 14 个原始审计点已逐项处理：`BeastHordeEvent` 当前已有真实 reader，不入白名单；其余仍 writer-only 的领域/反馈事件进入 triage 并指向后续反馈、叙事、UI 或清理 plan。

### 关键 commit

`81770280a` P1 事件接线守护（2 files / +588，含 scanner、triage 白名单、stale whitelist pin、lifetime/path/comment fixture 测试）

### 测试

`CARGO_BUILD_JOBS=2 cargo test test_coverage_guards -- --nocapture` → **8 passed / 0 failed**；`CARGO_BUILD_JOBS=2 cargo fmt --check && CARGO_BUILD_JOBS=2 cargo clippy --all-targets -- -D warnings && CARGO_BUILD_JOBS=2 cargo test` → **9002 passed / 0 failed / 1 ignored**。

### 后续（P3 已完成）

- 无。P3 已完成，本 plan 全阶段收口。

---

## P2 落地记录（多 PR plan 阶段进展，非归档 Finish Evidence）

e2e CI 范围补全落地，覆盖此前 client 测试不进 CI、完整 App 无启动 smoke、生产 proto 路径缺少 oversize cap 三类测试盲区。

### 落地清单

- `.github/workflows/e2e.yml`：push / PR paths 纳入 `client/**`；新增 pinned `actions/setup-java` Java 17 + Gradle cache；Redis 前新增 `Client stage (gradlew test)`，在 `client/` 运行 `./gradlew test`。
- `server/src/main.rs`：拆出 `build_server_app() -> App`；新增 `BONG_FULL_APP_STARTUP_SMOKE=1` 路径，构建完整 App、替换临时 `PersistenceSettings`、断言 `NetworkSettings` / `NetworkBridgeResource` / `RedisBridgeResource` / `PersistenceSettings` 就绪并 tick 一帧。
- `server/tests/full_app_startup.rs`：新增 binary integration smoke，启动 `CARGO_BIN_EXE_bong-server`，设置 `BONG_SKIP_SKIN_PREFETCH=1` 与不可达 `REDIS_URL`，断言成功退出并输出 `full app startup smoke ok`。
- `server/src/network/agent_bridge.rs`：生产 proto path 改走 `to_proto_bytes_checked()`；新增 `enforce_payload_size()`，对 `MAX_PAYLOAD_BYTES` 以上 payload 返回 `PayloadBuildError::Oversize`。
- `server/src/persistence/mod.rs`：`PersistenceSettings::with_paths` 从 test-only constructor 调整为 runtime 可用 constructor，供 smoke binary 使用临时数据路径，避免污染真实 `data/bong.db`。

### 测试

- `CARGO_BUILD_JOBS=2 cargo fmt --check`
- `CARGO_BUILD_JOBS=2 cargo clippy --all-targets -- -D warnings`
- `CARGO_BUILD_JOBS=2 cargo test proto_path_rejects_oversize_payloads -- --nocapture` → 1 passed
- `CARGO_BUILD_JOBS=2 cargo test payload_size_guard_accepts_exact_cap -- --nocapture` → 1 passed
- `CARGO_BUILD_JOBS=2 cargo test full_app_startup_smoke -- --nocapture` → 3 passed + `tests/full_app_startup.rs` 1 passed
- `CARGO_BUILD_JOBS=2 cargo test --test full_app_startup -- --nocapture` → 1 passed
- `CARGO_BUILD_JOBS=2 cargo test` → 9026 passed / 0 failed / 1 ignored，另 `tests/full_app_startup.rs` 1 passed
- `JAVA_HOME=$HOME/.sdkman/candidates/java/17.0.18-amzn ./gradlew --max-workers=2 test build` → BUILD SUCCESSFUL

### 后续（P3 已完成）

- 无。P3 已完成，本 plan 全阶段收口。

---

## P3 落地记录（多 PR plan 阶段进展，非归档 Finish Evidence）

mock-masking 真实现契约守护落地，覆盖此前 fake agent / test-only JSON / null-client NPE 三类“测试替身过强，生产路径未被验证”的盲区。

### 落地清单

- `agent/packages/tiandao/tests/agent-real-context-injection.test.ts`：新增真 `TiandaoAgent` + spy `LlmClient` 集成测试，分别覆盖 `setChatSignals`、`setNpcDeathEvents`、`setButtonClickEvents` 注入后进入 LLM user message。
- `server/src/network/halfstep_rechallenge_emit.rs`：`assert_no_halfstep_on_server_data_channel` 改为 `prost::Message::decode` 生产 proto envelope；若测试 JSON 误入 `bong:server_data` 直接 panic。
- `server/src/network/agent_ui.rs`：`assert_no_agent_ui_on_server_data_channel` 同步改为 proto decode，守护 `AgentUiRequest` / `AgentUiClose` 继续走专属 channel。
- `client/src/main/java/com/bong/client/network/AgentUiPayloadHandler.java`：抽出 `parseAndMaybeOpenRawRequest(...)`，保留生产 `handleRawRequest(...)` 行为并提供 package-private ready-client 测试 seam。
- `client/src/test/java/com/bong/client/network/AgentUiPayloadHandlerReadyClientTest.java`：用 stub opener 覆盖 server 裸 payload parse → open happy path。
- `client/src/test/java/com/bong/client/agentui/AgentUiPayloadHandlerTest.java`：移除 null-client NPE-as-proxy 断言，改锁定 null client 不抛、不写 store。

### 关键 commit

- `b2d17fc97` test(agent): 补天道真实现上下文注入守护
- `e84f7e9aa` test(server): 用 proto 解码守护专属通道路由
- `8439d3cd6` test(client): 用 ready stub 覆盖 Agent UI 原始请求

### 测试

- `cd server && CARGO_BUILD_JOBS=2 cargo fmt --check`
- `cd server && CARGO_BUILD_JOBS=2 cargo clippy --all-targets -- -D warnings`
- `cd server && CARGO_BUILD_JOBS=2 cargo test` → **9029 passed / 0 failed / 1 ignored**，`tests/full_app_startup.rs` **1 passed**
- `cd client && JAVA_HOME=$HOME/.sdkman/candidates/java/17.0.18-amzn ./gradlew --max-workers=2 test build` → **BUILD SUCCESSFUL**
- `cd agent && npm run build` → **schema + tiandao build passed**
- `cd agent/packages/tiandao && npm test` → **71 files passed / 792 tests passed**
- `cd agent/packages/schema && npm test` → **27 files passed / 704 tests passed**

---

## Finish Evidence

### 落地清单

- **P0 proto 序列化穷举守护**：`server/src/schema/server_data.rs`、`server/src/schema/proto_convert.rs`、`server/src/network/agent_bridge.rs`；新增 S2C/C2S 全变体 proto encode 守护、JSON-bypass 显式分类与 HalfStepRechallenge proto panic pin。
- **P1 emit→consumer 接线守护**：`server/src/main.rs`、`server/src/test_coverage_guards.rs`；新增 `EventWriter<E>` / `EventReader<E>` 扫描、writer-only triage 白名单与 stale/orphan 反向守护。
- **P2 e2e CI 范围补全**：`.github/workflows/e2e.yml`、`server/src/main.rs`、`server/tests/full_app_startup.rs`、`server/src/network/agent_bridge.rs`、`server/src/persistence/mod.rs`；client test 入 e2e CI、完整 App startup smoke、proto payload size cap。
- **P3 mock-masking 真 impl 契约测试**：`agent/packages/tiandao/tests/agent-real-context-injection.test.ts`、`server/src/network/halfstep_rechallenge_emit.rs`、`server/src/network/agent_ui.rs`、`client/src/main/java/com/bong/client/network/AgentUiPayloadHandler.java`、`client/src/test/java/com/bong/client/network/AgentUiPayloadHandlerReadyClientTest.java`、`client/src/test/java/com/bong/client/agentui/AgentUiPayloadHandlerTest.java`；真 TiandaoAgent prompt 注入、production proto decode guard、ready-client stub happy path。

### 关键 commit

- `50083a4d3` plan-test-coverage-guards-v1 P0：proto 序列化穷举编译期守护（防 proto-panic 类复发）
- `20132c058` plan-test-coverage-guards-v1 P1：EventWriter 接线守护
- `fbbe7f5af` plan-test-coverage-guards-v1 P2：补齐 e2e CI 与启动守护
- `b2d17fc97` test(agent): 补天道真实现上下文注入守护
- `e84f7e9aa` test(server): 用 proto 解码守护专属通道路由
- `8439d3cd6` test(client): 用 ready stub 覆盖 Agent UI 原始请求

### 测试结果

- **P0**：`cargo test proto_convert` → **53 passed / 0 failed**；mutation 验证覆盖漏 proto arm、漏 bypass 分类、漏 fixture。
- **P1**：`CARGO_BUILD_JOBS=2 cargo test test_coverage_guards -- --nocapture` → **8 passed / 0 failed**；`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` → **9002 passed / 0 failed / 1 ignored**。
- **P2**：`CARGO_BUILD_JOBS=2 cargo test` → **9026 passed / 0 failed / 1 ignored**；`cargo test --test full_app_startup` → **1 passed**；`JAVA_HOME=$HOME/.sdkman/candidates/java/17.0.18-amzn ./gradlew --max-workers=2 test build` → **BUILD SUCCESSFUL**。
- **P3**：`CARGO_BUILD_JOBS=2 cargo fmt --check && CARGO_BUILD_JOBS=2 cargo clippy --all-targets -- -D warnings && CARGO_BUILD_JOBS=2 cargo test` → **9029 passed / 0 failed / 1 ignored**，`tests/full_app_startup.rs` **1 passed**；client Gradle **BUILD SUCCESSFUL**；agent `npm run build` + tiandao/schema tests **792 + 704 passed**。

### 跨仓库核验

- **server**：`ServerDataPayloadV1::is_json_bypass`、`ServerDataV1::to_proto_bytes()`、`assert_no_halfstep_on_server_data_channel`、`assert_no_agent_ui_on_server_data_channel`、`event_emitters_have_readers_or_triage_entries`、`build_server_app()`。
- **agent**：`TiandaoAgent.setChatSignals`、`TiandaoAgent.setNpcDeathEvents`、`TiandaoAgent.setButtonClickEvents`、`LlmClient.chat` user message prompt。
- **client**：`AgentUiPayloadHandler.handleRawRequest`、`parseAndMaybeOpenRawRequest`、`handleRawRequestForReadyClientTests`、`AgentUiPayloadHandlerReadyClientTest`。
- **CI / e2e**：`.github/workflows/e2e.yml` client test stage、`server/tests/full_app_startup.rs` full-app startup smoke。

### 遗留 / 后续

- 无遗留阻塞。本 plan 范围是测试盲区守护，不承担 P1 triage 中列出的业务孤岛修复；这些仍按后续 gameplay / 代码断连狩猎 plan 消费。
