# plan-container-filter-and-completion-v1 — 容器品类筛选 + 12 僵尸容器使用闭环补全

> **来源**：手搓 104 产出物僵尸审计「容器全死（12）」一类。给容器加品类筛选 + 把 12 个 `category=misc` 的占位容器全部补成可用。
> **依赖**：[[plan-nested-pack-base-v1]]（提供 `ItemInstance.sub_container` 子容器机制 + `PackContainerOpen/Move/Close` 协议 + `SubContainerPanel` 浮窗 + `container_id = "pack_{instance_id}"` 命名规则）。**该 plan 全部 merge 到 main 后才开本 plan**——本 plan P3/P4 直接挂在它的随身子包与 client 面板上。可放置容器（`trade_crate` / `herb_crate` 放置版 / `dead_drop_box`）→ [[plan-placeable-container-blocks-v1]]。
> **状态**：骨架（草案）。筛选架构已定（新增 `ItemCategory` 变体），保鲜接现成 shelflife。§8 已凭实地证据定案三条，仅 `moisture_guard` SpoilOnly 精确 rate（§8 #2）悬留待 §8.1 收口。

给容器加 `accept_filter` 品类筛选维度（只收矿石 / 只收草药等），把 12 个僵尸容器全部补成有真实使用闭环：随身子包接套包系统、保鲜类接 `ContainerFreshnessBehavior`、经济/情报类定行为。让每个容器都不再是孤岛。

## 阶段总览

| 阶段 | 内容 | 状态 | 验收日期 |
|------|------|------|----------|
| P0 | 新增 `ItemCategory::Mineral/Anqi/Liquid` 变体 + `ContainerSpec.accept_filter` 数据模型 + `ContainerAcceptFilter` 枚举 | ⬜ | |
| P1 | 筛选执行：`validate_attach_fits` 白名单校验 + 套包子容器（`pack_` 前缀）filter 校验 | ⬜ | |
| P2 | 保鲜行为映射：密封药瓶 / 防潮包 / 封灵匣 接 `ContainerFreshnessBehavior`（扩 `item_freshness_behavior`） | ⬜ | |
| P3 | anqi 物品 category 迁移 + 随身子包闭环 + filter 落地到全部随身容器 | ⬜ | |
| P4 | client filter 元数据同步（`LootContainerOpenV1.accept_filter`）+ 灰显非法拖拽 + 12 容器逐个验收 | ⬜ | |

## 接入面（防孤岛）

- **进料**：
  - [[plan-nested-pack-base-v1]] 的 `ItemInstance.sub_container: Option<ContainerState>`（其 P0）+ `PackContainerOpen/Move/Close` 协议（其 P2）+ `container_id = "pack_{instance_id}"` 命名规则（其 P3，snapshot 递归 emit 的依据）+ `SubContainerPanel.java`（其 P4 浮窗）。
  - `ContainerSpec`（`server/src/inventory/mod.rs:169`，现 5 字段：rows/cols/weight_capacity/equip_slot/durability_cost_per_op，**无 accept_filter**）。
  - `ContainerSpecToml`（`mod.rs:1514`，现 5 字段，**无 accept**）+ `parse_container_spec`（`mod.rs:1532`）。
  - `ItemCategory`（`mod.rs:225`，**现 14 变体**：Pill/Herb/RecipeFragment/RecipeHint/Weapon/Armor/Treasure/BoneCoin/Tool/Scroll/Misc/Block/Container/Food——`BoneCoin` 已是专属 category，coin_box filter 最易）+ `parse_item_category`（`mod.rs:1951`，14 别名分支）。
  - 保鲜走 `server/src/shelflife/types.rs` 的 `ContainerFreshnessBehavior`（Normal/Halve/Freeze/DryingRack/SpoilOnly/AgeAccelerate，6 变体全已落地有单测）+ `shelflife/container.rs:30` `container_storage_multiplier` / `:82` `enter_container` / `:94` `exit_container`（**三者已实装，但 enter/exit 仅在 container.rs 内部单测被调用，全仓无生产路径调用——本 plan P2 接生产路径**）。
  - 容器→行为映射模板 `server/src/spiritwood/mod.rs:627` `item_freshness_behavior`（现仅映射 `ling_xia`→Freeze、`food.container.ice_cellar`→SpoilOnly{0.3}）。生产侧消费者是 `shelflife/sweep.rs:54`（按容器内"行为修改器物品"取 behavior 改本容器全部 item 速率）+ `network/inventory_snapshot_emit.rs:340` `enrich_with_derived_freshness`。
  - `container_id_to_equip_slot`（`mod.rs:3299`，3 槽 back_pack/waist_pouch/chest_satchel 映射，有 pin 测试）——P1 装备容器 spec 反查链路。
  - **`ItemRegistry`**（查 template category / `ContainerSpec.accept_filter`）——`validate_attach_fits` 现签名（`mod.rs:3643`，实地核验）**无 registry 入参**，P1 必须增 `registry: &ItemRegistry` 并改 **3 处调用点**（`mod.rs:2535`/`4126`/`4147`，均实地核验）。
  - **`ContainerState`**（`mod.rs:323`，实地核验仅 `id/name/rows/cols/items` 五字段，**不含 template_id / accept_filter / spec**）——这意味着 filter 元数据**不在 ContainerState 上**，P1 必须运行时经 registry 反查（见下「出料」+ P1 第一交付物）。
