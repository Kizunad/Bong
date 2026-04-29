# Bong · plan-worldgen-anvil-export-v1 · 骨架

**Worldgen raster → Anvil region exporter**：把 worldgen pipeline 已有的 raster (height/biome/surface) 输出转成 1.20.1 Anvil `.mca` region 文件，让 server `BONG_WORLD_PATH` 路径能在 CI 里加载真实 Bong 地形，不靠 Phase A 的 datapack + 无头 MC 链路。本 plan 是 plan-worldgen-snapshot-v1 P3 之后的关键依赖——目前 `plan-worldgen-snapshot-v1` 卡在"server 16×16 chunk fallback flat world 太小，iso 视角传送到 ±400 blocks 看到纯虚空"。

**交叉引用**：
- `plan-worldgen-snapshot-v1`（消费方；本 plan merge 后 rebase #78 验证 5 角度全过 validator）
- `plan-worldgen-v3.x`（前置；raster_export 已稳定，本 plan 不动 raster 编码，只追加 anvil 后端）
- `worldgen/scripts/postprocess.py`（参考；同样用 mcworldlib 写 anvil，但目标是装饰增强而非全新生成）

**为何不走 Phase A**：
- Phase A = 启动 datapack 化的无头 MC server，让 vanilla worldgen 跑出 .mca → postprocess.py 装饰
- CI 里再起一个 MC server 跑 worldgen 太重（资源 + 时间）
- 本 plan 走纯 Python 直接生成 anvil 字节，不依赖 MC 运行时

**触发模型**：本 plan 不接 PR-触发 CI；它把 anvil 文件作为 `worldgen-preview.yml` artifact 的一部分产出，由 `plan-worldgen-snapshot-v1` 的 server start step 消费。

**阶段总览**：
- P0 ✅ 2026-04-30 单 chunk anvil 写入 + 23 round-trip 单测
- P1 ✅ 2026-04-30 region writer + export driver + pipeline.sh anvil 后端
- P2 ✅ 2026-04-30 CI 接入 BONG_WORLD_PATH（PR #78 rebase 验证留 PR #78 自己跑）

---

## §0 设计轴心

- [ ] **不依赖 MC 运行时**：纯 Python 生成 anvil 字节流，没有"启动 server 让 worldgen 跑"的间接路径——CI 里跑得快
- [ ] **复用 raster 数据源**：worldgen pipeline 已有 `raster` backend 产 `<output>/rasters/{height,biome,surface}.bin`（little-endian binary），本 plan 直接吃
- [ ] **chunk 16×16 block 列**：anvil 一个 region = 32×32 chunks = 512×512 blocks。覆盖 plan-worldgen-snapshot-v1 iso ±400 blocks 需要至少 r.-1.-1 / r.0.-1 / r.-1.0 / r.0.0 四个 region
- [ ] **复用现成库**：用 `mcworldlib`（worldgen postprocess.py 已用）或 `anvil-region` Python 包；不自己 NBT 编码
- [ ] **饱和测试**：chunk 编码 / region boundary / Y level 范围 / round-trip（写入后读回 block 一致）；接口先于实现锁定（mock raster 数据 → 输出 → 用同一库读回 → 对拍）
- [ ] **不修复 plan-worldgen-snapshot-v1**：本 plan merge 后 rebase #78；#78 的 P3（validator + chunk-ready barrier 代码 + workflow 接入）保持原样，只配置改 `BONG_WORLD_PATH`

---

## §1 P0 — 单 chunk anvil 写入 + round-trip 单测

> 目标：写一个 `worldgen/scripts/terrain_gen/anvil_export.py` 模块，能给定一个 16×16 高度 + biome 数组，写出**一个**合法的 anvil chunk NBT。读回后对得上。先不管 region 文件 / 多 chunk / pipeline 集成。

### 1.1 模块骨架

