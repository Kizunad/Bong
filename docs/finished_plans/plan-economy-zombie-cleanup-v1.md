# plan-economy-zombie-cleanup-v1 — 经济/流派/材料/工具类僵尸物品消杀

> 一句话：处理僵尸物品审计集中删除 / 接通类——2 个蜕壳流伪装道具**接通**（disguise_wrap / camouflage_net 挂上 `FalseSkinTier`，走 tuike_v2 真实使用闭环）、2 个换代放置 kit **清理**（forge_station_kit / furnace_kit_fantie 配方产物改指新代 ID）、6 个无系统经济物品**删除**（bone_coin_blank 等，TOML+配方+icon+全仓引用清零）、10 个材料断链 / 工具类**删除**（2026-06-10 扩入，含 `trade_scale_stand` 原子补删）。
>
> 来源：僵尸物品审计；用户拍板 2026-06-10「#1/#3 立一个，#2 就算了删除吧」+「工具 4 直接删除，该做适配的适配」（适配 2 件见 [[plan-gathering-tool-bind-v1]]）。本 plan 已据 reminder.md「plan-economy-zombie-cleanup-v1（PR #472，Pi review 2026-06-10 的 5 条勘误）」逐条修订（蜕壳入口定位、rat_bait 删除理由、3 处路径勘误、配方注释锚点、P0 函数名级交付物），并合入 PR #475 材料断链 / 工具类裁决。

**依赖**：无（纯接通/清理，不依赖其他 plan）。P1 kit 清理与 `plan-block-lifecycle-v1` 已落地的放置链路（`alchemy/furnace.rs::furnace_tier_from_item_id`、`forge/station.rs::handle_place_station_request`）**只读对接、不改其逻辑**——本 plan 只改配方产物指向已被放置链路认可的 ID。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | 蜕壳流伪装道具接通（disguise_wrap / camouflage_net） | ✅ 2026-06-11 |
| P1 | 放置 kit 换代清理（forge_station_kit / furnace_kit_fantie） | ✅ 2026-06-11 |
| P2 | 6 个经济物品删除（bone_coin_blank 等） | ✅ 2026-06-11 |
| P3 | 10 个材料断链 / 工具类删除（2026-06-10 扩入，含 `trade_scale_stand` 原子补删） | ✅ 2026-06-11 |

全部开放问题已在 §8.1 收口（见文末）。**实施时以 §8.1 决议为准**，§8 原表保留以备追溯。

---

## 接入面（防孤岛 checklist）

### 进料

- **蜕壳流系统（tuike_v2，主路径）**：
  - 档位映射 `server/src/combat/tuike_v2/state.rs:111`（`fn false_skin_tier_for_item(template_id) -> Option<FalseSkinTier>`），当前认 `tuike_false_skin_{fan,light,mid,heavy,ancient}` + 兼容 v1 两 ID（silk→Fan、rotten_wood_armor→Mid）。P0 在此扩 2 个分支。
  - 档位枚举 `FalseSkinTier { Fan, Light, Mid, Heavy, Ancient }`（`state.rs:117`），含 `material_factor` / `maintain_qi_per_sec` / `min_realm` const。
  - 装备 tick `server/src/combat/tuike_v2/tick.rs:43`、技能触发 `server/src/combat/tuike_v2/skills.rs:362`——均走 `false_skin_tier_for_item`，扩档位后自动接通，无需改这两处。
- **蜕壳流系统（tuike v1，向后兼容桥，仍把守装备入口）**：
  - `server/src/combat/tuike.rs:114`（`fn false_skin_kind_for_item(template_id) -> Option<FalseSkinKind>`），只认 `SPIDER_SILK_FALSE_SKIN_ITEM_ID` / `ROTTEN_WOOD_ARMOR_ITEM_ID`（`tuike.rs:22-23`）。
  - **装备守卫** `server/src/inventory/mod.rs:3863`（`EquipSlotV1::FalseSkin` 分支调 `false_skin_kind_for_item`，None 即拒装）。
  - **装备处理** `server/src/network/client_request_handler.rs:7452`（移入 `EquipSlot::FalseSkin` 时调 `false_skin_kind_for_item` 取 realm gate）。
  - ⚠️ 这两处仍走 v1 lookup——P0 必须同步打通，否则新道具即便 tuike_v2 认了也装不上（见 §8.1 #1）。
- **配方表** `server/src/craft/workbench_recipes.rs`：
  - `// #87 伪装包裹`（`disguise_wrap`×2，`CraftCategory::TuikeSkin`，料 `rough_cloth`×2+`hui_jin_tai`×1）
  - `// #94 伪装网`（`camouflage_net`×1，`CraftCategory::TuikeSkin`，料 `rough_cloth`×3+`spirit_grass`×2）
  - `// #99 凡铁炉组件`（产物 `furnace_kit_fantie`，P1 改产物）
  - `// #100 锻造台组件`（产物 `forge_station_kit`，P1 改产物）
  - `// #83 骨币胚` / `// #85 交易秤台` / `// #86 标记石` / `// #88 标价签` / `// #89 交易傀儡骨架` + `rat_bait`（`"workbench.cultivation.rat_bait"`，`workbench_recipes.rs:954`）——P2 删除
  - `// #29 朱砂粉` / `// #53 铁剑胚` / `// #56 石矛头` / `// #57 弹弓` / `// #58 弹弓石` / `// #3 石锄` / `// #7 研钵` / `// #10 隔热手套` / `// #9 交易秤`——P3 删除
