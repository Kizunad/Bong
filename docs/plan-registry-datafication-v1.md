# plan-registry-datafication-v1 — 内容注册表数据化：手搓配方 / 功法元数据 / 方块名映射迁数据文件

> **一句话主题**：把三处最挡"横向扩内容"的硬编码注册表迁成扫盘数据文件——craft 手搓/制作台配方（pin 测试锁定 90 条 + 5 条 legacy 的 Rust 元组表）、功法元数据（49 条 const 数组）、terrain 方块名映射（`blocks.rs` + `raster.rs` 孪生双份 match）——**零新系统、零 wire 改动，有效数据的运行时语义零变化**（唯一有意变更：无效引用从运行时静默失败改为启动期 fail fast，错误契约见 P2），只搬装载来源不动消费方，让"加一条内容 = 加一个数据条目"的覆盖面从物品/丹方/锻造蓝图扩到配方/功法/地形材质。

**状态**：Finished（P0/P1/P2 已按各自最终主线合入提交验收；P3 为 2026-07-27 范围裁决；实施以 §8.1 决议为准）。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | craft 配方数据化——workbench 90 条（pin 锁定）+ legacy 5 条 → `assets/craft/recipes/*.toml` 扫盘 + 对拍回归门 | ✅ 2026-08-06 |
| P1 | 功法元数据数据化——`TECHNIQUE_DEFINITIONS` 49 条 → TOML + 双向 wiring 启动校验 | ✅ 2026-08-23 |
| P2 | 方块名映射查表化——`blocks.rs` + `raster.rs` 孪生表合一 + manifest 引用启动期 fail-fast（替代静默丢材质） | ✅ 2026-08-08 |
| P3 | 范围裁决项——矿物 registry / NPC 原型默认掉落 / 丹道 6 方包装（§8.1 #5 已裁决为本 plan 不实施） | ✅ 2026-07-27 |

---

## 背景与调研结论（2026-07-18，4 路 Explore 实证）

数据驱动已是仓库主流：物品 TOML 递归扫盘（`inventory/mod.rs:1634` `load_item_registry` → `:1695` `collect_item_toml_paths`）、锻造 JSON（`forge/blueprint.rs:343` `load_dir`）、炼丹 JSON（`alchemy/recipe.rs:294`）、加工 TOML（`lingtian/processing.rs:273`）、护甲 JSON（`combat/armor.rs:198`）、种族 JSON（`body_plan/race_registry.rs:267`）、zones JSON（`world/zone.rs:225`）——加一条 = 加一个数据条目。

三处逆流硬编码（本 plan 靶子）：

1. **craft 配方**：`craft/workbench_recipes.rs:79` `register_workbench_recipes` 十组子函数 Rust 元组表——真实规模以 pin 测试为准：`register_workbench_recipes_succeeds`（`:1380-1387`）断言 `registry.len() == 90`（89 workbench/coffin + 1 台自身），`workbench_recipe_count_by_group`（`:1488-1501`）另锁 `workbench.*` 精确 86 条；`:78` 头部注释自报 99 条已过期（经济僵尸清理后未同步——这条注释漂移本身就是硬编码表的病征）。另有 `craft/mod.rs` `register_examples` 5 条 legacy 手搓（eclipse_needle_iron / poison_decoction_fan / fake_skin_light / zhenfa_trap_iron / herb_knife_iron）。明明 forge / alchemy / processing 三兄弟全是扫盘数据，唯独手搓是代码，风格孤例。
2. **功法元数据**：`cultivation/known_techniques.rs:59` `TECHNIQUE_IDS: [&str; 49]` + `:158` `TECHNIQUE_DEFINITIONS: [TechniqueDefinition; 49]`——display_name / grade / required_realm / required_meridians / required_race / qi_cost / cast_ticks / cooldown / range / icon / category 全部 const 写死。加一条功法要动 3-4 处 Rust，是全仓扩内容成本最高的注册表。
3. **方块名映射（孪生双份）**：`world/terrain/blocks.rs` `block_from_name` match 体 ~247 行（`blocks.rs:17-263`），且 `raster.rs:1259` 还有一份同语义镜像 `block_state_from_name`（blocks.rs 头注自述 mirrors 关系）——两份手工同步的 match。真实消费方：`flora.rs:56/:524`、`raster.rs`、`structures.rs`、`nbt_io.rs`、`nbt_registry.rs`、`cmd/dev/gallery.rs`。worldgen surface_palette / DecorationSpec.blocks 引入新方块名不在表内 = 运行时静默返回 `None` 丢材质，跨 Python→Rust 的隐性契约，历史上最易漏的扩内容卡点。

关键有利事实：`CraftRegistry`（`craft/registry.rs:17`）本身是 `register()` 式 HashMap，与装载来源天然解耦——只换喂入方式，registry / session / unlock / UI 分组逻辑全不动。

## 接入面（docs/CLAUDE.md §二 checklist）