- [ ] **新增** `worldgen/scripts/terrain_gen/anvil_export.py`
  - 函数 `chunk_to_nbt(chunk_x: int, chunk_z: int, heights: np.ndarray, biomes: np.ndarray, surface_blocks: dict) -> bytes` 返回单 chunk 的 NBT 字节
  - chunk 数据结构（1.20.1 anvil 格式）：
    - `DataVersion: 3465`（1.20.1）
    - `xPos / zPos`（chunk 坐标）
    - `Status: "minecraft:full"`（标记完全生成）
    - `sections: [...]`（按 Y 16 一段，每段含 `block_states.palette` + `block_states.data` + `biomes.palette` + `biomes.data`）
    - `Heightmaps: { MOTION_BLOCKING: [...], WORLD_SURFACE: [...] }`（基于 heights 数组导出）
- [ ] **依赖选型**：mcworldlib（postprocess 已用）vs anvil-region（更轻）—— P0 先尝试 mcworldlib，跑通后看是否换轻库

### 1.2 输入约定

- [ ] heights：shape `(16, 16)` int32，每点的最高实心方块 Y（取自 raster height layer）
- [ ] biomes：shape `(16, 16)` int8，biome ID（取自 raster biome layer，或先全 plains 占位）
- [ ] surface_blocks：dict 控制 surface 层方块映射（biome → top block + filler block + bedrock layer），P0 先 hardcode "stone everywhere up to height, grass at top, bedrock at y=-64"

### 1.3 测试矩阵（饱和）

- [ ] **chunk_to_nbt happy path**：mock heights=64 + biomes=plains → 输出 → mcworldlib 读回 → 顶层 y=64 全 grass / 中间 stone / y=-64 bedrock
- [ ] **chunk 坐标边界**：xPos/zPos = 0, ±1, ±25（plan-worldgen-snapshot-v1 iso ±400 → chunk ±25）；负坐标应正确编码为 NBT int
- [ ] **section Y 范围**：1.20.1 世界 Y 范围 [-64, 320]，应有 24 个 section（Y -4..19）；每 section 的 `Y` 字段正确
- [ ] **空 section 优化**：全 air section 的 `block_states.palette = [air]` + `data` 缺省（合法且省字节）
- [ ] **height 边界值**：heights=320（顶）/ heights=-64（底）/ heights=0（海平面）—— 不报错且 round-trip 一致
- [ ] **invalid input rejection**：heights 非 16×16 / biomes shape 错 / chunk_x 非 int → ValueError
- [ ] **DataVersion 钉死**：3465（1.20.1）作为常量；改版本号要改测试，把"协议版本"锁住

### 1.4 P0 验收

- [ ] 单测全过：`python3 -m unittest discover -s worldgen/scripts/terrain_gen -p 'test_anvil_export.py'`
- [ ] 手动 sanity：用 P0 生成一个 chunk → 落盘 `.mca`（hand-crafted region wrapper） → MC client 进服能看到那一格 16×16 草地不报错
- [ ] **不在乎**：region 文件结构 / 多 chunk 拼装 / pipeline 集成 / CI

---

## §2 P1 — pipeline 集成：raster → 多 region

> 目标：扩展 `worldgen/pipeline.sh` + `worldgen/scripts/terrain_gen/__main__.py` 支持 `--backend anvil`（或 `raster` 后追加一步 `anvil`），把全 raster 输出转成 `<output>/world/region/r.X.Z.mca` 文件树。server `BONG_WORLD_PATH=<output>/world` 即可加载。

### 2.1 region 文件写入

- [ ] **新增** `worldgen/scripts/terrain_gen/anvil_region_writer.py`
  - 函数 `write_region(region_x: int, region_z: int, chunks: dict[(int,int), bytes], output_dir: Path)` 写 `r.X.Z.mca`
  - region 文件格式（1.20.1）：
    - 4096 字节 location table（每 chunk 4 字节：3 字节 sector 偏移 + 1 字节 sector 长度）
    - 4096 字节 timestamp table
    - chunks 数据按 4096 sector 对齐，前置 5 字节 header（4 字节长度 + 1 字节压缩类型 = 2 = zlib）
