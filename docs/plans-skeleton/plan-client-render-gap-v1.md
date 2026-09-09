# plan-client-render-gap-v1 — 客户端手持物与防具渲染缺口

> **一句话主题**：在不改 server gameplay、schema、wire 或物品语义的前提下，收口 Bong 手持物的注册/宿主耦合与防具的运行时 3D 外观缺口，让 `template_id` 能稳定落到可辨识的客户端模型。
>
> **状态**：骨架（skeleton）。本文件只登记事实、边界、阶段和决策门；未 promotion 为 active，不在本 PR 实施任何 Java、Python、Rust、TOML、模型或贴图改动。
>
> **当前复核基线**：`origin/main` / `c6f978a47efa0c401aef6d9a5b609aa7a99f4860`。所有清单以该基线的实际文件为准，不能把审计稿或旧快照当作现状。

## 阶段总览

| 阶段 | 工作性质 | 状态 | 验收日期 |
|---|---|---|---|
| P0 | 盘点、证据固化、所有权与宿主策略决策门 | ⬜ | 待验收 |
| P1 | 已有运行时模型的纯注册/接线缺口 | ⬜ | 待验收 |
| P2 | 共宿主解耦与显式借用关系 | ⬜ | 待验收 |
| P3 | 缺失运行时 3D 几何与防具模型资产 | ⬜ | 待验收 |
| P4 | 视觉回归、资源完整性与 client gate | ⬜ | 待验收 |

## 0. 范围、硬边界与防重

### 0.1 本骨架负责什么

- 盘点 server 已存在的手持物/工具模板与防具模板，建立「已注册」「共宿主」「只有作者资产」「只有 GUI icon」「没有可用 3D 运行时资源」的可复核分类。
- 规划 client 的手持模型注册、vanilla 宿主解耦、模型资源和 `ArmorModelRegistry`/`ArmorFeatureRenderer` 接线的缺口收口。
- 以玩家可观察的 FPV/TPV/GUI/ground 表现和装备槽表现为验收对象；未决设计在本骨架的开放问题中保留，不由自动消费阶段替用户拍板。

### 0.2 不做什么

- 不改 `server/assets/items/*.toml`、server 物品注册、伤害/护甲/装备规则、玩法数值或持久化。
- 不改 `template_id` 的 schema、Redis、CustomPayload、`weapon_spec`/装备状态 wire，也不新造跨端事件。
- 不改 `docs/worldview.md`、`docs/CLAUDE.md`、既有 plan 或本骨架以外的文档；不把本文件 promotion 为 active，不在本任务归档。
- 不把 `qi_physics`、真元流动或任何 gameplay ledger 接入渲染层；本主题只消费已有物品身份和装备快照。
- 不因模型缺口临时新增 `pub`、`pub(crate)`、`#[doc(hidden)]` 或测试专用 seam；若某路线需要可见性变化，必须回到 P0 重新决策。

### 0.3 与既有手持注册骨架的所有权边界

当前主线**实际存在** `docs/plans-skeleton/plan-held-item-registration-v1.md`，其中已经规划了 `BongHeldItemRegistry`、render-only Fabric Item、39 个现有注册项、9 个顶层 weapon/tool 漏项、宿主 override 清理和显式 `borrowsFrom`。因此本 plan 不得再造第二个通用手持注册表，也不得并行删除同一批 vanilla host override。

本 plan 额外收口的是「渲染缺口」总体验收，尤其是防具的运行时 ModelPart/几何缺口和未注册物品的视觉分类；P1/P2 中涉及通用手持注册或宿主迁移的实现，升 active 前必须明确是并入 `plan-held-item-registration-v1`、由本 plan 接手，还是拆成依赖关系。若所有权没有单一答案，不得进入实施。

既有 `plan-held-item-registration-v1` 不因本文件创建而修改；本文件只记录防重结论和未来接线契约。

## 1. 接入面 Checklist（`docs/CLAUDE.md` §二）

### 1.1 进料

- **server 物品事实源**：顶层 `server/assets/items/*.toml` 中 `category = "weapon" | "tool"` 的 43 个模板；涉及来源包括 `weapons.toml`、`tools.toml`、`materials.toml`、`forge.toml`、`workbench_materials.toml`、`zhenfa.toml`、`core.toml`、`craft_legacy_items.toml`。`server/assets/items/niche/*.toml` 另行盘点，不自动并入 43 个基线。
- **server 防具事实源**：`server/assets/items/armor.toml` 的 28 个 `armor_<material>_<slot>` 模板，7 套材质/形态：`straw`、`bone`、`hide`、`iron`、`copper`、`spirit_cloth`、`scroll_wrap`。
- **client 装备状态**：`InventoryStateStore.snapshot().equipped()` / `equippedSlots()`、`InventoryItem`、`EquipSlotType`，以及既有 `WeaponEquippedStore`、`EquippedShieldStore`；它们是视觉输入，不重新定义 server 装备语义。
- **作者资产输入**：`modelScript/generators/` 的手持/防具生成器、`modelScript/models/` 的 `.bbmodel`、`bbmodel-maker` 提供的 `bbmodel-render` 命令和既有 `client/src/main/resources` 资源。当前仓库已把通用渲染器移到依赖包，不再存在旧的 `scripts/models/render_bbmodel.py` 路径；生成器或 `.bbmodel` 仍只能算作者输入，不能代替已接入客户端的运行时几何。

