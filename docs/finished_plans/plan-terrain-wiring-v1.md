# plan-terrain-wiring-v1 — Deterministic 建筑布局端到端接线（layout runner → placement manifest → server stamp）

> **一句话主题**：把已写好但**两端悬空且内部不完整**的 `layouts/` 建筑子系统补完并接进运行时——worldgen 侧补桥接 + 实装 stub kind + 让主流程产出 `placement_manifest.json`，server 侧读该 manifest 的展平方块按 chunk stamp，让丹宗遗源 / 王印台等宗门 compound 的建筑真正刷进世界。
>
> **调查依据**：`terrain-broken-links-audit`（2026-06-02，13 断链）+ `validate-terrain-wiring-plan`（2026-06-02，对抗式核验出 3 blocker + 7 major，已在 §11 收口）。本 plan 收口断链 **#1 #2 #3 #4 #5 #6 #8 #10**；#7/#9（giant_sword/ambient）#11/#12/#13 归其他 plan（见 §遗留，归属已按验证修正）。
>
> ⚠️ **关键认知（验证修正）**：layout runner **不是单纯没接线**——`run_layout`→`export_placement_manifest` 内部无数据通路（B2）、`stamp_radial`/`block_grid` 是空 stub（B3）、既有 manifest schema 与初稿冲突（B1）。本 plan P0 是**补完 + 接线**，不是零成本接线。详见 §11 决议。

## 阶段总览

| 阶段 | 主题 | 断链 | 状态 |
|------|------|------|------|
| **P0** | Worldgen layout 子系统补完 + 主流程接线（桥接 + 实装 stub + flatten/mask + export pass） | #1 #8 | ✅ 2026-06-03 |
| **P1** | Server 读 placement_manifest 展平方块，按 ChunkPos stamp（链路另一端，与 P0 成对） | #2 | ✅ 2026-06-03 |
| **P2** | 丹宗遗源端到端激活（资产已就绪，首个消费者验证基础设施） | #3 #5 #10 | ✅ 2026-06-03 |
| **P3** | 王印台端到端激活（新造 4 NBT + 补蓝图/POI，证明可复用） | #4 #6 #10 | ✅ 2026-06-03 |

> 依赖：**P0 与 P1 必须成对 land**（同一条 Python→Rust 链两端）→ P2 用已就绪的丹宗资产做首个端到端验证 → P3 造王印台资产证明可复用。

## 接入面（防孤岛）

- **进料**：
  - bake 蓝图 `server/zones.worldview.example.json`（`TerrainProfileSpec.architectural_layout` / `height.compound_flatten_radius`，`blueprint.py:100-101` 已解析但下游不消费）
  - profile spec `worldgen/terrain-profiles.example.json`（**注意：仓库根 `worldgen/` 下，不在 `scripts/terrain_gen/`**）
  - 已有 `LayoutSpec`：`layouts/dan_zong_compound.py:178 DAN_ZONG_COMPOUND_LAYOUT`（49 placements）、`layouts/wangyintai_compound.py:97 WANGYINTAI_COMPOUND_LAYOUT`（17 placements）
  - NBT 资产 `server/structures/<zone>/*.nbt`（worldgen 侧 `nbt_builder.load_structure` 解析）
- **出料**：
  - worldgen 产 raster `*.bin` + `manifest.json` + **`placement_manifest.json` sidecar**（沿用**既有** `export_placement_manifest` 格式 `{ version, structures:[{ nbt_path, origin, rotation, blocks:[{pos,block,properties}] }] }`，`runner.py:154-179`，已展平到逐方块）
  - server `raster.rs` 读 sidecar → 按 `ChunkPos` 预分桶 → chunk 生成 pass `place_authored_structures` 直接 stamp 展平方块进 `ChunkLayer`。**server 不读 NBT**（worldgen 已展平），客户端走原版 chunk 同步渲染
- **共享类型 / 契约**：
  - **复用既有 `placement_manifest.json` 展平格式**（`export_placement_manifest` 已实现，**不重新定义** schema；server 侧 `PlacementManifest`/`PlacementStructure` serde struct 对拍该既有格式）。
  - server `RasterManifest`（`raster.rs:307-324`）新增 `placement_manifest: Option<PlacementManifest>` 字段。
