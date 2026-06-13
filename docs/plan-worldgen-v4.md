# plan-worldgen-v4 — 地形生成全面重构：span 列表示 · zone DSL · 灵气配平 · NBT 装饰 · Web 3D 控制台

> **骨架（skeleton）**。一句话主题：把地形生成从「单层 heightmap + 程序化装饰 + 灵气两层漂移」重构为「多段 span 列 + 3D 噪声雕刻 + zone 声明式 DSL + 全局灵气预算配平 + NBT 资产装饰 + Web 交互式 3D 预览」，目标是能自然生成大峡谷 / 浮空岛 / 多层洞穴等奇幻地貌，且 zone 完全定义自己长什么样、长什么作物、灵气多少。
>
> 立项动机：当前 v3 系（plan-worldgen-v3 / v3.1，finished）生成效果不出色——单层 heightmap 无法表达垂直地貌（浮空岛/洞穴全靠专用补丁层）；装饰是程序化几何（place_tree/place_boulder），程序味重；worldgen `qi_density` 与运行时 `Zone.spirit_qi` 是两份互不同步的数据；预览只有俯视 PNG。
>
> 用户已拍板的方向性决策（2026-06-12）：
> 1. 底层表示 = **多段 span 列导出 + Python 端局部 3D 噪声雕刻**（生成自由度接近真 3D，存储/消费成本停在 2.5D）
> 2. Web 预览 = **交互控制台**（three.js 体素视图 + 调参触发重生成）
> 3. 迁移策略 = **原地推倒重写**（不开 v2 双轨；每 PR 仍须 main 可跑、测试全绿）
> 4. 装饰 = **大部分 NBT 预制**（画廊服务器审阅 → 游戏内原地修改 → 回写导出），巨树等少数保留程序生成
> 5. 画廊拿方块 = **owo 方块面板**，接入塔科夫式背包（plan-nested-pack-base-v1 体系），所有方块可拿

## 阶段总览

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | span 列表示地基（Python 导出 + Rust 消费原地重写，行为等价迁移） | ✅ 2026-06-12 |
| P1 | Web 3D 预览控制台（只读查看器 + zone 级重生成按钮） | ✅ 2026-06-12 |
| P2 | Zone 地形 DSL + 19 个 profile 全量重写为声明式组合 | ✅ 2026-06-13 |
| P3 | 3D 噪声雕刻奇幻地貌（大峡谷悬壁 / 浮空岛群 / 拱门 / 多层洞穴） | ✅ 2026-06-13 |
| P4 | 全局灵气配平（统一灵气场 → zone spirit_qi 预算 → zones.json，对齐 `SPIRIT_QI_TOTAL`） | ✅ 2026-06-13 |
| P5 | 画廊审阅闭环（Rust NBT IO + `/gallery` + structure save 回写 + owo 方块面板） | ⬜ |
| P6 | NBT 装饰资产管线（DecorationSpec→NBT 模板 + 运行时 stamp + 程序生成保留清单） | ⬜ |
| P7 | 迁移收口与验收（死层清理 / snapshot CI / anvil export / 性能基准 / e2e） | ⬜ |

## 接入面 checklist

- **进料**：
  - `worldgen/zones.worldview.example.json` blueprint（zone bounds / spirit_qi / POI）+ `terrain-profiles.example.json`
  - `qi_physics::ledger`（`WorldQiBudget` / `DEFAULT_SPIRIT_QI_TOTAL`，`server/src/qi_physics/{ledger.rs,constants.rs}`）——P4 配平的预算口径**只引用不新造**
  - `scripts/nbt/nbt_builder.py`（`load_structure()` / `StructureBuilder.save()`，P5/P6 NBT 读写基座）
  - `server/structures/{dan_zong,wangyintai}/*.nbt`（11 个既有资产，画廊首批审阅对象）
  - `botany/env_lock.rs` 8 种环境锁的 raster 层语义（P0 适配的硬约束）
- **出料**：
  - rasters v4（spans 二进制 + 清理后的语义层）+ manifest v4 → `TerrainProvider`（`server/src/world/terrain/raster.rs`）
  - `server/zones.json` 的 `spirit_qi` 字段由 worldgen 灵气场派生导出 → `ZoneRegistry` 启动加载（消除 qi_density↔spirit_qi 两层漂移）
  - NBT 装饰模板 → chunk 生成期 stamp（`flora.rs` 重写）
  - `worldgen/console/`（dev 工具，不进生产链路）