- **出料**：
  - 容器筛选拒绝非法 attach → `validate_attach_fits`（`mod.rs:3643`）在 bounds-check 之前返回 `Err`。`validate_attach_fits` 拿到的 `ContainerState` 不携带 filter，须经 registry 反查：装备槽经 `container_id_to_equip_slot`→查 equipped 物品 template→registry 取 `ContainerSpec.accept_filter`；套包容器经 `pack_{instance_id}` 解析→在 inventory 找 `ItemInstance`→registry 取其 template spec。
  - 保鲜容器内物品走 `item_freshness_behavior` → sweep/snapshot 自动套用减缓衰减（**P2 不新增 shelflife 代码**，只扩 spiritwood 映射函数）。
  - 12 容器各有真实 kind/filter/behavior（验收清单见下表）。
  - `LootContainerOpenV1`（`server/src/schema/server_data.rs:611`，**现无 accept_filter 字段**）扩 `accept_filter` → protobuf `.proto` 同步 → client Java DTO 同步 → `SubContainerPanel`/`GridSlotComponent.setHighlightState(INVALID)`（`GridSlotComponent.java:60`，`HighlightState.INVALID` 已实装 `:35`，红 tint `0x33CC2222` `:75`）。
- **共享类型 / event**：`ItemCategory` 加 `Mineral/Anqi/Liquid` 三变体（连带 `parse_item_category` 别名 + **所有 `match category` 穷举处补臂**——`default_max_stack_count_for_category`（`mod.rs:1589`，**无 `_` 兜底，加变体编译强约束补全**）必补；`combat/foreign_qi_resistance.rs:27` 有 `_ =>` 兜底不强改但需确认语义）；`ContainerSpec` 加 `accept_filter: Option<Vec<ContainerAcceptFilter>>`；新增 `ContainerAcceptFilter` 枚举（`ItemCategory` 粗筛 + 可选 `template_prefix: String` 细筛）；**复用 `ContainerFreshnessBehavior` 不另造保鲜机制**（孤岛红旗：同源现象一份实现）。
- **跨仓库契约**：server **只给 `LootContainerOpenV1`（`server_data.rs:611`，实地核验确认无 accept_filter 字段）加 `accept_filter: Vec<ContainerAcceptFilterV1>`**（新增 schema 类型 `ContainerAcceptFilterV1`）。套包子容器打开因 [[plan-nested-pack-base-v1]] P2 **复用 `LootContainerOpenV1` + `LootContainerSourceKindV1::PackItem { instance_id }`**（实地核验该 dep plan L35：套包路径走的就是 `LootContainerOpenV1`，C2S 是 `PackContainerOpen/Move/Close` 三个 client_request **enum 变体**，**不存在 `PackContainerOpenV1` 这个 struct**），故 `accept_filter` 自动覆盖套包路径，**无需第二个 schema**。protobuf `.proto` 同步 → client Java DTO（`ProtoServerDataBridge.java` 解析）→ `SubContainerPanel` 收 filter 后调 `GridSlotComponent.setHighlightState(HighlightState.INVALID)`；**agent 无关**（纯本地背包，无 Redis/IPC 流量）。
- **worldview 锚点**：容器描述里的「隔灵 / 保鲜」呼应 worldview §二「真元极易挥发」（`worldview.md:1517` 离体真元被瞬间分解）——保鲜容器=减缓挥发；封灵匣（spirit_seal_box，Freeze）= 高灵物短存最佳；coin_box（骨币匣）呼应 §九「封灵骨币」（`worldview.md:838`，骨币真元缓慢流失，拒绝囤积疯狂流转）——**coin_box 仅类别筛选，不延缓骨币半衰**（半衰归 §九 货币机制，不在本 plan）；sealed_envelope/dead_drop_box 情报袋呼应 §九「盲盒死信箱」（`worldview.md:850`，dead_drop_box 阵法防砸归 [[plan-placeable-container-blocks-v1]]）。
- **qi_physics 锚点**：保鲜 = 减缓 `spirit_quality`/`freshness` 衰减，**走 shelflife 现有 `container_storage_multiplier`，不新增任何 `*_DECAY*` 常数 / 衰变函数**（孤岛红旗自查：同源现象一份实现，归 shelflife）。本 plan 保鲜是**物品属性衰减**非玩家真元流动，不走 `qi_physics::ledger::QiTransfer`（保鲜不把真元凭空消失也不归还 zone——离体真元衰减语义已由 shelflife profile 定义，本 plan 只换 multiplier 参数）。
- **§808「灵物操作磨损」转移税显式划出本 plan scope**：worldview §九:808（实地核验确认存在）「带灵气值物品被从箱子拿出 / 转移到炉子触发 inventory 操作，灵气纯度/耐久固定扣 1%-5%」是**天道交易税**机制。[[plan-nested-pack-base-v1]] L39 把该交互甩给本 plan「走 shelflife」——但实地核验 shelflife 现有 API（`container_storage_multiplier`/`enter_container`/`exit_container`）**全是时间驱动衰减，无一次性扣减接口**，且该税扣的是 `spirit_quality`，按守恒律红旗扣掉的纯度必须有去向（归还 zone 走 `qi_physics::ledger::QiTransfer`），属 qi_physics 锚点机制——**与本 plan 的「品类筛选 + 保鲜 multiplier」不同源，不在本族 scope**。**本 plan 不实装转移税**，另立 plan（需先扩 qi_physics 定义扣减率常数 + ledger 路径，本 plan 不自拍 1-5% 数值）。已在 reminder.md 同步登记，并待 dep plan 升 active 时从其 L39 删除「归本 plan 走 shelflife」甩锅措辞（两 plan 不可互相指认）。