- **跨仓库契约**：worldgen（Python，`run_layout`/`export_placement_manifest`）↔ server（Rust，`PlacementManifest` + `place_authored_structures`）。**无 agent / client 改动**（建筑即 chunk 方块，原版同步，天然双方契约）。
- **worldview 锚点**：末法残土宗门遗迹——丹宗遗源（炼丹宗门残迹）/ 王印台（忘音台·观天台废墟）。资产由 `plan-dandao-path-v1` / `plan-woliu-path-v1` 锚定，本 plan 只补完接线。
- **qi_physics 锚点**：**N/A**——静态地形方块，无真元流动 / 衰减 / 守恒。

## P0 — Worldgen layout 子系统补完 + 主流程接线

**断链 #1 #8 + 验证 B1/B2/B3/M1/M2/M3**：layout runner 内部不完整 + 主流程不调用。本阶段补完内部数据通路 + 实装 stub + 接 stitcher/export。

交付物（可核验）：

- **桥接 run_layout→export（B2）**：`worldgen/scripts/terrain_gen/layouts/runner.py` — 改 `run_layout`（`:190`）累积每个 `_paste_nbt`（`:226`，当前返回值被丢弃）产的 `NbtPasteResult`，经 `LayoutResult`（新增 `paste_results: tuple[NbtPasteResult, ...]` 字段）或新 helper `collect_nbt_paste_results(spec, zone) -> list[NbtPasteResult]` 暴露，使 `run_layout(...) → export_placement_manifest(paste_results)` 真正可串。
- **实装 stub kind（B3）**：`runner.py:227-236` 的 `stamp_radial`（径向读 pen NBT 按角度旋转 paste）+ `block_grid`（中轴大道 inline block 网格 stamp）从 `logger.debug` 空实现改为真产 `NbtPasteResult.blocks`。丹宗 49 placement 中 16 内外环药圃 `kind=stamp_radial`（`dan_zong_compound.py:69/88`）+ 1 中轴大道 `kind=block_grid`（`:158`）依赖此。
- **facing 旋转（M3，定 worldgen 侧）**：`runner.py:126-133 _paste_nbt` 在旋转坐标（`_rotate_offset:53-65`）的同时旋转 `facing`/`axis`/`rotation` 类 blockstate property，使 manifest 落地即终态（server 不再处理旋转）。dan_zong 用 rotation 90/180/270（`dan_zong_compound.py:110-113`）。
- **registry + 透传（#1/M2）**：
  - `layouts/__init__.py` 新增 `COMPOUND_LAYOUT_REGISTRY: dict[str, LayoutSpec]`（name→spec，键对齐 `architectural_layout` 字符串；**命名避开既有 `LAYER_REGISTRY`**）
  - `blueprint.py` 把 `architectural_layout: str|None` / `compound_flatten_radius: int|None` 从 `TerrainProfileSpec`（`:100-101`）透传到 `BlueprintZone`（`:54-68`）→ `ZoneFieldPlan`（`fields.py:269-281`，**新增这两字段**，作 grep 抓手）
- **stitcher 接 flatten/mask（#8/M2）**：`stitcher.py` — `synthesize_fields()` 主循环（插入点：`_blend_tile_layers` 之后、`compact_layers()` 之前，约 `:576-579`）对带 `compound_flatten_radius` 的 zone 调 `apply_compound_flatten(field, radius, target_height)`（`:418`）+ `compute_layout_density_mask`（`:445`）。**`target_height = POI.pos_xyz.y`**（见 §11 M2 决议）。
- **export pass（#1/M1）**：`__main__.py` — 新增 `run_layout_pass()`：对带 `architectural_layout` 的 zone 查 `COMPOUND_LAYOUT_REGISTRY` → `run_layout` → `export_placement_manifest()` 落盘 `worldgen/generated/terrain-gen/rasters/placement_manifest.json`。**对「zone 有 architectural_layout 但 pois 缺对应 poi_kind」warn+skip**（防 `runner.py:209-215` ValueError 拖垮整条 pass，M1）。
- **测试声明**（`worldgen/tests/`，区分既有回归 vs 新增）：
  - 【既有，回归不得破】determinism/flatten/density mask 单测（`test_layout_infrastructure.py:356/426/451/551/577/618`、`test_nbt_paste.py:200-235`）
  - 【新增】桥接：`run_layout` 产出喂 `export_placement_manifest` 后 manifest `structures[].blocks` 非空
  - 【新增】stamp_radial / block_grid 产出非空 block（B3 回归锁）
  - 【新增】facing 旋转：90° 后楼梯/门 property facing 正确旋转
  - 【新增】export pass：带 `architectural_layout` 的 zone 产 `placement_manifest.json`，缺 poi_kind 的 zone warn+skip 不崩
  - 【新增】stitcher 主流程：flatten 半径内高程 == target_height，density mask 半径内 flora 归零