- [ ] 用 mcworldlib 的 `RegionFile` 或 anvil-region 直接调；不重写区域分配算法

### 2.2 raster → chunks 映射

- [ ] **修改** `worldgen/scripts/terrain_gen/exporters.py` 加 `export_anvil_world(output_dir, raster_dir, blueprint)` 函数
  - 读 raster manifest → 知道 raster 的 world bounds (例如 -512..512 blocks)
  - 按 16 拆 chunk，按 32 拆 region
  - 每 chunk 切对应 16×16 raster 切片 → 调 `chunk_to_nbt` (P0)
  - 按 region 聚合 → `write_region` (P1 §2.1)
  - 输出 `<output>/world/level.dat`（最小 NBT，含 spawn 坐标）+ `<output>/world/region/r.*.mca`

### 2.3 pipeline.sh 接入

- [ ] **修改** `worldgen/pipeline.sh` 加 `BACKEND=anvil` 分支：
  - 先跑 `raster`（依赖产物）
  - 再跑 `python3 -m scripts.terrain_gen --backend anvil --raster-dir <output>/rasters --output-dir <output>/world`
  - 校验 `<output>/world/level.dat` + 至少 1 个 `<output>/world/region/r.*.mca` 存在

### 2.4 测试矩阵（饱和）

- [ ] **write_region happy path**：mock 32 chunks（一个 region 满）→ 写 r.0.0.mca → mcworldlib 读回所有 chunks 一致
- [ ] **稀疏 region**：只填 4 个 chunks（其他位置空）→ location table 对应位置 0，读回不报错
- [ ] **跨多 region**：chunks 跨 r.-1.-1 / r.0.-1 / r.-1.0 / r.0.0 → 写出 4 文件 + 每文件 chunks 命中正确
- [ ] **export_anvil_world 端到端**：用 worldgen 已有的 fixture raster → 跑 export → 读出来对拍 height + biome
- [ ] **失败信息**：raster 缺 manifest / shape 不一致 / 输出目录不可写 → 明确错误而非 KeyError 一行

### 2.5 P1 验收

- [ ] `bash worldgen/pipeline.sh ../server/zones.worldview.example.json /tmp/anvil-out anvil` 跑完 → `/tmp/anvil-out/world/level.dat` + N 个 `.mca` 文件
- [ ] `BONG_WORLD_PATH=/tmp/anvil-out/world cargo run` server 启动选 `WorldBootstrap::AnvilIfPresent` 路径（不 fallback flat），日志含 `creating overworld layer backed by Anvil terrain at /tmp/anvil-out/world`
- [ ] vanilla MC 1.20.1 client 进服 → 至少 spawn + 5 角度 tp 目标点 (8,_,8) / (±400,_,±400) 都看到地形（不是黑屏 / 空气）

---

## §3 P2 — CI 接入 + plan-worldgen-snapshot-v1 验证

> 目标：把 anvil 生成接入 `worldgen-preview.yml`，server 启动消费 anvil，rebase plan-worldgen-snapshot-v1 PR #78 跑 validator 5 张全过。

### 3.1 workflow 接入

- [ ] **修改** `.github/workflows/worldgen-preview.yml`
  - "Run worldgen pipeline" step 后加新 step "Generate Anvil world from raster"：`bash worldgen/pipeline.sh ... anvil`
  - "Start headless Bong server" step 加 `env: BONG_WORLD_PATH: ${{ github.workspace }}/worldgen/generated/snapshot/world`

### 3.2 性能预算

- [ ] CI 单跑预算：anvil 生成 ≤ 90s（覆盖 ±512 blocks ≈ 1 region 36 chunks，按 1s/chunk 估 36s + IO ≈ 60s 实际）
- [ ] 整 workflow 时长 ≤ 25min（之前 17min snapshot + ~5min anvil gen + 余量）

