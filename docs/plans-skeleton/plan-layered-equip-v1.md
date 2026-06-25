# plan-layered-equip-v1 — 分层/叠加装备槽（穿戴层 + 手持态）

> 装备槽从「单槽单件」改为「单槽多件分层」：每槽 = `worn`(穿戴层，Vec) + `held`(手持，Option)。伪皮归胸槽 worn 层（蜕壳流读取点从 `EQUIP_SLOT_FALSE_SKIN` 专槽改为扫 CHEST worn 层）；双手武器（spear/staff）放一手 held → 锁对侧手；法宝从装备面板的 `treasure_belt` 槽移除，激活态改由灵宝 UI 内新建的「触发位」承载。混装各自生效（加性）。wire 表示 = 方案B（每槽 worn repeated + held optional，不污染 InventoryItemView）。源于用户真机测试反馈，与已合的背包 tab UX（PR #714）解耦。

## 阶段总览

| 阶段 | 内容 | 状态 | 验收日期 |
|------|------|------|----------|
| P0 | 数据模型 + wire schema（equipped → SlotContents{worn:Vec, held:Option}）+ 存档迁移 | ⬜ | YYYY-MM-DD |
| P1 | 装备校验 + 分层规则（worn cap / held 互斥 / 双手锁对侧手 / 多臂手槽） | ⬜ | YYYY-MM-DD |
| P2 | 蜕壳流 retarget（伪皮归胸槽 worn 层，v2 + v1 全点）+ 移除 FALSE_SKIN 专槽 | ⬜ | YYYY-MM-DD |
| P3 | 效果叠加语义（混装各自生效）+ 负重核验 + equipped 真元 carrier 守恒求和 | ⬜ | YYYY-MM-DD |
| P4 | 客户端面板重构 + 分层渲染 + 法宝激活触发位（灵宝 UI 内） | ⬜ | YYYY-MM-DD |
| P5 | 容量升级 hook（worn_cap 由常量 → 可升级派生值） | ⬜ | YYYY-MM-DD |

> **worldview 前置**：`docs/worldview.md` §545 / §468 / 分层装备锚点的补写，按 docs/CLAUDE.md §6.3 **人工单独 PR 先 land**，不进自动 consume（见 §11 决议 #10 + §10）。

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

### 跨仓库契约（server / agent / client 逐 symbol）
| 契约 | server | agent | client | proto |
|------|--------|-------|--------|-------|
| equipped 快照 | `EquippedInventorySnapshotV1`（schema/inventory.rs:174） | `EquippedInventorySnapshotV1`（inventory.ts:240） | `InventoryModel.equipped` + `InventorySnapshotHandler.EQUIP_SLOT_BY_WIRE_NAME` | `EquippedInventorySnapshot`（envelope.proto:556） |
| 槽枚举 | `EquipSlotV1`（schema/inventory.rs:33；删 6 变体） | `EquipSlotV1`（inventory.ts:21；删 6 字面量） | `EquipSlotType`（EquipSlotType.java） | （字段命名，无独立枚举 message） |
| C2S move 落位 | `InventoryLocationV1::Equip{slot,state}`（schema/inventory.rs:212；新增 state） | `InventoryLocationV1`（inventory.ts:258，加 state） | `EquipSlotType` + drop 落位 | `oneof location`（envelope.proto:2586） |
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

> **前置**：§11 决议门全部收口（worn/held 表示=方案B、C2S move 落位=加 state、背包槽=SlotContents worn_cap=1 退化、存档迁移、ExtraHand/worn_cap 全集）。带开放问题进 P0 = 违反 docs/CLAUDE.md §五。