## P1 — Server 读 placement_manifest 按 ChunkPos stamp

**断链 #2 + 验证 M4**：server 无 placement 消费。**format A**（worldgen 已展平方块，server 不读 NBT，直接 stamp）。**与 P0 成对**。

交付物（可核验）：

- **serde 契约（B1 对拍）**：`server/src/world/terrain/raster.rs` — 新增 `PlacementManifest`/`PlacementStructure`/`PlacementBlock` serde struct 对拍**既有** `export_placement_manifest` 格式（`{version, structures:[{nbt_path, origin, rotation, blocks:[{pos, block, properties}]}]}`）；`RasterManifest`（`:307-324`）加 `placement_manifest: Option<PlacementManifest>`；`TerrainProvider::load()`（`:458-599`）加载 sidecar。
- **空间索引（M4）**：load 时把所有 `PlacementBlock` 按 `ChunkPos` 预分桶 `HashMap<ChunkPos, Vec<(BlockPos, BlockState)>>`（great_hall 实测 111,750 blocks / 跨 96 chunk，避免每 chunk 线性扫 11 万条；`MAX_NEW_CHUNKS_PER_CLIENT_PER_TICK=1`，`mod.rs:390`）。
- **chunk placement pass（zone-agnostic，P3 复用保证）**：新模块 `server/src/world/terrain/authored.rs` — `place_authored_structures(chunk, chunk_pos, provider)`：**纯按 `chunk_pos` 查预分桶 stamp，不读 zone 名**（保证 P3 王印台零新增 server 代码）；接入点在 `decorate_chunk`（`structures.rs:100`）之后、biome 之前（建筑覆盖装饰，与 P0 density mask 双保险）。
- **property→BlockState**：把 manifest 的 `block`+`properties` 解析成 valence `BlockState`（已旋转，P0 终态）。
- **测试声明**（`server/src/world/terrain/` 单测 + e2e）：
  - 契约对拍：P0 产的 `placement_manifest.json` sample ↔ `PlacementManifest` 正反序列化（双端 pin，对齐根 CLAUDE.md schema 测试要求）
  - ChunkPos 分桶：跨 chunk 边界结构两 chunk 各 stamp 自己那部分，无重复无缺漏
  - 大结构：>4 chunk 结构相交，只 stamp 命中 chunk 的列
  - 缺块容错：manifest 某 block 无效 blockstate → warn 跳过不 panic
  - placement_manifest 缺失（旧 manifest）：`Option::None`，chunk 正常生成无 panic（向后兼容）

## P2 — 丹宗遗源端到端激活

**断链 #3 #5 #10**：丹宗资产已就绪，卡在 P0/P1 + 死资产 + biome 越界 + stale manifest。首个端到端验证。

交付物（可核验）：

- **死资产收口（#3）**：`layouts/dan_zong_compound.py` — `master_sarcophagus.nbt` 加入 layout（great_hall 地下室 `kind=nbt` placement）；`herb_garden_pen_inner.nbt`/`outer.nbt`（旧大型园圃，9093B/10849B，非 6x6/8x8 别名）决策：废弃则 `git rm`，保留则加 Placement。终态：`server/structures/dan_zong/` 每个 NBT 都被引用，无死资产、无 stub-kind 假活资产。
- **biome 修复（#10，append-only）**：`worldgen/scripts/terrain_gen/bakers/raster_export.py` — `BIOME_PALETTE`（`:25-37`，长 12）**append-only 扩容**（保持 index 0-11 不变，因 `raster.rs:492-499` 硬编 `forest=palette[7]`/`river=palette[8]`；新 biome 追加 12+）覆盖 `dan_zong_yi_yuan.py:116 biome_id=12`；新增 bake 期断言 `assert biome_id.max() < len(BIOME_PALETTE)`（防 `raster.rs:681-686 unwrap_or(default_wilderness_biome)` 静默退化）。
- **重烤（#5）**：`bash scripts/dev-reload.sh` → 新 manifest 含 `dan_zong_yi_yuan` tile + placement_manifest 含丹宗结构（顺带烤入 5/9 后缺失的 A 类 zone：baolongwang_cavern_deep / 3×scorch）。
- **测试 / 验收**：
  - e2e：`/tpzone dan_zong_yi_yuan` 后该区 chunk 含 `dan_zong_great_hall` 主殿方块（server chunk dump 断言）
  - placement resolve：server 启动日志 `loaded N placements`，丹宗所有 **nbt + stamp_radial + block_grid** kind placement 的 block 数 > 0（不再写"49 全 resolve"——精确到 block 非空）
  - Y 锚点（M2）：great_hall 地板 Y == flatten 摊平地表 Y（POI.y=82），无悬空/掩埋
  - biome（正向断言）：丹宗区 `biome == 丹宗映射 biome`（不写"≠ plains"，default 恰为 plains 时脆弱）