### 1.2 出料

- **手持模型路径**：当前 `client/src/main/java/com/bong/client/weapon/BongWeaponModelRegistry.java` 的 `Entry`、`WeaponRenderBootstrap.isBongManagedModel()`、`WeaponVanillaIconMap.createStackFor()`、`HeldItemStackResolver.resolveMainHand()`/`resolveOffHand()`，以及 `assets/bong/models/item/` 与仍被 SML 劫持的 `assets/minecraft/models/item/`。
- **防具模型路径**：`client/src/main/java/com/bong/client/armor/ArmorModelRegistry.java`、`ArmorPartModel.CUBE_TABLES`、`ArmorFeatureRenderer.collectRenderable()`/`render()`、`ArmorRenderBootstrap.register()`、`MixinPlayerEntityArmor`，以及 `assets/bong/textures/armor/`。
- **GUI icon 与 fallback**：`ArmorTintRegistry` 和 `client/src/main/resources/assets/bong-client/textures/gui/items/armor/`；GUI icon 存在不等于手持或上身 3D 模型存在。
- **资源包/构建出料**：模型、MTL、OBJ、贴图、资源包 manifest/sha1（若实施变更触及资源包），以及 client test 能够观察到的注册表和资源完整性。

### 1.3 共享类型 / event

- 复用 `template_id`、`InventoryItem`、`EquipSlotType`、`WeaponEquippedStore`、`EquippedShieldStore` 和现有 client render callback；不新增 store，不新增事件，不复制 server 物品实现。
- 若继续使用 SML，复用现有 `SpecialModelLoaderEvents.LOAD_SCOPE` 接线；若改为 render-only Fabric Item 或 ModelPart，必须在 P0 说明与现有宿主链的互斥关系和迁移顺序。
- 防具几何的运行时真相须与当前 `ArmorPartModel` 的 cube-table 约定一致；作者 `.bbmodel` 不作为运行时第二真相源。

### 1.4 跨仓库契约

- **唯一契约键是 `template_id`**：server 物品定义、装备快照/`weapon_spec` 和 client 注册表通过同一字符串对接。本 plan 不新增 wire 字段、不改 schema、不改 `template_id` 命名。
- agent 不参与手持/防具模型解析；client 的渲染缺口不得反向要求 agent 了解 OBJ、ModelPart、GUI 坐标或宿主 Item。
- server 仍是物品和装备语义权威；client 只把已收到的 `template_id` 映射为现有渲染资源。

### 1.5 worldview / qi_physics 锚点

- **worldview**：不新增世界观实体、境界、经济或 zone；已有 ID 只作视觉键。若后续需要新增显示名或新命名，必须先核 `docs/worldview.md §三 L63` 的命名禁词，再单独走相应 plan。
- **qi_physics**：不涉及。渲染注册、模型借用、贴图和 ModelPart 不产生或转移真元，不得调用 ledger、不准复制任何 qi 常数，也不能借渲染改动掩盖物品玩法缺口。

## 2. 立 plan 前预检记录（2026-09-09）

### 2.1 调研范围与防孤岛结论

已复核 `docs/worldview.md`、`docs/finished_plans/`、`docs/plans-skeleton/`、`docs/plans-skeleton/reminder.md`、`docs/plan-*.md` 和当前 client/server/modelScript 接线：

- **`docs/worldview.md`**：grep 了命名、装备/手持、防具、阵法、经济和灵气相关锚点。`worldview.md §三 L63-L72` 规定不得回用上古境界称谓；`worldview.md §四 L256-L260` 规定装备/护甲作用于既有部位状态；`worldview.md §五 L413-L421` 区分凡器手持与地师将真元封入环境陷阱；`worldview.md §五 L552-L558` 明确护甲可分层穿戴而武器/工具是“持”而非“穿”；`worldview.md §九 L846-L850` 与 `§十 L872-L878` 是经济/灵气总量锚点。本 plan 不修改 worldview、不新增命名或经济语义；未来只沿用已有 `template_id`，若实施需新增显示名，必须遵守 `worldview.md §三 L63` 的命名约束，并不得把视觉注册改成物品/真元经济变更。
- **`docs/plans-skeleton/reminder.md`**：以 `client|render|渲染|手持|防具|护甲|模型|weapon|armor|视觉|装备` 检索后，没有与本 plan 的客户端手持注册、vanilla 宿主解耦或防具 ModelPart/3D 模型直接匹配的待办，结论是**无匹配待办**。检索到的放置类容器渲染约束、`niche_guardian` SFX 和转移税等条目分别属于既有放置/音频/qi plan owner，不并入本 plan，也不在本 PR 改动 reminder。

