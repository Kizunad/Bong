# plan-registry-datafication-v1 — 内容注册表数据化：手搓配方 / 功法元数据 / 方块名映射迁数据文件

> **一句话主题**：把三处最挡"横向扩内容"的硬编码注册表迁成扫盘数据文件——craft 手搓/制作台配方（pin 测试锁定 90 条 + 5 条 legacy 的 Rust 元组表）、功法元数据（49 条 const 数组）、terrain 方块名映射（`blocks.rs` + `raster.rs` 孪生双份 match）——**零新系统、零 wire 改动，有效数据的运行时语义零变化**（唯一有意变更：无效引用从运行时静默失败改为启动期 fail fast，错误契约见 P2），只搬装载来源不动消费方，让"加一条内容 = 加一个数据条目"的覆盖面从物品/丹方/锻造蓝图扩到配方/功法/地形材质。

**状态**：Active（2026-07-27 pre-P0 决议已收口，实施以 §8.1 为准）。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | craft 配方数据化——workbench 90 条（pin 锁定）+ legacy 5 条 → `assets/craft/recipes/*.toml` 扫盘 + 对拍回归门 | ⬜ |
| P1 | 功法元数据数据化——`TECHNIQUE_DEFINITIONS` 49 条 → TOML + 双向 wiring 启动校验 | ⬜ |
| P2 | 方块名映射查表化——`blocks.rs` + `raster.rs` 孪生表合一 + manifest 引用启动期 fail-fast（替代静默丢材质） | ⬜ |
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

## P0 craft 配方数据化 ⬜

- 新 `server/assets/craft/recipes/` 目录，TOML 格式（文件粒度 §8 #1）：字段镜像 `CraftRecipe`（id / category / display_name / materials / qi_cost / time_sec / output / unlock_sources / station / requirements）。time 以秒存储、加载时 ×20 ticks（对齐 `workbench_recipes.rs:8` 现注释惯例）。
- 新 loader `craft/data.rs`：`load_craft_recipes_from_dir` 启动扫盘 → 逐条 `registry.register()`。`deny_unknown_fields`；materials/output 引用的 item id 必须在 `ItemRegistry`（启动校验 fail fast）；重复 id 拒载（复用 `RegistryError::DuplicateId`）。
- 迁移范围：`register_workbench_recipes` 全部十组 + `register_workbench_self_recipe` + `HANDCRAFT_STONE_TOOLS` station 覆写语义（`:21`，TOML 里直接写 `station = "none"`）+ `craft/mod.rs` `register_examples` 5 条。**流派 plan 在自己 P0 内 code-register 的招式配方不迁**（§8 #2）。
- **对拍回归门（本 plan 核心测试策略）**：迁移 commit 前先落一个 test fixture——基线取 **P0 实施起点的实际 Rust 表**（脚本化 dump 当刻 register 结果；90 + 5 仅为 2026-07-18 参考值，防同批 plan-craft-chain-items-v1 先行加配方后字面数失效，一切数量断言取快照长度不写字面数）；迁移后断言 TOML 加载结果与快照**逐条相等** + 数量 pin 承接既有 `register_workbench_recipes_succeeds` / `workbench_recipe_count_by_group` 两 pin（随基线同步刷新），并顺带修正 `:78` 过期头注。既有 session / unlock / reclaim / UI 分组测试全绿不动（尤其 `session.rs:1744` 手搓无台可做 pin）。
- 饱和测试：坏 TOML 拒载（未知字段 / 重复 id / 引用不存在 item / 负数 qi / 零产出 / malformed TOML）+ 加载边界（空目录 / 目录不存在 / 文件扫描顺序无关性）——这些直接决定启动期是否**静默得到空 registry**，必须 fail fast 不许空转；失败断言必须携带文件路径 + recipe id，对拍失败必须同时输出期望值与实际值；`CraftCategory` / `UnlockSource` / `CraftStationKind` 每 serde 变体正反 sample pin。

## P1 功法元数据数据化 ⬜

- 新 `server/assets/cultivation/techniques.toml`：49 条全字段按现有 source order 迁移。resolver 函数指针**留 Rust**（`SkillRegistry` 注册模式不动——本 plan 只外置元数据，不外置行为）。
- 新 owned `TechniqueRegistry` Resource（有序 `Vec<TechniqueDefinition>` + `id → index`），保持 NPC 同 seed 选招与命令展示的原顺序；系统消费方取 `Res<TechniqueRegistry>`，纯函数显式收 `&TechniqueRegistry`。玩家持久化 `KnownTechniques { id, proficiency, active }` 与 `KnownTechniquesLoadFailed` 写保护不动。详见 §8.1 #3。
- **分类 wiring 启动校验（fail fast，防孤岛）**：不能把 metadata 49 条与 resolver 68 条强行做双向全等。loader 对 metadata 条目显式标记 `metadata_backed` / `direct_generic`；`metadata_backed` 必须存在 resolver，`direct_generic` 允许走统一非 resolver 入口；resolver-only 的 22 条由所属 subsystem 持有，不反向要求本表元数据。所有 `SkillRegistry ∩ TechniqueRegistry` 条目仍必须在**完整同步构造完成后**有 `SkillMeridianDependencies` 声明；metadata 的 `min_health` 不与仅存 `MeridianId` 的 deps 表做伪字段相等。详见 §8.1 #3。
- 与 **plan-skill-av-relink-v1（active）** 协调：图标链防回归测试（#1220，skill_scroll 单一真相源）以 icon id 为锚——元数据外置**不得改任何 icon id 语义**，迁移后该测试族必须原样全绿。
- 对拍回归门同 P0：旧 const 数组 canonical 快照 == TOML 加载结果逐条相等；数量从快照长度派生，不在迁移后测试中另写一份 49 条真源。realm / race gate / category 枚举字符串每变体正反 serde sample。

