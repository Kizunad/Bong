# BugHunt: worldgen 离散 id/enum 层误用 maximum 算术混合，边界带被截断成错误取值

## Bug 摘要

严重度：high（skeptic 未调整，维持 high）。

`worldgen/scripts/terrain_gen/fields.py` 的 `LAYER_REGISTRY` 里，**4 个 `export_type="uint8"` 的离散 id/enum 层**（`spirit_eye_candidates` L237、`underground_tier` L252、`fossil_bbox` L284、`tsy_presence` L300）被错误标成 `blend_mode="maximum"`。stitcher 在 `_blend_tile_layers` 的额外层循环里，对非 `swap`/`minimum`/`lerp` 的层一律走 `blended = np.maximum(base_arr, overlay_arr * weight)`（`stitcher.py:408`），这里 `weight` 是区域边界处的**分数值**（0~1 连续插值权重，见 `_compute_boundary_weight_array`）。`overlay_arr`（uint8）乘上一个分数 `weight` 后被 numpy 自动升格成 `float64`，产出 0.6、1.2 这类**不存在于合法取值域内的小数中间值**，写回 `base_tile.layers[extra_layer]`（`stitcher.py:411`）。这个浮点污染直到 `TileFieldBuffer.compact_layers()`（`fields.py:543-550`）才被截断回 `uint8`——而 `np.ascontiguousarray(..., dtype=uint8)` 是**截断（向下取整）不是四舍五入**：0.6→0（把候选/存在标记悄悄抹掉），1.2→1（在真值只可能是 0 或 2 的情况下伪造出一个从未出现过的"浅层"取值）。

`stitcher.py:400-406` 紧邻的 `swap` 分支注释原话点破了这个反模式——"never multiply or maximum integer ids (would corrupt the ... index by mixing zones together)"——但 registry 偏偏把这 4 个 uint8 层配进了被自己注释禁止的那条路径，而不是像其余所有离散 id 层（`surface_id`/`subsurface_id`/`biome_id`/`flora_variant_id`/`ground_cover_id`/`zongmen_origin_id`/`mineral_kind`/`anomaly_kind`/`tsy_origin_id`/`tsy_depth_tier`，全部 `blend_mode="swap"`）一样走干净的 dithered `np.where` 二选一。这是 `LAYER_REGISTRY` 配置错误，不是有意设计。

## 实际游玩体验影响

这 4 个层各自的边界带损坏，直接影响玩家能感知到的采集/环境判定，且**没有崩服、没有报错**，纯粹是数据悄悄错——最难被发现的一类 bug：

- **`underground_tier`**（0=地表/1=浅洞/2=中洞/3=深渊）：`server/src/botany/env_lock.rs:154-156` 的 `EnvLock::UndergroundTier { tier }` 对采样值做**精确相等**判定，用来门禁「5 灵草 environment locks」（`server/src/world/terrain/raster.rs:239-247` doc comment原话）。`wuxing_abyss`（`abyssal_maze` 地形，边界宽度 104 格）的整圈外环会读到被截断的错误 tier，导致某些灵草在该应该能长的边界带完全长不出来，或者在不该长的浅层伪造出深渊灵草。
- **`fossil_bbox`**（0=无/1=鲸落外围肋骨/2=富矿核心）：`server/src/mineral/anchors.rs:155-186` 的 `spawn_fossil_mineral_nodes` / `fossil_mineral_positions` 逐列读 `sample_fossil_bbox` 来实体化矿脉。`north_wastes`（`waste_plateau`，边界宽度 96）外环会出现矿脉节点随机消失或错位实体化。
- **`spirit_eye_candidates`**：`qingyun_peaks`（`broken_peaks`，宽度 128）、`lingquan_marsh`（`spring_marsh`，宽度 128）、`blood_valley`（`rift_valley`，宽度 72）三个区域的边界带候选密度被静默削薄——玩家绕着这些区域边缘找灵眼会比预期少得多。
- **`tsy_presence`**：`raster.rs:274-276`（doc comment）标注它是"该列是否在 TSY 家族 AABB 内"的热路径 mask；4 个 tsy_* profile 全都对整块 tile 写全 1，但边界带混合后会读到 false-0，导致末法残土维度地基判定在维度边缘不可靠。

三个受影响区域（`qingyun_peaks`/`lingquan_marsh`/`blood_valley`）+ `north_wastes` + `wuxing_abyss` 都是玩家正常探索会走到的地表区域，边界带宽度 72~128 格，是一圈相当宽的常规探索环，不是极端角落。

## 证据定位

