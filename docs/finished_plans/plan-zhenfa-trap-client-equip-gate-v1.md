# plan-zhenfa-trap-client-equip-gate-v1

> 主题：`warning_trap` / `blast_trap` / `slow_trap`（以及同批漏收的 `array_flag`）在正常 craft 链路里可产出，但 client `InventoryEquipRules.TOOL_TEMPLATE_IDS` 白名单不识别它们为 tool，导致**手槽装不上**；server 又按 `ItemCategory::Tool` 禁止它们进 hotbar，玩家做出来后实际无法进入手槽使用——不是手感差，是内容不可用。
>
> 验证状态：2026-07-04 bughunt 线程 H 读码确认，已做两轮默认怀疑式证伪，未找到 client 侧绕过手槽装备的合法路径。2026-07-04 promote 收口时补做 server 全量 `category = "tool"` 资产审计，发现 `array_flag` 同属此类未收录漏项且**当前可通过 `zhenfa.flag.basic` 配方正常合成**，并入本 plan 一并修；`niche_house_puppet` / `niche_zhenfa_trap_{basic,middle,advanced}` 亦为 `category = "tool"` 但零 craft/loot 接入（仅 `/give` dev-only 可得，非正常玩法可产出），明确排除出本 plan 范围。

| 阶段 | 主题 | 状态 |
|---|---|---|
| P0 | client tool 识别口径与 server 对齐 | ✅ 2026-07-26 |
| P1 | 装备/快捷栏/提示文案回归 | ✅ 2026-07-26 |
| P2 | zhenfa 陷阱端到端可用性回归 | ✅ 2026-07-26 |

## 接入面

- **进料**：`server/assets/items/zhenfa.toml`（`warning_trap` / `blast_trap` / `slow_trap` / `array_flag`，均 `category = "tool"`）→ `server/src/craft/mod.rs::register_zhenfa_content_recipes`（三陷阱）+ `register_zhenfa_v2_recipes`（`array_flag`，配方 id `zhenfa.flag.basic`，`server/src/craft/mod.rs:519-533`）产出实物 → 玩家背包。
- **出料**：client `InventoryEquipRules.isTool()` 判定 → `canEquip()` 放行手槽 → `preferredWeaponQuickEquipSlot()` quick-equip 选槽 → `InspectScreen` 拖拽/quick-equip 落地 → `InventoryStateStore.snapshot().equipped()` mainHand 快照 → `MixinClientPlayerInteractionManagerAlchemy.bong$alchemyInteractBlock()` 右键分支 → `ClientInteractionItemResolver.zhenfaKindForItem(mainHand)` → `ZhenfaLayoutScreen` → `ClientRequestSender`/`ClientRequestProtocol.encodeZhenfaPlace` 出网络请求。
- **共享类型 / event**：不新建白名单机制，复用 `InventoryEquipRules.TOOL_TEMPLATE_IDS`（已有 `FALSE_SKIN_TEMPLATE_IDS`/`SHIELD_TEMPLATE_IDS` 同构先例，client 无 server `ItemRegistry` 访问权，靠静态白名单镜像是本仓既定模式，见 `InventoryEquipRules.java:54-55` 采矿/伐木工具补录注释）。本 plan 不引入共享真相源/codegen（§8.1 #1 决议）。
- **跨仓库契约**：无 IPC schema / Redis key / proto 改动。契约面仅是 `server/src/inventory/mod.rs` 的 `ItemCategory::Tool`（`validate_move_semantics` 禁止进 hotbar 行 4994-4996、`validate_equip_to` 允许进 hand 行 5104）与 client `TOOL_TEMPLATE_IDS` 静态镜像对齐；右键布置分支（`zhenfa_place` wire envelope）本身不改。
- **worldview 锚点**：阵法/凡阶道具产出链路（zhenfa 系统），纯 UI/装备判定缺口，不涉及境界推进或经济锚点。
- **qi_physics 锚点**：N/A——本 plan 不动真元流转/衰减，纯 client 装备判定白名单补录 + 回归测试。

## 读码证据

- `client/src/main/java/com/bong/client/inventory/InventoryEquipRules.java`
  - `TOOL_TEMPLATE_IDS`（46-64 行）仅含旧工具与采矿/伐木工具；**不含** `warning_trap` / `blast_trap` / `slow_trap` / `array_flag`
  - `canEquip()`（114 行起）主手/副手放行条件依赖 124 行 `isTool(itemId)`
  - `canPlaceIntoHotbar()`（198-201 行）也依赖 `isTool(itemId)` 判断客户端能否放进 hotbar
  - `preferredWeaponQuickEquipSlot()`（178-196 行）quick-equip 选槽同样依赖 `canEquip` → `isTool`