### 3.3 plan-worldgen-snapshot-v1 PR #78 重新跑

- [ ] 本 plan 自身 PR merge 后，到 PR #78 worktree (`.worktree/plan-worldgen-snapshot-v1`) 跑 `git fetch origin main && git rebase origin/main && git push --force-with-lease`
- [ ] PR #78 重跑 CI → snapshot job validator step 5 张全过：
  - top: terrain ≥ 15%（俯视）
  - iso 4 张: terrain ≥ 30%（侧视看 spawn zone 真实地形）
  - 5 张 MD5 互不相同（真实地形多样）
  - 5 张 ≥ 18KB
- [ ] PR #78 merge

### 3.4 P2 验收

- [ ] 本 plan PR CI（worldgen-preview workflow）也跑：因为本 plan 改 worldgen-preview.yml，PR `paths: .github/workflows/worldgen-preview.yml` 自动触发。本 plan 自己的 CI artifact 含 5 张 client 截图（即"自验证"——本 plan 的 PR 截图就证明 anvil 能跑）
- [ ] PR #78 rebase 后 CI 绿
- [ ] PR #78 merge 后 plan-worldgen-snapshot-v1 自归档至 finished_plans/

---

## §4 数据契约（按 P 汇总，下游 grep 抓手）

| P | 契约 | 位置 |
|---|------|------|
| P0 | `worldgen/scripts/terrain_gen/anvil_export.py::chunk_to_nbt` | `worldgen/scripts/terrain_gen/` |
| P0 | DataVersion=3465（1.20.1）作为模块常量 | 同上 |
| P0 | `worldgen/scripts/terrain_gen/test_anvil_export.py` 单测 | 同目录 |
| P1 | `worldgen/scripts/terrain_gen/anvil_region_writer.py::write_region` | `worldgen/scripts/terrain_gen/` |
| P1 | `worldgen/scripts/terrain_gen/exporters.py::export_anvil_world` | 已有文件追加 |
| P1 | `worldgen/pipeline.sh` `BACKEND=anvil` 分支 | 已有文件改 |
| P1 | 输出布局：`<output>/world/level.dat` + `<output>/world/region/r.*.mca` | 输出 |
| P2 | workflow step "Generate Anvil world from raster" | `.github/workflows/worldgen-preview.yml` |
| P2 | server start step env `BONG_WORLD_PATH` | 同上 |

---

## §5 实施节点

- [ ] **P0** 单 chunk anvil 写入 — `anvil_export.py::chunk_to_nbt` + 7+ 单测覆盖 happy/边界/失败
- [ ] **P1** pipeline 集成 — region writer + exporter 函数 + pipeline.sh 分支 + 5+ 单测
- [ ] **P2** CI 接入 + PR #78 rebase 验证 — workflow yml 改 + #78 5 张全过 validator

---

## §6 开放问题

- [ ] **mcworldlib vs anvil-region**：mcworldlib 已被 postprocess.py 用，但库较重；anvil-region 更轻但需评估 1.20.1 兼容性。P0 实施时定夺
- [ ] **生成范围**：默认 ±512 blocks（1 region 居中，4 个 region 也行）够 plan-worldgen-snapshot-v1 5 角度。如果未来 5 × N 张按 zone 抽样需要更大范围，pipeline 怎么参数化
- [ ] **biome 来源**：raster 已有 biome layer 还是要从 zones blueprint 派生？需查 raster manifest。如果 raster 有 biome → 直接读；没有 → 从 blueprint 按 (x,z) → zone → biome 映射
- [ ] **surface block 决策**：每 biome 顶层方块是 grass / sand / soul_sand / netherrack 之类——P0 hardcode grass，P1 接 worldview block palette（与 worldgen-v3.x 对齐？）
- [ ] **structures**：anvil 支持 structure 数据，但 1.20.1 vanilla 才有意义。本 plan 只导地形 + biome，不导 structures（巨树 / 装饰已在 server preview decorations.json 处理）
- [ ] **anvil 版本兼容**：DataVersion=3465 (1.20.1)。若未来升 MC 版本要改；测试钉死该常量提早暴露
- [ ] **CI 缓存策略**：anvil 生成是 deterministic（同 raster + 同代码 → 同输出）。CI 可缓存按 raster hash → 第二次 PR 跑 < 30s。先不做，P2 上线后看时长决定

