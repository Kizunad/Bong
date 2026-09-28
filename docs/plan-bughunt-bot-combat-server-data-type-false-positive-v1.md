# plan-bughunt-bot-combat-server-data-type-false-positive-v1

> BugHunt 验证记录。分区：e2e-protocol。主题：核验战斗 bot e2e 的 `bong:server_data` 类型断言是否会把任意 protobuf 消息误判为 `combat_event` / `cast_sync`。

## 一句话 bug

该候选描述对应的旧版实现已由主线提交 `7751a90653`（#2212）修复。本次在当前主线重新核验 `scripts/bot/scenarios/_combat_helpers.py::wait_for_server_data_after`、`scripts/bot/server_data.py::decode_server_data_payload` 和 `scripts/bot/proto_min.py::server_data_payload_name`：protobuf oneof 会先被识别为具体运行时类型，heartbeat 不会满足战斗类型等待。

## 实际游玩体验影响

- 上述影响是候选 bug 在旧实现中的风险；当前主线的 `combat_attack_hit.py` 和 `combat_skill_cast.py` 已按具体 `payload_type` 等待，不会因 heartbeat 等无关 server_data 通过。
- `combat_skill_cast.py` 仍独立断言 `bong:vfx_event`，同时对 `cast_sync` 使用 typed server_data 断言，两个反馈面互不替代。

## 复现路径

1. 构造合法 protobuf heartbeat：`b"\x12\x04\x0a\x02ok"`，即 `ServerDataEnvelope.heartbeat { message: "ok" }`。
2. 当前 `proto_min.server_data_payload_name(heartbeat)` 和 `decode_server_data_payload(heartbeat)["type"]` 都返回 `heartbeat`。
3. 构造 field 34 的 `cast_sync`（`b"\x92\x02\x00"`）与 field 51 的 `combat_event_floater`（`b"\x9a\x03\x00"`）；当前分别识别为 `cast_sync` 与兼容运行时名 `combat_event`。
4. 因而 `wait_for_server_data_after(... expected_types={"combat_event"})` 会拒绝 heartbeat，只接受 field 51 的战斗浮字；候选中的“任意 protobuf 均通过”在当前 HEAD 不可复现。

## 根因证据

- `scripts/bot/scenarios/_combat_helpers.py:142-158`：等待 helper 只接受已解码且 `payload_type` 命中期望集合的 server_data。
- `scripts/bot/server_data.py:13-27` 与 `scripts/bot/bot.py:316-334`：生产 protobuf 先解码，再以具体 `payload_type` 发出 `server_data` 事件。
- `scripts/bot/proto_min.py:75-126`、`:218-236`：登记 field 34/51，并把 field 51 的 proto 名 `combat_event_floater` 映射为既有场景兼容名 `combat_event`。
- `scripts/bot/proto_min.py:2294-2311`：`server_data_payload_name` 对 oneof field 使用同一运行时名称桥，不把未知或 heartbeat 伪装成战斗类型。
- `scripts/bot/scenarios/combat_attack_hit.py:29`：近战场景期待 `combat_event`。
- `scripts/bot/scenarios/combat_skill_cast.py:69`：凝针场景期待 `cast_sync` / `combat_event`。
- `proto/bong/envelope.proto:51`：`cast_sync = 34`。
- `proto/bong/envelope.proto:69`：`combat_event_floater = 51`，client bridge 映射为 legacy JSON type `combat_event`。
- `client/src/main/java/com/bong/client/network/ProtoServerDataBridge.java:103`：`COMBAT_EVENT_FLOATER -> "combat_event"`。
- `server/src/network/mod.rs:3381`：`process_bridge_messages` 可向所有 client 广播 heartbeat server_data，heartbeat 是真实干扰源而非随意 bytes。

## 去重说明

- 不重复 #974 / #988 / #994 / #999 / #1010 / #1021：那些是具体玩法 C2S/S2C 协议漂移，本题是 bot e2e 断言层的战斗 server_data 类型假阳性。
- 不重复 `docs/finished_plans/plan-bot-e2e-coverage-v1.md` P6：`7751a90653` 已落地本题所需的 oneof 身份登记、运行时兼容名和 bot 解码链路；本次只核验其在当前主线仍能阻断 heartbeat 假阳性。

