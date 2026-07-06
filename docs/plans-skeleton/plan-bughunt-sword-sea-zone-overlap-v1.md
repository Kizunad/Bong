# BugHunt: 巨剑沧海与无垠深渊大面积 AABB 重叠导致 zone 归属错判

## Bug 摘要

`server/zones.json` 中 `giant_sword_sea` 与 `wuxing_abyss` 在主世界存在大面积 3D AABB 重叠。`ZoneRegistry::find_zone` 对多 zone 命中采用“最小 AABB 体积优先”，因此玩家站在巨剑沧海玩法区的重叠部分时，位置型 runtime 系统会把当前位置解析成 `wuxing_abyss`，而不是 `giant_sword_sea`。

worldgen 侧同时缺少 `giant_sword_sea` blueprint zone / generator 接线：`wuxing_abyss` 有 `abyssal_maze` 地形，`giant_sword_sea` 只有 profile 壳但未注册 generator。结果是巨剑沧海的 base raster / manifest semantic ownership 也不属于剑海。

## 实际游玩体验影响

玩家在巨剑沧海内看到巨剑、遇到剑海道具或物资棺时，HUD / zone_info / ambient / movement / cultivation 等按位置查 zone 的系统会把当前位置当作“无垠深渊”。典型表现：

- `zone_info.zone` 下发 `wuxing_abyss`，危险度和灵气读深渊值，而不是巨剑海。
- ambient 先解析成 `wuxing_abyss`，再因没有专属 recipe fallback 到 `ambient_wilderness`；即使以后补 `ambient_sword_sea`，位置解析仍会先落错 zone。
- 依赖 `find_zone(dimension, position)` 的 movement / weather / cultivation / NPC hydrate 路径会使用深渊 zone 语义。
- worldgen raster 中重叠区的 base semantic layers 由 `wuxing_abyss` 或 wilderness 承担，硬编码巨剑装饰只能后置放剑，不能修正 zone/raster semantic ownership。

## 证据定位

- `server/zones.json`:
  - `giant_sword_sea` AABB: x `3800..5400`, y `-64..320`, z `800..2400`，见 `server/zones.json:741` 附近。
  - `wuxing_abyss` AABB: x `4500..6000`, y `-64..120`, z `500..2300`，见 `server/zones.json:846` 附近。
  - 两者重叠: x `4500..5400`, y `-64..120`, z `800..2300`，体积约 `248,400,000`。
- `server/src/world/zone.rs:281` 附近：`find_zone` 在多 zone 命中时用 `aabb_volume()` 选最小者。`wuxing_abyss` 体积约 `496,800,000`，小于 `giant_sword_sea` 的约 `983,040,000`。
- `server/src/network/mod.rs:2009` 附近：`zone_name_for_position` 直接 `find_zone(Overworld, position)`。
- `server/src/network/mod.rs:2168` 附近：`emit_zone_info_on_zone_transition` 用该 zone 下发 `zone/spirit_qi/danger_level/status/active_events/perception_text`。
- `server/src/audio/ambient.rs:226` 附近：ambient 每个 client 用 `find_zone(dim, position)` 算 zone。
- `server/src/audio/ambient.rs:325` 附近：未知 zone recipe fallback 到 `ambient_wilderness`。
- `server/zones.worldview.example.json:975` 附近：blueprint 有 `wuxing_abyss`，profile 是 `abyssal_maze`。
- `worldgen/terrain-profiles.example.json:153` 附近：存在 `giant_sword_sea` profile 壳。
- `worldgen/scripts/terrain_gen/profiles/__init__.py:30` 附近：`_GENERATORS` 未注册 `giant_sword_sea` generator。
- `server/src/world/terrain/giant_sword.rs:82` 附近：巨剑硬编码装饰后置执行，但它不改变 zone ownership。

## 触发路径

1. 玩家进入主世界坐标 `(4600, 78, 1600)` 或 `(5000, 92, 2000)`。
2. 该点同时落在 `giant_sword_sea` 与 `wuxing_abyss` AABB 内。
3. `ZoneRegistry::find_zone(Overworld, pos)` 选择体积更小的 `wuxing_abyss`。
4. `zone_info` 下发深渊 zone 信息；ambient / movement / cultivation 等位置型系统也读深渊语义。
5. chunk 生成后 `giant_sword::decorate_chunk` 仍可能放置巨剑，但玩家当前 zone/raster semantic ownership 已经错位。

## 反方审查记录

### 第一轮质疑

反方未接受初版结论，指出：

