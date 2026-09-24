# plan-bughunt-zone-atmosphere-zoneid-profile-mismatch-v1

> Finished BugFix plan（2026-09-24 接续本地未推送修复并复验）。一句话：client `ZoneAtmosphereProfileRegistry` 用 **terrain profile id** 选大气（`spawn_plain` / `spring_marsh` / `dark_cavern`），但运行时 `zone_info` 下发的是 **live zone id**（`spawn` / `lingquan_marsh` / `youan_depths`），导致这些主路径区域稳定回退到 `wilderness` atmosphere。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 第一性原理证真：从 `zone_info` 到 `ZoneAtmosphereProfileRegistry.forZone` 的可达链与 RED 契约测试 | ✅ 2026-07-11 |
| P1 | client registry 的最小 live-zone alias 修复与饱和映射测试 | ✅ 2026-07-11 |
| P2 | Java 17 client 完整门禁、主线同步、无上下文 validator 与归档证据 | ✅ 2026-07-11 |

## 接入面与范围决策

- **进料**：server `zone_info.zone` 的 live zone id，经 `BongHudStateStore` / `ZoneState.zoneId()` 进入 `ZoneAtmosphereRenderer`。
- **出料**：`ZoneAtmosphereProfileRegistry.forZone` 返回既有 `ZoneAtmosphereProfile`，继续供 `ZoneAtmospherePlanner` 生成雾色、天空 tint、粒子与入场转场；不新增视觉资产。
- **共享契约**：保留现有 `zone_info` wire 和 `ZoneState`，只在 client registry 对明确的 live zone id 做归一化；不扩 schema、不让 server 下发第二套可漂移字段。
- **server 发送入口**：`server/src/network/mod.rs:emit_zone_info_on_zone_transition` 构造 `ServerDataPayloadV1::ZoneInfo`，调用同文件 `send_server_data_payload` 下发；`zone_name_for_position` 提供 live zone id。
- **agent 参与面**：不涉及运行时；`agent/packages/tiandao/src` 无 `zone_info` 消费，server 入口直接发送而不经 Redis/agent IPC；`agent/packages/schema/src/server-data.ts:ServerDataZoneInfoV1` 只是既有静态定义。
- **client 调用链**：`client/src/main/java/com/bong/client/network/ZoneInfoHandler.java:ZoneInfoHandler.handle` 产出 `ZoneState`，`ZoneAtmosphereRenderer.update` 交给 `ZoneAtmospherePlanner.plan`，再以 `zoneState.zoneId()` 调 `ZoneAtmosphereProfileRegistry.forZone`。
- **契约边界**：本修复不改 `server/src/schema/server_data.rs:ServerDataPayloadV1::ZoneInfo`、`agent/packages/schema/src/server-data.ts:ServerDataZoneInfoV1` 或 agent runtime；只改 client registry 的 live-zone alias。
- **worldview 锚点**：区域视觉辨识对应 `worldview.md §十三` 的初醒原、灵泉湿地、幽暗地穴地理差异。
- **决策**：live zone id 是运行时 source of truth；terrain profile id 是 client 资源键。最小修复由 client registry 持有显式 alias，不改 server/IPC，也不根据字符串形态猜测。

## 0. 结论

- 高置信、玩家可感知、位于 `worldgen/ui` 主路径的真 bug。
- 不是 world environment resync / ambient audio / npc trade gate 复读：问题点在 **client atmosphere profile 选键**，不是重连同步，也不是音频桥。
- 这个 bug 对实际游玩体验的影响：玩家在出生区、灵泉沼、幽暗深窟等区域看不到各自应有的雾色/天空色/粒子/入场转场，长期只吃 `wilderness` 的通用黄灰雾效，区域辨识度和“到新地貌”的体感显著变弱。

## 1. 现状证据

- server 实际 zone id 来自 `Zone.name`，`zone_info` 发包直接写 `zone: zone.name.clone()`：`server/src/network/mod.rs:2201-2207`。
- 静态 zone 配置里存在 `spawn`、`lingquan_marsh`、`youan_depths`：`server/zones.json:428,638,675`。
- client atmosphere registry 只加载/注册 `spawn_plain`、`spring_marsh`、`dark_cavern` 等 profile id：`client/src/main/java/com/bong/client/atmosphere/ZoneAtmosphereProfileRegistry.java:12-20,102-108`。
- `forZone()` 只做 exact match；非 TSY miss 后直接回退 `wilderness`：`ZoneAtmosphereProfileRegistry.java:61-70`。
- atmosphere 渲染每 tick 直接拿 `BongHudStateStore.snapshot().zoneState().zoneId()` 做 profile lookup：`client/src/main/java/com/bong/client/atmosphere/ZoneAtmosphereRenderer.java:44-50`，无 zone-id → terrain-profile 的别名层。
- `spring_marsh.json` 明确承诺灵泉湿地专属 `lingqi_ripple` / `enlightenment_dust` / `MIST_BURST`：`client/src/main/resources/assets/bong/atmosphere/spring_marsh.json:1-18`；而 miss 后会退到 `wilderness.json` 的通用尘雾：`.../wilderness.json:1-11`。
- 现有测试只 pin `spawn_plain` / `spring_marsh` 这类 profile id，从未用 live zone id `spawn` / `lingquan_marsh` 做 lookup：`client/src/test/java/com/bong/client/atmosphere/ZoneAtmosphereTest.java:33-171,352-355`。