## P0 — ItemCategory 升变体 + accept_filter 数据模型 ⬜

- `ItemCategory`（`mod.rs:225`）加 `Mineral` / `Anqi` / `Liquid` 三变体。
- `parse_item_category`（`mod.rs:1951`）加别名分支：`"mineral" | "ore" => Mineral`、`"anqi" | "hidden_weapon" => Anqi`、`"liquid" => Liquid`。
- **全仓所有 `match category` 穷举处补臂**（编译器强制完整）：
  - `default_max_stack_count_for_category`（`mod.rs:1589`，**无 `_` 兜底，必补**）：`Mineral => 64`（同 Herb/Block，散料可大堆）、`Anqi => 32`（暗器实物已带 `max_stack_count = 32` 字段，`mod.rs:1180` `template.max_stack_count.max(1)` 优先覆盖默认值，故此处取 32 与现状一致）、`Liquid => 16`（同 Misc）。
  - `combat/foreign_qi_resistance.rs:27` `foreign_qi_resistance_for_use`（有 `_ =>` 兜底，三新变体落入兜底——语义为「非丹药无外来真元抗性」，正确，不强改但需在测试中 pin 确认）。
- `ContainerSpec`（`mod.rs:169`）加 `pub accept_filter: Option<Vec<ContainerAcceptFilter>>`，标 `#[serde(default)]`（保证现有三装备容器序列化不破）。
- `ContainerSpecToml`（`mod.rs:1514`）加 `#[serde(default)] accept: Vec<String>`；`parse_container_spec`（`mod.rs:1532`）解析每条字符串为 `ContainerAcceptFilter`（复用 `parse_item_category` 做 category 粗筛；前缀语法 `"prefix:anqi_"` 解析为 `template_prefix`）。
- 新增 `ContainerAcceptFilter` 枚举（参照 `mod.rs` 现有 spec 模式）：
  ```rust
  pub enum ContainerAcceptFilter {
      Category(ItemCategory),
      TemplatePrefix(String),
  }
  ```
  `accept_filter == None` 或 `Some(vec![])` 语义 = **全收（通用容器）**（§8 #3，唯一合理选项）。`Some(non-empty)` = 物品须命中其中任一 filter 才可放入。