- **进料**：装载模式范本 = `lingtian/processing.rs:273` `load_default`（TOML 扫盘）/ `forge/blueprint.rs:360`（JSON read_dir）；被迁数据源 = `workbench_recipes.rs` 元组表 / `craft/mod.rs` `register_examples` / `known_techniques.rs` const 数组 / `blocks.rs` match。
- **出料**：运行时消费方**零接口变化**——`CraftRegistry::grouped_for_ui`、`SkillRegistry` resolver 查找（`cultivation/skill_registry.rs:79`、`:104` `init_registry`）、方块名解析的全部消费点（`flora.rs` / `raster.rs` / `structures.rs` / `nbt_io.rs` / `nbt_registry.rs` / `cmd/dev/gallery.rs`），全部不动。
- **共享类型 / event**：`CraftRecipe` / `TechniqueDefinition` / `BlockState` 结构不变；新增仅 `*Toml` 反序列化中间结构（`deny_unknown_fields`，对齐 `ItemTemplateToml` 惯例 `inventory/mod.rs:2285`）。
- **跨仓库契约**：零 wire / proto / schema 改动；client 零改动（`SkillIconIds` 按 id 约定解析，元数据外置不影响）；agent 零改动。
- **worldview 锚点**：纯基建无新玩法；数据条目 display_name 仍受 §三 L63 命名禁词约束（loader 可顺带 lint，§8 #6）。
- **qi_physics 锚点**：qi_cost 数值只搬运不改，不新增常数不碰 ledger。

## P0 craft 配方数据化 ✅ 2026-08-06

- **主线落点**：`fa09f1406d7f967e03c2bd307632e594bbdb38af`（2026-08-06，PR #1906）；这是 PR-A 的最终主线提交，不只采用最初的迁移 commit。实际交付文件包括 `server/assets/craft/recipes/` 下 15 个 TOML、`server/src/craft/data.rs`、`server/src/craft/fixtures/legacy_p0_registrar.rs`、`server/src/craft/registry_datafication_p0_baseline.json`，以及 `craft/mod.rs` / `workbench_recipes.rs` 的生产接线。

- 新 `server/assets/craft/recipes/` 目录，TOML 格式（文件粒度 §8 #1）：字段镜像 `CraftRecipe`（id / category / display_name / materials / qi_cost / time_sec / output / unlock_sources / station / requirements）。time 以秒存储、加载时 ×20 ticks（对齐 `workbench_recipes.rs:8` 现注释惯例）。
- 新 loader `craft/data.rs`：`load_craft_recipes_from_dir` 启动扫盘 → 逐条 `registry.register()`。`deny_unknown_fields`；materials/output 引用的 item id 必须在 `ItemRegistry`（启动校验 fail fast）；重复 id 拒载（复用 `RegistryError::DuplicateId`）。
- 迁移范围：`register_workbench_recipes` 全部十组 + `register_workbench_self_recipe` + `HANDCRAFT_STONE_TOOLS` station 覆写语义（`:21`，TOML 里直接写 `station = "none"`）+ `craft/mod.rs` `register_examples` 5 条。**流派 plan 在自己 P0 内 code-register 的招式配方不迁**（§8 #2）。
- **对拍回归门（本 plan 核心测试策略）**：迁移 commit 前先落一个 test fixture——基线取 **P0 实施起点的实际 Rust 表**（脚本化 dump 当刻 register 结果；90 + 5 仅为 2026-07-18 参考值，防同批 plan-craft-chain-items-v1 先行加配方后字面数失效，一切数量断言取快照长度不写字面数）；迁移后断言 TOML 加载结果与快照**逐条相等** + 数量 pin 承接既有 `register_workbench_recipes_succeeds` / `workbench_recipe_count_by_group` 两 pin（随基线同步刷新），并顺带修正 `:78` 过期头注。既有 session / unlock / reclaim / UI 分组测试全绿不动（尤其 `session.rs:1744` 手搓无台可做 pin）。
- 饱和测试：坏 TOML 拒载（未知字段 / 重复 id / 引用不存在 item / 负数 qi / 零产出 / malformed TOML）+ 加载边界（空目录 / 目录不存在 / 文件扫描顺序无关性）——这些直接决定启动期是否**静默得到空 registry**，必须 fail fast 不许空转；失败断言必须携带文件路径 + recipe id，对拍失败必须同时输出期望值与实际值；`CraftCategory` / `UnlockSource` / `CraftStationKind` 每 serde 变体正反 sample pin。

## P1 功法元数据数据化 ✅ 2026-08-23

- **主线落点**：`73014399b540557df345f5d3203fb3493bc151ae`（2026-08-23，PR #1336）；PR 内的审查修补随最终合入提交收口。实际交付文件包括 `server/assets/cultivation/techniques.toml`、`server/src/cultivation/known_techniques.rs`、`skill_registry.rs`、`technique_mentor.rs`、`technique_observe.rs`、`technique_scroll.rs`、`burst_meridian.rs` 与 `first_hit_dash.rs`。当前生产入口以 `TechniqueRegistry::load_default` 和 `validate_startup_wiring` 为准，49 条是迁移兼容基线而不是生产上限。