## P3 — 王印台端到端激活

**断链 #4 #6 #10 + 验证 M1/M7**：王印台 layout 引用 4 个不存在的 NBT + 缺 bake 蓝图条目 + 缺 guantiantai POI。证明基础设施可复用。

交付物（可核验，**M7 跨 plan 边界已拆分声明**）：

- **(a) 新增 NBT 资产（本 plan 合规新增，走 §10.1 三轮 + `<PROMISE>`）**：新建 `scripts/nbt/gen_wangyintai_structures.py`（仿 `gen_dan_zong_structures.py`）产 4 NBT 到 `server/structures/wangyintai/`：`jingxuguan_side_hall.nbt` / `corridor_fragment.nbt` / `fallen_vortex_disc.nbt` / `guantiantai_ruins.nbt`。
- **(b) 补丁 woliu 已落地文件（对 `plan-woliu-path-v1` 的必要后续补丁，非重复定义）**：`layouts/wangyintai_compound.py:50/67/82/93` payload 改带 `wangyintai/` 目录前缀真实路径。
- **补 bake 蓝图 + POI（#6/M1）**：`server/zones.worldview.example.json` 新增 wangyintai zone（`terrain_profile:"wangyintai"` + `architectural_layout:"wangyintai_compound"` + **`pois:[{kind:"guantiantai", pos_xyz:[对齐 aabb 中心]}]`**，与 `wangyintai_compound.py:99 poi_kind="guantiantai"` 对齐，否则 `run_layout` raise ValueError；坐标对齐 `zones.json:465` aabb min[3500,40,-2150] max[4500,200,-1150]）。⚠️ woliu Finish Evidence 自报已交付此 zone 但实测 grep=0（虚报红旗），本 plan P3 补全。
- **biome（#10）**：`BIOME_PALETTE` append 覆盖 `wangyintai.py:107 biome_id=17`。
- **重烤 + 测试 / 验收**：
  - `bash scripts/dev-reload.sh` → manifest 含 wangyintai tile + placement_manifest 含 17 placement
  - e2e：`/tpzone wangyintai` 后 chunk 含 `guantiantai_ruins` 方块
  - 复用性断言：王印台走 P0/P1 同一套 `COMPOUND_LAYOUT_REGISTRY` + `place_authored_structures`（zone-agnostic，P1 已保证），**零新增 server 代码**

## §11 决议（pre-P0 收口，2026-06-02）

> 由 `validate-terrain-wiring-plan` 核验产出，每条双锚点（文件:行号 + plan 章节）。**实施以本节为准**。

### B1 placement_manifest schema —— 采用既有展平格式（A），server 不读 NBT
**决议**：沿用 `export_placement_manifest` 既有产出 `{version, structures:[{nbt_path, origin, rotation, blocks:[{pos, block, properties}]}]}`（worldgen 已展平到逐方块）。server **不引** valence_nbt/fastnbt，直接 stamp manifest 里的展平方块。撤回初稿的 `zones[...]` 新 schema。
**落点**：`worldgen/scripts/terrain_gen/layouts/runner.py:154-179`（既有格式）/ `test_nbt_paste.py:200-235`（既有 pin，不破）/ plan §接入面 + §P1。

### B2 run_layout↔export 桥接
**决议**：`run_layout` 累积 `_paste_nbt` 的 `NbtPasteResult`，经 `LayoutResult.paste_results` 暴露给 `export_placement_manifest`。
**落点**：`runner.py:190 run_layout` / `:226 _paste_nbt`（返回值现被丢弃）/ `:149-150 export_placement_manifest` / plan §P0 桥接交付物。