- **物品模板** `server/assets/items/workbench_materials.toml`（P0-P2 的 10 个 id 在此：`disguise_wrap:793` / `camouflage_net:601` / `furnace_kit_fantie:933` / `forge_station_kit:944` / `bone_coin_blank:760` / `trade_scale_stand:804` / `price_tag:837` / `trade_puppet_frame:815` / `waymark_stone:782` / `rat_bait:509`；P3 删除清单见 §P3 表与 §8.1 #4）
- **放置链路（P1 只读对接，不改）**：
  - `server/src/alchemy/furnace.rs:106`（`fn furnace_tier_from_item_id`，认 `furnace_fantie`→tier1）
  - `server/src/forge/station.rs:134`（`handle_place_station_request`，查 `template.forge_station_spec`；`fan_iron_anvil` 在 `server/assets/items/forge.toml` 有 `[item.forge_station] tier=1`）

### 出料

- **P0**：disguise_wrap / camouflage_net 进入蜕壳流真实使用闭环——装备 → `StackedFalseSkins` 叠层 → 受击吸污 → `false_skin_state` HUD payload emit → client `FalseSkinStateHandler` 渲染。
- **P1**：`#99`/`#100` 合成产物变为已被放置链路认可的 `furnace_fantie` / `fan_iron_anvil`；旧代 kit 模板删除后 `/give` 补全与配方表均无死 ID。
- **P2**：`ItemRegistry` 加载后 6 个经济僵尸 ID 消失；`/give` 补全收缩；配方表/loot/NPC stock 全仓 0 引用；启动 smoke 不崩。
- **P3**：10 个材料断链 / 工具类 ID 消失；2 个临门适配项不在本 plan 删除（归 [[plan-gathering-tool-bind-v1]]）；`heat_gloves` 与 `bing_jia_shou_tao` 明确区分，防误删冰甲手套。

### 共享类型 / event

- **复用** tuike_v2 现有状态 emit `false_skin_state`（`server/src/network/false_skin_state_emit.rs:91` `emit_tuike_v2_false_skin_state_payloads`）；**不新造** event，**不新增 ItemCategory**，**不新增 `FalseSkinTier` 变体**（disguise_wrap / camouflage_net 复用现有 `Fan` 档，见 §8.1 #1）。

### 跨仓库契约

- **server ↔ client**：`false_skin_state` payload（`ServerDataType::FalseSkinState` → type id `"false_skin_state"`，`server/src/network/agent_bridge.rs:106`）；client 消费方 `client/src/main/java/com/bong/client/combat/handler/FalseSkinStateHandler.java` + HUD `FalseSkinStackHud.java`——P0 复用此既有通道，**无新 payload**。
- **client icon 资产**：P1/P2/P3 删除的物品需同步删 PNG（路径 `client/src/main/resources/assets/bong-client/textures/gui/items/`，确认存在的删：`furnace_kit_fantie.png` / `forge_station_kit.png` / `bone_coin_blank.png` / `trade_scale_stand.png` / `price_tag.png` / `trade_puppet_frame.png` / `waymark_stone.png` / `rat_bait.png` / `stone_hoe.png` / `mortar_stone.png` / `heat_gloves.png` / `trade_scale.png` / `powder_zhu_sha.png` / `iron_sword_blank.png` / `stone_spearhead.png` / `sling_stone.png` / `sling_weapon.png`）。⚠️ **新增** `fan_iron_anvil.png`（P1 把 `#100` 产物改指 `fan_iron_anvil` 使其首次进背包，当前**无**此图标，须 `/gen-image item` 生成——见 §P1 交付物 4）；`furnace_fantie.png` 已存在不需补。⚠️ **保留** `disguise_wrap.png` / `camouflage_net.png`（P0 接通项）。
- **agent**：P2/P3 删除 ID 与 P0 接通道具在 agent schema / WorldModel 中**零引用**，agent 不参与。

### worldview 锚点

- **P0**：`docs/worldview.md` §五「防御三流」之 2「替尸 / 蜕壳流：剥离式弃子防御」（"用拟态灰烬蛛丝、死域朽木等材料制作伪灵皮穿在外面，注入少量真元模拟气息"）；流派 Primary Axis 表「替尸·蜕壳（防）| 伪皮档位（轻/中/重）+ 材料色克 + 单层吸收上限」。disguise_wrap / camouflage_net 是低成本伪皮材料，对标最低档 `Fan`。
- **P2/P3**：删除项无锚点正是删除理由。`worldview.md` §九「经济与交易」虽有摆摊/游商傀儡/骨币铸造的**叙事**层，但 `docs/finished_plans/plan-mineral-v1.md:38` 明确「骨币归 plan-fauna-v1，铸币闭环」未落地；摊位/标价/游商系统全无 plan 支撑。朱砂粉 / 剑胚 / 矛头 / 弹弓 / 石锄等若要复活均需对应系统 plan，不在本消杀 plan 内保留半截物。

### qi_physics 锚点

- **无新物理常数**。P0 不引入任何衰减/逸散常数：tuike_v2 维护真元消耗走既有 `FalseSkinTier::maintain_qi_per_sec`（`state.rs`，已有，本 plan 不增改）；若 disguise_wrap/camouflage_net 走 `Fan` 档则直接复用其 0.1 qi/s。受击吸污/蜕落逻辑全在 tuike_v2 既有代码路径，P0 不碰 `qi_physics::ledger`。P1/P2/P3 不涉真元。

---

## P0 — 蜕壳流伪装道具接通（2）