- `client/src/main/java/com/bong/client/inventory/InspectScreen.java`
  - 拖到装备槽走 `isEquipSlotDropValid()`（3584 行）`-> InventoryEquipRules.canEquip()`
  - `quickEquipFromGrid()`（3617 行）先过 `InventoryEquipRules.isTool(item)`（3620 行），未识别为 tool 时直接 return
- `client/src/main/java/com/bong/client/mixin/MixinClientPlayerInteractionManagerAlchemy.java`
  - `bong$alchemyInteractBlock()` 90 行读 `InventoryStateStore.snapshot().equipped().get(EquipSlotType.MAIN_HAND)` 得 mainHand 快照，106 行 `ClientInteractionItemResolver.zhenfaKindForItem(mainHand)` 命中即打开 `ZhenfaLayoutScreen`——**该分支本身链路完整**，前提是物品先被 `InventoryEquipRules` 放进 `MAIN_HAND` 槽（§8 ② 已核实，见 §8.1 #2）
- `server/src/inventory/mod.rs`
  - `validate_move_semantics()`（4947 行起）4994-4996 行明确 `ItemCategory::Tool` **禁止进 hotbar**
  - `validate_equip_to()`（5065 行起）5104 行明确 `ItemCategory::Tool` **允许进 main/off/extra hand**
- `server/src/craft/mod.rs`
  - `register_zhenfa_content_recipes()`（588 行）正常注册 `warning_trap` / `blast_trap` / `slow_trap` 配方与产物（`register()` 101 行调用，启动时必跑）
  - `register_zhenfa_v2_recipes()`（439 行）519-533 行注册 `zhenfa.flag.basic` 配方产出 `array_flag`（`register()` 98 行调用，启动时必跑）
- `server/assets/items/zhenfa.toml`
  - `array_flag` / `warning_trap` / `blast_trap` / `slow_trap` 四个物品在 item registry 里都声明为 `category = "tool"`
- `server/assets/items/niche/house_puppet.toml`、`niche/zhenfa_trap_{basic,middle,advanced}.toml`
  - 同为 `category = "tool"`，但 `grep -rn` `server/src` 零命中（无 craft 配方/无 loot table 接线）——仅 dev-only `/give` 可得，非正常玩法产出链路，**排除出本 plan 范围**（§8.1 #1 决议）

## 玩家影响

- 玩家按正常玩法做出 `warning_trap` / `blast_trap` / `slow_trap` / `array_flag` 后，会遇到 client 装备入口和 server 权威校验背离：
  - 拖到手槽被 UI 拒绝
  - 尝试拖到 hotbar 即使 client 乐观放行，server 也会回 `forbidden_in_hotbar`
- 结果不是"手感差"而是**内容不可用**：陷阱符/阵旗产出后无法稳定进入实际布阵/放置链路。

## P0 client tool 识别口径与 server 对齐 ✅ 2026-07-26

- 交付物：`client/src/main/java/com/bong/client/inventory/InventoryEquipRules.java:46-64` `TOOL_TEMPLATE_IDS` 补 `warning_trap` / `blast_trap` / `slow_trap` / `array_flag` 四项，附行内注释说明来源（zhenfa.toml category=tool，`register_zhenfa_content_recipes`/`register_zhenfa_v2_recipes` 正常产出）。
- 验收抓手：
  - `InventoryEquipRules.canEquip()` 对四种物品主手放行
  - `preferredWeaponQuickEquipSlot()` 能为四种物品选出手槽
  - `canPlaceIntoHotbar()` 对四种物品返回 `false`（与 server `forbidden_in_hotbar` 契约对齐）
- 测试：`client/src/test/java/com/bong/client/inventory/InventoryEquipRulesTest.java` 新增 4 条 pin case（仿现有 118-126 行 `toolCanEquipMainHandButNotHotbarOrArmor` / 138-148 行 `stonePickaxeCanEquipBothHands` 模式）：`warningTrapCanEquipMainHandNotHotbar` / `blastTrapCanEquipMainHandNotHotbar` / `slowTrapCanEquipMainHandNotHotbar` / `arrayFlagCanEquipMainHandNotHotbar`，每条断言 `isTool` + `canEquip(MAIN_HAND)` + `!canPlaceIntoHotbar`。

