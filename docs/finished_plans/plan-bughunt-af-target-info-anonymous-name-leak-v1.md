# plan-bughunt-af-target-info-anonymous-name-leak-v1

> **Finished plan**。一句话主题：`inspect / social / hud / state` 主路径发现 1 个高置信真 bug：**匿名玩家头顶名牌虽已隐藏，但 `TargetInfo` 顶部 HUD 仍会在一次右键/攻击后泄漏其真实名字 5 秒**。这条链路直接绕过 `plan-social-v1` 的匿名设计，对 PvP/尾随/试探交互都有明确玩法后果。

> 立项动机：本轮限定扫描 `inspect/preview/social-hud/state` 主路径，并排除已立项题（trade bundle 少发货 / sparring invite hijack / trade gate / season stale client / tide_sky 漏接等）。本题落点集中、证据链短、玩家体感强，适合 skeleton-only 立项。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 匿名玩家被 `TargetInfo` HUD 反查真名 | fix_pr | ✅ 2026-07-18 |

## P0 — 匿名玩家被 `TargetInfo` HUD 反查真名（✅ 2026-07-18）

- **#1 major（fix_pr）**：匿名系统只挡住了**头顶名牌**，没有挡住**顶部 TargetInfo HUD**。
  - `client/src/main/java/com/bong/client/mixin/MixinEntityRenderer.java:17-35` 明确把远端玩家名牌显示门控到 `SocialStateStore.shouldShowRemoteNameTag(player.getUuidAsString(), playerName)`；`docs/finished_plans/plan-social-v1.md:292-296` 也把“client 端 name tag 默认隐藏 / server 下发 AnonymityPayload 决定显示”写成正式交付。
  - 但 `client/src/main/java/com/bong/client/mixin/MixinClientPlayerInteractionManagerAlchemy.java:41-68` 在**左键攻击**与**主手右键交互**后都会无条件 `TargetInfoStateStore.observeEntity(...)`。
  - `client/src/main/java/com/bong/client/hud/TargetInfoState.java:69-90` 处理 `PlayerEntity` 时，直接取 `living.getDisplayName().getString()` 作为 `displayName`，**完全不查询 `SocialStateStore`，也没有匿名兜底文案**；realm 还被硬编码成空串。
  - `client/src/main/java/com/bong/client/hud/TargetInfoHudPlanner.java:55-72` 会把这个 `displayName` 原样渲染到屏幕顶部；对玩家目标只是不画 HP/真元条，**并不会隐藏名字本身**。
  - 结果：匿名玩家虽然头顶没名牌，但只要被人点一下/打一下，顶部 HUD 就会显示其真实用户名并保持 `TargetInfoState.HOLD_MILLIS = 5000` 毫秒。匿名机制被一次交互直接绕过。

## 这个 bug 对实际游玩体验的影响

- 匿名玩家在遭遇战、尾随试探、切磋前试探时，本应只暴露“有人在这里”，现在却会被一次轻触直接暴露真实身份。
- 这让 `plan-social-v1` 的“默认匿名、暴露后才显名”规则失去实战意义；玩家不需要等 `social_exposure`、不用 inspect 面板，也不用任何高阶感知，就能从 HUD 读出真名。
- `docs/finished_plans/plan-hud-immersion-v2.md:5` 已明确写了“匿名系统（默认不显示名字）→ HUD 不应暴露他人太多信息”；当前实现和该约束正面冲突。

## 建议修法

- `TargetInfoState.fromEntity(PlayerEntity)` 改为复用 `SocialStateStore.shouldShowRemoteNameTag(player.getUuidAsString(), playerName)` 这条既有匿名判定，而不是直读 `living.getDisplayName()`。
- 未暴露时，`TargetInfo` 应降级成匿名文案（如“某修士”或其他已定匿名占位），并继续保持玩家目标不显示 HP/真元；不要在这条快速 HUD 上额外泄漏 realm。
- 已暴露/已知名的玩家仍可沿用现有显示逻辑，避免伤到正常熟人/盟友可见性。

## 测试抓手

- 补 `TargetInfoState` / `TargetInfoHudPlanner` 单测：
  - 匿名 remote：`SocialStateStore.replaceAnonymity(... anonymous=true ...)` 后，玩家目标 HUD **不得**出现真实名字。
  - 已暴露 remote：`anonymous=false` 时，HUD 正常显示名字。
  - pin `TargetInfoStateStore.observeEntity(PlayerEntity, now)` 走玩家分支，防未来只测 `TargetInfoState.create(...)` 这类绕开真实入口的假阳性。