**目标**：让 `disguise_wrap`、`camouflage_net` 这两个有配方（#87/#94，已标 `CraftCategory::TuikeSkin`）、有 TOML、却无任何 lookup 收录的半截僵尸，接通 tuike_v2 真实使用闭环。**采用 §8.1 #1 决议的路线 A（扩 lookup）+ 三处同步打通**。

### 交付物（可核验抓手）

1. **tuike_v2 档位映射扩分支** `server/src/combat/tuike_v2/state.rs` `fn false_skin_tier_for_item`：新增两个 const ID `DISGUISE_WRAP_ITEM_ID = "disguise_wrap"`、`CAMOUFLAGE_NET_ITEM_ID = "camouflage_net"`，两者均 `=> Some(FalseSkinTier::Fan)`（最低档，对标其廉价配方，见 §8.1 #1）。
2. **v1 装备入口同步打通**：扩 `server/src/combat/tuike.rs:114` `fn false_skin_kind_for_item`——新增 `DISGUISE_WRAP_ITEM_ID | CAMOUFLAGE_NET_ITEM_ID => Some(FalseSkinKind::SpiderSilk)`（复用最轻 v1 kind，仅为通过装备守卫；实际叠层行为由 tuike_v2 的 `Fan` 档决定）。这一步同时打通 `inventory/mod.rs:3863` 守卫与 `client_request_handler.rs:7452` realm gate（两者都调此函数）。
3. **配方保持不变**：#87/#94 已是 `CraftCategory::TuikeSkin`、产物已是这两 ID，无需改配方（路线 A 不动产物）。
4. **e2e 使用闭环**：合成 → 装备到 `EquipSlotV1::FalseSkin` → `StackedFalseSkins` 入层 → 受击吸污 → `emit_tuike_v2_false_skin_state_payloads` emit `false_skin_state` → client `FalseSkinStateHandler` 收到层数变化。

### 测试声明（饱和化）

- `tuike_v2::state` 单测：
  - happy path —— `false_skin_tier_for_item("disguise_wrap")` / `("camouflage_net")` 均 `== Some(Fan)`。
  - 回归（状态转换不破）—— 既有 5 档 ID + v1 兼容两 ID 的 lookup 结果逐一对拍不变（pin 全部 6 个既有映射，任何变体掉档撞红）。
  - 错误分支 —— 未知 ID（如 `"bone_coin"`）仍 `== None`。
- `tuike::false_skin_kind_for_item` 单测：新增两 ID `is_some()`；既有 silk/rotten_wood_armor 回归不变；未知 ID `is_none()`。
- 装备守卫单测（`inventory`）：`EquipSlotV1::FalseSkin` 接受 `disguise_wrap`/`camouflage_net`（realm 达标时）；realm gate **走 v1 `false_skin_kind_for_item`→`FalseSkinKind::SpiderSilk`→`can_equip_false_skin(FalseSkinKind::SpiderSilk.min_realm())`**（核 `tuike.rs:56` = `Realm::Induce`，**非** v2 `Fan.min_realm()`=`Awaken`）。边界对拍：`Realm::Awaken`（醒灵）应被**拒绝**（< Induce），`Realm::Induce`（引气）及以上**接受**。**已知妥协**：入装门槛随 v1 kind（SpiderSilk=Induce）走，与 v2 `Fan` 档语义（Awaken）不一致——见 §8.1 #1 第 6 点。测试断言锁的是 v1 SpiderSilk 的 Induce 门槛，不写 `Fan.min_realm()`。
- e2e（集成）：合成 disguise_wrap → equip → 受一次攻击 → 断言 `false_skin_state` payload 中该实例层数从 1 减、`StackedFalseSkins.layers` 非空；断言 ash 残渣按 `FalseSkinTier::Fan.residue_output_item_id()` 产出（`tuike_false_skin_ash`）。

### 视听规格

P0 **不新增**视听资产——完全复用 tuike_v2 既有蜕壳视听：
- 伪皮蜕落 VFX：复用 `server/src/network/tuike_ash_emit.rs` 既有灰烬 emit（`bong:vfx_event` 既有蜕壳 ID，不新建贴图）。
- HUD：复用 `FalseSkinStackHud.java` 既有层数堆叠渲染（`Fan` 档色）。
- 音效：复用 tuike_v2 既有蜕落 audio_recipe。
- narration：复用 tuike_v2 既有蜕落 narration，本 plan 不新增文案。

（disguise_wrap 与 camouflage_net 在视听/机制上**与现有 Fan 档完全一致**——本 plan 只接通"它们能被当 Fan 档伪皮使用"，不做差异化效果。差异化「驻地遮蔽」效果不在 v1 范围，见 §8.1 #1 拒绝理由。）

---

## P1 — 放置 kit 换代清理（2）

**目标**：`forge_station_kit` / `furnace_kit_fantie` 是 `fan_iron_anvil` / `furnace_fantie` 的旧代 ID，放置 handler 不认（`furnace_kit_fantie` 不在 `furnace_tier_from_item_id` 表；`forge_station_kit` TOML 无 `forge_station_spec`），合成后落背包成死货。**采用 §8.1 #2 决议：改配方产物指向已被放置链路认可的新代 ID，删旧代 TOML 模板，保留"合成出可放置炉/砧"玩法。**

### 交付物（可核验抓手）

1. **配方产物替换** `server/src/craft/workbench_recipes.rs`：
   - `// #99 凡铁炉组件`：产物 `"furnace_kit_fantie"` → `"furnace_fantie"`（已被 `furnace_tier_from_item_id`→tier1 认可，且 `furnace_fantie` 当前**无独立配方**，grep `workbench_recipes.rs` 仅此处与放置链路引用——改产物不撞车）。
   - `// #100 锻造台组件`：产物 `"forge_station_kit"` → `"fan_iron_anvil"`（在 `forge.toml` 有 `[item.forge_station] tier=1`，被 `handle_place_station_request` 认可）。