- **共享类型 / event**：重写 `ColumnSample`（保留 trait 面：`SurfaceProvider`、`EnvLayerSampler`）；复用 `Decoration` / `PlacementManifest`；不新增任何 qi 流动 event——本 plan 灵气改动全在**生成期静态配置**，运行时流动仍归 `qi_physics::ledger`
- **跨仓库契约**：server（terrain 全模块重写 + gallery 命令 + NBT IO）；client（owo 方块面板 + 画廊 UX，新增 C2S give-block intent 需进 `agent/packages/schema` + samples 双端对拍）；agent 不参与（`world/events.rs` qi_heatmap 读 `zone.spirit_qi`，接口不变）
- **worldview 锚点**：§二（灵气守恒律 / 压强法则 / 死域 / 负灵域——P4 灵气场三层环境的正典依据）、§十（全服灵气总量恒定缓慢衰减——配平预算口径）、§四（灵脉 / 灵眼）、各 zone 正典命名（青云断峰 / 灵泉湿地 / 血谷 / 北荒 等沿用，不新造地名）
- **qi_physics 锚点**：P4 只做生成期静态分布派生，调用面 = 读 `DEFAULT_SPIRIT_QI_TOTAL` / `WorldQiBudget::from_env` 的预算口径做导出期校验断言；**不新增物理常数、不动 excretion/release/collision 任何公式**；`/zone_qi set` dev 命令行为不变。运行时 wilderness 灵气账户不在本 plan scope（见 §8 #3）
- **与 active plan 避撞**：`plan-zhenfa-content-v2` 依赖 `EnvField.local_zone_qi` 读数正确——P4 不改 spirit_qi 的运行时读取路径，只改初始值来源，无冲突；`plan-nested-pack-base-v1` 的 owo overlay spike（其 §8 #5）是 P5 方块面板的 UI 先例，P5 排期在其后；衰减常数迁移归 `plan-qi-physics-patch-v1`，本 plan 不碰

---

## P0 — span 列表示地基（行为等价迁移）

核心思想：**先换表示不换景观**。P0 结束时世界长相与 v3 基线一致（pin 对拍），但底层已是 span 列，为 P2/P3 铺路。

- **Python 侧**（`worldgen/scripts/terrain_gen/`）：
  - `fields.py`：新增 span 列核心类型（`ColumnSpans`，定长 4 段 × `(i16 floor_y, i16 ceiling_y)` + count 面，编码见 §8.1 #1）；`LAYER_REGISTRY` 大清理——`sky_island_mask/base_y/thickness`、`cave_mask`、`ceiling_height`、`entrance_mask`、`cavern_floor_y` 等垂直补丁层**收编进 spans 删除**；`qi_density` 等语义层、`flora_*` 生态层保留
  - 兼容转换 shim：现有 19 个 profile 暂不重写，其 2D 输出（height + 垂直补丁层）由 shim 自动折算成 spans（普通列 = 1 段；浮空岛 = 第 2 段；洞穴 = 切开实体段）
  - `stitcher.py`：span blend 语义（zone overlay 的 spans 与 wilderness base spans 按 boundary_weight 融合，规则在 P0 决议定）
  - `bakers/raster_export.py`：导出 `spans.bin`（替代 `height.bin` + 全部垂直补丁层），manifest `version: 2`
- **Rust 侧**（`server/src/world/terrain/`）：
  - `raster.rs`：`TileFields` / `ColumnSample` 重写——`spans: SmallVec<[(f32, f32); N]>` 替代 `height` + sky_island 三层 + cave 三层；`sample()` / `query_surface()` 保持「地表段顶面」语义（§8.1 #2，消费方零迁移）
  - `column.rs`：`fill_column()` 改按段填充，删除 sky_island / cave 专用分支
  - 消费方适配（爆炸半径已调研锁定）：`flora.rs` / `structures.rs` / `giant_sword.rs`（base_y 取段顶）、`botany/env_lock.rs`（mask + tier 语义层保留、`env_sky_island()` 改 span 派生，§8.1 #12）、`npc/navigator.rs`（`query_surface` fallback）、`network/mod.rs`（neg_pressure 同步不受影响）