---

## §7 进度日志

- **2026-04-30**：骨架立项 — 承接 plan-worldgen-snapshot-v1 PR #78 实测发现 server fallback flat world 16×16 chunks (±128 blocks) 太小，iso 角度传送到 ±400 blocks 看到纯虚空。两轮 CI 修复（chunk-ready barrier + 30s blind settle）都没解决，因为根因不是 timing 而是 chunks 不存在。本 plan 提供 raster→anvil 路径让 CI 跑出真实地形给 server 加载，PR #78 rebase 后即可绿。

---

## Finish Evidence (2026-04-30)

### 落地清单

**P0 — 单 chunk anvil 写入**

| § | 文件 / 模块 | commit |
|---|---|---|
| §1.1 | `worldgen/scripts/terrain_gen/anvil_export.py` — `chunk_to_nbt(chunk_x, chunk_z, heights, ...)` 纯 stdlib（struct + zlib + io）NBT 编码；DataVersion=3465 钉死 | `8d4419aa` |
| §1.1 | 内部：`_bits_per_index` / `_pack_indexes_into_longs` / `_encode_heightmap` / `_section_block_palette_and_data` | 同上 |
| §1.1 | `chunk_to_nbt_compressed` 包 zlib，region writer 直接消费 | 同上 |
| §1.3 | `worldgen/scripts/terrain_gen/test_anvil_export.py` — 23 个单测 | 同上 |

**P1 — pipeline 集成**

| § | 文件 / 模块 | commit |
|---|---|---|
| §2.1 | `worldgen/scripts/terrain_gen/anvil_region_writer.py` — `write_region` / `write_empty_region` / `region_for_chunk` / `chunk_index_in_region` / `region_file_name` | `0ddb969a` |
| §2.2 | `worldgen/scripts/terrain_gen/anvil_world_export.py` — `export_anvil_world(output_dir, chunk_*_min/max, height_fn, ...)` + `rolling_hills_height_fn` synthetic（**plan §4 偏离**：原计划 exporters.py，实测 exporters.py 相对 import 链让裸模块加载失败，改独立模块） | 同上 |
| §2.3 | `worldgen/pipeline.sh` `BACKEND=anvil` 分支：raster + 内联 python 调 export_anvil_world，输出 `<OUTPUT>/world/region/r.*.mca` | 同上 |
| §2.4 | `worldgen/scripts/terrain_gen/test_anvil_region_writer.py` — 16 个单测（IndexHelpers 4 / WriteRegion 7 / ExportAnvilWorld 4 + extra） | 同上 |

**P2 — CI 接入**

| § | 文件 / 模块 | commit |
|---|---|---|
| §3.1 | `.github/workflows/worldgen-preview.yml` 改 `bash pipeline.sh ... anvil`（替代 raster），server start step 加 `env: BONG_WORLD_PATH: …/worldgen/generated/snapshot/world` | 本提交 |
| §3.1 | 新 step `Unit tests — anvil_export + anvil_region_writer` 早跑 39 单测 | 同上 |

