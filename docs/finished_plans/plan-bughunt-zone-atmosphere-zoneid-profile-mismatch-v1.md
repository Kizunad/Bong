# plan-bughunt-zone-atmosphere-zoneid-profile-mismatch-v1

> Active BugFix plan（2026-07-11 从 `docs/plans-skeleton/plan-bughunt-zone-atmosphere-zoneid-profile-mismatch-v1.md` 升格）。一句话：client `ZoneAtmosphereProfileRegistry` 用 **terrain profile id** 选大气（`spawn_plain` / `spring_marsh` / `dark_cavern`），但运行时 `zone_info` 下发的是 **live zone id**（`spawn` / `lingquan_marsh` / `youan_depths`），导致这些主路径区域稳定回退到 `wilderness` atmosphere。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 第一性原理证真：从 `zone_info` 到 `ZoneAtmosphereProfileRegistry.forZone` 的可达链与 RED 契约测试 | ⏳ |
| P1 | client registry 的最小 live-zone alias 修复与饱和映射测试 | ⬜ |
| P2 | Java 17 client 完整门禁、主线同步、无上下文 validator 与归档证据 | ⬜ |

## 接入面与范围决策

- **进料**：server `zone_info.zone` 的 live zone id，经 `BongHudStateStore` / `ZoneState.zoneId()` 进入 `ZoneAtmosphereRenderer`。
- **出料**：`ZoneAtmosphereProfileRegistry.forZone` 返回既有 `ZoneAtmosphereProfile`，继续供 `ZoneAtmospherePlanner` 生成雾色、天空 tint、粒子与入场转场；不新增视觉资产。
- **共享契约**：保留现有 `zone_info` wire 和 `ZoneState`，只在 client registry 对明确的 live zone id 做归一化；不扩 schema、不让 server 下发第二套可漂移字段。
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
| P0 | 用 `ZoneAtmosphereTest` 增加 live zone id 契约测试，修复前证明 `spawn` / `lingquan_marsh` / `youan_depths` 错落 `wilderness`；同时确认直接 profile id、TSY fallback 和未知 zone fallback 的既有语义 | ⏳ |
| P1 | 在 `ZoneAtmosphereProfileRegistry` 加显式、不可变的 live-zone → profile-id alias；补齐已存在但未注册的 `dan_zong_yi_yuan` profile；`forZone` 与 `hasProfile` 使用同一 direct-first 解析入口，覆盖三条正向 alias、已有 profile id、空值/空白、未知 id 与 TSY 路径 | ⬜ |
| P2 | 用 JDK 17 运行 targeted test 和 `./gradlew test build`；同步最新 `origin/main` 后复验，记录 validator SHA 与完整 Finish Evidence，再归档 | ⬜ |

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
