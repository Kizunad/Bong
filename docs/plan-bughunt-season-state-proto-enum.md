# plan-bughunt-season-state-proto-enum

> 一句话主题：修复 `player_state.season_state.season` 的 protobuf 枚举前缀断链，并用真实 proto bytes 把 bridge、router、`SeasonStateStore` 与季节视觉消费契约锁死。

## 阶段总览

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | nested `season_state.season` 的 `SEASON_` 前缀归一化 | ✅ 2026-07-18 |
| P1 | 四个有效 Season 与两种多段枚举专属 pin | ✅ 2026-07-18 |
| P2 | missing / UNSPECIFIED / unknown numeric 安全 no-op | ✅ 2026-07-18 |
| P3 | 真实 proto bytes → bridge → router → applyDispatch → store 接线 | ✅ 2026-07-18 |

## Bug 摘要

生产 `bong:server_data` 的 `player_state` 已携带 `season_state`，但 Fabric client 的 proto bridge 只归一化顶层 `realm` 枚举，未归一化嵌套 `season_state.season`。protobuf JSON 会把季节写成 `SEASON_WINTER` / `SEASON_SUMMER_TO_WINTER` 等全名，而 `SeasonStatePayload` 只认 legacy wire 值 `summer` / `summer_to_winter` / `winter` / `winter_to_summer`。结果是 client 收到真实生产包后丢弃季节状态，`SeasonStateStore` 不更新。

## 实际游玩体验影响

fresh client 会停在默认夏季；如果 client 曾通过旧格式或测试路径写入过季节状态，则会停在旧季节，无法随 server 的 `player_state` 更新。玩家实际看到的是季节视觉长期失真：冬季雾色/天空冷色、雪粒、季节灵气条饱和度、突破/吐纳粒子密度等都不会按服务器当前季节变化。

这不是单纯 UI 文案问题。`SeasonStateStore` 是季节视觉的生产读源，`ZoneAtmosphereRenderer`、`ZoneAtmospherePlanner`、`SeasonVisuals`、`SeasonParticleEmitter`、`MiniBodyHudPlanner`、突破/吐纳粒子路径都读取它。

## 证据定位

- `server/src/network/mod.rs:2083-2127`：每次 `player_state` S2C 都从 `WorldSeasonState` 或 `query_season` 注入 `season_state`。
- `proto/bong/envelope.proto:371-383`：`Season` 是 protobuf enum，canonical JSON 输出为 `SEASON_*` 全名。
- `client/src/main/java/com/bong/client/network/ProtoServerDataBridge.java:1250-1255`：`bridgePlayerState` 只执行 `normalizeRealmField(root, "realm")`，没有处理 `season_state.season`。
- `client/src/main/java/com/bong/client/network/SeasonStatePayload.java:20-25`：季节解析失败则返回 `Optional.empty()`。
- `client/src/main/java/com/bong/client/state/SeasonState.java:35-45`：`fromWire` 只认小写无前缀 wire 值。
- `client/src/main/java/com/bong/client/BongNetworkHandler.java:818-820`：只有 `dispatch.seasonState()` 存在时才写 `SeasonStateStore`。
- `client/src/test/java/com/bong/client/network/ProtoServerDataBridgeTest.java:2066-2077`：现有 bridge 测试只 pin `realm` enum。
- `client/src/test/java/com/bong/client/network/PlayerStateHandlerTest.java:100-112`：现有 handler 测试只喂 legacy 小写季节值，没覆盖 proto bridge 输出。

## 触发路径

1. server tick 构造 `ServerDataPayloadV1::PlayerState`，注入 `season_state = Some(SeasonStateV1)`。
2. `serialize_server_data_payload` 走 protobuf `ServerDataEnvelope`。
3. Fabric client 收到 `bong:server_data`，`ProtoServerDataBridge.bridgePlayerState` 转 legacy JSON。
4. 输出形如 `"season_state":{"season":"SEASON_WINTER",...}`。
5. `PlayerStateHandler` 调 `SeasonStatePayload.readOptional`，`SeasonState.Phase.fromWire("SEASON_WINTER")` 失败。
6. `ServerDataDispatch.seasonState()` 为空，`BongNetworkHandler.applyDispatch` 不写 `SeasonStateStore`。
7. 季节相关视觉继续读取默认夏季或旧季节状态。

## 反方审查记录

### Round 1

反方攻击点：可能不是 production 路径、可能已有测试覆盖、可能与 #873 / #927 / #1006 或已知 e2e PR 重复。

结论：通过。`PLAYER_STATE` 确实走 proto bridge；bridge 只归一化 `realm`；client 解析器只认小写季节值；现有测试只覆盖 realm 或 legacy 小写 fixture。#873 是 `zone_environment` 跨位面重发问题，#927 是 `tide_sky_omen`，#1006 是 skill milestone world_state schema，均不是 `player_state.season_state` 嵌套 enum bridge。

### Round 2

反方攻击点：是否存在生产旁路写 `SeasonStateStore`；影响是否应表述为“默认夏季”。

结论：通过，但修正文案。生产写入口只看到 `BongNetworkHandler.applyDispatch -> SeasonStateStore.replace`，来源是 `PlayerStateHandler` 的 dispatch；`world_state.season_state` / `bong:season_changed` 是 Redis/agent 路径，不写 Fabric client store。准确影响是：fresh client 停默认夏季；已有旧值的 client 停旧季节。

## 落地阶段