- 新 `server/assets/cultivation/techniques.toml`：49 条全字段按现有 source order 迁移。resolver 函数指针**留 Rust**（`SkillRegistry` 注册模式不动——本 plan 只外置元数据，不外置行为）。
- 新 owned `TechniqueRegistry` Resource（有序 `Vec<TechniqueDefinition>` + `id → index`），保持 NPC 同 seed 选招与命令展示的原顺序；系统消费方取 `Res<TechniqueRegistry>`，纯函数显式收 `&TechniqueRegistry`。玩家持久化 `KnownTechniques { id, proficiency, active }` 与 `KnownTechniquesLoadFailed` 写保护不动。详见 §8.1 #3。
- **分类 wiring 启动校验（fail fast，防孤岛）**：不能把 metadata 49 条与 resolver 68 条强行做双向全等。loader 对 metadata 条目显式标记 `metadata_backed` / `direct_generic` / `dedicated_input`；`metadata_backed` 必须存在 resolver，`direct_generic` 必须命中真实通用完成消费者，`dedicated_input` 必须命中 code-owned 专属输入 consumer registry，且 registry 中每个代码拥有的专属输入 ID 都必须反向存在于 TOML 并标成 `dedicated_input`。resolver-only 的 22 条由所属 subsystem 持有，不反向要求本表元数据。所有 `SkillRegistry ∩ TechniqueRegistry` 条目仍必须在**完整同步构造完成后**有 `SkillMeridianDependencies` 声明；metadata 的 `min_health` 不与仅存 `MeridianId` 的 deps 表做伪字段相等。详见 §8.1 #3。
- 与 **plan-skill-av-relink-v1（active）** 协调：图标链防回归测试（#1220，skill_scroll 单一真相源）以 icon id 为锚——元数据外置**不得改任何 icon id 语义**，迁移后该测试族必须原样全绿。
- 对拍回归门同 P0：旧 const 数组 canonical 快照 == TOML 加载结果逐条相等；数量从快照长度派生，不在迁移后测试中另写一份 49 条真源。realm / race gate / category 枚举字符串每变体正反 serde sample。

## P2 方块名映射查表化（孪生表合一）✅ 2026-08-08

- **主线落点**：`4691f972c0223037ffa9423eed6f28933d378add`（2026-08-08，PR #1890）。实际交付文件包括 `server/assets/worldgen/block_catalog.toml`、`world/terrain/blocks.rs`、`blocks_legacy_oracle.rs`、`raster_legacy_oracle.rs`、`raster.rs`、`flora.rs`、`structures.rs`、`nbt_io.rs`、`nbt_registry.rs` 与 `terrain/mod.rs`。
- **两份 match 已同时收编**：当前 canonical 入口是 `world/terrain/blocks.rs::BlockCatalog::load` / `block_from_name`（当前约 `blocks.rs:21,293`）；catalog 有 213 个 logical key，其中 211 个 direct，显式 alias 为 `glowshroom → shroomlight`、`iron_nugget → air`。`raster.rs::block_state_from_name`（当前约 `raster.rs:2047`）只转调 canonical resolver，生产侧 39-arm 镜像已删除；`raster_legacy_oracle.rs` 仅保留 test-only 对拍。
- **六个消费面逐一核验**：`flora.rs` 使用预解析的 `resolved_blocks`；`raster.rs` 负责 surface/decoration/placement lowering；`structures.rs` 消费已 lower 的 `BlockState` placement；`nbt_io.rs::PaletteEntry::block_state` 走统一 property lowerer；`nbt_registry.rs::DecorationNbtPreflight` / `palette_diagnostics` 参与启动预检；`cmd/dev/gallery.rs::structure_placements` 复用同一 NBT palette lowering。
- **启动期 fail-fast 已真实接入**：`TerrainProvider::load` / `load_preflighted` 与 `terrain/mod.rs::prepare_raster_bootstrap_with_nbt_preflight` 在构造 provider 前汇总 surface、decoration、NBT、placement 的未知 block、property 和 template 诊断。`invalid_cross_source_manifest` 与 `load_preflighted_aggregates_surface_decoration_template_and_placement_errors` 负例确认一轮列出全部未知名；`block_state_from_placement_rejects_unknown_blocks_and_properties` 和 NBT palette diagnostics 覆盖属性/索引错误。
- **现网数据核验**：合入后的 `scripts/dev-reload.sh` 全链与 raster 后验均通过；overworld 为 `306/306` tiles、TSY 为 `9/9`，随后 server 启动预检加载 `306` 个 overworld tiles、`9` 个 TSY tiles，无新的启动失败。原决议中的行号已漂移，本节以当前符号名和上述复核后的行号为准。

## P3 范围裁决项 ✅ 2026-07-27

§8.1 #5 已裁决：以下三项全部 out-of-scope，本 plan 不实施、不顺手改代码；归档证据登记为后续独立验真/立项候选：

