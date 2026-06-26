# plan-layered-equip-v1 — 分层/叠加装备槽（穿戴层 + 手持态）

> 装备槽从「单槽单件」改为「单槽多件分层」：每槽 = `worn`(穿戴层，Vec，**栈语义 LIFO，尾=顶，仅栈顶可操作**) + `held`(手持，Option)。伪皮归胸槽 worn 层（蜕壳流读取点从 `EQUIP_SLOT_FALSE_SKIN` 专槽改为扫 CHEST worn 层）；双手武器（spear/staff）放一手 held → 锁对侧手；法宝从装备面板的 `treasure_belt` 槽移除，激活态改由灵宝 UI 内新建的「触发位」承载。混装各自生效（加性）。wire 表示 = 方案B（每槽 worn repeated + held optional，不污染 InventoryItemView）。源于用户真机测试反馈。**本 plan 在装备侧取代 PR #714（commit a5eef15b，已 merge）的「行囊」tab 结构**：删除行囊 tab，**背包不再有专属装备槽**——每件背包按其 `ContainerSpec.equip_slot` 指定的**身体部位**（head/chest/legs/feet 之一）作该身体槽的一个 **worn 层**穿戴（计入该槽 worn_cap，穿上后仍生成容器网格，右侧容器 tab 照旧），`body_pocket` 改默认常驻容器 tab 的第一个（默认/最左激活页）（见 §11.1 #13/#17/#18）。

> **决议 #17 简化效果**：取消 `back_pack/waist_pouch/chest_satchel` 三个专属装备槽（连同 wire 槽名 / proto·rust·agent-ts snapshot 字段 / client `EQUIP_SLOT_BY_WIRE_NAME` 三条映射 / EquipSlotType·EquipSlotV1 三个变体一并删除）。背包骑身体槽 worn 层后，全链触点**变少**（不再补背包槽 wire / 不再补 3 个 snapshot 字段），rebuild_containers / compute_max_weight 从「按固定背包槽 key 查」改为「扫所有身体槽 worn 层里带 `container_spec` 的件」。详 §11.1 #17。

## 阶段总览

| 阶段 | 内容 | 状态 | 验收日期 |
|------|------|------|----------|
| P0 | 数据模型 + wire schema（equipped → SlotContents{worn:Vec, held:Option}）+ 存档迁移 | ✅ | 2026-06-26 |
| P1 | 装备校验 + 分层规则（worn 栈 LIFO 仅顶可卸 / held 互斥 / 双手锁对侧手 / 多臂手槽） | ✅ | 2026-06-26 |
| P2 | 蜕壳流 retarget（伪皮归胸槽 worn 层，v2 + v1 全点）+ 移除 FALSE_SKIN 专槽 | ✅ | 2026-06-26 |
| P3 | 效果叠加语义（混装各自生效）+ 负重核验 + equipped 真元 carrier 守恒求和 | ✅ | 2026-06-26 |
| P4 | 客户端面板重构（删行囊 tab + 背包随身体槽 worn 层渲染 + body_pocket 默认容器 tab 第一个 + 删行囊重量条沿用 BottomInfoBar）+ worn 栈叠放渲染 + 法宝激活触发位（灵宝 UI 内） | ✅ | 2026-06-26 |
| P5 | 容量升级 hook（worn_cap 由常量 → 可升级派生值） | ✅ | 2026-06-26 |

> **worldview 前置**：`docs/worldview.md` §545 / §468 / 分层装备锚点的补写，按 docs/CLAUDE.md §6.3 **人工单独 PR 先 land**，不进自动 consume（见 §11 决议 #10 + §10）。

> **聚焦验证产出（2026-06-25 折入）**：装备功能联动全普查见 **§12 装备功能联动总册**（31 条，逐项标 read_site + 分层取值桶 + 已覆盖 / 缺口）。14 条 confirmed_gaps（3 blocker + 6 major + 5 minor）已按 plan_fix 折进对应 §阶段/桶（见各阶段块行内标注 + §12 表「缺口」列指向）；8 条防御 / 负重公式写死在 **§P3**；死代码纠偏（rebuild_containers_from_equipment 运行时是死代码 / attrition_exempt 链路 / 背包破损簇）折进 §P0.1/§P0.2 + §11.1 #13.5/#17/#20。验证已剔除 3 条 real=false（背包件防御=0 自动 skip、CHEST cap 共享设计取舍、rebuild reconcile 过度设计），不折进。

---

## 二、接入面 checklist

### 进料（从哪些现有模块取数据 / 物品 / event）
- `inventory::PlayerInventory.equipped`（mod.rs；本 plan 主改对象，HashMap<String, ItemInstance> → HashMap<String, SlotContents>）
- `inventory::ItemRegistry` → `WeaponSpec`（mod.rs:245）/ `ItemCategory`（mod.rs:286）：派生 worn vs held 分类、双手判定
- `inventory::LoadoutSpec.equipped`（mod.rs:391）+ `server/assets/inventory/loadouts/default.toml`：默认 loadout 模板（第二处同型字段）
- `combat::tuike::false_skin_kind_for_item`（tuike.rs:120）：判定 CHEST worn 层里哪件是伪皮
- `combat::weapon::WeaponKind`（weapon.rs:19）：spear/staff 双手派生源

### 出料（产出去哪里）
- `combat::DerivedAttrs`（防御/属性派生，混装各自生效加性）
- `combat::tuike::FalseSkin` / `combat::tuike_v2::StackedFalseSkins` 组件（伪皮装备态 → 蜕壳流；**复用既有组件，不新造**）
- `inventory::spirit_treasure::ActiveSpiritTreasures` / scan 出的 `SpiritTreasureEntry`（法宝被动激活；承载从 equipped 槽改为「触发位」）
- wire snapshot → client：`EquippedInventorySnapshotV1`（server→agent→client）+ `WeaponEquippedV1` / `TreasureViewV1` payload

### 共享类型 / event（复用，不另建）
- `FalseSkin`（tuike.rs:131）/ `StackedFalseSkins`（tuike_v2）— 蜕壳流组件，retarget 读取来源即可，**禁止新造平行组件**
- `ShedEvent`（tuike.rs；combat/mod.rs:223 注册）— 蜕层事件不变
- `ItemInstance` / `ItemCategory` / `WeaponSpec` — 物品本体不变
- `InventoryLocationV1`（schema/inventory.rs:206）— C2S move 落位，**扩字段不新建类型**
- `EquipSlotV1::BackPack|WaistPouch|ChestSatchel`（client `EquipSlotType.BACK_PACK|WAIST_POUCH|CHEST_SATCHEL`，EquipSlotType.java:16-18）— **决议 #17：取消这三个背包专属装备槽枚举，删除**（连同 FALSE_SKIN/TWO_HAND/TREASURE_BELT 一起删，详 §11.1 #17）。背包改骑身体槽（head/chest/legs/feet）worn 层，**不再补背包槽 wire**——client `EQUIP_SLOT_BY_WIRE_NAME` 只需补 extra_hand_0/1 两条，背包随对应身体槽的 `<bodyslot>_worn` 数组下发。决议 #13.2 旧「补 back_pack/waist_pouch/chest_satchel wire」随 #17 作废，详 §P4 + 跨仓库契约表
- `ContainerSpec.equip_slot`（server mod.rs:227 String）— **决议 #17：从指向背包专属槽（back_pack/waist_pouch/chest_satchel）改为指向身体槽（head/chest/legs/feet 之一，由背包物品自身决定）**。TOML 校验 `valid_slots`（mod.rs:1736-1741）随之改；现有配 container_spec 的物品（`worn_grass_pouch`/`grass_pouch`，core.toml:384-407，现 `equip_slot="back_pack"`）落哪个身体槽列开放点（物品数据层实现细节，§11.1 #17）
- `InventoryModel.BODY_POCKET_CONTAINER_ID`（client model/InventoryModel.java:20，容器 id 非 EquipSlotType）— body_pocket 由 #714 的「行囊面板内 grid」改回**默认常驻容器 tab，且为容器 tab 列表第一个（默认/最左激活页）**（决议 #13/#18，调和 #714「不单独占 tab」张力）

### #714 取代关系（已 merge，commit a5eef15b，仅动 InspectScreen.java +83/-33）
- 本 plan 在**装备侧取代** #714 引入的「行囊」tab 结构：删除行囊 tab + 行囊面板全部客户端构件（清单见 §P4）。**决议 #17**：不再「把背/腰/胸 3 个背包 equip 槽迁进主装备面板」——背包专属槽取消，每件背包按 `ContainerSpec.equip_slot` 指定的身体槽（head/chest/legs/feet）作该槽 worn 层渲染（与盔甲/伪皮同槽叠层）；body_pocket 改默认容器 tab 第一个（决议 #13/#18）；行囊重量条删除，沿用整体 inventory 底部 `BottomInfoBar`（决议 #19）。
- 行囊面板构件全集中在 `InspectScreen.java`（无独立 `TAB_BACKPACK` 常量；行囊是右列容器 tab，靠 `activeContainer == containerCount` 哨兵标识激活）。决议 #13 给出删除清单。

### 跨仓库契约（server / agent / client 逐 symbol）
| 契约 | server | agent | client | proto |
|------|--------|-------|--------|-------|
| equipped 快照 | `EquippedInventorySnapshotV1`（schema/inventory.rs:174；**删 back_pack/waist_pouch/chest_satchel 字段（决议 #17），身体槽 worn 化，仅补 extra_hand_0/1**） | `EquippedInventorySnapshotV1`（inventory.ts:240；**补 extra_hand_0/1 两字段修漂移 + 删 back_pack/waist_pouch/chest_satchel（不再 worn 化，决议 #17），身体槽 worn 化，见 §P0.4**） | `InventoryModel.equipped` + `InventorySnapshotHandler.EQUIP_SLOT_BY_WIRE_NAME`（**仅补 extra_hand_0/1 两条 wire 映射，不补背包三槽（决议 #17 取消背包专属槽，背包随身体槽 worn 数组下发）；blocker 仅剩 extra_hand_0/1 缺失，见 §P4**） | `EquippedInventorySnapshot`（envelope.proto:556） |
| 槽枚举 | `EquipSlotV1`（schema/inventory.rs:33；删 9 变体 = false_skin/two_hand/treasure_belt_0..3 6 个 + back_pack/waist_pouch/chest_satchel 3 个，决议 #17） | `EquipSlotV1`（inventory.ts:21；删 9 字面量） | `EquipSlotType`（EquipSlotType.java；删 9 变体，新增 extra_hand_0/1） | （字段命名，无独立枚举 message） |
| C2S move 落位 | `InventoryLocationV1::Equip{slot,state}`（schema/inventory.rs:212；新增 state） | `InventoryLocationV1`（inventory.ts:258，加 state） | `EquipSlotType` + drop 落位 **+ `EquipLoc.state`（ClientRequestProtocol.java:571 新增 `String state` + toJson 写 state，blocker，与 server state 必填同 PR-1，见 §P0.3 客户端清单）** | `oneof location`（envelope.proto:2586） |
| 武器装备推送 | `WeaponEquippedV1` | — | `WeaponHotbarHudPlanner` | `WeaponEquipped`（envelope.proto:1587；注释删 two_hand） |
| 法宝面板 | `treasure_equipped_emit.rs:57` | — | `TreasurePanelSync.TREASURE_SLOTS` | `TreasureView` |

### worldview 锚点
- §439 替尸·蜕壳流（防御三流）— 伪皮机制正典出处
- §468 流派主战斗变量表（伪皮档位 轻/中/重 + 单层吸收上限）— 多层伪皮叠加依据
- §545 装备 loadout（武器/护甲/伪皮/暗器载体）— 分层穿戴正典落点
- 新立「修士可层叠穿戴/分层装备」锚点（§545 段内）— 否则属无 worldview 锚点玩法（§四红旗）
- 三处补写按人工单独 PR 收口（见 §11 决议 #10 + §10）

### qi_physics 锚点
- **不引入任何新物理常数 / 衰减公式**。本 plan 只改 equipped 数据结构，物理底盘归 `qi_physics`（`plan-qi-physics-v1`）唯一实现。
- equipped 真元 carrier 守恒：分层后所有「遍历 equipped 累加真元」的求和点必须 `flat_map(worn).chain(held)`，保证同槽两件 carrier 真元 = 两件之和（P3 守恒 pin，落点 `qi_physics::ledger::inventory_qi` ledger.rs:571）。
- 多层伪皮维持的真元成本（tuike_v2 `false_skin_maintenance_tick`，combat/mod.rs:31）走既有 `release_qi_amount_to_zone` / `QiTransfer` 守恒路径，**不新增维持公式**；多层叠加只是成本累加，物理不变（P3 锚点声明）。

---

## P0 — 数据模型 + wire schema + 存档迁移

> **前置**：§11 决议门全部收口（worn/held 表示=方案B、C2S move 落位=加 state、**背包无专属槽=骑身体槽 worn 层（决议 #17）**、存档迁移、ExtraHand/worn_cap 全集）。带开放问题进 P0 = 违反 docs/CLAUDE.md §五。

### P0.1 server 数据模型
- `server/src/inventory/mod.rs`：
  - 新增 `struct SlotContents { worn: Vec<ItemInstance>, held: Option<ItemInstance> }`（serde；空槽序列化为 `{worn:[],held:null}`）。**`worn` Vec 语义 = 栈（LIFO，决议 #12）：约定栈顶 = Vec 末尾**——装备 = `worn.push(item)`（push 到尾）、卸下 = `worn.pop()`（pop 从尾），**只有栈顶（最上层 / Vec 末尾）能被拖下/卸下**，下层被压住需先脱上层。提供 `fn worn_top(&self) -> Option<&ItemInstance>`（`worn.last()`）/ `fn worn_top_mut` 作栈顶访问入口。
  - 新增 `enum EquipState { Worn, Held }`（serde rename `worn`/`held`）。
  - `PlayerInventory.equipped: HashMap<String, ItemInstance>` → `HashMap<String, SlotContents>`。
  - 新增 `fn worn_cap(slot: &str) -> u8`：head/feet = 2；chest/legs = 3；**main_hand/off_hand/extra_hand_0/extra_hand_1 = 0（held-only，不计 worn cap，见决议 #6+#14）**。**决议 #17：无背包专属槽**——背包作为身体槽（head/chest/legs/feet）的 worn 层穿戴，占其所在身体槽的一个 worn 层、受该槽 cap，不再有独立 cap=1 退化槽。**声明：held 槽 worn 恒空、worn 栈 LIFO（决议 #12）只作用于 head/chest/legs/feet worn 槽**（手槽给 worn_cap≠0 会产生「手槽能否叠件」的死码歧义，故归 0）。
  - 新增 `fn classify_equip_state(item: &ItemInstance, registry) -> EquipState`：`ItemCategory::Weapon|Tool` → Held；`Armor|Treasure|Shield`（mod.rs:286 各变体）+ 伪皮物品 → Worn；`Container`（背包）→ Worn（**决议 #17：背包占其 `ContainerSpec.equip_slot` 指定身体槽的一个 worn 层，不再是独立退化槽 worn[0]**）。规则见决议 #16。
  - 新增 `fn weapon_two_handed(kind: WeaponKind) -> bool`（或 `WeaponSpec.is_two_handed`，决议 #7）：`Spear|Staff` 派生（先例 mod.rs:4231）。
- **删除常量 + 枚举变体**：`EQUIP_SLOT_FALSE_SKIN`(mod.rs:79)、`EQUIP_SLOT_TWO_HAND`(:82)、`EQUIP_SLOT_TREASURE_BELT_0..3`(:83-86)、**`EQUIP_SLOT_BACK_PACK`(:91)/`EQUIP_SLOT_WAIST_POUCH`(:94)/`EQUIP_SLOT_CHEST_SATCHEL`(:97)（决议 #17 取消背包专属槽）**；`EquipSlotV1::FalseSkin|TwoHand|TreasureBelt0..3|BackPack|WaistPouch|ChestSatchel`。
- **EquipSlotV1 删 9 变体的全 match 站点**（6 旧 + 3 背包，grep 实测，逐一改）：
  - `equip_slot_to_wire`（mod.rs:4664-4671 区间）删 FalseSkin/TwoHand/TreasureBelt 分支
  - `wire_to_slot`（mod.rs:4525-4539）删对应反向分支
  - `validate_move_semantics` TwoHand 分支（mod.rs:4224-4258）+ FalseSkin 分支（:4260-4268）+ TreasureBelt 分支（:4247-4259）整段移除/改写（详 P1）
  - 旧存档含已删变体的 serde 反序列化 fallback（见 P0.4 迁移）
- **删常量连带处理：背包耐久磨损/破损整簇（gap#8 major，§12 补漏）**：`container_id_to_equip_slot`(mod.rs:3626) / `apply_backpack_wear`(:3644) / `handle_backpack_break`(:3682) / `slot_display_name`(:3587) / `BackpackBreakEvent`（mod.rs:3626-3720 整簇）+ 测试 `:9844-10168`（20+ 处直引常量）整簇 `#[allow(dead_code)]` 但函数体引用 `EQUIP_SLOT_BACK_PACK` 等 + 测试直引；**P0.1 删常量后编译红 + 测试红**。去向（随 #17，blocker 级编译门）：随容器 id↔worn 背包件映射重写为『容器 id → worn 背包件 instance』键、或**显式标废弃并删测试**（`handle_backpack_break` 是 `rebuild_containers_from_equipment` 的唯一 caller，整簇运行时是死代码 —— 见 §11.1 #13.5 死代码纠偏）。**删槽常量必须连带处理此簇，不能只删常量留悬空引用**。

### P0.2 equipped 160+ 触点按语义分桶（实测 256 处 `.equipped` 跨 ~49 文件，mod.rs 占大头）
> 分层后取值语义随桶而定。每桶在对应阶段逐文件改；P0 锁定分桶规则，后续阶段按桶 retarget。