- `docs/finished_plans/plan-armor-visual-v1.md` 已交付凡物甲的 server/craft、tint、icon 和 vanilla leather fallback；它不等于真实 3D 上身模型。
- `docs/finished_plans/plan-armor-model-render-v1.md` 已交付 `ArmorFeatureRenderer`/`ArmorPartModel` 的运行时链，并完成铁甲/骨甲各四件的 cube 表；本 plan 不重复改这 8 件的已完成几何。
- `docs/finished_plans/plan-weapon-v1.md`、`plan-weapon-v1.1.md` 已记录手持模型、SML/OBJ 和 host override 的历史契约；本 plan 继承现状证据，不把旧资源路径误报为新契约。
- `docs/finished_plans/plan-zhenfa-trap-client-equip-gate-v1.md` 已覆盖 `array_flag` 等正常装备门，并明确 niche 四项因只可由 dev `/give` 获得而排除；本 plan 只保留它们的渲染归类问题，不修改该 plan。
- `docs/plans-skeleton/plan-held-item-registration-v1.md` 是当前最直接的同主题骨架，已声明通用注册和宿主解耦所有权；本 plan 因增加防具/运行时视觉缺口而单独记录，但 P1/P2 实施前必须做单一所有权收口。
- 未发现同名 `plan-client-render-gap-v1` active plan；未发现一份已完成、覆盖本 plan 全部「手持宿主 + 7 套防具运行时几何」的 plan。上述既有 plan 继续保持原文件不动。

### 2.2 复核命令与计数口径

本次在当前基线使用了以下只读检查，后续 P0 应把等价检查固化为可重复的验收命令：

```text
find docs -maxdepth 2 -type f | sort
grep -RInE 'BongWeaponModelRegistry|ArmorModelRegistry|WeaponRenderBootstrap|ArmorRenderBootstrap' client docs
grep -RInE '^category[[:space:]]*=[[:space:]]*"(weapon|tool)"' server/assets/items --include='*.toml'
grep -c 'entries.put(' client/src/main/java/com/bong/client/weapon/BongWeaponModelRegistry.java
grep -cE '^id[[:space:]]*=[[:space:]]*"armor_' server/assets/items/armor.toml
grep -cE 'register\("armor_' client/src/main/java/com/bong/client/armor/ArmorModelRegistry.java
find modelScript/generators modelScript/models client/src/main/resources -type f | sort
grep -RInE 'niche_house_puppet|niche_zhenfa_trap_(basic|middle|advanced)' server client
```

计数必须明确目录边界：顶层 `server/assets/items/*.toml` 的 weapon/tool 集合是 43；嵌套 `server/assets/items/niche/*.toml` 的 4 个 tool 单独报告。当前 `BongWeaponModelRegistry` 有 39 个 `entries.put`，与顶层集合交集为 34，缺失 9；`armor.toml` 有 28 件，`ArmorModelRegistry` 只有 8 件。

### 2.3 当前已核实清单

#### 手持/工具

顶层 43 个 `category=weapon|tool` 中尚未出现在 `BongWeaponModelRegistry` 的 9 个是：

```text
array_flag  blast_trap  bone_spike_crude  eclipse_needle_iron
herb_knife_iron  iron_dagger  slow_trap  warning_trap  wooden_club
```

当前注册表的 39 项并不等于覆盖 39 个顶层 weapon/tool：其中 34 项覆盖顶层集合，另有 `hoe_iron`/`hoe_lingtie`/`hoe_xuantie`（`lingtian.toml` 的 `misc`）和 `wooden_shield`/`bone_shield`（`workbench_materials.toml` 的 `shield`）。后续清单必须保留这个分类差异，不能把锄头或盾牌重复计入 43。

已确认的共宿主为 7 个 vanilla host、19 个模板：

| host model path | 当前共用模板 |
|---|---|
| `item/stone_sword` | `bone_sword`、`gua_dao`、`iron_sword_flawed`、`qing_feng_sword_flawed`、`ling_feng_sword_flawed`、`stone_knife` |
| `item/iron_sword` | `iron_sword`、`qing_feng_sword`、`ling_feng_sword` |
| `item/bone` | `bone_dagger`、`bone_spike` |
| `item/leather` | `hand_wrap`、`bing_jia_shou_tao` |
| `item/stone_pickaxe` | `stone_pickaxe`、`pickaxe_copper` |
| `item/stone_axe` | `stone_axe`、`axe_copper` |
| `item/flint_and_steel` | `dun_qi_jia`、`gu_hai_qian` |

#### 防具与作者资产

- server 侧是 7 套 × 4 槽 = 28 件；client `ArmorTintRegistry` 当前仅列 6 个材质（没有 `straw`），`ArmorModelRegistry`/`ArmorPartModel.CUBE_TABLES` 当前只有 `iron_*` 和 `bone_*` 共 8 件/8 个 model key。
- `hide`、`scroll_wrap`、`straw` 的生成器**在当前基线实际存在**：分别是 `modelScript/generators/gen_hide_armor.py`、`gen_scroll_wrap_armor.py`、`gen_straw_armor.py`；同时存在不同完整度的作者 `.bbmodel`。`scroll_wrap` 还有若干 `client/assets/bong/textures/armor` 贴图，但没有对应 `ArmorModelRegistry`/cube-table 运行时接线；这些事实不能被简化成「生成器不存在」。
- `straw` 当前作者文件只覆盖护腿/草鞋等部分形态，仍不是四槽完整运行时套装；`hide`/`scroll_wrap` 的作者文件也只是离线输入。`copper`、`spirit_cloth` 当前只见 GUI icon（`bong-client:textures/gui/items/armor/armor_copper.png`、`armor_spirit_cloth.png`），未见对应运行时 3D 几何。
- 因而任务层面的运行时结论仍是：`hide`、`scroll_wrap`、`straw`、`copper`、`spirit_cloth` 五套都不满足「可由 `ArmorFeatureRenderer` 直接渲染的完整独立 3D 外观」；但后续工作量应按作者源文件的实际成熟度分别估算，不得把已有离线生成器重复造一遍。

