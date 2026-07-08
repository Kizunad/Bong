# plan-bughunt-bot-combat-server-data-type-false-positive-v1

> BugHunt skeleton。分区：e2e-protocol。主题：战斗 bot e2e 的 `bong:server_data` 类型断言在 protobuf 生产态下退化为“任意 server_data 即通过”，导致 `combat_event` / `cast_sync` 断链可被 CI 漏掉。

## 一句话 bug

`scripts/bot/scenarios/_combat_helpers.py::wait_for_server_data_after` 对 protobuf `bong:server_data` 不解析 oneof，`_server_data_type_matches` 在 JSON type 解析失败时直接返回 `True`，使战斗场景声称等待 `combat_event` / `cast_sync`，实际可被 heartbeat 等任意 protobuf server_data 满足。

## 实际游玩体验影响

- 近战命中 `combat_event` 飘字断链时，玩家会看到攻击命中但没有伤害/格挡/毒伤等战斗浮字反馈，bot e2e 仍可能绿。
- 技能施放的 `cast_sync` 断链时，玩家的施法条、完成/中断状态与 server 权威状态不同步；`combat_skill_cast.py` 仍有 `bong:vfx_event` 专属断言，但无法证明 `cast_sync` 或 `combat_event` 已到达。
- 这会削弱 #980 已落地战斗 bot e2e 对玩家可感知战斗反馈协议的保护，不是单纯“未来深断言”优化。

## 复现路径

1. 构造合法 protobuf heartbeat：`b"\x12\x04\x0a\x02ok"`，即 `ServerDataEnvelope.heartbeat { message: "ok" }`。
2. `proto_min.server_data_payload_name(heartbeat)` 返回 `heartbeat`，证明该 bytes 是可识别的 server_data oneof。
3. 调用 `_server_data_type_matches(heartbeat, {"combat_event"})`，当前返回 `True`。
4. 最小模拟：anchor 后只放一个 `Event(kind="payload", channel="bong:server_data", data=heartbeat)`，调用 `wait_for_server_data_after(... expected_json_types={"combat_event"})` 会返回该 heartbeat 事件；全程没有 `combat_event`。

## 根因证据

- `scripts/bot/scenarios/_combat_helpers.py:65`：`wait_for_server_data_after` 只检查 raw `payload` 事件和 `channel == "bong:server_data"`。
- `scripts/bot/scenarios/_combat_helpers.py:111`：`_server_data_type_matches` 只尝试 JSON `type`。
- `scripts/bot/scenarios/_combat_helpers.py:113`：注释承认生产态 server_data 是 protobuf。
- `scripts/bot/scenarios/_combat_helpers.py:115`：`payload_type is None or payload_type in expected_types` 让所有非 JSON protobuf 落入通过分支。
- `scripts/bot/scenarios/combat_attack_hit.py:29`：近战场景期待 `{"combat_event"}`。
- `scripts/bot/scenarios/combat_skill_cast.py:69`：凝针场景期待 `{"cast_sync", "combat_event"}`。
- `scripts/bot/proto_min.py:349`：`SERVER_DATA_PAYLOAD_NAMES` 只登记到 `lingtian_session = 31`，缺战斗 oneof。
- `proto/bong/envelope.proto:51`：`cast_sync = 34`。
- `proto/bong/envelope.proto:69`：`combat_event_floater = 51`，client bridge 映射为 legacy JSON type `combat_event`。
- `client/src/main/java/com/bong/client/network/ProtoServerDataBridge.java:103`：`COMBAT_EVENT_FLOATER -> "combat_event"`。
- `server/src/network/mod.rs:3381`：`process_bridge_messages` 可向所有 client 广播 heartbeat server_data，heartbeat 是真实干扰源而非随意 bytes。

## 去重说明

- 不重复 #974 / #988 / #994 / #999 / #1010 / #1021：那些是具体玩法 C2S/S2C 协议漂移，本题是 bot e2e 断言层的战斗 server_data 类型假阳性。
- 不重复 `docs/plans-skeleton/plan-bot-e2e-coverage-v1.md` P6：P6 是未来“全量 protobuf 深断言 / Python binding”决策；本题是已上线 `combat_attack_hit.py` / `combat_skill_cast.py` 的具体 false-positive，可用零依赖 oneof 名称浅解析修复。

## 修复计划骨架

- P0：在 `scripts/bot/proto_min.py::SERVER_DATA_PAYLOAD_NAMES` 补 `34: "cast_sync"`、`51: "combat_event_floater"`。
- P0：把 `_combat_helpers.wait_for_server_data_after` 改为优先使用 `proto_min.server_data_payload_name(data)`；允许 `combat_event` 期望匹配 `combat_event_floater`，或把场景期望名改成 proto oneof 名。
- P1：给 `_combat_helpers` 补单测：合法 heartbeat protobuf 不得匹配 `combat_event`；合法 `cast_sync` / `combat_event_floater` oneof 才能匹配对应期望。
- P1：让 `combat_attack_hit.py` 与 `combat_skill_cast.py` 的描述区分 `bong:vfx_event` 覆盖和 `bong:server_data` 覆盖，避免再次把“任意 server_data”写成“战斗反馈 payload”。

## 验证计划

- `cd scripts/bot && python3 -m unittest test_protocol.py` 或仓库既有 bot 协议测试命令。
- 新增最小单测：`heartbeat` oneof 对 `{"combat_event"}` 返回 false。
- 新增最小单测：`cast_sync` oneof 对 `{"cast_sync"}` 返回 true。
- 新增最小单测：`combat_event_floater` oneof 对 `{"combat_event"}` 或 `{"combat_event_floater"}` 返回 true，取决于修复时决定的权威期望名。
- 跑战斗 bot 场景：`scripts/bot/scenarios/combat_attack_hit.py`、`scripts/bot/scenarios/combat_skill_cast.py`，确认它们不再被 unrelated heartbeat/server_data 满足。

## 对抗结论

反方第一轮质疑：原候选缺真实时序证明、随意 bytes 不够贴近生产 protobuf、`combat_skill_cast.py` 仍有 VFX 断言、命名需区分 `combat_event_floater` 与 legacy `combat_event`、需说明与 P6 深断言去重。

修正后结论：通过。使用合法 heartbeat protobuf 证明当前 helper 在无 `combat_event/cast_sync` 时仍会满足战斗场景断言；范围收窄为 #980 已落地战斗 e2e 的具体假阳性，不依赖全量 protobuf 深断言。