| 桶 | 语义 | Vec 化后取值 | 代表触点 | 改在哪阶段 |
|----|------|--------------|----------|-----------|
| ① 取 held 武器/手槽件 | 拿单件手持武器/工具 | `.get(slot).and_then(\|s\| s.held.as_ref())`；get_mut→`.held.as_mut()`；`remove(hand_slot)`→**清 `held`（`.held=None`，不整槽 remove）** | combat/weapon.rs（current_weapon_from_inventory:169，删 two_hand 分支，详 P2）、combat/resolve.rs、weapon_equipped_emit.rs、**combat/carrier.rs（真元充能核心：carrier.rs:459/579/692/750/758/781/798/1377，get→.held.as_ref / get_mut→.held.as_mut / :798 remove→.held=None，blocker，漏改充能/降级取不到手持件静默失效或 panic，见 §P3 守恒）**、**gathering/tools.rs（:4 import EQUIP_SLOT_TWO_HAND、:238 get(TWO_HAND)、:253 filter_map(get)；删 TWO_HAND 读取、双手工具从 main_hand.held 取、main/off get→.held、测试 :381/437/524 改不插 TWO_HAND 槽）**、**spiritwood/mod.rs（:21 import、:504 `.or_else(\|\| equipped.get(EQUIP_SLOT_TWO_HAND))` 删 TWO_HAND or_else、main_hand get→.held、:720 测试改）**、**anqi_v2.rs:840 / shield_block.rs:230(off_hand) / lingtian/systems.rs:247,1499(get_mut),1515(`remove(MAIN_HAND)`→改清 held，否则整 SlotContents 被删)**、**mineral/break_handler.rs（gap#1 blocker，§12 补漏）：:33 `import EQUIP_SLOT_TWO_HAND`、:410 `equipped_pickaxe_tier` 现 `[MAIN_HAND,TWO_HAND].filter_map(get)` → 删 :33 import + 删 TWO_HAND filter_map 项、main_hand→`.held`；常量删除后此文件不改即编译红** | P1（carrier/tools/spiritwood/anqi/shield/lingtian/**mineral** 随 TWO_HAND 常量删除必须 PR-2 一并改，否则编译红） |
| ② 遍历 worn 累加 | armor 防御 / 属性派生 / 负重 / 真元求和 | `.worn.iter()` 全件 + `.held` | combat/components.rs(DerivedAttrs)、**combat/armor_sync.rs:26-42（护甲聚合 `.max()`→加性，详 §P3）**、**combat/jiemai.rs:72-83（gap#4 major，§12 修正错分桶——`jiemai_armor_modifier_from_inventory` 遍历 HEAD/CHEST/LEGS/FEET 4 护甲槽取最重 weight 决定真脉 prep 窗口修正，完全不读手持，原误列桶①；分层后 `equipped.get(slot)` 返回 `&SlotContents` → `.map(\|i\| i.weight)` 编译红，语义改 `worn.iter().map(\|i\| i.weight).fold(0.0, f64::max)`，与 body_mass/calculate_current_weight worn 化一致，归 P3）**、**combat/body_mass.rs:148-159（equipped_armor_mass，gap#13 minor：worn.iter() 后须 filter『有 ArmorProfile 的件』排除背包/伪皮自重，详 §P3）**、calculate_current_weight(mod.rs:3462)、ledger.rs:571 | P3 |
| ③ 按 slot 查单件 | 特定槽特定件 | 扫 worn 找特定件 | **forge/artifact_meridian.rs:681（gap#11 minor，§12 展开行号：`broken_slot` find_map `match TWO_HAND => EquipSlotV1::TwoHand` 双重编译红（删 TWO_HAND 常量 + 删 EquipSlotV1::TwoHand 变体），删 TWO_HAND match 项 + flat_map(worn+held) 按 instance 定位 broken weapon 槽）**、combat/armor_sync.rs、tuike(_v2)、**compute_max_weight(mod.rs:3489-3507，:3490-3494 现遍历固定背包槽常量数组 `[BACK_PACK,WAIST_POUCH,CHEST_SATCHEL]` 求 container_spec.weight_capacity，决议 #17 改为扫所有身体槽 worn 层里带 container_spec 的件 `equipped.values().flat_map(\|s\| s.worn.iter())` 求和)**、**rebuild_containers_from_equipment(mod.rs:3519-3584，决议 #17：:3536-3540 现 `for slot_id in [BACK_PACK,WAIST_POUCH,CHEST_SATCHEL]` 硬编码背包槽列表 + :3541 `.get(slot_id).and_then(\|item\|..)` 改为「扫所有身体槽 worn 层里带 `container_spec` 的件」生成容器（容器 id 用件 instance_id 或槽+层索引，不再用背包 slot 名），blocker，背包→容器网格唯一产出点，漏改背包穿上不生成容器 tab；测试 mod.rs:9254/9306/9335 rebuild_containers_* 随改)** | P0(背包+rebuild_containers)/P2(tuike) |
| ④ 迭代全件 | 全 equipped 物品扫一遍 | `.values().flat_map(\|s\| s.worn.iter().chain(s.held.iter()))` | shelflife/sweep.rs、world/block_drop.rs、world/block_place.rs、durability、tsy_death_drop.rs、clearinv/reset、**spirit_treasure.rs:238 scan（改读触发位后，若仍扫装备槽 treasure worn 件则 flat_map(worn.chain(held))，详 §P2 spirit_treasure）**、**social/niche_defense.rs:480/:518（gap#9 major，§12 补漏：灵龛守护材料计数，:480 `equipped.values().filter(\|i\| i.template_id)` count + :518 `.find(\|i\| i.template_id)`，SlotContents 化后 `.template_id` 两处编译红 → flat_map(worn.chain(held))；实施时顺手 grep social/ 其余 equipped.values() 点）**、**forge/artifact_meridian.rs:913 values_mut / :932 values（gap#11 minor，§12 展开行号：原仅桶③/④泛指 forge → :913 改 `flat_map(\|s\| s.worn.iter_mut().chain(s.held.iter_mut()))`、:932 改 `flat_map(worn.chain(held))`）**、**死亡掉落（mod.rs:3283/3306/3354 + tsy_death_drop.rs:72/194，gap#2 blocker → 见下「死亡掉落显式子任务」）** | P3(qi/sweep) + 各阶段顺手 |

- `equipped.remove(slot)` 全站点逐个明确「移整槽全件 vs 指定件」语义：mod.rs:3095（unequip→rehome）、:3354（**死亡掉落整槽 remove —— gap#2 blocker：改按 instance 精确移除，见下「死亡掉落显式子任务」(b)，禁止整槽删连带未掉落 worn 下层/held**）、:3690（**背包卸下 —— 决议 #17：背包是身体槽 worn 层，按 instance_id 在该身体槽 worn 栈定位、仅栈顶可 pop，不再整槽/退化槽 worn[0]**）、tsy_death_drop.rs:194（**秘境死亡掉落同 :3354，按 instance 精确移除**）、以及其余 remove 点。
- `move_equipped_item_to_first_container_slot`（mod.rs:2991-3010）/ unequip 路径重写：**worn 层只能 pop 栈顶**——按 instance_id 定位时，若命中件不是 `worn.last()`（被压住的下层），拒绝移动 + 特定 err 文案（决议 #12）；命中栈顶则 `worn.pop()`。`held` 单件直接精确移除（held 不分层）。**不再整槽 `.get/.remove`、不再 `.position()` 移任意中间件**（中间件被压住不可动，决议 #12 锁死 LIFO）。
- **底层 helper SlotContents 重写（LIFO pop 栈顶的实现基座，major，漏改则 §P0.2「仅 pop 栈顶」空中楼阁）**：`move_equipped_item_to_first_container_slot`(:2991) 内部调 detach_instance + attach_at_location，这 4 个 helper 当前把 equipped value 当 `&ItemInstance` 单件遍历/写入，须 SlotContents 化：
  - `detach_instance`(mod.rs:4544，:4550 `.retain(\|_, item\| item.instance_id != instance_id)`)：改区分 worn（仅命中 `worn.last()` 才 pop，命中下层拒）/ held（清 Option）；不再 retain 单件。
  - `attach_at_location`(mod.rs:4560)：按 `EquipState` 写 `worn.push`（受 cap）/ `held`。
  - `clone_item_at`(mod.rs:4395) / `inventory_item_by_instance_borrow`(mod.rs:3029)：遍历改 `flat_map(\|s\| s.worn.iter().chain(s.held.iter()))`。

#### 死亡掉落显式子任务（gap#2 blocker — 桶④升级，§12 补漏）
> 标准死亡掉落 `inventory/mod.rs:3283/3306（match TWO_HAND）/:3354（整槽 `equipped.remove(&slot)`）` + 秘境 `inventory/tsy_death_drop.rs:72（match TWO_HAND）/:194（整槽 `equipped.remove(&slot)`）`。两处 `match TWO_HAND` 删常量后编译红；**整槽 `equipped.remove(&slot)` 在 SlotContents 化后会删整槽（含未掉落的 worn 下层 / held），是数据破坏陷阱**。原桶④ 仅把 tsy_death_drop 列作「顺手」，未点名 TWO_HAND match + 整槽 remove，升级为显式子任务：
> - **(a) 武器保护判定**：两处 `match MAIN/OFF/TWO_HAND` 删 TWO_HAND 分支；改为按 instance 在 worn/held 定位命中件，判 `durability ≥ 0.5` 决定高耐真武器免 50% 掉落 Roll（双手兵器从 main_hand.held 派生，与决议 #7 一致）。
> - **(b) candidate 收集 + 精确移除**：partition 从「整槽 `equipped.remove(&slot)`」改为 `flat_map(\|s\| s.worn.iter().chain(s.held.iter()))` 收 candidate instance；掉落时**按 instance 从对应 worn 栈 / held 精确移除**——worn 件按 instance_id 在该槽 worn Vec 定位后移除该一件（保留同槽其余 worn 层）、held 命中则清 `held=None`；**禁止 `equipped.remove(&slot)` 整槽删（会连带删未掉落的 worn 下层 / held）**。
> - **(c) pin 测试**：双手武器 + 护甲叠层场景，掉落选中件后断言——① 只掉选中件、② 高耐真武器（durability≥0.5）保护件留存、③ 整槽其余 worn 件不被连带删（同槽 worn=[甲A,甲B] 掉 甲B 后 甲A 仍在）、④ held 武器与同槽 worn 独立移除。

#### 封灵容器 attrition_exempt 运行时链路（gap#3 blocker — 唯一真 wired 背包→功能联动，§12 补漏，与决议 #20 同步）
> `container_attrition_exempt`(mod.rs:4504) → `container_id_to_equip_slot(container_id)`(mod.rs:4509) → `equipped.get(slot)` → `spec.attrition_exempt`；运行时入口 `client_request_handler.rs:9499`(SlotMove) / `:9864`(Pickup) 真实调用（封灵搬运跳过 qi 磨损，是装备槽 attrition_exempt 当前唯一真 wired 的运行时联动）。**决议 #17 删 back_pack/waist_pouch/chest_satchel slot 常量 + 改容器 id 后，`container_id_to_equip_slot` 仅识别旧三固定字符串 → 恒 None → 封灵豁免静默失效**。plan 原全文 0 处提及（见 §11.1 #20）。
> - 判定从『container_id → slot 名反查 → equipped.get(slot)』改为『**容器 id 直接映射其对应 worn 背包件 instance（容器 id == 背包件 instance_id 或槽+层索引，与 rebuild_containers 容器 id 命名规则一致，见 §11.1 #13.5/#17）→ 读该件 `container_spec.attrition_exempt`**』。
> - `container_id_to_equip_slot`（mod.rs:3626 + :4509 两处同名 / 旧 slot 名反查）**重写为「容器 id → worn 背包件 instance 定位」或废弃**；验证 `client_request_handler.rs:9499/:9864` 两个运行时调用点改用新映射。
> - pin 测试：封灵背包件（`container_spec.attrition_exempt=true`）穿身体槽 worn → 其容器内物品 SlotMove/Pickup 跳过 qi 磨损（attrition_exempt=true）；非封灵背包件 → 不豁免。

### P0.3 wire schema（方案B，三端同构）
- **proto** `proto/bong/envelope.proto`：
  - `EquippedInventorySnapshot`（:556-573）现 `optional InventoryItemView` 字段 → 每个装备槽拆 `repeated InventoryItemView <slot>_worn` + `optional InventoryItemView <slot>_held`（装备槽：head/chest/legs/feet/main_hand/off_hand/extra_hand_0/extra_hand_1）。删 `false_skin`(:561)/`two_hand`(:564)/`treasure_belt_0..3`(:565-568) 字段。**决议 #17：删除 back_pack/waist_pouch/chest_satchel 三个背包槽字段（不 worn 化、不保留）**——背包件随其所在身体槽（head/chest/legs/feet）的 `<slot>_worn` 数组下发，是该身体槽的普通 worn 层。proto 当前缺 extra_hand_0/1 字段时一并新增（client/agent 漂移源头，blocker，见 §P0.4 + §P4）。
  - `WeaponEquipped`（:1587）`slot` 注释（:1588 `main_hand / off_hand / two_hand`）删 `two_hand`（决议 #25，server→client 推送非 C2S）。
  - C2S move：`oneof location`（:2586）的 equip 变体加 `EquipState state`（enum 或 string `worn`/`held`）。
- **Rust** `server/src/schema/inventory.rs`：
  - `EquippedInventorySnapshotV1`（:174）每装备槽字段 `Option<InventoryItemViewV1>` → `worn: Vec<InventoryItemViewV1>` + `held: Option<InventoryItemViewV1>`；删 false_skin/two_hand/treasure_belt 字段 + **back_pack/waist_pouch/chest_satchel 三个背包字段（决议 #17）**。
  - `EquipSlotV1`（:33）删 9 变体（6 旧 + back_pack/waist_pouch/chest_satchel，决议 #17）。
  - `InventoryLocationV1::Equip{slot}`（:212-213）→ `Equip{slot, state: EquipStateV1}`；`RawInventoryLocationV1::Equip`（:372/:380）+ `TryFrom`（:353）同步加 state（含 default 兼容旧 sample？—决议 #2 定 state 必填，旧 sample 一并改）。
  - 新增 `enum EquipStateV1 { Worn, Held }`（serde rename）。
  - **删除死常量 `CONTAINER_ID_BACK_PACK`/`CONTAINER_ID_WAIST_POUCH`/`CONTAINER_ID_CHEST_SATCHEL`（schema/inventory.rs:19/21/23，gap#14 minor，§12 补漏）**：无引用但 docstring 绑「装备 back_pack 槽产生的容器」旧契约，决议 #17 改容器 id 后误导，删除（或重命名为按 instance 的容器 id 规则注释）。
- `schema/proto_convert.rs`：`equipped_to_proto` / 反向映射重写为 worn(repeated)+held(optional)；move location 映射加 state。
- `schema/proto_gen.rs`：随 proto 重生成（`equipped` touchpoint 在此文件）。
- **client C2S `EquipLoc`（PR-1，blocker — server state 必填后客户端不发 state 则所有装备 move 反序列化失败，必须与 server state 同 PR，不能拖到 P4）**：
  - `client/.../network/ClientRequestProtocol.java`：`EquipLoc`(:571) 当前仅 `String slot` + toJson 写 kind+slot → 加 `String state`（`worn`/`held`）+ toJson 写 state。
  - 5 处调用点按 classify(worn/held) 传 state：InspectScreen.java:2448/2533/3290/3305/3767。
  - `ClientRequestProtocolTest` 补 equip-worn / equip-held 编码 pin（toJson 含 state）。
- **server C2S 落位 resolver `find_inventory_instance_location`（client_request_handler.rs:9367，gap#5 major — 生产路径，原 §P0.3 只点 schema 层未点此）**：现 `equip_slot_v1_for_runtime(slot).map(\|slot\| Equip{slot})` 构造 `Equip{slot}` 无 state，决议 #2 state 必填后**编译红 / 反序列化失败**。改为在每个 `SlotContents` 的 worn/held 里**按 instance 定位**：命中 worn → `Equip{slot, state: Worn}`、命中 held → `Equip{slot, state: Held}`；`equip_slot_v1_for_runtime` / `equip_slot_v1_for_runtime_key`（mod.rs:4521 附近）按需带 state。归 PR-1（与 schema state 必填同 PR，否则装备 move 链路全断）。

### P0.4 agent TS schema（IPC source of truth，**漏改即双端校验红**）
- `agent/packages/schema/src/inventory.ts`：
  - **先修既有漂移（blocker）**：`EquippedInventorySnapshotV1`（:240-255，`additionalProperties:false`）当前仅 12 字段。**决议 #17：缺的是 extra_hand_0/extra_hand_1 两字段**（rust schema.rs + proto 已有 → 现存 agent↔server IPC 漂移）；**back_pack/waist_pouch/chest_satchel 不再补、不再 worn 化——决议 #17 取消背包专属槽**，背包随身体槽（head/chest/legs/feet）的 worn 数组下发。**P0.4 显式交付物 = 补齐 extra_hand_0/1 两字段对齐 rust+proto + 删除背包三槽（若存在则一并删，对齐 proto/rust）+ 统一 worn/held 拆分**。
  - 每装备槽 `NullableInventoryItemViewV1` → `Type.Array(InventoryItemViewV1)`（worn）+ `NullableInventoryItemViewV1`（held）；删 `false_skin`(:246)/`two_hand`(:249)/`treasure_belt_0..3`(:250-253) + **back_pack/waist_pouch/chest_satchel（若存在，决议 #17）**。**不再有背包槽 worn 字段**（背包件出现在 head/chest/legs/feet 的 `_worn` 数组里）。
  - `EquipSlotV1`（:21）删 `false_skin`(:26)/`two_hand`(:29)/`treasure_belt_0..3`(:30-33) + **back_pack/waist_pouch/chest_satchel 字面量（决议 #17）**。
  - `InventoryLocationV1`（:258）equip 变体加 `state` 字段（:271 区域）。
- `npm run build -w @bong/schema` 重 export JSON Schema → `cd agent/packages/schema && npm test` 对拍绿（否则双端校验即红）。

