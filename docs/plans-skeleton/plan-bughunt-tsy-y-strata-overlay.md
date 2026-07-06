# plan-bughunt-tsy-y-strata-overlay

> Skeleton-only bughunt plan. 一句话主题：TSY family 的 shallow / mid / deep 三层共享同一 XZ，但 worldgen stitcher 只做 2D overlay，最终 raster 被最后的 deep 层覆盖成单层地形；入口、出口、容器、NPC 仍按浅/中/深 Y 坐标生成，导致正常 TSY 体验落点和内容悬空/坠落。

## Bug 摘要

TSY 设计契约要求同一 family 在同一 XZ patch 内按 Y 分层：shallow Y∈[40,120]、mid Y∈[0,40]、deep Y∈[-40,0]。`docs/finished_plans/plan-tsy-worldgen-v1.md:70` 和 `:996-1002` 都明确把三层共享 XZ + Y 分层定为收敛方案。

但当前 `worldgen/scripts/terrain_gen/stitcher.py:746-756` 对 tile 逐个遍历所有相交 zone，并把每个 zone 的 2D overlay 依次 blend 到同一个 `TileFieldBuffer`。`height` 是单个 2D layer，`tsy_depth_tier` 也是单个 swap layer；同一 XZ 的 shallow -> mid -> deep 依次覆盖后，最终只剩 deep 的 height / spans / depth_tier。

这不是 #992。#992 是启动脚本没有传 `BONG_TSY_RASTER_PATH`，导致 TSY provider 为空；本 bug 是 provider 存在、TSY raster 成功加载后，内容本身已被烘焙成错误的 deep-only 地形。

## 对实际游玩体验的影响

玩家从主世界裂缝进入 TSY 时，`target_family_pos_xyz` 指向 `_shallow` 中心，例如 `server/zones.worldview.example.json:391` 的 `550,100,550` 和 `:442` 的 `250,100,250`。`server/src/world/tsy_poi_consumer.rs:126-142` 会把这个坐标直接塞进 `RiftPortal::entry` 的 TSY 目标锚点。

实际 raster 在这些 XZ 的可站立 surface 已经落到 deep 层：`zongmen_01` 中心目标是 Y=100，但真实 surface_y=-38；`daneng_01` 中心目标是 Y=100，但真实 surface_y=-33。玩家进入后不是站在浅层入口带，而是被传到浅层空中，随后掉向 deep 地形；浅层 PVP 死地、中层主废墟、深层核心的玩法分层被压扁成一层。

同时 `server/zones.tsy.json:121-127`、`:154-160`、`:187-192` 等 POI 仍按 shallow/mid/deep Y 坐标放置容器、NPC、relic core；`server/src/world/tsy_dev_command.rs:379-416` 也按 `TsyDepth::Shallow/Mid/Deep` 的 zone AABB 随机撒容器，不查询 raster surface。结果是浅层/中层内容会生成在空中或与实际地形脱节，玩家看见的是断裂的坍缩渊，而不是三层探索空间。

## 证据定位

- 设计契约：`docs/finished_plans/plan-tsy-worldgen-v1.md:70` 要求 shallow/mid/deep 共享 XZ、Y 轴分层；`:95` 写明 entry 的 `target_family_pos_xyz` 指向 TSY `_shallow` 中心；`:996-1002` 明确默认三层 Y 区间；`:851-857` 给出三层不同玩法定位。
- 蓝图数据：`server/zones.tsy.json:16-94` 的 `tsy_lingxu_01_*` 三层同 XZ `[0,100]`，Y 分别 `[40,120]` / `[0,40]` / `[-40,0]`；`server/zones.tsy.json:98-188` 的 `tsy_zongmen_01_*` 同理，且 shallow exit / loot / NPC 都在 Y=80/100。
- 覆盖根因：`worldgen/scripts/terrain_gen/stitcher.py:300-304` 对单个 `height` layer 做 2D blend；`:390-401` 对 swap layer 做硬替换；`:746-756` 在同一 tile 上遍历所有相交 zone，后来的 deep overlay 覆盖前面的 shallow/mid。
- 校验盲点：`worldgen/scripts/terrain_gen/harness/raster_check.py:304-318` 的 TSY 三层齐全检查只看 `tile.zones` 名字后缀是否含 `_shallow/_mid/_deep`，不验证 `tsy_depth_tier.bin` 是否实际包含 1/2/3，也不验证目标 POI 附近的 spans/surface_y。
- 运行时消费：`server/src/world/terrain/mod.rs:572-609` 在 `BONG_TSY_RASTER_PATH` 存在时加载 TSY provider；`:656-690` chunk 生成按维度取 provider；`server/src/world/terrain/raster.rs:945-1013` 每个 XZ 只采样一个 `ColumnSample`；`server/src/world/terrain/column.rs:105-114` 把 `sample.surface_y()` 作为唯一物理顶面。

## 真实 raster 取证

取证方式：在 `worldgen/` 下直接调用 worldgen Python 模块导出到 `/tmp/bong-tsy-raster-proof/rasters`，`tile_size=128`，`layer_whitelist=None`，只选中四个 shallow zone。该路径不写仓库文件，也不生成/修改 NBT 或资源文件。

导出结果摘要：

| 目标点 | manifest tile.zones | target_y | sample_depth | nonzero depth unique | spans | surface_y |
|---|---|---:|---:|---|---|---:|
| `lingxu` `(50,50)` | shallow/mid/deep | 100 | 3 | `[3]` | `[[-64,-58]]` | -58 |
| `zongmen` `(250,250)` | shallow/mid/deep | 100 | 3 | `[3]` | `[[-64,-38]]` | -38 |
| `daneng` `(550,550)` | shallow/mid/deep | 100 | 3 | `[3]` | `[[-64,-33]]` | -33 |
| `gaoshou` `(1050,550)` | shallow/mid/deep | 100 | 3 | `[3]` | `[[-64,-23]]` | -23 |