## 2. 实施阶段

| 阶段 | 内容 | 状态 |
|---|---|---|
| P0 | 用 `ZoneAtmosphereTest` 增加 live zone id 契约测试，修复前证明 `spawn` / `lingquan_marsh` / `youan_depths` 错落 `wilderness`；同时确认直接 profile id、TSY fallback 和未知 zone fallback 的既有语义 | ✅ 2026-07-11 |
| P1 | 在 `ZoneAtmosphereProfileRegistry` 加显式、不可变的 live-zone → profile-id alias；补齐已存在但未注册的 `dan_zong_yi_yuan` profile；`forZone` 与 `hasProfile` 使用同一 direct-first 解析入口，覆盖三条正向 alias、已有 profile id、空值/空白、未知 id 与 TSY 路径 | ✅ 2026-07-11 |
| P2 | 用 JDK 17 运行 targeted test 和 `./gradlew test build`；同步最新 `origin/main` 后复验，记录 validator SHA 与完整 Finish Evidence，再归档 | ✅ 2026-07-11 |

## 3. 两轮反方裁决

- 第 1 轮反方：`server 也许发的是别名而不是 zone.name`。
- 裁决：证伪。`server/src/network/mod.rs:2201-2207` 明确把 `zone.name.clone()` 写进 `zone_info`；`server/zones.json` 的 live id 又明确是 `spawn` / `lingquan_marsh` / `youan_depths`。
- 第 2 轮反方：`即便 atmosphere miss，server environment / audio 也足够覆盖，不算实际 bug`。
- 裁决：证伪。`server/src/world/environment.rs:288-340` 对普通静态 zone 只在 scorch / tribulation / TSY / weather 时加环境效果；平时这些区的雾色、天空色、粒子、入场转场主要靠 client atmosphere。audio 侧虽有 `lingquan_marsh | spring_marsh` alias（`server/src/audio/ambient.rs:348-352`），但它只保住环境声，保不住视觉 atmosphere。

## 4. 验收口径

- 进入 `spawn` 时不再落 `wilderness` baseline，而是命中 `spawn_plain` 预期雾色/天空。
- 进入 `lingquan_marsh` 时必须出现 `spring_marsh` 的湿地视觉（至少 fog tint + ripple/dust 粒子）。
- 进入 `youan_depths` 时必须命中 `dark_cavern` 而不是通用荒野雾。
- 新增 pin test：live zone id lookup 不得静默回退到 `wilderness`。

## 5. 非目标

- 本骨架不处理 ambient audio recipe 命名统一；音频桥当前已有 `lingquan_marsh | spring_marsh` alias。
- 本骨架不扩展 dynamic pseudo vein / tribulation scorch 的新 atmosphere 设计；这里只修“现有静态主路径 zone 命名对不上 profile key”。

## 6. 已收口问题与后续边界

- alias owner 固定放在 client `ZoneAtmosphereProfileRegistry`：它连接 live zone id 与 client-only 资源键，避免为了三条稳定映射扩大 server/schema 改动。
- 本次 alias 只纳入已有证据和既有资源一一对应的 `spawn→spawn_plain`、`lingquan_marsh→spring_marsh`、`youan_depths→dark_cavern`；审计同时发现 live zone 与资源同名的 `dan_zong_yi_yuan.json` 未进入 `REQUIRED_PROFILE_IDS`，一并补齐注册。其它区域没有已存在的明确专属 profile 时继续走既有 fallback，不臆造映射。
- 后续若 dynamic zone 需要可配置 atmosphere profile，应另立 plan 评估显式 `atmosphere_profile_id` 契约；不在本 BugFix 中扩协议。

## Finish Evidence