## P1 装备/快捷栏/提示文案回归 ✅ 2026-07-26

- 校准 client 行为一致性：
  - `InspectScreen.isEquipSlotDropValid()`（3584 行）/ `quickEquipFromGrid()`（3617 行）无需改代码——均转调 `InventoryEquipRules.isTool/canEquip`，P0 补白名单后自动生效；本阶段只补回归测试锁住这条自动生效路径，防止未来重构改动调用链而悄悄脱钩
  - hotbar/quick-use 高亮与落点判定不再给出误导性绿灯（`canPlaceIntoHotbar` 已在 P0 锁死为 false）
- 验收抓手：
  - `InspectScreen` 拖拽 `warning_trap`/`blast_trap`/`slow_trap`/`array_flag` 到装备槽成功
  - `InspectScreen` 不再把四种物品当普通可进 hotbar 的杂物（`canPlaceIntoHotbar` 回归覆盖）
- 测试：`client/src/test/java/com/bong/client/inventory/InspectScreenPackOpenInteractionTest.java`（或同目录既有 InspectScreen 拖拽测试文件，视 P0 收尾时目录现状而定）补至少 1 条四种物品之一（`warning_trap`）拖拽装备槽成功 + hotbar 落点被拒的端到端 UI pin。

## P2 zhenfa 陷阱端到端可用性回归 ✅ 2026-07-26

- 端到端锁定正常游玩链：
  - craft 产出 `warning_trap` / `blast_trap` / `slow_trap` / `array_flag`
  - 从背包装备到手槽
  - 右键世界交互链正确命中对应 zhenfa request（`MixinClientPlayerInteractionManagerAlchemy.bong$alchemyInteractBlock()` 106 行分支，链路已确认完整，见 §8.1 #2，本阶段只需补 mainHand 快照来源已正确装备的前置条件测试，不改交互代码）
- 验收抓手：
  - client 单测：`InventoryEquipRules` / `InspectScreen` 回归（P0/P1 已覆盖）
  - server 侧 pin：`server/src/inventory/mod.rs:6380` `loads_item_registry_from_assets()` 测试内 6557-6577 行 `required_tool` 列表补充 `warning_trap` / `blast_trap` / `slow_trap` / `array_flag`，锁住 `ItemCategory::Tool` 解析契约（该测试当前只覆盖 7 个旧工具，遗漏本 plan 涉及的 4 个 zhenfa 物品）

## §8 开放问题（已在 §8.1 收口）

1. 只补这三种陷阱，还是把所有真实 `ItemCategory::Tool` 统一改成非白名单/共享真相源？
2. `array_flag` / `niche_house_puppet` 等同类 tool 漏项是否并入同一 PR，还是作为 follow-up 另立条目？

全部已在 §8.1 收口。原表保留以备追溯，**实施时以 §8.1 决议为准**。

## §8.1 决议（pre-P0 收口，2026-07-04）

### #1 只补三种陷阱，还是统一改共享真相源？

**决议**：
1. **不引入共享真相源/codegen**，维持既有静态白名单模式——但本 PR 必须做一次**全量 `category = "tool"` 资产审计**并把审计结果落地，而不是只手工加 plan 里点名的三项。
2. 审计结果（`python3` 扫 `server/assets/items/**/*.toml` 全部 `category = "tool"` 条目）：`cai_yao_dao`/`bao_chu`/`cao_lian`/`dun_qi_jia`/`gua_dao`/`gu_hai_qian`/`bing_jia_shou_tao`/`axe_bone`/`axe_iron`/`axe_copper`/`pickaxe_bone`/`pickaxe_iron`/`pickaxe_copper`/`stone_pickaxe`/`stone_axe` 均已在 `TOOL_TEMPLATE_IDS` 中；缺口只有 `zhenfa.toml` 的 4 项（`array_flag`/`warning_trap`/`blast_trap`/`slow_trap`）和 `niche/*.toml` 的 4 项（`niche_house_puppet`/`niche_zhenfa_trap_basic`/`niche_zhenfa_trap_middle`/`niche_zhenfa_trap_advanced`）。
3. 拒绝共享真相源方案的理由：client 无 server `ItemRegistry` 运行时访问权（同构先例见 `FALSE_SKIN_TEMPLATE_IDS` 76-82 行注释"客户端无 ItemRegistry，靠此白名单"），要做到"真"共享真相源需要新增构建期 codegen（如 schema 导出 tool id 列表）或运行时 IPC 查询，两者都超出本 plan"medium"量级 bug 修复的范围，且会引入新的跨仓库契约面（本 plan §接入面已声明"无 IPC schema 改动"）。留作独立 plan 的候选项，本 plan 不做。

