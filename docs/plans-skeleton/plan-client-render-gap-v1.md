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

## 10. Finish Evidence

> 当前仍是 skeleton，尚无已完成阶段、实现 commit 或 gate 结果；进入 active/完成归档时按根 `CLAUDE.md` 模板补写，不得把本次文档预检冒充实施证据。

- **落地清单**：待 P0–P4 实施后填写真实文件路径与 `template_id`/slot 清单。
- **关键 commit**：待实施 commit、日期和一句话摘要。
- **测试结果**：待填写实际 client gate、资源检查、渲染回归和数量证据。
- **跨仓库核验**：当前仅确认既有 `template_id` 接线；待实施后再次核 server/client，agent 如无接触面则明确写「不涉及」。
- **遗留 / 后续**：待开放问题决议与实施验收后填写；本骨架当前不替未来阶段声明完成。
