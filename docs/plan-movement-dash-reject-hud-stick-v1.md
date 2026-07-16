# plan-movement-dash-reject-hud-stick-v1

> 一句话主题：把 `MovementState.rejected_action` 收口为 server 发送后即消费的 edge-triggered one-shot，阻止体力恢复包把一次 dash 拒绝反复续成 HUD 常驻。
>
> 来源：bug-hunt report-only skeleton；2026-07-11 升格为 BugFix active plan。范围仅限 movement state 下发与既有 HUD 时序契约，不改 movement schema、不新增动画/VFX/SFX、不改变 dash 数值。

## 阶段总览

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | 证真、修复 dash reject one-shot 生命周期并锁定跨层 HUD 时序 | [BLOCKED: Rust 1.96.1 下 `cargo clippy --all-targets -- -D warnings` 命中 origin/main 既有 69 个 lint；full `cargo test` 还稳定命中 1 个无关 POI 墙钟性能阈值失败，本 plan 不跨 scope 修全仓基线] |

## 接入面

- **进料**：`server/src/movement/mod.rs::handle_movement_action_intents` 接收正常玩家 `MovementActionIntent::Dashing`；`combat::lifecycle::stamina_tick` 改变 `Stamina` 后触发 movement state 再发送。
- **出料**：`emit_movement_state_payloads` 经 `bong:server_data` 发送 `MovementStateV1`；client `MovementStateStore.replace` 记录拒绝发生时刻，`MovementHudPlanner` 渲染 300ms 红色 flash 与 3s visible + 500ms fade。
- **共享类型 / event**：复用 `MovementState`、`Stamina`、`MovementActionRequestV1::Dash`、`MovementStateV1` 与 `ServerDataPayloadV1::MovementState`，不另建拒绝事件或序列字段。
- **跨仓库契约**：server `MovementState::to_payload` / `emit_movement_state_payloads` → 既有 schema/protobuf `MovementStateV1.rejected_action` → client `ProtoServerDataBridge` / `MovementStateHandler` / `MovementStateStore` / `MovementHudPlanner`。**agent 不参与**：该状态经 `bong:server_data` 由 server 直达 Fabric client，不经过 Redis 天道 IPC，也不改变 agent 可见世界状态。
- **worldview 锚点**：`docs/worldview.md §四 L334-L342` 将贴身缩距定为末法战斗轴心；本 plan 只修复既有 dash 拒绝反馈的 one-shot 生命周期，**无需修改 worldview 正典**。
- **qi_physics 锚点**：**不适用**。本 plan 不改变 dash 位移、体力消耗或任何真元/灵气流动，不新增物理常数，也不触碰 `qi_physics::ledger`。

## §P0 开放问题（P0 决策门前需收口）

1. 一次 dash reject 应在 payload 构造、序列化还是实际交付后的哪个边界消费；序列化失败后如何在没有外部状态变化时可靠重试？
2. 两次独立但枚举值相同的 `Dash` reject 应在 server 还是 client 区分；是否需要扩 schema sequence？
3. 此修复是否需要 agent、worldview、qi_physics 或新动画/VFX/SFX 接入？

以上问题全部已在 §P0.1 收口。原列表保留以备追溯，实施与验收以 §P0.1 决议为准。

## §P0.1 决议（pre-P0 收口，2026-07-16）

### #1 消费边界与失败重试

**决议**：
1. 仅在 movement payload 成功序列化并交给 `Client` 后消费 `rejected_action`；失败发生在 ack 前，必须保留原 reject。
2. 序列化失败时给该玩家实体写入 `MovementStateEmitPending`；下一拍由 emit filter 自动选中并重试，成功后移除 pending，第三拍不得重复发送。
3. 不依赖后续 `Changed<Stamina>` 或其他外部变化救活失败消息，也不在失败时发送半成品 payload。

**落点**：`server/src/movement/mod.rs:475-552`（pending filter、发送与成功 ack）/ `server/src/movement/mod.rs:1370-1431`（失败、无外部变化重试、第三拍不重复）/ plan §P0「可核验交付物」与 §P0.1 #1。

### #2 相同 Dash reject 的事件语义