- 矿物 registry：`mineral/registry.rs:80` `build_default_registry` 手写 18 条（跨 `MineralId` 枚举 + registry + `minerals.toml` + `mineral_anchors.json` 四处）。
- NPC 原型默认掉落：`npc/loot.rs:63` `default_loot_for_archetype` 静态 match 表（世界/TSY/宗门掉落均已 JSON，唯此硬编）。
- 丹道 6 方包装：`dandao/recipes.rs:41` `fn dandao_recipes() -> [DandaoRecipeSpec; 6]` 定长数组构造函数（引用 alchemy JSON 但包装层硬编）。

---

## §8 开放问题（已于 §8.1 收口）

1. **数据文件粒度**：craft 单文件 vs 按十大类分文件（倾向按类分，对齐 alchemy 一方一文件的可 review 性）；techniques 同题（倾向按 `SkillCategory` 分）。
2. **流派 code-register 配方是否迁**：倾向不迁——dugu-v2 / tuike-v2 / zhenfa 等配方与 skill plan 生命周期绑定，迁了反拆散 plan 内聚；本 plan 只迁 workbench + legacy example，需在 P0 里 grep 确认边界清单。
3. **known_techniques 调用方处理**：保留门面函数（签名不变内部查 Resource）vs 全量改调用方——grep 调用面大小后定，倾向调用面 >10 处则留门面。
4. **方块名方案选型实证**：valence `BlockKind::from_str` 可用性（match 体 `blocks.rs:17-263` 里有多少带 props 的非直映射条目必须保留特例）；与孪生表 `raster.rs::block_state_from_name` 的现状差集清点；生成查表脚本是否值得。
5. **P3 三项去留拍板**（矿物 / NPC 掉落 / 丹道包装）。
6. **数据 lint**：loader 测试是否顺带做 worldview §三 L63 禁词 lint（display_name 扫 玄/陨/星/仙/太/古）——低成本高护栏，倾向做。
7. **与 plan-craft-chain-items-v1（同批 skeleton）顺序协调**：若本 plan P0 先 merge，物品 plan 的新配方直接落 TOML；反之先落 Rust 表行、本 plan 迁移时一并搬。**两 plan 不得同时改 `workbench_recipes.rs`**（merge conflict 高危区，历史上并行 PR 改同一表已叠出过重复字段编译错）。

全部已在 §8.1 收口。原表保留以备追溯，**实施时以 §8.1 决议为准**。

## §8.1 决议（pre-P0 收口，2026-07-27）

### #1 数据文件粒度

**决议**：
1. craft 用 `server/assets/craft/recipes/*.toml` 多文件扫盘，按现 `workbench_recipes.rs` 的领域组拆分；每个文件可含多条 `[[recipes]]`，兼顾 review 可读性与避免约百个碎文件。
2. techniques 只有 49 条且顺序属于运行时契约，使用单一 `server/assets/cultivation/techniques.toml`，文件内 `[[techniques]]` 顺序即 registry 顺序；禁止按 category 合并后重排。
3. 两类 loader 均递归发现、按路径排序、`deny_unknown_fields`、空目录/无匹配文件/坏文件 fail fast；解析与跨引用 preflight 全通过后才原子写入 registry。

**落点**：`server/src/craft/mod.rs:88-127`、`server/src/craft/registry.rs:40-88`、`server/src/cultivation/known_techniques.rs:67-166`；plan P0/P1。

### #2 craft 迁移边界

**决议**：
1. 本 plan 只迁 `register_workbench_recipes` 的实际生产集合与 `register_examples` 5 条；数量由实施起点旧 registrar canonical dump 推导，不信过期注释，也不在新 loader 测试里另写字面总数。
2. anqi/zhenfa/tuike/poison/armor/gathering/basic-processing/coffin 等所属模块的 code-register 配方继续留 Rust；它们保持原注册顺序，数据配方在同一 `CraftRegistry` 内按现顺序接入。
3. loader 必须对材料、产出和 scroll 解锁物品查 `ItemRegistry`；mentor archetype 当前没有稳定且启动顺序兼容的单一 registry，本阶段只做非空校验，不伪造跨表校验。

**落点**：`server/src/craft/mod.rs:88-127,157-280`、`server/src/craft/workbench_recipes.rs:79-1360`、`server/src/inventory/mod.rs:1634-1715`；plan P0。

### #3 功法 runtime API 与 wiring 范围