- **测试**：span 编解码正反 pin（每种列形态：单段/浮岛/多层洞/空列）；行为等价对拍（重生成后对 v3 基线抽样列 height/水位/biome 一致）；`ColumnSample` 全字段消费方回归；botany env_lock 8 锁专属 case

## P1 — Web 3D 预览控制台

- `worldgen/console/`：前端 three.js（greedy mesh 把 spans + surface palette 重建为体素网格，分 tile LOD）；图层开关：地形 / 水体 / 灵气热力（P4 前先显示 qi_density）/ 装饰点位 / NBT 包围盒（P6 后）
- `worldgen/scripts/terrain_gen/console_server.py`：HTTP 后端（FastAPI + uvicorn，dev-only 依赖，§8.1 #7）——`GET /api/manifest`、`GET /api/tile/{x}/{z}`、`POST /api/regen`（按 zone 增量重跑 pipeline 后热刷新）
- 控制台左侧 zone 列表 + 参数面板；P1 阶段参数面板 = blueprint JSON 字段直编，P2 DSL 落地后升级为 schema 自动生成表单
- 验收：浏览器打开控制台能飞行查看 spawn 周边 3D 地形、切图层、对一个 zone 改参数点重生成后视图刷新
- pipeline.sh / dev-reload.sh 加 console 入口（不挡现有流程）

## P2 — Zone 地形 DSL + profile 全量重写

- 目标形态：zone 在 blueprint 里**声明式**定义地形——`terrain_style`（基底高度场 + 雕刻器链 + 特征件）、`surface_palette`、`flora_table`（作物/植被集 + 密度曲线）、`qi_grade` 六档 + 可选 `qi_override`（§8.1 #4，接 P4）
- `worldgen/scripts/terrain_gen/dsl.py` + 可复用算子库（height ops / carve ops / scatter ops / mask ops）；19 个手写 numpy generator 重写为 DSL 组合，保留自定义 python 算子注册的 escape hatch
- P0 的兼容 shim 在本阶段逐 profile 退役，P2 收尾时删除
- 测试：DSL schema 正反 sample pin；每个迁移 profile 与迁移前 console 截图对照验收；算子库单测饱和

## P3 — 3D 噪声雕刻奇幻地貌

- `worldgen/scripts/terrain_gen/carvers/`：`canyon_carver`（谷壁带 3D 噪声内凹悬壁）、`floating_island_carver`（岛底钟乳垂坠雕刻）、`arch_carver`（石拱门）、`cave_network_carver`（多层连通洞穴）——全部输出 span 修改，不引入任何新 raster 层
- 重做核心景观 profile：rift_valley → 真·大峡谷（悬壁 + 层叠岩带），sky_isle → 浮空岛群（错落高度 + 雕底），洞穴网络 profile
- 视觉验收走 console：**每个景观 profile 按 §6.1 三轮打磨 + `<PROMISE>` 担保**（console 截图作为 round 证据）
- 测试：carver determinism（同 seed 两跑 span 全等）；雕刻后守恒检查（span 合法性：段不重叠、floor<ceiling）

## P4 — 全局灵气配平

- `worldgen/scripts/terrain_gen/qi_field.py`：全图**统一灵气场**（灵脉网络沿地形特征走线 + zone 贡献核 + 死域/负灵域，对应 worldview §二三层环境）；`qi_density` raster 与 zone `spirit_qi` **同源派生**（一份场两种导出），消除两层漂移
- zone `spirit_qi` 派生：qi_grade 档位铺场（override 进配平闭环，§8.1 #4）→ zone bounds 面积加权均值 → 写 `server/zones.json`（幂等字段导出 + 运行时字段 merge + `generated_by` 标记，§8.1 #11）；wilderness 此阶段只产**静态报表**（场总量 / 分布直方图进 manifest），运行时账户不归本 plan（§8.1 #3）
- 导出期配平断言：各 zone 预算 + wilderness 余量对齐 `WorldQiBudget` 口径（引用 const，**不写字面 100**）
- console 灵气热力图层切到新场；`/zone_qi set` dev 行为不变
- 测试：派生函数正反 pin（富集区/死域/负灵域三态）；配平断言专属 case；zones.json 导出对拍 blueprint

## P5 — 画廊审阅闭环

