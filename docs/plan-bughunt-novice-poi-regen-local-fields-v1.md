# plan-bughunt-novice-poi-regen-local-fields-v1

> Active Plan. 一句话主题：worldgen console / incremental regen 在重烘非 spawn zone 时，用局部 `fields` 重算并覆盖全局新手 POI manifest，导致下一次用该 raster manifest 启服时，新手炼器台、丹炉、残卷等 POI 坐标漂移到 fallback 或错误局部语义。

## 阶段总览

| 阶段 | 状态 | 可核验交付物 |
|---|---|---|
| P0：POI 局部合并语义 | ✅ 2026-07-10 | `raster_export._merge_regen_poi_payload` 仅替换目标 zone，保留其他 zone 与全局 novice 条目 |
| P1：完整选择窗口契约 | ✅ 2026-07-10 | `novice_poi_selection_tile_ids` 从 plan + 最大 2000 格搜索半径 + 一采样步梯度 halo 独立推导 required tiles |
| P2：console 有界重算 | ✅ 2026-07-10 | 目标 rewrite tiles 与 novice window 相交时 `synthesize_fields(tile_filter=...)`；默认蓝图 305 active tiles → 16 required tiles |
| P3：worldgen 回归与原子性 | ✅ 2026-07-10 | 远区保留、近区非 spawn/spawn 重算、fields/manifest 缺 tile、写盘前失败、非目标 POI 哨兵测试 |
| P4：server 启动加载闭环 | ✅ 2026-07-10 | 真实 v2 manifest → `TerrainProvider::load` → 生产 `poi_novice::register` Startup → registry + `PoiSpawned` 六类完整载荷；Bot 只读诊断 |
| P5：PR gates 与归档 | ⏳ | PR #1153 e2e / snapshot / `/review`；全绿后 `scripts/plan-finish.sh` |

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

## 4. 实施阶段

- [x] P0：拆分 manifest POI 刷新语义。`regen_zone()` 只刷新被重烘 zone 的 blueprint 静态 POI / profile 派生 POI；全局 spawn novice POI 保留旧 manifest 值，除非调用方提供完整选择窗口。
- [x] P1：选择器从 generation plan、最大 relaxed radius 与一采样步梯度 halo 独立推导 required tile IDs；full/bounded 输入统一裁剪到固定网格，局部 `GeneratedFieldSet` 不能用自身 IDs 或外接矩形自证完整。
- [x] P2：console `/api/regen` 先合成本地 rewrite fields；其 tile 与 16-tile novice window 相交时再有界合成完整 window 并重算（含近区非 spawn），完全不相交的远区 regen 保留全局 novice POI，不做全图 synthesis。
- [x] P3：补 regression pin：远区非 spawn novice 逐项保留、近区非 spawn/spawn 完整窗口重算、目标 zone profile POI 刷新、非目标 zone authored/profile POI 逐字段保留。
- [x] P4：文档化边界：blueprint/profile POI 仅按目标 zone patch；global novice POI 由 spawn 周边选择窗口独立管理。真实 manifest 测试经生产 `poi_novice::register` 触发 loader，逐类锁定坐标、selection tag、选择半径及 `PoiSpawned` 完整载荷；`/tppoi novice` + `terrain_poi_novice_startup` 提供协议 Bot 只读可观察面。

## 5. 验证计划

- [x] worldgen：完整 manifest + 远区非 spawn 局部 fields 后，六类 `poi_novice` entries 逐项不变；近区非 spawn rewrite tiles 与 required window 相交时，从完整当前 window 修复陈旧坐标。
- [x] worldgen：spawn 使用 plan-derived 完整选择窗口重算；默认 blueprint 305 active tiles 降为 16 required tiles；远端额外 fields 不再改变 `np.gradient` 边缘语义，bounded/full 坐标一致。
- [x] console：目标 zone profile-derived POI 刷新；非目标 zone authored/profile 哨兵条目原样保留。
- [x] 原子性：novice fields 缺 required tile、existing manifest 缺 required tile、`--zone-filter` full export 缺窗口时，均在 raster/manifest 写入前失败。
- [x] server：真实磁盘 v2 manifest 经 `TerrainProvider::load` 与生产 `poi_novice::register` Startup，六类 registry ID/坐标/半径/selection tag 与六个 `PoiSpawned` 完整载荷全部命中；Bot 场景黑盒确认 registry 资源由真实 world 注册链提供且可观察。
- [ ] PR gate：#1153 e2e、snapshot、统一 `/review` 完成；CodeRabbit 额度/Review 429 仅按 infra 失败记录，不伪装为代码通过。

## 6. 对抗复核结论

已完成两轮对抗复核。

- 候选观点：非 spawn 增量 regen 用局部 `fields` 重算全局 novice POI，并覆盖同一 `manifest["pois"]`。
- 反方质疑：影响面可能只是 dev console；`manifest["pois"]` 刷新可能是有意；fallback 本身是设计路径；缺少 server 启服后的玩家可见闭环。
- 修正/反驳：限定为“原地写坏 manifest 后下一次启服/联调/快照可见”；补齐 `TerrainProvider::load` 与 `PoiNoviceLoader::load` 闭环；明确 blueprint 静态 POI 刷新合理，但 novice POI 是 spawn 全局派生，不能用远区局部 fields 重算。
- 首轮无上下文 Ultra validator：FAIL，发现局部 fields 自证完整与 spawn 全图 9.75 GiB synthesis；已分别由 `60343597`、`c98d5c83`、`52417abc` 修复。
- 二轮无上下文 Ultra validator：PASS；独立确认 305→16、bounded/full 坐标一致、fields/manifest 原子拒绝及 server 坐标直传。
- PR `/review` 首轮 substantive findings：目标 zone 之外 POI 被全量重建、server pin 绕过生产 Startup、active plan 仍标 skeleton；均已纳入本阶段返工。
- 三轮无上下文 Ultra validator：FAIL；复现 full 输入多带远端 tile 会扩大 selector 外接矩形，使 required tile 边缘从单边梯度切到中央梯度并改变 POI。`db8f6e3f` 改为 plan-derived 固定网格 + 一采样步 halo，并补 edge-only bounded/full 对拍；随后进入四轮复审。
- 四轮无上下文 Ultra validator：首轮 FAIL 发现非整除 `tile_size/sample_stride` 跨 tile 样本碰撞；`13d2bea8` 改为 selection-bounds 全局采样相位并补正/负 seam 回归。全新 gpt-5.6-sol Ultra 复审 PASS，独立覆盖 630 组正负坐标、缺口、非矩形、单 tile、非整除及 `tile_size < stride`。
- PR `/review` 二轮 substantive findings：近区非 spawn 与 required window 相交仍保留旧 novice POI、Startup 测试未走生产 register/完整载荷断言、缺 Bot e2e、active plan 未记录 validator PASS；均已纳入本轮返工。