#### niche 四项

`niche_house_puppet`、`niche_zhenfa_trap_basic`、`niche_zhenfa_trap_middle`、`niche_zhenfa_trap_advanced` 实际位于 `server/assets/items/niche/*.toml`，四项均声明 `category = "tool"`；它们不在顶层 43 的统计口径内。当前 `server/src` 未找到这四个 ID 的生产消费，既有 plan 记录它们是无 craft/loot、只由 dev `/give` 可得的内容。

这证明它们「存在且是 TOML tool」，但还不能证明它们是手持物：是放置物、灵龛/阵法实体，还是未来应进入手持渲染注册，须留到开放问题决议，不能在本骨架中擅自纳入或排除。

#### 与任务卡旧事实的五处纠偏

1. `docs/plans-skeleton/plan-held-item-registration-v1.md` 当前存在，不能按「不存在」另起一份通用注册 plan。
2. 43 的数字只适用于顶层 `server/assets/items/*.toml` 的 weapon/tool；嵌套 niche 四项需单独报告，不能静默混入或漏报。
3. `modelScript/generators/gen_wooden_club.py` 和 `modelScript/models/WoodenClub.bbmodel` 当前存在；但 `wooden_club` 仍无 client item-model 注册，所以运行时缺口依旧。
4. `gen_hide_armor.py`、`gen_scroll_wrap_armor.py`、`gen_straw_armor.py` 当前都存在，并有不同程度的作者输出；这不等于五套防具已有可用的 client 3D 运行时模型，P3 仍需补齐/接入，不能把生成器“缺失”当作工作量依据。
5. `niche_house_puppet` 与 `niche_zhenfa_trap_*` 并非不存在，而是位于嵌套 `server/assets/items/niche/`；它们的玩法形态和渲染归属仍未决，不能按顶层正常手持漏项直接注册。

## 3. P0 — 盘点、判据与决策门

P0 是所有实施的硬门。先补齐逐模板、逐槽位和逐资源证据，再收口本 plan 与 `plan-held-item-registration-v1` 的所有权，不能先改 registry 后补解释。

### 3.1 权威清单

- 由 `server/assets/items/*.toml` 的真实 `[[item]]` section 解析顶层 weapon/tool 集合，输出 43 个 ID；用 `server/assets/items/niche/*.toml` 单列 4 个 niche tool；用 `armor.toml` 输出 28 个 armor ID。
- 从 `BongWeaponModelRegistry` 解析 39 个 entry，并报告三组集合：顶层交集 34、顶层缺失 9、注册表外的 misc/shield 5；禁止仅以 Java entry 数量宣称覆盖完成。
- 对每个手持候选标记：`own OBJ/JSON`、有意 `borrow`、vanilla plain fallback、共宿主、无运行时资源、仅 GUI icon、是否需要先解决 niche 形态。
- 对每个 armor `material × slot` 标记：server item、GUI icon、作者 `.bbmodel`、运行时 texture、`ArmorModelRegistry` entry、`ArmorPartModel` key、实际 renderer path，任一缺项都要有处置理由。

### 3.2 完成判据

- **注册完成**：正常装备/手持状态中的 `template_id` 能命中唯一、可诊断的 client entry；FPV/TPV 解析链不因未知项静默显示另一件物品；目标资源和所需 SML/ModelPart 接线均存在。
- **独立外观完成**：运行时拥有该模板自己的模型资源或明确记录的单向借用关系；共享 vanilla host 不得被误报为独立外观。故意借用必须可在数据/测试中查到，不能靠两个 entry 恰好指向同一 host 来表达。
- **防具完成**：四槽的 server `template_id`、client material/slot、`ArmorModelRegistry`、`ArmorPartModel` cube 表、纹理和 `ArmorFeatureRenderer` 挂载逐一相符；没有双层 leather fallback、错槽或破损甲仍渲染的回归。
- **资源完成**：model JSON/OBJ/MTL/贴图或 ModelPart 运行时输入可由测试检查；资源包 manifest 发生变化时同步 sha1/size 证据。

### 3.3 P0 交付物

- 一份可重复生成的手持 43 + niche 4 + armor 28 分类表/验收输出（实现时再决定落在既有测试/manifest 的哪个 owner 中，不在本 skeleton 新增脚本）。
- 与 `plan-held-item-registration-v1` 的单一 owner 决议：通用 render-only Item、宿主迁移、host override 清理各由哪一份 plan 负责。
- P2 宿主策略、P3 五套防具范围和四项 niche 形态问题全部得到人工决议；未决不得升 active。

## 4. P1 — 纯注册/接线缺口（不造模型）

P1 只处理 P0 认定「运行时模型已经存在、缺的只是 registry/入口接线」的条目；生成器、GUI icon 或离线 `.bbmodel` 单独存在时，不得放入 P1 冒充已具备模型。