**决议**：
1. reject 的 edge-triggered one-shot 由 server 发送边界保证；client 不按枚举值去重。
2. 既有 protobuf 前缀枚举必须走 `ProtoServerDataBridge` 归一化，再由 `MovementStateHandler` 写入 store；清理包之后再次收到 `Dash` 仍是新的合法反馈。
3. 不新增 sequence 或修改 `MovementStateV1` wire shape，避免为单点生命周期修复扩大 schema 范围。

**落点**：`client/src/main/java/com/bong/client/network/ProtoServerDataBridge.java:811-819`（三个 movement enum 前缀归一化）/ `client/src/main/java/com/bong/client/network/MovementStateHandler.java:19-35` 与 `client/src/main/java/com/bong/client/movement/MovementStateStore.java:15-34`（handler→store 时序）/ plan §P0「实施摘要」与 §P0.1 #2。

### #3 跨层与正典范围

**决议**：
1. agent 不参与：movement state 是 server→Fabric client 的直接 CustomPayload，不进入天道 Redis IPC。
2. `docs/worldview.md §四 L334-L342` 只作为 dash 贴身缩距的既有正典锚点；本修复不改变玩法语义，因此不回写 worldview。
3. qi_physics 不适用，且复用既有 HUD/动画/VFX/SFX；边界严格限制为 reject 下发、protobuf 解码、store 计时和 HUD 时序，不改 dash 数值或资源。

**落点**：`server/src/movement/mod.rs:309-370`（正常玩家 dash 拒绝入口）/ `client/src/main/java/com/bong/client/hud/MovementHudPlanner.java:10-12,65-78,126-137`（既有 300ms/3s+500ms 契约）/ plan §接入面、§P0「验收标准」与 §P0.1 #3。

## P0 — dash reject one-shot 生命周期

### 第一性原理验证

1. 从正常玩家输入链确认 dash intent 可达 `handle_movement_action_intents`，并在冷却、体力不足、动作占用等拒绝分支写入 `MovementState.rejected_action = Some(Dash)`。
2. 确认 `emit_movement_state_payloads` 的过滤器包含 `Changed<Stamina>`；恢复期 `stamina_tick` 每 4 tick 修改 `Stamina.current`，会在没有新拒绝时继续发送 movement state。
3. 确认当前 `MovementState::to_payload` 每次原样复制持久的 `rejected_action`；client `MovementStateStore.replace` 对每个非空 reject 都刷新 `rejectedAtMs` 与 `hudActivityAtMs`。
4. 以失败测试证明：第一次 payload 带 `Dash` 后，仅由后续状态发送生成的 payload 仍带 `Dash`，违反 `docs/finished_plans/plan-movement-v1.md` P1 的 0.3s one-shot HUD 契约。

### 实施摘要（以 §P0.1 决议为准）

- 在 `server/src/movement/mod.rs` 增加发送边界上的 one-shot 消费：构造本次 payload 时读取当前 `rejected_action`；payload 成功序列化并交给 client 后清空 server 状态。序列化失败时保留 reject 并写入实体级 pending，下一拍无需外部 change 即自动重试。
- 不在 client 通过“相同枚举值”去重：协议没有 reject sequence，同一玩家连续两次真实 dash reject 也都是 `Dash`，客户端值去重会吞掉合法反馈。
- 不改 `MovementStateV1` schema；既有 animation、VFX、SFX、HUD 样式与 dash 数值全部保持不变。

### 可核验交付物

- `server/src/movement/mod.rs`：`MovementState::take_payload`（或等价发送后消费函数）与 `emit_movement_state_payloads` one-shot 接线。
- server pin：`movement_emit_system_consumes_reject_and_stamina_followups_stay_clear` 使用 `MockClient` 真实执行 `emit_movement_state_payloads`，锁定 Added 首包 `Dash`、ack 不自激、`Changed<Stamina>` 后续包 `None`、第二次新 reject 可再发；`serialization_failure_automatically_retries_on_next_update` 用可测 seam 注入 `Oversize` 序列化失败，锁定首拍保留 reject+pending、次拍无外部 change 自动重试成功并消费、第三拍不重复。
- `client/src/test/java/com/bong/client/hud/MovementHudPlannerTest.java` / `client/src/test/java/com/bong/client/network/MovementStateHandlerTest.java`：锁定 1300/1301ms reject flash 与 4000/4001/4499/4500ms HUD fade 边界；`prefixedProtoEnumsReachStoreThroughProductionBridge` 以真实 protobuf 的 Dashing/Normal/Dash 前缀枚举穿过 `ProtoServerDataBridge`→handler→store；新的独立 reject 仍可重新触发。
- 门禁：`cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`；`cd client && JAVA_HOME=<JDK17> PATH=<JDK17>/bin:$PATH ./gradlew test build`。