- **Rust NBT IO**（前置能力，P6 复用）：新建 `server/src/world/terrain/nbt_io.rs`，valence_nbt 0.8.0 直读写 gzip structure NBT（§8.1 #6），覆盖 11 个既有资产 round-trip pin + Rust↔Python 交叉对拍
- `/gallery` dev 命令：把 `server/structures/**/*.nbt` 按网格 stamp 到画廊区，每格挂悬浮名牌 + 记录包围盒
- `/structure save <name>`：把画廊格子区域回序列化覆盖原 .nbt（与 `scripts/nbt/nbt_builder.py` 的 Python 实现 round-trip 对拍）
- **owo 方块面板**：client `BlockPickerScreen` 以 `LootContainerPanel` 浮窗模式嵌入 InspectScreen（dev/creative 才显示）；C2S `BlockPickerActionV1` + dev handler + `vanilla:<block_id>` ItemTemplate 自动生成（§8.1 #5）；schema 包 + samples 双端对拍
- dev-only 红线：gallery / save / 方块面板全部挂 dev 命令树，不进生产 gameplay
- 测试：NBT round-trip（读→摆→存→读 全等）；画廊格包围盒；give-block 链路 e2e

## P6 — NBT 装饰资产管线

- `DecorationSpec` 扩展：`nbt_template` + 变体列表（替代 size_range 缩放——NBT 不可缩放，靠 3~5 变体 + 随机旋转/镜像防重复感）
- 资产生产线：脚本出初稿（`scripts/models/gen_flora_*.py` 风格）→ P5 画廊审阅精修 → 入 `server/structures/decorations/<kind>/`
- `flora.rs` 重写：密度采样保留（hash 抖动 + 聚类门控不变），`place_decoration()` 的程序几何分支替换为 NBT 模板 stamp（地形贴合 anchor 规则：贴地/嵌地/悬挂）
- **程序生成保留清单**（§8.1 #9 已收口，判据：逐格微密度/地形自适应留程序、离散摆件走 NBT）：保留 `mega_tree.rs` 四巨树、洞穴逐格微装饰、单格地被、水生逐格；NBT 化 `flora.rs` 离散摆件全部 + `structures.rs` 中型结构；NBT 模板全量驻留内存、stamp memcpy 级（§8.1 #10）
- 资产类交付按 §6.1 三轮 + `<PROMISE>`；每类装饰必须 3+ 变体（对齐 feedback：忌单方向 stub）
- 测试：模板 stamp determinism；anchor 规则各形态 case；变体/旋转覆盖 pin

## P7 — 迁移收口与验收

- 删除：P0 兼容 shim 残留、死 raster 层读取代码、`flora.rs` 程序几何死分支、未消费预留层裁决（spirit_eye_candidates / realm_collapse_mask / anomaly_* 留或删，逐个裁决记录）
- 适配：worldgen-preview snapshot CI、`anvil_export.py`（读 spans 重写）、`harness/raster_check.py` 后验规则更新
- 性能基准：chunk 生成耗时对比 v3 基线（预算 §8 #10）；spans 采样 / NBT stamp 热路径 profile
- 端到端验收：`scripts/smoke-test.sh` + 进游戏实测大峡谷/浮空岛/画廊；Finish Evidence 填写

---

## §8 开放问题（升 active / P0 决策门前需收口）