### P0.1 server 数据模型
- `server/src/inventory/mod.rs`：
  - 新增 `struct SlotContents { worn: Vec<ItemInstance>, held: Option<ItemInstance> }`（serde；空槽序列化为 `{worn:[],held:null}`）。
  - 新增 `enum EquipState { Worn, Held }`（serde rename `worn`/`held`）。
  - `PlayerInventory.equipped: HashMap<String, ItemInstance>` → `HashMap<String, SlotContents>`。
  - 新增 `fn worn_cap(slot: &str) -> u8`：head/feet/main_hand/off_hand/extra_hand_0/extra_hand_1 = 2；chest/legs = 3；背包槽（back_pack/waist_pouch/chest_satchel）= 1（退化，见决议 #5）。
  - 新增 `fn classify_equip_state(item: &ItemInstance, registry) -> EquipState`：`ItemCategory::Weapon|Tool` → Held；`Armor|Treasure|Shield`（mod.rs:286 各变体）+ 伪皮物品 → Worn；`Container`（背包）→ Worn（退化槽 worn[0]）。规则见决议 #16。
  - 新增 `fn weapon_two_handed(kind: WeaponKind) -> bool`（或 `WeaponSpec.is_two_handed`，决议 #7）：`Spear|Staff` 派生（先例 mod.rs:4231）。
- **删除常量 + 枚举变体**：`EQUIP_SLOT_FALSE_SKIN`(mod.rs:79)、`EQUIP_SLOT_TWO_HAND`(:82)、`EQUIP_SLOT_TREASURE_BELT_0..3`(:83-86)；`EquipSlotV1::FalseSkin|TwoHand|TreasureBelt0..3`。
- **EquipSlotV1 删 6 变体的全 match 站点**（grep 实测，逐一改）：
  - `equip_slot_to_wire`（mod.rs:4664-4671 区间）删 FalseSkin/TwoHand/TreasureBelt 分支
  - `wire_to_slot`（mod.rs:4525-4539）删对应反向分支
  - `validate_move_semantics` TwoHand 分支（mod.rs:4224-4258）+ FalseSkin 分支（:4260-4268）+ TreasureBelt 分支（:4247-4259）整段移除/改写（详 P1）
  - 旧存档含已删变体的 serde 反序列化 fallback（见 P0.4 迁移）

### P0.2 equipped 160+ 触点按语义分桶（实测 256 处 `.equipped` 跨 ~49 文件，mod.rs 占大头）
> 分层后取值语义随桶而定。每桶在对应阶段逐文件改；P0 锁定分桶规则，后续阶段按桶 retarget。

| 桶 | 语义 | Vec 化后取值 | 代表触点 | 改在哪阶段 |
|----|------|--------------|----------|-----------|
| ① 取 held 武器 | 拿单件手持武器 | `.get(slot).and_then(\|s\| s.held.as_ref())` | combat/weapon.rs、combat/resolve.rs、combat/jiemai.rs、weapon_equipped_emit.rs | P1 |
| ② 遍历 worn 累加 | armor 防御 / 属性派生 / 负重 / 真元求和 | `.worn.iter()` 全件 + `.held` | combat/components.rs(DerivedAttrs)、calculate_current_weight(mod.rs:3462)、ledger.rs:571 | P3 |
| ③ 按 slot 查单件 | 特定槽特定件 | `.worn[0]`（退化槽）/ 扫 worn 找特定件 | forge/artifact_meridian.rs、combat/armor_sync.rs、tuike(_v2)、compute_max_weight(mod.rs:3489) | P0(背包)/P2(tuike) |
| ④ 迭代全件 | 全 equipped 物品扫一遍 | `.values().flat_map(\|s\| s.worn.iter().chain(s.held.iter()))` | shelflife/sweep.rs、world/block_drop.rs、world/block_place.rs、durability、tsy_death_drop.rs、clearinv/reset | P3(qi/sweep) + 各阶段顺手 |

- `equipped.remove(slot)` 全站点逐个明确「移整槽全件 vs 指定件」语义：mod.rs:3095（unequip→rehome）、:3354、:3690（背包卸下，整槽 .worn[0]）、以及其余 remove 点。
- `move_equipped_item_to_first_container_slot`（mod.rs:2991-3010）重写：find 到槽后在 `worn` Vec 用 `.position(by instance_id)` 或 `held` 精确定位移除单件，而非整槽 `.get/.remove`（决议 #12）。