### B3 stamp_radial / block_grid 实装
**决议**：实装两 stub 真产 block（不改成 kind=nbt，保留 layout 语义）。
**落点**：`runner.py:227-236`（现 logger.debug 空实现）/ `dan_zong_compound.py:69/88/158` / plan §P0 实装交付物 + §P2 验收措辞。

### M1 wangyintai guantiantai POI + export 容错
**决议**：P3 zone 补 `kind="guantiantai"` POI；P0 export pass 对缺 poi_kind 的 zone warn+skip。
**落点**：`runner.py:209-215`（ValueError）/ `wangyintai_compound.py:99` / plan §P3 + §P0 export pass。

### M2 flatten target_height = POI.y + 透传字段
**决议**：`apply_compound_flatten` 第 3 参 `target_height` 取 `POI.pos_xyz.y`（great_hall NBT 本地 Y=0..34 + POI y=82 → 地板对齐 82）；`architectural_layout`/`compound_flatten_radius` 经 `BlueprintZone`→`ZoneFieldPlan` 新增字段透传。
**落点**：`stitcher.py:418-422 apply_compound_flatten` / `blueprint.py:54-68 BlueprintZone` / `fields.py:269-281 ZoneFieldPlan` / `zones.worldview.example.json:1461`（zone height base[62,78]/peak92 与 POI y=82 须对账）/ plan §P0 + §P2 Y 锚点验收。

### M3 facing 旋转 —— worldgen 侧
**决议**：worldgen `_paste_nbt` 旋转坐标的同时旋转 facing/axis/rotation property，manifest 落地即终态；server 不处理旋转（故 P1 测试不测 facing，P0 测）。
**落点**：`runner.py:53-65 _rotate_offset`（仅转坐标）/ `:126-133 _paste_nbt`（property 原样透传）/ plan §P0 facing 交付物。

### M5 block-entity —— 显式降级（不支持，入 §遗留）
**决议**：首版**不支持** block-entity 子标签（`load_structure` 只读 palette Name+Properties，`nbt_builder.py:629-655`）。great_hall 的 lectern 落地为空讲经台、skull 为默认头颅——**显式承认降级**，不静默丢。后续如需补：扩 `load_structure` 读 nbt 子标签 + manifest 加 block_entity 字段 + server 写 block entity。
**落点**：`nbt_builder.py:281-282`（写入侧已支持）/ `:629-655 load_structure`（读取侧不读）/ plan §遗留。

## 遗留 / 后续（不在本 plan 范围，归属已按验证 M6 修正）

- **断链 #7 #9**：`giant_sword_sea`（需新建 Generator，四缺）+ `ambient_sword_sea.json` + 巨剑海 POI → 归 **`plan-sword-path-v2`（active，`docs/plan-sword-path-v2.md:571` 已引用该 zone，:726-727 P3-P5 deferred）**；v3 骨架范围是 BOSS AI/剑意/VFX 无 worldgen，**不归 v3**。本 plan P0/P1 基础设施 land 后 sword plan 可直接复用接线。`tsy_zhanchang` zones.tsy.json 条目 → 归 tsy plan。
- **断链 #11 A**：`corpse_mounds` / `ascension_pits` 顶层段 server 消费 → 归 `plan-terrain-tribulation-scorch-v1`。
- **断链 #12**：`ColumnSample` 9 个已加载未消费 raster 层 → **`plan-terrain-layer-query-v1` / `plan-tsy-worldgen-v1` 均已归档（finished_plans，非 active）**；若 ColumnSample 消费缺口仍存在，需核验其 Finish Evidence 是否覆盖，否则新立 plan（不可甩给已归档 plan）。
- **断链 #13**：`ZoneFieldPlan.landmarks` 死字段 → 单独 cleanup（先核查是否 `pois` 历史前身）。
- **block-entity 降级（§11 M5）**：lectern/skull 首版退化，后续补。
- **文档 hygiene**（独立）：`plan-dandao-path-v1` 阶段表 P-1~P5 ⬜ 应回退（虚报归档）；`plan-woliu-path-v1`（王印台 zone 虚报）/ `plan-sword-path-v1` Finish Evidence ⚠️ 红旗；`rm docs/plan-cultivation-pacing-v1.md`（untracked 孤本）。

## §10 消费本 plan 的工作流约束（consume-plan agent 必读）

