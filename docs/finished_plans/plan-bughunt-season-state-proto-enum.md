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
2. **P1 ✅ 2026-07-18**：`ProtoServerDataBridgeTest` 为 `SUMMER`、`SUMMER_TO_WINTER`、`WINTER`、`WINTER_TO_SUMMER` 各设专属 bridge pin；四例均以固定 numeric `1/2/3/4` 构包并断言 generated enum 的 `getNumber()`，两种多段后缀必须保留下划线。
3. **P2 ✅ 2026-07-18**：真实 proto bytes 覆盖 nested message 缺席、`SEASON_UNSPECIFIED` 与未知数值 `99`；三例分别预置字段互异的非默认 sentinel，经 bridge、router 与 private 生产 `applyDispatch` 后逐字段断言 `SeasonStateStore` 不变、同包合法 `PlayerStateStore` 已更新，非法可选 season 不吞掉合法 `player_state`、不覆盖 store 或伪造夏季。
4. **P3 ✅ 2026-07-18**：真实 proto envelope 经 `ProtoServerDataBridge`、`ServerDataRouter` 与 private 生产 `BongNetworkHandler.applyDispatch` 后可观察地更新 `PlayerStateStore` 与 `SeasonStateStore`；测试仅以 non-null、`player == null` 的 headless client 进入生产入口，不再暴露或复用 test-only 落库 helper；保持 `SeasonStatePayload` legacy 小写解析契约不变。

## 验收结果

- [x] GitHub e2e 的 `Client stage (gradlew test)` 通过。
- [x] 四个有效 Season、两个多段后缀、missing / UNSPECIFIED / unknown numeric 均有真实 proto bytes 回归。
- [x] `proto bytes → bridge → router → private applyDispatch → PlayerStateStore / SeasonStateStore` 可观察生产链路通过 Java 17 gate 与 exact-SHA validator。
- [x] Schema、Agent、server release build、server tests、smoke/e2e 与 Bot e2e 全部通过。

## 风险

- 只应改 bridge 归一化和测试，不改 server season 物理或 agent Redis schema。
- 不要把季节名新增为 HUD 文本；`plan-jiezeq-v1` 要求 client 通过物象暗示季节，不显式显示季节 tag。
- 注意 `SEASON_UNSPECIFIED` 不应被误映射为 `summer`，否则会掩盖坏包。
- e2e bot 目前更容易只看 `bong:server_data` 是否到达，可能漏掉 Java handler 丢弃嵌套 enum，需要补 client-side bridge/handler pin。

## Finish Evidence

- **落地清单**：
  - P0：`client/src/main/java/com/bong/client/network/ProtoServerDataBridge.java` 的 `bridgePlayerState` 对 nested `season_state.season` 调用 `stripEnumPrefix(..., "SEASON_")`。
  - P1：`client/src/test/java/com/bong/client/network/ProtoServerDataBridgeTest.java::{bridgePlayerStateNormalizesSeasonStateSummer,bridgePlayerStateNormalizesSeasonStateSummerToWinter,bridgePlayerStateNormalizesSeasonStateWinter,bridgePlayerStateNormalizesSeasonStateWinterToSummer}` 以固定 numeric `1/2/3/4` 构包，并分别 pin generated enum `getNumber()` 与 legacy wire 映射。
  - P2：`client/src/test/java/com/bong/client/BongNetworkHandlerTest.java::{missingSeasonStatePreservesEveryExistingSeasonStoreField,unspecifiedSeasonPreservesEveryExistingSeasonStoreField,unknownNumericSeasonPreservesEveryExistingSeasonStoreField}` 为三类输入使用不同非默认 sentinel，经真实 proto bytes → bridge → router → private `applyDispatch` 后逐字段锁定 `SeasonStateStore` no-op，并逐字段断言同包合法 `PlayerStateStore` 已继续落库。
  - P3：`client/src/main/java/com/bong/client/BongNetworkHandler.java::applyDispatch` 保持 private、non-null client 的生产契约，并直接消费 `dispatch.seasonState()` 写入 `SeasonStateStore`；`client/src/test/java/com/bong/client/BongNetworkHandlerTest.java::realPlayerStateProtoDispatchUpdatesStoresThroughPrivateProductionApplyDispatch` 以反射进入该 private 生产入口，从真实 proto bytes 同时对拍完整 `PlayerStateStore` 与 WINTER 全字段落库。