### P0.5 双端 sample（实为 4 份 + 新增 move-intent equip 变体）
- 重写 `agent/packages/schema/samples/inventory-snapshot.sample.json` + `server-data.inventory-snapshot.sample.json` 的 equipped block 为 worn/held（**含 chest 槽叠 2 worn + 1 held 非空样例**（其中一件 worn 可为背包件，示意背包骑身体槽 worn 层，决议 #17）+ main_hand held 武器样例 + **新补的 extra_hand_0/extra_hand_1 字段样例（决议 #17：仅补这 2 个，不含背包三槽）**，与 §P0.4 补字段同步，否则 additionalProperties:false 校验红）。
- 同步 `inventory-event.sample.json` + `server-data.inventory-event.sample.json`（move-intent 内 equip location）。
- 新增 `move-intent` 的 `equip-worn` / `equip-held` 正反 sample（两态各一）。
- `schema/inventory.rs` roundtrip pin 测试随改（equip-worn/equip-held 双向）。

### P0.6 存档迁移（决议 #4，§11.1 收口后落地）
- `server/src/player/state.rs`：
  - 定 `const INVENTORY_SCHEMA_VERSION: i64`（现值 +1 bump；现存档 schema_version 列已存在，save 路径 state.rs:704+ 已写）。
  - `load_player_inventory_from_sqlite`（state.rs:1090，现 SELECT 仅 `inventory_json` 于 :1095、裸 `serde_json::from_str::<PlayerInventory>` 于 :1115）改：先 `SELECT inventory_json, schema_version`；旧版本走 `migrate_equipped_v1_to_v2(Value)` 再反序列化。
  - 新增 `fn migrate_equipped_v1_to_v2(v: &mut Value)`：每槽 `object`(单件) → `{worn:[object], held:null}` 或 `{worn:[], held:object}`（按 classify_equip_state 判定）。旧专槽映射去向**定死**：
    - `false_skin` → `chest.worn` 追加一件
    - `two_hand` → `main_hand.held` + `off_hand` 标记 lock（迁移期 lock 由 P1 状态机在 load 后重算，迁移只落 main_hand held）
    - `treasure_belt_0..3` → 法宝激活态承载（「触发位」，决议 #8；迁移落 active store / 触发位，不进装备槽 worn）
    - **`back_pack/waist_pouch/chest_satchel`（决议 #17：旧专槽迁移）→ 旧件按 registry 查其 `ContainerSpec.equip_slot` 重定向到的身体槽（head/chest/legs/feet），`push` 到该身体槽的 worn 栈尾（受该槽 cap；现存档背包件均 `back_pack`，按物品数据层定的身体槽落位）；找不到 container_spec 的旧件按 classify 落对应身体槽 worn**——不再有 back_pack/waist_pouch/chest_satchel 槽 key 存在于迁移后结构
    - `extra_hand_0/1` → 武器落 `held`（**不误塞多件**）
- `LoadoutSpec.equipped`（mod.rs:391）+ `instantiate_loadout`（mod.rs:987-988）+ `load_default_loadout`（mod.rs:1199）+ `server/assets/inventory/loadouts/default.toml`（**TOML 不是 JSON**）：模板单件 push 进对应槽 `worn` 栈尾、武器落 `held`。**default.toml 现为 `[[equip]]` 数组表（array-of-tables，每条 `slot`/`template_id`/`durability`，default.toml:81-93；非内联 `[equipped] slot=item` 表）**——保持 `[[equip]]` 数组结构，每 slot 实例化后落 SlotContents：武器（现 `slot=main_hand/template_id=iron_sword`）→ `held`、其余 → `worn` 栈尾。**决议 #17：现 `slot=back_pack/worn_grass_pouch` 改为 `slot=<身体槽>/worn_grass_pouch`**（slot 改为 worn_grass_pouch 的 `ContainerSpec.equip_slot` 重定向到的身体槽，与物品数据层一致；worn_grass_pouch 具体落 chest 还是另立身体槽 = 物品数据实现细节，§11.1 #17 开放点）；`slot=chest/fake_spirit_hide` 伪皮 → `chest.worn`（与 P2「伪皮归胸」一致）。`instantiate_loadout`(:987) for-loop 同步改 SlotContents 装配。
  - **静态 `[[containers]]` id 对齐（gap#10/#15，§12 补漏）**：default.toml:32 `[[containers]] id=back_pack` 是运行时背包容器**真实来源**（`instantiate_inventory_from_loadout` mod.rs:967-985 静态拷贝），非 rebuild。决议 #17 改 `[[equip]]` 背包件 slot 为身体槽后，**静态容器 `id=back_pack` 与背包件新 instance / 身体槽命名空间须对齐**（容器 id 与穿戴背包件可互查，否则孤儿容器 / 背包件不生成正确容器 tab + 封灵 attrition_exempt 反查失配）；按 §11.1 #13.5 死代码纠偏方向 (A)/(B) 二选一定命名规则。本 plan 不引入「rebuild reconcile」协调层（验证已判 real=false 过度设计）。

### P0 测试
- `inventory::*` SlotContents 序列化/反序列化 roundtrip（空槽 / 单 worn / 多 worn / worn+held / 仅 held）。
- `worn_cap` 各槽边界（head=2 / chest=3 / legs=3 / feet=2 / 手槽 main_hand/off_hand/extra_hand_0/extra_hand_1=0；**决议 #17：无背包专属槽，背包计入其所在身体槽 cap，不单测「背包=1」**）。
- schema sample 正反对拍（4 份 + move-intent equip-worn/equip-held）。
- 旧格式 fixture 升级测试：`migrate_equipped_v1_to_v2` 对每类旧槽（false_skin→chest worn / two_hand→main_hand held / treasure_belt→触发位 / **背包旧专槽 back_pack/waist_pouch/chest_satchel→按 container_spec 重定向身体槽 worn 栈，断言迁移后无背包槽 key、件落对应身体槽 worn，决议 #17**）逐一断言去向。
- `load_player_inventory_from_sqlite` schema_version 分流（旧版本触发 migrate、新版本直读）。
- default.toml 加载后结构与 PlayerInventory 一致（instantiate_loadout 单件 push worn 栈尾、武器落 held、**worn_grass_pouch 落其身体槽 worn 而非背包槽，决议 #17**）。
- `agent/packages/schema` npm test 双端对拍绿。

---

## P1 — 装备校验 + 分层规则

- `server/src/inventory/mod.rs` `validate_move_semantics`（mod.rs:4151-4321）Equip 分支按 SlotContents 重写：
  - **worn 层 = 栈（LIFO，决议 #12）**：装备 = `worn.push(item)`（push 到尾 = 栈顶，受 `worn_cap`）；目标槽 `worn.len() < worn_cap(slot)` 才接受；满 = 拒绝带文案「该部位已穿戴 {cap} 层，无法再叠加」（决议 #3 拒绝不顶替）。**从 worn 槽移出/卸下只允许栈顶件**（`worn.last()`）：移动非栈顶（被压住的下层）= 拒绝带特定文案「{上层件名} 压在上面，请先脱下上层」；脱掉栈顶后下一层（新 `worn.last()`）成为新的可操作/可显示顶层。`move_equipped_item_to_first_container_slot` / unequip 路径按「只能 pop 栈顶」重写（详 P0.2）。
  - **held**（决议 #3 拒绝不卸）：目标槽 `held.is_some()` = 拒绝带文案「该手已持械，请先卸下」。held 不计 worn cap。
  - **手持槽全集**（决议 #6）：`main_hand / off_hand / extra_hand_0 / extra_hand_1` 都是 held 槽（每槽 ≤1）。现 `ExtraHand0|ExtraHand1` 走与 MainHand 同分支（mod.rs:4307-4319），改为 held 语义。
  - **双手武器**（决议 #7）：`weapon_two_handed(kind)` 为真且放进 main_hand/off_hand 任一 held → 标记对侧主/副手 lock（**extra_hand 独立不锁**）；拖入被锁手 = 拒绝带文案「双手兵器占用双手，对侧已锁定」。迁移现 from_two_hand swap 路径（mod.rs:4163-4242，TwoHand 专槽 + from_two_hand 旗标）到「一手 held + 对侧 lock」模型；删 `EquipSlotV1::TwoHand` 校验段（:4224-4258）。
  - 武器槽判定（mod.rs:3287-3306，现 match `MAIN_HAND|OFF_HAND|TWO_HAND`）删 TWO_HAND，加 extra_hand。
  - `compute_swap_*` / `attach_at_location` 改为向 `worn` Vec 追加（满则拒）/ 写 `held`（占则拒），而非整槽覆盖。
- **lock 态承载**：lock 是派生态（双手 held 武器 → 对侧手 disable），不落存档独立字段；load 后由状态机从 held 武器双手属性重算（迁移期同此，见 P0.6）。
- **背包件**（决议 #17 取代旧决议 #5 退化槽）：背包**无专属槽**，作为其 `ContainerSpec.equip_slot` 指定身体槽（head/chest/legs/feet）的一个 worn 层入栈，走该身体槽通用 worn cap + LIFO 校验（与盔甲/伪皮同槽叠层，无特判分支）。`compute_max_weight`（mod.rs:3489-3507）从「特判固定背包槽读 worn[0]」改为「扫所有身体槽 worn 层里带 `container_spec` 的件求和 weight_capacity」算 max_weight。**不再有背包退化槽 worn_cap=1 / 满 1 拒 逻辑**。

### P1 测试（按 state transition 枚举）
- worn cap：head/feet 满 2 拒、chest/legs 满 3 拒；**决议 #17：背包件入身体槽 worn 栈，与盔甲/伪皮同槽 cap 共算（不再单测「背包满 1 拒」）——例：chest 已 2 件 + 拖入背包件第 3 件合法、第 4 件拒**。
- held 互斥：手槽已 held → 拒（不 swap）。
- 双手武器：放 main_hand → off_hand lock、拖入 off_hand 拒；extra_hand 不受锁；spear/staff 各一例；非双手武器（剑/斧）不锁对侧。
- worn+held 共存：同槽 worn 满 + held 一件并存合法。
- 卸下后可装：held 卸下 → 同手可装新 held；worn 移除一层 → 可再叠。
- **worn 栈 LIFO（决议 #12）**：脱栈顶（`worn.last()`）成功 → 槽剩 N-1 件；脱被压的下层（非栈顶）→ 拒绝 + 命中「请先脱下上层」文案；脱顶后再脱 → 原 worn[N-2] 成新顶可脱（验证「脱顶后下层成新顶」）。
- 拒绝文案：四类拒绝（cap 满 / held 占 / 锁手 / 脱被压下层）各自命中特定 err 文案。
- `move_equipped_item_to_first_container_slot`（mod.rs:2991）按 instance_id 在 worn/held 精确定位：worn 仅命中栈顶才移（pop），命中下层拒绝；held 单件直接移；不误移整槽、不移任意中间件。

---

## P2 — 蜕壳流 retarget + 移除 FALSE_SKIN 专槽

> **关键**：活的蜕壳流是 `tuike_v2/`（StackedFalseSkins 多层 / 3 招共用 / maintenance / residue），v1 `tuike.rs` 仍注册（combat/mod.rs:368）。两套都要 retarget。

### v2 retarget（combat/mod.rs:244 注册，当前 gameplay 真消费）
- `tuike_v2/tick.rs`:8 import + `:47` `sync_false_skin_stack_from_inventory` 现 `inventory.equipped.get(EQUIP_SLOT_FALSE_SKIN)` → 扫 CHEST 槽 `worn` 层里的伪皮物品（`false_skin_kind_for_item`，tuike.rs:120 判定）。多件伪皮 → 多层 StackedFalseSkins。
- `tuike_v2/skills.rs`:11 import + `:356` `equipped_false_skin`（don/shed/transfer_taint 三招共用入口，:66 调用）现 `:361` `inventory.equipped.get(EQUIP_SLOT_FALSE_SKIN)` → 扫 CHEST worn。
- `tuike_v2/mod.rs`:30-31 系统注册名不变（sync + maintenance），只改读取来源。
- `false_skin_state_emit.rs` 读 `StackedFalseSkins` 组件（**非直接 slot**），retarget 后组件来源不受影响——**确认这一点**（组件由 sync 写，sync 已 retarget）。
- `tuike_v2/tests.rs`（:79/:631/:640 用 EQUIP_SLOT_FALSE_SKIN）随改为 CHEST worn 装伪皮。
- **v2 damage_capacity 是 dead_code，本 plan 不接 resolve（gap#7 major，§12 范围声明）**：`tuike_v2/state.rs:201 damage_capacity` / `:210 remaining_damage_capacity` 均 `#[allow(dead_code)]`，resolve 实际只用 v1 contam filter（resolve.rs:1320），v2 伤害吸收数值**未接战斗结算**。**本 plan 只 retarget『读取来源』（单槽 EQUIP_SLOT_FALSE_SKIN → CHEST worn 多层），不接通 v2 `damage_capacity` 到 resolve（属 `tuike_v2` 自身 plan 范围，dead_code 待 tuike_v2 plan 接入）**。P2 测试『多件伪皮分层吸收』改为**只断言 `StackedFalseSkins.layers` 层数 + 各层 spirit_quality / damage_capacity 字段值**（不断言战斗吸收数值），并标注「v2 伤害吸收接入 resolve 不在本 plan 范围」。

### v1 retarget（combat/mod.rs:368 注册）
- `tuike.rs`:17 import + `sync_false_skin_from_inventory`（**tuike.rs:545，gap#6 major，§12**；combat/mod.rs:368 调用）读点从 `equipped.get(EQUIP_SLOT_FALSE_SKIN)` → 扫 CHEST worn。蜕层（shed）从 CHEST worn 移除一件伪皮，emit `ShedEvent` 不变（tuike.rs:131 FalseSkin 组件 / :152 state_payload 不变）。
- **v1 FalseSkin 单组件取件写死（gap#6 major）**：v1 `FalseSkin` 是单组件（单 instance / 单 kind），无法表达 CHEST worn 多件；v1 是 contam-filter 真实消费链（resolve.rs:1320 tuike_filter_contam）。**写死：v1 FalseSkin 取 CHEST worn 栈顶（`worn.last()`）伪皮件建组件**（单层语义不变，与 v2 多层分工）；**多层吸收语义全归 v2 StackedFalseSkins**。补测试：CHEST worn 叠 2 件伪皮 → `v1 FalseSkin.instance_id == worn.last().instance_id`（只认栈顶，下层不进 v1 单组件）。
- `combat/resolve.rs`:56 import + FalseSkin/StackedFalseSkins 双 query + TWO_HAND wire 映射（:744 区域）+ v1 depleted 链 + `equipped.get(EQUIP_SLOT_FALSE_SKIN)` 读点全 retarget（按 grep 实测 resolve.rs FalseSkin/TwoHand 站点逐一）。
- `inventory_snapshot_emit.rs` false_skin field（snapshot 产出端）随 schema 删字段同步。

### 移除 FALSE_SKIN 专槽
- 删 `EQUIP_SLOT_FALSE_SKIN` 常量（已在 P0.1 删）、`EquipSlotV1::FalseSkin`、客户端 `FALSE_SKIN`（EquipSlotType.java，P4）。
- `equip_false_skin` dead C2S 清理：proto `EquipFalseSkin`（envelope.proto:258）+ 对应 handler（确认无其他消费后删；若仍被引用则降级为 no-op 并标 deprecated）。

### combat/weapon.rs 武器同步 retarget（major — P2 漏点名，two_hand 分支取数 + 独立 EquipSlot 枚举）
- `current_weapon_from_inventory`（weapon.rs:169）：现 `:175` `.get("two_hand")→EquipSlot::TwoHand` 读双手武器。改为 main/off 读 `.held`、**删 two_hand 分支**（双手兵器从 `main_hand.held` 派生 + `weapon_two_handed` 标对侧 lock，决议 #7）。
- **删独立枚举 `combat::weapon::EquipSlot::TwoHand` 变体**（weapon.rs:236-239，独立于 schema `EquipSlotV1`，含 `MainHand|OffHand|TwoHand`）；只留 MainHand|OffHand（extra_hand 走 held 同分支按需补）。
- `weapon_equipped_emit.rs` 同源迭代此枚举 → 删 two_hand 迭代项（与 §P0.3 proto `WeaponEquipped` 注释删 two_hand 对齐）。

### spirit_treasure scan SlotContents 迭代 + treasure worn vs 触发位正交（minor — #16/#19 收口）
- `scan_inventory_for_spirit_treasures`（spirit_treasure.rs:231，:238 `for item in equipped.values()`）：改读触发位（#8）后，**装备槽里的 treasure worn 件仍是 worn 装备件（#16，与激活态正交）**——若保留扫装备槽 treasure 作 worn 件，迭代改 `flat_map(\|s\| s.worn.iter().chain(s.held.iter()))`（桶④）。
- `push_entry_for_item`（:358，:372 `passive_active=equipped`）改 `passive_active = in_trigger_slot`（**仅触发位件 passive_active=true，装备槽 worn treasure equipped=true/passive_active=false，二者正交**）。
- 测试：装备槽 treasure → equipped=true / passive_active=false；触发位 treasure → passive_active=true。

### P2 测试
- 伪皮装进 CHEST worn → `FalseSkin`/`StackedFalseSkins` 组件生成 + tuike 层数正确。
- **v1 FalseSkin 取栈顶（gap#6）**：CHEST worn 叠 2 件伪皮 → `FalseSkin.instance_id == worn.last().instance_id`（下层不进 v1 单组件）。
- 多件伪皮分层（v2 StackedFalseSkins 多层）：**断言 `StackedFalseSkins.layers` 层数 + 各层 spirit_quality / damage_capacity 字段值**（gap#7：v2 damage_capacity 是 dead_code，不断言战斗吸收数值，接 resolve 归 tuike_v2 plan）。
- 蜕层（shed）移除一件 CHEST worn 伪皮 + emit ShedEvent。
- 三招（don/shed/transfer_taint）走 CHEST worn 读取入口。
- v1 sync_false_skin_from_inventory CHEST worn 读取一致。
- false_skin_state_emit 组件来源不受 retarget 影响。

---

## P3 — 效果叠加语义 + 负重 + equipped 真元 carrier 守恒

