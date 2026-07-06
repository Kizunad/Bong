# BugHunt: worldgen carver owner 与 provenance 混用

## 摘要

`worldgen` 的 tile `contributing_zones` 同时承担两个语义：

- provenance：这个 tile 与哪些 zone 的扩展 AABB 相交，方便 manifest / console / 调试展示。
- export control：`raster_export._tile_carver_chain` 把列表中第一个带 carver 的 zone 当作该 tile 的几何主导者。

当某个 zone 与 tile 粗 AABB 相交、但真实 boundary weight 全为 0 时，`_blend_tile_layers` 仍把它写入 `contributing_zones`，随后 export 阶段可能让这个零权重 zone 的 canyon / cave / floating island carver 改写整块 tile 的 spans。

## 实际游玩体验影响

玩家在 zone 边界附近会看到不属于当前位置主导地貌的 3D 雕刻外溢。已确认真实蓝图里 `tile_6_-7` 处在 `zhanhun_plain`（战魂平野）北缘，但 `blood_valley` 对该 tile 的真实权重为 0，仍抢到 `rift_valley` 的 canyon carver，导致大量列被切成峡谷悬壁/洞穴段。

直接体验是：战魂平野边缘出现血谷峡谷式断壁和空洞，地表连续性、寻路、资源落点、区域语义都会漂移。玩家可能以为进入血谷峡谷，但 zone/HUD/资源语义仍偏向战魂平野或边界混合，形成“看起来是峡谷、玩法却不是峡谷”的错位。

## 避重说明

已避开既有 BugHunt / plan 主题：

- #1042 `spawn` 出生 `safe_y` 与地表漂移。
- #1036 `spawn` 教学 POI 高度漂移。
- #1028 TSY 裂缝 family 前缀漂移。
- #1015 北荒渊口遮蔽焦土语义。
- #1008 worldgen pipeline 根目录入口断链。
- #998 TSY Y 分层被 2D overlay 压成 deep 单层。
- #992 `start.sh` 漏接 TSY raster 环境变量。
- #986 巨剑沧海重叠错判为无垠深渊。
- #971 矿脉固定锚点旧坐标漂移到 spawn。

本 plan 的根因是 `contributing_zones` 字段同时控制 manifest provenance 和 export carver owner，触点集中在 `worldgen/scripts/terrain_gen/stitcher.py` 与 `worldgen/scripts/terrain_gen/bakers/raster_export.py`。

## 复现路径

只读复现，不修改仓库：

```bash
cd worldgen
python3 - <<'PY'
from pathlib import Path
import numpy as np
from scripts.terrain_gen.blueprint import load_blueprint, load_profile_catalog, DEFAULT_BLUEPRINT_PATH, DEFAULT_PROFILES_PATH
from scripts.terrain_gen.fields import WorldTile
from scripts.terrain_gen.stitcher import build_generation_plan, synthesize_fields, _compute_boundary_weight_array
from scripts.terrain_gen.noise import _tile_coords
from scripts.terrain_gen.spans_fold import spans_for_tile
from scripts.terrain_gen.bakers.raster_export import _zone_carver_chains, _tile_carver_chain, CARVE_SEED
from scripts.terrain_gen.carvers import apply_carver_chain

bp = load_blueprint(DEFAULT_BLUEPRINT_PATH)
cat = load_profile_catalog(DEFAULT_PROFILES_PATH)
zone = next(z for z in bp.zones if z.name == "blood_valley")
tile = WorldTile(6, -7, 6 * 512, 6 * 512 + 511, -7 * 512, -7 * 512 + 511)
wx, wz = _tile_coords(tile.min_x, tile.min_z, 512)
w = _compute_boundary_weight_array(zone, wx, wz).ravel()
print("blood_valley weight_max", float(np.nanmax(w)), "positive_cols", int(np.count_nonzero(w > 0)), "of", w.size)

plan = build_generation_plan(bp, cat, DEFAULT_BLUEPRINT_PATH, DEFAULT_PROFILES_PATH, Path("/tmp/bong-carver-owner-proof"), 512)
plan.tiles = [tile]
fields = synthesize_fields(plan)
buf = fields.tiles[0]
chains = _zone_carver_chains(plan)
chain = _tile_carver_chain(buf, chains)
base = spans_for_tile(buf, suppress_fold_isle=any(c.name == "floating_island" for c in chain))
carved = apply_carver_chain(base, chain, origin_x=buf.tile.min_x, origin_z=buf.tile.min_z, tile_size=buf.tile_size, seed=CARVE_SEED)
diff = sum(1 for a, b in zip(base, carved) if a.spans != b.spans)
first = next(i for i, (a, b) in enumerate(zip(base, carved)) if a.spans != b.spans)
print("contributing_zones", buf.contributing_zones)
print("carver_chain", [c.name for c in chain])
print("diff_cols", diff)
print("first_diff_world", (buf.tile.min_x + first % 512, buf.tile.min_z + first // 512), base[first].spans, "->", carved[first].spans)
PY
```

