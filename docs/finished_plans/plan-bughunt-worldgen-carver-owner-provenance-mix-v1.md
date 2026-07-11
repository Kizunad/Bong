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

- [x] ✅ 2026-07-11 明确拆分字段语义：保留 `tiles[].zones` 作为 manifest / console provenance 时，新增 `carver_owner_zones`，避免 `_tile_carver_chain` 继续从 provenance 字段取控制权。
- [x] ✅ 2026-07-11 修改 `_blend_tile_layers`，让“正权重实际改过几何/层”的 zone 单独记录；零权重 AABB 命中继续记录到 provenance，但不能成为 carver owner。
- [x] ✅ 2026-07-11 修改 `_tile_carver_chain`：只从正贡献 owner 列表选 chain；没有正贡献 carver owner 时不应用 carver。
- [x] ✅ 2026-07-11 保持 manifest / console 兼容：`tiles[].zones` 继续输出原 `contributing_zones`。
- [x] ✅ 2026-07-11 加入最小 fixture 与真实 `tile_6_-7` 回归，锁定零权重 `blood_valley` 不得改写 spans。

## 验证计划

- [x] ✅ 2026-07-11 `cd worldgen && python3 -m unittest discover -s tests -p 'test_spans_export.py' -v`
- [x] ✅ 2026-07-11 `cd worldgen && python3 -m unittest discover -s tests -p 'test_span_blend.py' -v`
- [x] ✅ 2026-07-11 `cd worldgen && python3 -m unittest discover -s tests -p 'test_v3_behavior_baseline.py' -v`
- [x] ✅ 2026-07-11 `cd worldgen && python3 -m scripts.terrain_gen --backend raster --zone-filter spawn,blood_valley,zhanhun_plain --output-dir /tmp/pr1174-carver-raster-4e9443ee`（同步 main 后 `spawn` 覆盖完整 novice POI 选择窗口）
- [x] ✅ 2026-07-11 `cd worldgen && python3 -m scripts.terrain_gen.harness.raster_check /tmp/pr1174-carver-raster-4e9443ee/rasters`
- [x] ✅ 2026-07-11 `cd worldgen && uvx --from ruff==0.15.20 ruff check scripts/terrain_gen/fields.py scripts/terrain_gen/stitcher.py scripts/terrain_gen/bakers/raster_export.py tests/test_carver_owner_provenance.py`
- [x] ✅ 2026-07-11 `cd worldgen && uvx --from ruff==0.15.20 ruff format --check tests/test_carver_owner_provenance.py`
- [x] ✅ 2026-07-11 抽样验证 `tile_6_-7`：`blood_valley` 保留为 provenance，但不再成为 carver owner；chain 为空且 spans 差异列为 0。

## 对抗复核结论

已完成两轮对抗复核。

第一轮反方质疑指出：`contributing_zones` 未明确定义为“正权重 zone”，粗 AABB 命中可能是合法 provenance；不能单凭零权重 append 判定 bug。

第二轮修正口径后，反方最终裁决通过：问题成立点是同一字段同时做 provenance 和 export control。`_blend_tile_layers` 零权重分支只记录、不改层，但 `_tile_carver_chain` 把记录顺序解释为几何主导 zone；`tile_6_-7` 证据覆盖真实配置、选错 chain、spans 被实际改写，不是元数据误标。

## Finish Evidence

### 落地清单

- `TileFieldBuffer` 新增独立的 `carver_owner_zones` 运行时字段，使用 `default_factory=list` 保持构造兼容并避免实例间共享。
- `_blend_tile_layers` 继续把粗 AABB 命中的 zone 记录到 `contributing_zones`，但只把真实正权重 blend 的 zone 记录为 carver owner。
- `_tile_carver_chain` 仅遍历 `carver_owner_zones`；没有正贡献 owner 时返回空 chain，不再回退 provenance。
- full export 与 incremental regen 的 manifest 均继续从 `contributing_zones` 输出 `tiles[].zones`，外部格式不变。
- 新增最小 fixture 与真实 `tile_6_-7` 回归，精确锁定空 chain 和所有 spans 差异列为 0。

### 验证与跨仓库核验