- 对明确的手持候选接入既有 `template_id → model/stack` 链，保持 `HeldItemStackResolver` 主/副手优先级和 `WeaponVanillaIconMap` 的 fallback 语义；不改 server `weapon_kind`、装备规则或攻击。
- 对已经有完整 `ArmorPartModel`/texture 的防具只做 `ArmorModelRegistry` 与 renderer 接线；当前基线已完成的铁/骨 8 件必须保持行为和 cube digest 不变。
- 不在本阶段新造 `BongHeldItemRegistry`；若采用该基础设施，按 §0.3 交给 `plan-held-item-registration-v1` 或完成明确的 owner 转移，不能两份 plan 各有一套注册入口。
- P1 验收：每个入选模板的 `template_id`、entry、资源、FPV/TPV 解析和未知 ID 行为均有 pin；不存在「注册表命中但模型 missing」或「有模型但没有 stack」的半接线。

## 5. P2 — 共宿主解耦与显式借用

P2 只在 P0 决定宿主路线之后实施。当前 19 个模板挤在 7 个 vanilla host 上，导致同一 host 的模型改动会无声改变其他物品的外观。

- 先把每一条关系标为：独立 Bong render-only Item、显式单向 OBJ 借用、保留 vanilla plain host，或经决议维持短期兼容；不能把共宿主继续作为隐式数据模型。
- 若拆到独立模型，清理/迁移 SML vanilla override、fake stack、资源包 manifest 和测试；若显式借用，借用者必须在注册数据中指向被借者而不是再次复制生产实现。
- 所有拆分必须保留 `template_id`、装备/攻击/耐久语义和现有 server/client wire；视觉差异不能变成 gameplay 变化。
- 验收包括：19 项逐项查到最终外观来源；任一 host 的模型变化不会意外改动不应共享的模板；故意共享关系可审计；FPV、TPV、GUI、ground（若 P0 判定走该链）均无 missing model。

## 6. P3 — 缺失运行时 3D 几何与防具资产

P3 是成本最高阶段。候选来源必须按 §2.3 复核，不把作者文件存在误当成客户端已交付。

### 6.1 手持候选

- 9 个顶层 registry 漏项 `array_flag`、`blast_trap`、`bone_spike_crude`、`eclipse_needle_iron`、`herb_knife_iron`、`iron_dagger`、`slow_trap`、`warning_trap`、`wooden_club` 先经过 P0 的「手持还是放置」分类。
- 已有作者生成器/模型的条目只补完整输出、安装和注册；没有运行时 3D 的条目按独立模型、显式借用或经决议的 placeholder 处置，并在验收表逐项写理由。
- zhenfa 四项若被裁定为放置物，不得为了凑 43 个 hand-held entry 硬接手持渲染；若被裁定为手持，需同时验收放置交互不被 fake stack 影响。

### 6.2 五套防具

- `hide`、`scroll_wrap`、`straw`、`copper`、`spirit_cloth` 均当前缺少完整的 `ArmorModelRegistry` + `ArmorPartModel` 运行时链；是否一次全做由开放问题决定，不能在骨架阶段拍板。
- 每套按 `helmet/chestplate/leggings/boots` 四槽分别核对几何、纹理、挂点、遮挡、破损过滤、与铁/骨的远距轮廓差异；部分作者模型（例如 straw 当前只有部分部件）不能假装四槽完成。
- 运行时真相继续落在既有 `ArmorPartModel` cube 表/ModelPart 约定；`.bbmodel` 只作为离线资产，必须经预览、转写和测试 pin 后才算接入。
- 防具 icon 与 3D 模型分开验收：GUI icon 只能证明 icon 存在，不能替代穿戴/上身几何。

## 7. P4 — 视觉回归与门禁

- **注册/资源 pin**：扩展 `BongWeaponModelRegistryTest`、`ArmorModelRegistryTest` 或最终 owner 的等价测试，核对 server 清单、`template_id` 集合、model/texture 路径、host/borrow 关系、四槽映射和 unknown ID 行为。
- **渲染回归**：FPV、TPV/F5、GUI、ground（若 P0 判定使用 ItemRenderer）分别检查；穿戴全套/单槽/错槽/破损/卸下，确保玩家能从远处区分不同手持物和五套防具，不出现 vanilla host 串形、missing model 或 leather 双层。
- **协议回归**：沿既有 server → 装备快照/`weapon_spec` → client resolver 链验证，证明只改变视觉接线；不得新增或修改 schema/wire。niche 若仍未决，只记录不纳入协议验收。
- **资源包回归**：若模型路径/资源文件变化，运行资源包构建与 manifest/sha1 检查；严禁留下未注册的孤儿 override。
- **client gate**：实施期按 client 所触范围运行 `cd client && ../scripts/build-token.sh gradle test build`（Java 17）；跨栈变更才增加对应栈 gate。本骨架 PR 不运行 cargo/gradle，不以 docs-only PR 声称 gameplay 或 client gate 已通过。
- **完成口径**：所有进入实施的阶段均有可重跑命令、实际数量和截图/预览证据；只跑 smoke 或只证明 GUI icon 存在不算 P4 通过。

## 8. 视觉资产纪律（升 active 后强制执行）