**决议**：
1. `KnownTechniques` 玩家持久化形状和 load-failed 写保护不变；元数据改成 owned `TechniqueDefinition` / `TechniqueRequiredMeridian` / `RaceGateOwned`，由 `TechniqueRegistry(Resource)` 持有有序 `Vec` + id 索引。系统注入 `Res<TechniqueRegistry>`，非 ECS helper 显式接收 registry；不使用 `Box::leak` 或 `OnceLock` 全局兼容层，避免全局 fixture 污染和第二真源。
2. 现 `TECHNIQUE_DEFINITIONS` 与零参 `technique_definition(id) -> &'static` 无法在启动数据上自然保留；“消费接口不变”解释为外部可观察 registry 查询/顺序/payload 语义不变，而不是强保不可能的 `'static` 内部签名。所有生产调用方在本 PR 同步机械迁移到 registry 借用，wire/schema/client/agent 零改动。
3. `SkillRegistry` 现实是 68 resolver，而 metadata 为 49：交集 46；metadata-only/direct 3 条为 `movement.dash`、`shield_block`、`body.guangbo_ticao`，其中前两条是独立 C2S/gameplay consumer 的 `dedicated_input`，后者是通用 cast 完成消费者的 `direct_generic`；resolver-only 22 条由 Yidao/Woliu 侵蚀/Dugu v2/Baomai extra/Dandao 等 subsystem 自持。因此校验按 `metadata_backed` / `direct_generic` / `dedicated_input` 分类，不做必红的集合全等；专属输入 consumer registry 还需对代码 ID→TOML metadata 做双向校验。
4. `SkillMeridianDependencies` 必须由单一同步 builder 构造完整后再校验，不能在 `cultivation::register` 仅插入首批声明时抢跑；声明表只表示 channel ID 集，metadata 另含 `min_health`，不做伪全字段相等。保留并强化“交集条目必须显式 declared”的不变量；`declare` 重复覆盖应改为拒绝重复，空声明与未声明继续通过 `is_declared` 区分。

**落点**：`server/src/cultivation/known_techniques.rs:24-166,1117-1121`、`server/src/cultivation/skill_registry.rs:79-123,217-293`、`server/src/cultivation/meridian/severed.rs:423-446`、`server/src/cultivation/mod.rs:216-255`；plan P1。

### #4 方块孪生表合一与属性边界

**决议**：
1. `blocks.rs` 生产 catalog 现有 213 个 logical key；`raster.rs` surface fast-path 39 个且严格为其子集（差集 174 / 0）。删除 39-arm 镜像，surface 也统一走 canonical resolver。
2. catalog 仍只开放现有 213 logical key，不直接接受全部 vanilla `BlockKind`，避免无意扩大 worldgen 内容契约；213 项中 211 项可由 `BlockKind::from_str(name).to_state()` 解析，两项显式 alias 保留：`glowshroom → shroomlight`、`iron_nugget → air`。
3. NBT/placement 的 properties 继续走统一 property lowerer；启动校验必须汇总未知 block 名、未知 property 名/值、以及属性不适用于该 block 三类错误，不再静默丢弃。有效属性后的 `BlockState` 结果与旧路径逐条对拍。
4. `TerrainProvider::load` 在 manifest/sidecar/NBT 解析后、构造 provider 前用排序集合一次性报告 surface palette、decoration blocks、NBT template/palette/property、placement block/property 的全部错误；缺失旧 placement sidecar 的兼容语义不变，但 sidecar 存在而损坏必须报错，不能降级为空。

**落点**：`server/src/world/terrain/blocks.rs:17-263`、`server/src/world/terrain/raster.rs:850-994,1401-1453,1480-1573`、`server/src/world/terrain/nbt_io.rs:90-150`、`server/src/world/terrain/nbt_registry.rs:223-240`；plan P2。

**实施后符号核对（2026-09-10）**：上述决议行号已漂移；当前应以 `BlockCatalog::load` / `block_from_name`、`TerrainProvider::load` / `load_preflighted`、`PaletteEntry::block_state`、`DecorationNbtPreflight` 和 `structure_placements` 为准，分别落在 `blocks.rs`、`raster.rs`、`nbt_io.rs`、`nbt_registry.rs` 与 `cmd/dev/gallery.rs`。`raster.rs::block_state_from_name` 仍存在但只是 canonical resolver 适配器，不是旧的 39-arm 镜像。

### #5 P3 三项去留

**决议**：
1. 矿物 registry、NPC 原型默认掉落、丹道 6 方包装全部排除在本 plan 与本 PR；用户明确委托的原子范围是 craft/technique/block 三类硬编码表。
2. P3 标为“不实施/后续另立”，不向 `reminder.md` 写入（本 PR 的 docs 权限只消费本 plan，且三个主题应先各自验真再立轨道）。
3. 归档证据中登记三项为 out-of-scope，不得为了凑阶段顺手迁移。

**落点**：`server/src/mineral/registry.rs:80`、`server/src/npc/loot.rs:63`、`server/src/dandao/recipes.rs:41`；plan P3。

### #6 worldview 名称 lint

**决议**：
1. 不在通用 loader 中新增“玄/陨/星/仙/太/古”机械拒绝；这些字在既有正典内容、专名或迁移快照中可能合法存在，基础设施 PR 不应借迁移改变有效内容语义。
2. 本 plan 通过旧表→新数据逐条 snapshot equality 保证名称零改动；未来新增内容的 worldview review 仍由正典检查和所属玩法 plan 负责。

