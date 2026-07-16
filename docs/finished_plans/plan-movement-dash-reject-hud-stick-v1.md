# plan-movement-dash-reject-hud-stick-v1

> 一句话主题：把 `MovementState.rejected_action` 收口为 server 成功入队后消费的 edge-triggered one-shot，阻止体力恢复包把一次 dash 拒绝反复续成 HUD 常驻。
>
> 来源：bug-hunt report-only skeleton；2026-07-11 升格为 BugFix active plan。范围仅限 movement state 下发与既有 HUD 时序契约，不改 movement schema、不新增动画/VFX/SFX、不改变 dash 数值。

## 阶段总览

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | 证真、修复 dash reject one-shot 生命周期并锁定跨层 HUD 时序 | ✅ 2026-07-17 |

## 接入面

- **进料**：`server/src/movement/mod.rs::handle_movement_action_intents` 接收正常玩家的 `MovementActionIntent`，真实 dash 入口为 `MovementActionIntent.action = MovementAction::Dashing`；`combat::lifecycle::stamina_tick` 改变 `Stamina` 后也会触发 movement state emit。
- **出料**：`emit_movement_state_payloads` 经 `bong:server_data` 入队 `MovementStateV1`；client 依次经过 `ProtoServerDataBridge`、`MovementStateHandler`、`MovementStateStore`，最后由 `MovementHudPlanner` 渲染 300ms 红色 flash 与 3s visible + 500ms fade。
- **共享类型 / event**：复用 `MovementState`、`Stamina`、`MovementActionRequestV1::Dash`、`MovementStateV1` 与 `ServerDataPayloadV1::MovementState`，不新增拒绝事件、sequence 或 wire 字段。
- **跨层契约**：server `MovementState::to_payload` / `emit_movement_state_payloads` → Rust schema `server/src/schema/movement.rs::MovementStateV1` → `server/src/schema/proto_convert.rs::movement_state_to_proto` → client `ProtoServerDataBridge` / `MovementStateHandler` / `MovementStateStore` / `MovementHudPlanner`。
- **agent**：不参与。该状态由 server 直接发送给 Fabric client，不进入 Redis 天道 IPC，也不改变 agent 可见世界状态。
- **worldview / qi_physics**：不改。`docs/worldview.md §四 L334-L342` 仅作为既有贴身缩距语义锚点；本 plan 不改变 dash 位移、体力消耗或真元/灵气流动，不触碰 `qi_physics::ledger`。

## §P0 开放问题

1. 一次 dash reject 应在 payload 构造、序列化还是发送队列边界消费；序列化失败后如何在没有外部状态变化时可靠重试？
2. 两次独立但枚举值相同的 `Dash` reject 应在 server 还是 client 区分；是否需要扩 schema sequence？
3. 此修复是否需要 agent、worldview、qi_physics 或新动画/VFX/SFX 接入？

以上问题已在 §P0.1 的 review 返工中补录决议，实施与验收以该决议为准。

## §P0.1 决议（review 返工补录决议，2026-07-16）

### #1 消费边界与失败重试

1. 只有 payload 成功序列化且代码已调用 `Client::send_custom_payload` 把完整 `bong:server_data` payload 写入 Valence client 发送队列后，才调用 `MovementState::acknowledge_payload_sent` 消费 `rejected_action`。该边界只证明成功入队，不宣称网络送达或 client 已接收。
2. 序列化失败发生在 ack 前，必须保留原 reject，并为实体写入 `MovementStateEmitPending`；下一拍由 `MovementStateEmitFilter` 自动选中重试，成功后移除 pending，第三拍不得重复发送。
3. 重试不依赖后续 `Changed<Stamina>`、`Changed<MovementState>` 或其他外部变化，也不在失败时发送半成品 payload。

### #2 相同 Dash reject 的事件语义

1. reject 的 edge-triggered one-shot 由 server 入队边界保证；client 不按枚举值去重。
2. 生产 protobuf 中的 `MOVEMENT_ACTION_DASHING`、`MOVEMENT_ZONE_KIND_NORMAL`、`MOVEMENT_ACTION_REQUEST_KIND_DASH` 必须经 `ProtoServerDataBridge` 归一化，再由 `MovementStateHandler` 写入 store；清理包之后再次收到 `Dash` 仍是新的合法反馈。
3. 不新增 sequence 或修改 `MovementStateV1` wire shape，避免为单点生命周期修复扩大 schema 范围。

### #3 跨层与正典范围