- 模型类工作必须至少三轮：Round 1 first cut；Round 2 以三视图/玩家预览自评并修正；Round 3 最终检查。终轮 commit message 必须写入正确拼写的 `<PROMISE>` 担保块，说明已完成 3 轮打磨及仍有限制。
- 复杂模型按部件制作和验收：使用 `part_base()`/`part_body()` 等部件函数，逐件预览后再用 `all_cubes()`/等价组合；不要一把生成一个不可解释的大盒子。Blockbench 外观以当前依赖提供的 `bbmodel-render <模型>`（或其等价渲染工具）实证，不以平涂示意图代替。
- item icon 必须走 `/gen-image item` 和仓库规定的资源路径；当前 harness 若跑不了，保留接线并在对应 TODO 标 `[BLOCKED: 需 /gen-image 生成 <清单>]`，不手绘模糊占位、不跳过 icon 接线。
- 每个材质/模板的模型、贴图、icon、资源包路径和视觉差异必须可追溯到 `template_id`；作者文件、运行时文件和 GUI icon 不得混为一谈。

## §9 开放问题（P0 决策门前需收口）

以下只提出问题，不在本骨架中拍板；进入 P0 实施前要按 `docs/CLAUDE.md §五` 逐条形成带「文件:行号 + plan 章节」双锚点的决议。

1. **P2 宿主策略**：19 个共宿主模板应采用 render-only Fabric Item、显式单向 OBJ 借用、继续使用 vanilla host，还是走绕开 ItemRenderer 的自绘链？在 `plan-held-item-registration-v1` 与本 plan 之间，最终 owner 应如何唯一化？
2. **五套防具范围**：`hide`、`scroll_wrap`、`straw`、`copper`、`spirit_cloth` 是一次全部补齐，还是按材质/部件分批？若分批，哪一批先满足完整四槽和远距可辨识验收？
3. **zhenfa/灵龛形态**：`array_flag`、`blast_trap`、`slow_trap`、`warning_trap` 四件与 `niche_house_puppet` 是否属于手持物范畴，还是放置物/灵龛实体？哪些（如果有）应走手持注册，哪些应走独立放置物渲染？
4. **骨刺外观关系**：`bone_spike` 与 `bone_spike_crude` 是否应共享外观、显式借用外观，还是必须各有独立模型？如果共享，如何在注册数据和验收中表达为有意关系而非共宿主副作用？
5. **client-only 注册可行性**：render-only Fabric Item 在当前 Fabric 1.20.1/Valence 服务器连接、静态 registry、创造栏/REI、掉落物和登录同步上的行为是否都可接受？若不可接受，应采用哪条不新增 server wire 的 fallback？
6. **掉落与显示名边界**：Bong 的掉落/地面显示是否走 vanilla `ItemRenderer`，未注册 lang 是否可能泄露到玩家界面？这些路径是否属于本 plan，还是留作独立 follow-up？

全部六条开放问题已在 §9.1 收口。原表保留以备追溯，**实施时以 §9.1 决议为准**。

## §9.1 决议（pre-P0 收口，2026-09-09）

### #1 P2 宿主策略与唯一 owner

**决议**：
1. 选用 **render-only Fabric Item** 作为手持物的宿主策略：每个进入手持渲染范围的 `template_id` 都注册自己的 client-only `bong:<template_id>` Item，由 `BongHeldItemRegistry`/等价单一注册表供 `HeldItemStackResolver` 生成 fake `ItemStack`，再接入现有 FPV/TPV 与 SML 模型加载链。
2. 显式单向 OBJ 借用只作为注册数据中的关系字段（例如 `borrowsFrom`），不能替代 Item 注册；有意借用必须能从注册数据和 pin 测试查到被借者，不能再用两个模板恰好共用一个 vanilla host 表达。通用手持注册、宿主迁移和旧 host override 清理由**本 plan 唯一负责**，不以另一个 owner 文档作为前置依赖。
3. 拒绝继续使用 vanilla host 作为长期方案：当前 `Entry` 直接保存 `hostItemSupplier`/`vanillaModelPath`，`WeaponVanillaIconMap` 据此合成 stack，且 SML 按 host path 劫持；这正是 19 个模板挤在 7 个宿主上、模型改动会无声串改的耦合。拒绝把 OBJ 借用单独当作方案，因为它只描述资源关系，不能消除 host 冲突；拒绝自绘链，因为现有 `MixinHeldItemRenderer` 与 `MixinPlayerEntityHeldItem` 已把 fake stack 送入 vanilla FPV/TPV 渲染，重写自绘会重复 display 变换、GUI 和资源加载链。

**落点**：`client/src/main/java/com/bong/client/weapon/BongWeaponModelRegistry.java:20-24,146-173,243-248`、`client/src/main/java/com/bong/client/weapon/WeaponVanillaIconMap.java:30-36`、`client/src/main/java/com/bong/client/weapon/WeaponRenderBootstrap.java:24-39`、`client/src/main/java/com/bong/client/mixin/MixinHeldItemRenderer.java:20-29,47-63`、`client/src/main/java/com/bong/client/mixin/MixinPlayerEntityHeldItem.java:20-38,46-80`（依据代码）/ 本 plan `§4 P1`、`§5 P2`、`§3.2`（实施入口与验收）。

### #2 五套防具的实施批次