- 重叠可能是历史设计，`find_zone` 的最小 AABB 语义允许合法嵌套。
- 黑武士自然刷新只取第一个 patrol anchor，后两个 anchor 不能作为核心生产路径证据。
- 巨剑硬编码装饰会后置放剑，不能声称整片剑海没有生成。
- `giant_sword_sea` worldgen generator 缺口在 `plan-terrain-wiring-v1` 中已有 deferred 记录，不能单独当新 bug。
- `ambient_sword_sea` 缺 recipe 是独立债，不能混入根因。

### 补证与让步

采纳质疑并收窄：

- 核心 bug 改为“位置型 runtime 系统在巨剑海玩法区错判为深渊”，不再把黑武士后两个 anchor 当主证据。
- 明确巨剑硬编码装饰仍会执行，问题是 zone ownership / raster semantic ownership 错。
- 补充 `zone_info`、ambient、movement/cultivation/NPC hydrate 等位置型 `find_zone` 路径。
- 补充生产 AABB overlap 体量，证明这不是边界贴边或小型内嵌渊口。
- 查开放 PR，未见 `giant_sword_sea` / `wuxing_abyss` / 大面积 overlap 同方向；#971 是矿脉锚点旧坐标，不重复。

### 最终裁决

反方最终通过：

> `giant_sword_sea` 与 `wuxing_abyss` 的大面积生产 AABB 重叠会让位置型 runtime 系统把巨剑海玩法区解析为无垠深渊，同时 worldgen blueprint 缺少剑海 zone 语义层；这不是单纯装饰缺口，也不是已证明可接受的嵌套设计。

## Skeleton Fix Plan

TODO:

- [ ] 在 `server/zones.json` 和 `server/zones.worldview.example.json` 对齐巨剑沧海与无垠深渊边界，确保巨剑海关键玩法点 `(4200,85,1200)`、`(4600,78,1600)`、`(5000,92,2000)` 均解析为 `giant_sword_sea`。
- [ ] 保留真正内嵌小 zone 的最小 AABB 优先语义，例如 rift mouth，不把全局 `find_zone` 策略改坏。
- [ ] 为生产 zones 增加 overlap guard 或白名单测试：大面积 overlap 必须显式列入白名单并解释语义，非白名单 overlap 失败。
- [ ] 补 worldgen blueprint drift guard：`server/zones.json` 中 gameplay zone 若存在 terrain profile 或硬编码 terrain 系统引用，必须在 blueprint/generator 接线中有明确来源，或在测试中显式标注为 hardcoded-only exception。
- [ ] 评估 `giant_sword_sea` worldgen generator 接线：若继续 hardcoded-only，manifest/raster 需要明确声明剑海语义 ownership；若接入 profile generator，补对应 `BlueprintZone`、generator 注册、tile/manifest 覆盖测试。
- [ ] 单独记录但不混入本 bug 根因：`ambient_sword_sea` recipe 缺失可作为后续音频债处理。

## 验收测试计划

- server 单测：
  - `find_zone(Overworld, DVec3::new(4600.0, 78.0, 1600.0))` 返回 `giant_sword_sea`。
  - `find_zone(Overworld, DVec3::new(5000.0, 92.0, 2000.0))` 返回 `giant_sword_sea`。
  - rift mouth 这类合法内嵌 zone 仍返回更小的 rift zone。
- network 单测：
  - 玩家从非剑海进入 `(4600,78,1600)` 时，`ZoneInfo.zone == "giant_sword_sea"`，`danger_level/spirit_qi` 取剑海 zone。
- audio 单测：
  - ambient zone change 在剑海关键点使用 `giant_sword_sea` 作为 zone key，不再经过 `wuxing_abyss`。
- worldgen 单测：
  - 默认 blueprint/generator 覆盖检查中 `giant_sword_sea` 不再是 profile-only 无 generator 的裸壳，或被显式列入 hardcoded-only exception 并附 guard。
  - raster active tile / manifest zone params 对剑海关键点有可解释的 semantic ownership，不被 `wuxing_abyss` 静默接管。
- 联调验收：
  - 进入巨剑沧海重叠区，HUD zone、ambient、物资棺刷新区域和巨剑装饰语义一致。

## 风险

- 直接改 `find_zone` 规则风险高，可能破坏 rift mouth / TSY 层等合法内嵌 zone。优先修正生产坐标与测试白名单。
- 如果移动 `wuxing_abyss` 或 `giant_sword_sea` AABB，需同步 patrol anchors、worldgen POI、物资棺边界、黑武士锚点、矿物/结构锚点，不得制造 #971 类旧坐标漂移。
- `giant_sword_sea` generator 可能是较大工作量，Skeleton PR 不应在本轮实现，只记录修复计划。
- `ambient_sword_sea` 缺 recipe 是相邻音频债，不是本 bug 的根因；修复 zone ownership 后若仍缺音频，应另立音频计划或并入后续 sword sea 接线计划。