### 8 条防御/负重公式（写死到实现精度，来自聚焦验证 defense_math_to_lock，gap#12/#13 收口）
> 实施时逐条对照；P3 测试断言走 armor_sync.rs / body_mass.rs / calculate_current_weight 的可观察输出（不绑内部调用次数）。

1. **护甲减伤聚合公式（armor_sync.rs:36/:42）**：
   `defense_profile[(body,kind)] = ( Σ_{worn 件 i}( ap.kind_mitigation[kind] × ap.effective_multiplier_for_durability_ratio(item.durability) ) + limb_defense_technique ).clamp(0.0, ARMOR_MITIGATION_CAP)`。
   `.max()` → `+=` **加性累加（不取最高）**；跨槽按身体部位聚合——**同一身体部位多件加性、不同身体部位独立累加**。
2. **逐件 clamp 决策写死**：armor_sync 累加时**各件 NOT 单独 clamp**，只在写入 entry 前对累加值 `.min(ARMOR_MITIGATION_CAP = 0.85)`；矩阵存**已 clamped 值**（非裸 sum 1.0）。
3. **系统顺序写死**：`armor_sync`(CombatSystemSet::Intent) **先于** `body_conditioning`(Physics) 写同一 `defense_profile` entry；`body_conditioning.rs:174` 的 `+= limb_def` 后 `.min(ARMOR_MITIGATION_CAP)` 保持；`resolve.rs:92` 的 `.clamp(0.0, ARMOR_MITIGATION_CAP)` 声明为**最终唯一兜底**（三层兜底非 bug，但矩阵存 clamped 值、boundary 测试见下）。
4. **盾牌 block_ratio 正交链**：`shield_block` resolve 单独 `.clamp(0.0, 0.95)`，**不进 defense_profile**，与 armor mitigation **不 double-count**（盾与护甲两条独立链）。
5. **durability 缩放**：`effective_multiplier_for_durability_ratio` 破甲（durability ≤ 0）→ `broken_multiplier(0.3)`，否则 `1.0`；每件**先各自 × effective_mul 再纳入加性 sum**。
6. **equipped_armor_mass（body_mass.rs:148-159）**：遍历 4 护甲槽 worn 时 **filter『有 ArmorProfile 的件』**，排除背包/伪皮自重（否则误算进击退 `total_mass`）。
7. **calculate_current_weight（mod.rs:3462）** = `Σ_所有槽( Σ_worn weight×stack_count + held weight×stack_count )`，**含 held 武器、含背包件自重**；背包 `weight_capacity` **仅进 compute_max_weight 不进 current_weight**。
8. **v1 FalseSkin contam 吸收**取 CHEST `worn.last()` 单件（多层语义归 v2 StackedFalseSkins）；v2 `damage_capacity` 仍 dead_code，本 plan 不接入 resolve（与 §P2 一致，gap#6/#7）。

### 效果叠加（混装各自生效，加性 —— 上方公式 1/2/3/4/5 落地）
- **真正聚合落点（major — 不是 components.rs，那只是 DerivedAttrs struct 定义）**：`combat/armor_sync.rs` `sync_armor_to_derived_attrs`（mod.rs 注册到 `CombatSystemSet::Intent`）现 `:26` 每槽 `inv.equipped.get(slot)` 取单件、`:42` 按 (body,kind) `.and_modify(\|e\| *e = e.max(m))` **取最高**。改为：遍历槽内 `worn` 全件、**同槽多件 = 加性 sum 后 clamp（`.max()`→ `+=` 累加），不取最高**（公式 1）。各件不单独 clamp、累加后写 entry 前 `.min(CAP)`（公式 2）。
- **跨槽语义写死**：跨槽（head+chest+legs+feet 各自 worn 件）按 (body,kind) **仍按身体部位聚合**——同一身体部位多件加性、不同身体部位独立累加；不再用 `.max()` 在槽间取最高（旧 `.max()` 是同 (body,kind) 取最高，加性化后改累加）。
- **与 body_conditioning 合成顺序（公式 3，gap#12 收口）**：`body_conditioning.rs:174`（功法光波体操给四肢 += limb_defense，与装备护甲**共写** defense_profile）非 equipped 联动但同写矩阵——系统顺序 Physics 在 Intent armor_sync 之后，对同 entry `+= limb_def` 后 `.min(CAP)`；`resolve.rs:92` 为最终兜底。
- `body_mass.rs:148-159` 护甲负重 `filter_map(get)` 改 `worn.iter()` + **filter ArmorProfile（公式 6，gap#13）**（与 calculate_current_weight 一致但 mass 须排背包/伪皮）。
- `combat/components.rs` `DerivedAttrs` struct 字段定义不变（加性目标由 armor_sync 写入端实现）。held 武器属性按原语义。
- 桶② 其余触点逐一改 `.worn.iter()` 累加 + `.held`（含 jiemai.rs:72-83 改 `worn.iter().map(\|i\| i.weight).fold(0.0, f64::max)`，gap#4）。

### 负重（决议 #24 —— 上方公式 7 落地）
- `calculate_current_weight`（mod.rs:3462，现 `equipped.values()...×stack_count`）改遍历每槽 `worn` 全件 + `held`（公式 7）；确认穿戴 + 手持全部计入（与旧行为对齐：旧 equipped.values() 含武器槽，手持武器仍计重）；**背包件自重计入 current_weight、背包 weight_capacity 仅进 compute_max_weight**。

### equipped 真元 carrier 守恒（qi_physics 锚点）
- `qi_physics::ledger::inventory_qi`（ledger.rs:564，:571 `inventory.equipped.values().map(item_qi).sum()`）→ `inventory.equipped.values().flat_map(\|s\| s.worn.iter().chain(s.held.iter())).map(item_qi).sum()`。
- **carrier imprint 纳入 held 求和（blocker — 漏改账实不符）**：`combat/carrier.rs` 写真元 imprint 走 ledger，手槽 carrier 件已改读 `.held`（桶①）；carrier imprint 来源（手持 carrier 件的真元）必须进 `inventory_qi` 的 held 求和（上一条 `chain(s.held.iter())` 已覆盖手槽 held），保证「手持 carrier 充能后 inventory_qi = held carrier item_qi」。
- 桶④ 同形真元广播 / 衰减点逐一 chain worn+held：`shelflife/sweep.rs`（values_mut 衰减）、`world/block_drop.rs` / `world/block_place.rs`（真元广播 chain(equipped.values())）。
- tuike_v2 `false_skin_maintenance_tick` 多层维持成本走既有 `release_qi_amount_to_zone` / `QiTransfer`（守恒，不新增公式）；多层 = 成本累加，物理不变（头部锚点已声明）。

### P3 测试
- 两件同类 armor 装进同槽 worn → `armor_sync` 防御加性翻倍（**断言走 armor_sync.rs 输出，不取最高**，公式 1）。
- **clamp boundary（gap#12，公式 2）**：两件 0.5 cut 甲同槽 worn（裸 sum 1.0）→ 矩阵值 = `0.85`（被 ARMOR_MITIGATION_CAP cap）**非 1.0**；矩阵存 clamped 值。
- **durability 缩放（公式 5）**：破甲（durability≤0）件 mitigation × broken_multiplier(0.3) 再纳入 sum；满耐 × 1.0。
- **盾正交（公式 4）**：盾 block_ratio 单独 clamp 0.95、不进 defense_profile，护甲 mitigation 与盾不 double-count。
- 混装（armor + 手套）各自生效。
- 跨槽（head+chest 各一件）防御按身体部位独立累加、不互相 max。
- **armor_mass filter（gap#13，公式 6）**：chest worn=[铁甲 12kg, 背包 2kg] → `equipped_armor_mass` = 12（仅有 ArmorProfile 的甲，排背包）；`current_weight` 含 14（甲+背包自重）；`max_weight` 含背包 weight_capacity 加成（公式 7）。
- 负重含全部 worn 层 + held 武器 + 背包件自重（公式 7）；背包 weight_capacity 不进 current_weight、仅进 max_weight。
- **守恒 pin**：同槽两件 carrier（各带真元）的 `inventory_qi` = 两件 item_qi 之和（决议头部 qi_physics 锚点）。
- **carrier 守恒 pin**：手持 carrier 充能后 `inventory_qi` = held carrier item_qi（carrier imprint 来源纳入 held 求和，blocker）。
- shelflife sweep 衰减遍历到全部 worn+held（无遗漏件不衰减）。
- **niche_defense 材料计数（gap#9）**：装备槽 worn 含 N 件某 template_id 材料 → `niche_defense.rs:480` count 计入 N（flat_map worn.chain(held) 后不漏件）；:518 find 命中。

---

## P4 — 客户端面板重构 + 分层渲染 + 法宝激活触发位

### 数据模型（client）
- `client/.../inventory/model/InventoryModel.java`：`equipped` 字段 `Map<EquipSlotType, InventoryItem>` → `Map<EquipSlotType, SlotContents>`（SlotContents = `{List<InventoryItem> worn, InventoryItem held}`，镜像 server）；`Builder.equip(slot, item)` API、`equipped()` getter、`parseEquipped` 返回类型随改。
- `client/.../inventory/model/EquipSlotType.java`（现 15 变体，约 :4-18）：删 `FALSE_SKIN`/`TWO_HAND`/`TREASURE_BELT_0..3`；**决议 #17：删 `BACK_PACK`/`WAIST_POUCH`/`CHEST_SATCHEL`（约 :16-18）**（背包专属槽取消，背包随身体槽 head/chest/legs/feet worn 层渲染，不再有独立背包 EquipSlotType）；**新增 `EXTRA_HAND_0`/`EXTRA_HAND_1`（major #11 — 当前 EquipSlotType.java 无该变体；server schema EquipSlotV1 + proto 已有 extra_hand_0/1，client 须新增）**。净结果：删 9、增 2。
- `client/.../network/InventorySnapshotHandler.java`：**`EQUIP_SLOT_BY_WIRE_NAME`（`Map.ofEntries`，约 :24-35）当前仅 12 条（head…treasure_belt_3），缺 extra_hand_0/extra_hand_1（blocker）**：删 6 个废 wire name（false_skin/two_hand/treasure_belt_0..3）、**决议 #17：仅新增 extra_hand_0/extra_hand_1 两条映射（不补背包三槽——背包专属槽取消，背包件随 head/chest/legs/feet 的 `<slot>_worn` 数组下发，不需独立 wire 名）**；`parseEquipped`（约 :220）改读 `<slot>_worn`(list) + `<slot>_held`(optional)（背包件即身体槽 worn 数组里的普通元素，无特判）。决议 #13.2「补背包槽 wire」随 #17 作废，wire 触点净减。
- `InventoryEquipRules.canEquip`（约 :85）：**签名 `Map<EquipSlotType,InventoryItem>`（单件）→ `Map<EquipSlotType,SlotContents>`（major #10 — 单件 map 表达不了栈状态）**；镜像 P1 worn cap + held 互斥 + 双手锁对侧手 + **worn 栈 LIFO 仅栈顶可拖（决议 #12）**；**删 `FALSE_SKIN`/`TWO_HAND`/`TREASURE_BELT` case + `BACK_PACK`/`WAIST_POUCH`/`CHEST_SATCHEL` case（约 :119 `case BACK_PACK,WAIST_POUCH,CHEST_SATCHEL -> false`）——决议 #17 取消背包专属槽，switch 穷尽性随之处理**；**背包件改走身体槽（head/chest/legs/feet）通用 worn 校验**：拖背包件到身体槽时 `isContainer(item)` 也算合法 worn 件、与盔甲/伪皮同槽 cap + LIFO 共算（不再有「背包槽恒 false」死分支、不再单算 cap=1）。

### 删除「行囊」tab 全部客户端构件（决议 #13，取代 #714 a5eef15b）
> 行囊面板构件全集中在 `client/.../inventory/InspectScreen.java`（无独立 `TAB_BACKPACK` 常量；行囊是右列容器 tab，靠 `activeContainer == containerCount` 哨兵激活）。逐项删/改：
- 删字段（约 :102-108 区域）：`backpackTabLabel`、`backpackEquipWrapper`、`backpackSlotBack`、`backpackSlotWaist`、`backpackSlotChest`、`backpackWeightLabel`（**决议 #19：行囊重量条删除，不并入任何面板**）、`bodyPocketGrid`。
- 删方法：`buildBackpackEquipPanel`（约 :1550）、`switchToBackpackTab`（约 :1502）、`buildLabeledEquipSlot`（约 :1590，行囊专属 labeled equip slot 构造）、`refreshBackpackEquipPanel`（约 :1606）、`refreshBackpackEquipPanelIfActive`（约 :1634）、**`backpackWeightBreakdown`（约 :1004，注释 for 行囊 tab — major #12；决议 #19：直接删除，不并入容器 tab——负重显示沿用整体 inventory 底部既有 `BottomInfoBar`（BottomInfoBar.java:12，挂 InspectScreen.java:648-649 mainPanel 底部，读 model.currentWeight()/maxWeight()），无需重复渲染）**。
- 删 tab 构建：行囊 tab `backpackTab`（约 :574-592，排第一的 FlowLayout + label `§f行囊`）；build 挂载 `backpackEquipWrapper`（约 :635-638）；默认激活行囊页 `switchToBackpackTab()`（约 :690）。
- 改 `activeGrid()`（约 :962 行囊分支返回 bodyPocketGrid）：行囊删后 body_pocket 改走普通容器 grid 路径（`containerGrids[body_pocket 索引]`，见下「body_pocket 默认容器 tab」），删 bodyPocketGrid 分支。
- 改 `switchToGridContainer("body_pocket")→switchToBackpackTab()` 路由（约 :1537-1539）：改为普通 `switchContainer` 到 body_pocket tab。
- 改 `placeItemAnywhere` 行囊兜底（约 :3008-3009 无背包容器回落 bodyPocketGrid）→ 回落 body_pocket 容器 grid。
- **系统化拆除 `activeContainer == containerCount` 哨兵机制（major #13 — #714 最易回归点，索引错位曾致零交互）**：哨兵语义贯穿多处，逐站点拆除，让 `activeContainer` 落 `0..containerCount-1`：
  - 删哨兵语义注释（约 :89-101，`activeContainer == containerCount means 行囊 is active`）。
  - `switchContainer` 守卫（约 :1480 `idx >= containerCount`）随哨兵删除调整。
  - `switchToBackpackTab` 置位点（约 :1521 `activeContainer = containerCount`）随方法删除移除。
  - `refreshBackpackEquipPanelIfActive`（约 :1635）+ 其在 `populateFromModel` 的调用一并删。
  - `placeItemAnywhere`（约 :2989 `i == activeContainer continue`）随哨兵体系调整。
  - 补「tab 标签不错位」回归断言（防 #714 索引错位回归）。
- **决议 #17：不再「把三个背包 equip 槽从行囊面板迁入主装备面板」**——背包专属槽取消，背包件按 `ContainerSpec.equip_slot` 指定身体槽（head/chest/legs/feet）作该身体槽 worn 层渲染（见下「装备面板布局 + worn 栈叠放渲染」，与盔甲/伪皮同槽叠层）。原行囊面板内的重量条 label（`backpackWeightLabel` + `backpackWeightBreakdown`）**删除，不单独渲染、不并入任何面板**——负重沿用整体 inventory 底部既有 `BottomInfoBar`（决议 #19）。

### body_pocket 改默认常驻容器 tab（决议 #13，调和 #714「不单独占 tab」张力）
- `InventoryModel.BODY_POCKET_CONTAINER_ID`（model/InventoryModel.java:20）是容器 id（**非 EquipSlotType**）。#714 把它从容器 tab 列表过滤掉（InspectScreen build 约 :556-564 `filteredContainerDefs` 过滤 body_pocket），只在行囊面板内 grid 渲染。行囊删后**改回默认常驻容器 tab**：
  - InspectScreen build（:556-564）：`containerDefs` **始终含 body_pocket 且排在容器 tab 列表第一个（最左，决议 #18）**（移除 :558-562 的 body_pocket 过滤；body_pocket 作 `containerDefs` index 0，其余容器——身体槽 worn 背包件生成的容器——排其后），作为右侧容器 tab 区的基础随身格 tab（始终存在），不再依附行囊面板。
  - 默认激活页：原 `:690 switchToBackpackTab()` → **改默认激活 body_pocket 容器 tab（决议 #18：body_pocket 恒为第一个/默认激活页，不再"或第一个容器"二义）**。`activeContainer` 默认值落 body_pocket 的索引（0）、哨兵（:92 现 `-1`）相应调整。
  - **与 #714 张力的调和**：#714「body_pocket 不单独占 tab」是基于「它已在行囊面板内渲染、独立 tab 是重复」；本 plan 删行囊面板后该前提消失，body_pocket 失去依附点，故反转为默认容器 tab 第一个（决议 #18，不再重复）。此调和已记入 §11.1 #13/#18。