**落点**：`client/src/main/java/com/bong/client/inventory/InventoryEquipRules.java:46-64`（P0 交付物）/ plan §P0。

### #2 `array_flag` / `niche_house_puppet` 等同类漏项是否并入本 PR？

**决议**：
1. **`array_flag` 并入本 PR**——它当前可通过 `zhenfa.flag.basic` 配方（`server/src/craft/mod.rs:519-533`，属 `register_zhenfa_v2_recipes`，`register()` 98 行调用，启动时必跑）正常合成，与三种陷阱同属"正常玩法可产出但 client 不识别为 tool"的同一缺陷类，不并入会留下同类回归。
2. **`niche_house_puppet` / `niche_zhenfa_trap_{basic,middle,advanced}` 不并入**——`grep -rn "<id>" server/src --include=*.rs` 对四者均零命中（无 craft 配方、无 loot table、无任何生产链路引用），仅 CLAUDE.md「Dev test commands」`/give <template_id>` dev-only 入口可获得。CLAUDE.md 明确该类命令"绕过...不允许复用到生产 gameplay 路径"，故它们不构成"正常玩法产出但用不了"的玩家可感知缺陷，超出本 plan"玩家影响"框定的范围。留作独立 follow-up（若这几个 niche 物品未来接入真实 craft/loot 链路时，应在那次接入的 PR 里一并补白名单，而非现在为不可达内容预先补丁）。
3. 右键世界交互链验证（原开放问题隐含的"确认右键布置读 mainHand 的分支链路"）：已核实 `MixinClientPlayerInteractionManagerAlchemy.bong$alchemyInteractBlock()`（90-116 行）读取 `InventoryStateStore.snapshot().equipped().get(EquipSlotType.MAIN_HAND)` 作为 mainHand 快照，命中 `ClientInteractionItemResolver.zhenfaKindForItem(mainHand)` 即弹出 `ZhenfaLayoutScreen`——**该分支链路本身完整无缺口**，唯一断点是"物品能否先被装进 MAIN_HAND 槽"，即 §8.1 #1/#2 所修的白名单缺口。P2 无需改交互代码，只需回归测试锁住"装备成功后交互链可达"这一条件。

**落点**：`client/src/main/java/com/bong/client/inventory/InventoryEquipRules.java:46-64`（P0，array_flag 与三陷阱同批加入）/ `client/src/main/java/com/bong/client/mixin/MixinClientPlayerInteractionManagerAlchemy.java:90-116`（验证锚点，本 PR 不改动此文件）/ plan §P0 / §P2。

## 验证结论（2026-07-26 整理审计追认）

修复已在 origin/main 落地：client `InventoryEquipRules.TOOL_TEMPLATE_IDS` 补齐 `warning_trap` / `blast_trap` / `slow_trap` / `array_flag` 四项白名单，`server/src/inventory/mod.rs:7315-7320` 的 `required_tool` pin 同步收录，堵住了 plan 指出的"正常玩法可产出但 client 装备判定不识别"的缺口。对应 PR #861（2026-07-04，promote+决议）与 PR #962（2026-07-06，修复实装）均已 merge，`InspectScreenPackOpenInteractionTest` / `InventoryEquipRulesTest` 对应 pin 落地。

## Finish Evidence

- **落地清单**：`client/src/main/java/com/bong/client/inventory/InventoryEquipRules.java:46-64`（`TOOL_TEMPLATE_IDS`）、`server/src/inventory/mod.rs:7315-7320`（`required_tool` pin）
- **关键 commit/PR**：#861（2026-07-04，promote+决议）、#962（2026-07-06，修复已 merge）
- **测试结果**：`InspectScreenPackOpenInteractionTest` / `InventoryEquipRulesTest` 对应 pin；2026-07-26 审计为只读核验（Read+grep+git log 对拍 origin/main），未重跑测试套件
- **跨仓库核验**：server+client（`InventoryEquipRules.java` / `inventory/mod.rs`）
- **遗留 / 后续**：无