- 新增 `fn item_passes_filter(filter: &Option<Vec<ContainerAcceptFilter>>, item: &ItemInstance, registry: &ItemRegistry) -> bool`：None/empty → true；否则任一 `Category(c)` 命中 `item` 的 template category 或 `TemplatePrefix(p)` 命中 `item.template_id.starts_with(p)` → true。
- **测试（饱和化）**：
  - `parse_item_category` 三新变体正反 + 别名（ore/hidden_weapon）+ 大小写不敏感 + 未知仍 `Err`（happy + 边界 + 错误分支）。
  - 三新变体走 `default_max_stack_count_for_category` 各返回预期堆叠数（穷举每变体一条 pin）。
  - `accept_filter == None` → `item_passes_filter` 全收 pin；`Some(vec![])` → 全收 pin；`Some([Category(Mineral)])` → 矿石 true / 草药 false；`Some([TemplatePrefix("anqi_")])` → `anqi_bone_chip` true / `ore_iron` false；多 filter 取并集（矿石或液体二选一命中即收）。
  - 旧无 `accept` 字段的 TOML round-trip 默认 `accept_filter = None`（向后兼容 pin）。
  - `ContainerSpec` 含 `accept_filter` 的 serde round-trip。

## P1 — 筛选执行 ⬜

- **第一交付物（接线缺口，实地核验确认）：`ContainerState`（`mod.rs:323`）不携带 filter 元数据，filter 必须运行时经 `ItemRegistry` 反查。** 这要求：
  - **`validate_attach_fits`（`mod.rs:3643`）增 `registry: &ItemRegistry` 入参**——现签名 `(&PlayerInventory, &ItemInstance, &InventoryLocationV1)` 无 registry。同步改 **3 处调用点**（`mod.rs:2535`/`4126`/`4147`，实地核验）把 registry 线进去（如自身有递归调用一并改）。这是隐藏接线缺口，不先声明实施时会撞墙。
  - 从 `ContainerState`（无 spec）反查 filter 的**两条解析链**：① 装备容器经 `container_id_to_equip_slot`（`mod.rs:3299`）拿到 equip slot → 在 `inventory.equipped` 找 equipped 物品 → `registry` 取其 template `ContainerSpec.accept_filter`；② 套包容器经 `pack_{instance_id}` 前缀解析 instance_id → 在 `PlayerInventory` 找 `ItemInstance` → `registry` 取其 template `ContainerSpec.accept_filter`。
- `validate_attach_fits` `Container { container_id, .. }` 分支 **bounds-check 之前**插 `accept_filter` 白名单校验：
  - **装备型容器**：走上述解析链 ① → `item_passes_filter(filter, item, registry)` 校验，拒绝返回 `Err("container only accepts ...")`。
  - **套包子容器**（`container_id` 以 `pack_` 前缀）：走上述解析链 ②（依赖 [[plan-nested-pack-base-v1]] P3 的 `pack_` 命名规则）。
  - **MAIN_PACK 主背包**：无 filter（accept_filter = None），保持全收。
- **错误信息带修复线索**：`Err(format!("container '{cid}' only accepts {filter_desc}, but item '{}' is category {:?}", item.template_id, item_category))`，撞红时玩家/测试一眼看出拒绝原因。
- **测试（饱和化）**：
  - **反查链 pin（第一交付物）**：装备容器 `ContainerState`（仅 id/rows/cols/items）经 `container_id_to_equip_slot`→equipped→registry 拿到 `accept_filter` 正确（断言反查到的 filter == TOML 声明）；套包容器经 `pack_{instance_id}`→registry 同；`validate_attach_fits` 收到 `&ItemRegistry` 后 3 处调用点编译通过（签名变更回归 pin）。
  - happy：矿石进 `ore_sack`（Mineral filter）通过；骨币进 `coin_box`（BoneCoin filter）通过。
  - 错误分支：草药进 `ore_sack` 拒绝（`Err`，断言错误信息含 category 名）；普通暗器（已升 Anqi）进 `coin_box` 拒绝。
  - 边界：通用容器（accept=[]/None）全收 pin；filter 校验在 bounds-check 之前（先 filter 拒绝再不报 bounds）。
  - 状态转换：装备容器 filter 与套包子容器 filter 行为一致（同 spec 同结果）；`pack_` 前缀解析 instance_id 失败（不存在 instance）返回 `Err` 而非 panic。
  - 多 filter 容器：命中任一即通过、全不命中拒绝。

## P2 — 保鲜行为映射 ⬜

> **关键接入点**：现有保鲜走 `sweep.rs:54` —— 它扫描**容器内的"行为修改器物品"**（如 ice_cellar / ling_xia 放进容器后修改全容器物品速率）。本 plan 的保鲜容器（sealed_vial/spirit_seal_box/moisture_guard）是**子容器本体**，其内容物应被本体的 behavior 修饰。因此 P2 在 `sweep.rs` 与 `inventory_snapshot_emit.rs` 处理 `pack_{instance_id}` 子容器时，**用子容器本体 template 的 freshness behavior 修饰其内容物**（而非沿用「找容器内修改器物品」逻辑——子容器本体即修改器）。

