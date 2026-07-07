# BugHunt: worldgen 结构 loot manifest 运行时断链

## 摘要

`worldgen` 已把 `corpse_mounds` 与 `ascension_pits` 作为正式 `manifest.json` 顶层字段导出，字段内包含坐标、结构 kind、搜刮时间、loot pool / drop chance。但 server 端 `RasterManifest` 不反序列化这两个字段，`TerrainProvider` 也没有保存或暴露它们；运行时没有等价 consumer 把干尸堆或化虚渡劫坑落成可交互实体、容器、掉落或搜刮点。

结果是 worldgen 侧已经生成了“可搜刮结构奖励”的语义数据，server 启动加载 raster 时却静默丢弃。

## 实际游玩体验影响

玩家到达南荒余烬的干尸堆位置时，按设计应能搜刮到凡铁、退活骨币、干灵草等荒野残留物；到达北荒东陲焦土的化虚渡劫遗迹时，按设计应有极低概率获得“虚劫残屑”。当前 server 没有消费这些 manifest 字段，玩家实际只能看到地形/装饰语义，拿不到对应奖励，也不会出现明确的交互点。

这会让高风险探索点变成“看起来有故事、实际没有玩法回报”的空壳；尤其 `xujie_canxie` 已注册为稀有物品但入口 loot 不落地，会误导玩家反复搜索无结果。

## 避重说明

已避开 #969-#1105，尤其近期 worldgen：

- #1053：carver owner / provenance 混用。
- #1062：新手 POI 增量重烘局部 fields 污染。
- #1067：`raster_check` CLI 假绿。
- #1080：`raster_check` 必需层漏检假绿。
- #1091：暴龙王 POI 消费断链。
- #1097：structure manifest 掉落段运行时断链。
- #1103：焦土地表矿露头运行时断链。

本 bug 不是单个 structure manifest 掉落段，也不是暴龙王 POI 或地表矿露头；它是 `manifest.json` 顶层 `corpse_mounds` / `ascension_pits` 两段结构 loot 元数据整体没有 server schema/consumer。

归档 `plan-terrain-wiring-v1` 曾把“`corpse_mounds` / `ascension_pits` 顶层段 server 消费”记为遗留断链并转交 scorch plan；但 `plan-terrain-ash-deadzone-v1` 与 `plan-terrain-tribulation-scorch-v1` 的 evidence 只证明 worldgen 导出、item 注册和焦土记录，没有证明 server 运行时消费这两个顶层段。

## 证据定位

- `worldgen/scripts/terrain_gen/bakers/raster_export.py:259-261`：导出期收集 `fossil_bboxes`、`corpse_mounds`、`ascension_pits`。
- `worldgen/scripts/terrain_gen/bakers/raster_export.py:357-359`：三者写入 `manifest.json` 顶层字段。
- `worldgen/scripts/terrain_gen/bakers/raster_export.py:537-539`：增量 regen 也刷新 `corpse_mounds` / `ascension_pits`，说明不是一次性文档字段。
- `worldgen/scripts/terrain_gen/structures/corpse_mound.py:23-31`：干尸堆 loot pool 包含 `mineral_fan_tie`、`rotten_bone_coin`、`dried_spirit_herb`。
- `worldgen/scripts/terrain_gen/structures/corpse_mound.py:44-52`：干尸堆 manifest 记录 `kind=dried_corpse_mound`、`center_xz`、`loot_pool`、`search_seconds`。
- `worldgen/scripts/terrain_gen/structures/ascension_pit.py:21-35`：化虚坑 manifest 记录 `kind=tianjie_ascension_pit`、中心、半径、`xujie_canxie` 掉落概率。
- `server/src/world/terrain/raster.rs:520-534`：Rust `RasterManifest` 只声明 `pois`、`anomaly_kinds`、`abyssal_tier_floor_y`、`global_decoration_palette`、`fossil_bboxes`，没有 `corpse_mounds` / `ascension_pits`。
- `server/src/world/terrain/raster.rs:764-865`：`TerrainProvider::load` 只映射 POI、anomaly、abyssal、decoration、fossil、placement sidecar；未保存 mound/pit。
- 只读搜索 `grep -RIn "corpse_mounds\\|ascension_pits" server/src` 无命中，说明 server 端没有直接 consumer。