### P0.3 wire schema（方案B，三端同构）
- **proto** `proto/bong/envelope.proto`：
  - `EquippedInventorySnapshot`（:556-573）17 个 `optional InventoryItemView` 字段 → 每个装备槽拆 `repeated InventoryItemView <slot>_worn` + `optional InventoryItemView <slot>_held`（装备槽：head/chest/legs/feet/main_hand/off_hand/extra_hand_0/extra_hand_1）。删 `false_skin`(:561)/`two_hand`(:564)/`treasure_belt_0..3`(:565-568) 字段。背包槽（back_pack/waist_pouch/chest_satchel）保留 `optional`（退化，单件）或 `repeated <slot>_worn` 容量恒 1——决议 #5 定 `repeated` 统一形态、读取处 `worn[0]`。
  - `WeaponEquipped`（:1587）`slot` 注释（:1588 `main_hand / off_hand / two_hand`）删 `two_hand`（决议 #25，server→client 推送非 C2S）。
  - C2S move：`oneof location`（:2586）的 equip 变体加 `EquipState state`（enum 或 string `worn`/`held`）。
- **Rust** `server/src/schema/inventory.rs`：
  - `EquippedInventorySnapshotV1`（:174）每装备槽字段 `Option<InventoryItemViewV1>` → `worn: Vec<InventoryItemViewV1>` + `held: Option<InventoryItemViewV1>`；删 false_skin/two_hand/treasure_belt 字段。
  - `EquipSlotV1`（:33）删 6 变体。
  - `InventoryLocationV1::Equip{slot}`（:212-213）→ `Equip{slot, state: EquipStateV1}`；`RawInventoryLocationV1::Equip`（:372/:380）+ `TryFrom`（:353）同步加 state（含 default 兼容旧 sample？—决议 #2 定 state 必填，旧 sample 一并改）。
  - 新增 `enum EquipStateV1 { Worn, Held }`（serde rename）。
- `schema/proto_convert.rs`：`equipped_to_proto` / 反向映射重写为 worn(repeated)+held(optional)；move location 映射加 state。
- `schema/proto_gen.rs`：随 proto 重生成（`equipped` touchpoint 在此文件）。

### P0.4 agent TS schema（IPC source of truth，**漏改即双端校验红**）
- `agent/packages/schema/src/inventory.ts`：
  - `EquippedInventorySnapshotV1`（:240，`additionalProperties:false`）：每装备槽 `NullableInventoryItemViewV1` → `Type.Array(InventoryItemViewV1)`（worn）+ `NullableInventoryItemViewV1`（held）；删 `false_skin`(:246)/`two_hand`(:249)/`treasure_belt_0..3`(:250-253)。背包槽 worn 容量 1。
  - `EquipSlotV1`（:21）删 `false_skin`(:26)/`two_hand`(:29)/`treasure_belt_0..3`(:30-33) 字面量。
  - `InventoryLocationV1`（:258）equip 变体加 `state` 字段（:271 区域）。
- `npm run build -w @bong/schema` 重 export JSON Schema → `cd agent/packages/schema && npm test` 对拍绿（否则双端校验即红）。

### P0.5 双端 sample（实为 4 份 + 新增 move-intent equip 变体）
- 重写 `agent/packages/schema/samples/inventory-snapshot.sample.json` + `server-data.inventory-snapshot.sample.json` 的 equipped block 为 worn/held（**含 chest 槽叠 2 worn + 1 held 非空样例** + main_hand held 武器样例）。
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
    - 背包槽（back_pack/waist_pouch/chest_satchel/extra_hand_0/1）→ `worn[0]` 单件（**不误塞多件**，extra_hand 武器落 held）
- `LoadoutSpec.equipped`（mod.rs:391）+ `instantiate_loadout`（mod.rs:987-988）+ `load_default_loadout`（mod.rs:1199）+ `server/assets/inventory/loadouts/default.toml`（**TOML 不是 JSON**）：模板单件落 `worn[0]`、武器落 `held`；TOML 结构同步迁移（旧 `[equipped] slot=item` → 新分层表）。

### P0 测试
- `inventory::*` SlotContents 序列化/反序列化 roundtrip（空槽 / 单 worn / 多 worn / worn+held / 仅 held）。
- `worn_cap` 各槽边界（head=2 / chest=3 / legs=3 / feet=2 / extra_hand=2 / 背包=1）。
- schema sample 正反对拍（4 份 + move-intent equip-worn/equip-held）。
- 旧格式 fixture 升级测试：`migrate_equipped_v1_to_v2` 对每类旧槽（false_skin→chest worn / two_hand→main_hand held / treasure_belt→触发位 / 背包→worn[0]）逐一断言去向。
- `load_player_inventory_from_sqlite` schema_version 分流（旧版本触发 migrate、新版本直读）。
- default.toml 加载后结构与 PlayerInventory 一致（instantiate_loadout 单件落 worn[0]、武器落 held）。
- `agent/packages/schema` npm test 双端对拍绿。