- `worldgen/scripts/terrain_gen/fields.py:237` — `spirit_eye_candidates`：`LayerSpec(safe_default=0.0, blend_mode="maximum", export_type="uint8")`。
- `worldgen/scripts/terrain_gen/fields.py:252` — `underground_tier`：同上误配（紧邻 L250-251 doc comment 说明 0/1/2/3 四档语义）。
- `worldgen/scripts/terrain_gen/fields.py:284` — `fossil_bbox`：同上误配（doc comment 说明 0/1/2 三档语义）。
- `worldgen/scripts/terrain_gen/fields.py:300` — `tsy_presence`：同上误配。
- `worldgen/scripts/terrain_gen/fields.py:322-331` — `layer_compact_dtype`：对 `export_type=="uint8"` 的层强制返回 `np.uint8`，是最终导出前的截断口径来源。
- `worldgen/scripts/terrain_gen/fields.py:543-550` — `TileFieldBuffer.compact_layers`：`np.ascontiguousarray(values, dtype=target_dtype)` 把污染后的浮点值截断回 uint8（`np.ascontiguousarray([0.6,1.2,1.8], dtype=np.uint8)` 实测输出 `[0,1,1]`，验证是截断不是四舍五入）。
- `worldgen/scripts/terrain_gen/stitcher.py:164-201` — `_compute_boundary_weight_array`：`weight` 在整个边界带内是连续分数值，只有区域内部 `ratio <= 1 - blend_ratio` 才等于精确的 interior_weight（≥0.55~0.8），外环处处是分数。
- `worldgen/scripts/terrain_gen/stitcher.py:379-411` — `_blend_tile_layers` 额外层循环：`spec.blend_mode` 决定分支；`swap` 分支（L400-406）用 `np.where(swap, overlay_arr, base_arr).astype(base_arr.dtype)` 干净二选一；`maximum` 分支（L407-408）`blended = np.maximum(base_arr, overlay_arr * weight)`——这条正是 4 个 uint8 层实际落入的分支。
- `worldgen/scripts/terrain_gen/profiles/broken_peaks.py:270` / `spring_marsh.py:259` / `rift_valley.py:275` — 三处 `buffer.layers["spirit_eye_candidates"] = select_spirit_eye_candidates(...)` 生产者，写入 0/1 掩码。
- `worldgen/scripts/terrain_gen/profiles/abyssal_maze.py:352` — `buffer.layers["underground_tier"] = tier.ravel().astype(np.uint8)`，写入 0..3。
- `worldgen/scripts/terrain_gen/profiles/waste_plateau.py:239` — `buffer.layers["fossil_bbox"] = fossil_bbox.ravel().astype(np.uint8)`，写入 0..2。
- `worldgen/scripts/terrain_gen/profiles/tsy_zongmen_ruin.py:190` / `tsy_daneng_crater.py:219` / `tsy_zhanchang.py:190` / `tsy_gaoshou_hermitage.py:205` — 四处 `buffer.layers["tsy_presence"] = np.ones(area, dtype=np.uint8)`，整块 tile 写全 1。
- `server/src/botany/env_lock.rs:154-156` — `EnvLock::UndergroundTier { tier }` 对采样值做 `actual.round() as u8 == tier` 精确匹配，是「5 灵草 environment locks」的门禁消费者。
- `server/src/mineral/anchors.rs:155-186` — `spawn_fossil_mineral_nodes` / `fossil_mineral_positions` 读取 `sample_fossil_bbox` 逐列决定矿脉节点是否实体化。
- `server/src/world/terrain/raster.rs:239-247` — doc comment 明确写「the 5 灵草 environment locks key off them [underground_tier] directly」。
- `server/zones.worldview.example.json` — 实测 `qingyun_peaks`(broken_peaks, width=128) / `lingquan_marsh`(spring_marsh, width=128) / `blood_valley`(rift_valley, width=72) / `north_wastes`(waste_plateau, width=96) / `wuxing_abyss`(abyssal_maze, width=104)，与 finding 可达性claim 完全一致。
- `worldgen/scripts/terrain_gen/harness/raster_check.py:127-136`（`underground_tier` 只查 `max > 3`）、`:159-168`（`fossil_bbox` 只查 `max > 2`）——都只做上界检查，截断后的值仍落在合法区间内，**查不出这类"值域内但取值错误"的污染**。对照 `:236-249` 对 `realm_collapse_mask` 已有的更严格写法（`any(value not in (0, 1) for value in raw)` 精确集合校验），说明该模块本就有能力做更强校验，只是没有覆盖到这 4 个层。
- 补充复核发现：`fields.py:241` 的 `realm_collapse_mask` 同样是 `uint8` + `blend_mode="maximum"`，与这 4 个层同款误配模式；但实测 `worldgen/scripts/terrain_gen/profiles/*.py` 里没有任何 profile 的 `TileFieldBuffer.create(...)` 层列表包含 `"realm_collapse_mask"`（该层只经由 `stitcher.py:503-513` 的专用函数 `_apply_realm_collapse_mask` 写入，用干净的 `(weight > 0.0).astype(np.uint8)` 布尔转换，不经过额外层循环的 `maximum` 分支），所以**当前不会实际触发**这个具体污染路径——但它是同一 registry 模式下的一个脆弱点，值得在本次修复里顺手复核/加固，防止未来有 profile 直接往这层写值时重蹈覆辙。