## 触发路径

1. 运行 worldgen raster export，`manifest.json` 包含 `corpse_mounds` / `ascension_pits`。
2. server 通过 `BONG_TERRAIN_RASTER_PATH` 加载该 manifest。
3. Serde 对未知顶层字段默认忽略；`RasterManifest` 没有这两个字段，`TerrainProvider` 状态也无保存位置。
4. 后续 chunk 生成、POI startup consumer、容器/掉落系统都无从读取这些结构 loot 元数据。
5. 玩家到达对应结构位置，无法触发搜刮/掉落。

## 对抗复核结论

已 spawn adversarial subagent 做两轮复核，裁决为 REAL。

Round 1 反方尝试证明 server 已有消费路径。复核结果：现有 consumer 只覆盖 `pois()`、`fossil_bboxes()`、placement sidecar 等路径，没有 `corpse_mounds` / `ascension_pits` 等价路径。

Round 2 反方尝试证明这些字段只是预览/文档元数据或已被旧 plan 覆盖。复核结果：字段带 loot/search/drop 语义，且 full export 与 regen 都维护它；既有 skeleton 未覆盖该主题，归档 plan 只证明 worldgen 导出和物品存在，没有 runtime consumer evidence。

## Skeleton Fix Plan

1. **P0 manifest schema 接线**：在 server `RasterManifest` 中增加 `corpse_mounds` / `ascension_pits` serde struct，`TerrainProvider` 保存只读 metadata，并暴露查询接口。
2. **P1 runtime consumer**：启动期或 chunk/zone 初始化阶段消费 metadata，生成可交互 marker / loot container / 搜刮点；坐标必须带 dimension 与 zone 校验，不能只按裸 XZ。
3. **P2 loot 接入**：干尸堆按 `loot_pool` 与 `search_seconds` 生成搜刮结果；化虚坑按 `loot.drop_chance` 接入 `xujie_canxie` 掉落或可重复/一次性搜刮策略。
4. **P3 防静默丢弃**：新增 manifest pin 测试，含 `corpse_mounds` / `ascension_pits` 的 fixture 加载后必须在 `TerrainProvider` 可见；运行时 consumer 至少生成对应 marker 数量。
5. **P4 e2e 验收**：带真实 raster 启服后，玩家到南荒余烬干尸堆与北荒东陲化虚坑附近，能看到/触发对应交互反馈与 loot 结果。

## 验收测试计划

- `server/`：`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test world::terrain`
- `server/`：新增 consumer 后运行对应 `cargo test`，覆盖 manifest 解析、marker 生成、loot 结算、未知/空字段兼容。
- `worldgen/`：`python3 -m unittest tests.test_tribulation_scorch scripts.terrain_gen.test_ash_dead_zone -v`
- 仓库根：`BONG_SKIP_SKIN_PREFETCH=1 bash scripts/smoke-test-e2e.sh`

## 风险

- `corpse_mounds` / `ascension_pits` 是顶层 manifest metadata，不要误接成普通 `pois` 后破坏现有 POI consumer 语义。
- 化虚坑 loot 极低概率需要可测试的 deterministic override，否则验收会假绿或不稳定。
- 干尸堆搜刮若生成容器，需要明确一次性/刷新/持久化策略，避免重启刷 loot 或多人重复领取。

## 审计来源

BugHunt worldgen r11（2026-07-07）。本轮只新增 skeleton plan，不修改代码、配置、资源或依赖；已先执行 `gh pr list --state all --limit 600 --json number,title,headRefName,url` 并完成对抗复核。