### 装备面板布局 + worn 栈叠放渲染（视听精度内联，避免 owo fill 顶飞）
- `client/.../inventory/component/EquipmentPanel.java`（现绝对定位摆 11 槽 HEAD/OFF_HAND/MAIN_HAND/CHEST/TWO_HAND/FEET/FALSE_SKIN/LEGS/TREASURE_BELT_0..3，约 :29-45；`PANEL_HEIGHT=168` 约 :17）：新布局——**中列一线** HEAD/CHEST/LEGS/FEET 纵向排列；OFF_HAND（左侧）/ MAIN_HAND（右侧）两侧对称；extra_hand_0/1 在主副手下方（多臂可见时）。删 TWO_HAND（约 :35）/FALSE_SKIN（约 :38）/TREASURE_BELT_0..3（约 :42-45）槽位（删后中列 Row3/4/6 空，须重排）。**决议 #17：无背包专属槽**——背包件作为 head/chest/legs/feet 身体槽的 worn 层渲染（与盔甲/伪皮同槽叠层），EquipmentPanel **不为背包另起槽位**。
- **EquipmentPanel addSlot 清单（major #9 — 漏改则 drop 管线不认多臂槽）**：构造器 `addSlot` 当前仅 HEAD/OFF_HAND/MAIN_HAND/CHEST/TWO_HAND/FEET/FALSE_SKIN/LEGS/TREASURE_BELT_0..3，**决议 #17：仅须新增 `addSlot(EXTRA_HAND_0)`/`addSlot(EXTRA_HAND_1)`（不加背包三槽）**——否则 `slotAtScreen`(约 :69)/`slotFor`(约 :57) 只解析 `slotComponents`(addSlot 填充)，extra_hand 槽 drop 落不进、`populateFromModel` 也填不到。背包件随其身体槽（head/chest/legs/feet）的 `SlotContents.worn` 渲染，不需独立 addSlot。
- **EquipSlotComponent 单件 API → worn 栈（major #9）**：`EquipSlotComponent` 现 `item()`/`setItem()` 单件 → 改 `List worn` + `held`；`populateFromModel` 用 `model.equipped().get(key)` 取单件 → 改读 `SlotContents`（worn 列表 + held）；`slotAtScreen` 命中后区分**栈顶/下层**（层 hit-test，仅顶可拖）。**决议 #17：背包件即身体槽 worn 栈里的普通一层**（无专属退化槽、无特例渲染分支）——背包件在身体槽 worn 栈里与盔甲/伪皮一样叠放、有角标/dim/hit-test，统一处理。
- **背包随身体槽 worn 层渲染**（决议 #17 取代旧「背/腰/前 worn 槽迁入」）：**不再把 BACK_PACK/WAIST_POUCH/CHEST_SATCHEL 加入主装备面板**——背包件按其 `ContainerSpec.equip_slot` 指定的身体槽（head/chest/legs/feet）出现在该槽 worn 栈里（穿上后右侧容器 tab 照旧提供其容器网格）。布局上**无新增背包专属槽位**，省去原「中列下方再起背包三槽」的重排。
- **布局坐标 + PANEL_HEIGHT 核（minor #21）**：删 4 类废槽（TWO_HAND/FALSE_SKIN/TREASURE_BELT_0..3，4 槽）+ **新增 2 槽（extra_hand_0/1，决议 #17：不再 +背包三槽，净增 2 槽）**后，须**明确每槽新 (px,py) 绝对定位坐标 + 核 `PANEL_HEIGHT=168` 是否容纳新行（删 4 增 2 净减 2 槽，更易容纳；不够则调高）**，保留绝对定位避免 owo fill 顶飞；新坐标在 §10.2 三轮打磨中核对（首轮 first cut 列坐标表）。
- **owo 布局约束**：中列用 `Containers.verticalFlow` + 各槽 `Sizing.fixed(18)`（标准 slot 18px），**严禁 `Sizing.fill(100)`**（会占满父容器整宽把左右手两侧节点顶出边界消失——参 memory `feedback_owo_fill_overflow.md`）。左右手两侧 + extra_hand 用绝对定位 / `Positioning.relative` 锚到中列两边。
- `EquipSlotComponent` worn 栈多层渲染（**栈语义 LIFO，决议 #12**）：
  - **worn 多层叠放（栈序：尾=顶）**：栈底 `worn[0]` 在最下、栈顶 `worn[n]`(=`worn.last()`) 在最上，每层向右下偏移 **+2px / +2px**（z-order 递增，栈顶 z 最高），最多渲染 cap 层（chest 3 / head 2…）。**下层从底部露出一点示意被压住**（被上层遮住大部、仅底边露 ~2px）。
  - **栈顶高亮 + 可拖**：栈顶（`worn.last()`）正常亮度 + 描边高亮 `#FFE8B0`（淡金边 1px），**仅栈顶可拖下/卸下**；脱顶后下一层（新 `worn.last()`）渲染为新顶（高亮转移、可拖）。
  - **下层 dim / 不可交互态**：被压住的下层（非栈顶）叠 tint `#000000` opacity **0.40**（dim 示意压住），**不可拖**（drag 命中下层 = 无响应或飘红边提示「请先脱上层」，不静默移动）。
  - **层数角标**：右上角，位置相对 slot `(x+14, y-2)`，颜色 `#FFD27F`（淡金），字号 **7px**（vanilla small font scale 0.7），文案 `×N`（N≥2 才显示）。
  - **held 单独位**：held 物品渲染在 slot 右下角小图标位 `(x+9, y+9)`，叠在 worn 之上，**8px** 缩略图 + 武器图标边框区分。
  - **hover**：悬停栈顶显示 tooltip（该件名称 + worn 标记 + `栈顶·可卸下`）；悬停下层显示 tooltip（件名 + `被压住·先脱上层`）；held 单独 tooltip。
  - **双手另一手 disable**：被锁的对侧手槽叠 tint `#000000` opacity **0.55** + 灰色锁图标（中心 `(x+5,y+5)` 8px）；hover 显示「双手兵器占用」文案，不可拖入。
- drop 收口（`InspectScreen` `attemptDrop` TAB_EQUIP 分支）：**删除当前 swap 分支（major #8 — InspectScreen.java 约 :2442-2445 槽已占时执行 `InventoryItem old = eq.item(); eq.setItem(dragged); placeItemAnywhere(old)` swap，与决议 #3「拒绝不顶替」冲突，且 `eq.item()/setItem` 单件 API 与 worn 栈不兼容）**；改按 classify(worn/held) 校验 cap/占用：worn 合法则 push 栈顶、满则飘红退回不 swap，held 占则飘红退回不 swap，**不静默吞物**。拖**下层**（非栈顶）= 拒绝飘红边 + 文案「请先脱上层」（决议 #12，镜像 server）。

### 法宝激活触发位（灵宝 UI 内，**不在装备面板**）
> 决议 #8：法宝激活态承载从 `treasure_belt` 装备槽 → 灵宝 UI 内新建「触发位」。
- **server**：
  - 新建「触发位」承载（`ActivatedTreasures` store 或 PlayerInventory 内 `triggered_treasures: Vec<ItemInstance>`，容量默认 4 = 旧 belt 槽数，可后续配置——**P4 子细节注明，P5 可挂升级**）。
  - `spirit_treasure.rs`：`scan_inventory_for_spirit_treasures`（:231，现 :238 遍历 `equipped.values()`）改读触发位；`push_entry_for_item`（:358，:372 `passive_active=equipped`）改 `passive_active = in_trigger_slot`。**法宝离开装备 equipped 后由触发位维持激活态**（否则被动全失效——blocker）。
  - persistence：触发位随 PlayerInventory 入库（schema_version 已 bump；迁移把旧 treasure_belt → 触发位，P0.6）。
  - 新建「法宝激活」C2S 入口（**当前不存在**——SpiritTreasureScreen 只切 tab 无 equip）：右键灵宝 item 的 context 动作 → 新 C2S IntentHandler（**Marker/IntentHandler 模式，非 vanilla hack**，参 memory `feedback_no_vanilla_hacks.md`）→ 移入触发位。proto 新增 C2S message + handler。
  - `treasure_equipped_emit.rs`:57（现硬编码 `slots=[off_hand, belt_0..3]`）改从触发位产 payload。
- **client**：
  - `SpiritTreasureScreen.java`：在灵宝 UI 内渲染触发位（4 格），支持右键 item → 激活（飞入触发位）/ 卸下。
  - `TreasurePanelSync.java`（`TREASURE_SLOTS` 硬编码）+ `WeaponHotbarHudPlanner.java`（`get('off_hand')`）从触发位新来源拉数，不再依赖 belt 槽 key。
  - **视听**：激活 toast 文案「{灵宝名} 已激活」（屏幕中上，淡入 10tick / 停留 40tick / 淡出 10tick）；飞入动画——item 图标从右键位置贝塞尔曲线飞入触发位（**12tick**，ease-out），落位时触发位边框闪 `#7FD2FF`（淡青）1 次（6tick fade）。

### P4 测试
- `EquipmentPanelTest`：中列一线 + 左右手布局；**决议 #17：背包件作身体槽（head/chest/legs/feet）worn 层渲染，不在独立背包槽**（验证背包件出现在对应身体槽 worn 栈、与盔甲同槽叠放）；worn 栈多层叠放渲染（叠 2/3 层，栈底 worn[0] 下、栈顶 worn.last() 上）；层数角标 N≥2 显示；held 单独位；双手另一手 disable tint；**EquipmentPanel addSlot 含 extra_hand_0/1（不含背包三槽，slotAtScreen/slotFor 能命中 extra_hand 新槽）**。
- **`BackpackEquipSlotTest` 删除/迁移（major #12 + 决议 #17/#19）**：该测试整体针对「背包专属槽」语义——决议 #17 取消背包专属槽后，删除依赖已删 `BACK_PACK`/`WAIST_POUCH`/`CHEST_SATCHEL` 枚举 + `InspectScreen.backpackWeightBreakdown()`（约 :215/230/243/251/262）+ `model.equipped().get(BACK_PACK)` 单件断言（约 :280-294）的全部 case；**改写为「背包件骑身体槽 worn 层」断言**并入 `EquipmentPanelTest`（背包件 push 进 chest/legs/etc worn 栈、与盔甲同槽 cap/LIFO 共算、生成容器 tab）。重量条相关断言（依赖 `backpackWeightBreakdown`）**删除，改测 `BottomInfoBar` 整体重量展示（决议 #19）**。
- **worn 栈 LIFO 渲染/交互（决议 #12）**：栈顶高亮 + 可拖；下层 dim(opacity 0.40) + 不可拖（drag 命中下层飘红边「请先脱上层」）；脱顶后下一层渲染为新顶（高亮转移、变可拖）。
- **行囊删除（决议 #13）**：InspectScreen 无 `backpackEquipWrapper`/`buildBackpackEquipPanel`/`switchToBackpackTab`/`backpackSlot*`/`backpackWeightLabel`/`backpackWeightBreakdown`/`bodyPocketGrid`/`refreshBackpackEquipPanelIfActive` 残留；无 `§f行囊` tab；**无 `activeContainer == containerCount` 哨兵残留**。
- **containerCount 哨兵拆除 + tab 不错位回归（决议 #13，major #13）**：`activeContainer` 落 `0..containerCount-1`；tab 标签与激活索引不错位（防 #714 索引错位回归）。
- **body_pocket 默认容器 tab（决议 #13）**：`containerDefs` 始终含 body_pocket；右侧容器 tab 区出现 body_pocket tab（非行囊内）；新玩家默认激活页落 body_pocket；body_pocket pickup/drop 走普通容器 grid 链路（不依赖已删的 bodyPocketGrid）。
- **EQUIP_SLOT_BY_WIRE_NAME / parseEquipped（blocker #3 + 决议 #17）**：snapshot 含 `extra_hand_0_worn`/`extra_hand_0_held` → `model.equipped().get(EXTRA_HAND_0)` 有值（非 null）；**背包件随身体槽下发——snapshot `chest_worn` 含一个 container 件 → `model.equipped().get(CHEST).worn` 含该背包件**（不再有 `back_pack_worn` wire）；多臂/身体槽 worn 端到端可渲染。
- **`InventoryEquipRules.canEquip`（major #10 + 决议 #17）**：背包件（container）拖入身体槽 head/chest/legs/feet `worn 未满` → true；身体槽 worn 满 → false；held 占/锁手/拖下层 → false。**无背包专属槽 case**（删 BACK_PACK/WAIST_POUCH/CHEST_SATCHEL 分支）。
- **C2S `EquipLoc.state`（blocker #1）**：`ClientRequestProtocolTest` equip-worn/equip-held 编码 pin（toJson 含 state）。
- wire 解析多件（`<slot>_worn` list + `<slot>_held`）。
- drop 落位：worn 满拒退回、held 占拒、锁手拒、**拖下层拒（栈顶才可动）**、**已占槽不 swap（删 swap 分支后）**。
- 法宝激活：右键 item → C2S → 触发位 → scan passive_active=true；卸下 → 失活；战斗 HUD 法宝面板从触发位拉数（不依赖 belt 槽）。

### P4 多轮打磨（§6.1）
面板布局 + 分层渲染 + 法宝飞入动画属视觉资产，**强制 3 轮自我打磨**（round 1/3 first cut → round 2/3 截图 review → round 3/3 终轮）+ 终轮 commit `<PROMISE>` 担保块（拼写 PROMISE）。

---

## P5 — 容量升级 hook（后续）

- `worn_cap` 由固定常量 → 可受升级提升的派生值：`fn worn_cap(slot, &PlayerCultivation, &Techniques) -> u8`，升级来源（境界 / 功法 / 法宝）对应 worldview 节（决议 #24 注：升级来源需 worldview 锚点，P5 收口前补）。
- 触发位容量（默认 4）同此 hook 可升级。
- 占位，后续单独细化；P5 不进首轮 consume 关键路径（可独立 PR-5）。

---

## §11 开放问题（P0 决策门前需收口）

> 原表保留作历史回溯。**全部已在 §11.1 收口。实施时以 §11.1 决议为准。**

1. worn/held wire 表示（方案A 每件带标记 vs 方案B 每槽分两字段）
2. C2S move 落位语义（Equip{slot} 加 state / layer_index）
3. validate_move_semantics 多件分支（拒绝 vs swap）
4. 存档迁移（schema_version 门控 + migrate 函数 + 旧槽映射去向 + LoadoutSpec 同步）
5. 背包/多臂槽 vs 分层装备槽数据模型策略
6. 手持槽全集（ExtraHand0/1 是否 held、双手锁哪些手、worn_cap 表补 ExtraHand）
7. spear/staff 双手标志派生落点
8. 法宝激活态承载（删 belt 槽后 passive_active 靠什么判定 + C2S 入口）
9. 蜕壳流 retarget（v2 是活的，v1/v2 关系）
10. worldview 补写（节号 + 分层装备锚点）
12. worn 层栈语义（LIFO，仅栈顶可操作 / 卸下；下层被压住不可动）
13. 行囊 UI 移除 + 背包穿戴化 + body_pocket 转默认容器 tab（取代 #714 a5eef15b）
16. Treasure/Shield/Container worn/held 分类（补充开放问题，承接 #5/#8，编号与 §11.1 #16 对齐）
17. 背包专属槽 vs 身体槽 worn 层（最终设计决策，取代 #5 退化槽 + #13.2 背包 wire；编号与 §11.1 #17 对齐）
18. body_pocket 在容器 tab 的呈现位（最终设计决策，收口 #13 开放点；编号与 §11.1 #18 对齐）
19. 行囊重量条去向（最终设计决策，收口 #13 开放点；编号与 §11.1 #19 对齐）
20. 封灵容器 attrition_exempt 运行时链路 retarget（聚焦验证 gap#3 blocker 新增，决议 #17 改容器 id 后 container_id_to_equip_slot 恒 None 静默失效；编号与 §11.1 #20 对齐）

## §11.1 决议（pre-P0 收口，2026-06-25）

### #1 worn/held wire 表示 = 方案B
**决议**：每个装备槽 = `worn`(repeated/Vec) + `held`(optional)，**不污染 InventoryItemView**（后者在 containers/hotbar/dropped 复用，方案A 侵入面极大）。proto/Rust/agent-TS 三端同构。
**落点**：proto `EquippedInventorySnapshot`（envelope.proto:556-573，每装备槽拆 `<slot>_worn` repeated + `<slot>_held` optional）/ Rust `EquippedInventorySnapshotV1`（schema/inventory.rs:174）/ agent `EquippedInventorySnapshotV1`（inventory.ts:240）；server `SlotContents{worn:Vec, held:Option}`（mod.rs，PlayerInventory.equipped）。plan §P0.3 / §P0.4。

### #2 C2S move 落位 = Equip{slot, state}
**决议**：`InventoryLocationV1::Equip` 加 `state: EquipStateV1(Worn|Held)`（不用 layer_index——worn 追加到 Vec 尾、held 写 Option，index 由 server 决定）。state 必填，旧 sample 一并改。三端同步 + equip-worn/equip-held 正反 sample + roundtrip pin。
**落点**：schema/inventory.rs:212-213（Equip）+ :372/:380（RawInventoryLocationV1::Equip）+ :353（TryFrom）/ inventory.ts:258-271 / envelope.proto:2586（oneof location equip 变体）/ client EquipSlotType + InventorySnapshotHandler。plan §P0.3 / §P0.5。

### #3 校验冲突 = 拒绝不顶替
**决议**：worn cap 满 = 拒绝（带文案「已穿戴 N 层」）；held 槽占用 = 拒绝（卸下才换，带文案「请先卸下」）；双手武器锁定的手槽被拖入 = 拒绝（带特定文案「双手兵器占用双手」）。**全部拒绝，无 swap**（swap 语义易误吞物，留 P5+）。
**落点**：`validate_move_semantics`（mod.rs:4151-4321）Equip 各分支。plan §P1。

### #4 存档迁移
**决议**：(1) `load_player_inventory_from_sqlite`（state.rs:1090，现 :1095 SELECT 仅 inventory_json、:1115 裸 from_str）改先 SELECT schema_version 分流；(2) 写 `migrate_equipped_v1_to_v2(&mut Value)`：每槽 object → `{worn:[..],held:..}`（按 classify）；(3) `const INVENTORY_SCHEMA_VERSION` bump（现值 +1，save 路径 state.rs:704+ 已写 schema_version 列）；(4) 旧槽映射去向定死——false_skin→CHEST worn / two_hand→main_hand held(对侧 lock load 后重算) / treasure_belt_0..3→触发位激活承载 / **旧背包专属槽 back_pack/waist_pouch/chest_satchel→按 container_spec 重定向身体槽 worn 栈（决议 #17，迁移后无背包槽 key）** / extra_hand 槽→武器落 held（不误塞）；(5) `LoadoutSpec.equipped`（mod.rs:391）+ instantiate_loadout（:987）+ `server/assets/inventory/loadouts/default.toml`（**TOML，背包 slot 改身体槽，决议 #17**）同步迁移。
**落点**：state.rs:1090-1115 / mod.rs:391,987,1199 / default.toml。plan §P0.6。

