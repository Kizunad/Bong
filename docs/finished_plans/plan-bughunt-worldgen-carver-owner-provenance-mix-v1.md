# BugHunt: worldgen carver owner 与 provenance 混用

## 摘要

`worldgen` 的 tile `contributing_zones` 同时承担两个语义：

- provenance：这个 tile 与哪些 zone 的扩展 AABB 相交，方便 manifest / console / 调试展示。
- export control：`raster_export._tile_carver_chain` 把列表中第一个带 carver 的 zone 当作该 tile 的几何主导者。

当某个 zone 与 tile 粗 AABB 相交、但真实 boundary weight 全为 0 时，`_blend_tile_layers` 仍把它写入 `contributing_zones`，随后 export 阶段可能让这个零权重 zone 的 canyon / cave / floating island carver 改写整块 tile 的 spans。初版修复把控制字段拆成 tile 级 `carver_owner_zones`，但“任一列正权重”仍会把 carver 放大到整 tile；完整修复必须按列记录最终结构 owner。

## 实际游玩体验影响

玩家在 zone 边界附近会看到不属于当前位置主导地貌的 3D 雕刻外溢。已确认真实蓝图里 `tile_6_-7` 处在 `zhanhun_plain`（战魂平野）北缘，但 `blood_valley` 对该 tile 的真实权重为 0，仍抢到 `rift_valley` 的 canyon carver；更隐蔽的 `tile_4_-7` 只有 37,262/262,144 列血谷正权重，旧 tile 级 chain 却改写 66,298 列，其中 49,093 列血谷权重为 0。

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

以下是修复前 `5121932d` 的只读复现（该历史 HEAD 仍有 `_tile_carver_chain`；最终实现已将其替换为逐列 assignment）：

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

- [x] ✅ 2026-07-11 明确拆分字段语义：保留 `tiles[].zones` 作为 manifest / console provenance；`carver_owner_zones` 只作 owner palette，新增逐列 `carver_owner_index` 记录最终结构 owner。
- [x] ✅ 2026-07-11 修改 `_blend_tile_layers`：与 `blend_spans` 结构交接合同一致，仅 `weight >= 0.5` 的列移交 ownership；后 blend 的主导 zone 覆盖同列，零权重/弱正权重仅保留 provenance。
- [x] ✅ 2026-07-11 用 `_tile_carver_assignments` / `_carved_spans_for_tile` 按互斥 owner mask 应用各自 chain；没有结构 owner 时不雕刻，浮岛 2D fold suppression 也只作用于其 owner 列。
- [x] ✅ 2026-07-11 保持 manifest / console 兼容：`tiles[].zones` 继续输出原 `contributing_zones`。
- [x] ✅ 2026-07-11 加入最小 fixture、真实 `tile_6_-7` 全零 witness 与 `tile_4_-7` 部分权重 witness；锁定多 owner、full export、incremental regen 与 spans 字节合同。

## 验证计划

- [x] ✅ 2026-07-11 `cd worldgen && python3 -m unittest discover -s tests -p 'test_spans_export.py' -v`
- [x] ✅ 2026-07-11 `cd worldgen && python3 -m unittest discover -s tests -p 'test_span_blend.py' -v`
- [x] ✅ 2026-07-11 `cd worldgen && python3 -m unittest discover -s tests -p 'test_v3_behavior_baseline.py' -v`
- [x] ✅ 2026-07-11 `cd worldgen && python3 -m scripts.terrain_gen --backend raster --zone-filter spawn,blood_valley,zhanhun_plain --output-dir /tmp/pr1174-carver-raster-4e9443ee`（同步 main 后 `spawn` 覆盖完整 novice POI 选择窗口）
- [x] ✅ 2026-07-11 `cd worldgen && python3 -m scripts.terrain_gen.harness.raster_check /tmp/pr1174-carver-raster-4e9443ee/rasters`
- [x] ✅ 2026-07-11 `cd worldgen && uvx --from ruff==0.15.20 ruff check scripts/terrain_gen/fields.py scripts/terrain_gen/stitcher.py scripts/terrain_gen/bakers/raster_export.py tests/test_carver_owner_provenance.py`
- [x] ✅ 2026-07-11 `cd worldgen && uvx --from ruff==0.15.20 ruff format --check tests/test_carver_owner_provenance.py`
- [x] ✅ 2026-07-11 抽样验证 `tile_6_-7`：`blood_valley` 保留为 provenance，但不再成为 carver owner；chain 为空且 spans 差异列为 0。
- [x] ✅ 2026-07-11 `cd worldgen && python3 -m unittest discover -s tests -p 'test_carver_owner_provenance.py' -v`（13 项，含 `tile_4_-7`、多 owner、full/regen 字节对拍）
- [x] ✅ 2026-07-11 `cd worldgen && python3 -m unittest discover -s tests -p 'test_carvers.py'`（76 项）
- [x] ✅ 2026-07-11 span/fold/blend/export/stitcher/v3/layer 兼容组 85 项通过；合计本轮 174 项通过。