**决议**：
1. 选择**分批实施**，第一批固定为 `hide` + `scroll_wrap`。两套作者生成器都已经有 `helmet/chestplate/leggings/boots` 四个部件函数，能最快形成完整四槽，并以各自的材质轮廓做远距可辨识回归；第一批的完成条件仍是四槽运行时链全通，而不是只有离线源文件存在。
2. 第二批固定为 `straw`：当前实际只有 `leggings` 与 `boots`，必须先补齐 `helmet`/`chestplate`，不能把两槽作者输出报作完整套装。最后处理 `copper` + `spirit_cloth`：当前只核到 GUI icon，没有可供 `ArmorFeatureRenderer` 直接消费的运行时几何。五套仍全部属于本 plan 范围，不因分批而移出。
3. 每一批都必须逐槽接入 `ArmorModelRegistry`、`ArmorPartModel` cube/ModelPart、纹理和 `ArmorFeatureRenderer`/`ArmorRenderBootstrap`，并验收错槽、破损、卸下和远距轮廓；GUI icon、生成器和 `.bbmodel` 只能算输入证据，不能替代运行时完成证据。

**落点**：`modelScript/generators/gen_hide_armor.py:171-181,351-361,460-465,539-548`、`modelScript/generators/gen_scroll_wrap_armor.py:146-154,240-247,284-289,321-330`、`modelScript/generators/gen_straw_armor.py:220-225,350-359`、`client/src/main/java/com/bong/client/armor/ArmorModelRegistry.java:29-63`、`client/src/main/java/com/bong/client/armor/ArmorFeatureRenderer.java:75-124`、`client/src/main/java/com/bong/client/armor/ArmorRenderBootstrap.java:21-31`（依据代码）/ 本 plan `§6.2`、`§7`（批次与验收）。

### #3 阵旗、普通陷阱与 niche/灵龛物的形态

**决议**：
1. `array_flag` 定性为**手持的布阵控制工具**，进入手持注册范围，但它本身不是放置后的阵眼实体：服务端定义为 `tool`，客户端装备规则明确要求它能落入手槽，服务端 `has_zhenfa_flag` 从装备/背包查它作为布阵门。P1 只为它提供手持阶段的模型入口，不把手持 Item 当作阵眼的地面模型。
2. `warning_trap`、`blast_trap`、`slow_trap` 定性为**先手持、后放置**的消耗型阵法工具：客户端从主手识别后打开布阵 UI，服务端通过 `ZhenfaPlaceRequest` 校验 target face、物品实例和成本，成功后写入 zhenfa anchor block/registry。因此三件都需要手持显示，但落地后的视觉归独立放置物/阵眼渲染链；不得以普通手持模型冒充已放置陷阱。
3. `niche_house_puppet`、`niche_zhenfa_trap_basic`、`niche_zhenfa_trap_middle`、`niche_zhenfa_trap_advanced` 实际位于 `server/assets/items/niche/*.toml`，虽声明 `category = "tool"`，当前 `server/src` 与 client 主生产代码没有对应消费、制作/掉落或放置请求入口。它们现在定性为**不在本 plan 范围**；未来一旦出现真实 use/placement owner，再由该 owner 同时提出手持与放置渲染接线，不能只因 category 是 tool 就提前注册或假定为灵龛实体。

**落点**：`server/assets/items/zhenfa.toml:1-10,45-76`、`client/src/main/java/com/bong/client/inventory/InventoryEquipRules.java:64-72`、`server/src/zhenfa/mod.rs:1573-1686,4112-4143,4504-4529`、`client/src/main/java/com/bong/client/mixin/MixinClientPlayerInteractionManagerAlchemy.java:106-116`、`server/src/zhenfa/trap_content.rs:121-136`、`server/assets/items/niche/house_puppet.toml:1-10`、`server/assets/items/niche/zhenfa_trap_basic.toml:1-10`、`server/assets/items/niche/zhenfa_trap_middle.toml:1-10`、`server/assets/items/niche/zhenfa_trap_advanced.toml:1-10`（依据代码/资产；对 niche 的无消费结论由 `grep -RIn` 复核）/ 本 plan `§4 P1`、`§6.1`、`§7`（手持、放置和范围边界）。

### #4 `bone_spike` 与 `bone_spike_crude` 的外观关系

**决议**：
1. 两者采用**各自独立模型**，不共享外观，也不把一个现有 vanilla host 作为隐式共宿主。`bone_spike` 是 1×2、uncommon、带真元封存语义的完整暗器；`bone_spike_crude` 是 1×1、common、低攻击的粗削投掷物，尺寸、品阶、用途和玩家预期均不同，共用同一轮廓会掩盖可观察的物品差异。
2. `bone_spike` 直接接入现有 `gen_knife_trio.py` 的专属模型输出；`bone_spike_crude` 在 P3 单独补一个粗坯轮廓与资源。注册数据必须以两个 `template_id` 分别指向两个 model key/资源路径，验收 pin 既检查两项都可渲染，也检查其 model key/资源 digest 不相同。
3. 若未来有别的近似物品确实要复用模型，只能写显式单向 `borrowsFrom` 并在测试中区分「有意借用」与「多个模板共享 host」；本决议的两项不走该例外，不能以相同 `item/bone` 宿主或相同路径默示共享。

**落点**：`server/assets/items/materials.toml:142-154`、`server/assets/items/workbench_materials.toml:670-682`、`server/assets/craft/recipes/workbench/weapon.toml:1-19`、`modelScript/generators/gen_knife_trio.py:318-361`、`client/src/main/java/com/bong/client/weapon/BongWeaponModelRegistry.java:190-195`（依据代码/资产）/ 本 plan `§5 P2`、`§6.1 P3`、`§7`（独立模型与显式借用验收）。