1. **spans.bin 编码**：MAX_SPANS 取几（4？6？）；y 用 i16 还是 f32；定长槽位 vs 变长+索引；空列（虚空）表示。需估算 tile 体积与 mmap 随机读成本。
2. **多 span 列的 surface 语义**：`query_surface()`（NPC 导航）、botany 地表高度、装饰 base_y 在多段列各取哪段（最低段顶？最高可站立段？）；水位与段的交互。
3. **灵气配平守恒口径**：`SPIRIT_QI_TOTAL` 在 zone 总和与 wilderness 之间怎么分摊（现状 zone 总和仅 5~15 / 100，大头在账外）；wilderness 运行时账户是否需要另立 qi_physics 扩展 plan 以及本 plan 给它留什么静态接口。
4. **qi_density → spirit_qi 派生函数**：值域映射（0..1 → -1..1）、zone 内聚合方式（均值/面积加权/POI 加权）；zones.json 被 worldgen 覆盖 vs 手工调优的版本化/merge 策略（§8 #11 合并考虑）。
5. **E 键与方块 give 链路核实**：调研未发现屏蔽 E 的 mixin（与既有认知不符，需实地验证客户端行为）；vanilla block 不在 ItemTemplate 体系内，give-block 走「block 名直挂 ItemInstance」还是「dev 专用放置模式不进背包」。
6. **server 端 NBT 选型**：valence_nbt 直读写 .nbt vs Python 预编译中转；gzip/未压缩兼容；与 `nbt_builder.py` 的格式互通 pin。
7. **console 技术栈**：vite + three.js + FastAPI（独立 dev 工具）vs 复用 library-web（Astro 静态站，倾向不复用——需要动态后端）；与 dev-reload.sh 的关系。
8. **anvil export / snapshot CI 适配范围**：anvil 路线（CI 用）在 span 表示下是重写还是降级支持；snapshot 基线图全部重打。
9. **程序生成保留清单边界**：巨树、钟乳石、单格地被之外还有谁保留（boulder？crystal？）；判据写死（"需随地形自适应/逐格微密度"才保留）。
10. **性能预算**：chunk 生成 tick 预算（现状 `MAX_NEW_CHUNKS_PER_CLIENT_PER_TICK=1` 下单 chunk 耗时上限）；NBT 模板内存驻留策略。
11. **zones.json 双写治理**：worldgen 导出与手工调优共存策略（导出带 `generated_by` 标记 + 手工字段白名单？）。
12. **botany env_lock 在 span 下的语义**：SkyIslandMask / UndergroundTier 两锁改 span 几何查询还是保留独立语义层；19 种灵草环境锁回归矩阵。

> 全部已在 §8.1 收口（2026-06-12）。原表保留以备追溯，**实施时以 §8.1 决议为准**。

## §8.1 决议（pre-P0 收口，2026-06-12）

> 决议依据：4 个 Explore agent 并行核查代码现状（span 编码与 surface 语义 / E 键与 give 链路 / NBT 选型与性能基线 / 灵气口径与 CI 适配面）+ 用户拍板 2 项（#4 灵气档位制、#9 保留清单判据版）。

### #1 spans.bin 编码

**决议**：
1. 定长槽位 `MAX_SPANS = 4`，每段 `(floor_y: i16, ceiling_y: i16)`；世界 Y ∈ [-64, 432)（`MIN_Y=-64` / `WORLD_HEIGHT=496`），i16 余量充足。
2. 导出两个对齐友好的平面文件：`spans_count.bin`（u8/列，0 = 全虚空列）+ `spans.bin`（4 段 × 4B = 16B/列，未用槽位填哨兵 `i16::MAX`）。单 tile 约 4.45 MB，替换 `height.bin`(1.0M) + 六个垂直补丁层(6.0M)，净 **-2.5 MB/tile**（现状 206 tile、8.2 MB/tile）。Rust 端 mmap 直读 `offset = col_idx × 16`。
3. 拒绝变长 + 索引编码（额外间接跳转、实现复杂、收益不值）；拒绝 f32（精度浪费、体积翻倍）。

**落点**：`server/src/world/terrain/raster.rs:155-225`（ColumnSample 改 `spans: SmallVec<[(i16,i16); 4]>`）/ `worldgen/scripts/terrain_gen/fields.py:91-95`（LAYER_REGISTRY 删 sky_island_base_y/thickness、cave_mask、ceiling_height、entrance_mask、cavern_floor_y）/ `bakers/raster_export.py`（manifest version: 2）/ plan P0。

### #2 多 span 列 surface 语义

**决议**：
1. `query_surface()` 语义不变 = **地表段（最低实体段）顶面**；`SurfaceProvider` trait 不动，NPC 行为零迁移。
2. 各消费方维持独立作用域：NPC 导航只认地表段（`npc/navigator.rs:751`）；botany 多层生成分支保留、改由段派生（`botany/lifecycle.rs:206-214` 已有 cavern/sky_island 优先逻辑）；装饰 `placement_base_y()` 按类型选段（`flora.rs:97`：sky_isle_top → 高位段顶、sky_isle_bottom → 高位段底）；structures 按绝对 y 范围约束不变（`structures.rs:260-321`）。
3. 水位交互逻辑不变（`world/terrain/mod.rs:71-77` 可游泳判定）；洞穴/浮岛 NPC 寻路不在本 plan scope。

