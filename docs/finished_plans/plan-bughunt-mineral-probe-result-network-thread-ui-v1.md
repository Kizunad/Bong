# plan-bughunt-mineral-probe-result-network-thread-ui-v1

Skeleton Plan：`mineral_probe_result` S2C 回执在 Fabric 网络线程内直触 HUD/SFX，需改成主线程落地。

## 阶段总览

| 阶段 | 状态 | 目标 |
|---|---|---|
| P0 | ✅ 2026-08-26 | 将 `MineralProbeResultHandler` 改为纯解析 / dispatch 意图，不在 `ServerDataRouter.route()` 内触达 MC HUD/SFX |
| P1 | ✅ 2026-08-26 | 在 `BongNetworkHandler.applyDispatch(...)` 的 main-thread 路径统一落地 actionbar overlay 与本地 SFX |
| P2 | ✅ 2026-08-26 | 增加线程契约回归测试；live runClient 手测因无可驱动 server+Fabric 客户端的 e2e harness 未执行 |

## 实际游玩体验影响

玩家按 N 进行神识感矿脉后，server 回 `mineral_probe_result(found/denied)` 时，客户端当前在 Fabric 网络回调线程内直接调用 actionbar overlay 与本地音效。该路径违反 Fabric raw receiver 的线程契约，实际表现可能是矿脉反馈偶发不显示、音效丢失，或与渲染 / 客户端状态发生竞态；本 plan 不声称已有 crash log。

## 复现路径

1. 启动 server + Fabric client，进入世界。
2. 准星对准矿块或非矿块，按 N 触发 `ClientRequestSender.sendMineralProbe(x,y,z)`。
3. server 经 `mineral_probe` resolver 返回 `mineral_probe_result`。
4. 客户端走 `bong:server_data` raw receiver，同步路由到 `MineralProbeResultHandler`。
5. 观察 found/denied 的 actionbar 与 SFX 由 handler 直接触发，而不是通过 `client.execute(...)` 后的 main-thread applier。

## 根因证据

- Fabric API 1.3.12 sources `ClientPlayNetworking.registerGlobalReceiver(Identifier, PlayChannelHandler)` 注释明确：handler runs on network thread，读完 buffer 后访问 game state 必须经 `execute(Runnable)` 切到 render thread。
- `client/src/main/java/com/bong/client/BongNetworkHandler.java:245` 使用 raw `ClientPlayNetworking.registerGlobalReceiver(new Identifier("bong", "server_data"), ...)`。
- `client/src/main/java/com/bong/client/BongNetworkHandler.java:267` 在 receiver 回调内同步执行 `ROUTER.route(jsonPayload, readableBytes)`。
- `client/src/main/java/com/bong/client/BongNetworkHandler.java:285` 到 `:297` 只有 dispatch 携带 chat/narration/playerState/zone/uiOpen 等字段时才 `client.execute(() -> applyDispatch(...))`。
- `client/src/main/java/com/bong/client/network/ServerDataRouter.java:314` 同步调用 `handler.handle(envelope)`。
- `client/src/main/java/com/bong/client/network/ServerDataRouter.java:267` 到 `:268` 注册 `mineral_probe_result -> new MineralProbeResultHandler()`。
- `client/src/main/java/com/bong/client/network/MineralProbeResultHandler.java:55` 直接取 `MinecraftClient.getInstance()`；`:88` 到 `:93` found 分支直接 `mc.inGameHud.setOverlayMessage(...)` 和 `mc.player.playSound(...)`；`:111` 到 `:117` denied 分支同样直触 HUD/SFX。
- `MineralProbeResultHandler` 返回普通 `ServerDataDispatch.handled(...)`，没有把 HUD/SFX 意图交给后续 main-thread `applyDispatch`。
- 现有 `client/src/test/java/com/bong/client/network/MineralProbeResultHandlerTest.java:164` 到 `:189` 只覆盖 headless 下不抛异常，未锁定网络线程不得触 UI 的契约。

## 修复计划骨架

### P0 - 纯解析 / dispatch 意图

- 修改 `client/src/main/java/com/bong/client/network/MineralProbeResultHandler.java`：
  - `handle(...)` 只解析 `kind`、`display_name_zh`、`remaining_units`、`denial_reason`。
  - 输出结构化 client UI effect，例如 actionbar text/color + local sound id/volume/pitch。
  - 禁止在 `handle(...)` 内调用 `MinecraftClient.getInstance()`、`inGameHud.setOverlayMessage(...)`、`player.playSound(...)`。
- 扩展 `client/src/main/java/com/bong/client/network/ServerDataDispatch.java`：
  - 增加可选 `ActionbarSpec` / `LocalSoundSpec`，或等价的 `MineralProbeFeedbackSpec`。
  - 保持 `ServerDataRouter.route(...)` 纯路由，不执行 UI 副作用。

### P1 - main-thread 落地

- 修改 `client/src/main/java/com/bong/client/BongNetworkHandler.java`：
  - `registerServerDataChannel()` 的 `client.execute(() -> applyDispatch(...))` 判定纳入新 feedback spec。
  - `applyDispatch(...)` 在 main thread 内执行 actionbar overlay 与本地 SFX。
- 保持 `mineral_probe_result` 的 found/denied 文案与颜色规格不变：
  - found：丰度三档颜色 + `BLOCK_AMETHYST_BLOCK_CHIME`。
  - denied：灰色 actionbar + `BLOCK_NOTE_BLOCK_BASS`。

### P2 - 验证