- 现有 `client/src/test/java/com/bong/client/network/SocialServerDataHandlerTest.java:24-48` 只验证了**头顶名牌**门控；`TargetInfo` 路径目前零覆盖。

## 两轮反方裁决摘要

1. **反方第 1 轮**：也许匿名玩家的 `PlayerEntity.displayName` 已被 server 改写成化名，所以 `TargetInfo` 读到的不一定是真名。  
   **裁决**：证伪。全仓未发现任何把远端 `PlayerEntity` 名称改写为匿名占位的链路；现有匿名实现只有 `MixinEntityRenderer` 取消 `renderLabelIfPresent`，不是改名。
2. **反方第 2 轮**：也许 `plan-social-v1` 只要求隐藏头顶名牌，不要求隐藏顶部 HUD，所以这属于设计允许。  
   **裁决**：证伪。`plan-social-v1` 已把匿名作为正式玩法约束，且 `plan-hud-immersion-v2.md:5` 明写“HUD 不应暴露他人太多信息”；一次攻击/右键就泄漏真名，与“默认匿名、暴露后才显名”的设计目标冲突。

## 审计来源

bug-hunt 线程 AF（限定 worktree：`bughunt-loop-20260705-af`，范围：`inspect / preview / social-hud / state` 主路径）。候选链路先后排除了已修 bridge 枚举问题与已立项主题后，最终锁定 `TargetInfoStateStore -> TargetInfoState -> TargetInfoHudPlanner` 对匿名系统的旁路泄漏。结论：**real-on-main，player-facing，局部明确，可 fix_pr。**

## Finish Evidence

### 落地清单

- **P0 — 匿名玩家 TargetInfo 脱敏**：
  - `client/src/main/java/com/bong/client/hud/TargetInfoState.java`：`fromEntity(Entity, long)` 的 `PlayerEntity` 分支统一进入 `fromPlayerTargetInfo(...)`；`playerDisplayNameForTargetInfo(...)` 复用 `SocialStateStore.shouldShowRemoteNameTag(...)`，匿名或未知远端显示 `ANONYMOUS_PLAYER_DISPLAY_NAME`（“某修士”），已暴露远端保留允许显示的名称，玩家目标的 realm / HP / 真元仍不额外泄漏。
  - `client/src/main/java/com/bong/client/hud/TargetInfoStateStore.java` 与 `client/src/main/java/com/bong/client/mixin/MixinClientPlayerInteractionManagerAlchemy.java`：攻击及主手交互继续走 `observeEntity(...) -> TargetInfoState.fromEntity(...)` 的真实生产入口。
  - `client/src/main/java/com/bong/client/hud/TargetInfoHudPlanner.java`：顶部 HUD 只渲染已经过上述门控的 `TargetInfoState.displayName()`，且 `Kind.PLAYER` 不渲染 HP / 真元条。
  - `client/src/test/java/com/bong/client/hud/TargetInfoHudPlannerTest.java`：保留 9 条 JUnit 契约测试，覆盖 anonymous / exposed / unknown remote、装饰显示名、遗骸伪玩家例外，以及 `fromPlayerTargetInfo(...)` 玩家快照分支。
  - `client/build.gradle`、`client/src/gametest/resources/fabric.mod.json` 与 `client/src/gametest/java/com/bong/client/hud/TargetInfoPlayerGameTest.java`：新增独立 `gametest` source set / mod / `runGametest`，在 Fabric Loader/Knot transformed server runtime 中用真实 `ServerPlayerEntity` 锁定 `TargetInfoStateStore.observeEntity(...) -> snapshot() -> TargetInfoHudPlanner.buildCommands(...)`；anonymous、exposed、unknown 三案分别断言占位名、scoreboard 装饰名与 fail-closed 不泄漏。`test` 门禁显式依赖 `runGametest`，且 `runGametest` 每次启动前在专用、gitignored 的 `build/gametest/` 运行目录写入 `eula=true`，保证全新无 stdin 的 CI runner 不依赖本地残留文件也能启动 dedicated GameTest server。

### 关键 commit

- `bb43988db477851157ea107dc3dfda378b59ce40` — 2026-07-06 — PR #892 首次让 `TargetInfoState` 复用 `SocialStateStore` 匿名判定，并加入 anonymous / exposed HUD 回归。
- `b65fc1ca9d2419e582807582996720cb04480b9b` — 2026-07-08 — PR #1131 合并：将玩家真实入口收束到 `fromPlayerTargetInfo(...)`，修正遗骸伪玩家误匿名，并补足玩家分支与未知/装饰名回归测试。
- `b7a76c9a` — 2026-07-19 — PR #1231 补入 Fabric GameTest harness 与 3 条真实 `ServerPlayerEntity` 生产入口回归，并把 `runGametest` 接入既有 `test` 门禁。
- `cde7417231346ddbae1a49bed396a3d2767640e0` — 2026-07-19 — 修复 `runGametest` 在全新 CI runner 缺少 `eula.txt` 时被 owo 无 stdin 提示中止的问题；任务启动前显式生成专用运行目录的 `eula=true`。