2. **删旧代 TOML 模板** `server/assets/items/workbench_materials.toml`：删 `furnace_kit_fantie`（:933）、`forge_station_kit`（:944）两 `[[item]]` block。
3. **删 client icon** `furnace_kit_fantie.png`、`forge_station_kit.png`。
4. **补 `fan_iron_anvil` 图标（新可见物品）**：`#100` 产物改为 `fan_iron_anvil` 后，该 ID **首次**成为可合成 / 落背包 / 玩家持有的物品——之前它仅经 `forge.toml` 的 `forge_station_spec` + 测试 fixture 出现，玩家从不持有，故历史无图标（已核：`textures/gui/items/` 下**无** `fan_iron_anvil.png`，`ItemIconRegistry.java:15` `FALLBACK_ITEM_PATH` 会回退 `broken_artifact.png`）。**用 `/gen-image item` 生成** `client/src/main/resources/assets/bong-client/textures/gui/items/fan_iron_anvil.png`（参照 `fan_iron_anvil` 在 `forge.toml` 的"凡铁砧台/tier1 锻造台"语义出图），对齐 memory `feedback_item_icon_gen`（新 ItemTemplate 进合成需配图标）。`furnace_fantie` **不受影响**——`furnace_fantie.png` 已存在（已核），`#99` 改产物指向它无图标缺口。
5. **不改放置链路代码**——`furnace_tier_from_item_id`、`handle_place_station_request` 保持原状，仅靠产物指向接通。

### 测试声明（饱和化）

- 配方注册单测：`#99` 产物 `template_id == "furnace_fantie"`、`#100` 产物 `== "fan_iron_anvil"`；料表不变（#99 `iron_ingot`×6+`spirit_charcoal`×2+`stone_chunk`×4；#100 `iron_ingot`×4+`spirit_wood`×2+`stone_chunk`×6）。
- registry 加载单测：`furnace_kit_fantie` / `forge_station_kit` 模板查询 `== None`（死 ID 消失）；`furnace_fantie` / `fan_iron_anvil` 仍存在。
- 放置链路回归（集成）：合成 #99 产物 → 放置 → `furnace_tier_from_item_id` 返回 `Some(1)` → furnace ECS entity spawn；合成 #100 产物 → 放置 → `forge_station_spec.tier == 1` 通过 → station spawn。**断言放置成功**（不再被 warn 丢弃）。
- `/give` 补全：补全候选列表（registry 全 ID）不含 `furnace_kit_fantie` / `forge_station_kit`。

### 视听规格

逻辑/资产清理为主，**但有一处新可见物品图标缺口**：`#100` 产物改为 `fan_iron_anvil` 使其首次进玩家背包（见交付物 4），需 `/gen-image item` 出 `fan_iron_anvil.png`（否则回退 `broken_artifact.png`，违反 `feedback_item_icon_gen`）。`furnace_fantie` 图标已存在，无缺口。放置后 furnace/station 的方块视听沿用 `furnace_fantie` / `fan_iron_anvil` 既有放置表现（`plan-block-lifecycle-v1` 已实装），本 plan 不增改放置表现，仅补背包图标。

---

## P2 — 6 个经济物品删除

**目标**：删除 6 个无任何系统支撑的经济/工具物品。全仓 grep 确认（除 TOML+配方+icon 外）server/agent/client **零引用**：无 loot table、无 NPC stock、无 agent schema。

删除清单：

| id | 名称 | TOML 行 | 配方注释锚点 | 删除理由 |
|----|------|---------|------|----------|
| `bone_coin_blank` | 骨币胚 | `:760` | `// #83 骨币胚` | 铸币闭环无 plan 支撑（`plan-mineral-v1.md:38` 证铸币归 fauna 未落地） |
| `trade_scale_stand` | 交易秤台 | `:804` | `// #85 交易秤台` | 摊位系统不存在 |
| `price_tag` | 标价签 | `:837` | `// #88 标价签` | 挂牌定价不存在 |
| `trade_puppet_frame` | 交易傀儡骨架 | `:815` | `// #89 交易傀儡骨架` | 玩家自营游商不存在 |
| `waymark_stone` | 标记石 | `:782` | `// #86 标记石` | 世界标记不存在 |
| `rat_bait` | 鼠群诱饵 | `:509` | `workbench_recipes.rs:954`（`"workbench.cultivation.rat_bait"`） | **仅有配方定义，无消费端接入**（reminder.md 勘误：原"鼠群系统在 spawn_tutorial.rs"说法不成立，无任何系统读此 ID）；将来复活走新 plan 重立 |

### 交付物（可核验抓手）