### #5 背包/多臂槽统一 SlotContents（worn_cap=1 退化）—— ⚠️ 已被决议 #17 取代
**原决议（保留作历史回溯）**：背包槽（back_pack/waist_pouch/chest_satchel）统一用 SlotContents，但 `worn_cap=1` 退化、`held=None`；读取处全改 `.worn[0]`；`compute_max_weight` 特判背包槽读 worn[0] 算 max_weight。
**⚠️ 决议 #17 取代**：取消背包专属槽（back_pack/waist_pouch/chest_satchel 退化槽全部删除）。背包改为身体槽（head/chest/legs/feet）的普通 worn 层，走该身体槽通用 worn cap + LIFO，**无退化槽、无 cap=1、无 `.worn[0]` 特判**。`compute_max_weight` 改为扫所有身体槽 worn 层里带 `container_spec` 的件求和。本条 #5 退化逻辑作废，实施以 #17 为准。
**落点（#17 重定向）**：mod.rs `ContainerSpec.equip_slot`(:227 改指身体槽) / TOML valid_slots(:1736-1741) / compute_max_weight(:3489-3507 扫身体槽 worn) / rebuild_containers_from_equipment(:3519-3584 扫身体槽 worn) / multi-hand 常量保留(extra_hand)。plan §P0.1 / §P0.2 / §P1 / §11.1 #17。

### #6 手持槽全集
**决议**：`main_hand / off_hand / extra_hand_0 / extra_hand_1` 都是 held 槽（每槽 ≤1，不计 worn cap）。双手武器锁定语义 = **仅锁对侧主/副手**（main_hand 双手锁 off_hand，反之亦然）；**extra_hand 独立不锁**（多臂变异本就额外手）。worn_cap 设计表（**#14 订正：手槽 held-only，worn_cap=0；⚠️ 决议 #17 订正：删「背包 = 1」专属 cap**）：head/feet = 2；chest/legs = 3；**main_hand/off_hand/extra_hand_0/extra_hand_1 = 0（held-only，不计 worn cap）**；**背包无专属槽/无独立 cap（决议 #17：背包占其 `ContainerSpec.equip_slot` 指定身体槽 head/chest/legs/feet 的一个 worn 层，受该身体槽 cap）**。worn 栈 LIFO 只作用于 head/chest/legs/feet worn 槽（手槽 worn 恒空；背包件即身体槽 worn 栈普通一层）。
**落点**：mod.rs:4307-4319（ExtraHand0/1 现走 MainHand 同分支）/ worn_cap 表（§P0.1）。plan §P1。

### #7 spear/staff 双手派生
**决议**：从 `WeaponKind` 派生（加 `fn weapon_two_handed(kind) -> bool` matches `Spear|Staff`，先例 mod.rs:4231 现 TWO_HAND 校验已 match Spear|Staff）。归属 `inventory/mod.rs`（或 `WeaponSpec.is_two_handed` 字段，mod.rs:245 现无）。迁移现 from_two_hand swap 路径（mod.rs:4163-4242）到「一手 held + 对侧 lock」模型。
**落点**：mod.rs:245（WeaponSpec）/ :4231（先例）/ :4163-4242（from_two_hand 迁移）。plan §P1。

### #8 法宝激活 = 灵宝 UI 内「触发位」
**决议**：(1) 新建「触发位」承载（`triggered_treasures: Vec<ItemInstance>` 或 `ActivatedTreasures` store，容量默认 4 = 旧 belt 数，可后续配置——P4 子细节，P5 可升级）；(2) `scan_inventory_for_spirit_treasures`（spirit_treasure.rs:231，:238 遍历 equipped.values()）改读触发位、`push_entry_for_item`（:358，:372 passive_active=equipped）改 `passive_active=in_trigger_slot`；(3) `treasure_equipped_emit.rs:57`（slots=[off_hand,belt_0..3]）/ client `TreasurePanelSync.TREASURE_SLOTS` / `WeaponHotbarHudPlanner.get('off_hand')` 从触发位产 payload；(4) **新建「法宝激活」C2S**（当前不存在，SpiritTreasureScreen 只切 tab）= 右键灵宝 item context 动作 → 移入触发位（IntentHandler 模式，非 vanilla hack）；(5) persistence 随 PlayerInventory 入库。触发位在**灵宝 UI / SpiritTreasureScreen 内，不在装备面板**。
**落点**：spirit_treasure.rs:231-249,358-372 / treasure_equipped_emit.rs:57 / client SpiritTreasureScreen.java + TreasurePanelSync.java + WeaponHotbarHudPlanner.java。plan §P4。

### #9 蜕壳流 retarget（v2 是活的）+ v1/v2 伪皮分工（gap#6/#7 收口）
**决议**：活的是 `tuike_v2/`（tick.rs:47 sync_false_skin_stack_from_inventory、skills.rs:361 equipped_false_skin 三招共用），从 `equipped.get(EQUIP_SLOT_FALSE_SKIN)` 改为扫 CHEST worn 层。v1 `tuike.rs`（combat/mod.rs:368 注册仍活）+ resolve.rs FalseSkin 读点一并 retarget。`false_skin_state_emit` 读 `StackedFalseSkins` 组件（非直接 slot，由 sync 写），retarget sync 来源后**组件来源不受影响**（已确认）。移除 `EQUIP_SLOT_FALSE_SKIN` 常量 + `EquipSlotV1::FalseSkin`。**v1/v2 都保留注册，都 retarget**（不本 plan 内删 v1）。
**v1/v2 分工写死（聚焦验证 gap#6/#7）**：
- **v1 `FalseSkin` 单组件（tuike.rs:545，contam-filter 真实消费链 resolve.rs:1320）取 CHEST `worn.last()` 单件**——单层语义不变，无法表达多件；多件场景只认栈顶，下层不进 v1 单组件。补测试 `FalseSkin.instance_id == worn.last().instance_id`。
- **v2 `StackedFalseSkins.damage_capacity`（state.rs:201/:210）是 `#[allow(dead_code)]`，未接 resolve 战斗结算**——本 plan **只 retarget 读取来源（单槽→CHEST worn 多层），不接通 damage_capacity 到 resolve**（属 tuike_v2 自身 plan，dead_code 待其接入）。P2 测试『多件伪皮分层』只断言 `StackedFalseSkins.layers` 层数 + 字段值，不断言战斗吸收数值。
**落点**：tuike_v2/tick.rs:8,47 / tuike_v2/skills.rs:11,356,361 / tuike.rs:17,545 / tuike_v2/state.rs:201,210 / combat/mod.rs:244(v2),368(v1) / resolve.rs:56 FalseSkin/TwoHand 站点 + :1320 contam filter / false_skin_state_emit.rs。plan §P2 / §P3 公式 8。

### #10 worldview 补写（人工单独 PR 先 land）
**决议**：(1) §545（装备 loadout 段，worldview.md:545 现「装备 loadout（武器 / 护甲 / 伪皮 / 暗器载体）」）补『伪皮作胸部穿戴层，与真甲混穿各自生效』；(2) §468（worldview.md:468 主战斗变量表「伪皮档位 轻/中/重 + 单层吸收上限」）补档位注（多层叠加上限引 §11.1 #6 worn_cap）；(3) 在 §545 段内立「修士可层叠穿戴/分层装备」锚点（否则属无 worldview 锚点玩法，§四红旗）。**人工单独 PR 先 land**（docs/CLAUDE.md §6.3，CLAUDE.md/worldview.md 严禁 agent 自动改）；plan 写死节号供执行。
**落点**：docs/worldview.md:545 / :468 / §439（蜕壳流出处不改，仅引用）。plan §10 worldview PR。

### #12 worn 栈语义（LIFO，仅栈顶可操作）
**决议**：
1. 每个装备槽的 `worn: Vec<ItemInstance>` 语义 = **栈（LIFO，后进先出）**，约定**栈顶 = Vec 末尾**——装备 = `worn.push(item)`（push 到尾，受 `worn_cap`）、卸下 = `worn.pop()`（pop 从尾）。
2. **只有栈顶（最上层 / `worn.last()`）能被拖下/卸下**；下层被压住，必须先脱上层才能动到下层；脱掉栈顶后，下一层（新 `worn.last()`）成为新的可操作/可显示顶层。
3. `validate_move_semantics` 从 worn 槽移出/卸下时只允许栈顶件；移动非栈顶（被压住）件 → 拒绝 + 特定 err 文案「{上层件名} 压在上面，请先脱下上层」（决议 #3 拒绝不顶替的延伸：不允许从中间抽件）。`move_equipped_item_to_first_container_slot` / unequip 路径按「只能 pop 栈顶」重写（不再 `.position()` 移任意中间件、不整槽 `.remove`）。
4. **与决议 #1（worn 表示=方案B Vec）自洽**：Vec 形态不变，#12 只追加「Vec 用作栈、尾=顶、仅顶可动」的访问约定；提供 `worn_top()`/`worn_top_mut()`（`worn.last()`/`last_mut()`）作栈顶入口。
5. 客户端镜像：worn 栈叠放渲染（栈底 worn[0] 下、栈顶在最上可拖；下层 dim opacity 0.40 + 不可拖、底部露 ~2px 示意被压住）；脱顶后下层正确显示为新顶（高亮转移）。`InventoryEquipRules.canEquip` 镜像「仅栈顶可拖」。

**落点**：server `SlotContents`+`worn_top`（mod.rs，§P0.1）/ `move_equipped_item_to_first_container_slot`（mod.rs:2991-3010，改「仅 pop 栈顶」）/ `validate_move_semantics` worn 分支（mod.rs:4151-4321，加「移非栈顶拒绝」）/ unequip 路径（mod.rs:3095/:3354/:3690）；client `EquipSlotComponent` worn 栈渲染 + `InventoryEquipRules.canEquip`（EquipmentPanel.java / InspectScreen.java TAB_EQUIP drop 分支）。plan §P0.1 / §P0.2 / §P1 / §P4。

### #13 行囊 UI 移除 + 背包穿戴化 + body_pocket 转默认容器 tab
**决议**：
1. **删「行囊」tab + 面板全部客户端构件**（取代 PR #714，commit a5eef15b，已 merge；#714 仅动 `InspectScreen.java` +83/-33）：删字段 `backpackTabLabel`/`backpackEquipWrapper`/`backpackSlotBack`/`backpackSlotWaist`/`backpackSlotChest`/`backpackWeightLabel`/`bodyPocketGrid`（:102-108）+ 方法 `buildBackpackEquipPanel`(:1550)/`switchToBackpackTab`(:1502)/`buildLabeledEquipSlot`(:1590)/`refreshBackpackEquipPanel`(:1606)/`refreshBackpackEquipPanelIfActive`(:1634) + 行囊 tab `§f行囊`(:574-592) + build 挂载/默认激活(:635-638/:690)。
2. **背包改为穿戴装备 —— ⚠️ #13.2 已被决议 #17 取代**：原方案是在统一「装备」面板里把 3 个背包 equip 槽（背/腰/前胸）迁进主装备面板当 worn 槽、保留 `EquipSlotType.BACK_PACK|WAIST_POUCH|CHEST_SATCHEL` 枚举、`EQUIP_SLOT_BY_WIRE_NAME` 补 5 条 wire（含背包三槽）。**⚠️ 决议 #17 取代**：取消背包专属槽，背包按 `ContainerSpec.equip_slot` 指定身体槽（head/chest/legs/feet）作该身体槽 worn 层穿戴；`EquipSlotType.BACK_PACK|WAIST_POUCH|CHEST_SATCHEL` 删除；`EQUIP_SLOT_BY_WIRE_NAME` **只补 extra_hand_0/1 两条（不补背包三槽）**，背包件随身体槽 `<slot>_worn` 数组下发，`parseEquipped` 无背包特判。穿上后仍提供其容器网格（右侧容器 tab 照旧）。本条作废，实施以 #17 为准。
3. **body_pocket 改默认常驻容器 tab**：原在行囊面板（#714 把它从容器 tab 列表过滤、只在行囊内 grid 渲染）；行囊删后改为右侧容器 tab 区**始终存在**的基础随身格 tab（`containerDefs` 始终含 body_pocket，移除 InspectScreen build :558-562 的 body_pocket 过滤），不再依附行囊面板；新玩家默认激活页落 body_pocket。body_pocket 是 `InventoryModel.BODY_POCKET_CONTAINER_ID`（容器 id，**非 EquipSlotType**）。
4. **与 #714「body_pocket 不单独占 tab」张力的调和**：#714 该决策前提是「body_pocket 已在行囊面板内渲染、独立 tab 重复」；本 plan 删行囊面板后该前提消失，body_pocket 失去依附点，故反转为默认容器 tab（不再重复，不冲突）。
5. **server 侧容器产出必改（#13.5 订正 server「无需改」错误，blocker；与决议 #17 调和）**：实测 `rebuild_containers_from_equipment`（mod.rs:3519-3584，:3536-3540 `for slot_id in [BACK_PACK,WAIST_POUCH,CHEST_SATCHEL]` 硬编码背包槽列表 + :3541 `.get(slot_id).and_then(\|item\| registry.get(&item.template_id))` 把返回值当 `&ItemInstance`）SlotContents 化后编译红。**决议 #17 重定向（取代原「背包走 `worn[0]` 退化槽」）**：背包专属槽取消后，改为**扫所有身体槽（head/chest/legs/feet）worn 层里带 `container_spec` 的件**生成容器（`equipped.values().flat_map(\|s\| s.worn.iter())` 过滤 container_spec，容器 id 用件 instance_id 或槽+层索引，不再用背包 slot 名），桶③；现有测试 mod.rs:9254/9306/9335（rebuild_containers_* 系列）随改。`compute_max_weight`（mod.rs:3489-3507）同样从遍历固定背包槽常量改为扫身体槽 worn 件求和。
   - **⚠️ 死代码纠偏（gap#10/#15 修正 #13.5 前提，§12）**：经核实 `rebuild_containers_from_equipment` **运行时是死代码** —— 其唯一 caller 是 `handle_backpack_break`（背包破损簇，本身 dead，见 §P0.1 删常量连带处理）。**背包→容器网格的运行时真实来源是 `instantiate_inventory_from_loadout`（mod.rs:967-985）静态拷贝 default.toml `[[containers]]`（default.toml:32 `id=back_pack` 静态容器）**，不是 rebuild。故原 #13.5「rebuild 是唯一产出点」前提有误。修正方向二选一（实施时定，否则「背包骑身体槽 worn 后生成容器 tab」是空中楼阁）：
     - **(A) 接入运行时**：把 `rebuild_containers_from_equipment` 真正 wire 进装备/卸下流程（穿背包→调 rebuild 增容器 tab、卸下→删），并定 default.toml `[[containers]].id` 与 rebuild 新容器 id（按 instance_id 或槽+层索引）**命名规则一致**，否则静态容器 `id=back_pack` 与新 instance 命名空间失配 → 孤儿 `back_pack` 容器 / 背包件不生成正确容器。
     - **(B) 保持静态来源**：仍由 `instantiate_inventory_from_loadout` 静态拷贝 `[[containers]]` 产容器，**但 default.toml `[[containers]].id` 与 `[[equip]]` 背包件 instance / 身体槽 worn 命名规则对齐**（容器 id 与穿戴背包件可互查），rebuild 保持 dead 或删除。
   - 注：原决议剔除的「rebuild reconcile 过度设计」（验证 real=false）不折进——指的是**不需要**为静态容器额外加一套 reconcile 协调层；本纠偏只要求容器 id 命名规则一致，不引入 reconcile。

**开放点（多数已被 #17/#18/#19 收口；仅余 1 条物品数据层）**：
- ~~背/腰/前 背包 worn 槽的确切身体位~~ → **决议 #17 改写**：取消背包专属槽，背包按 `ContainerSpec.equip_slot` 指定身体槽（head/chest/legs/feet）作 worn 层；**仅余开放点**：现有各背包物品具体落哪个身体槽（worn_grass_pouch→? / 腰包类→? / 前挂→chest）= 物品数据层实现细节，实施时定（§11.1 #17）。
- ~~是否计入 worn_cap~~ → **决议 #17 已定**：背包件计入其所在身体槽（head/chest/legs/feet）的 worn cap，与盔甲/伪皮同槽叠层共算，无独立退化 cap。
- ~~body_pocket 作默认 tab 的确切呈现位~~ → **决议 #18 已定**：body_pocket 恒为容器 tab 列表第一个（默认/最左激活页），其余容器排其后。
- ~~原行囊面板重量条去向~~ → **决议 #19 已定**：删除 `backpackWeightLabel` + `backpackWeightBreakdown`（不并入任何面板），负重显示沿用整体 inventory 底部既有 `BottomInfoBar`（BottomInfoBar.java:12，挂 InspectScreen.java:648-649）；`BackpackEquipSlotTest` 对应测试删/迁（见 §P4 测试）。

**落点（符号名 + 约行号双锚；背包相关条目按决议 #17 重定向）**：client `InspectScreen.java`（删字段约 :102-108 含 `backpackWeightLabel`（#19）/ 方法 `switchToBackpackTab` 约 :1502 / `buildBackpackEquipPanel` 约 :1550 / `buildLabeledEquipSlot` 约 :1590 / `refreshBackpackEquipPanel` 约 :1606 / `refreshBackpackEquipPanelIfActive` 约 :1634 / `backpackWeightBreakdown` 约 :1004（删除，#19）/ 行囊 tab `§f行囊` 约 :574-592 / 挂载约 :635-638,690 / **`filteredContainerDefs` body_pocket 过滤段约 :556-564（订正，原误写 :557-563；body_pocket 排第一，#18）** / `activeGrid` 约 :962 / `switchToGridContainer` 约 :1537 / `placeItemAnywhere` 兜底约 :3008 / `activeContainer==containerCount` 哨兵约 :89-101,1480,1521,1635,2989）+ `EquipmentPanel.java`（约 :29-45，**仅新增 EXTRA_HAND_0/1 worn 槽（决议 #17：不加背包三槽，背包随身体槽 worn 渲染）**，核 `PANEL_HEIGHT` 约 :17）+ `EquipSlotType.java`（约 :16-18 **删三背包槽（决议 #17）**、新增 EXTRA_HAND_0/1）+ `InventoryModel.BODY_POCKET_CONTAINER_ID`（约 :20）+ `InventorySnapshotHandler.EQUIP_SLOT_BY_WIRE_NAME`（约 :24-35，**仅补 extra_hand_0/1 两条 wire（决议 #17）**）+ `ClientRequestProtocol.EquipLoc`（约 :571，加 state，PR-1）+ `InventoryEquipRules.canEquip`（约 :85,119）。**server 侧 `rebuild_containers_from_equipment`（mod.rs:3519-3584）+ `compute_max_weight`（mod.rs:3489-3507）+ `ContainerSpec.equip_slot`（mod.rs:227）必须改（#13.5 订正 + 决议 #17）**。plan §P4 / §11.1 #17/#18/#19 / §接入面 #714 取代关系。