- 扩 `item_freshness_behavior`（`spiritwood/mod.rs:627`）—— 把容器本体 template_id 映射到 `ContainerFreshnessBehavior`，新增三条 if-let 分支（照现有 `ling_xia_container_behavior` / `ice_cellar_container_behavior` 模式各加一个 `*_container_behavior` 私有函数）：
  - `sealed_vial`（`workbench_materials.toml`「保质期翻倍」）→ `Halve`
  - `spirit_seal_box`（「高灵物短存最佳」）→ `Freeze`（呼应 `inventory_snapshot_emit.rs:1293` 已有 Freeze 用法 + `enter/exit_container` 记账冻结期）
  - `moisture_guard`（「吸湿防霉」）→ `SpoilOnly { rate }`（精确 rate 见 §8 #2，未收口前 P2 阻塞）
- **接 enter/exit 生产路径**（当前 `enter_container`/`exit_container` 是孤岛接口，全仓无生产调用）：在 [[plan-nested-pack-base-v1]] P2 的 `handle_pack_container_move` 提交阶段——物品进 Freeze 子容器（`spirit_seal_box`）时调 `enter_container(freshness, behavior, now_tick)`，移出时调 `exit_container(freshness, now_tick)`。**触发时机定在 move 提交后**（不在 `validate_move_semantics` 校验阶段，避免校验失败仍记账）。非 Freeze 容器（Halve/SpoilOnly）不需 enter/exit（无冻结期记账），仅由 sweep/snapshot 按 behavior 实时算 multiplier。
- **sweep/snapshot 子容器分支**：`sweep.rs` 与 `inventory_snapshot_emit.rs` 遍历 `pack_*` 子容器时，`container_behavior = item_freshness_behavior(Some(子容器本体 instance))`，套用到子容器内 placed items。
- **测试（饱和化）**：
  - happy：物品进 `sealed_vial` 后 freshness 衰减减半（断言 `container_storage_multiplier(Halve, profile) == 0.5` 非 stepwise / 1.0 stepwise，与 `container.rs` 现有语义一致）；进 `spirit_seal_box` 冻结（time-based multiplier=0.0）。
  - 状态转换：`enter_container` 记 `frozen_since_tick`；`exit_container` 累加 `frozen_accumulated`；重复 enter 保持原 since（防时间倒流，container.rs 已有此语义，pin 它在生产路径生效）。
  - 边界：`moisture_guard` SpoilOnly 只作用 Spoil track（Decay/Age track 退 Normal，`container.rs:75` 已有此分流，pin）。
  - 错误分支/默认：未映射的子容器（如 herb_pouch）→ `item_freshness_behavior` 返回 Normal（衰减不变）。
  - exit 后内容物恢复 Normal 衰减速率。

## P3 — anqi category 迁移 + 随身子包闭环 + filter 落地 ⬜

- **anqi 物品 category 迁移**（`projectile_bag` 的 Anqi filter 前置）：`server/assets/items/anqi.toml` 中 12 个暗器实物（`anqi_bone_chip` / `anqi_yibian_shougu` / `anqi_lingmu_arrow` / `anqi_dyed_bone` / `anqi_fenglinghe_bone` / `anqi_shanggu_bone` 及各 `_charged` 变体）从 `category = "misc"` 升为 `category = "anqi"`。**3 个 anqi 容器**（`anqi_container_quiver` / `anqi_container_pocket_pouch` / `anqi_container_fenglinghe`）保持/升级为 `container`（若 [[plan-nested-pack-base-v1]] 未覆盖则本 plan 顺带，验收表标注）。校验现有 `max_stack_count = 32` 字段在升 Anqi 后仍优先覆盖默认（`mod.rs:1180`）。
- 给 [[plan-nested-pack-base-v1]] P5 升级的 5 随身子包补 `accept = [...]`（同改 `workbench_materials.toml` 同 5 条目，依赖图保证 B 在 A 后开，注意相邻段 merge——见 reminder「A-P5 / B-P3 改同一 TOML 协调」）：
  - `herb_pouch` = `["herb", "food"]`
  - `ore_sack` = `["mineral"]`（**filter 旗舰验收**）
  - `projectile_bag` = `["anqi"]`
  - `water_skin` = `["liquid"]`
  - `herb_crate` = `["herb"]`