## 触发路径

1. 世界地图 blueprint（如 `server/zones.worldview.example.json`）声明一个区域（如 `wuxing_abyss`/`abyssal_maze`，boundary width=104）。
2. `terrain_gen` 主流程为该区域相交的每个 tile 调用 `_build_zone_overlay_tile` 生成该区域自己的 profile 层（其中 `underground_tier` 等离散层写入合法整数值 0..3）。
3. `_blend_tile_layers` 对该 tile 做区域↔荒野混合：连续分数值 `weight`（`_compute_boundary_weight_array`）在边界带内处处 0<weight<1。
4. 额外层循环对 `underground_tier`（`LAYER_REGISTRY` 误配 `blend_mode="maximum"`）走 `np.maximum(base_arr, overlay_arr * weight)`，产出如 `2 * 0.6 = 1.2` 的浮点污染值，写回 `base_tile.layers["underground_tier"]`。
5. `TileFieldBuffer.compact_layers()` 把该浮点数组截断回 `uint8`：`1.2` → `1`（伪造出一个真值序列里从未出现过的"浅层"取值）。
6. 该值被烘焙进 `.bin` raster 导出文件；`raster_check.py` 的上界检查（`max <= 3`）通过，检测不到问题。
7. Rust 端 `server/src/botany/env_lock.rs` / `server/src/mineral/anchors.rs` 运行时逐列采样这个被污染的值，做出错误的灵草门禁 / 矿脉实体化判断——玩家在区域边界带 96~128 格的常规探索环内看到的地形语义与 profile 设计意图不符。

## 反方审查记录

- 第一轮质疑：核对 `LAYER_REGISTRY` 是否真的只有这 4 个 uint8 层用了 `maximum`——逐条读完 L205-303 全部条目，确认恰好这 4 个（其余 uint8 id/enum 层全是 `swap`），排除"随手挑几个巧合"的可能。核对 `weight` 是否真是连续分数而非仅在 0/1——读 `_compute_boundary_weight_array` 全函数，确认整条边界带内 `weight` 处处是平滑插值出来的分数，只有区域深处内部才钳到接近 1 的常量。
- 第二轮补证：逐个核对 4 个层各自的生产者文件行号（`broken_peaks.py`/`spring_marsh.py`/`rift_valley.py`/`abyssal_maze.py`/`waste_plateau.py`/4 个 `tsy_*` profile），确认都是干净写入合法整数域；核对 Rust 端确有真实消费者（`env_lock.rs::UndergroundTier` 精确相等判定、`anchors.rs::spawn_fossil_mineral_nodes` 逐列读矿脉），不是"写了没人读"的孤岛数据；实测 `np.ascontiguousarray([0.6,1.2,1.8], dtype=np.uint8)` 确认是截断不是四舍五入。核对 `server/zones.worldview.example.json` 确认可达性——5 个区域边界宽度 72~128 格全部命中默认 blueprint，非 dev-only 或极端路径。
- 查重结论：对照三个相邻/已知 bug 排除重叠——`plan-bughunt-tsy-y-strata-overlay`（2D 高度/深度层覆盖问题，不同故障模式，非 blend-mode 浮点截断）；`plan-bughunt-spirit-eye-raster-candidate-disconnect-v1`（`SpiritEyeRegistry` 从不读取候选层，是消费端断链，不是生产端数据污染——本 bug 是在那个消费者能读到之前，候选层本身已经被污染）；`plan-bughunt-raster-check-required-layers-v1`（校验器漏检缺失的 base 层导致 server panic，与本 bug 的"值域内但取值错误"是不同类问题）。三者均不重叠，本 bug 是独立、此前未记录的缺陷。
- 终裁：通过，严重度维持 high——真实、静默、边界带范围广，影响矿脉/灵草门禁数据但不触发 crash、不涉及真元守恒。
- 主循环复核：已亲读关键行确认（`fields.py` L205-303 全量 registry、`stitcher.py` L164-411 混合逻辑、5 个 profile 生产者文件、`env_lock.rs`/`anchors.rs` 消费者、`zones.worldview.example.json` 边界宽度、`raster_check.py` 现有校验粒度），并额外核实 `realm_collapse_mask`（`fields.py:241`）同款误配但当前不可达，已在证据定位补充说明。