> P0 worldgen 补完+接线 + P1 Rust 消费 + P2/P3 端到端激活（P3 含 4 NBT 建筑）。结构参 `plan-dandao-path-v1` §10。通用约束全部生效。

### §10.1 P3 NBT 建筑：三轮 + `<PROMISE>`
P3 (a) 的 4 个王印台 NBT 按 docs/CLAUDE.md §6.1：Round 1 `(round 1/3)` → Round 2 structure dump / ASCII 平面投影 `(round 2/3)` → Round 3 spec 一致性 `(round 3/3)`，终轮 commit 写 `<PROMISE>...</PROMISE>`。P0/P1/P2 纯逻辑按 atomic commit + 测试全绿。

### §10.2 多 PR 序列化
1. **PR-1（P0）** worldgen 补完+接线 — 纯 worldgen，产 `placement_manifest.json` sample
2. **PR-2（P1）** server 消费 — 纯 Rust，用 PR-1 sample 做 schema 对拍（**PR-1 merge 后立刻开**）
3. **PR-3（P2）** 丹宗激活 — 依赖 PR-1/2
4. **PR-4（P3）** 王印台激活 — 依赖 PR-1/2，造 NBT（三轮）+ 蓝图/POI

PR-1↔PR-2 是同一条链两端，必须都 land。

### §10.3 subagent + CR 等待
按 docs/CLAUDE.md §6.4/§6.5：每 PR 独立 `subagent_type:"claude"` + `model:"opus"` + 末 `ultrathink`；CR 走 `ScheduleWakeup delaySeconds=1200` 等待（每回合 20min，最多 3 回合），修完重新等 re-review。

### §10.4 单次 consume-plan 全自动到 merge
`/consume-plan terrain-wiring-v1` 后即可下班，consume-plan agent 按 §10.2 四 PR 序列依次 land 后归档入 `finished_plans/`。

## Finish Evidence

> 由 `/consume-plan terrain-wiring-v1` 全自动消费落地（2026-06-03）。worktree `auto/plan-terrain-wiring-v1`，16 commit（11 实施 + 5 Verify 修复）/ 35 文件 / +5521 行。

### 落地清单

**P0 — Worldgen layout 补完 + 接线**（断链 #1 #8 + §11 B1/B2/B3/M2/M3）
- `worldgen/scripts/terrain_gen/layouts/runner.py` — `run_layout` 累积 `NbtPasteResult` 经 `LayoutResult.paste_results` 暴露（B2 桥接）；`_stamp_radial`（farmland+wheat 网格）/ `_block_grid`（mossy_cobblestone 路径）实装（B3）；`_rotate_properties` facing/axis 旋转（M3）
- `worldgen/scripts/terrain_gen/layouts/__init__.py` — `COMPOUND_LAYOUT_REGISTRY`
- `blueprint.py` + `fields.py` + `profiles/base.py` — `architectural_layout`/`compound_flatten_radius` 透传 `BlueprintZone`→`ZoneFieldPlan`（M2）
- `stitcher.py` — `synthesize_fields` 接 `apply_compound_flatten`(target=POI.y) + `compute_layout_density_mask`
- `__main__.py` — `run_layout_pass` 产 `placement_manifest.json`（缺 poi_kind warn+skip）

**P1 — Server placement 消费**（断链 #2 + §11 B1/M4）
- `server/src/world/terrain/raster.rs` — `PlacementManifest`/`PlacementStructure`/`PlacementBlock` serde（对拍既有展平格式 `structures+blocks`）；`TerrainProvider.placement_index: HashMap<ChunkPos, Vec<..>>` 预分桶（M4）；sidecar 缺失向后兼容
- `server/src/world/terrain/authored.rs` — `place_authored_structures`（zone-agnostic，按 ChunkPos stamp，decorate_chunk 后 / biome 前）
- `server/src/world/terrain/blocks.rs` — `block_from_name` 补全 building 方块

**P2 — 丹宗激活**（断链 #3 #5 #10）
- `layouts/dan_zong_compound.py` — master_sarcophagus 接入 layout；旧大型园圃 git rm
- `bakers/raster_export.py` — `BIOME_PALETTE` append 扩容 + bake-time 断言（idx12=plains / 13=old_growth_pine_taiga）
- `profiles/dan_zong_yi_yuan.py` — biome_id 13；`runner.py`/`__main__.py` bare NBT 名 → `server/structures/<zone>/`