1. **删 TOML 模板**（6 个 `[[item]]` block）`server/assets/items/workbench_materials.toml`。
2. **删配方** `server/src/craft/workbench_recipes.rs`：#83 / #85 / #86 / #88 / #89 + `"workbench.cultivation.rat_bait"`（:954）。
3. **删 client icon**（6 PNG）`bone_coin_blank.png` / `trade_scale_stand.png` / `price_tag.png` / `trade_puppet_frame.png` / `waymark_stone.png` / `rat_bait.png`。
4. **存量实例处理**：按 §8.1 #3 决议——**不做折算返还**。删模板**不会让加载/快照 panic**，根因是 `ItemInstance`（`inventory/mod.rs:339`）自带全部展示数据（`template_id`/`display_name`/`grid_w`/`grid_h`/`weight`/`rarity`/`description`/`spirit_quality`/`durability`/`freshness`/...），序列化↔反序列化与快照构建（`inventory_snapshot_emit.rs:251` `item_view_from_instance` 直接 `item.template_id.clone()`）**全程不查 `ItemRegistry`**，故缺模板的旧实例被原样保留并照常发给 client（仅图标在 client 端回退 `broken_artifact.png`，server 不崩）。**注意**：这是"item instance 自带数据、不依赖 registry 查模板"的天然不 panic，**不是** `inventory_snapshot_emit` 有"registry 返回 None 即 skip"分支——实地核 `inventory_snapshot_emit.rs` **无任何 `ItemRegistry` 查询、无 skip 缺模板的兜底分支**。
5. **trade_scale 处置**：按 §8.1 #4——`trade_scale` 不在 P2 删除，归 P3 工具类删除与 `trade_scale_stand` 原子收口，防留下「交易秤本体仍可合成但唯一用途已删」的新僵尸。

### 测试声明（饱和化）

- registry 加载单测：6 个删除 ID 逐一 `registry.get(id) == None`（每个 ID 一条专属 case，任何残留撞红）。
- 配方表单测：遍历全配方，断言无产物 ID ∈ 删除清单；无料表 ID ∈ 删除清单（防"产物删了但仍作他配方原料"）。
- 全仓引用守门（CI/grep 断言或 test）：`grep -rn "bone_coin_blank\|trade_scale_stand\|price_tag\|trade_puppet_frame\|waymark_stone\|rat_bait" server/src server/assets client/src agent/` 仅命中本 plan 删除点之外 0 处。
- 启动 smoke（集成）：server 启动 + ItemRegistry 加载不崩；持有删除 ID 的玩家库存反序列化 + 构建 `inventory_snapshot` 不 panic——根因是 `ItemInstance` 自带展示数据、快照构建（`item_view_from_instance`）不查 `ItemRegistry`（见 §8.1 #3 勘误），**不是**"兜底跳过缺模板"。断言：构建出的快照仍含该删除 ID 的 item view（原样保留，未被丢弃），且无 panic / unwrap 崩溃。
- `trade_scale` 阶段隔离回归：P2 commit 不删 `trade_scale`（归 P3），P3 commit 则断言 `trade_scale` 与 `trade_scale_stand` 模板 / 配方 / icon 同 PR 清零。

### 视听规格

纯清理，无视听。删除后这些 ID 不再出现在任何 UI / `/give` 补全。

---

## P3 — 10 个材料断链 / 工具类删除（2026-06-10 扩入，含 `trade_scale_stand` 原子补删）

**目标**：合入材料断链调查 workflow 裁决。裁决原则同 P2：需整套新系统支撑才能活的删；差临门一脚的适配（`herb_bundle` / `cao_lian` 两件）移 [[plan-gathering-tool-bind-v1]]，本 plan 不抢。

**材料断链 5（含 sling_weapon 连带 1）**：

| id | 名称 | 删除理由 | 落点 |
|----|------|---------|------|
| `powder_zhu_sha` | 朱砂粉 | 炼丹直接吃原矿 NBT（`material=zhu_sha_aux, mineral_id=zhu_sha`），粉中间体被设计放弃，符箓无 plan | 模板 `workbench_materials.toml:142-150` + 配方 #29 |
| `iron_sword_blank` | 铁剑胚 | 所有 forge blueprint 一步直用 `fan_tie`，接通需重做 multi-step tempering 整套 | 模板 `:677-685` + 配方 #53；blueprint 不受影响 |
| `stone_spearhead` | 石矛头 | 无矛 `WeaponKind` / 模板 / 蓝图（NPC 的矛是硬编码 `zhinian_spear`），接通 = 新武器类型 | 模板 `:616-624` + 配方 #56 |
| `sling_stone` | 弹弓石 | 弹弓退化为近战刺击（`player_attack.rs:117/125`）无弹药消耗；weapon-v1 正典明确不做 ranged | 模板 `:644-652` + 配方 #58 |
| `sling_weapon` | 弹弓（连带） | `sling_stone` 删 + ranged 不立项后，这把近战兜底「弓」无独立价值 | 模板 `:627-641` + 配方 #57 |

**工具 4（用户指示直接删）**：

| id | 名称 | 落点 |
|----|------|------|
| `stone_hoe` | 石锄 | 模板 `:340-348` + 配方 #3；无 `ToolKind` / `GatheringToolSpec` 接线，零 runtime |
| `mortar_stone` | 研钵 | 模板 `:351-359` + 配方 #7 |
| `heat_gloves` | 隔热手套 | 模板 `:362-370` + 配方 #10，随行删 `scroll_workbench_heat_gloves` unlock。⚠️ **严防误删 `bing_jia_shou_tao`（冰甲手套）**——后者是 `xue_po_lian` 的 `required_tool`（`botany/registry.rs:240`），两者是不同 item |
| `trade_scale` | 交易秤 | 模板 `:771` + 配方 #9。修 P2 红旗：原 P2 列了 `trade_scale_stand` 却漏 `trade_scale` 本体；stand 配方 #85 消耗 `trade_scale`，四处必须同 PR 原子删：scale 模板 / 配方 #9 / stand 模板 / stand 配方 #85 |

### 交付物（可核验抓手）