1. **P0 ✅ 2026-07-18**：在 `ProtoServerDataBridge.bridgePlayerState` 中对 `season_state.season` 做嵌套 enum 前缀归一化：
   - `SEASON_SUMMER` -> `summer`
   - `SEASON_SUMMER_TO_WINTER` -> `summer_to_winter`
   - `SEASON_WINTER` -> `winter`
   - `SEASON_WINTER_TO_SUMMER` -> `winter_to_summer`
   - `SEASON_UNSPECIFIED` 应保持不可解析或显式丢弃，避免伪造夏季。
2. **P1 ✅ 2026-07-18**：`ProtoServerDataBridgeTest` 为 `SUMMER`、`SUMMER_TO_WINTER`、`WINTER`、`WINTER_TO_SUMMER` 各设专属 bridge pin，两种多段后缀必须保留下划线。
3. **P2 ✅ 2026-07-18**：真实 proto bytes 覆盖 nested message 缺席、`SEASON_UNSPECIFIED` 与未知数值 `99`；非法可选 season 不吞掉合法 `player_state`，且不得覆盖 store 或伪造夏季。
4. **P3 ✅ 2026-07-18**：真实 proto envelope 经 `ProtoServerDataBridge`、`ServerDataRouter`、`BongNetworkHandler.applyDispatch` 后可观察地更新 `SeasonStateStore`；保持 `SeasonStatePayload` legacy 小写解析契约不变。

## 验收结果

- [x] GitHub e2e 的 `Client stage (gradlew test)` 通过。
- [x] 四个有效 Season、两个多段后缀、missing / UNSPECIFIED / unknown numeric 均有真实 proto bytes 回归。
- [x] `proto bytes → bridge → router → applyDispatch → SeasonStateStore` 可观察链路通过静态 validator 与 CI。
- [x] Schema、Agent、server release build、server tests、smoke/e2e 与 Bot e2e 全部通过。

## 风险

- 只应改 bridge 归一化和测试，不改 server season 物理或 agent Redis schema。
- 不要把季节名新增为 HUD 文本；`plan-jiezeq-v1` 要求 client 通过物象暗示季节，不显式显示季节 tag。
- 注意 `SEASON_UNSPECIFIED` 不应被误映射为 `summer`，否则会掩盖坏包。
- e2e bot 目前更容易只看 `bong:server_data` 是否到达，可能漏掉 Java handler 丢弃嵌套 enum，需要补 client-side bridge/handler pin。

## Finish Evidence

- **落地清单**：
  - P0：`client/src/main/java/com/bong/client/network/ProtoServerDataBridge.java` 的 `bridgePlayerState` 对 nested `season_state.season` 调用 `stripEnumPrefix(..., "SEASON_")`。
  - P1/P2：`client/src/test/java/com/bong/client/network/ProtoServerDataBridgeTest.java` 锁定四个有效 Season、两种多段后缀，以及 missing / `SEASON_UNSPECIFIED` / unknown numeric `99`。
  - P3：`client/src/main/java/com/bong/client/BongNetworkHandler.java::applyDispatch` 写入 `SeasonStateStore`；`client/src/test/java/com/bong/client/BongNetworkHandlerTest.java::realPlayerStateProtoDispatchUpdatesSeasonStateStoreThroughNetworkApply` 从真实 proto bytes 对拍到可观察 store 状态。
- **关键 commit**：
  - `0446cae112c9e18dbdd5a411bb641d8f39f3df06` — 2026-07-13 — PR #1185 落地 nested Season enum bridge 修复，但误删 skeleton、未留下归档证据。
  - `76b58e228ebcde335e3a120185b48ed9a1de85cb` — 2026-07-18 — 从 `0446cae1^` 的 blob 恢复原 skeleton 并 promotion 为 Active plan。
  - `46a39f7bc994060d6dacec9f3fe0c36c564ce6e4` — 2026-07-18 — 补齐四变体、negative 边界与真实 `applyDispatch → SeasonStateStore` 接线测试。
- **测试结果**：静态 `git diff --check` PASS；generation-2 read-only validator 对 `46a39f7bc994060d6dacec9f3fe0c36c564ce6e4` 给出 PASS。手动 workflow dispatch [E2E run 29632931911](https://github.com/Kizunad/Bong/actions/runs/29632931911) 在同一精确 SHA 上 overall SUCCESS：`Client stage (gradlew test)`、Schema、Agent、server release build、`Server stage (cargo test)`、Smoke/E2E、Bot e2e 与 evidence artifact upload 均成功。
- **跨仓库核验**：proto `proto/bong/envelope.proto::Season/SeasonState/PlayerState.season_state`；server `server/src/schema/world_state.rs::SeasonV1/SeasonStateV1` 与 `server/src/schema/proto_convert.rs::season_to_proto/season_state_to_proto`；client `ProtoServerDataBridge.bridgePlayerState`、`SeasonStatePayload.readOptional`、`ServerDataRouter`、`BongNetworkHandler.applyDispatch`、`SeasonStateStore`，以及 `BongHudOrchestrator` / `ZoneAtmosphereRenderer` / `SeasonVisualController` 消费面均命中。本修复不改 Agent schema 或季节物理。
- **遗留 / 后续**：本地 Gradle 因已知 sandbox network namespace/socket 限制未盲跑，权威可执行 gate 为上述精确 SHA 的 GitHub e2e。`docs/plan-season-phase-stale-client-v1.md` 处理“跨相位主动同步 cadence”，属于不同根因与独立 plan，本 PR 不混修。