---

## P1 — 装备校验 + 分层规则

- `server/src/inventory/mod.rs` `validate_move_semantics`（mod.rs:4151-4321）Equip 分支按 SlotContents 重写：
  - **worn 层**（决议 #15 拒绝不顶替）：目标槽 `worn.len() < worn_cap(slot)` 才接受；满 = 拒绝带文案「该部位已穿戴 {cap} 层，无法再叠加」。
  - **held**（决议 #15 拒绝不卸）：目标槽 `held.is_some()` = 拒绝带文案「该手已持械，请先卸下」。held 不计 worn cap。
  - **手持槽全集**（决议 #6）：`main_hand / off_hand / extra_hand_0 / extra_hand_1` 都是 held 槽（每槽 ≤1）。现 `ExtraHand0|ExtraHand1` 走与 MainHand 同分支（mod.rs:4307-4319），改为 held 语义。
  - **双手武器**（决议 #7）：`weapon_two_handed(kind)` 为真且放进 main_hand/off_hand 任一 held → 标记对侧主/副手 lock（**extra_hand 独立不锁**）；拖入被锁手 = 拒绝带文案「双手兵器占用双手，对侧已锁定」。迁移现 from_two_hand swap 路径（mod.rs:4163-4242，TwoHand 专槽 + from_two_hand 旗标）到「一手 held + 对侧 lock」模型；删 `EquipSlotV1::TwoHand` 校验段（:4224-4258）。
  - 武器槽判定（mod.rs:3287-3306，现 match `MAIN_HAND|OFF_HAND|TWO_HAND`）删 TWO_HAND，加 extra_hand。
  - `compute_swap_*` / `attach_at_location` 改为向 `worn` Vec 追加（满则拒）/ 写 `held`（占则拒），而非整槽覆盖。
- **lock 态承载**：lock 是派生态（双手 held 武器 → 对侧手 disable），不落存档独立字段；load 后由状态机从 held 武器双手属性重算（迁移期同此，见 P0.6）。
- **背包/暗格槽**（决议 #5）：`BackPack|WaistPouch|ChestSatchel` worn_cap=1 退化，校验「worn 已有则拒」；`compute_max_weight`（mod.rs:3489，:3497 `equipped.get(slot)`）特判背包槽读 `worn[0]` 算 max_weight。

### P1 测试（按 state transition 枚举）
- worn cap：head/feet/extra_hand 满 2 拒、chest/legs 满 3 拒、背包满 1 拒。
- held 互斥：手槽已 held → 拒（不 swap）。
- 双手武器：放 main_hand → off_hand lock、拖入 off_hand 拒；extra_hand 不受锁；spear/staff 各一例；非双手武器（剑/斧）不锁对侧。
- worn+held 共存：同槽 worn 满 + held 一件并存合法。
- 卸下后可装：held 卸下 → 同手可装新 held；worn 移除一层 → 可再叠。
- 拒绝文案：三类拒绝（cap 满 / held 占 / 锁手）各自命中特定 err 文案。
- `move_equipped_item_to_first_container_slot`（mod.rs:2991）按 instance_id 在 worn/held 精确定位移单件，不误移整槽。

---

## P2 — 蜕壳流 retarget + 移除 FALSE_SKIN 专槽

> **关键**：活的蜕壳流是 `tuike_v2/`（StackedFalseSkins 多层 / 3 招共用 / maintenance / residue），v1 `tuike.rs` 仍注册（combat/mod.rs:368）。两套都要 retarget。