**落点**：`server/src/world/terrain/mod.rs:68-83` / `column.rs:33-35`（surface_y 改从 spans 取最低段顶）/ plan P0。

### #3 灵气配平守恒口径

**决议**：
1. 本 plan 只做**生成期静态配平**：统一灵气场全图积分在导出期归一为预算常数（引用 `DEFAULT_SPIRIT_QI_TOTAL` / `WorldQiBudget::from_env` 口径，`qi_physics/constants.rs:64`，**不写字面 100**）；zone 份额 + wilderness 份额写入 manifest 静态报表（现状 28 zone spirit_qi 总和仅 7.85，大头在账外，重构后报表补齐）。
2. 运行时守恒等式不动（`qi_physics/ledger.rs:443-477` summarize_world_qi、`:502-520` assert_conservation）；**不新增** `Wilderness` 账户变体——wilderness 保持"环境浓度分布"模式，账户化留给未来 qi_physics 扩展 plan，manifest 报表即其静态接口。
3. 拒绝本 plan 内扩 QiAccountKind / 改 ledger：qi_physics 公式与账户体系归 qi_physics plan 族（防孤岛红旗 §四）。

**落点**：`worldgen/scripts/terrain_gen/qi_field.py`（新建，导出期配平断言）/ manifest v2 `qi_budget_report` 字段 / plan P4。

### #4 qi_grade 档位制与派生函数（用户拍板 2026-06-12）

**决议**：
1. 统一灵气场 native 值域 **[-1, 1]**（覆盖 worldview §二 三层环境：负灵域 / 死域 / 馈赠区）。zone DSL 声明 `qi_grade` 六档（negative [-1,-0.1) / dead [-0.1,0.02) / trace [0.02,0.15) / common [0.15,0.45) / rich [0.45,0.7) / font [0.7,1]），可选 `qi_override` 连续值精调。
2. **override 进配平闭环**（用户硬约束）：override 作为场生成的硬约束铺场，全图归一时由 wilderness 与未 override 区域吸收差额；导出期配平断言把 override 计入总量——override 精调浓度，**不豁免守恒**。
3. 派生：`spirit_qi` = zone bounds 内场的面积加权均值 clamp [-1,1] → 写 zones.json；`qi_density` raster = `clamp01((field+1)/2)` 维持 [0,1] 兼容现有消费（`harness/raster_check.py` 值域规则、POI selector、`profiles/*.py` 现有 `qi_base = zone.spirit_qi` 单向依赖升级为同源闭环）。现有 28 zone 值就近归档为初始 qi_grade（保持手感），归档表进 P4 交付物。

**落点**：`worldgen/scripts/terrain_gen/qi_field.py` + `dsl.py`（qi_grade schema）/ `server/zones.json`（导出）/ `blueprint.py:214`（spirit_qi 字段流向）/ plan P2、P4。

### #5 E 键真相与 give-block 链路

**决议**：
1. E 键并非被屏蔽而是被**重路由**：`client/.../mixin/MixinMinecraftClient.java:26-30` 拦截 `setScreen(InventoryScreen)` 改开 InspectScreen（任何 gamemode 下原版创造物品栏都打不开）。**保持重路由不动**，方块面板做进塔科夫背包体系（用户已拍板）。
2. 四件套新增：① schema C2S `BlockPickerActionV1 { block_id, count }`（进 `agent/packages/schema` + samples 双端对拍）；② server dev handler（参照 `cmd/dev/give.rs:48-75`，dev-gated + gamemode 校验）；③ ItemRegistry 启动时为全 vanilla block 自动生成 `template_id = "vanilla:<block_id>"`、`category = Block` 的 ItemTemplate，`block_item_to_state()` 加 `vanilla:` 前缀直通分支（`world/block_place.rs:309-335`，现状仅 15 项硬编映射 `BlockVanillaIconMap.java:22-37`）；④ client `BlockPickerScreen` 以 `LootContainerPanel` 浮窗模式嵌入 InspectScreen，仅 dev/creative 显示（对齐 HUD conditional display 约束）。
3. 拒绝处理 Valence `CreativeInventoryAction` 包（server 现无 hook；绕过自定义背包会跳过重量/格子校验，破坏 inventory 模型唯一入口）。

**落点**：`client/.../inventory/InspectScreenBootstrap.java:31-52`（路由先例）/ `server/src/inventory/mod.rs:369-371`（ItemRegistry）/ plan P5。