- **关键 commit**：
  - `0446cae112c9e18dbdd5a411bb641d8f39f3df06` — 2026-07-13 — PR #1185 落地 nested Season enum bridge 修复，但误删 skeleton、未留下归档证据。
  - `76b58e228ebcde335e3a120185b48ed9a1de85cb` — 2026-07-18 — 从 `0446cae1^` 的 blob 恢复原 skeleton 并 promotion 为 Active plan。
  - `46a39f7bc994060d6dacec9f3fe0c36c564ce6e4` — 2026-07-18 — 补齐四变体、negative 边界与真实 `applyDispatch → SeasonStateStore` 接线测试。
  - `3f5e3b3547e7344340481547fc01e2ee137e4c5b` — 2026-07-18 — 恢复 `applyDispatch` 的 private / non-null client 契约，并在首轮返工中抽取落库 helper（该 test-only 接缝随后由 `7429a0a4` 移除）。
  - `cc2e4a42c1d1315301df148172e4f93cbf39884d` — 2026-07-18 — 四个 Season 以固定 numeric `1/2/3/4` 构包，并 pin generated enum wire 编号。
  - `38f6a6df22cf1bc78891c34df7e7bfb12a4f6a49` — 2026-07-18 — 以 tree/parents 等价方式重建原 `3d1f6d96` 主线 merge，补齐精确 `Model: gpt-5.6-sol-xhigh` trailer。
  - `42ef019adffdd4d2d355295f4e3c890c26a90ea1` — 2026-07-18 — 更新重写后的提交、Java gate 与 exact-SHA 验收证据。
  - `d52b47aaf251064284c94180667dd00795e16248` — 2026-07-18 — 合并当时最新 `origin/main`；GitHub E2E Redis Smoke run 29648892492 attempt 2 对该 SHA 成功。
  - `7429a0a42bfa5b42c3d4a85eaa6c171f041ed704` — 2026-07-19 — 移除 test-only `applySeasonStateStore` 接缝，测试改为进入 private 生产 `applyDispatch`，并锁定非法 season 不吞合法 `player_state`。
- **测试结果**：历史基线 [E2E run 29648892492](https://github.com/Kizunad/Bong/actions/runs/29648892492) attempt 2 在 `d52b47aaf251064284c94180667dd00795e16248` overall SUCCESS（Bot e2e 29 pass / 1 skip / 0 fail）。review 返工 code HEAD `7429a0a42bfa5b42c3d4a85eaa6c171f041ed704` 在 clean worktree 上通过 Java 17 targeted gate（`BongNetworkHandlerTest`、`ProtoServerDataBridgeTest`、`PlayerStateHandlerTest`）与 `./gradlew clean test build --no-daemon`：14/14 tasks executed，471 suites / 4125 tests，0 failures / 0 errors / 0 skipped，5m35s；fresh、read-only `gpt-5.6-sol-max` validator 对同一 SHA 给出 `PASS 7429a0a42bfa5b42c3d4a85eaa6c171f041ed704`，并核验真实 proto bytes、private 生产 `applyDispatch`、合法 PlayerState 保留与三类非法 season 最终 store no-op。归档文档提交后的最终 HEAD、主线同步、最终 validator 与新 e2e 绑定在固定证据评论 [#5011969518](https://github.com/Kizunad/Bong/pull/1227#issuecomment-5011969518)；只有评论中的 Target HEAD、PR 远端 HEAD 与本归档文件所在 Git HEAD 精确一致且 Verdict 为对应 SHA 的 PASS 时有效。
- **跨仓库核验**：proto `proto/bong/envelope.proto::Season/SeasonState/PlayerState.season_state`；server `server/src/schema/world_state.rs::SeasonV1/SeasonStateV1` 与 `server/src/schema/proto_convert.rs::season_to_proto/season_state_to_proto`；client `ProtoServerDataBridge.bridgePlayerState`、`SeasonStatePayload.readOptional`、`ServerDataRouter`、private `BongNetworkHandler.applyDispatch`、`PlayerStateStore`、`SeasonStateStore`，以及 `BongHudOrchestrator` / `ZoneAtmosphereRenderer` / `SeasonVisualController` 消费面均命中。本修复不改 Agent schema 或季节物理。
- **遗留 / 后续**：本轮已在 Java 17 下完成 targeted 与 clean full client gate；最终 PR HEAD 的外部 validator/e2e 继续由固定证据评论精确绑定。`docs/plan-season-phase-stale-client-v1.md` 处理“跨相位主动同步 cadence”，属于不同根因与独立 plan，本 PR 不混修。