### v2 retarget（combat/mod.rs:244 注册，当前 gameplay 真消费）
- `tuike_v2/tick.rs`:8 import + `:47` `sync_false_skin_stack_from_inventory` 现 `inventory.equipped.get(EQUIP_SLOT_FALSE_SKIN)` → 扫 CHEST 槽 `worn` 层里的伪皮物品（`false_skin_kind_for_item`，tuike.rs:120 判定）。多件伪皮 → 多层 StackedFalseSkins。
- `tuike_v2/skills.rs`:11 import + `:356` `equipped_false_skin`（don/shed/transfer_taint 三招共用入口，:66 调用）现 `:361` `inventory.equipped.get(EQUIP_SLOT_FALSE_SKIN)` → 扫 CHEST worn。
- `tuike_v2/mod.rs`:30-31 系统注册名不变（sync + maintenance），只改读取来源。
- `false_skin_state_emit.rs` 读 `StackedFalseSkins` 组件（**非直接 slot**），retarget 后组件来源不受影响——**确认这一点**（组件由 sync 写，sync 已 retarget）。
- `tuike_v2/tests.rs`（:79/:631/:640 用 EQUIP_SLOT_FALSE_SKIN）随改为 CHEST worn 装伪皮。

### v1 retarget（combat/mod.rs:368 注册）
- `tuike.rs`:17 import + `sync_false_skin_from_inventory`（combat/mod.rs:368 调用）读点从 `equipped.get(EQUIP_SLOT_FALSE_SKIN)` → 扫 CHEST worn。蜕层（shed）从 CHEST worn 移除一件伪皮，emit `ShedEvent` 不变（tuike.rs:131 FalseSkin 组件 / :152 state_payload 不变）。
- `combat/resolve.rs`:56 import + FalseSkin/StackedFalseSkins 双 query + TWO_HAND wire 映射（:744 区域）+ v1 depleted 链 + `equipped.get(EQUIP_SLOT_FALSE_SKIN)` 读点全 retarget（按 grep 实测 resolve.rs FalseSkin/TwoHand 站点逐一）。
- `inventory_snapshot_emit.rs` false_skin field（snapshot 产出端）随 schema 删字段同步。

### 移除 FALSE_SKIN 专槽
- 删 `EQUIP_SLOT_FALSE_SKIN` 常量（已在 P0.1 删）、`EquipSlotV1::FalseSkin`、客户端 `FALSE_SKIN`（EquipSlotType.java，P4）。
- `equip_false_skin` dead C2S 清理：proto `EquipFalseSkin`（envelope.proto:258）+ 对应 handler（确认无其他消费后删；若仍被引用则降级为 no-op 并标 deprecated）。

### P2 测试
- 伪皮装进 CHEST worn → `FalseSkin`/`StackedFalseSkins` 组件生成 + tuike 层数正确。
- 多件伪皮分层吸收（v2 StackedFalseSkins 多层）。
- 蜕层（shed）移除一件 CHEST worn 伪皮 + emit ShedEvent。
- 三招（don/shed/transfer_taint）走 CHEST worn 读取入口。
- v1 sync_false_skin_from_inventory CHEST worn 读取一致。
- false_skin_state_emit 组件来源不受 retarget 影响。

---

## P3 — 效果叠加语义 + 负重 + equipped 真元 carrier 守恒

### 效果叠加（混装各自生效，加性）
- `combat/components.rs` `DerivedAttrs` 等防御/属性派生：从「按 slot 取单件」改为遍历槽内 `worn` 全件累加（armor 防御加性、手套等效果各自加性，**不取最高**）。held 武器属性按原语义。
- 桶② 触点逐一改 `.worn.iter()` 累加 + `.held`。

### 负重（决议 #24）
- `calculate_current_weight`（mod.rs:3462，现 `equipped.values()...×stack_count`）改遍历每槽 `worn` 全件 + `held`；确认穿戴 + 手持全部计入（与旧行为对齐：旧 equipped.values() 含武器槽，手持武器仍计重）。

### equipped 真元 carrier 守恒（qi_physics 锚点）
- `qi_physics::ledger::inventory_qi`（ledger.rs:564，:571 `inventory.equipped.values().map(item_qi).sum()`）→ `inventory.equipped.values().flat_map(\|s\| s.worn.iter().chain(s.held.iter())).map(item_qi).sum()`。
- 桶④ 同形真元广播 / 衰减点逐一 chain worn+held：`shelflife/sweep.rs`（values_mut 衰减）、`world/block_drop.rs` / `world/block_place.rs`（真元广播 chain(equipped.values())）。
- tuike_v2 `false_skin_maintenance_tick` 多层维持成本走既有 `release_qi_amount_to_zone` / `QiTransfer`（守恒，不新增公式）；多层 = 成本累加，物理不变（头部锚点已声明）。

