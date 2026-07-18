# BugHunt Skeleton Plan: player_state season_state proto enum 断链

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

## Skeleton Fix Plan

1. 在 `ProtoServerDataBridge.bridgePlayerState` 中对 `season_state.season` 做嵌套 enum 前缀归一化：
   - `SEASON_SUMMER` -> `summer`
   - `SEASON_SUMMER_TO_WINTER` -> `summer_to_winter`
   - `SEASON_WINTER` -> `winter`
   - `SEASON_WINTER_TO_SUMMER` -> `winter_to_summer`
   - `SEASON_UNSPECIFIED` 应保持不可解析或显式丢弃，避免伪造夏季。
2. 增加 bridge pin 测试：构造 `PlayerState.season_state.season = SEASON_WINTER`，断言 legacy JSON 输出 `winter`。
3. 增加 route/handler 对拍测试：真实 proto envelope 经 bridge + router 后能产生 `dispatch.seasonState().phase() == WINTER`。
4. 保持 `SeasonStatePayload` legacy 小写解析契约不变，避免扩大协议面。

## 验收测试计划

- client: `./gradlew test build`。
- 新增 `ProtoServerDataBridgeTest` 覆盖 `SEASON_WINTER`、`SEASON_SUMMER_TO_WINTER`、`SEASON_WINTER_TO_SUMMER`。
- 新增或扩展 `PlayerStateHandlerTest` / router 测试，验证 proto bridge 后的 `player_state` 会更新 `SeasonStateStore` 所需 dispatch。
- 手动联调时在仓库根设置 `export BONG_SKIP_SKIN_PREFETCH=1` 后跑 `bash scripts/smoke-test-e2e.sh`，并用 `/season set winter` 或等价 dev 命令确认 client 不再停默认夏季。

## 风险

- 只应改 bridge 归一化和测试，不改 server season 物理或 agent Redis schema。
- 不要把季节名新增为 HUD 文本；`plan-jiezeq-v1` 要求 client 通过物象暗示季节，不显式显示季节 tag。
- 注意 `SEASON_UNSPECIFIED` 不应被误映射为 `summer`，否则会掩盖坏包。
- e2e bot 目前更容易只看 `bong:server_data` 是否到达，可能漏掉 Java handler 丢弃嵌套 enum，需要补 client-side bridge/handler pin。