## Skeleton Fix Plan

- [ ] 把 `worldgen/scripts/terrain_gen/fields.py` 中 `spirit_eye_candidates`（L237）、`underground_tier`（L252）、`fossil_bbox`（L284）、`tsy_presence`（L300）四个 `LayerSpec` 的 `blend_mode` 从 `"maximum"` 改为 `"swap"`，与仓库里其余全部离散 id/enum uint8 层（`surface_id`/`mineral_kind`/`anomaly_kind`/`tsy_origin_id` 等）保持一致的处理路径，复用 `stitcher.py:400-406` 现成的 dithered `np.where` 二选一分支（无需新代码）。
- [ ] 如果产品设计上确实需要 `tsy_presence` / `fossil_bbox` 保持"只增不减"的语义（即一旦某列被任一重叠区域标记过就永不被后续区域清零），不要继续套用算术 `maximum`——改成显式阈值合并：`blended = np.maximum(base_arr, np.where(weight >= 0.5, overlay_arr, 0)).astype(base_arr.dtype)`，确保结果永远是 `{base_arr 取值, overlay_arr 取值}` 之一，不产出伪造中间值。**需要人工/后续 PR review 确认这条"grow-only"需求是否真实存在**——若不存在，直接用上一条的 `swap` 即可，更简单且与代码库既有模式一致。
- [ ] 顺手审计 `realm_collapse_mask`（`fields.py:241`，同款 `uint8`+`maximum` 误配）：确认所有 profile 的 `TileFieldBuffer.create(...)` 层列表都不包含它（当前如此，靠 `_apply_realm_collapse_mask` 专用函数写入才安全），并在该 `LayerSpec` 旁补一行注释说明"为什么这里的 maximum 目前是安全的、以及未来新增 profile 直接写这层时必须走 swap 或专用函数，不能指望这条 registry 配置本身"，防止未来复发同款 bug。
- [ ] 不改动其余 uint8 层（`surface_id`/`biome_id`/`flora_variant_id`/`mineral_kind`/`anomaly_kind`/`tsy_origin_id`/`tsy_depth_tier`/`ground_cover_id`/`zongmen_origin_id`）——它们已经是 `swap`，本次修复范围只限这 4 个（+ 视审计结论决定是否顺手修 `realm_collapse_mask` 的注释）。
- [ ] `worldgen/scripts/terrain_gen/harness/raster_check.py` 补离散层值域校验：把 `underground_tier`（当前只查 `max > 3`，L127-136）、`fossil_bbox`（当前只查 `max > 2`，L159-168）、`spirit_eye_candidates`（当前完全没有校验）、`tsy_presence`（当前完全没有校验）都升级成 `realm_collapse_mask` 已有的精确集合校验模式（L236-249：`any(value not in (0, 1) for value in raw)`）——`underground_tier` 校验 `{0,1,2,3}`，`fossil_bbox` 校验 `{0,1,2}`，`spirit_eye_candidates`/`tsy_presence` 校验 `{0,1}`。这条校验本身**不能**独立证明本次修复生效（截断后的值仍落在合法集合内，只是"取值错误"而非"越界"），但能防止未来任何新离散层再引入越界式的更粗糙污染，是 defense-in-depth 而非本 bug 的直接回归锁。
- [ ] 本修复不涉及真元/灵气流动——`qi_density`/`mofa_decay`/`qi_vein_flow` 三层本来就正确使用 `lerp`/`maximum`（连续值，非离散 id），不在本次改动范围内，无需过 `qi_physics::ledger` 守恒口径。
- [ ] 本修复不涉及 C2S 请求路径——纯 worldgen 离线烘焙管线 bug，没有 client/server 运行时协议改动，无需 server gate。

## 验收测试计划

全部落在 `worldgen/` pytest（`cd worldgen && python -m pytest` 或随 `bash scripts/dev-reload.sh` 的 `[2/4] raster 后验`一并跑）：