### #6 server 端 NBT 选型

**决议**：
1. 直接用 **valence_nbt 0.8.0**（已在依赖树，`server/Cargo.lock:3259`，`faction_tint.rs:267-278` / `preview/decorations.rs:19` 已活跃使用 Compound/Value）——不引新 crate、不走 Python 预编译中转。
2. 新模块 `server/src/world/terrain/nbt_io.rs`：`read_structure_nbt()` / `write_structure_nbt()`，格式对齐 `scripts/nbt/nbt_builder.py:623-655`（DataVersion 3465 + size/palette/blocks 根结构，**gzip mandatory**——11 个既有 .nbt 全部 gzip 头已验证，裸 NBT 拒绝）。round-trip pin 覆盖 11 个既有资产 + Rust↔Python 交叉对拍（Rust 写 Python 读、反之）。
3. `authored.rs` 的 placement_manifest.json 链路 P5 不动（向后兼容），P6 按需再议直读迁移；palette 块名必须过 `blocks.rs:260` `AUTHORED_NBT_BLOCK_NAMES` 合规检查（既有零漏测试 `raster.rs:1814-1855` 范本）。

**落点**：`server/src/world/terrain/nbt_io.rs`（新建）/ plan P5。

### #7 console 技术栈

**决议**：
1. 独立 dev 工具：前端 **vite + three.js**（TypeScript，`worldgen/console/`），后端 **FastAPI + uvicorn**（`console_server.py`，依赖只进 worldgen dev requirements）。
2. 不复用 library-web（Astro 纯静态、无后端、定位是图书馆前端）；与 dev-reload.sh 解耦，console 自行触发按 zone 增量 regen。
3. tile 数据 HTTP 直传 spans/语义层二进制（ArrayBuffer），前端 greedy mesh + LOD；不引数据库、不做鉴权（仅 localhost dev）。

**落点**：`worldgen/console/`（新建）/ plan P1。

### #8 anvil export / snapshot CI 适配

**决议**：
1. `anvil_export.py` 改读 spans 按段 loop fill——改造面小：现状只读 heightmap + 三层简单 palette、biome 单条 plains（`anvil_export.py:65-68,194-195`）。
2. `harness/raster_check.py`：**P0 即激活** span 合法性规则（段不重叠、floor < ceiling、哨兵规范、count 与槽位一致）；既有「qi_density vs zone.spirit_qi gross mismatch」规则（`raster_check.py:9`）在 P4 升级为同源派生硬断言。
3. snapshot CI（`.github/workflows/worldgen-preview.yml:76-127`，`BACKEND=anvil` + `BONG_WORLD_PATH` 链路）框架不动；基线图无持久化（每次 CI 重生成），无需重打基线，validation R1/R2/R3 规则继续生效。

**落点**：`worldgen/scripts/terrain_gen/anvil_export.py` / `harness/raster_check.py` / `scripts/dev-reload.sh:51-67`（validate 入口）/ plan P0、P7。

### #9 程序生成保留清单（用户拍板 2026-06-12：判据版）

**决议**：
1. 判据写死：**逐格微密度装饰或需随地形连续变形 → 保留程序；离散摆件 → NBT**。
2. 保留程序：`mega_tree.rs` 四巨树（灵木/古松/枯木/沼柏）；洞穴逐格微装饰（钟乳石/石笋/地衣/苔藓/洞穴藤/蘑菇/紫晶簇，`decoration.rs`）；单格地被（ground_cover 全部）；水生逐格（睡莲/海带/海草/岩浆）。NBT 化：`flora.rs` 的中小树/灌木/巨石/水晶/大蘑菇/倒木/坟冢/悬挂水晶（9 个 `place_*` 几何函数除 flower 外全部退役）+ `structures.rs` 中型结构（废墟柱/破坛/骨堆/灵矿脉/裂谷桥/spawn portal）。
3. `giant_sword.rs` 剑海特区维持现状（已成型特区，不在本 plan 改造范围）。

**落点**：`server/src/world/terrain/{flora.rs,structures.rs,decoration.rs,mega_tree.rs}` / plan P6。

### #10 性能预算