- **落地清单**：
  - P0：`client/src/test/java/com/bong/client/atmosphere/ZoneAtmosphereTest.java` 新增 live zone id、direct-first、TSY、空值/空白与未知 zone 的契约测试；历史修复前 JDK 17 targeted run 为 25 tests / 2 failed，失败点即 live-zone alias；本次移植提交为 `d412733ce`。
  - P1：`client/src/main/java/com/bong/client/atmosphere/ZoneAtmosphereProfileRegistry.java` 新增 `LIVE_ZONE_PROFILE_IDS`，统一 `forZone` / `hasProfile` 的 direct-first 解析，并把既有 `dan_zong_yi_yuan.json` 纳入 required/fallback registry；本次移植提交为 `d91942001`、`a405a8c3c`。
  - P2：Java 17 client targeted 与完整门禁全绿；主线同步后重新跑过 client，并对主线带入的 agent 变更执行了 schema/tiandao 门禁。
- **关键 commit**（本次接续，2026-09-24；括号内为原本地来源）：
  - `9b05a775b`：升格 active plan（来源 `5391bc49a`）。
  - `d412733ce`：加入 live zone 映射契约（来源 `ce04a4ccc`）。
  - `d91942001`：实现三条显式 live-zone alias 与统一 lookup（来源 `7907bc283`）。
  - `a405a8c3c`：补齐丹宗 profile 注册并恢复 direct key 优先级（来源 `5c5325958`）。
  - `f57d0be0f`：完成 plan 归档（来源 `fb1d16f8c`）；Finish Evidence 来源 `e792355ce`。
- **测试结果**：
  - 修复前（历史来源）：`JAVA_HOME=$HOME/.cache/codex-jdks/jdk-17 ./gradlew test --tests com.bong.client.atmosphere.ZoneAtmosphereTest` → 25 tests，2 failed（预期 RED）。
  - 修复后 targeted：`scripts/build-token.sh gradle test --tests com.bong.client.atmosphere.ZoneAtmosphereTest` → 26/26 PASS。
  - 完整 client gate（主线同步前）：`JAVA_HOME=/home/serverkizuna/opt/jdk-17.0.19+10 PATH=/home/serverkizuna/opt/jdk-17.0.19+10/bin:$PATH scripts/build-token.sh gradle test build --rerun-tasks` → 584 suites / 5055 tests / 0 failures / 0 errors / 0 skipped，21 actionable tasks 全执行，BUILD SUCCESSFUL。
  - 主线同步：`git fetch origin` 后 `origin/main=6d7e1699b8cc3025c8ffe5c2b211c4c9fa7049fe`；`git merge origin/main` 无冲突，产生 merge commit `d454aaf1b2725f45627340e196a8f0c0a88dd6db`。合入内容触及 agent runtime/test，未触及本 plan 的 client 文件。
  - 主线同步后 client 复验：同一 Java 17 `scripts/build-token.sh gradle test build --rerun-tasks` → 584 suites / 5055 tests / 0 failures / 0 errors / 0 skipped，21 actionable tasks 全执行，BUILD SUCCESSFUL。
  - 主线同步后 agent 复验：`npm ci`；`npm run build -w @bong/schema`；`npm test -w @bong/schema` → 33 files / 913 tests passed；`npm test`（`agent/packages/tiandao`）→ 72 files / 873 tests passed。
- **validator 证据**：
  - 首轮 `FAIL 7907bc283007eb82dd8a83fa373301534385f734`：发现 `dan_zong_yi_yuan` 漏注册与 alias 遮蔽 direct key；已返工。
  - 修复关口 `PASS 5c53259580211c28e39fd707db928301440bdea4`：9 个 atmosphere JSON 全注册，direct/alias/fallback 语义与 JDK 17 26/26 测试成立。
  - 主线同步关口 `PASS d454aaf1b2725`：最新 `origin/main` 合入无冲突；client、schema、tiandao 复验均通过。
- **跨栈核验**：
  - server：继续由 `send_player_state_payload_to_client` / `zone_name_for_position` 下发 live `Zone.name`，本修复不改 wire。
  - client：`ZoneAtmospherePlanner` 仍从 `ZoneState.zoneId()` 调 `ZoneAtmosphereProfileRegistry.forZone`，现能命中已有视觉 profile；未新增或修改视觉资产。
  - agent：主线同步带入 `agent/packages/tiandao/src/runtime.ts` 与对应测试更新，已用 slot-2 自己的依赖完成 schema/tiandao 门禁；本 plan 的 atmosphere 契约仍只在 client registry 内闭环。
  - worldgen：无代码或契约改动；只复用既有 terrain/atmosphere 资源对应关系。
- **遗留 / 后续**：dynamic zone 若需自定义 atmosphere profile，应另立 plan 设计显式字段；本修复不扩大协议。实机逐 zone 截图由 PR e2e/人工视觉验收继续承担，不影响本次确定性 lookup 契约闭环。
