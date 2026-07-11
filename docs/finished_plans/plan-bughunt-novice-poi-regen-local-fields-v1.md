# plan-bughunt-novice-poi-regen-local-fields-v1

> Finished Plan（2026-07-11）。一句话主题：worldgen console / incremental regen 在重烘非 spawn zone 时，用局部 `fields` 重算并覆盖全局新手 POI manifest，导致下一次用该 raster manifest 启服时，新手炼器台、丹炉、残卷等 POI 坐标漂移到 fallback 或错误局部语义。

## 阶段总览

| 阶段 | 状态 | 可核验交付物 |
|---|---|---|
| P0：POI 局部合并语义 | ✅ 2026-07-10 | `raster_export._merge_regen_poi_payload` 仅替换目标 zone，保留其他 zone 与全局 novice 条目 |
| P1：完整选择窗口契约 | ✅ 2026-07-10 | `novice_poi_selection_tile_ids` 从 plan + 最大 2000 格搜索半径 + 一采样步梯度 halo 独立推导 required tiles |
| P2：console 有界重算 | ✅ 2026-07-10 | 目标 rewrite tiles 与 novice window 相交时 `synthesize_fields(tile_filter=...)`；默认蓝图 305 active tiles → 16 required tiles |
| P3：worldgen 回归与原子性 | ✅ 2026-07-10 | 远区保留、近区非 spawn/spawn 重算、fields/manifest 缺 tile、写盘前失败、非目标 POI 哨兵测试 |
| P4：server 启动加载闭环 | ✅ 2026-07-10 | 真实 v2 fixture → `TerrainProvider::load` → 生产 `poi_novice::register` Startup → registry + `PoiSpawned` 六类独立全字段对拍；Bot 强制六类各 1 并核对 selection |
| P5：PR gates 与归档 | ✅ 2026-07-11 | `a019a085` 的 e2e、snapshot、CodeRabbit 全绿并完成独立 Ultra PASS；归档提交后的新 HEAD 需重新 `/review`，以有效 4/0 作为 PR merge gate |

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
- [x] P4：文档化边界：blueprint/profile POI 仅按目标 zone patch；global novice POI 由 spawn 周边选择窗口独立管理。真实 manifest 测试经生产 `poi_novice::register` 触发 loader，以独立期望逐字段锁定六类 registry / `PoiSpawned` 的 id、kind、zone、name、坐标、selection、qi、danger 与完整 tags；Bot e2e 默认生成含六类各 1 的真实 v2 raster fixture，`/tppoi novice` 黑盒核对目标类别计数与 selection，并允许合法运行时 `surface_stash` 扩展项。
- [x] P5：核验 PR #1153 在 `a019a085` 上的 e2e run `29086695644`、snapshot run `29086695634` 与 CodeRabbit 均为 SUCCESS，补齐最终独立 Ultra PASS 及本节 Finish Evidence，并通过 `scripts/plan-finish.sh` 归档。由于归档本身生成新 HEAD，统一 `/review` 必须在归档提交推送后重新触发；该有效 4/0 是 merge gate，不在归档前伪记为已完成。

## 5. 验证计划

- [x] worldgen：完整 manifest + 远区非 spawn 局部 fields 后，六类 `poi_novice` entries 逐项不变；近区非 spawn rewrite tiles 与 required window 相交时，从完整当前 window 修复陈旧坐标。
- [x] worldgen：spawn 使用 plan-derived 完整选择窗口重算；默认 blueprint 305 active tiles 降为 16 required tiles；远端额外 fields 不再改变 `np.gradient` 边缘语义，bounded/full 坐标一致。
- [x] console：目标 zone profile-derived POI 刷新；非目标 zone authored/profile 哨兵条目原样保留。
- [x] 原子性：novice fields 缺 required tile、existing manifest 缺 required tile、`--zone-filter` full export 缺窗口时，均在 raster/manifest 写入前失败。
- [x] server：真实磁盘 v2 manifest 经 `TerrainProvider::load` 与生产 `poi_novice::register` Startup，六类 registry / `PoiSpawned` 与独立完整期望逐字段相等；Bot 场景用真实 fixture 黑盒确认六个目标类别各 1、selection 精确且 registry 总计数自洽，loader 断链或任一类别缺失都会失败。
- [x] PR gate（归档前可核验部分）：#1153 在 `a019a085` 上的 e2e、snapshot、CodeRabbit 均成功；此前 `/review` 的 substantive findings 已逐轮返工。归档后新 HEAD 尚无有效 4/0，必须重新评论 `/review` 并等待四票 APPROVE 后才可 merge；CodeRabbit 额度或 Review 429 仅按 infra 失败记录，不伪装为代码通过。

## 6. 对抗复核结论

已完成多轮对抗复核。