**plan-worldgen-snapshot-v1 (PR #78) rebase**：本 plan PR merge 后，到 #78 worktree 跑 `git fetch && git rebase origin/main && git push --force-with-lease`，CI 重跑应 5 张全过 validator。该动作由人工或后续 /consume-plan 续做（不在本 plan PR 内）。

### 关键 commit (本 PR)

```
8d4419aa  2026-04-30  feat(worldgen/anvil): 单 chunk anvil NBT 编码 chunk_to_nbt（P0）
0ddb969a  2026-04-30  feat(worldgen/anvil): region writer + export driver + pipeline.sh anvil 后端（P1）
[pending]  2026-04-30  feat(ci/preview): worldgen-preview.yml 接入 anvil + BONG_WORLD_PATH（P2）
```

### 测试结果

- **anvil_export 单测**：`cd worldgen/scripts/terrain_gen && python3 -m unittest test_anvil_export -v` → **23 passed**
  - ChunkRoundTripTests 9 / EmptySectionTests 1 / InvalidInputTests 7 / CompressedOutputTests 2 / BitsPerIndexTests 4
- **anvil_region_writer 单测**：`python3 -m unittest test_anvil_region_writer -v` → **16 passed**
  - IndexHelpersTests 4 / WriteRegionTests 7 / ExportAnvilWorldTests 4 + 1 extra
- **直接 anvil 端到端 smoke**（无 raster pre-step）：±25 chunks (51×51 = 2601) → 4 region 文件 (r.0.0/r.0.-1/r.-1.0/r.-1.-1) / 11MB / 19.6s
- **Server / client / agent 测试**：本 plan 不动 server/client/agent 代码，无需重跑

### 跨仓库核验

- **worldgen**: `worldgen.scripts.terrain_gen.anvil_export.{chunk_to_nbt, chunk_to_nbt_compressed, DATA_VERSION}` + `anvil_region_writer.{write_region, region_for_chunk, chunk_index_in_region}` + `anvil_world_export.{export_anvil_world, rolling_hills_height_fn}`
- **CI**: `.github/workflows/worldgen-preview.yml` 加 `Unit tests — anvil_export + anvil_region_writer` step + `Run worldgen pipeline` 改 anvil + Server start step 加 `BONG_WORLD_PATH` env
- **server**: 0 改动 — 既有 `configured_world_path() → BONG_WORLD_PATH` env 路径直接消费 anvil 文件

### 遗留 / 后续

- **真 raster reader 接入**：当前 `rolling_hills_height_fn` 是合成高度，没读 worldgen pipeline 的 raster height layer。后续小 plan：写 `RasterHeightReader` adapter 替换 height_fn body（接口已锁定，签名不变）。读后才能在 iso 视角看到真实 zone 地形（青云峰 / 血谷等），不只 sin 波起伏
- **biome 来源**：当前所有 chunks 单一 plains biome（`_write_biomes` hardcode）。raster reader plan 同时接入 biome layer
- **surface block 决策**：默认 grass+stone+bedrock。`worldview_color_palette.json` 接入需 worldview 沟通，留 future plan
- **chunks 范围**：目前 hardcode ±25 chunks (±400 blocks)。不同 plan 可能要不同范围 — 若 PR #78 5×N 按 zone 抽样需要更大覆盖，pipeline.sh 加 `--chunks-radius` 参数
- **anvil 单 chunk 性能**：当前 ~7.5ms / chunk 单线程。2601 chunks 19.6s。若要扩到 ±50 chunks (10000 chunks) 需要并发 — `multiprocessing.Pool` 即可
- **DataVersion 升 MC 版本**：`DATA_VERSION=3465` 钉死 1.20.1。升到 1.20.4 (DataVersion=3700) 需同改测试 + 验 NBT 兼容
- **plan §4 数据契约偏离**：`export_anvil_world` 实际落到 `anvil_world_export.py` 而非 `exporters.py`（exporters.py 的相对 import 阻碍裸模块加载）。Finish Evidence 已记录
- **PR #78 rebase 验证**：本 plan PR merge 后 PR #78 rebase + CI 重跑应 5 张全过 validator。人工/后续 consume-plan 续做