- 补其余随身容器（升 `category = "container"` + 加 `[item.container]` 块 + `accept`）：
  - `sealed_vial`（accept `["pill", "food"]`，行为 Halve 见 P2）
  - `coin_box`（accept `["bonecoin"]`——唯一已有专属 category，筛选最易）
  - `sealed_envelope`（accept `["recipe_fragment", "recipe_hint", "scroll"]` 情报袋）
  - `spirit_seal_box`（accept `["treasure", "pill"]` 高灵物，行为 Freeze 见 P2）
  - `moisture_guard`（accept `[]` 通用/或 `["herb"]`，行为 SpoilOnly 见 P2 + §8 #2）
- 容量（rows×cols）见 12 容器验收表，与 [[plan-nested-pack-base-v1]] §8.1 #4 一并定稿。
- **测试（饱和化）**：
  - 每个随身容器各一条：装合法物品通过 + 装非法物品拒绝（`coin_box` 只收骨币、`ore_sack` 只收矿石、`sealed_envelope` 只收情报、`projectile_bag` 只收暗器）。
  - anqi 物品迁移后 category 解析为 `Anqi` pin（每类暗器至少一条）；`anqi_*` 进 `projectile_bag` 通过、`ore_iron` 进 `projectile_bag` 拒绝。
  - 资产加载：全部 12 容器 + 12 暗器 TOML 解析无 panic（registry load pin，撞红即说明 category/spec 字段写错）。

## P4 — client filter 同步 + 灰显 + 12 容器验收 ⬜

- `LootContainerOpenV1`（`server_data.rs:611`）加 `pub accept_filter: Vec<ContainerAcceptFilterV1>`（默认空 = 全收）。**套包子容器打开复用此同一 `LootContainerOpenV1`**（dep plan L35：`source_kind = LootContainerSourceKindV1::PackItem { instance_id }`），故无需第二个 schema——`PackContainerOpen/Move/Close` 是 C2S enum 变体，S2C 一律走 `LootContainerOpenV1`。新增 schema 类型 `ContainerAcceptFilterV1`（serde + protobuf 双端）：
  ```rust
  pub enum ContainerAcceptFilterV1 {
      Category(String),       // ItemCategory 字符串名
      TemplatePrefix(String),
  }
  ```
- protobuf `.proto` 同步加 `repeated` 字段 + client Java DTO（`ProtoServerDataBridge.java` 解析）。
- client `SubContainerPanel`（[[plan-nested-pack-base-v1]] P4 新建）收到 `accept_filter` 存为面板字段；拖拽中（`InspectScreen.mouseDragged` 的子面板分支，由 nested-pack P4 建立）对**当前拖拽物品**不通过 filter 的子容器目标格调 `GridSlotComponent.setHighlightState(HighlightState.INVALID)`，松手放非法格被 server `validate_attach_fits` 拒绝（client 灰显仅预提示，server 是权威）。
- **视听**（非法拖拽灰显 + 合法放入是玩家可感知）：
  - **灰显**：复用现有 `HighlightState.INVALID`，红 tint `0x33CC2222`（`GridSlotComponent.java:75`，ARGB alpha=0x33≈20%）。**以现有 INVALID 常数为准**（§8 #5 已决，骨架原写的 `#C04040 opacity 0.4` 作废——理由：改 INVALID 颜色会影响全仓所有 INVALID 调用方如 botany 边框，得不偿失）。HUD layer：随 owo-lib 面板 component draw pass（无独立 HudRenderLayer）。无 fade（即时 tint，跟随 `updateHighlights` 帧刷新）。
  - **合法放入 SFX**：vanilla sound ID `minecraft:item.bundle.insert`（无需本地资产），audio_recipe 单层：`{ sound: "item.bundle.insert", pitch: 1.0, volume: 0.5, delay_ticks: 0 }`，在 server `handle_pack_container_move` 成功提交后随 `LootContainerUpdateV1` 触发 / 或 client drop 成功本地播。
  - **非法放入 SFX**：vanilla `minecraft:block.note_block.bass`（低沉拒绝音），audio_recipe：`{ sound: "block.note_block.bass", pitch: 0.6, volume: 0.4, delay_ticks: 0 }`，client 收到 server `Err`（或本地预判拒绝）时播。
  - 无粒子（纯 UI 操作）；无动画；无 narration（背包操作不走天道叙事）。