- 新增 / 更新 client 单测：
  - `MineralProbeResultHandlerTest` 锁定 found/denied 只生成 dispatch spec，不触 `MinecraftClient`。
  - `ServerDataRouterTest` 锁定 `route(mineral_probe_result)` 无 UI 副作用。
  - `BongNetworkHandler` 相关测试锁定新 spec 会触发 `client.execute/applyDispatch` 路径。
- 手动 UI 验证：
  - `cd client && ./gradlew test build`。
  - JDK 17 下 `./gradlew runClient`，进服后对矿块 / 非矿块按 N，确认 actionbar 与 SFX 都出现，且日志无网络线程访问 UI 报错。

## 重复避让

- 不重复 #1030 `craft_outcome` 网络线程完成反馈。
- 不重复 #1016 `cast_sync` 网络线程关闭功法配置浮窗。
- 不重复旧 `plan-exploration-probe-return-v1` 的“矿脉回执接通与显示规格”；本 plan 只记录线程落点缺口。
- 不声明这是唯一同类问题；`ShieldBrokenHandler`、`ShieldBlockHitHandler`、`BreakthroughCinematicHandler` 等同类 handler 可作为后续审计，不纳入本 PR 范围。

## 对抗结论

- 第一轮反方质疑：需要证明 raw receiver 确为网络线程、`ROUTER.route(...)` 是否同步执行 handler、后续 `applyDispatch` 是否已保护该路径、是否存在死代码或重复主题。
- 修正 / 反驳：Fabric sources 明确 raw receiver 线程契约；`ServerDataRouter.route(...)` 同步 `handler.handle(...)`；`MineralProbeResultHandler` 直接触 HUD/SFX 且返回普通 `handled(...)`；N 键入口、server emit 系统和 router 注册均可达；重复检索未发现 `mineral_probe_result` 网络线程主题。
- 最终裁决：高置信，支持开 BugHunt PR，不降级为 `NO_CANDIDATE`；适合本轮只新增 skeleton plan。推荐修复路线为“纯解析 / dispatch 意图 + main-thread applier”，而不是在 handler 内临时包一层局部 `client.execute`。

## Finish Evidence

### 落地清单

- P0：`MineralProbeResultHandler` 仅解析 `kind`、文案、余量和拒绝原因，输出 `MineralProbeFeedbackSpec`；`ServerDataDispatch` 携带可选 spec；handler 不再依赖 Minecraft、Text、HUD 或 SFX。
- P1：`BongNetworkHandler` 的 server-data dispatch 判定接入 feedback spec，`applyDispatch(...)` 在 client-thread 内统一应用 actionbar 颜色与 vanilla 音效；`ServerDataRouter` 未增加 UI 副作用。
- P2：补齐 `MineralProbeResultHandlerTest`、`ServerDataRouterTest` 和 `BongServerDataThreadingTest`，覆盖 found/denied spec、三档颜色、音效参数、纯路由和 `client.execute` 顺序。
- production source digest 合同按本 PR 的实际客户端 production diff 更新：`R7InventoryContractTest` 与 `production-source-baseline.tsv`。

### 关键 commit

- `52a4b1803a042d1869991c50f4fa05e647721f9a`（2026-08-26）：promotion，skeleton → active。
- `c8986170e3e7c183fd098d3d71e060b5c37f22bc`（2026-08-26）：引入结构化矿脉反馈并接入主线程 applier。
- `27dfbb028b4a702ca69e8406e0929b713ebcebc9`（2026-08-26）：同步 production source digest 合同。
- `2ebabdc288d5c5d355013cd7b64aa44cf822d660`（2026-08-26）：移除 handler 内遗留 Text helper，收紧为 spec-only 边界。

### 测试结果

- Java 17 完整门禁：`JAVA_HOME=/home/serverkizuna/java/jdk-17.0.19+10 PATH=/home/serverkizuna/java/jdk-17.0.19+10/bin:$PATH flock /tmp/bong-gradle.lock -c 'cd client && ./gradlew test build'`；4941 tests，0 failures、0 errors、0 skipped；3 个 GameTest 全部通过；`BUILD SUCCESSFUL`。
- 受影响测试集：Mineral handler、ServerDataRouter、server-data threading 与 R7 source-baseline 合同通过；validator 记录 82 tests、0 failures/errors。
- validator：`gpt-5.6-luna` 对拍最终 HEAD `2ebabdc288d5c5d355013cd7b64aa44cf822d660`，PASS，已关闭。
- 最终 `git fetch origin` 后 `git merge origin/main`：`Already up to date`，基线 `origin/main=32624f398764e78be60ccccd7b8f47d5172e7b11`。
- live runClient N 键 found/denied 手测未执行：当前仓库没有可直接驱动 server + Fabric 客户端交互的 e2e harness；线程 seam、spec 字段和完整 Gradle/GameTest gate 已覆盖自动化路径。

### 跨仓库核验

- client：`mineral_probe_result` → `MineralProbeResultHandler` → `MineralProbeFeedbackSpec` → `BongNetworkHandler.applyDispatch`。
- server、schema、agent 未修改；wire payload、`mineral_probe_result` 字段和 server emit 契约未改动。

### 遗留 / 后续

- 可在具备 live server + Fabric client 的环境中补按 N 键的 found/denied 体验手测；不属于本 PR 的协议或 server 变更。
- `ShieldBrokenHandler`、`ShieldBlockHitHandler`、`BreakthroughCinematicHandler` 等其他 handler 的线程审计仍不在本 plan 范围。