### #16 Treasure/Shield/Container worn/held 分类（补充决议，§11 开放问题表第 16 行）
**决议**：`ItemCategory::Weapon|Tool` → Held（不计 worn cap）；`Armor|Treasure|Shield`（mod.rs:286 各变体）+ 伪皮物品 → Worn（计 worn cap）；`Container`（背包）→ Worn（**决议 #17 订正：归其 `ContainerSpec.equip_slot` 指定身体槽（head/chest/legs/feet）的普通 worn 层，计该身体槽 worn cap，不再是独立退化槽 worn_cap=1**）。Treasure 物品作为装备件归 Worn（计 cap）；其「激活态」另由触发位承载（#8），二者正交——装备槽里的 treasure 仍是 worn 件，激活才进触发位（scan 迭代见 §P2 spirit_treasure：装备槽 treasure equipped=true/passive_active=false，触发位 treasure passive_active=true）。
**落点**：`classify_equip_state`（mod.rs，§P0.1）；ItemCategory（mod.rs:286）；validate_move_semantics OffHand 接受 Treasure（mod.rs:4173）/ Shield（:4184）。plan §P0.1 / §P1 / §P2。

### #17 背包不再有专属槽，按 ContainerSpec 指定身体部位作 worn 层（取代 #5 退化槽 + #13.2/#16 背包分类 + v2 已折进的「背包槽 wire/字段」修法）
**决议**：
1. **取消固定的 `EQUIP_SLOT_BACK_PACK / WAIST_POUCH / CHEST_SATCHEL` 三个专属装备槽**（连同 `EquipSlotV1::BackPack|WaistPouch|ChestSatchel` / client `EquipSlotType.BACK_PACK|WAIST_POUCH|CHEST_SATCHEL` / proto·rust·agent-ts 三个背包 snapshot 字段 / client `EQUIP_SLOT_BY_WIRE_NAME` 三条背包 wire 映射 全部删除）。这是重要模型简化——全链触点**变少**（不再补背包槽 wire、不再补背包 snapshot 字段）。
2. **每个背包物品的 `ContainerSpec.equip_slot`（mod.rs:227，现值 back_pack/waist_pouch/chest_satchel）改为指向身体槽**（head/chest/legs/feet 之一，由背包物品自身决定，例：腿部小包→legs、破草包→chest/back 对应身体部位）。背包作为该身体槽的一个 **worn 层**穿戴（`worn.push`），计入该身体槽 worn_cap、走 LIFO 栈语义（决议 #12），穿上后仍生成其容器网格（右侧容器 tab 照旧）。TOML `valid_slots` 校验（mod.rs:1736-1741）随之改为身体槽集。
3. **重新调和之前因「背包独立槽」折进的 v2 修法（这些现在变了）**：
   - client `EQUIP_SLOT_BY_WIRE_NAME`（InventorySnapshotHandler.java 约 :24-35）**不再补 back_pack/waist_pouch/chest_satchel 三条**（背包 wire 槽名取消）；**只需补 extra_hand_0/extra_hand_1 两条**。背包随身体槽 `<bodyslot>_worn` 数组下发。决议 #13.2「补背包槽 wire」彻底作废。
   - agent-ts / proto / rust schema **不再有 back_pack/waist_pouch/chest_satchel 三个 snapshot 字段**（删除，而非 worn 化）；背包件出现在对应身体槽的 `<bodyslot>_worn` 数组里。§P0.3/#5 关联的「背包槽 wire 形态」表述随之简化（不存在背包槽 wire）。
   - `rebuild_containers_from_equipment`（mod.rs:3519-3584，:3536-3540 现 `for slot_id in [BACK_PACK,WAIST_POUCH,CHEST_SATCHEL]`）：从「按固定背包槽 key 查」改为「扫所有身体槽 worn 层里带 `container_spec` 的件」生成容器（容器 id 用件 instance_id 或槽+层索引）。
   - `compute_max_weight`（mod.rs:3489-3507，:3490-3494 现遍历固定背包槽常量数组）背包加成：从「按背包槽 key 查」改为「扫 worn 层里 `container_spec` 件」求和 weight_capacity。
   - worn_cap：head/feet=2、chest/legs=3 不变；背包占其所在身体槽的一个 worn 层（受该槽 cap），**不再有独立 cap=1 退化槽**（决议 #5 退化逻辑删除）。
   - `server/assets/inventory/loadouts/default.toml` `[[equip]]` 里 `slot=back_pack/worn_grass_pouch` 等的 slot 改为对应身体槽（worn_grass_pouch 落 chest 还是另立 = 物品数据实现细节，见开放点）。
   - 客户端 `EquipmentPanel`：**不再 addSlot 背包三槽**；背包作为身体槽 worn 层渲染（与盔甲/伪皮同槽叠层）。
   - `EquipSlotType`(client) / `EquipSlotV1`(server/agent) **删 BACK_PACK/WAIST_POUCH/CHEST_SATCHEL**（连同之前已计划删的 FALSE_SKIN/TWO_HAND/TREASURE_BELT，共 9 变体）。
4. **开放点（物品数据层，实施时定）**：现有各背包物品具体落哪个身体槽——worn_grass_pouch→? / 腰包类→? / 前挂→chest。当前仓库仅 `worn_grass_pouch`/`grass_pouch`（core.toml:384-407）配 container_spec、均 `equip_slot="back_pack"`，迁移时按物品数据层定的身体槽改写。

**与现有决议关系**：取代 #5（背包退化槽全删）；订正 #13.2（背包 wire 作废，仅留 extra_hand_0/1）+ #13.5（rebuild_containers 改扫身体槽 worn，非背包槽 key）；订正 #16（Container→身体槽普通 worn 层，非退化槽）。与 #12（worn 栈 LIFO）自洽——背包件即身体槽 worn 栈普通一层。无新冲突。

**落点**：server `ContainerSpec.equip_slot`（mod.rs:227）/ TOML valid_slots（mod.rs:1736-1741）/ 背包常量 EQUIP_SLOT_BACK_PACK/WAIST_POUCH/CHEST_SATCHEL（mod.rs:91/94/97 删）/ `worn_cap`（§P0.1 无背包槽）/ `classify_equip_state`（§P0.1）/ rebuild_containers_from_equipment（mod.rs:3519-3584）/ compute_max_weight（mod.rs:3489-3507）/ migrate_equipped_v1_to_v2（§P0.6 旧背包槽→身体槽 worn）/ default.toml（slot 改身体槽）/ `EquipSlotV1`（schema/inventory.rs:33；inventory.ts:21）/ proto EquippedInventorySnapshot（envelope.proto:556 删背包字段）/ client EquipSlotType.java（删三背包槽）/ InventorySnapshotHandler.EQUIP_SLOT_BY_WIRE_NAME（约 :24-35 仅补 extra_hand）/ EquipmentPanel.java（不 addSlot 背包）/ InventoryEquipRules.canEquip（背包件走身体槽 worn 校验）。plan 头部 / 阶段总览 P4 / 跨仓库契约表 / §P0.1-P0.6 / §P1 / §P4。

### #18 body_pocket 是容器 tab 第一个（默认/最左）
**决议**：`body_pocket` 始终作为右侧容器 tab 列表的**第一个**（默认激活页 / 最左），其余容器（身体槽 worn 背包件生成的容器）排其后。`containerDefs` 把 body_pocket 置 index 0；默认激活页落 body_pocket（`activeContainer` 默认 = body_pocket 索引 0）。订正 §11.1 #13 里「body_pocket 确切呈现位」开放点为已定（不再「或第一个容器」二义）。
**落点**：client InspectScreen build `filteredContainerDefs` body_pocket 过滤段（约 :556-564，移除过滤 + 置 index 0）/ 默认激活页（约 :690，原 switchToBackpackTab → 默认 body_pocket）/ `activeContainer` 默认值（约 :92）。plan §P4「body_pocket 改默认常驻容器 tab」/ §11.1 #13 开放点。

### #19 删行囊重量条，用整体 inventory 底部已有重量显示
**决议**：删 `backpackWeightLabel`（行囊专属重量条 label，InspectScreen.java 约 :107）+ 静态方法 `backpackWeightBreakdown`（InspectScreen.java 约 :1004），**重量条不再单独渲染、不并入任何面板**；负重显示**沿用整体 inventory 底部既有 `BottomInfoBar`**（已存在：`BottomInfoBar.java:12`，挂在 InspectScreen.java:648-649 mainPanel 底部，读 `model.currentWeight()/maxWeight()` 整体重量、过载变红，刷新在 populateFromModel InspectScreen.java:1664）。`BackpackEquipSlotTest` 里依赖 `backpackWeightBreakdown` 的断言（约 :215/230/243/251/262）删除/改测 `BottomInfoBar`。订正 §11.1 #13 里「重量条去向」开放点为已定（删除，沿用 BottomInfoBar）。
**落点**：client InspectScreen.java（删 `backpackWeightLabel` 约 :107 / `backpackWeightBreakdown` 约 :1004）/ `BottomInfoBar.java:12`（既有组件，不改，仅沿用）/ BackpackEquipSlotTest（重量断言删/迁）。plan §P4 删行囊构件清单 / §11.1 #13 开放点。

### #20 封灵容器 attrition_exempt 运行时链路 retarget（聚焦验证 gap#3 blocker，新增决议）
**决议**：`container_attrition_exempt`(mod.rs:4504) → `container_id_to_equip_slot(container_id)`(mod.rs:4509) → `equipped.get(slot)` → `spec.attrition_exempt` 是**装备槽 attrition_exempt 当前唯一真 wired 的运行时联动**（封灵搬运跳过 qi 磨损），运行时入口 `client_request_handler.rs:9499`(SlotMove) / `:9864`(Pickup) 真实调用。决议 #17 删 back_pack/waist_pouch/chest_satchel slot 常量 + 改容器 id 后，`container_id_to_equip_slot` 仅识别旧三固定字符串 → **恒 None → 封灵豁免静默失效**（plan 原全文 0 处提及）。
1. attrition_exempt 判定从『container_id → slot 名反查 → equipped.get(slot)』改为『**容器 id 直接映射其对应 worn 背包件 instance（容器 id 与背包件 instance / 槽+层索引命名规则一致，见 #13.5/#17）→ 读该件 `container_spec.attrition_exempt`**』。
2. `container_id_to_equip_slot`（mod.rs:3626 背包破损簇内 + mod.rs:4509 attrition 链 两处）**重写为「容器 id → worn 背包件 instance 定位」或废弃**；验证 `client_request_handler.rs:9499/:9864` 两运行时调用点改用新映射。
3. 与 #13.5 死代码纠偏方向一致——容器 id 命名规则统一后，attrition 反查 / rebuild 产容器 / 静态 `[[containers]]` id 三者共用一套命名空间。
**落点**：mod.rs:4504(container_attrition_exempt) / :4509,3626(container_id_to_equip_slot 两处) / client_request_handler.rs:9499,9864（运行时入口）/ default.toml:32 静态容器 id。plan §P0.2「封灵容器 attrition_exempt 运行时链路」子任务 / §P0.6 静态容器 id 对齐 / §11.1 #13.5/#17。

---

## §12 装备功能联动总册（聚焦验证普查 2026-06-25，防缺失统计记录）

> 用户点名「把装备件靠『在 inventory.equipped 里』提供的功能联动逐项统计并记录，防缺失」的交付物。31 条联动（聚焦验证 census→rebut→synthesize 三阶段逐点核实，read_site 均 file:line 实测）。
> **列**：item_kind / function / read_site(file:line) / 分层后取值语义（桶 ①取 held / ②遍历 worn 累加 / ③按 slot 查单件 / ④迭代全件，见 §P0.2 桶表） / 覆盖（✅ 已在 plan 阶段覆盖 / ⚠️ 缺口，指向折入触点）。
> ⚠️ 行对应 confirmed_gaps，gap# 编号与各阶段块行内标注一致；折入后仍标 ⚠️ 以保留「曾是缺口」的可追溯性。剔除验证 real=false 3 条（背包件防御=0 自动 skip、CHEST cap 共享、rebuild reconcile 过度设计），不入册。

