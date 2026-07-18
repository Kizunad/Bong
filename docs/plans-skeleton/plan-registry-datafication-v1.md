# plan-registry-datafication-v1 — 内容注册表数据化：手搓配方 / 功法元数据 / 方块名映射迁数据文件

> **一句话主题**：把三处最挡"横向扩内容"的硬编码注册表迁成扫盘数据文件——craft 手搓/制作台配方（pin 测试锁定 90 条 + 5 条 legacy 的 Rust 元组表）、功法元数据（49 条 const 数组）、terrain 方块名映射（`blocks.rs` + `raster.rs` 孪生双份 match）——**零新系统、零 wire 改动，有效数据的运行时语义零变化**（唯一有意变更：无效引用从运行时静默失败改为启动期 fail fast，错误契约见 P2），只搬装载来源不动消费方，让"加一条内容 = 加一个数据条目"的覆盖面从物品/丹方/锻造蓝图扩到配方/功法/地形材质。

**状态**：骨架（skeleton）。升 active 前按 docs/CLAUDE.md §五 收口 §8。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | craft 配方数据化——workbench 90 条（pin 锁定）+ legacy 5 条 → `assets/craft/recipes/*.toml` 扫盘 + 对拍回归门 | ⬜ |
| P1 | 功法元数据数据化——`TECHNIQUE_DEFINITIONS` 49 条 → TOML + 双向 wiring 启动校验 | ⬜ |
| P2 | 方块名映射查表化——`blocks.rs` + `raster.rs` 孪生表合一 + manifest 引用启动期 fail-fast（替代静默丢材质） | ⬜ |
| P3 | 范围裁决项——矿物 registry / NPC 原型默认掉落 / 丹道 6 方包装（§8 #5 收口后定） | ⬜ |

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

- 新 `server/assets/cultivation/techniques.toml`（或按 category 分文件，§8 #1）：49 条全字段迁移。resolver 函数指针**留 Rust**（`SkillRegistry` 注册模式不动——本 plan 只外置元数据，不外置行为）。
- 新 `TechniqueRegistry` Resource 替换 const 数组查找；`known_techniques.rs` 门面函数签名保留与否按调用面大小定（§8 #3）。
- **双向 wiring 启动校验（fail fast，防孤岛）**：① 每条元数据声明可施放的功法必须在 `SkillRegistry` 有 resolver；② 每个已注册 resolver id 必须有元数据条目；③ 声明 `required_meridians` 的功法必须与 `SkillMeridianDependencies::declare` 对齐（docs/CLAUDE.md §四 经脉红旗的机械化检查）。
- 与 **plan-skill-av-relink-v1（active）** 协调：图标链防回归测试（#1220，skill_scroll 单一真相源）以 icon id 为锚——元数据外置**不得改任何 icon id 语义**，迁移后该测试族必须原样全绿。
- 对拍回归门同 P0：const 数组快照 == TOML 加载结果逐条相等 + 49 数量 pin。realm / race gate / category 枚举字符串每变体正反 serde sample。

## P2 方块名映射查表化（孪生表合一）⬜

- **范围必须同时收编两份 match**：`blocks.rs::block_from_name`（match 体 `blocks.rs:17-263`）与镜像实现 `raster.rs:1259` `block_state_from_name`——合一为单一真相源后各消费点（`flora.rs` / `raster.rs` / `structures.rs` / `nbt_io.rs` / `nbt_registry.rs` / `cmd/dev/gallery.rs`）统一走新查表。只迁一份 = 静默丢材质风险原样保留 + 两表进一步失去同步，视为不合格交付。
- 方案 §8 #4 收口后定，倾向：valence `BlockKind::from_str` 兜底 + 极小特例映射（数据或常量表，覆盖带状态属性的非直映射条目），退路是脚本生成的静态查表。
- **静默 `None` → 启动期 fail fast（有意的失败语义变更）**：`TerrainProvider::load` 时把 manifest 携带的 surface_palette / decoration blocks 全量预解析，未知方块名启动即报错。错误契约：报错信息列出**全部**未知名及各自来源（palette 项 / decoration id / NBT 文件），触发条件 = manifest 引用的任一方块名不可解析；有效数据的运行时行为不变。发布策略：对拍测试保证现两表覆盖名全数可解析 + 合并前 `scripts/dev-reload.sh` 对现网 raster 全链过一遍，故现存数据不会触发新的启动失败。
- 饱和测试：两份现 match 覆盖的**全部名字新旧解析结果对拍**（一致性快照，含两表差集专项——若两表现状已有分歧条目，逐条裁决记录进 plan）；未知名报错路径；manifest 校验命中 / 漏配用例；`raster_check` 后验流程不受影响（`bash scripts/dev-reload.sh` 全绿）。