- 逐个核验 12 容器各自 kind/filter/behavior（验收表见下）。
- **测试（饱和化 e2e）**：
  - client 收 `accept_filter` 后拖非法物品到子容器格 → 该格显示 INVALID tint（render 验证 tint 值）。
  - 拖合法物品 → 无 INVALID tint、放入成功发 `PackContainerMove`。
  - 端到端：装 `ore_sack` 进背包 → 双击打开 → 拖矿石入（通过 + insert SFX）→ 拖草药入（灰显 + 拒绝 + bass SFX）→ server `validate_attach_fits` 返回对应 Ok/Err。
  - schema：`LootContainerOpenV1` 含 `accept_filter` 字段双端 serde sample 对拍；`source_kind = LootContainerSourceKindV1::PackItem { instance_id }` 时同一 `LootContainerOpenV1` 仍携带 `accept_filter`（套包路径复用 pin，证明无需第二个 schema）；`ContainerAcceptFilterV1` 两变体正反 sample。

## 12 容器分类验收表（调查综合产出）

| 容器 | kind | 容量 (r×c) | filter | 行为 | 落地阶段 |
|------|------|------|--------|------|----------|
| `herb_pouch` 灵草囊 | subbag | 3×3 | Herb/Food | 套包，Normal 保鲜 | A-P5（升 container）+ B-P3（加 accept） |
| `ore_sack` 矿石袋 | subbag | 3×3 | **Mineral** | 套包（filter 旗舰验收） | A-P5 + B-P3 |
| `projectile_bag` 暗器袋 | subbag | 3×4 | **Anqi** | 套包 | A-P5 + B-P3（依赖 B anqi 迁移） |
| `water_skin` 水囊 | subbag | 2×2 | **Liquid** | 套包 | A-P5 + B-P3 |
| `herb_crate` 灵草箱 | subbag | 4×4 | Herb | 套包随身大筐（放置版见 plan-C） | A-P5 + B-P3 |
| `sealed_vial` 密封药瓶 | subbag | 2×2 | Pill/Food | 套包 + **Halve** 保鲜 | B-P2（行为）+ B-P3（container+filter） |
| `spirit_seal_box` 封灵匣 | subbag/intel | 2×2 | Treasure/Pill（高灵物） | 套包 + **Freeze** 保鲜 | B-P2 + B-P3 |
| `moisture_guard` 防潮包 | subbag | 3×3 | accept=[]/Herb | 套包 + **SpoilOnly** 防霉（rate §8 #2） | B-P2 + B-P3 |
| `coin_box` 骨币匣 | economy | 3×3 | **BoneCoin** | 套包（仅筛选，不延缓骨币半衰） | B-P3 |
| `sealed_envelope` 封缄 | intel | 1×2 | RecipeFragment/Hint/Scroll | 套包情报袋 | B-P3 |
| `trade_crate` 货箱 | placeable | 4×4 | accept=[] | → [[plan-placeable-container-blocks-v1]] | Plan C |
| `dead_drop_box` 死信箱 | placeable/intel | 2×2 | RecipeFragment/Hint/Scroll | → [[plan-placeable-container-blocks-v1]]（阵法防砸） | Plan C |

> 注：`trade_crate` / `dead_drop_box` 为世界方块容器，本 plan **不**实装（仅在此表登记契约），由 [[plan-placeable-container-blocks-v1]] 处理；若该 plan 的方块容器物品（持 ContainerState）将来被放入随身套包，filter 与本 plan 一致（交叉引用，见 reminder「Plan A 与 Plan D §8 交叉引用」，新名 [[plan-workbench-place-runtime-v1]]）。

## §8 开放问题（P0 决策门前需收口）

> Q1（筛选粒度）/ Q3（accept=[] 全收）/ Q5（INVALID 颜色）已凭实地证据在正文定案（见各阶段块脚注），原表保留以备追溯，**实施时以下方定案为准**。仅 Q2 需 §8.1 收口。

| # | 问题 | 定案 / 状态 |
|---|------|------|
| 1 | 矿石/液体筛选粒度？ | **已定**：升 `ItemCategory::Mineral/Anqi/Liquid` 变体（用户已决议；`mod.rs:225` 现 14 变体无此三者）；矿石必要时再用 `ItemInstance.mineral_id` NBT（`mod.rs` 已有字段）二级细筛——本 plan 仅做 category 粗筛，NBT 细筛留后续。 |
| 2 | `moisture_guard` SpoilOnly 精确 rate？ | **悬留（P2 前必拍）**：现有 `inventory_snapshot_emit.rs:1421-1428` + `spiritwood/mod.rs:628` 对 `ice_cellar` 用 `SpoilOnly { rate: 0.3 }`。候选：沿用 0.3（与 ice_cellar 一致，叙事「防霉」=减缓 Spoil track 70%）或重校。未定则 `item_freshness_behavior` 对 moisture_guard 永远 Normal 兜底，保鲜对玩家隐形。§8.1 收口时拍数。 |
| 3 | `accept=[]` 语义？ | **已定**：`None` 或 `Some(vec![])` 均 = 全收（通用容器）。唯一合理选项，正文 P0 `item_passes_filter` 已实现。 |
| 4 | 各容器精确 grid？ | **已定**（见上验收表，与 [[plan-nested-pack-base-v1]] §8.1 #4 一并定稿；water_skin 取 2×2、sealed_vial 取 2×2）。 |
| 5 | 非法拖拽灰显颜色？ | **已定**：用现有 `HighlightState.INVALID`（`GridSlotComponent.java:75` 红 tint `0x33CC2222`）。骨架原写 `#C04040 opacity 0.4` 作废——改 INVALID 常数会波及全仓所有 INVALID 调用方（botany 边框等），不值。 |

