# plan-bughunt-anqi-hud-schema-drift-v1

状态：Skeleton Plan
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

- [ ] 在 `agent/packages/schema/src/server-data.ts` 增加 `AnqiHudV1` / `ServerDataAnqiHudV1` TypeBox schema，字段与 Rust `AnqiHudV1` 对齐。
- [ ] 将 `Type.Literal("anqi_hud")` 加入 `ServerDataType`，并将 `ServerDataAnqiHudV1` 加入 `ServerDataV1` union。
- [ ] 在 `SCHEMA_REGISTRY` / `GENERATED_SCHEMA_FILES` 登记 `server-data-anqi-hud-v1.json`，生成对应 artifact。
- [ ] 增加正反 sample 与 schema 测试：`kind="echo"` / `kind="charge"` / `kind="abrasion"` 正例，缺字段、额外字段、非法数值反例。
- [ ] 增加 Rust/TS parity pin：至少覆盖 Rust wire shape 中的 `type:"anqi_hud"` 与所有必需字段。

## 7. 验证计划

- [ ] `cd agent/packages/schema && npm test`
- [ ] `cd agent/packages/schema && npm run generate:check`
- [ ] `cd agent && npm run build -w @bong/schema`
- [ ] 抽查 `agent/packages/schema/generated/server-data-v1.json` 包含 `anqi_hud` 分支。
- [ ] 抽查 `agent/packages/schema/generated/server-data-anqi-hud-v1.json` 存在，且删除后 freshness gate 会红。

## 8. 对抗复核记录

第一轮候选是 `ServerDataVortexStateV1` 单体 generated artifact 缺失。反方指出 `server-data-v1.json` 已覆盖 `vortex_state`，生产 S2C 走 protobuf + client router，影响偏契约审计且接近 #1061；该候选降级并丢弃。

第二轮收窄为 `anqi_hud`：两名 adversarial reviewer 独立确认 server 已发、client 已路由、TS `ServerDataType` / `ServerDataV1` 无 `anqi_hud`，且不重复 #1059/#1061/#1093/#1111/#1068-#1072。

Adversarial conclusion：PASS。`anqi_hud` 是正式 server_data payload，但 agent/schema 完全漏镜像；实际游玩主链路未必立即断，核心 bug 是暗器 HUD 回归与协议漂移保护缺口。