| # | item_kind | function | read_site (file:line) | 分层取值语义（桶） | 覆盖 |
|---|-----------|----------|-----------------------|--------------------|------|
| 1 | armor | 防御减伤聚合 — 4 槽护甲 ArmorProfile 按(BodyPart,WoundKind)生成 DerivedAttrs.defense_profile | armor_sync.rs:26 (get(slot)) + :42 (.max()) | ② worn.iter() 累加；.max()→+= 加性后 clamp | ✅ §P3 公式1 |
| 2 | armor | 护甲负重 armor_mass（影响击退 BodyMass.total_mass） | body_mass.rs:148-159 equipped_armor_mass | ② worn.iter() + **filter ArmorProfile** | ✅ §P3 公式6（gap#13） |
| 3 | armor | 真脉/截脉 prep 窗口修正 — 4 护甲槽最重 weight 决定 jiemai_armor_modifier | jiemai.rs:72-83 (filter_map(get).map(weight).fold(max)) | ② worn.iter().map(weight).fold(max)（原误分桶①） | ⚠️ §P0.2 桶② + §P3（gap#4 major） |
| 4 | weapon | 当前武器派生 — main/off/two_hand 取手持武器 + Weapon component 同步 | weapon.rs:169 (:175 get two_hand) + 枚举 weapon.rs:236-239(TwoHand) | ① main/off→.held、删 two_hand 分支+枚举变体 | ✅ §P2 |
| 5 | weapon | 武器损坏脱手定位 — broken weapon 按 instance 找槽(含 TWO_HAND) | resolve.rs:740 (match TWO_HAND) + forge/artifact_meridian.rs:681 (match TWO_HAND→EquipSlotV1::TwoHand) | ③ flat_map(worn+held) 按 instance 定位；删 TWO_HAND | ✅ §P2 resolve + §P0.2 桶③ forge:681（gap#11） |
| 6 | tool | 采集工具派生 — main/off/two_hand 找采集工具 spec | gathering/tools.rs:238 (get TWO_HAND) + :253 (filter_map get) | ① 双手工具 main_hand.held、main/off→.held、删 TWO_HAND | ✅ §P0.2 桶① |
| 7 | tool | 矿物采集 pickaxe tier — main_hand/two_hand 取镐品阶 | mineral/break_handler.rs:33 (import) + :410 ([MAIN,TWO_HAND].filter_map(get)) | ① main_hand→.held、删 TWO_HAND import+项 | ⚠️ §P0.2 桶①（gap#1 blocker） |
| 8 | tool | 斧砍灵木 tier — main_hand/two_hand 取斧品阶 | spiritwood/mod.rs:504 (get MAIN.or_else(TWO_HAND)) | ① main_hand→.held、删 TWO_HAND or_else | ✅ §P0.2 桶① |
| 9 | shield | 盾牌格挡 — off_hand 取盾 is_shield → RaiseShield，block_ratio 减伤 + 体力 drain | shield_block.rs:230 (get OFF_HAND).filter(is_shield)；消费 resolve.rs block_ratio.clamp(0,0.95) | ① off_hand→.held；block_ratio 与 armor 正交两链 | ✅ §P0.2 桶① + §P3 公式4 |
| 10 | treasure | 法宝被动激活 scan — 遍历 equipped 标 equipped/passive_active → ActiveSpiritTreasures | spirit_treasure.rs:238 (equipped.values()) + :372 (passive_active=equipped) | ④ 激活迁触发位(#8)；装备槽 worn treasure flat_map(worn.chain(held)) equipped=true/passive_active=false | ✅ §P2 + §11.1 #8/#16 |
| 11 | false_skin | v1 蜕壳流 contam 吸收 — FalseSkin 单组件逐层吸收 contam（resolve 真实消费链） | tuike.rs:545 sync_false_skin_from_inventory(get FALSE_SKIN 单件)；消费 resolve.rs:1320 | 取 CHEST worn.last() 单件，多层归 v2 | ⚠️ §P2 v1 retarget + §11.1 #9（gap#6 major） |
| 12 | false_skin | v2 StackedFalseSkins 多层伤害吸收 — damage_capacity 逐层吸收 | tuike_v2/state.rs:201/:210 (#[allow(dead_code)])；tick.rs:47 sync | 改扫 CHEST worn 多层；damage_capacity 不接 resolve（dead_code） | ⚠️ §P2 + §11.1 #9（gap#7 major，归 tuike_v2 plan） |
| 13 | false_skin | 伪皮装备态读取来源 + 多层维持真元成本 + 蜕层 ShedEvent | tuike_v2/tick.rs:47 + skills.rs:356/361 + tuike.rs:17 + resolve.rs:56 (get FALSE_SKIN) | 读点 FALSE_SKIN 专槽→扫 CHEST worn(false_skin_kind_for_item) | ✅ §P2 |
| 14 | carrier | 暗器载体真元 imprint — main/off 取 carrier 件充能/降级/丢出/共鸣 + 守恒求和 | carrier.rs:459/579/692/750/758/781/798/1377 + anqi_v2.rs:840；ledger.rs:571 | ① main/off→.held + §P3 held 求和 | ✅ §P0.2 桶① + §P3（blocker 已覆盖） |
| 15 | backpack | 容器网格生成 — 扫背包槽 ContainerSpec → 生成容器 tab | mod.rs:3519 rebuild_containers_from_equipment(:3536 for [BACK_PACK,WAIST_POUCH,CHEST_SATCHEL]) | ③ 扫身体槽 worn 带 container_spec 件；**但 rebuild 运行时死代码，真来源=静态 [[containers]]** | ⚠️ §11.1 #13.5/#17 死代码纠偏（gap#10 PARTIAL） |
| 16 | backpack | 负重容量 max_weight — 背包 ContainerSpec.weight_capacity 求和加到 BASE_CARRY_CAPACITY | mod.rs:3489 compute_max_weight (遍历固定背包槽常量数组) | ③ 扫身体槽 worn 带 container_spec 件求和 | ✅ §P1 + §11.1 #17 |
| 17 | backpack | 封灵容器 attrition_exempt — 搬运跳过 qi 磨损（唯一真 wired 背包→功能联动） | mod.rs:4504 container_attrition_exempt → :4509 container_id_to_equip_slot → equipped.get(slot)；运行时 client_request_handler.rs:9499/:9864 | 容器 id → worn 背包件 instance → spec.attrition_exempt | ⚠️ §P0.2 子任务 + §11.1 #20（gap#3 blocker） |
| 18 | backpack | 背包耐久磨损/破损整簇（apply_backpack_wear/handle_backpack_break/slot_display_name/BackpackBreakEvent，全靠 slot 名键） | mod.rs:3626 container_id_to_equip_slot / :3644 / :3682 / :3587；测试 :9844-10168 | 整簇 dead_code 但引用常量 → 删常量编译红+测试红 | ⚠️ §P0.1 删常量连带处理（gap#8 major） |
| 19 | backpack | default.toml 静态 [[containers]] id=back_pack — 运行时背包容器真实来源（非 rebuild） | default.toml:32(id=back_pack 静态) + :82(slot=back_pack 装备件)；instantiate_inventory_from_loadout mod.rs:967-985 静态拷贝 | 静态容器 id 与新 instance / 身体槽命名空间须对齐 | ⚠️ §P0.6 静态容器 id 对齐 + §11.1 #13.5（gap#10 major） |
| 20 | backpack | ContainerSpec.equip_slot TOML 校验 valid_slots | mod.rs:1736-1741 valid_slots=[BACK_PACK,WAIST_POUCH,CHEST_SATCHEL] + err 文案 | valid_slots 改身体槽集 [head,chest,legs,feet] | ✅ §11.1 #17 |
| 21 | backpack | 背包件存档迁移 旧专槽→身体槽 worn | state.rs:1090 load_player_inventory_from_sqlite + migrate_equipped_v1_to_v2(待建) | 旧 back_pack 件按 container_spec 重定向身体槽 worn 栈尾 | ✅ §P0.6 |
| 22 | all | 负重当前值 current_weight — 全 equipped 件 weight×stack 求和(含 held 武器) | mod.rs:3462 calculate_current_weight | ② 每槽 worn 全件 + held；背包件自重计入、weight_capacity 不计入 | ✅ §P3 公式7 |
| 23 | all | 装备件真元 carrier 守恒求和 — inventory_qi ledger 遍历 equipped 累加 item_qi | ledger.rs:571 (equipped.values().map(item_qi).sum()) | ② flat_map(worn.chain(held)).map(item_qi).sum() | ✅ §P3（blocker 已覆盖） |
| 24 | all | 耐久/真元衰减 + 真元广播 sweep — values_mut/values 遍历 equipped 全件 | shelflife/sweep.rs values_mut + world/block_drop.rs chain(equipped.values()) + forge/artifact_meridian.rs:913/:932 | ④ values().flat_map(worn.iter().chain(held.iter())) | ✅ §P0.2 桶④（forge:913/932 展开行号，gap#11） |
| 25 | weapon | 死亡掉落武器保护(标准+秘境) — main/off/two_hand 高耐免 50% Roll；整槽 equipped.remove(&slot) partition 掉落 | mod.rs:3283/3306(match TWO_HAND)/:3354(remove(&slot)) + tsy_death_drop.rs:72(match)/:194(remove) | ④ 删 TWO_HAND match、按 instance 在 worn/held 精确移除（禁整槽 remove 连带删未掉落件） | ⚠️ §P0.2 死亡掉落显式子任务（gap#2 blocker） |
| 26 | all | C2S move 落位 resolver — 按 instance 在 equipped 找槽返回 Equip{slot} | client_request_handler.rs:9367 find_inventory_instance_location | 在 worn/held 定位 instance 推导 state(worn→Worn/held→Held) | ⚠️ §P0.3 server resolver（gap#5 major） |
| 27 | material(equipped) | 灵龛守护材料计数 — 统计某 template_id 在 equipped 的 count/find 决定守护激活 | social/niche_defense.rs:480 (filter count) + :518 (find) | ④ flat_map(worn.chain(held))；.template_id 两处编译红 | ⚠️ §P0.2 桶④ + §P3 测试（gap#9 major） |
| 28 | armor(technique) | 功法光波体操给四肢加性防御 limb_defense → 同写 defense_profile（与装备护甲共享矩阵） | body_conditioning.rs:174 entry((part,kind)).and_modify(+limb_def.min(CAP)) | 非 equipped 联动，但 armor_sync 加性化后合成顺序/clamp 须协调 | ⚠️ §P3 公式3（gap#12 minor，并入 defense math） |
| 29 | backpack/container | schema 容器 id 常量 CONTAINER_ID_*（dead 但绑旧 slot=容器 id 契约） | schema/inventory.rs:19/21/23 (无引用) | 删容器 id 后 docstring 误导 → 删除 | ⚠️ §P0.3 删死常量（gap#14 minor） |
| 30 | all | 技能施放/物品查询 instance→template 解析 — 遍历 equipped 按 instance/template 找 | cast_emit.rs:443 + client_request_handler.rs:3264/8414/9341/11704/11734 | ④ 通用规则覆盖 .values().flat_map(worn.chain(held))，编译时统一改 | ✅ §P0.2 桶④（LOW，泛化覆盖） |
| 31 | all | unequip→rehome 路径 + helper(detach_instance/attach_at_location/clone_item_at) | mod.rs:2991 move_equipped_item_to_first_container_slot + :4544 detach + :4560 attach + :4395 clone | LIFO 仅 pop 栈顶 + helper SlotContents 化 | ✅ §P0.2 四 helper + LIFO 重写 |

**统计**：31 条联动 = **18 条 ✅ 已覆盖** + **13 条 ⚠️ 缺口**（对应 14 条 confirmed_gaps；gap#11 forge:681、gap#13 body_mass filter 落在已覆盖行——§12 行 5/24 / 行 2 核心 retarget 已 ✅、只补行号或 filter 细节，故未单列 ⚠️ 行）。13 条 ⚠️ 按 §12 行号 + 严重性：
- **3 blocker**：行 7（gap#1 mineral/break_handler TWO_HAND 漏列）/ 行 25（gap#2 死亡掉落 TWO_HAND match + 整槽 remove 数据破坏）/ 行 17（gap#3 封灵 attrition_exempt 静默失效）。
- **6 major**：行 3（gap#4 jiemai 错分桶）/ 行 26（gap#5 C2S resolver 缺 state）/ 行 11（gap#6 v1 伪皮取件未定）/ 行 12（gap#7 v2 damage_capacity dead_code）/ 行 18（gap#8 背包破损簇编译红+测试红）/ 行 27（gap#9 niche_defense 编译红）/ 行 19（gap#10 default.toml 静态容器失配）—— 注：gap#10 跨行 15/19（rebuild 死代码 + 静态容器），故 major 实为 7 个缺口落 8 行。
- **2 minor**：行 28（gap#12 body_conditioning clamp 合成顺序）/ 行 29（gap#14 schema CONTAINER_ID 死常量）。
- **1 PARTIAL/LOW**：行 15（gap#10 rebuild 运行时死代码，与行 19 同源）。
全部 ⚠️ 已按 gap# 折入对应阶段（行内带 gap# 标注），标 ⚠️ 仅作「曾是缺口」可追溯。剔除 real=false 3 条不入册。

---

## §10 实施工作流

### §10.1 多 PR 序列化拆分（依赖顺序，前一个 merge 后开下一个）

> scope = 5 PR + 1 人工 worldview PR。单 plan 内序列化，不拆多 plan（docs/CLAUDE.md §6.3）。

| PR | 范围 | 阶段 | 依赖 |
|----|------|------|------|
| **worldview PR**（人工） | §545/§468 补写 + 分层装备锚点 | §11.1 #10 | 无（**最先 land**） |
| **PR-1** | 数据模型 SlotContents/EquipState + wire schema（proto/Rust/agent-TS，**删背包三槽（决议 #17）+ agent-TS 补 extra_hand_0/1 两字段修漂移 + ContainerSpec.equip_slot 改指身体槽**）+ 4 sample + 存档迁移（旧背包槽→身体槽 worn）+ **client C2S `EquipLoc.state`（ClientRequestProtocol.java:571 + 5 调用点 + 测试，blocker，必须与 server state 必填同 PR）** | P0 | worldview PR |
| **PR-2** | 装备校验分层规则 + 蜕壳流 retarget（v1+v2）+ 移除 FALSE_SKIN 专槽 | P1 + P2 | PR-1 |
| **PR-3** | 效果叠加 + 负重 + equipped 真元 carrier 守恒求和 | P3 | PR-2 |
| **PR-4** | client 面板重构（删行囊 tab + 背包随身体槽 worn 层渲染（决议 #17）+ body_pocket 默认容器 tab 第一个（决议 #18）+ 删行囊重量条沿用 BottomInfoBar（决议 #19），决议 #13）+ worn 栈 LIFO 叠放渲染（决议 #12）+ 法宝激活触发位（含 §6.1 三轮打磨 + PROMISE） | P4 | PR-3 |
| **PR-5** | 容量升级 hook（worn_cap + 触发位容量可升级） | P5 | PR-4 |

- **worldview PR 必须最先 land**：CLAUDE.md/worldview.md 严禁 agent 自动改（docs/CLAUDE.md §6.3 唯一例外）；consume agent 遇 worldview 改动停下交人工。归档前必须先 land。
- 每 PR 各自走完整 CI + CR + Pi review 等待协议（§10.3）；前一个未 APPROVED/收敛不开下一个。
- PR-1 涉及 schema：pull 后必 `npm run build -w @bong/schema` 重建 dist（memory `project_schema_dist_rebuild.md`）；改 client 资产（PR-4）必同步 resourcepack.rs + manifest sha1/size（memory `feedback_resourcepack_sha1_sync.md`）。

### §10.2 视觉资产多轮打磨（PR-4）
P4 面板布局 / 分层渲染 / 法宝飞入动画属视觉资产 → 强制 3 轮（round 1/3 first cut → round 2/3 截图 review → round 3/3 终轮）+ 终轮 commit `<PROMISE>` 担保块（拼写 PROMISE）。纯逻辑 PR（PR-1/2/3/5）按常规 atomic commit + 测试全绿。

### §10.3 PR 实施用独立 subagent（context 隔离）
每个 PR 起独立 subagent，主线只接收 result（200-500 token）。强制配置：
```
Agent(
  subagent_type: "claude",
  model: "opus",
  prompt: "...本 PR 范围 + 必读 §10.2 多轮（仅 PR-4）+ 测试要求...\n\nultrathink"
  # isolation 不用 worktree（共享主 worktree 避免 nested）
)
```
- subagent 只负责实施 + 提 PR，**不等 review**（等待逻辑归主线）。
- 主线 merge 命令简单不消耗 context，主线亲自做。

### §10.4 CodeRabbit + Pi review ScheduleWakeup 等待协议
- `gh pr checks <PR>`：pass→merge / pending→`ScheduleWakeup delaySeconds=1200` 等下回合 / fail→按 commands/consume-plan.md step 7 严重性桶处理。
- 禁止 sleep loop / busy poll；每回合 1200s；最多 3 回合（总 60min）卡死才停交人工。
- 修完 review 意见**必须重新等 CR re-review**，不自行判定「我修好了应该过」。
- 等 **CodeRabbit + Pi agent（github-actions）两个 bot 都确认无阻塞**，Pi agent 写 ✅ Approve 才合（memory `feedback_wait_coderabbit_approve.md`）。
- 发长 background 等待后顺手 `ScheduleWakeup ~3000s` 防 cache/session 冷（memory `feedback_workflow_launch_wakeup.md`）。

### §10.5 单次 consume-plan 全自动到 merge
用户提交 `/consume-plan layered-equip-v1` 后即可下班，醒来看 plan 是否在 `docs/finished_plans/`。**前置**：worldview PR 已人工 land + §11.1 决议全收口（本节已完成）。consume agent 按 §10.1 顺序逐 PR 实施 → 各自走 §10.4 等待 → 全 merge 后填 `## Finish Evidence` → `git mv` 入 finished_plans/。

---

## Finish Evidence（2026-06-26 全自动 consume 完成）

### 落地清单（阶段 → 真实模块/文件）
- **P0 数据模型 + wire + 迁移**：`server/src/inventory/mod.rs`（`struct SlotContents{worn:Vec, held:Option}` + `EquipState` + `worn_cap` + `classify_equip_state` + `weapon_two_handed`，删 7 常量/9 EquipSlotV1 变体 + 256 触点四桶 retarget）；`server/src/schema/inventory.rs`（`EquippedInventorySnapshotV1` 方案B worn[]+held + C2S `EquipLoc.state`）；`proto/bong/envelope.proto`（每槽 worn repeated + held optional）；`agent/packages/schema/src/inventory.ts`（同构 + 补漂移字段）；`server/src/player/state.rs`（`INVENTORY_SCHEMA_VERSION` + `migrate_equipped_v1_to_v2`：false_skin→chest worn / two_hand→main_hand held+锁 off_hand / treasure_belt→triggered_treasures / 背包旧槽→身体槽 worn）。
- **P1 校验分层规则**：`server/src/inventory/mod.rs` `validate_move_semantics`（worn cap / worn 栈 LIFO 仅顶可卸 / held 互斥 / 双手锁 main+off+extra_hand / treasure-shield-container 分类）。
- **P2 蜕壳流 retarget**：`server/src/combat/tuike.rs`（v1 取 CHEST worn.last()）+ `tuike_v2/{tick.rs,skills.rs}`（扫 CHEST worn 层）+ `resolve.rs` 读点；FALSE_SKIN 专槽全移除。
- **P3 防御加性 + 负重 + qi 守恒**：`server/src/combat/armor_sync.rs`（`.max()`→Σ_worn 加性 clamp 0.85）+ `body_mass.rs`（equipped_armor_mass filter ArmorProfile）+ `qi_physics/ledger.rs`（inventory_qi flat_map(worn).chain(held) 守恒 pin）。
- **P4 client 面板 + 法宝触发位**：`client/.../inventory/component/{EquipmentPanel,EquipSlotComponent}.java`（头/胸/腿/足中列+左右手+多臂 + worn 栈叠层渲染/栈顶高亮/下层 dim 0.40/双手灰显）+ `InspectScreen.java`（删行囊 tab/哨兵 + body_pocket 默认首 tab + attemptDrop 不 swap + LIFO）+ `InventoryEquipRules.java`（SlotContents 镜像）+ `InventoryModel`/`InventorySnapshotHandler`/`EquipSlotType`；**法宝触发位**：`server/src/inventory/mod.rs`（`triggered_treasures` + `TREASURE_TRIGGER_CAP=4` + `apply_treasure_activate`）+ `spirit_treasure.rs`（passive_active 读触发位）+ `client_request_handler.rs`（`handle_treasure_activate`）+ proto/agent C2S `TreasureActivate` + `SpiritTreasureScreen`（右键激活 + 飞入 12tick + toast）。
- **P5 容量升级 hook**：`server/src/inventory/mod.rs`（`worn_cap_bonus`/`treasure_trigger_cap` 占位扩展点，默认 0/常量；升级源待 worldview 锚点）。

### 关键 commit（squash，2026-06-25~26）
- `8cf341edd` P0 模型+wire+迁移 (#736) · `f1d7d7779` P1+P2 校验/蜕壳饱和测试 (#738) · `8ad22d7d1` P3 防御加性/负重/守恒 (#740) · `9310bb47e` P4 client 面板重构 (#742) · `568925c22` 法宝触发位完整链路 (#745) · `8f254b9c6` P5 升级 hook (#746) · worldview 锚点 (#727) · plan 转 active (#728)。

### 测试结果
- server `cargo fmt --check` + `clippy --all-targets -D warnings` 全绿 + `cargo test` 9871+ passed / 0 failed（各 PR 累加 P0-P5 pin）。
- agent `npm test -w @bong/schema` 717→721 passed（equip-worn/held sample 对拍 + 修 server↔agent 漂移）。
- client `gradlew compileJava test`(Java 17) 2957→2969 passed。
- e2e（全栈 smoke）每 PR pass；P5 末轮发现并修「并行 PR 合并产生重复 `triggered_treasures` 字段(E0062)」merge 产物。

### 跨仓库核验
- server：`SlotContents`/`EquipSlotV1`/`triggered_treasures`/`worn_cap_bonus`/`apply_treasure_activate`。
- agent：`EquippedInventorySnapshotV1`(inventory.ts 方案B)/`TreasureActivateRequestV1`。
- client：`EquipSlotType`(删 9 增 extra_hand)/`SlotContents`/`EQUIP_SLOT_BY_WIRE_NAME`/`EquipLoc.state`。

### 遗留 / 后续（不在本 plan，需独立推进）
1. **P5 升级源**：`worn_cap_bonus`/触发位容量目前恒返回占位值；实际"境界/功法/法宝 → +cap"的升级机制需先补 worldview 锚点（决议 #24）再接线。
2. **v2 多层伤害吸收**：`tuike_v2::StackedFalseSkins.damage_capacity` 仍 dead_code，未接 `resolve` 战斗结算——归 tuike_v2 自身 plan（本 plan 只 retarget 读取来源）。
3. **触发位 UI 细化**：触发位法宝以文字标签渲染（未接 item 贴图）；worn 栈 hover 分层 tooltip（"栈顶可卸/下层被压"）未做——纯视觉增强，可后续。