观察到的关键输出：

```text
blood_valley weight_max 0.0 positive_cols 0 of 262144
contributing_zones ['blood_valley', 'zhanhun_plain']
carver_chain ['canyon', 'canyon']
diff_cols 36418
first_diff_world (3072, -3584) ((-64, 77),) -> ((59, 77), (-64, 32))
```

## 根因证据

- `worldgen/scripts/terrain_gen/stitcher.py:288-291`：`weight` 全 0 时仍把 `zone.name` append 到 `base_tile.contributing_zones`，然后直接 return；这条路径没有实际 blend 任何 height / surface / extra layer。
- `worldgen/scripts/terrain_gen/stitcher.py:293-404`：正权重路径才实际修改 height、water、feature、boundary_weight、extra layers。
- `worldgen/scripts/terrain_gen/bakers/raster_export.py:154-170`：`_tile_carver_chain` 逐个读取 `buffer.contributing_zones`，返回第一个有 carver 的 zone chain；注释明确把第一个 contributing zone 当作 “geometry is dominated by” 的 zone。
- `server/zones.worldview.example.json:556-576`：`blood_valley` 使用 `rift_valley` / `rotated_rift` / boundary width 72。
- `worldgen/scripts/terrain_gen/profiles/rift_valley.py:91-110`：`rift_valley` 注册两段 canyon carver。
- `server/zones.worldview.example.json:875-895`：`zhanhun_plain` 使用 `ancient_battlefield`，不是 `rift_valley`，不应继承血谷峡谷 carver。

## 修复计划骨架

- [ ] 明确拆分字段语义：保留 `tiles[].zones` 作为 manifest / console provenance 时，新增或派生 `carver_owner_zones` / `positive_contributing_zones` / `dominant_zone`，避免 `_tile_carver_chain` 继续从 provenance 字段取控制权。
- [ ] 修改 `_blend_tile_layers` 或合成主循环，让“正权重实际改过几何/层”的 zone 单独记录；零权重 AABB 命中可以继续记录到 provenance，但不能成为 carver owner。
- [ ] 修改 `_tile_carver_chain`：只从正贡献 owner 列表选 chain；若没有正贡献 carver owner，则不应用 carver，而不是退回第一个 provenance zone。
- [ ] 保持 manifest / console 兼容：若现有消费者依赖 `tiles[].zones` 显示粗相交 zone，不要在修复中顺手删除零权重 provenance。
- [ ] 加 `tile_6_-7` 或最小 fixture 回归：`blood_valley weight_max == 0` 时不能让 `rift_valley` canyon chain 改写该 tile spans。

## 验证计划

- [ ] `cd worldgen && python3 -m unittest discover -s tests -p 'test_spans_export.py' -v`
- [ ] `cd worldgen && python3 -m unittest discover -s tests -p 'test_span_blend.py' -v`
- [ ] `cd worldgen && python3 -m unittest discover -s tests -p 'test_v3_behavior_baseline.py' -v`
- [ ] `cd worldgen && python3 -m scripts.terrain_gen --backend raster --zone-filter blood_valley,zhanhun_plain`
- [ ] `cd worldgen && python3 -m scripts.terrain_gen.harness.raster_check generated/terrain-gen/rasters`
- [ ] 抽样验证 `tile_6_-7`：`blood_valley` 仍可作为 provenance 出现时，也不得作为 carver owner；spans 差异列应回归为 0 或仅来自真正正权重 owner。

## 对抗复核结论

已完成两轮对抗复核。

第一轮反方质疑指出：`contributing_zones` 未明确定义为“正权重 zone”，粗 AABB 命中可能是合法 provenance；不能单凭零权重 append 判定 bug。

第二轮修正口径后，反方最终裁决通过：问题成立点是同一字段同时做 provenance 和 export control。`_blend_tile_layers` 零权重分支只记录、不改层，但 `_tile_carver_chain` 把记录顺序解释为几何主导 zone；`tile_6_-7` 证据覆盖真实配置、选错 chain、spans 被实际改写，不是元数据误标。