1. agent 不参与：movement state 是 server → Fabric client 的直接 CustomPayload，不进入天道 Redis IPC。
2. worldview 仅保留既有锚点，不回写正典；qi_physics 不适用。
3. 复用既有 HUD、动画、VFX 与 SFX；边界严格限制为 reject 下发、protobuf 解码、store 计时和 HUD 时序，不改 dash 数值或资源。

## P0 — dash reject one-shot 生命周期

### 第一性原理验证

1. 正常玩家输入会以 `MovementActionIntent.action = MovementAction::Dashing` 到达 `handle_movement_action_intents`；冷却、体力不足、动作占用等拒绝分支把 `MovementState.rejected_action` 写为 `Some(MovementActionRequestV1::Dash)`。
2. `MovementStateEmitFilter` 包含 `Changed<Stamina>`；恢复期 `stamina_tick` 会持续修改 `Stamina.current`，因此没有新拒绝时仍可能发送 movement state。
3. 修复前，持久的 `rejected_action` 会被后续 movement state 重复带出；client `MovementStateStore.replace` 对每个非空 reject 都刷新 `rejectedAtMs` 与 `hudActivityAtMs`，从而反复续命闪红和 HUD 隐藏计时。
4. client 不能按相同枚举值去重：两次独立、合法的玩家输入都可能产生 `Dash` reject，而协议没有 sequence。最小正确断点是 server 成功入队后的 ack。

### 实施摘要

- `MovementStateEmitPending` 记录序列化失败后必须自动重试的实体。
- `MovementStateEmitFilter` 同时覆盖 `Added<MovementState>`、`Changed<MovementState>`、`Changed<Stamina>` 与 `With<MovementStateEmitPending>`。
- `emit_movement_state_payloads` 调用可注入 serializer 的 `emit_movement_state_payloads_with`；失败时保留 reject 并插入 pending，成功时移除 pending。
- `send_movement_state_payload` 构造 `MovementStateV1`、完成序列化并经 `send_server_data_payload` 调用 `Client::send_custom_payload` 入队；只有本次 payload 携带 reject 时，入队后才调用 `MovementState::acknowledge_payload_sent`。
- 不修改 `MovementStateV1` schema；client 继续以每次非空 reject 为独立事件，stamina-only 清理包不刷新历史时刻。

### 可核验交付物

- `server/src/movement/mod.rs`：`MovementStateEmitPending`、`MovementStateEmitFilter`、`emit_movement_state_payloads`、`emit_movement_state_payloads_with`、`send_movement_state_payload`、`MovementState::acknowledge_payload_sent`。
- server tests：
  - `movement_emit_system_consumes_reject_and_stamina_followups_stay_clear`
  - `serialization_failure_automatically_retries_on_next_update`
  - `acknowledging_payload_without_rejection_is_idempotent`
- client tests：
  - `prefixedProtoEnumsReachStoreThroughProductionBridge`
  - `sameDashRejectRefreshesTimingAfterClearFollowup`
  - `staminaOnlyFollowupDoesNotRenewPriorRejectionTiming`
  - `rejectedFlashAndHudFadeHonorEveryContractBoundary`
  - `disconnectResetClearsRejectTimingAndSameDashRefreshesInNewSession`
- 双端契约说明：server 的真实 `MockClient` emit 测试在 Rust `cfg(test)` 下固定 pin JSON payload；client 测试则手工构造真实 protobuf，覆盖 `DASHING/NORMAL/DASH → ProtoServerDataBridge → MovementStateHandler → MovementStateStore`。二者共同锁定 server 与 client 两端契约，但不冒充一条由 server bytes 直接喂给 client 的单一 bytes e2e。

### 验收标准

1. 一次 dash reject 只在首份成功入队的 movement state payload 中出现；后续仅因 `Changed<Stamina>` 产生的 payload 不再携带旧 reject。
2. 序列化失败保留 reject 并在无外部状态变化时下一拍自动重试；成功后清 pending，第三拍不重复发送。
3. client 红色 reject flash 在 1300/1301ms 边界正确切换；HUD 在 4000/4001/4499/4500ms 的 visible/fade/hide 边界正确切换。
4. 两次独立但值相同的 dash reject 都能刷新反馈；stamina-only followup 不续命历史 reject；断线 reset 后新 session 的同值 reject 能重新触发。
5. server/client 全门禁通过，JDK 固定为 17；最终 HEAD 的 fresh read-only validator 给出 PASS。