**落点**：`docs/worldview.md §三 L63`、plan P0/P1 对拍回归门。

### #7 并行冲突与实施形态

**决议**：
1. 实施期间每次修改旧配方表前先 `git fetch origin` 并检查开放 PR/最新 main 是否触碰 owned files；若有并行变更，先 merge 后重新生成 canonical baseline，禁止手工拼旧快照。
2. 本次按用户授权的固定 claim 分支 `refactor/plan-registry-datafication-v1` 走**单 PR**闭环；P0/P1/P2 用独立 atomic commits 分层，不沿用骨架原“固定 3 PR”草案。
3. 所有 cargo 命令在 `scripts/build-token.sh` 未落 main 前经 `flock /tmp/bong-cargo.lock -c "cargo ..."`；push 前紧邻执行 `git fetch origin && git merge origin/main`，任何合入变更后重跑完整 server 门与 P2 worldgen 门。

**落点**：本 plan §10、用户本轮 claim/worktree/gate 协议；plan P0-P2。

## §10 实施工作流（单 PR，三阶段 atomic commits）

1. **P0 craft**：先从旧 `register_examples + register_workbench_recipes` 生成 canonical fixture，再落 TOML/loader/启动引用校验，逐条对拍后删除生产硬编码来源；其余模块 code-register 不动。
2. **P1 technique**：先从旧 49 条 const 生成有序 canonical fixture，再落 owned `TechniqueRegistry`、迁移调用方与分类 wiring 校验；保住 NPC source order、icon/cast/race payload 快照。
3. **P2 block**：先锁 213 catalog 与 39 fast-path 的逐条结果，再以 catalog + `BlockKind::from_str` + 两 alias 合一 resolver，删除 raster 镜像，补 manifest/NBT/placement 汇总 fail-fast。
4. 每阶段独立中文 atomic commit；每个 commit 均带 `Model: gpt-5.6-sol-xhigh` 与 `Co-Authored-By: Claude <noreply@anthropic.com>` trailer。纯 server 基建，无视觉资产三轮要求。
5. 每阶段先跑 targeted tests；push 前跑 `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`（全部经 flock），P2 追加 `bash scripts/dev-reload.sh`。管道不得吞退出码。
6. push 前 `git fetch origin && git merge origin/main`；merge 带入任何变化则重新执行完整门禁，并针对最终 HEAD 启动 fresh-context、read-only、首步 SHA 对拍的对抗 validator。任何 HEAD 变化都使旧 PASS 失效。
7. plan 全阶段完成后更新状态、补 `## Finish Evidence` 并迁入 `docs/finished_plans/`；push 固定 claim 分支，中文 PR 标题/body 含完整 `plan-registry-datafication-v1`，独立评论 `/review`，等待 e2e 与 review 收敛。

## Finish Evidence

**落地清单**：

- **P0 craft 配方数据化**：`server/assets/craft/recipes/` 收编 90 条 workbench/coffin/self 与 5 条 legacy 配方；`server/src/craft/data.rs` 实现递归、有序、`deny_unknown_fields` 的 TOML loader，并在原子提交前校验重复 ID、材料/产出/卷轴物品引用、数值和文件类型；`server/src/craft/fixtures/legacy_p0_registrar.rs` 与 `registry_datafication_p0_baseline.json` 冻结迁移前 95 条注册结果，逐字段对拍保持 station、unlock、材料顺序和运行时语义。
- **P1 功法元数据数据化**：`server/assets/cultivation/techniques.toml` 是当前 metadata 集合的唯一真源；checked-in 49 条只作为迁移兼容基线，不是生产数量上限。`server/src/cultivation/known_techniques.rs` 落地 owned `TechniqueRegistry`（source-order `Vec` + ID index），`known_techniques_legacy_oracle.rs` 逐字段锁旧表并验证其为有序兼容子序列。`cultivation::register` 在资源可见前按当前数据逐条动态校验：`metadata_backed` 必须有同 ID resolver 与显式经脉依赖声明，`direct_generic` 必须有真实的通用完成消费者，`dedicated_input` 必须命中 code-owned dedicated-input consumer registry；同时反向校验每个代码拥有的专属输入 ID 都存在于 TOML 且明确标成 `dedicated_input`。resolver-only、dependency-only 以及 direct-generic 的可选 dependency 均合法。历史 68 resolver / 49 metadata / 46 交集 / 3 direct-generic / 22 resolver-only 仅是迁移时事实，不再参与生产 admission。生产调用方全部显式借用 registry，未新增全局/static 兼容门面。
- **P2 方块目录与启动预检**：`server/assets/worldgen/block_catalog.toml` 的 checked-in 213 logical key（211 direct + `glowshroom -> shroomlight`、`iron_nugget -> air` 两 alias）是迁移兼容基线，不是生产上限；TOML 可只改数据新增任意可 lower 的 direct，或新增指向同文件 direct 的 alias。`world/terrain/blocks.rs` 两阶段解析当前声明集合，允许 forward alias，拒绝未声明 target、合法但未列入 catalog 的 vanilla target、self/alias-chain/空/namespaced target，并保持 catalog miss 不回退任意 vanilla block。`blocks_legacy_oracle.rs` 将旧 213 项锁为同值有序兼容子序列，`raster_legacy_oracle.rs` 继续锁定旧 39-key fast-path 子集；`raster.rs` 的 39-arm 镜像已删除。`nbt_io.rs`、`nbt_registry.rs`、`terrain/mod.rs`、`world/mod.rs` 将 overworld/可选 TSY、surface palette、decoration NBT、placement sidecar、属性和值及 raster palette 边界集中预检，全部通过后才构造 provider/layer；有效 worldgen 生产 manifest 的已知元数据显式接纳，未来未知字段继续被 `deny_unknown_fields` 拒绝。同步修复 `frost_cluster_v3.nbt` 中误施于 `blue_ice` 的 `facing=up` 属性。
- **P3 范围裁决**：按 §8.1 #5 保持矿物 registry、NPC 原型默认掉落、丹道 6 方包装原状；本 PR 不顺手扩 scope。

