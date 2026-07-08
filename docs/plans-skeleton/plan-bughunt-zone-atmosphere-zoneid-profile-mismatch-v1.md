# plan-bughunt-zone-atmosphere-zoneid-profile-mismatch-v1

> 骨架（2026-07-05）。一句话：client `ZoneAtmosphereProfileRegistry` 用 **terrain profile id** 选大气（`spawn_plain` / `spring_marsh` / `dark_cavern`），但运行时 `zone_info` 下发的是 **live zone id**（`spawn` / `lingquan_marsh` / `youan_depths`），导致这些主路径区域稳定回退到 `wilderness` atmosphere。

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

## 2. 修法草案

| 阶段 | 内容 | 状态 |
|---|---|---|
| P0 | 明确 atmosphere 的 source-of-truth：是按 live zone id 配，还是先把 zone id 归一到 terrain profile id | ⬜ |
| P1 | 若保留 live zone id：给 registry 加别名表（至少 `spawn→spawn_plain`、`lingquan_marsh→spring_marsh`、`youan_depths→dark_cavern`）并补 pin test | ⬜ |
| P2 | 若改成统一 terrain-profile key：server/IPC 增补显式字段，避免 client 继续猜名字 | ⬜ |
| P3 | 回归所有已知 zone/profile 映射，防 future zone 命名再静默吃 `wilderness` fallback | ⬜ |

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

## 6. 开放问题

- 别名表的 owner 放 client registry 还是抽成双端共享常量，哪边更不容易再漂移。
- `spawn_plain` / `spring_marsh` / `dark_cavern` 之外，是否还存在更多“terrain profile 名 != live zone id”的漏网项，需要一次性 audit。
- 若后续要给 dynamic zone 复用同一套 atmosphere，是否应直接下发 `atmosphere_profile_id`，避免继续靠字符串猜测。