### 测试结果

- 2026-07-19，Java 17：先将本地 `client/build/gametest/eula.txt` 置于 `eula=false` 的冷启动等价状态，再执行 `cd client && JAVA_HOME=/home/serverkizuna/java/jdk-17.0.19+10 PATH=/home/serverkizuna/java/jdk-17.0.19+10/bin:$PATH ./gradlew runGametest --console=plain` — **PASS**；`runGametest` 自行改写为 `eula=true`，Fabric Loader/Knot transformed server runtime 执行 **3 tests / 0 failures**，`client/build/gametest-results.xml` 分别记录 anonymous / exposed / unknown 三案。
- 2026-07-19，合并 `origin/main@a07839ab02a531ca9267d337eb839f707b12f848` 后以 Java 17 执行：`cd client && JAVA_HOME=/home/serverkizuna/java/jdk-17.0.19+10 PATH=/home/serverkizuna/java/jdk-17.0.19+10/bin:$PATH ./gradlew test build --console=plain` — **PASS**；`test` 门禁执行上述 **3/3 GameTest**，JUnit 报告保持 **4128 tests / 0 skipped / 0 failures / 0 errors**（其中 `TargetInfoHudPlannerTest` 为 **9/9**），随后 `build` 成功。
- PR #1231 的 GitHub `e2e` runs `29670095186` / `29671623308` 均在全新 runner 的 `Client stage (gradlew test)` 可重复失败：`runGametest` 找不到 `server.properties` / `eula.txt` 后进入 owo 的交互式 `EULA:` 提示，无 stdin 导致 `java.util.NoSuchElementException`，进程以 255 退出；`cde7417231346ddbae1a49bed396a3d2767640e0` 已针对该 PR 引入的冷启动缺口补齐确定性 EULA 准备，待新 HEAD e2e 复验。
- PR #1131：GitHub `e2e` check **PASS**（2026-07-07，20m55s）。该 PR 的 `review` 失败来自 4 个 Codex reviewer 执行失败，CodeRabbit 失败来自 review limit / prepaid credits exhausted；两者均未产生代码级 finding。
- 生产链动态核验：真实 `ServerPlayerEntity` -> `TargetInfoStateStore.observeEntity(...)` -> `TargetInfoStateStore.snapshot()` -> `TargetInfoHudPlanner.buildCommands(...)`；anonymous / unknown 均显示“某修士”且不含 profile 或 scoreboard 装饰名，exposed 保留 scoreboard 装饰名。

### 跨仓库核验

- **server**：`server/src/social/mod.rs::emit_anonymity_payloads_for_joined_clients` / `serialize_social_anonymity_payload_for_viewer` 生成权威 `ServerDataPayloadV1::SocialAnonymity` 快照。
- **agent / schema**：`agent/packages/schema/src/social.ts::SocialAnonymityPayloadV1`、`agent/packages/schema/src/server-data.ts::ServerDataSocialAnonymityV1` 与 `agent/packages/schema/samples/server-data.social-anonymity.sample.json` 固定现有 wire 契约；本修复无需新增天道运行时协议。
- **client**：`ProtoServerDataBridge` / `ServerDataRouter` / `SocialServerDataHandler` 将 `social_anonymity` 写入 `SocialStateStore`；`MixinEntityRenderer` 与 `TargetInfoState` 共同复用 `shouldShowRemoteNameTag(...)`，头顶名牌和顶部 TargetInfo HUD 不再出现匿名判定分叉。

### 遗留 / 后续

- 本 plan 范围内无遗留 blocker。普通 Gradle JUnit 缺少 Loader/Knot 对 Minecraft named jar 的 transformed package-access 处理，不能合规构造可运行的 literal `PlayerEntity` fixture；PR #1231 因此新增 Fabric GameTest，并已在真实 `ServerPlayerEntity` 上动态锁定完整生产入口，不再以 helper 单测或静态调用链代替入口回归。
- 遗骸伪玩家例外仅在 zero UUID + `Remains_` profile + “遗骸” fallback 三条件同时满足时生效；其他未知玩家继续 fail-closed 为“某修士”。