### P3 测试
- 两件同类 armor → 防御加性翻倍（不取最高）。
- 混装（armor + 手套）各自生效。
- 负重含全部 worn 层 + held 武器。
- **守恒 pin**：同槽两件 carrier（各带真元）的 `inventory_qi` = 两件 item_qi 之和（决议头部 qi_physics 锚点）。
- shelflife sweep 衰减遍历到全部 worn+held（无遗漏件不衰减）。

---

## P4 — 客户端面板重构 + 分层渲染 + 法宝激活触发位

### 数据模型（client）
- `client/.../inventory/model/InventoryModel.java`：`equipped` 字段 `Map<EquipSlotType, InventoryItem>` → `Map<EquipSlotType, SlotContents>`（SlotContents = `{List<InventoryItem> worn, InventoryItem held}`，镜像 server）；`Builder.equip(slot, item)` API、`equipped()` getter、`parseEquipped` 返回类型随改。
- `client/.../inventory/model/EquipSlotType.java`（EquipSlotType.java:8-15 区域）：删 `FALSE_SKIN`/`TWO_HAND`/`TREASURE_BELT_0..3`；保留背包三槽（back_pack/waist_pouch/chest_satchel）+ 新增/保留 extra_hand_0/1。
- `client/.../network/InventorySnapshotHandler.java`：`EQUIP_SLOT_BY_WIRE_NAME` 删 6 wire name；解析改读 `<slot>_worn`(list) + `<slot>_held`(optional)。
- `InventoryEquipRules.canEquip`：镜像 P1 worn cap + held 互斥 + 双手锁对侧手。

### 面板布局 + 分层渲染（视听精度内联，避免 owo fill 顶飞）
- `client/.../inventory/component/EquipmentPanel.java`：新布局——**中列一线** HEAD/CHEST/LEGS/FEET 纵向排列；OFF_HAND（左侧）/ MAIN_HAND（右侧）两侧对称；extra_hand_0/1 在主副手下方（多臂可见时）。删 TWO_HAND/FALSE_SKIN/TREASURE_BELT 槽位。
- **owo 布局约束**：中列用 `Containers.verticalFlow` + 各槽 `Sizing.fixed(18)`（标准 slot 18px），**严禁 `Sizing.fill(100)`**（会占满父容器整宽把左右手两侧节点顶出边界消失——参 memory `feedback_owo_fill_overflow.md`）。左右手两侧用绝对定位 / `Positioning.relative` 锚到中列两边。
- `EquipSlotComponent` 多层渲染：
  - **worn 多层叠放**：每层向右下偏移 **+2px / +2px**（worn[0] 底、worn[n] 顶，z-order 递增），最多渲染 cap 层（chest 3 / head 2…）。
  - **层数角标**：右上角，位置相对 slot `(x+14, y-2)`，颜色 `#FFD27F`（淡金），字号 **7px**（vanilla small font scale 0.7），文案 `×N`（N≥2 才显示）。
  - **held 单独位**：held 物品渲染在 slot 右下角小图标位 `(x+9, y+9)`，叠在 worn 之上，**8px** 缩略图 + 武器图标边框区分。
  - **hover**：悬停某层显示 tooltip（该件名称 + worn/held 标记）；多层 hover 顶层优先。
  - **双手另一手 disable**：被锁的对侧手槽叠 tint `#000000` opacity **0.55** + 灰色锁图标（中心 `(x+5,y+5)` 8px）；hover 显示「双手兵器占用」文案，不可拖入。
