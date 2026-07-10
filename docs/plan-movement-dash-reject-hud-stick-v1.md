# plan-movement-dash-reject-hud-stick-v1

> 一句话主题：把 `MovementState.rejected_action` 收口为 server 发送后即消费的 edge-triggered one-shot，阻止体力恢复包把一次 dash 拒绝反复续成 HUD 常驻。
>
> 来源：bug-hunt report-only skeleton；2026-07-11 升格为 BugFix active plan。范围仅限 movement state 下发与既有 HUD 时序契约，不改 movement schema、不新增动画/VFX/SFX、不改变 dash 数值。

## 阶段总览

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | 证真、修复 dash reject one-shot 生命周期并锁定跨层 HUD 时序 | ⏳ |

## 接入面

- **进料**：`server/src/movement/mod.rs::handle_movement_action_intents` 接收正常玩家 `MovementActionIntent::Dashing`；`combat::lifecycle::stamina_tick` 改变 `Stamina` 后触发 movement state 再发送。
- **出料**：`emit_movement_state_payloads` 经 `bong:server_data` 发送 `MovementStateV1`；client `MovementStateStore.replace` 记录拒绝发生时刻，`MovementHudPlanner` 渲染 300ms 红色 flash 与 3s visible + 500ms fade。
- **共享类型 / event**：复用 `MovementState`、`Stamina`、`MovementActionRequestV1::Dash`、`MovementStateV1` 与 `ServerDataPayloadV1::MovementState`，不另建拒绝事件或序列字段。
- **跨仓库契约**：server `MovementState::to_payload` / `emit_movement_state_payloads` → schema `MovementStateV1.rejected_action` → client `MovementStateHandler` / `MovementStateStore` / `MovementHudPlanner`。
- **worldview / qi_physics**：纯 movement HUD 生命周期修复，不涉及世界观命名、真元/灵气流动或守恒 ledger。

## P0 — dash reject one-shot 生命周期

### 第一性原理验证

1. 从正常玩家输入链确认 dash intent 可达 `handle_movement_action_intents`，并在冷却、体力不足、动作占用等拒绝分支写入 `MovementState.rejected_action = Some(Dash)`。
2. 确认 `emit_movement_state_payloads` 的过滤器包含 `Changed<Stamina>`；恢复期 `stamina_tick` 每 4 tick 修改 `Stamina.current`，会在没有新拒绝时继续发送 movement state。
3. 确认当前 `MovementState::to_payload` 每次原样复制持久的 `rejected_action`；client `MovementStateStore.replace` 对每个非空 reject 都刷新 `rejectedAtMs` 与 `hudActivityAtMs`。
4. 以失败测试证明：第一次 payload 带 `Dash` 后，仅由后续状态发送生成的 payload 仍带 `Dash`，违反 `docs/finished_plans/plan-movement-v1.md` P1 的 0.3s one-shot HUD 契约。

### 修复决议

- 在 `server/src/movement/mod.rs` 增加发送边界上的 one-shot 消费：构造本次 payload 时读取当前 `rejected_action`；payload 成功序列化并交给 client 后清空 server 状态。序列化失败时保留标记，允许下一次发送重试。
- 不在 client 通过“相同枚举值”去重：协议没有 reject sequence，同一玩家连续两次真实 dash reject 也都是 `Dash`，客户端值去重会吞掉合法反馈。
- 不改 `MovementStateV1` schema；既有 animation、VFX、SFX、HUD 样式与 dash 数值全部保持不变。

### 可核验交付物

- `server/src/movement/mod.rs`：`MovementState::take_payload`（或等价发送后消费函数）与 `emit_movement_state_payloads` one-shot 接线。
- server pin：首次 take/send payload 携带 `MovementActionRequestV1::Dash`，紧接的第二次 payload 为 `None`；无 reject、连续两次新 reject、序列化失败保留重试语义均有覆盖。
- `client/src/test/java/com/bong/client/hud/MovementHudPlannerTest.java` / `client/src/test/java/com/bong/client/movement/MovementStateTest.java`：单次 reject 在 300ms 边界后不再闪红，HUD 在 3000ms visible + 500ms fade 后 auto-hide；新的独立 reject 仍可重新触发。
- 门禁：`cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`；`cd client && JAVA_HOME=<JDK17> PATH=<JDK17>/bin:$PATH ./gradlew test build`。

### 验收标准

1. 冷却或体力不足导致的一次 dash reject 只在第一份 movement state payload 中出现；后续仅因 `Changed<Stamina>` 产生的 payload 不再携带旧 reject。
2. client 红色 reject flash 在约 300ms 后结束；movement HUD 在最后一次真实动作/拒绝后的约 3.5s 完全隐藏，不随恢复包续命。
3. 两次独立玩家输入产生的两次 reject 都能各自触发反馈；序列化失败不会静默吞掉尚未下发的 reject。
4. server/client 全门禁通过，JDK 固定为 17；validator 对待审 HEAD 给出 PASS 后才允许 push 与开 PR。

## 验证证据 / 结论

待第一性原理复现与测试完成后填写。