- RED：新增回归在修复前出现 2 个 `AttributeError` 与 2 个行为失败，分别证明缺少独立 owner 字段、export 仍选择 provenance-only chain、无 owner 时仍回退 provenance 雕刻。
- 修复 commits：`7be3664f` 锁定失败契约，`9c1e8c3a` 分离 owner/provenance，`bd7225d6` 补齐真实边界 tile 回归，`80da3626` 锁定空 chain/零 spans 差异，`7edc4006` 补断言上下文，`4e9443ee` 以中文诊断和 ASCII 标点闭环 CodeRabbit/RUF001；`4618c56d` 与 `a4b3731a` 均仅为同步 `origin/main` 的普通 merge。
- 聚焦测试：新回归 5 项、`test_spans_export.py` 17 项、`test_span_blend.py` 9 项、`test_stitcher_dispatch.py` 10 项、`test_v3_behavior_baseline.py` 12 项，共 53 项通过。
- snapshot 相关验证：anvil export/region/spans/world-spans、span codec/fold/raster-check、layer registry 共 119 项通过；`scripts/preview/test_*.py` 31 项通过。
- Ruff：4 个 PR Python 文件 `ruff check` 通过；本轮实际修改的 `test_carver_owner_provenance.py` 通过 `ruff format --check`。其余 3 个实现文件全文件 formatter 会重排 main 既有大量无关代码，未把该噪音混入返工。
- pipeline：同步 main 后，旧 `blood_valley,zhanhun_plain` 过滤因缺完整 novice POI 选择窗口按新合同正确拒绝；加入覆盖 16 个必需 tile 的 `spawn` 后，生产 CLI raster 生成 52 tiles 成功，`raster_check` 退出码 0，manifest 中 `tile_6_-7` provenance 仍为 `['blood_valley', 'zhanhun_plain']`。
- 最终 main 同步：`a4b3731a` 合入 `origin/main@37447572`；主线新增 26 个提交与本 PR 的 5 个文件零交集，合并前后目标文件 blob 对拍一致。合并后重跑 carver 5 项、`test_spans_export.py` 17 项、snapshot validator 31 项及 Ruff lint/当前测试文件格式检查，均通过。
- 跨仓库核验：变更仅落在 `worldgen/` 与 plan 文档；读取 `server/zones.worldview.example.json` 验证 `blood_valley → rift_valley`、`zhanhun_plain → ancient_battlefield` 的真实配置；未改 server/client/agent schema、依赖或 manifest 合同。
- 第一轮 fresh validator：`gpt-5.6-sol-xhigh` 在精确 HEAD `73c5c0324a7e4da5c86ba8eaa12a3701e11ecf9b` 上确认真实 bug、最小正确修复、manifest 兼容、真实 witness 与提交 trailer，结论 `VERDICT: PASS`。
- 第二轮最终 validator：在归档后 HEAD `a2c82ae363946a9ccbe9526aebefb5f0f7837ec0` 确认实现正确，但因真实 witness 未精确断言空 chain/0 spans 差异，以及 Finish Evidence 结构不完整，结论 `VERDICT: FAIL`；前者由 `80da3626` 精确锁定，后者由 `23442aeb` 与本轮证据更新补齐。
- CodeRabbit 返工：有效 inline finding 要求 `base.contributing_zones` 断言补中文失败诊断；`7edc4006` 补入诊断，`4e9443ee` 进一步显式写出期望、原因、实际值，并消除 3 处 RUF001 全角标点告警，比较逻辑未变。
- 最终 fresh validator：无上下文、严格只读的 `gpt-5.6-sol` Ultra 在精确 HEAD `127c3e003c2610cc4883f05c39692a6d2cedbc82` 对拍工作树与 `origin/main`，复核 owner/provenance、真实 witness、CodeRabbit/RUF001、manifest、测试与 plan 证据，未发现 actionable finding，结论 `VERDICT: PASS`。

### 归档顺序与遗留/后续

- 归档 rename commit `2abeed58` 先于 Finish Evidence commit `a2c82ae3` 落下，这是执行顺序偏差；未改写历史，改以独立证据 commit 和本次返工 commit 诚实保留审计轨迹，最终 fresh validator 以最新 HEAD 为准。
- 遗留：无已知代码、测试或跨栈阻塞；验证 raster 写入 `/tmp/pr1174-carver-raster-4e9443ee`，不纳入版本控制。
- 后续：PR #1174 的 worldgen diff 触发 snapshot；e2e workflow 的 path filter 不含 `worldgen/**`/`docs/**`，因此本 PR 不适用 e2e。最终以 snapshot、CodeRabbit、独立 validator 与 mergeable 状态为合并 gate；`/review` 若仅为 `hlool` 503 则按降级策略忽略，真实 finding 必须返工。