- drop 收口（`InspectScreen` TAB_EQUIP 分支）：拖入装备槽时按 classify（worn/held）+ cap 落位；满则飘红边 + 退回原位，**不静默吞物**。

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
- `EquipmentPanelTest` / `BackpackEquipSlotTest`：中列一线 + 左右手布局；worn 多层叠放渲染（叠 2/3 层）；层数角标 N≥2 显示；held 单独位；双手另一手 disable tint。
- wire 解析多件（`<slot>_worn` list + `<slot>_held`）。
- drop 落位：worn 满拒退回、held 占拒、锁手拒。
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
**决议**：(1) `load_player_inventory_from_sqlite`（state.rs:1090，现 :1095 SELECT 仅 inventory_json、:1115 裸 from_str）改先 SELECT schema_version 分流；(2) 写 `migrate_equipped_v1_to_v2(&mut Value)`：每槽 object → `{worn:[..],held:..}`（按 classify）；(3) `const INVENTORY_SCHEMA_VERSION` bump（现值 +1，save 路径 state.rs:704+ 已写 schema_version 列）；(4) 旧槽映射去向定死——false_skin→CHEST worn / two_hand→main_hand held(对侧 lock load 后重算) / treasure_belt_0..3→触发位激活承载 / 背包+extra_hand 槽→worn[0] 单件（不误塞）；(5) `LoadoutSpec.equipped`（mod.rs:391）+ instantiate_loadout（:987）+ `server/assets/inventory/loadouts/default.toml`（**TOML**）同步迁移。
**落点**：state.rs:1090-1115 / mod.rs:391,987,1199 / default.toml。plan §P0.6。

### #5 背包/多臂槽统一 SlotContents（worn_cap=1 退化）
**决议**：背包槽（back_pack/waist_pouch/chest_satchel）统一用 SlotContents，但 `worn_cap=1` 退化、`held=None`；读取处全改 `.worn[0]`（替代旧 `equipped.get(slot)`）。`compute_max_weight`（mod.rs:3489，:3497）特判背包槽读 worn[0] 算 max_weight。**不拆独立字段**（统一结构减少分支）。
**落点**：mod.rs:91-103（背包/extra_hand 常量）/ :3489-3497（compute_max_weight）/ :3690（背包卸下 remove）/ :9969 区域（insert 同一 equipped）。plan §P0.1 / §P0.2 / §P1。

### #6 手持槽全集
**决议**：`main_hand / off_hand / extra_hand_0 / extra_hand_1` 都是 held 槽（每槽 ≤1，不计 worn cap）。双手武器锁定语义 = **仅锁对侧主/副手**（main_hand 双手锁 off_hand，反之亦然）；**extra_hand 独立不锁**（多臂变异本就额外手）。worn_cap 设计表补全：head/feet/main_hand/off_hand/extra_hand_0/extra_hand_1 = 2；chest/legs = 3；背包 = 1。
**落点**：mod.rs:4307-4319（ExtraHand0/1 现走 MainHand 同分支）/ worn_cap 表（§P0.1）。plan §P1。

### #7 spear/staff 双手派生
**决议**：从 `WeaponKind` 派生（加 `fn weapon_two_handed(kind) -> bool` matches `Spear|Staff`，先例 mod.rs:4231 现 TWO_HAND 校验已 match Spear|Staff）。归属 `inventory/mod.rs`（或 `WeaponSpec.is_two_handed` 字段，mod.rs:245 现无）。迁移现 from_two_hand swap 路径（mod.rs:4163-4242）到「一手 held + 对侧 lock」模型。
**落点**：mod.rs:245（WeaponSpec）/ :4231（先例）/ :4163-4242（from_two_hand 迁移）。plan §P1。

### #8 法宝激活 = 灵宝 UI 内「触发位」
**决议**：(1) 新建「触发位」承载（`triggered_treasures: Vec<ItemInstance>` 或 `ActivatedTreasures` store，容量默认 4 = 旧 belt 数，可后续配置——P4 子细节，P5 可升级）；(2) `scan_inventory_for_spirit_treasures`（spirit_treasure.rs:231，:238 遍历 equipped.values()）改读触发位、`push_entry_for_item`（:358，:372 passive_active=equipped）改 `passive_active=in_trigger_slot`；(3) `treasure_equipped_emit.rs:57`（slots=[off_hand,belt_0..3]）/ client `TreasurePanelSync.TREASURE_SLOTS` / `WeaponHotbarHudPlanner.get('off_hand')` 从触发位产 payload；(4) **新建「法宝激活」C2S**（当前不存在，SpiritTreasureScreen 只切 tab）= 右键灵宝 item context 动作 → 移入触发位（IntentHandler 模式，非 vanilla hack）；(5) persistence 随 PlayerInventory 入库。触发位在**灵宝 UI / SpiritTreasureScreen 内，不在装备面板**。
**落点**：spirit_treasure.rs:231-249,358-372 / treasure_equipped_emit.rs:57 / client SpiritTreasureScreen.java + TreasurePanelSync.java + WeaponHotbarHudPlanner.java。plan §P4。