## Finish Evidence

### 落地清单

- P0 server：`server/src/movement/mod.rs` 以 `MovementStateEmitPending` + `MovementStateEmitFilter` 保证失败自动重试，并由 `send_movement_state_payload` 在成功序列化、调用 `Client::send_custom_payload` 入队后执行 `MovementState::acknowledge_payload_sent`。
- P0 Rust schema / proto converter：复用 `server/src/schema/movement.rs::{MovementActionRequestV1, MovementStateV1}` 与 `server/src/schema/proto_convert.rs::movement_state_to_proto`，wire shape 未变。
- P0 client：`ProtoServerDataBridge` 归一化 movement protobuf 枚举，`MovementStateHandler` 写入 `MovementStateStore`，`MovementHudPlanner` 执行 reject flash 与 HUD fade；`MovementKeybindings.resetOnDisconnect` 清除跨 session 时序。
- P0 tests：server 三条 exact 测试锁定成功消费、失败重试与空 ack 幂等；client 五条定向测试锁定生产 bridge、同值重触发、stamina-only followup、全部 HUD 边界与断线 reset。
- 最终 post-merge HEAD：`afc588fe3fac3351b2cebb0b5564914b2f31b12d`。

### 关键 commit

- `e901c5a8` · 2026-07-11 · 升格 plan-movement-dash-reject-hud-stick-v1，明确拒绝提示生命周期。
- `dc22011c` · 2026-07-11 · 增加 server/client 跨层 one-shot 时序 pin。
- `14845dbd` · 2026-07-11 · 在 server 成功发送边界消费 dash reject。
- `c0525b83` · 2026-07-11 · 覆盖真实 ECS 发送与失败重试链。
- `f5262b64` · 2026-07-12 · 完善 movement 拒绝修复审计证据。
- `9afe3dd6` · 2026-07-16 · 完善拒绝重试、HUD 全边界、生产 protobuf bridge 与断线 reset 测试。
- `9eee77c9` · 2026-07-16 · 格式化 movement 拒绝重试实现。
- `afc588fe` · 2026-07-17 · 合并最新 main，并同步最终基线修复。

### 测试结果

- `cd server && cargo fmt --check`：PASS。
- `cd server && cargo clippy --all-targets -- -D warnings`：PASS，耗时 9m03s。
- `cd server && cargo test`：lib `11713 passed / 0 failed / 1 ignored`，main `11 passed`，full_app `1 passed`，tarkov `4 passed`，doc `0 passed / 5 ignored`；总计 `11729 passed / 0 failed / 6 ignored`。
- `cd client && JAVA_HOME=<Temurin 17.0.19> PATH=<Temurin 17.0.19>/bin:$PATH ./gradlew test build`：`BUILD SUCCESSFUL`，耗时 4m44s；JUnit XML 汇总 `4095 tests / 0 failures / 0 errors / 0 skipped`。
- fresh read-only validator（`gpt-5`）对 `afc588fe3fac3351b2cebb0b5564914b2f31b12d`：PASS；server exact `3/3`、Java 17 定向 `39/39`，`0 blocking / 0 major`。
- 旧 clippy 与 POI blocker 已由合入 main 解决，最终完整门禁已证实全绿，本 plan 状态不再阻塞。

### 跨仓库核验

- server：`MovementActionIntent.action = MovementAction::Dashing` → `handle_movement_action_intents` → `MovementState.rejected_action` → `emit_movement_state_payloads(_with)` → `send_movement_state_payload` → `MovementState::acknowledge_payload_sent`。
- Rust schema / proto converter：`MovementStateV1.rejected_action` → `movement_state_to_proto` → `MovementStateProto.rejected_action`。
- client：`ProtoServerDataBridge` → `MovementStateHandler` → `MovementStateStore.replace` → `MovementHudPlanner`；断线走 `MovementKeybindings.resetOnDisconnect`。
- agent：不参与；worldview 与 qi_physics 均未修改。

### 遗留 / 后续

- 本 plan 无阻塞遗留；旧 clippy 与 POI 基线问题已在最新 main 中解决并由最终门禁复验。
- 本 plan 不新增 schema sequence；server one-shot 入队边界已能保留两次独立 `Dash` reject 的事件语义。
- server MockClient JSON pin 与 client 手工 protobuf pin 是双端契约证据；若后续需要单一 bytes e2e，应另立范围接通 production server bytes 到 Fabric decoder，不在本 bugfix 中伪报覆盖。
