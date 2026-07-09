# BugHunt: structure manifest 掉落段运行时断链

## 摘要

worldgen 会在 raster manifest 顶层写出 `corpse_mounds` 与 `ascension_pits`，其中携带南荒余烬干尸堆的搜刮掉落，以及北荒东陲渡劫坑的 `xujie_canxie` 掉落。但 Rust 运行时的 `RasterManifest` 只反序列化并消费 `fossil_bboxes`，没有接入这两段，导致生成数据被 serde 静默丢弃。

## 证据

- `worldgen/scripts/terrain_gen/bakers/raster_export.py:259-261` 同时收集 `fossil_bboxes`、`corpse_mounds`、`ascension_pits`。
- `worldgen/scripts/terrain_gen/bakers/raster_export.py:357-359` 将三者写入 manifest 顶层；`worldgen/scripts/terrain_gen/bakers/raster_export.py:537-539` 增量 regen 也刷新这三段。
- `worldgen/scripts/terrain_gen/structures/corpse_mound.py:23-54` 为 `ash_dead_zone` 固定导出 3 个 `dried_corpse_mound`，掉落池含 `mineral_fan_tie`、`rotten_bone_coin`、`dried_spirit_herb`。
- `worldgen/scripts/terrain_gen/structures/ascension_pit.py:21-58` 为 `tribulation_scorch` 且配置 `ascension_pit_xz` 的 zone 导出 `tianjie_ascension_pit`，掉落含 `xujie_canxie`。
- 真实蓝图验证：`server/zones.worldview.example.json` 可导出 3 个 `south_ash_dead_zone` 干尸堆和 1 个 `north_waste_east_scorch` 渡劫坑。
- `server/src/world/terrain/raster.rs:513-534` 的 `RasterManifest` 只有 `fossil_bboxes`，没有 `corpse_mounds` / `ascension_pits`。
- `server/src/world/terrain/raster.rs:821-835` 只把 `manifest.fossil_bboxes` 转成 runtime `FossilBbox`；`server/src/world/terrain/raster.rs:919-922` 只暴露 `fossil_bboxes()`。
- 仓库搜索未发现 `corpse_mounds` / `ascension_pits` 在 server runtime 侧的结构、资源或搜刮 consumer。

## 实际游玩体验影响

玩家进入南荒余烬时，地形/叙事暗示干尸堆可搜刮，但运行时不会生成可交互的尸丘 loot，玩家拿不到凡铁、退活骨币、干灵草。玩家探索北荒东陲焦土的化虚渡劫坑时，也不会通过该结构获得计划中的极稀有 `虚劫残屑` 掉落。结果是地图上有主题地点和 manifest 数据，但资源奖励闭环缺失，探索回报低于设计。

## 去重结论

不重复 #1053、#1062、#1067、#1080、#1091。

- #1053 是 carver owner/provenance 混用。
- #1062 是增量重烘覆盖新手 POI manifest。
- #1067 / #1080 是 raster_check 校验假绿。
- #1091 是暴龙王巢穴 POI runtime 无消费。

本问题是 `corpse_mounds` / `ascension_pits` 两个顶层 structure manifest 字段被 Rust manifest loader 丢弃，影响南荒余烬与北荒焦土结构 loot。它与 #1091 同属“数据存在但 runtime 不消费”的形态，但对象、字段、文件和玩家影响不同。

## 修复方向

- 在 `server/src/world/terrain/raster.rs` 为 `corpse_mounds` 与 `ascension_pits` 增加 manifest 结构、runtime 结构和只读 accessor。
- 为两类结构接入实际运行时 consumer：干尸堆按 `center_xz` / `search_seconds` / `loot_pool` 生成可搜刮资源；渡劫坑按 `center_xz` / `radius` / `loot` 生成 `xujie_canxie` 掉落机会或等价交互入口。
- 加 pin 测试：给 fixture manifest 同时写 `fossil_bboxes`、`corpse_mounds`、`ascension_pits`，断言三者均被 loader 保留且 consumer 可见。
- 加集成测试：真实蓝图导出的 `south_ash_dead_zone` 至少 3 个干尸堆可被运行时枚举；`north_waste_east_scorch` 的渡劫坑能进入掉落/交互路径。

## 对抗审查

- 第 1 轮对抗 subagent 独立排查 worldgen 分区，输出 `NO_CANDIDATE`。
- 第 2 轮对抗 subagent 针对此候选复核，结论为“支持成立，高置信，不只是文档遗留”，并确认不重复 #1091。