## 对抗复核结论

已完成多轮对抗复核。

第一轮反方质疑指出：`contributing_zones` 未明确定义为“正权重 zone”，粗 AABB 命中可能是合法 provenance；不能单凭零权重 append 判定 bug。

第二轮修正口径后，反方最终裁决通过：问题成立点是同一字段同时做 provenance 和 export control。`_blend_tile_layers` 零权重分支只记录、不改层，但 `_tile_carver_chain` 把记录顺序解释为几何主导 zone；`tile_6_-7` 证据覆盖真实配置、选错 chain、spans 被实际改写，不是元数据误标。

独立 Ultra 在 `5121932dbbbccda8ddec5adac8fbc182155043a3` 推翻了 tile 级初版 PASS：`tile_4_-7` 只有 37,262 列血谷正权重，tile 级 canyon chain 却改写 66,298 列，其中 49,093 列血谷权重为 0。返工据此把 ownership 降到逐列，并把 `weight >= 0.5` 的结构交接阈值、后写覆盖、多 owner、浮岛 suppression、full/incremental 两条出口一并锁住。最终独立 validator 以 PR 评论中绑定最终 HEAD 的 PASS 为准，避免为“记录 PASS”再改变已经被审的 SHA。

## Finish Evidence

### 落地清单

- `TileFieldBuffer` 用 `carver_owner_zones` 保存 owner palette，并用 tile-area 长度的 `uint16 carver_owner_index` 保存每列最终结构 owner；0 表示 wilderness/no owner。
- `_blend_tile_layers` 继续把粗 AABB 命中的 zone 记录到 `contributing_zones`，但只有 `weight >= 0.5` 的列移交结构 ownership；后 blend 主导 zone 按既有顺序覆盖同列 owner，弱正权重不夺取结构。
- `_tile_carver_assignments` 把 owner index 解析为互斥列 mask；`_carved_spans_for_tile` 在完整 tile 世界坐标上运行每条 chain，只接纳 owner mask 内输出，避免切片重排噪声坐标。
- floating-island 的 2D fold suppression 从 tile 级布尔改为逐列 mask，仅 owner 列抑制旧 slab，邻接 zone 不受影响。
- full export 与 incremental regen 的 manifest 均继续从 `contributing_zones` 输出 `tiles[].zones`，外部格式不变。
- 新增 synthetic 阈值/多 owner/error branch、真实 `tile_6_-7` 全零与 `tile_4_-7` 部分权重回归，并把专属测试纳入 snapshot workflow。

### 验证与跨仓库核验