### #5 render-only Fabric Item 在 Fabric 1.20.1/Valence 下的可行性

**决议**：
1. 结论为**架构上可行，但必须以 P0 spike 的四项实证作为放行门**：登录不掉线、模型能加载、合成的 client-only stack 能在 FPV/TPV 手持渲染、GUI 入口不报错。依据是客户端模组声明为 `environment = "client"`，现有 server→client 装备消息只读取 `template_id` 并写入 store，`WeaponVanillaIconMap` 已证明渲染 stack 是客户端惰性合成，不是 server inventory/wire 数据；因此不会把 `bong:<id>` 作为 server 物品或新字段下发。
2. 静态 Item registry、创造栏/REI、掉落物和登录同步分别按以下口径验收：`Registries.ITEM` 只在 client init 注册；不加入 `ItemGroup`，且当前 `client/build.gradle` 没有 REI 依赖，P0 仍要在无/有外部 REI 的实际客户端分别确认不泄露内部 Item；登录只验证现有 Valence 连接和 `template_id` 装备事件；掉落不依赖 Item registry，因为当前地面链是 `DroppedItemStore` 的自绘 billboard。任一实际 spike 失败都不得升 active。
3. fallback 固定为保留当前 fake vanilla host + SML 链：继续由 `WeaponVanillaIconMap` 生成已知 vanilla stack，并由 `WeaponRenderBootstrap`/两个 held-item mixin 驱动渲染；fallback 不新增 server wire，不改 `template_id`、装备状态或掉落协议，也不转向本 plan 之外的第二套自绘实现。

**落点**：`client/src/main/resources/fabric.mod.json:12-27`、`client/src/main/java/com/bong/client/BongClient.java:157-158`、`client/src/main/java/com/bong/client/network/WeaponEquippedHandler.java:35-68`、`client/src/main/java/com/bong/client/weapon/WeaponVanillaIconMap.java:23-36`、`client/build.gradle:45-87`、`client/src/main/java/com/bong/client/inventory/render/DroppedItemWorldRenderer.java:27-38,55-68`（依据代码/构建配置）/ 本 plan `§3.2`、`§4 P1`、`§7`（P0 spike、fallback 与 client gate）。

### #6 掉落、地面显示与显示名边界

**决议**：
1. Bong 的 ground/drop rendering **留在既有 inventory/drop owner，不纳入本 plan 的 ItemRenderer 迁移**：`DroppedItemWorldRenderer` 从 `DroppedItemStore` 读取坐标和 `InventoryItem`，直接画 client-only billboard；它不 spawn `ItemEntity`，也不调用 vanilla `ItemRenderer`。玩家死亡时 vanilla `dropInventory` 还会被取消，掉落由 server-authoritative 的 `inventory_event`/`dropped_loot_sync` 链进入 store。
2. 显示名的权威来源继续是 server 下发的 `InventoryItem.displayName()`，HUD 以该字段组装地面 marker；不因 render-only Item 新增 server lang、schema 或 drop wire。与此同时，未注册 lang 的风险是真实存在的：如果 client-only/fake stack 被送入 vanilla tooltip/name 路径，当前 mixin 文档已明确会显示宿主 Item 名称，新的 `bong:<id>` 还可能显示原始 translation key；所以「held-item 注册不得泄露 vanilla tooltip/创造栏/掉落实体」是本 plan 的 P0/P1 接入门，而不是把风险推给 ground follow-up。
3. 本 plan 只负责上述 held-item 隔离门，以及在 P4 验证 FPV/TPV/GUI 不会把 server display name 替换成 vanilla/raw lang；实际掉落位置、pickup、marker 文案和 proto 字段继续由 inventory/drop owner 维护。未来若要改掉落形态或让掉落走 `ItemRenderer`，另开 follow-up，不在本骨架决议中扩大范围。

**落点**：`client/src/main/java/com/bong/client/inventory/render/DroppedItemWorldRenderer.java:27-38,55-60,99-125`、`client/src/main/java/com/bong/client/network/InventoryEventHandler.java:103-145`、`client/src/main/java/com/bong/client/network/DroppedLootSyncHandler.java:12-55,58-85`、`client/src/main/java/com/bong/client/hud/DroppedItemHudPlanner.java:293-296`、`client/src/main/java/com/bong/client/mixin/MixinPlayerEntityDrop.java:21-33`、`client/src/main/java/com/bong/client/mixin/MixinPlayerEntityHeldItem.java:27-33`（依据代码）/ 本 plan `§4 P1`、`§7`、`§10`（边界、隔离门与后续 owner）。

## 10. Finish Evidence

> 当前仍是 skeleton，尚无已完成阶段、实现 commit 或 gate 结果；进入 active/完成归档时按根 `CLAUDE.md` 模板补写，不得把本次文档预检冒充实施证据。

- **落地清单**：待 P0–P4 实施后填写真实文件路径与 `template_id`/slot 清单。
- **关键 commit**：待实施 commit、日期和一句话摘要。
- **测试结果**：待填写实际 client gate、资源检查、渲染回归和数量证据。
- **跨仓库核验**：当前仅确认既有 `template_id` 接线；待实施后再次核 server/client，agent 如无接触面则明确写「不涉及」。
- **遗留 / 后续**：待开放问题决议与实施验收后填写；本骨架当前不替未来阶段声明完成。