关键点：manifest 的 `tile.zones` 仍能让 `raster_check.py` 误以为三层齐全，但实际 `tsy_depth_tier.bin` 非零值只有 `3`，且物理 `spans` 也已经是 deep 表面，不只是语义标签错误。

## 触发路径

1. 正常 worldgen 生成 TSY raster，`server/zones.tsy.json` 中同 family 的 shallow/mid/deep 三层共享 XZ。
2. `synthesize_fields()` 选中该 tile 后仍融合所有相交 zone；同 XZ 的三层按 blueprint 顺序依次写入同一个 2D tile。
3. raster 导出把最终 2D `height` 折叠成 `spans_count.bin` / `spans.bin`，浅/中层没有独立 slot 保留下来。
4. 服务器带 `BONG_TSY_RASTER_PATH` 启动，TSY provider 成功加载；玩家踩主世界 `rift_portal`，`target_family_pos_xyz` 把玩家送到 shallow Y=100。
5. 目标 XZ 的 chunk 按 deep-only raster 生成，Y=100 附近没有浅层地面；浅/中层 POI、容器、NPC 仍按旧 Y 坐标生成，体验断裂。

## 反方审查记录

Round 1 反方结论：通过。反方未找到 #971/#986/#992 或其他开放 PR 覆盖；确认不是 ZoneRegistry 三维 AABB 逻辑问题，而是 TSY raster 的物理地形/semantic depth 在同 XZ 三层 overlay 后被最后的 deep 层覆盖。反方同时指出 `spans_fold` 无法从已经覆盖后的 2D layer 恢复三层。

Round 2 反方结论：通过。反方重点攻击了 `zone_filter` / `tile_size=128` 假象、自动落地/平台兜底、semantic-only 误报、与 #992 重复等方向，均未推翻。裁决要点：

- `zone_filter` 只筛要生成哪些 tile，不筛哪些 zone 参与 blend；同一 XZ 的 overlay 顺序不因 tile size 改变。
- `apply_dimension_transfers` 直接设置目标坐标，不做 `query_surface` snap 或平台生成；即使玩家物理坠落，也不是正确的浅层入口体验。
- `tsy_depth_tier=[3]` 之外，`height` 与 `spans` 也已是 deep，属于物理地形错误。
- #992 修复 provider 空缺后，本 bug 会更稳定暴露。

## Skeleton Fix Plan

P0 先补回归证明，不直接改方案：

- 新增 worldgen 级测试：对 `server/zones.tsy.json` 的每个 family，在 shallow/mid/deep 代表点分别断言 `tsy_depth_tier` 可观测到 1/2/3，并断言对应 POI/portal 目标附近的 `surface_y` 落在该层 Y 区间内。
- 扩 `raster_check.py` 或新增 cross-manifest check：不能只看 `tile.zones` 后缀，必须读取 `tsy_depth_tier.bin` 与 `spans.bin`，验证 `_shallow/_mid/_deep` 有真实物理层。

P1 决策修复路线，三选一，需在 fix PR 前收敛：

- 方案 A：扩 TSY raster 表示为可表达同 XZ 多个 Y layer 的多层/3D 结构，让 shallow/mid/deep 都能保留独立 spans 与语义层。
- 方案 B：调整 TSY 蓝图布局，让三层不再共享 XZ，而是拆成不同 XZ patch，通过 portal/路径连接，适配现有 2D raster。
- 方案 C：承认当前 2D deep-only 地形，重写所有 entry/exit/POI/container/NPC 落点到真实 surface。此方案与已收敛的 Y 分层契约冲突，只有重新决策世界观/玩法布局时才可选。

P2 运行态联调：

- 带 `BONG_TSY_RASTER_PATH` 启服后，从主世界 entry 进入 `zongmen_01` / `daneng_01`，玩家落点在 shallow 层可站立，不会坠落到 deep。
- `/tsy_spawn tsy_lingxu_01` 或等价自动 spawn 后，shallow/mid/deep 容器和 NPC 均贴合对应层 surface，不悬空、不埋入 deep 单层地形。

## 验收测试计划

- `cd worldgen && python3 -m pytest tests/test_terrain_gen_cli.py tests/test_stitcher_dispatch.py tests/test_raster_check_spans.py -q` 级别的快速单元回归，新增 TSY Y 分层 regression 测试。
- `cd worldgen && python3 -m scripts.terrain_gen --tsy-blueprint ../server/zones.tsy.json --tsy-output-dir <tmp> ...` 或等价模块化导出，随后 `python3 scripts/terrain_gen/harness/raster_check.py <tsy-rasters>`，要求四个 family 的 shallow/mid/deep 都有真实 depth 与 surface。
- 仓库根 `BONG_TSY_RASTER_PATH=<tmp>/rasters/manifest.json bash scripts/smoke-test-e2e.sh`，至少覆盖 entry 传送、TSY chunk 生成、浅层出口/容器/NPC 不悬空。

## 风险

- 修复可能触碰 worldgen raster schema、Rust `TerrainProvider::sample`、chunk column fill、POI/portal 落点等跨边界契约，不能当作单点数值调整。
- 若选择方案 A，多层 Y raster 会扩大 manifest / mmap / column fill 复杂度，需要迁移现有 `spans` 单 surface 假设。
- 若选择方案 B 或 C，会背离 `plan-tsy-worldgen-v1` 已收敛的 Y 分层契约，需要明确更新设计文档，不应在修复 PR 里悄悄改玩法。
- 目前尚未做游戏内截图或 block probe；已有真实 raster 证据足以证明根因，但最终验收仍应补一条运行态落点证据。