- 初版 RED：新增回归在修复前出现 2 个 `AttributeError` 与 2 个行为失败，证明 provenance 与 tile owner 混用；Ultra 后续又以真实 `tile_4_-7` 证明 tile 级 owner 仍会把局部正权重放大到整 tile。
- 最终 RED/修复 commits：`c6a88c24` 锁定逐列 owner、部分权重、多 owner、浮岛 suppression、full/regen 写盘合同；`5458745d` 实现逐列结构 ownership。此前 `7be3664f`、`9c1e8c3a`、`bd7225d6`、`80da3626`、`7edc4006`、`4e9443ee` 保留初版修复与 CodeRabbit/RUF001 审计轨迹；`4618c56d`、`a4b3731a` 仅为普通 main merge。
- 本轮聚焦测试：owner 13 项、`test_carvers.py` 76 项、spans 16 项、fold 19 项、blend 9 项、export 17 项、stitcher 10 项、v3 baseline 12 项、layer fixture 2 项，共 174 项通过。
- snapshot 相关验证：anvil export/region/spans/world-spans、span codec/fold/raster-check、layer registry 共 119 项通过；`scripts/preview/test_*.py` 31 项通过。
- Ruff：`fields.py`、`stitcher.py`、`spans_fold.py`、`raster_export.py` 与两份测试 `ruff check` 通过；`test_carver_owner_provenance.py` 通过 `ruff format --check`。
- pipeline：同步 main 后，旧 `blood_valley,zhanhun_plain` 过滤因缺完整 novice POI 选择窗口按新合同正确拒绝；加入覆盖 16 个必需 tile 的 `spawn` 后，生产 CLI raster 生成 52 tiles 成功，`raster_check` 退出码 0，manifest 中 `tile_6_-7` provenance 仍为 `['blood_valley', 'zhanhun_plain']`。
- 本轮开始基线：`a4b3731a` 已合入 `origin/main@37447572`；返工后将在 push 前重新 fetch/merge 最新 `origin/main`，若 HEAD 变化则重跑完整门禁与新 Ultra。
- 跨仓库核验：变更仅落在 `worldgen/` 与 plan 文档；读取 `server/zones.worldview.example.json` 验证 `blood_valley → rift_valley`、`zhanhun_plain → ancient_battlefield` 的真实配置；未改 server/client/agent schema、依赖或 manifest 合同。
- 第一轮 fresh validator：`gpt-5.6-sol-xhigh` 在精确 HEAD `73c5c0324a7e4da5c86ba8eaa12a3701e11ecf9b` 上确认真实 bug、最小正确修复、manifest 兼容、真实 witness 与提交 trailer，结论 `VERDICT: PASS`。
- 第二轮最终 validator：在归档后 HEAD `a2c82ae363946a9ccbe9526aebefb5f0f7837ec0` 确认实现正确，但因真实 witness 未精确断言空 chain/0 spans 差异，以及 Finish Evidence 结构不完整，结论 `VERDICT: FAIL`；前者由 `80da3626` 精确锁定，后者由 `23442aeb` 与本轮证据更新补齐。
- CodeRabbit 返工：有效 inline finding 要求 `base.contributing_zones` 断言补中文失败诊断；`7edc4006` 补入诊断，`4e9443ee` 进一步显式写出期望、原因、实际值，并消除 3 处 RUF001 全角标点告警，比较逻辑未变。
- 被推翻的旧 PASS：无上下文、严格只读的 `gpt-5.6-sol` Ultra 曾在 `127c3e003c2610cc4883f05c39692a6d2cedbc82` 判 PASS；后续更强制的全新 Ultra 在 `5121932dbbbccda8ddec5adac8fbc182155043a3` 以 `tile_4_-7` 的 49,093 个零权重越界差异列给出 `VERDICT: FAIL`，因此旧 PASS 不再作为最终 gate。
- 最终 fresh validator：待本证据 commit、最新 main 同步与完整门禁完成后，对精确最终 HEAD 启动新的 `fork_context:false`、`gpt-5.6-sol` Ultra 只读复核；PASS 绑定 SHA 写入 PR 评论与最终报告。

### 归档顺序与遗留/后续

- 归档 rename commit `2abeed58` 先于 Finish Evidence commit `a2c82ae3` 落下，这是执行顺序偏差；未改写历史，改以独立证据 commit 和本次返工 commit 诚实保留审计轨迹，最终 fresh validator 以最新 HEAD 为准。
- 遗留：当前无已知代码 finding；尚待最新 main 同步、最终 Ultra PASS、push 后 snapshot/CodeRabbit 与 review 降级说明 gate。
- 后续：PR #1174 的 worldgen diff 触发 snapshot；e2e workflow 的 path filter 不含 `worldgen/**`/`docs/**`，因此本 PR 不适用 e2e。最终以 snapshot、CodeRabbit、独立 validator 与 mergeable 状态为合并 gate；`/review` 若仅为 `hlool` 503 则按降级策略忽略，真实 finding 必须返工。