## P3 范围裁决项 ⬜

§8 #5 收口后定去留（并入本 plan 追加阶段 or 登记 `reminder.md` 另立）：

- 矿物 registry：`mineral/registry.rs:80` `build_default_registry` 手写 18 条（跨 `MineralId` 枚举 + registry + `minerals.toml` + `mineral_anchors.json` 四处）。
- NPC 原型默认掉落：`npc/loot.rs:63` `default_loot_for_archetype` 静态 match 表（世界/TSY/宗门掉落均已 JSON，唯此硬编）。
- 丹道 6 方包装：`dandao/recipes.rs:41` `fn dandao_recipes() -> [DandaoRecipeSpec; 6]` 定长数组构造函数（引用 alchemy JSON 但包装层硬编）。

---

## §8 开放问题（升 active / P0 决策门前收口）

1. **数据文件粒度**：craft 单文件 vs 按十大类分文件（倾向按类分，对齐 alchemy 一方一文件的可 review 性）；techniques 同题（倾向按 `SkillCategory` 分）。
2. **流派 code-register 配方是否迁**：倾向不迁——dugu-v2 / tuike-v2 / zhenfa 等配方与 skill plan 生命周期绑定，迁了反拆散 plan 内聚；本 plan 只迁 workbench + legacy example，需在 P0 里 grep 确认边界清单。
3. **known_techniques 调用方处理**：保留门面函数（签名不变内部查 Resource）vs 全量改调用方——grep 调用面大小后定，倾向调用面 >10 处则留门面。
4. **方块名方案选型实证**：valence `BlockKind::from_str` 可用性（match 体 `blocks.rs:17-263` 里有多少带 props 的非直映射条目必须保留特例）；与孪生表 `raster.rs::block_state_from_name` 的现状差集清点；生成查表脚本是否值得。
5. **P3 三项去留拍板**（矿物 / NPC 掉落 / 丹道包装）。
6. **数据 lint**：loader 测试是否顺带做 worldview §三 L63 禁词 lint（display_name 扫 玄/陨/星/仙/太/古）——低成本高护栏，倾向做。
7. **与 plan-craft-chain-items-v1（同批 skeleton）顺序协调**：若本 plan P0 先 merge，物品 plan 的新配方直接落 TOML；反之先落 Rust 表行、本 plan 迁移时一并搬。**两 plan 不得同时改 `workbench_recipes.rs`**（merge conflict 高危区，历史上并行 PR 改同一表已叠出过重复字段编译错）。

## §10（升 active 时补）

scope **固定 3 PR**（依赖序列化：PR-1 = P0 → PR-2 = P1 → PR-3 = P2，三者互不跨改同一注册表文件）；P3 若 §8 #5 裁决纳入则 scope 升 4——届时升 active 必须先按 docs/CLAUDE.md §六 模板补全 §10 全文（含"单次 consume-plan 全自动到 merge"章节）再消费。纯 server 基建 plan，无视听规格要求（docs/CLAUDE.md §四 豁免条款）；每 PR 以对拍回归门为 merge 前置；本地门禁 server 栈全跑 + PR-3 追加 `scripts/dev-reload.sh` 全链。**单 plan 边界**：每个实施 PR 仅消费并修改本 plan，不得与 plan-craft-chain-items-v1 在同一 PR 内交叉改动（跨 plan 变更拆独立 PR）；快照/pin 更新规则：他 plan 新增配方后，本 plan 对拍基线在 PR-1 实施起点重新 dump，数量断言随快照长度走。