**关键 commit**：

- `fa09f1406d7f967e03c2bd307632e594bbdb38af`（2026-08-06）：PR #1906 / P0 最终主线合入，配方 TOML、loader、迁移 oracle 与生产接线落地。
- `4691f972c0223037ffa9423eed6f28933d378add`（2026-08-08）：PR #1890 / P2 最终主线合入，block catalog、canonical resolver、双表对拍与 terrain/NBT 启动预检落地。
- `73014399b540557df345f5d3203fb3493bc151ae`（2026-08-23）：PR #1336 / P1 最终主线合入，TechniqueRegistry、TOML 元数据、调用方迁移与动态 wiring 校验落地。
- `2263ae943bd69c4d1a66ac6a40c45d8a9679048b`（2026-07-29）：第一次正常归档本 plan；它是文档流转证据，不是三阶段实现提交。
- `3caaf02b7cb40e48f59beda38d0d5e6ac91746ee`（2026-08-20）：PR #1315 分支基于旧归档状态重新创建 finished 副本；它解释了重复文件来源，不代表第二次 P1 实现。

**测试结果**：

- fresh-context validator 在 `79efaf56b4fd552a97b5fa72086c000598f4ac40` 发现迁移实现仍以固定 count/ID/alias/fingerprint 拒绝合法数据扩展；上述两个 follow-up commit 将迁移数字降为 test-only compatibility evidence，并以 data-only 扩展正向测试及严格引用反向矩阵锁定动态生产契约。
- 历史 targeted Rust 门（非本次返工 exact head）：block catalog `14 passed`；known techniques `13 passed`；无 resolver 的已定义 skill-bar cast generic fallback `1 passed`。本次返工的 exact-head targeted 请求在编译阶段被 sandbox SIGKILL，未运行这些测试。
- PR #1315 首次 e2e 在 canonical Bot novice raster 启动预检命中 `unknown field surface_y, expected kind or token`：Python producer 与 bot scenario 一直以 `kind/token/surface_y/support/feet_y/head_y` 六字段承载可独立核验的 raster 证据，而 Rust 嵌套 `deny_unknown_fields` schema 只声明前两项。`0f517547c` 将其修正为六字段均 required 且保持精确 JSON 类型，继续拒绝缺字段、错类型与未来未知字段；运行时 `BotRasterFixture` 仍只持有并发布自己消费的 `kind/token`。
- Bot schema 修复 targeted 门：Rust `bot_fixture_metadata_is_optional_and_validated_before_ready_use` → `1 passed`；canonical Python `scripts.bot.test_protocol.NoviceRasterFixtureTest` → `4 passed`；真实 producer 生成的 fixture 经 server 可执行文件启动 45 秒（预期由 `timeout` 以 124 结束）无 panic，日志确认 `loaded 5 terrain tiles / 6 POIs`、`BOT_RASTER_FIXTURE_READY`、`decoration NBT registry: 54 templates resident`。
- Bot schema 修复后完整 server 门（`0f517547c`）：`flock -x /tmp/bong-cargo.lock bash -lc 'cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test'` 全绿——library `12191 passed / 0 failed / 2 ignored`（共发现 `12193`）；CLI `12 passed`；full-app startup `2 passed`；shutdown-signal Rust integration `2 passed`；Tarkov backpack e2e `4 passed`；doc tests `5 ignored`。
- 历史 GitHub e2e run `30532612538` 在 `b279791e` 全绿（非本次返工 exact head），证明 Bot schema 漂移已从当时真实集成路径消失；其后的历史 validator 发现并修正启动探针契约。`e88200146` 改为对当前非空、唯一集合的动态断言；`e4da30aa2` 用复制后的完整 assets 根追加历史 data-only `direct_generic` 正例；本次返工 exact head 的 resolver-backed acceptance 仅有代码回归测试，因 sandbox SIGKILL 未执行，不宣称其测试通过。
- 历史启动探针完整 server 门（`e4da30aa2`，非本次返工 exact head）：同一 flock 命令全绿——library `12191 passed / 0 failed / 2 ignored`（共发现 `12193`）；CLI `12 passed`；full-app startup `3 passed`；shutdown-signal Rust integration `2 passed`；Tarkov backpack e2e `4 passed`；doc tests `5 ignored`。其中历史 data-only 正例现由最终 boundary 的 resolver-backed 正例替代，consumerless direct_generic 另有拒绝正例；本次返工未重新取得完整 server 门 verdict。
- 修复后 worldgen 合同：`python -m unittest worldgen.tests.test_decoration_contract worldgen.tests.test_nbt_block_palette` → `22 passed`。
- 修复后完整 raster 重新生成与后验：overworld `306 tiles / 84 POIs / 112 decorations / 138969 placements`、TSY `9 tiles / 56 POIs`；`validate_rasters` 分别为 overworld `306/306`、TSY `9/9` 全部通过。
- 修复后可执行文件启动预检：经 `/tmp/bong-cargo.lock` 执行 `cargo build`，以生成的 overworld + TSY manifests 及 `BONG_SKIP_SKIN_PREFETCH=1` 启动 30 秒无 panic；日志确认 `loaded 306 terrain tiles`、`loaded TSY 9 terrain tiles`、`decoration NBT registry: 54 templates resident`。
- **本次 VRFY 返工 exact-head 证据（代码提交 `6e3e5ccb010273d25d18dfff3df84b21695bdb99`）**：`execution-pr1315-technique-boundaries` 已 terminal `failed`（exit 1），请求在 `scripts/build-token.sh cargo fmt --check` 处停止，sandbox stable toolchain 缺少 `cargo-fmt`，因此没有产品测试结果；`execution-pr1315-targeted` 已 terminal `failed`（exit 101），首个 targeted `cargo test cultivation::known_techniques::tests` 在编译 `bong-server` lib 时被 2 CPU / 4 GiB sandbox 以 SIGKILL（signal 9）终止，测试 body 尚未运行，后续命令因 `set -e` 未开始。两项均为基础设施终止，不据此宣称产品失败或通过。
- 本次返工只保留上述两份 host execution 终端证据；没有 fresh validator 运行，也不存在本次 exact head 的 validator PASS SHA。最终静态 `git diff --check` 通过；cargo fmt、Rust targeted tests、完整门禁和 e2e 均没有本次 exact head 的 PASS verdict。
- 以上修复后门禁完成后紧邻执行 `git fetch origin && git merge origin/main`；如 fetch/merge 带入任何变更，必须重新运行受影响门禁并重新绑定 validator，不能沿用旧 SHA。
- **本地安全隔离**：未运行 `scripts/test-tmux-shutdown-order.sh`、`scripts/test-server-lifecycle.sh`、`scripts/smoke-test-e2e.sh` 或任何会调用 quarantined shutdown 路径的本地 suite；该覆盖留给 GitHub e2e。