## 验证结论

- P0：✅ 2026-09-28。主线 `7751a90653`（PR #2212）已补齐 field 34/51 的 oneof 登记、runtime name bridge 和 protobuf 解码路径；本 plan 没有新的生产代码改动。
- P1：✅ 2026-09-28。现有 `CombatServerDataGateTest`、oneof identity matrix 和 cast/combat decoder tests 已覆盖 heartbeat 拒绝、cast_sync 命中、combat_event 兼容名；本次以当前 HEAD 和实际 bot-e2e 重新验证。
- 结论：候选 bug 在当前主线不成立，证据是合法 heartbeat 解码为 `heartbeat`，两个战斗场景的 typed 等待不会接受它；保留本 plan 作为对旧候选的核验记录。

## 验证计划

- `python3 scripts/bot/test_protocol.py`：569 tests，全部通过。
- `BOT_E2E_PROFILE=debug BOT_E2E_SCENARIOS='combat_attack_hit,combat_skill_cast' bash scripts/bot-e2e.sh`：两个场景均 PASS（2/2），且同一 harness 的 runner/tee 失败优先级测试确认真实失败仍返回非零。

## 对抗结论

反方第一轮质疑：原候选缺真实时序证明、随意 bytes 不够贴近生产 protobuf、`combat_skill_cast.py` 仍有 VFX 断言、命名需区分 `combat_event_floater` 与 legacy `combat_event`、需说明与 P6 深断言去重。

修正后结论：不成立。当前 HEAD 已包含 #2212 的 oneof 身份解码和严格 typed 等待；合法 heartbeat 不会满足战斗断言，真实 combat/cast 场景通过，runner failure propagation 测试仍保持失败可见。

## Finish Evidence

### 落地清单

- 本 PR 仅更新本 plan 的验真与归档证据；生产修复已存在于 `scripts/bot/proto_min.py`、`scripts/bot/server_data.py`、`scripts/bot/bot.py` 和 `scripts/bot/scenarios/_combat_helpers.py`。
- `SERVER_DATA_PAYLOAD_NAMES` 登记 field 34 `cast_sync` 与 field 51 `combat_event_floater`，`SERVER_DATA_PAYLOAD_RUNTIME_NAMES` 保留场景兼容名 `combat_event`；等待 helper 只接受解码后的具体 `payload_type`。

### 关键 commit

- `f0a2e7e01`（2026-09-28）：提升 skeleton 为 active plan。
- `7751a90653`（2026-09-13，PR #2212）：补齐 Bot server_data oneof identity、解码和战斗场景 typed 断言链路。

### 测试结果

- `python3 scripts/bot/test_protocol.py`：569 passed，0 failed。
- `BOT_E2E_PROFILE=debug BOT_E2E_SCENARIOS='combat_attack_hit,combat_skill_cast' bash scripts/bot-e2e.sh`：`combat_attack_hit` PASS、`combat_skill_cast` PASS，2/2。
- `CombatServerDataGateTest.test_wait_ignores_raw_heartbeat_unknown_and_malformed_payloads`、`test_server_data_identity_dispatch_covers_every_oneof_tag`、`test_bot_e2e_pipeline_propagates_runner_then_tee_status` 均包含在上述协议测试中，分别锁定误报阻断、oneof 身份和真失败传播。

### 跨仓库核验

- bot：`proto_min.server_data_payload_name`、`server_data.decode_server_data_payload`、`_combat_helpers.wait_for_server_data_after`。
- server：`proto/bong/envelope.proto` 的 `cast_sync = 34`、`combat_event_floater = 51`，以及真实 heartbeat 广播路径 `server/src/network/mod.rs::process_bridge_messages`。
- client：`ProtoServerDataBridge` 将 `COMBAT_EVENT_FLOATER` 映射为既有 `combat_event` runtime name；agent 不参与该 bot 协议断言。

### 遗留 / 后续

- 本 plan 不新增 protobuf Python binding，也不扩大到非战斗 server_data 深解码；后续 oneof 字段新增仍应由既有 identity matrix 发现并补齐。