> §8.1 收口（仅 Q2）后追加 `## §8.1 决议（pre-P0，YYYY-MM-DD）`，带 file:line + plan 章节双锚点（依据 docs/CLAUDE.md §5.1）。

## §10 实施工作流

依据 docs/CLAUDE.md §六。本 plan scope = 5 PR，多 PR 序列化（不拆多 plan）。**纯逻辑 + 数据 + UI 接线，无 NBT 建筑 / bbmodel 资产 → §10.1 三轮打磨 + `<PROMISE>` 不适用**（client UI 灰显接现成 INVALID 高亮，非新视觉资产产出）。

### §10.1 依赖前置门

**[[plan-nested-pack-base-v1]] 必须全部 merge 到 main 后才开本 plan**：P1 的 `pack_` 前缀校验依赖其 P3 命名规则；P2 的 enter/exit 接入依赖其 P2 `handle_pack_container_move`；P3 的随身子包 container 化依赖其 P5；P4 的灰显依赖其 P4 `SubContainerPanel` + `InspectScreen.mouseDragged` 子面板分支。升 active 判断门：nested-pack-base **P5 已合入 main**。

### §10.2 PR 拆分点（依赖顺序，前一个 merge 后开下一个）

1. **PR-1（P0）**：`ItemCategory` 三变体 + `ContainerAcceptFilter` + `ContainerSpec.accept_filter` 数据模型 + `item_passes_filter` + 全 match 补臂。纯 server 数据，独立可测。
2. **PR-2（P1）**：`validate_attach_fits` 增 `&ItemRegistry` 入参 + 改 3 处调用点（`mod.rs:2535`/`4126`/`4147`）+ ContainerState→registry 反查两链 + 白名单校验（装备容器 + `pack_` 子容器）。依赖 PR-1 + nested-pack `pack_` 规则。
3. **PR-3（P2）**：扩 `item_freshness_behavior` 三映射 + enter/exit 接 `handle_pack_container_move` 生产路径 + sweep/snapshot 子容器分支。依赖 PR-1 + nested-pack P2。
4. **PR-4（P3）**：anqi category 迁移（anqi.toml）+ 12 随身容器 TOML 升 container/补 accept。依赖 PR-1/2/3 + nested-pack P5（同改 workbench_materials.toml，注意 merge）。
5. **PR-5（P4）**：`LootContainerOpenV1` 扩 `accept_filter`（套包路径因复用同一 schema 自动继承）+ protobuf + client DTO + `SubContainerPanel` 灰显 + 视听 SFX + 12 容器 e2e 验收。依赖 PR-1..4 + nested-pack P4。

### §10.3 每 PR 独立 subagent（context 隔离）

```
Agent(
  subagent_type: "claude",
  model: "opus",
  prompt: "...本 PR 范围 + 测试要求（饱和化口径）+ 依赖前置确认...\n\nultrathink"
)
```
主线只接收 subagent result（200-500 token），不亲自跑实施；merge 命令主线亲自做。

### §10.4 CodeRabbit ScheduleWakeup 等待协议

每 PR 走完整等待：`gh pr checks <PR>` pending → `ScheduleWakeup delaySeconds=1200`，最多 3 回合 60min；fail 按 commands/consume-plan.md step 7 严重性桶处理；修完 review 必重等 CR re-review，不自判通过；前一个 PR 未 APPROVED/收敛不开下一个。

### §10.5 单次 consume-plan 全自动到 merge

用户提交 `/consume-plan plan-container-filter-and-completion-v1` 后即可下班；醒来看本 plan 是否在 `docs/finished_plans/`。全部 P ✅ + Finish Evidence 填好后由 consume-plan 在 PR 末尾 `git mv` 入 finished_plans/。

## Finish Evidence

（迁入前必填：落地清单 / 关键 commit / 测试结果 / 跨仓库核验 server·client symbol / 遗留依赖其他 plan 的待办）
