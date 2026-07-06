# plan-bughunt-novice-poi-regen-local-fields-v1

> Skeleton Plan. 一句话主题：worldgen console / incremental regen 在重烘非 spawn zone 时，用局部 `fields` 重算并覆盖全局新手 POI manifest，导致下一次用该 raster manifest 启服时，新手炼器台、丹炉、残卷等 POI 坐标漂移到 fallback 或错误局部语义。

## 1. 实际游玩体验影响

限定语：本 bug 不主张 `/api/regen` 会热更新正在运行的生产服；影响路径是 dev console / incremental regen 对同一 `rasters/manifest.json` 原地写入后，下一次以这份 manifest 启服、联调或生成快照时，server 会加载被局部 fields 重算污染的 novice POI 坐标。

玩家可见后果：

- 初醒原新手环的炼器台、凡铁丹炉、残卷藏匿点、灵草谷等 `poi_novice` 坐标不再来自 spawn 周边真实 qi/height 采样，而可能回退到硬编码 fallback。
- `PoiNoviceRegistry` 启服时用污染后的 manifest 替换全量 registry，后续“第一次炼器 / 第一次炼丹 / 第一次拾取知识 / 第一次采集”等引导、容器/站点刷新、生命记录触发都指向错误位置。
- 玩家可能在实际地形上找不到预期新手 POI，或者看见 POI 与当前 raster 地形/灵气条件不匹配，破坏出生后资源与引导路径。

## 2. 复现路径

1. 先跑一次完整 worldgen raster 导出，得到正常 `generated/terrain-gen/rasters/manifest.json`。
2. 打开 worldgen console，或等价调用 console regen 路径，对远离 spawn 的非 spawn zone 执行增量重烘，例如 `qingyun_peaks`、`blood_valley`、`lingquan_marsh`。
3. 该路径按 `synthesize_fields(plan, zone_filter={zone_name})` 只合成触及该 zone 的 tiles。
4. `regen_zone()` patch tile entries 后，无条件用这份局部 `fields` 重新构造 `manifest["pois"]`。
5. 下次以该 manifest 设置 `BONG_TERRAIN_RASTER_PATH` 启服，进入游戏并检查新手 POI registry / 对应站点坐标。

只读验证片段：`zone_filter=spawn` 时 `novice_forge_station=[224,71,-240]`、`novice_alchemy_furnace=[0,72,-200]`、`novice_scroll_hidden=[176,72,-96]`，均为 relaxed 实采样坐标，不是 fallback。非 spawn 远区因 AABB 距 spawn 数千格，局部 fields 不覆盖 spawn 半径，几何上会走候选为空路径。

## 3. 根因证据

- 初次完整导出在 `worldgen/scripts/terrain_gen/bakers/raster_export.py:253-254` 使用完整 `fields` 执行 `build_novice_poi_manifest_payload(fields)`，这是正确路径。
- `regen_zone()` 的契约写明 `fields` 必须来自 `synthesize_fields(plan, zone_filter={zone_name})` 或更窄局部过滤，见 `worldgen/scripts/terrain_gen/bakers/raster_export.py:455-464`。
- `synthesize_fields()` 的 `zone_filter` 只选择与指定 zone 相交的 tiles，见 `worldgen/scripts/terrain_gen/stitcher.py:691-741`。
- `regen_zone()` patch 完 tiles 后，在 `worldgen/scripts/terrain_gen/bakers/raster_export.py:528-531` 无条件重建 `pois_payload` 并覆盖 `manifest["pois"]`。
- `build_novice_poi_manifest_payload()` 默认以 spawn center `(0,70,0)` 和半径 1500 选点，见 `worldgen/scripts/poi_novice_selector.py:241-256`。
- `_field_set_to_selector_inputs()` 只从传入 `fields.tiles` 建采样 bounds，见 `worldgen/scripts/poi_novice_selector.py:387-430`。
- `_select_one()` 候选为空时落入 `FALLBACK_LOCATIONS`，见 `worldgen/scripts/poi_novice_selector.py:98-105` 与 `:301-330`。
- runtime 闭环成立：`TerrainProvider::load()` 读取 manifest，见 `server/src/world/terrain/raster.rs:712-730`；`PoiNoviceLoader::load()` 在 Startup 从 `providers.overworld.pois()` 导入 `poi_novice` 并 `registry.replace_all(sites)`，见 `server/src/world/poi_novice.rs:237-264` 与 `:277-283`。

## 4. 修复计划骨架

- [ ] P0：拆分 manifest POI 刷新语义。`regen_zone()` 只允许刷新被重烘 zone 的 blueprint 静态 POI / profile 派生 POI；全局 spawn novice POI 必须保留旧 manifest 值，除非本次 regen 覆盖 spawn 周边完整选择范围。
- [ ] P1：为 novice POI 选择器补显式输入契约：拒绝用不覆盖 spawn selection radius 的局部 `fields` 生成全局 novice POI，或要求调用方传完整 full-world / spawn-window fields。
- [ ] P2：修复 console `/api/regen`：对非 spawn zone regen 时 patch `tiles[]` 和 zone-derived metadata，但不 clobber `poi_novice` entries；对 spawn zone regen 时重新计算 novice POI，并保证采样窗口完整。
- [ ] P3：补 regression pin：非 spawn `regen_zone(qingyun_peaks)` 后，`manifest["pois"]` 中所有 `poi_novice` 坐标与完整导出一致；spawn regen 仍允许按完整 spawn window 重算。
- [ ] P4：文档化边界：blueprint 静态 POI 的刷新是合理需求；本 bug 只针对全局 spawn 周边派生的 novice POI。

## 5. 验证计划

- [ ] worldgen 单测：构造完整 manifest + 非 spawn 局部 fields，调用 `regen_zone()` 后断言 `poi_novice` entries 未变。
- [ ] worldgen 单测：构造 spawn 局部 fields 且覆盖选择半径，允许 novice POI 重算，并断言不落入意外 fallback。
- [ ] console API 测试：`POST /api/regen {zone_name:"qingyun_peaks"}` 后，返回 rewritten tiles，但 manifest 中 `poi_novice` 坐标保持完整导出结果。
- [ ] server pin：用污染前后 manifest 启动 `PoiNoviceLoader::load` fixture，断言 registry 中 forge/alchemy/scroll_hidden 的坐标来自正确 manifest，且不会被 fallback 误替换。
- [ ] 集成回归：`BONG_TERRAIN_RASTER_PATH=<regen 后 manifest>` 启服，检查 `PoiNoviceRegistry` 六类新手 POI 均在 spawn 新手半径内且 selection tag 不因非 spawn regen 退化。

## 6. 对抗复核结论

已完成两轮对抗复核。

- 候选观点：非 spawn 增量 regen 用局部 `fields` 重算全局 novice POI，并覆盖同一 `manifest["pois"]`。
- 反方质疑：影响面可能只是 dev console；`manifest["pois"]` 刷新可能是有意；fallback 本身是设计路径；缺少 server 启服后的玩家可见闭环。
- 修正/反驳：限定为“原地写坏 manifest 后下一次启服/联调/快照可见”；补齐 `TerrainProvider::load` 与 `PoiNoviceLoader::load` 闭环；明确 blueprint 静态 POI 刷新合理，但 novice POI 是 spawn 全局派生，不能用远区局部 fields 重算。
- 最终裁决：认可为高置信 BugHunt skeleton 候选；据当前 #969-#1053 主题与仓内 plan 检索，不重复既有 worldgen BugHunt。