- 候选观点：非 spawn 增量 regen 用局部 `fields` 重算全局 novice POI，并覆盖同一 `manifest["pois"]`。
- 反方质疑：影响面可能只是 dev console；`manifest["pois"]` 刷新可能是有意；fallback 本身是设计路径；缺少 server 启服后的玩家可见闭环。
- 修正/反驳：限定为“原地写坏 manifest 后下一次启服/联调/快照可见”；补齐 `TerrainProvider::load` 与 `PoiNoviceLoader::load` 闭环；明确 blueprint 静态 POI 刷新合理，但 novice POI 是 spawn 全局派生，不能用远区局部 fields 重算。
- 首轮无上下文 Ultra validator：FAIL，发现局部 fields 自证完整与 spawn 全图 9.75 GiB synthesis；已分别由 `60343597`、`c98d5c83`、`52417abc` 修复。
- 二轮无上下文 Ultra validator：PASS；独立确认 305→16、bounded/full 坐标一致、fields/manifest 原子拒绝及 server 坐标直传。
- PR `/review` 首轮 substantive findings：目标 zone 之外 POI 被全量重建、server pin 绕过生产 Startup、active plan 仍标 skeleton；均已纳入本阶段返工。
- 三轮无上下文 Ultra validator：FAIL；复现 full 输入多带远端 tile 会扩大 selector 外接矩形，使 required tile 边缘从单边梯度切到中央梯度并改变 POI。`db8f6e3f` 改为 plan-derived 固定网格 + 一采样步 halo，并补 edge-only bounded/full 对拍；随后进入四轮复审。
- 四轮无上下文 Ultra validator：首轮 FAIL 发现非整除 `tile_size/sample_stride` 跨 tile 样本碰撞；`13d2bea8` 改为 selection-bounds 全局采样相位并补正/负 seam 回归。全新 gpt-5.6-sol Ultra 复审 PASS，独立覆盖 630 组正负坐标、缺口、非矩形、单 tile、非整除及 `tile_size < stride`。
- PR `/review` 二轮 substantive findings：近区非 spawn 与 required window 相交仍保留旧 novice POI、Startup 测试未走生产 register/完整载荷断言、缺 Bot e2e、active plan 未记录 validator PASS；均已纳入本轮返工。
- 五轮无上下文 Ultra validator：PASS（HEAD `0e34c238`）；独立执行 990 组属性检查，复核空间相交重算、非整除采样网格、目标 zone POI patch、生产 register Startup、`PoiSpawned` 完整载荷及 Bot 只读观察面，未发现 blocking/major correctness finding。
- PR `/review` 三轮 substantive findings：Bot 接受空 registry、Startup 测试以同源 registry/event 互证而未独立锁定 zone/name/qi/danger/full tags；现已改为 stdlib-only 真实 v2 Bot fixture + 六类各 1/selection 黑盒断言，并让 registry 与事件分别对拍独立全字段期望。聚焦验证为 worldgen/console `74 passed + 3 subtests`、server `poi_novice` 24 项、`tppoi` 6 项、命令树 3 项、raster 映射 1 项，以及真实 fixture 启服 Bot 单场景 PASS。
- 六轮无上下文 Ultra validator：FAIL（HEAD `fd17a04a`），发现 `bot-e2e.sh` 在 raster 模式仍等待 fallback 专属日志、selection 子串断言会把 `relaxed_radius_2000_qi_margin_0_1` 误认成 `relaxed_radius_2000`。已改为 fallback/raster/anvil 共用的 world bootstrap 完成锚点，并解析完整 `selection=` token 精确比较；Bot 纯逻辑 49 项含两种合法策略互相混淆的正反回归，debug 编排实际越过就绪门，真实 fixture 单场景再次 PASS。
- 七轮无上下文 Ultra validator：FAIL（HEAD `edfdc40c`），发现 cargo 编译窗口内若旧 listener 抢占 25565，新进程可完成 Startup 锚点但 TCP bind 失败，脚本仍可能连到旧服。已增加 listener PID → `/proc` 父链归属校验，只有端口由本次 `cargo run` 进程树持有才算就绪，并对 Valence `failed to start TCP listener` 日志立即失败；独立 listener 归属正/反测试及真实 debug 编排均通过。
- 八轮无上下文 Ultra validator：FAIL（HEAD `9408d286`），发现 ownership 汇总 IPv4/IPv6 全部 listener，而 Bot 可被配置去连另一地址族旧服。已将自起模式限定为 `127.0.0.1` 且 ownership 仅检查 `ss -4`；远端/IPv6 必须显式 `BOT_E2E_REUSE=1`。IPv6 自起负分支 exit 2、IPv4 ownership 正/反测试及真实 debug 编排均通过。
- 九轮无上下文 Ultra validator：PASS（HEAD `a019a085`）。全新 `fork_context:false`、gpt-5.6-sol Ultra/priority 严格只读审查 `abf2d44a..a019a085` 的 17 个改动文件，复核前三轮连续 FAIL 的 listener 归属、地址族和真实 fixture 诚实性返工，未发现 blocking/major correctness finding。

## Finish Evidence

- 完成范围：P0-P4 的局部 POI 合并、固定选择窗口、有界重算、写盘原子性与生产 Startup/Bot 闭环均已落地；实现提交范围为 `abf2d44a..a019a085`，共 17 个改动文件。
- 聚焦测试：worldgen/console `74 passed + 3 subtests`；server `poi_novice` 24 项、`tppoi` 6 项、命令树 3 项、raster 映射 1 项；Bot 纯逻辑 49 项；真实 v2 raster fixture 启服场景 PASS。
- 端到端证据：PR #1153 的 e2e run `29086695644` 在 `a019a085` 上 SUCCESS，完整 server tests、smoke harness 与 Bot e2e 23/23；snapshot run `29086695634` SUCCESS；CodeRabbit / Review SUCCESS。
- 独立审计：归档前最终 `fork_context:false` gpt-5.6-sol Ultra/priority validator 对 `a019a085` 给出 PASS，未发现 blocking/major；此前每项 FAIL 均有对应修复提交与正反回归记录，未将失败轮伪写为通过。
- Review 边界：`a019a085` 缺少有效统一 `/review` 4/0，本文不声明其已通过。归档提交会产生新 HEAD；推送后必须针对该 HEAD 评论 `/review`，仅在有效 4/0 且 e2e、snapshot、CodeRabbit 保持绿色时合并 PR。
- 遗留：无已知业务代码 blocker；仅剩归档后 PR merge gates。CodeRabbit 额度或 Review 429 属基础设施失败，可以按约定评论说明，但不得替代有效 4/0，也不得强合。