1. **删 TOML 模板**：上述 9 个 `[[item]]` block，加上与 `trade_scale` 原子补删的 `trade_scale_stand`；`bing_jia_shou_tao` 必须保留。
2. **删配方 / unlock**：上述 9 条配方；`heat_gloves` 随行删 `scroll_workbench_heat_gloves` unlock；`trade_scale` 与 `trade_scale_stand` 同 commit 清理。
3. **删 client icon**：`textures/gui/items/{stone_hoe,mortar_stone,heat_gloves,trade_scale,trade_scale_stand,powder_zhu_sha,iron_sword_blank,stone_spearhead,sling_stone,sling_weapon}.png`（存在的删，build 产物随 gradle 重建不手删）。
4. **未知 template_id 兜底**：这批仅 dev `/give` 可得，无 gameplay 来源，风险低；实施前实地确认 item-load 对未知 `template_id` 的行为。若无兜底，则 P3 先补「未知 id 不 panic + warn」最小路径，不做主动迁移脚本。

### 测试声明（饱和化）

- registry 加载单测：10 个删除 ID 逐一 `registry.get(id) == None`，`bing_jia_shou_tao` 仍存在。
- 配方表单测：遍历全配方，断言无产物 / 原料 ID ∈ P3 删除清单；`scroll_workbench_heat_gloves` unlock 清零。
- 全仓引用守门：`server/src` / `server/assets` / `client/src` / `agent/` 对 10 个 ID 仅允许测试 / plan 文档命中，生产引用 0。
- 存量兜底：未知 `template_id` 加载不 panic + warn 日志可断言；若确认现路径天然不查 registry，则用快照构建回归锁住。
- smoke：server 启动 + ItemRegistry 加载不崩。

### 视听规格

纯删除，无新增视听。删除后这些 ID 不再出现在任何 UI / `/give` 补全。

---

## §8 开放问题（P0 决策门前需收口）

1. **P0 路线 A vs B**：蜕壳入口扩 lookup（A，玩法增量）还是配方改产物（B，纯收缩）？A 需确认档位归属与装备入口分裂。
2. **P1 删 ID vs 改产物**：倾向改产物，需先核 `furnace_fantie` / `fan_iron_anvil` 是否已有自己的配方。
3. **存量实例迁移**：删除 6 ID 在已存档库存中的处理（折 bone_coin 返还 vs 直接清除）。
4. **trade_scale 连带处置**：删 #85 后 `trade_scale` 失去唯一下游，是否一并删？
5. **camouflage_net 驻地遮蔽**：若走 A，遮蔽网放置形态是否依赖 `plan-workbench-place-runtime-v1`？
6. **P3 存量兜底现状**：item-load 对未知 `template_id` 的行为（panic? 跳过? 原样保留?）实施前 grep 确认，无兜底则 P3 先补最小 warn 路径。

> 全部已在 §8.1 收口。原表保留以备追溯，**实施时以 §8.1 决议为准**。

---

## §8.1 决议（pre-P0 收口，2026-06-10）

### #1 P0 路线 A vs B + 档位归属 + 装备入口分裂

**决议**：
1. **走路线 A（扩 lookup），不做配方改产物**——保留 disguise_wrap / camouflage_net 作为独立的低成本伪皮材料，玩法增量。
2. 两道具均映射到最低档 `FalseSkinTier::Fan`（对标其廉价配方：`rough_cloth`+`hui_jin_tai` / `rough_cloth`+`spirit_grass`）。**不新增 `FalseSkinTier` 变体**——复用现有 5 档，避免造近义重名。
3. 走 **tuike_v2 主路径**扩 `false_skin_tier_for_item`（`state.rs:111`），因为 `tick.rs:43` / `skills.rs:362` 全部走 v2 lookup，叠层机制 `StackedFalseSkins` 可直接复用。
4. **必须同步扩 v1 `false_skin_kind_for_item`（`tuike.rs:114`）**——因为装备守卫 `inventory/mod.rs:3863` 与 realm gate `client_request_handler.rs:7452` 仍走 v1 lookup，只扩 v2 会让新道具被装备守卫拒绝（研究报告风险点 2 实证）。v1 侧映射到 `FalseSkinKind::SpiderSilk`（最轻 kind，仅为过守卫；真实叠层行为由 v2 `Fan` 决定）。
5. **拒绝**给 camouflage_net 做差异化「驻地遮蔽」效果（区别于 disguise_wrap 单件气息掩盖）——该效果需放置形态（见 #5），不在 v1 范围。v1 内两道具行为与 Fan 档一致。
6. **入装 realm 门槛走 v1 kind，与 v2 Fan 档不一致（已知妥协，不在本 plan 修正）**：realm gate 在 `client_request_handler.rs:7452` 与守卫 `inventory/mod.rs:3863` 均调 v1 `false_skin_kind_for_item`→`can_equip_false_skin(realm, kind)`，`kind` 为 v1 `FalseSkinKind`。两道具按本决议映射到 `FalseSkinKind::SpiderSilk`，其 `min_realm`（`tuike.rs:56`）= `Realm::Induce`（引气）；而 v2 `FalseSkinTier::Fan.min_realm()`（`state.rs:64`）= `Realm::Awaken`（醒灵）。核 `components.rs:15` `Realm` 枚举序 `Awaken(1) < Induce(3)`，故**实际入装门槛 = Induce，比 Fan 档语义所暗示的 Awaken 更高一级**——醒灵境玩家按 Fan 档本应能穿，但会被 v1 SpiderSilk 闸以"境界不足"拒绝。**本 plan 接受此 v1 闸（Induce），不改 realm gate 走向**：若要真正按 Fan(Awaken) 门槛，需让 gate 改走 v2 `false_skin_tier_for_item(..).min_realm()` 而非 v1 kind，会扩大 P0 改动面（动 `client_request_handler.rs` + `inventory/mod.rs` 两处守卫逻辑、且需重测既有 v1 silk/rotten_wood 装备路径），超本"接通/消杀"plan 范围。P0 测试断言锁定 v1 SpiderSilk 的 Induce 门槛（见 §P0 装备守卫单测），不写 `Fan.min_realm()`。