### 验收标准

1. 冷却或体力不足导致的一次 dash reject 只在第一份 movement state payload 中出现；后续仅因 `Changed<Stamina>` 产生的 payload 不再携带旧 reject。
2. client 红色 reject flash 在约 300ms 后结束；movement HUD 在最后一次真实动作/拒绝后的约 3.5s 完全隐藏，不随恢复包续命。
3. 两次独立玩家输入产生的两次 reject 都能各自触发反馈；序列化失败不会静默吞掉尚未下发的 reject。
4. server/client 全门禁通过，JDK 固定为 17；validator 对待审 HEAD 给出 PASS 后才允许 push 与开 PR。

## 验证证据 / 结论

结论：**真 bug**，且可由正常玩家 dash 输入稳定触发，不是 dev-only 或死代码路径。

1. **玩家路径可达**：`handle_movement_action_intents` 的冷却、体力、动作占用等拒绝分支会在 `server/src/movement/mod.rs:351-370` 把正常 `Dashing` intent 写成 `MovementState.rejected_action = Some(Dash)`。
2. **重发链成立**：`MovementStateEmitFilter` 在 `server/src/movement/mod.rs:475-481` 包含 `Changed<Stamina>`；`stamina_tick` 在 `server/src/combat/lifecycle.rs:272-304` 每 4 tick 写回恢复中的 `Stamina.current`。修复前 `to_payload` 会在每次下发原样复制同一个持久 reject。
3. **HUD 续命链成立**：`MovementStateStore.replace` 在 `client/src/main/java/com/bong/client/movement/MovementStateStore.java:21-34` 对每个非空 reject 重置 `rejectedAtMs` 与 `hudActivityAtMs`；重发间隔 200ms 小于 `MovementHudPlanner.REJECT_FLASH_MS = 300ms`，因此会连续刷新闪红与 3.5s auto-hide 计时。
4. **反方路线排除**：client 不能按相同枚举值去重，因为两次独立合法拒绝同样是 `Dash`，而协议没有 sequence。最小正确断点是 server 发送边界。
5. **修复结果**：`emit_movement_state_payloads` 通过 `send_movement_state_payload` 先构造、序列化并交付 payload，仅成功后调用 `acknowledge_payload_sent` 消费 reject。序列化失败在 ack 之前返回，不会发送半成品或丢失待重试标记。真实 ECS 测试还确认 emit 内 ack 不会在下一 tick 自激发送，之后仅 `Changed<Stamina>` 触发的 payload 为 `rejected_action=None`。
6. **RED 证据**：修复前运行 `cargo test movement::tests::rejected_action_payload_is_one_shot_and_can_be_rearmed -- --exact` 因 one-shot 消费入口缺失而编译红（`E0599: no method named take_payload`）；代码对拍同时确认连续 `to_payload` 会原样复制持久 reject。
7. **局部 GREEN 证据**：`movement_emit_system_consumes_reject_and_stamina_followups_stay_clear` 与 `serialization_failure_keeps_reject_for_next_successful_send` 各 `1 passed`；既有 `cargo test movement::tests::` 为 44 passed；JDK 17.0.19 下 client 相关两组测试通过。

## 当前验证证据（未满足 Finish Evidence，不可归档）