## P2 方块名映射查表化（孪生表合一）⬜

- **范围必须同时收编两份 match**：`blocks.rs::block_from_name`（match 体 `blocks.rs:17-263`）与镜像实现 `raster.rs:1259` `block_state_from_name`——合一为单一真相源后各消费点（`flora.rs` / `raster.rs` / `structures.rs` / `nbt_io.rs` / `nbt_registry.rs` / `cmd/dev/gallery.rs`）统一走新查表。只迁一份 = 静默丢材质风险原样保留 + 两表进一步失去同步，视为不合格交付。
- 方案 §8 #4 收口后定，倾向：valence `BlockKind::from_str` 兜底 + 极小特例映射（数据或常量表，覆盖带状态属性的非直映射条目），退路是脚本生成的静态查表。
- **静默 `None` → 启动期 fail fast（有意的失败语义变更）**：`TerrainProvider::load` 时把 manifest 携带的 surface_palette / decoration blocks 全量预解析，未知方块名启动即报错。错误契约：报错信息列出**全部**未知名及各自来源（palette 项 / decoration id / NBT 文件），触发条件 = manifest 引用的任一方块名不可解析；有效数据的运行时行为不变。发布策略：对拍测试保证现两表覆盖名全数可解析 + 合并前 `scripts/dev-reload.sh` 对现网 raster 全链过一遍，故现存数据不会触发新的启动失败。
- 饱和测试：两份现 match 覆盖的**全部名字新旧解析结果对拍**（一致性快照，含两表差集专项——若两表现状已有分歧条目，逐条裁决记录进 plan）；未知名报错路径；manifest 校验命中 / 漏配用例；`raster_check` 后验流程不受影响（`bash scripts/dev-reload.sh` 全绿）。

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
3. `SkillRegistry` 现实是 68 resolver，而 metadata 为 49：交集 46；metadata-only/direct 3 条为 `movement.dash`、`shield_block`、`body.guangbo_ticao`；resolver-only 22 条由 Yidao/Woliu 侵蚀/Dugu v2/Baomai extra/Dandao 等 subsystem 自持。因此校验按 `metadata_backed` / `direct_generic` 分类，不做必红的集合全等。
4. `SkillMeridianDependencies` 必须由单一同步 builder 构造完整后再校验，不能在 `cultivation::register` 仅插入首批声明时抢跑；声明表只表示 channel ID 集，metadata 另含 `min_health`，不做伪全字段相等。保留并强化“交集条目必须显式 declared”的不变量；`declare` 重复覆盖应改为拒绝重复，空声明与未声明继续通过 `is_declared` 区分。

**落点**：`server/src/cultivation/known_techniques.rs:24-166,1117-1121`、`server/src/cultivation/skill_registry.rs:79-123,217-293`、`server/src/cultivation/meridian/severed.rs:423-446`、`server/src/cultivation/mod.rs:216-255`；plan P1。

### #4 方块孪生表合一与属性边界

**决议**：
1. `blocks.rs` 生产 catalog 现有 213 个 logical key；`raster.rs` surface fast-path 39 个且严格为其子集（差集 174 / 0）。删除 39-arm 镜像，surface 也统一走 canonical resolver。
2. catalog 仍只开放现有 213 logical key，不直接接受全部 vanilla `BlockKind`，避免无意扩大 worldgen 内容契约；213 项中 211 项可由 `BlockKind::from_str(name).to_state()` 解析，两项显式 alias 保留：`glowshroom → shroomlight`、`iron_nugget → air`。
3. NBT/placement 的 properties 继续走统一 property lowerer；启动校验必须汇总未知 block 名、未知 property 名/值、以及属性不适用于该 block 三类错误，不再静默丢弃。有效属性后的 `BlockState` 结果与旧路径逐条对拍。
4. `TerrainProvider::load` 在 manifest/sidecar/NBT 解析后、构造 provider 前用排序集合一次性报告 surface palette、decoration blocks、NBT template/palette/property、placement block/property 的全部错误；缺失旧 placement sidecar 的兼容语义不变，但 sidecar 存在而损坏必须报错，不能降级为空。

**落点**：`server/src/world/terrain/blocks.rs:17-263`、`server/src/world/terrain/raster.rs:850-994,1401-1453,1480-1573`、`server/src/world/terrain/nbt_io.rs:90-150`、`server/src/world/terrain/nbt_registry.rs:223-240`；plan P2。

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