**落点**：`server/src/combat/tuike_v2/state.rs:111`（v2 lookup 扩分支）/ `server/src/combat/tuike.rs:114`（v1 lookup 扩分支）/ `server/src/inventory/mod.rs:3863`（守卫经 v1 自动接通，门槛 = SpiderSilk.min_realm = Induce）/ `server/src/network/client_request_handler.rs:7452`（realm gate 经 v1 自动接通，门槛 = Induce）/ plan §P0。

### #2 P1 删 ID vs 改产物

**决议**：
1. **改产物，不删配方**——保留"合成出可放置炉/砧"玩法路径。
2. #99 产物 `furnace_kit_fantie` → `furnace_fantie`；#100 产物 `forge_station_kit` → `fan_iron_anvil`。
3. 核实依据：`furnace_fantie` 在 `workbench_recipes.rs` **无独立配方**（grep 仅 `furnace_tier_from_item_id` 与放置链路引用它），改产物不与既有配方撞车；`fan_iron_anvil` 同理在 `forge.toml` 有 `forge_station_spec` 而无独立合成配方。两旧代 TOML 模板（kit）删除。

**落点**：`server/src/craft/workbench_recipes.rs`（`// #99` / `// #100`）/ `server/assets/items/workbench_materials.toml:933,944`（删 kit 模板）/ plan §P1。

### #3 存量实例迁移

**决议**：
1. **不做折算返还，存量实例原样保留**——删模板后旧实例不会崩，因为 `ItemInstance`（`inventory/mod.rs:339`）自带全部展示数据（template_id/display_name/grid/weight/rarity/description/spirit_quality/durability/freshness/...），其序列化↔反序列化与快照构建（`inventory_snapshot_emit.rs:251` `item_view_from_instance` 直接 `item.template_id.clone()`）**全程不查 `ItemRegistry`**。缺模板的旧物品照常加载、照常进快照发 client，server 端不 panic，client 端图标回退 `broken_artifact.png`。
2. **勘误（Pi review 2026-06-10）**：原决议称"由 `inventory_snapshot_emit` 既有兜底跳过（registry 返回 None → warn 但跳过）"——实地核 `inventory_snapshot_emit.rs` **无此机制**：构建快照不查 registry，也无"缺模板 skip"分支，删模板的旧物品会被原样塞进快照而非跳过。真正的"不 panic"来自 item instance 自带数据、不依赖 registry 查模板，**与 `inventory_snapshot_emit` 的兜底无关**。亦核 `persistence/mod.rs` 库存反序列化路径无 `registry.get(template_id).expect/unwrap` 强查（其 `.expect` 仅用于 death/archetype registry count，非 item template lookup）。
3. 拒绝"折 bone_coin 返还"路线：折算逻辑需写进 `persistence/mod.rs` 存档加载路径，改动复杂度远超清理本身收益；且这些 ID 为早期僵尸物品，线上存量预期极少。
4. 边界：P2 测试需覆盖"持删除 ID 快照的库存加载/构建快照不 panic"（已列入 §P2 测试）——但测试通过的原因是 instance 自带数据、根本无人查模板，**不是** plan 旧文所述的兜底跳过。

**落点**：`server/src/inventory/mod.rs:339`（`ItemInstance` 自带数据，反序列化不查 registry，只读验证不改）/ `server/src/network/inventory_snapshot_emit.rs:251`（`item_view_from_instance` 直接 clone template_id，无 registry 查询，只读验证不改）/ `server/src/persistence/mod.rs`（库存反序列化无 item template 强查，只读验证不改）/ plan §P2 交付物 4。

### #4 trade_scale 连带处置

**决议**：
1. **P2 不删，P3 删除**：`trade_scale`（`workbench_materials.toml:771`，配方 `"workbench.tool.trade_scale"`）原本未在 6 个经济删除清单内；PR #475 已按用户「工具 4 直接删除」把它纳入 P3 工具类删除。
2. 理由：`trade_scale_stand` 配方 #85 消耗 `trade_scale`。若 P2 删 stand 而保留 scale，会留下「交易秤本体可合成但唯一用途已删」的新僵尸；因此 P3 必须把 `trade_scale` 模板 / 配方 #9 与 `trade_scale_stand` 模板 / 配方 #85 同 PR 原子删。
3. 边界：P2 commit 阶段保留 `trade_scale`，避免 P2 范围漂移；P3 commit 阶段删除 `trade_scale.png`，并用测试断言 stand/scale 双清零。

**落点**：`server/assets/items/workbench_materials.toml:771`（P3 删除）/ `server/src/craft/workbench_recipes.rs` `// #9` + `// #85`（P3 同 PR 删除）/ plan §P2 交付物 5 + §P3。

### #5 camouflage_net 驻地遮蔽

**决议**：
1. **本 plan 不实现驻地遮蔽**——camouflage_net 在 v1 中与 disguise_wrap 同为 Fan 档伪皮材料（#1 决议），无放置形态。
2. 理由：遮蔽网若走真实方块放置依赖 `plan-workbench-place-runtime-v1`（`block_item_to_state` 仅映射 6 种原版方块，camouflage_net 不在其中，研究报告风险点 5）；若走 ECS Component 形态则需新设计，两者均超本"消杀/接通"plan 范围。
3. 后续：若将来要做驻地遮蔽，另立 plan 或并入放置类 plan 族，届时 camouflage_net 可从 Fan 档升级为遮蔽道具。已登记 reminder.md 待办。