**P3 — 王印台激活**（断链 #4 #6 #10）
- `server/structures/wangyintai/` — 4 NBT（`scripts/nbt/gen_wangyintai_structures.py` 产，三轮 + PROMISE）：jingxuguan_side_hall / corridor_fragment / fallen_vortex_disc / guantiantai_ruins
- `server/zones.worldview.example.json` — wangyintai zone + `architectural_layout` + `compound_flatten_radius:64` + `{kind:guantiantai}` POI
- `bakers/raster_export.py` — `BIOME_PALETTE[17]=windswept_hills`

**Verify 修复**（opus 对抗审查抓 1 blocker + 3 major → 5 fix commit）
- FIX A `blocks.rs` 补 28 方块 + dual-pin 测试（server zero-drop 63 块 / palette⊆白名单）
- FIX B BIOME index 12 防回归（rift_mouth_barrens/pseudo_vein_oasis 保 plains）
- FIX C flatten/density-mask 集成测试 12 条
- FIX D 死资产 git rm + 诚实化测试

### 关键 commit
- 实施（2026-06-02）：`9b60cd64a` B2/B3/M3 → `2695caaa8` M2 透传 → `e66c07de3` registry/stitcher/export → `b508f23ff` P0 75 测试 → `255c7a6a4` P1 server → `e70fc15aa`/`90f956705`/`bc8264272`/`cd32e5b53` P2 → `3d482dfd2`/`77e66bf74` P3
- 修复（2026-06-03）：`189f6cc0d` FIX A → `59dba6ee1` FIX B → `6dbc30e07` FIX C → `aba196f32` FIX D → `c7576b062` clippy

### 测试结果
- worldgen：`python3 -m pytest tests/ scripts/terrain_gen/ -q` → **412 passed**
- server：`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` → **6997 passed**，fmt/clippy clean
- 关键 pin：server `authored_nbt_palette_zero_drop_in_build_placement_index`（63 块全 stamp，drop=0）/ `all_authored_structure_blocks_resolve`；worldgen `test_nbt_block_palette`（palette⊆白名单）/ `test_flatten_density_mask`（全链路集成）/ `test_rift_mouth_barrens` + `test_pseudo_vein_oasis`（biome 防回归）

### 跨仓库核验
- worldgen(Python)：`COMPOUND_LAYOUT_REGISTRY` / `run_layout`→`export_placement_manifest` / `apply_compound_flatten` / `_resolve_layout_target_height` / `BIOME_PALETTE`
- server(Rust)：`PlacementManifest` / `place_authored_structures` / `block_from_name` / `TerrainProvider.placement_index`
- 跨进程契约：`placement_manifest.json`（worldgen producer ↔ server consumer，展平 structures+blocks 格式）
- **无 agent/client 改动**（建筑即 chunk 方块，原版同步）

### 遗留 / 后续
- **#5 重烤 + 游戏内 e2e（merge 后本地步骤）**：`bash scripts/dev-reload.sh` 重烤 + `/tpzone dan_zong_yi_yuan`/`wangyintai` 看建筑。worktree 无 venv + 需真服 + 产物 gitignored 不进 PR，故 PR 内未跑；单测已锁 placement 数据正确性（zero-drop 63 块全 resolve）。
- **block-entity 降级**（§11 M5）：lectern/讲经台落空、skull 默认；后续扩 `load_structure` 读 nbt 子标签。
- **iron_nugget→AIR**（Verify minor）：`fallen_alchemist_bone.nbt` 装饰矿渣（item 非方块）映 AIR 渲染成洞；后续改 NBT 授权用真方块再 dump。
- **P3(b) 路线变更**：未改 wangyintai_compound.py payload（避开改 woliu 已落地文件），改由 `__main__._LAYOUT_NBT_SUBDIR` base_dir 解析，功能等价。
- **stamp_radial 草药占位**：药圃用 wheat 占位，未实现差异化灵草（B3 自述 stand-in，范围内）。
- **§遗留主题归其他 plan**：#7/#9 giant_sword/ambient → `plan-sword-path-v2`(active)；#11 corpse_mounds/ascension_pits → scorch；#12 ColumnSample 9 层 → layer-query/tsy-worldgen（已归档，需核验或新立）；#13 landmarks → cleanup。
- **文档 hygiene**（独立）：plan-dandao-path-v1 阶段表虚报、plan-woliu/sword Finish Evidence 红旗、`rm docs/plan-cultivation-pacing-v1.md`。