- **happy path**：新增 `worldgen/tests/test_stitcher_discrete_layer_blend.py`（或并入既有 stitcher 测试文件），对 `spirit_eye_candidates`/`underground_tier`/`fossil_bbox`/`tsy_presence` 四层分别构造 `base_arr`/`overlay_arr` 为已知合法整数（如 `underground_tier` 用 `base=0, overlay=2`），扫一组 `weight ∈ {0.0, 0.1, 0.3, 0.5, 0.7, 0.9, 1.0}`，断言 `_blend_tile_layers` 产出的每个元素都 `∈ {base_val, overlay_val}`（不再出现 `1`、`1.2` 这类中间值）——失败信息需带上具体的 `weight` 和实际输出值，方便直接定位是哪个权重档位回归。
- **边界 case**：`weight` 恰好等于 `swap_threshold`（含 dither noise 后的浮动阈值）时的取整行为——断言输出仍是二选一，不因浮点比较误差产生第三值；`weight=0.0` 严格取 `base_val`，`weight=1.0` 严格取 `overlay_val`（对齐仓库既有"边界值不漏判"的惯例，参考 `_compute_circular_mask` 注释里 `<=` 而非 `<` 的教训）。
- **错误/回归分支**：反向断言——本次修复前的 `blend_mode="maximum"` 配置（用一个临时 monkeypatch 的 `LayerSpec`）确实会产出 `1.2`/`0.6` 这类浮点污染值（锁住"这条路径本身是坏的"这一事实，防止将来有人把 4 层的 `blend_mode` 悄悄改回 `maximum` 而没人发现），修复后的配置不会。
- **状态转换（多区域连续混合）**：构造两个相邻/重叠区域依次对同一 tile 调用 `_blend_tile_layers`（模拟一个 tile 同时与两个 `blueprint_zones` 相交的真实场景），断言链式混合后离散层值全程保持合法整数域，不因多次 `maximum` 叠加产生复合浮点误差。
- **`TileFieldBuffer.compact_layers` 契约测试**：直接构造一个已经是纯合法整数（无浮点污染）的层数组跑 `compact_layers()`，断言值不变（防止误把"修 blend 之外顺手改 compact_layers 截断逻辑"当成本 bug 的修复范围——本 bug 的正确修法是不产出浮点污染，而不是在截断处加舍入）。
- **`raster_check.py` 值域校验回归**（`worldgen/scripts/terrain_gen/harness/` 下已有的 `raster_check` 测试文件里新增用例）：构造一个 `underground_tier.bin` fixture 含合法值 `{0,1,2,3}` 应 PASS；构造一个含非法值（如全部离散层校验升级后应该拦的越界值）应 FAIL 并带清晰错误信息；`spirit_eye_candidates`/`tsy_presence` 同样各配一条 PASS + 一条 FAIL fixture。
- **端到端**：`bash scripts/dev-reload.sh`（仓库根目录）走完整 `[1/4] regen` + `[2/4] raster 后验`，确认 `wuxing_abyss`/`north_wastes`/`qingyun_peaks`/`lingquan_marsh`/`blood_valley` 五个区域重新烘焙后 `raster_check.validate_rasters` 全绿，且（可选）人工抽查 `underground_tier`/`fossil_bbox` 导出 raster 在这几个区域边界带内的取值分布确实只落在生产者写入过的合法值集合里。

## 风险

- 若 `tsy_presence`/`fossil_bbox` 真的存在产品设计上要求的"只增不减"语义（详见 Fix Plan 第二条），直接改成 `swap` 可能引入新的行为变化（某列原本因为算术 maximum 巧合保留住的标记，改成 dithered swap 后在特定权重区间被换成 0）——这条需要在 PR 里明确调用哪个方案并说明理由，不能两个方案都不选就合并。
- 本次修复只改 `blend_mode` 配置 + 复用既有 `swap` 分支代码，不改动 `_blend_tile_layers` 的分支逻辑本身，风险面很小；但任何触碰 `LAYER_REGISTRY` 的改动都要求全量重跑 `scripts/dev-reload.sh` 而不是只跑单元测试——因为这是运行时按需读取的 mmap raster 二进制文件，线上/本地已烘焙的旧 raster 不会自动重新生成，必须重新烘焙才能看到修复效果。
- `raster_check.py` 的值域收紧（第二类校验从"越界检查"升级到"精确集合校验"）如果发现某个未预料到的历史区域已经有值域外的合法值（例如某个 profile 曾经写过 `underground_tier=4` 之类未在四档语义里的值），会让原本静默通过的 raster 现在报错——这属于修复的正常副作用（暴露了另一处潜在数据问题），但需要在 PR 里说明"如果 CI 因此新增报错，先核实是不是本 bug 的截断污染残留在已提交的测试 fixture 里，而不是无脑放宽阈值"。