**落点**：plan §P0 视听规格（明确不做差异化）/ reminder.md（后续待办）。

---

（本 plan scope = 4 PR，consume-plan 按 P0→P1→P2→P3 拆 PR；P2/P3 均涉及删除与全仓 grep 守门，建议最后串行，避免与 P0/P1 接通 / 改产物冲突。）

## Finish Evidence

### 落地清单

- **P0：蜕壳流伪装道具接通**：
  - `server/src/combat/tuike_v2/state.rs` 将 `disguise_wrap` / `camouflage_net` 映射到 `FalseSkinTier::Fan`。
  - `server/src/combat/tuike.rs` 将两件低成本伪装道具映射到 `FalseSkinKind::SpiderSilk`，打通装备守卫与 realm gate。
  - `server/src/inventory/mod.rs` 增加伪皮装备守卫回归，锁定醒灵拒绝、引气接受的 v1 门槛。
- **P1：放置 kit 换代清理**：
  - `server/src/craft/workbench_recipes.rs` 将 #99/#100 产物改为 `furnace_fantie` / `fan_iron_anvil`。
  - `server/assets/items/workbench_materials.toml` 删除 `furnace_kit_fantie` / `forge_station_kit` 模板。
  - `client/src/main/resources/assets/bong-client/textures/gui/items/` 删除旧代 kit 图标，并补齐 `fan_iron_anvil.png`。
- **P2：6 个经济僵尸物品删除**：
  - `server/assets/items/workbench_materials.toml` 删除 `bone_coin_blank`、`trade_scale_stand`、`price_tag`、`trade_puppet_frame`、`waymark_stone`、`rat_bait` 模板。
  - `server/src/craft/workbench_recipes.rs` 删除对应配方并保留 P2/P3 阶段隔离测试。
  - `server/src/network/inventory_snapshot_emit.rs` 增加删除模板旧实例快照不 panic 且原样保留的回归。
- **P3：材料断链 / 工具类僵尸物品删除**：
  - `server/assets/items/workbench_materials.toml` 删除 `powder_zhu_sha`、`iron_sword_blank`、`stone_spearhead`、`sling_stone`、`sling_weapon`、`stone_hoe`、`mortar_stone`、`heat_gloves`、`trade_scale`、`trade_scale_stand` 模板。
  - `server/src/craft/workbench_recipes.rs` 删除对应配方与 `scroll_workbench_heat_gloves` unlock，并用守门测试确认 `bing_jia_shou_tao` 未被误删。
  - `scripts/images/batch_items.py` 与 `client/src/main/resources/assets/bong-client/textures/gui/items/` 同步删除这批僵尸物品的生成 prompt / PNG 图标。

### 关键 commit / PR

- `78bccaa0368def5ca24f209a2533e791b9271a08`（2026-06-11）— `plan-economy-zombie-cleanup-v1 P0：接通伪装道具蜕壳入口`，PR #484。
- `fcd883efc3ea4b3598afc1ca5fe95160f3f2a8ad`（2026-06-11）— `清理放置组件旧代物品`，PR #485。
- `a3a36b3672a07cd94b9618dba28938be5fbf055c`（2026-06-11）— `清理经济僵尸物品`，PR #488。
- `a4d4c0f509d98d870799815269b7831947ea8ed3`（2026-06-11）— `清理材料工具僵尸物品`，PR #489。

### 测试结果

- PR #484：GitHub `e2e` 通过；`/review` 通过，无阻塞问题。
- PR #485：GitHub `Build resource pack` 与 `e2e` 通过。
- PR #488：GitHub `Build resource pack` 与 `e2e` 通过；`/review` rerun 后通过。
- PR #489：本地通过 `python3 -m py_compile scripts/images/batch_items.py`、`cargo fmt --check`、`cargo clippy -j 2 --all-targets -- -D warnings`、定向 `cargo test` 与全量 `BONG_SKIP_SKIN_PREFETCH=1 CARGO_BUILD_JOBS=2 nice -n 10 ionice -c3 cargo test -j 2`（8210 passed, 0 failed, 1 ignored）；GitHub `Build resource pack`、`CodeRabbit` 与 `e2e` 通过；`/review` 通过，无阻塞问题。

### 跨仓库核验

- **server**：`false_skin_tier_for_item`、`false_skin_kind_for_item`、`material_tool_zombie_templates_and_recipes_are_removed`、`inventory_snapshot_preserves_deleted_template_instances` 均可 grep 命中。
- **client**：旧代 kit、P2/P3 删除 ID 的 PNG 图标已删除；`fan_iron_anvil.png` 已存在；`disguise_wrap.png` / `camouflage_net.png` 保留。
- **agent/schema**：本 plan 删除 / 接通 ID 在 agent schema 与 WorldModel 中无新增契约；无需 schema 变更。
- **scripts**：`scripts/images/batch_items.py` 已删除 P3 僵尸物品 prompt，`py_compile` 通过。

### 遗留 / 后续

- 本 plan 范围内无阻塞遗留。
- `herb_bundle` / `cao_lian` 的临门适配不在本 plan 范围，后续由 `plan-gathering-tool-bind-v1` 承接。
- `camouflage_net` 驻地遮蔽不是本 plan v1 目标；若需要放置形态与驻地遮蔽效果，另立放置/遮蔽玩法 plan。