**跨仓库核验**：

- server：`craft::data::load_craft_recipes_from_dir` / `cultivation::known_techniques::TechniqueRegistry` / `validate_startup_wiring` / `world::terrain::blocks::BlockCatalog` / `prepare_raster_bootstrap_with_nbt_preflight` 全部接入生产 startup 与既有消费方。
- worldgen：`test_decoration_contract`、`test_nbt_block_palette` 读取同一 `block_catalog.toml`，生产导出的 overworld/TSY manifests 均通过 Rust 启动预检。
- client / agent / schema：本 plan 无 wire、proto、schema 或客户端行为变更；`origin/main...HEAD` 无 `client/`、`agent/` 路径改动，功法 ID、icon ID、cast/race/category payload 由旧表对拍保持不变。

**遗留 / 后续**：

- **重复 plan 处置**：`2263ae943` 已将 active 正常归档；之后 `fa09f1406`（PR #1906）重新带回旧 active 文件，`3caaf02b7`（PR #1315）又带入另一份 finished 文件。两份内容和状态不同，且 active 只有 157 行旧 PR-A 计划、finished 才包含完整 Finish Evidence；本次只删除 `docs/plan-registry-datafication-v1.md`，保留并修正本文件，不覆盖任何代码或其它 plan。
- 矿物 registry、NPC 原型默认掉落、丹道 6 方包装仍按 P3 裁决留待各自独立验真/立项。
- `BONG_TSY_RASTER_PATH` 未配置时仍保持 overworld-only 合法；一旦配置，损坏或不完整 TSY 数据会按本 plan 的严格启动契约 fail fast。
- §8.1 #3/#4 与 P1/P2 中的 49/68/46/22/3、213/211/2 数字保留为迁移时历史背景；生产 admission 以当前 TOML 和当前 runtime registry 的动态契约为准，不得重新把这些数字或历史 ID/alias 集合引入生产校验。
- 本 plan 不新增 gameplay、真元常数、ledger 流向或视觉资产；既有有效内容语义保持不变。