### #9 蜕壳流 retarget（v2 是活的）
**决议**：活的是 `tuike_v2/`（tick.rs:47 sync_false_skin_stack_from_inventory、skills.rs:361 equipped_false_skin 三招共用），从 `equipped.get(EQUIP_SLOT_FALSE_SKIN)` 改为扫 CHEST worn 层。v1 `tuike.rs`（combat/mod.rs:368 注册仍活）+ resolve.rs FalseSkin 读点一并 retarget。`false_skin_state_emit` 读 `StackedFalseSkins` 组件（非直接 slot，由 sync 写），retarget sync 来源后**组件来源不受影响**（已确认）。移除 `EQUIP_SLOT_FALSE_SKIN` 常量 + `EquipSlotV1::FalseSkin`。**v1/v2 都保留注册，都 retarget**（不本 plan 内删 v1）。
**落点**：tuike_v2/tick.rs:8,47 / tuike_v2/skills.rs:11,356,361 / tuike.rs:17 / combat/mod.rs:244(v2),368(v1) / resolve.rs:56 FalseSkin/TwoHand 站点 / false_skin_state_emit.rs。plan §P2。

### #10 worldview 补写（人工单独 PR 先 land）
**决议**：(1) §545（装备 loadout 段，worldview.md:545 现「装备 loadout（武器 / 护甲 / 伪皮 / 暗器载体）」）补『伪皮作胸部穿戴层，与真甲混穿各自生效』；(2) §468（worldview.md:468 主战斗变量表「伪皮档位 轻/中/重 + 单层吸收上限」）补档位注（多层叠加上限引 §11.1 #6 worn_cap）；(3) 在 §545 段内立「修士可层叠穿戴/分层装备」锚点（否则属无 worldview 锚点玩法，§四红旗）。**人工单独 PR 先 land**（docs/CLAUDE.md §6.3，CLAUDE.md/worldview.md 严禁 agent 自动改）；plan 写死节号供执行。
**落点**：docs/worldview.md:545 / :468 / §439（蜕壳流出处不改，仅引用）。plan §10 worldview PR。

### #16 Treasure/Shield/Container worn/held 分类
**决议**：`ItemCategory::Weapon|Tool` → Held（不计 worn cap）；`Armor|Treasure|Shield`（mod.rs:286 各变体）+ 伪皮物品 → Worn（计 worn cap）；`Container`（背包）→ Worn 退化槽（worn_cap=1）。Treasure 物品作为装备件归 Worn（计 cap）；其「激活态」另由触发位承载（#8），二者正交——装备槽里的 treasure 仍是 worn 件，激活才进触发位。
**落点**：`classify_equip_state`（mod.rs，§P0.1）；ItemCategory（mod.rs:286）；validate_move_semantics OffHand 接受 Treasure（mod.rs:4173）/ Shield（:4184）。plan §P0.1 / §P1。

---

## §10 实施工作流

### §10.1 多 PR 序列化拆分（依赖顺序，前一个 merge 后开下一个）

> scope = 5 PR + 1 人工 worldview PR。单 plan 内序列化，不拆多 plan（docs/CLAUDE.md §6.3）。

| PR | 范围 | 阶段 | 依赖 |
|----|------|------|------|
| **worldview PR**（人工） | §545/§468 补写 + 分层装备锚点 | §11.1 #10 | 无（**最先 land**） |
| **PR-1** | 数据模型 SlotContents/EquipState + wire schema（proto/Rust/agent-TS）+ 4 sample + 存档迁移 | P0 | worldview PR |
| **PR-2** | 装备校验分层规则 + 蜕壳流 retarget（v1+v2）+ 移除 FALSE_SKIN 专槽 | P1 + P2 | PR-1 |
| **PR-3** | 效果叠加 + 负重 + equipped 真元 carrier 守恒求和 | P3 | PR-2 |
| **PR-4** | client 面板重构 + 分层渲染 + 法宝激活触发位（含 §6.1 三轮打磨 + PROMISE） | P4 | PR-3 |
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