**决议**：
1. 单 chunk 生成目标 **< 25 ms、硬顶 30 ms**（tick 50 ms − packet ~5 ms − systems ~10 ms − 余量 5 ms；`mod.rs:388-401` 注释实测基准 ~30 ms/chunk @ 20 TPS）；`MAX_NEW_CHUNKS_PER_CLIENT_PER_TICK = 1` 硬性保留。
2. NBT 模板**全量驻留内存**：启动解压全部装饰/结构模板（现 11 资产解压 ~4.3 MB；decorations 增量预算 ≤ 32 MB，超了再上 LRU）；stamp 必须 memcpy 级（< 1 ms），**禁止运行时 gzip 解压**（great_hall 4 MB 解压 ~50 ms 会直接爆 tick）。
3. span 化 `fill_column` 预估 +0.5~1 ms/chunk（段均值 1.5~2），预算内。P7 建 `server/benches/` chunk 生成基准（v3 基线对比 + NBT stamp 微基准）。

**落点**：`server/src/world/terrain/mod.rs:264,273,388-401` / plan P6、P7。

### #11 zones.json 双写治理

**决议**：
1. worldgen 导出只写**幂等字段**（name / aabb / spirit_qi / danger_level），运行时手工字段（active_events / patrol_anchors / blocked_tiles）按 zone name match **merge 保留**（字段差异已核：blueprint 独有 display_name/pois/worldgen/dimension，zones.json 独有上述三个运行时字段）。
2. 文件头加 `generated_by: "worldgen-v4 <commit>"` 标记；导出流程强制 `git diff server/zones.json` 预览——仅 spirit_qi 变动可自动过，结构性变动（zone 增删/aabb 改）人工 review。
3. 拒绝双向同步（运行时回写 blueprint）：blueprint 是 source of truth，zones.json 是导出物 + 运行时字段宿主（git 历史证实混合维护模式：批量同步 097f34771 + 手工微调 52c922a3c 并存，merge 策略正是为此设计）。

**落点**：`server/src/world/zone.rs:196-256`（ZoneRegistry::load 加载点）/ P4 导出脚本（新建 merge 逻辑）/ plan P4。

### #12 botany env_lock 在 span 下的语义

**决议**：
1. `sky_island_mask`、`underground_tier` **保留为独立语义层**（5 种灵草依赖：yun_ding_lan / xuan_gen_wei / ying_yuan_gu / xuan_rong_tai / yuan_ni_hong_yu，`botany/registry.rs:181-223`）；`EnvLayerSampler` trait 接口不变。
2. `sky_island_base_y/thickness` 二进制层删除（与 #1 的层清单收口一致）：`env_sky_island()` 改由 span 列派生（取高位悬空段的 floor/ceiling），签名不变、消费方（`env_lock.rs:136-146` SkyIslandMask Top/Bottom 判定）无感。
3. 拒绝纯几何查询方案（遍历 spans 推断 tier）：热路径成本 +25% 且丢失 tier 离散分类语义。P0 等价对拍：同 seed 下 5 种灵草生成位置新旧一致。

**落点**：`server/src/botany/env_lock.rs:136-150` / `botany/registry.rs:181-223`（定义不动）/ `worldgen/scripts/terrain_gen/fields.py`（mask + tier 层保留）/ plan P0。

## §10 实施工作流（骨架预划，升 active 时按 docs/CLAUDE.md §六 展开）

- scope 估 **10~14 PR**，单 plan 多 PR 序列化（§6.3），大致拆分：P0 拆 Python/Rust 两 PR 起步，P1~P7 各 1~2 PR，依赖顺序 = 阶段顺序（P5 先于 P6；P1 尽早以服务后续视觉验收）
- 每 PR 独立 subagent 实施（§6.4 配置），主线只管 merge 与 CR 等待（§6.5 ScheduleWakeup 协议）
- 视觉交付（P3 景观 profile、P6 NBT 资产）强制三轮打磨 + `<PROMISE>`（§6.1），console 截图为 round 证据
- push 前对峙自检 workflow（P3+ 阶段替换 Verify，对齐 memory feedback_consume_presubmit_debate）
- 升 active 前置：✅ §8 已全部收口为 §8.1 决议（2026-06-12，7 份 Explore 调研：terrain 消费方爆炸半径 / 灵气两层关系 / NBT+背包现状 / span 编码与 surface 语义 / E 键与 give 链路 / NBT 选型与性能基线 / 灵气口径与 CI 适配面）