`[BLOCKED: Rust 1.96.1 下执行 cd server && cargo clippy --all-targets -- -D warnings 失败；origin/main 既有 69 个 clippy 诊断，主要为 manual_is_multiple_of / derivable_impls / manual_checked_ops。本 movement PR 新增段无独立诊断。低负载 full cargo test 连续三轮均为 10933 passed / 1 failed / 1 ignored，唯一失败 world::poi_novice::tests::scatter_surface_stashes_terminates_when_existing_poi_blankets_the_aabb，墙钟分别为 12.5747s、12.7438s、11.8222s；该用例单独 exact 复跑 1 passed、7.15s，表明当前全套并行门禁仍存在与 movement 无关的性能阈值阻塞。未获授权跨 scope 修复全仓基线，因此 P0 保持 BLOCKED 且 plan 维持 active。]`

### 落地清单

- P0 server：`server/src/movement/mod.rs::emit_movement_state_payloads` 在 payload 序列化并交付后调用 `MovementState::acknowledge_payload_sent`，使 `rejected_action` 成为可重试的 edge-triggered one-shot。
- P0 server tests：真实 `MockClient + App + emit_movement_state_payloads` 路径覆盖 Added、无自激、`Changed<Stamina>` 与再次 reject；可测 serializer seam 覆盖失败前不 ack 与下次成功重试。
- P0 client tests：`MovementStateHandlerTest` 锁定 stamina-only 包不刷新历史 reject 时间；`MovementHudPlannerTest` 锁定 300ms 闪红边界与 3000ms + 500ms auto-hide。

### 关键 commit

- `e901c5a8` · 2026-07-11 · 升格 plan 并收口拒绝提示生命周期。
- `dc22011c` · 2026-07-11 · 增加 server/client 跨层 one-shot 时序 pin。
- `14845dbd` · 2026-07-11 · 在 server 成功发送边界消费 dash reject。
- `1dce0256` · 2026-07-11 · 回填第一性原理证真证据。
- `c0525b83` · 2026-07-11 · 按 `/review` 要求改为真实 ECS 发送链与可注入失败路径测试。

### 测试结果

- `cd server && cargo fmt --check`：通过。
- `cd server && cargo test`：当前 HEAD 连续三轮均为 `10933 passed, 1 failed, 1 ignored`；唯一失败为 `world::poi_novice::tests::scatter_surface_stashes_terminates_when_existing_poi_blankets_the_aabb` 的 10s 墙钟阈值，实测 `12.5747s`、`12.7438s`、`11.8222s`。该用例单独 `--exact` 复跑为 `1 passed`、`7.15s`；因此不宣称 full suite 全绿。
- `cd client && JAVA_TOOL_OPTIONS=-Djava.io.tmpdir=<worktree-private> JAVA_HOME=<JDK17> PATH=<JDK17>/bin:$PATH ./gradlew test build`：JDK `17.0.19`，`3710` tests，`BUILD SUCCESSFUL`。
- `cd server && cargo clippy --all-targets -- -D warnings`：当前 `rustc 1.96.1 (31fca3adb 2026-06-26)` 对 `origin/main` 既有代码报 69 个 lint（主要为 `manual_is_multiple_of` / `derivable_impls` / `manual_checked_ops`）；本 PR 新增 helper/tests 无独立诊断。原始结果已如实保留，未跨 scope 修改全仓基线。
- 无上下文 read-only validator：旧 verdict 只绑定反馈前 HEAD，因 `c0525b83` 变更已作废；当前 HEAD 待 fresh validator。

### 跨仓库核验

- server：`MovementState::rejected_action` → `emit_movement_state_payloads` → `MovementStateV1.rejected_action`。
- schema：复用既有 `MovementActionRequestV1::Dash` / `MovementStateV1`，未改 wire shape。
- client：`MovementStateHandler` → `MovementStateStore.replace` → `MovementHudPlanner.REJECT_FLASH_MS/HOVER_VISIBLE_MS/HOVER_FADE_MS`。

### 遗留 / 后续

- Rust 1.96.1 新 clippy lint 导致 `origin/main` 级全仓 `-D warnings` 基线不绿；full suite 还存在无关 POI 墙钟性能阈值阻塞。当前 plan 因此保持 `[BLOCKED]`、不归档，待独立仓库维护 PR 修复基线后再完成归档。
- 本 plan 不新增 schema sequence，因为 server one-shot 边界已能区分两次独立 `Dash` reject，避免扩大协议 scope。
