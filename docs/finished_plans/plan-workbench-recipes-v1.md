# Bong · plan-workbench-recipes-v1

**末法制作台——基础入门制作站 + 100 份世界观配方 + bbmodel 模型**。在末法残土，什么都要自己搓。没有宗门发你一套行头，没有商铺卖你现成工具。一张制作台，一堆杂料，你能活多久全看你会搓什么。制作台是玩家进入一切玩法循环的入口：采集→加工→制作→使用→交易。100 份配方覆盖生存凡器、材料加工、容器、护甲、武器、修炼辅材、阵法、经济、住所、炼丹/炼器预备十大类，全部严格遵守 worldview 灵质守恒（§八 磨损律：产出 sq ≤ 投入 sq × 0.95）与物理推导。

---

## 世界观锚点

- `worldview.md §二:30` — 真元极易挥发 → 所有涉真元配方 qi_cost > 0 且走 qi_physics ledger
- `worldview.md §四:239` — 体表 16 部位 × 6 档伤口 → 夹板/绷带急救配方（#51-52, #74）
- `worldview.md §五:397` — 攻防七流基础入门物资（暗器坯 #54 / 伪装 #87,#94 / 阵纸 #75-82 / 毒蛊 #69-70）
- `worldview.md §八:806` — 灵物操作磨损（零拷贝生存）→ 加工产出灵质 ≤ 投入灵质 × 0.95
- `worldview.md §九:838` — 封灵骨币半衰期 + 盲盒死信箱 → 骨币铸造 #83 / 死信箱 #37 / 交易容器 #41
- `worldview.md §九:850` — 盲盒死信箱：箱遭非对应破坏→阵法启动物品化灰
- `worldview.md §十:866` — 灵气零和 SPIRIT_QI_TOTAL=100 + 搜打撤 → 采集工具/容器/急救品
- `worldview.md §十四:1275` — 完整体验循环所需全部基础物资

**library 锚点**：
- `docs/library/ecology/末法药材十七种.json` — 灵草配方材料来源（#8, #66-71）
- `docs/library/ecology/矿物录.json` — 矿物加工/阵法配方材料（#21, #29, #36, #77, #92）
- `docs/library/ecology/异兽三形考.json` — 兽产加工：苔鞣法/鼠尾油（#16-18, #24, #72）
- `docs/library/ecology/绝地草木拾遗.json` — 特殊采集手套材料（#10, #11, #25）
- `docs/library/ecology/灵物磨损笔记.json` — 磨损律推导依据（#27, #31-33, #38, #73, #84）

---

## 交叉引用（已完成 plan）

- `plan-craft-v1` ✅ — 手搓制作底盘（CraftRecipe / CraftRegistry / CraftSession 状态机），本 plan 扩展 `station` 字段
- `plan-craft-ux-v1` ✅ — 手搓 UI，WorkbenchScreen 复用 CraftScreen 框架
- `plan-forge-v1` ✅ — 锻造底盘（#100 forge_station_kit 为其进料）
- `plan-alchemy-v1` ✅ — 炼丹底盘（#99 furnace_kit_fantie 为其进料）
- `plan-qi-physics-v1` ✅ — 守恒律 / QiTransfer（qi_cost>0 配方走此底盘）
- `plan-shelflife-v1` ✅ — 物品腐败（产出物品保质期初始化）
- `plan-backpack-equip-v1` ✅ — 背包/容器系统（容器类配方产出走此底盘）
- `plan-weapon-v1` ✅ — 武器系统（武器类配方产出走此底盘）
- `plan-botany-v1/v2` ✅ — 植物系统（灵草材料来源）
- `plan-gathering-ux-v1` ✅ — 采集 UX（采集产物 → 制作台进料）
- `plan-armor-v1` ✅ — 护甲系统（#43-50 护甲配方产出走此底盘）

**交叉引用（skeleton / active）**：
- `plan-onboarding-loop-v1` active — 新手引导循环，制作台是核心入门节点
- `plan-model-asset-v1` skeleton — 模型资产管线，bbmodel 走此流程

---

## 接入面 Checklist

- **进料**：
  - `inventory::PlayerInventory` — 材料消耗、产出写入
  - `botany::PlantKindRegistry` — 灵草 ID 校验（ci_she_hao / ning_mai_cao / ...）
  - `mineral::MineralRegistry` — 矿物 ID 校验（fan_tie / ling_tie / ...）
  - `gathering::*` — 采集产物 → 制作台进料（grass_fiber / stone_chunk / ...）
  - `lingtian::*` — hoe_iron 引用
  - `botany_v2::*` — bai_yan_peng（白盐蓬）引用
  - `fauna::*` — 兽产（shu_gu / rat_tail / ash_spider_silk / raw_beast_hide）
  - `qi_physics::ledger::WorldQiAccount` — qi_cost 走 `QiTransfer { reason: Crafting }`
  - `cultivation::{Cultivation, QiColor}` — realm / qi_color 软 gate 校验
  - `shelflife::*` — 产出物品保质期初始化
- **出料**：
  - `inventory` — 产出 ItemInstance
  - `craft::events::CraftCompletedEvent` — 制作完成事件广播
  - `network::craft_emit` — 客户端同步
  - Redis `bong:craft/outcome` — Agent narration 素材
- **共享类型/event**：
  - **复用** `craft::recipe::{CraftRecipe, CraftCategory, CraftRequirements, RecipeId}`
  - **新增** `CraftRecipe.station: Option<CraftStationKind>` — `None` = 手搓，`Some(Workbench)` = 制作台
  - **新增** `CraftStationKind::Workbench` 枚举
- **跨仓库契约**：
  - server: `WorkbenchBlock`（方块 ECS）、`CraftStationKind::Workbench`、`register_workbench_recipes()`
  - client: `WorkbenchScreen`（右键打开）、`bong:models/block/workbench.json`（bbmodel export）
  - agent: `bong:craft/outcome` 包含 `station: "workbench"` 字段
- **qi_physics 锚点**：
  - 所有 `qi_cost > 0` 配方走 `QiTransfer { from: Player, to: Zone, amount: qi_cost, reason: Crafting }`
  - 真元经制作过程逸散回 zone（对齐 §二「极易挥发」）
  - **不新增物理常数**——复用现有 Crafting reason

---

## §0 阶段总览

| 阶段 | 内容 | 状态 |
|---|---|---|
| **P0** ✅ 2026-05-22 | 制作台方块定义 + bbmodel 模型 + 服务端方块交互 | PR-1 #298, PR-2 #301, PR-3 #302 |
| **P1** ✅ 2026-05-22 | 100 配方 + 新增 ItemTemplate 补全（~80 新模板） | PR-1 #298 |
| **P2** ✅ 2026-05-22 | Server 端 WorkbenchRecipe 注册 + CraftRegistry 扩展 + 100 配方注册 | PR-2 #301 |
| **P3** ✅ 2026-05-22 | Client UI（WorkbenchScreen）+ bbmodel 渲染 + 音效/VFX 规格 | PR-3 #302 |
| **P4** ✅ 2026-05-22 | 饱和测试（100 pin + 守恒律 + session + 物理推导 + e2e） | PR-4 |
| **P5** ✅ 2026-05-22 | e2e 集成 + 归档 | PR-4 |

---

## P0：制作台方块 + bbmodel 模型

### P0.1 制作台方块（Server 端）

**方块 ID 映射**：复用 MC 1.20.1 `minecraft:crafting_table` block state。Server 在玩家右键凡铁制作台 item 时，在目标位置 spawn 对应 block + ECS entity `WorkbenchBlock`。

```rust
// server/src/craft/workbench.rs (新文件)
#[derive(Component)]
pub struct WorkbenchBlock {
    pub placed_by: Entity,
    pub placed_at_tick: u64,
}
```

**交互**：玩家对 WorkbenchBlock 右键 → Server 发 `WorkbenchOpenPayload` → Client 打开 WorkbenchScreen。

**放置/拆除**：
- 放置：消耗背包中 `workbench_item` × 1 → spawn block + entity
- 拆除：左键长按 3s → 回收 `workbench_item` × 1 → despawn

### P0.2 制作台 bbmodel 模型

**设计规格**（基于原版制作台 16×16×16 扩展）：

```
基础几何：原版 crafting_table 方块（16×16×16 cube）
└── 顶面：末法残土制作台纹理
    ├── 3×3 浅刻网格（骨白色底 #E8DCC8 + 褐色线 #5C4A3A）
    ├── 中心 4×4 px 阵纹标记（丹砂红 #8B3A3A，浅浮雕 0.5px 凸起）
    └── 四角 2×2 px 骨钉（骨白 #D4C8B0，1px 凸起）
└── 侧面：灵木纹理
    ├── 深褐底色 #3E2E1E + 细微灵气脉络（灰蓝 #6B7B8A，0.5px 线条随机分布）
    ├── 正面中部：小型铁拉环（凡铁色 #6E6E6E，2×3 px 凹槽 + 1×1 px 环）
    └── 背面/左右面：纯灵木纹理
└── 底部：灰石基座纹理（#7A7A7A 石块质感）
└── 额外几何（bbmodel 扩展，非原版 cube）：
    ├── 四根骨钉（每根 1×1×2 cube，从顶面四角向上凸 2px）
    ├── 正面拉环（1×1×1 cube + 1px 厚环形，从正面中部外凸 1px）
    └── 底部石基（16×1×16 cube，y=-1 到 y=0，石灰色）
```

**bbmodel 文件**：`local_models/Workbench.bbmodel`（Blockbench JSON）
**导出**：`client/src/main/resources/assets/bong/models/block/workbench.json`（MC JSON model）
**纹理**：`client/src/main/resources/assets/bong/textures/block/workbench_top.png` / `workbench_side.png` / `workbench_front.png` / `workbench_bottom.png`（各 16×16 px）

### P0.3 制作台物品定义

```toml
# server/assets/items/core.toml 追加
[[item]]
id = "workbench_item"
name = "制作台"
category = "misc"
grid_w = 2
grid_h = 2
base_weight = 6.0
rarity = "common"
spirit_quality_initial = 0.0
description = "灵木板钉上凡铁骨钉的粗木台子。放下就能搓东西。"
```

### P0.4 制作台自身的制作配方（手搓）

制作台本身由手搓制作（现有 CraftCategory::Tool）：

```
ID: craft.tool.workbench
材料: spirit_wood × 4 + iron_ingot × 2 + shu_gu × 2
qi_cost: 0.0（纯凡物组装）
time: 60s (1200 ticks)
output: workbench_item × 1
requirements: 无
unlock: 默认解锁（入门配方）
```

---

## P1：100 配方 + 新增 ItemTemplate

### P1.0 新增 ItemTemplate（~80 个）

以下物品在现有 `server/assets/items/` 中不存在，需新建 `server/assets/items/workbench_materials.toml`：

#### 基础材料

> **ID 校验注**：`stone_chunk`（碎石）、`raw_beast_hide`（生兽皮）、`shu_gu`（鼠骨，通用兽骨代表）均已在现有 toml 中确认。矿物原料（fan_tie/cu_tie 等）在 mineral 模块运行时注册，本 plan 补充对应 ItemTemplate toml 以确保可查。

| ID | 名称 | category | grid | weight | rarity | sq | 描述 |
|----|------|----------|------|--------|--------|----|----|
| rough_cloth | 粗布 | misc | 1×1 | 0.2 | common | 0.0 | 蛛丝编织的粗糙布片，防潮包扎皆可 |
| tanned_hide | 熟皮 | misc | 1×1 | 0.5 | common | 0.0 | 灰烬苔鞣制的兽皮，柔韧防水 |
| bone_chip_mat | 骨片 | misc | 1×1 | 0.1 | common | 0.0 | 兽骨劈碎的薄片，削刮缝补用 |
| bone_meal_mat | 骨粉 | misc | 1×1 | 0.15 | common | 0.0 | 兽骨碾磨的粉末，灵田肥料或丹药辅料 |
| spirit_charcoal | 灵木炭 | misc | 1×1 | 0.3 | common | 0.3 | 灵木闷烧的碳块，保留微灵可作灵火引 |
| rat_tail_oil | 鼠尾油 | misc | 1×1 | 0.15 | common | 0.33 | 噬元鼠尾巴炼出的油脂，防水润滑（rat_tail sq=0.70×3=2.1→oil sq=0.33≤2.0） |
| salt_crystal | 盐蓬晶 | misc | 1×1 | 0.2 | common | 0.15 | 白盐蓬析出的晶体，诱鼠兼灵石代用 |
| spider_silk_cord | 蛛丝绳 | misc | 1×1 | 0.1 | common | 0.5 | 灰烬蛛丝搓成的韧绳，比草绳结实十倍 |
| wood_plank | 木板 | misc | 1×1 | 0.6 | common | 0.0 | 灵木锯开的板材，建造或容器基材 |
| dried_grass | 干草 | misc | 1×1 | 0.05 | common | 0.0 | 晒干的野草，铺床引火垫药 |
| zhenfa_blank_array | 阵法白纸 | misc | 1×1 | 0.05 | common | 0.15 | 丹砂油浸粗布阵纸（rat_tail_oil sq=0.33→×0.95/2=0.157，取0.15） |
| powder_dan_sha | 丹砂粉 | misc | 1×1 | 0.1 | common | 0.0 | 丹砂碾制的细粉，画符染色调和用 |
| powder_zhu_sha | 朱砂粉 | misc | 1×1 | 0.1 | uncommon | 0.13 | 朱砂碾磨的赤粉，高阶丹药引料（zhu_sha sq=0.3→研磨×0.95/2=0.14，取0.13） |
| herb_bundle | 灵草束 | herb | 1×1 | 0.5 | common | 0.80 | 灵草扎束延缓散灵（spirit_grass sq=0.85×5=4.25→束sq=0.80≤4.04） |

#### 矿物原料 ItemTemplate（mineral 模块运行时注册矿脉/方块逻辑，本 plan 补 toml 定义 inventory 可查的掉落物 ItemTemplate，两者注册维度不同不冲突）

> 写入 `server/assets/items/minerals.toml`（新建文件，仅定义可拾取矿物掉落物的 ItemTemplate）

| ID | 名称 | category | grid | weight | rarity | sq | 描述 |
|----|------|----------|------|--------|--------|----|----|
| fan_tie | 凡铁矿 | misc | 1×1 | 2.0 | common | 0.0 | 地表至 y=0 的凡铁矿石，冶炼得粗铁锭 |
| cu_tie | 粗铁矿 | misc | 1×1 | 2.5 | common | 0.0 | y=-32~-64 的粗铁矿，杂质多但量大 |
| ling_tie | 灵铁 | misc | 1×1 | 1.8 | uncommon | 0.6 | 血谷矿脉产，储灵兵胎或密封容器(library 矿物录) |
| ling_jing | 灵晶 | misc | 1×1 | 0.5 | rare | 0.8 | 青云/血谷产，法宝阵眼核心(library 矿物录) |
| dan_sha | 丹砂 | misc | 1×1 | 0.3 | common | 0.0 | 洞穴/红岩产，Mellow 辛度调和(library 矿物录) |
| zhu_sha | 朱砂 | misc | 1×1 | 0.3 | uncommon | 0.3 | 火山/血谷产，高阶药引(library 矿物录) |
| xiong_huang | 雄黄 | misc | 1×1 | 0.3 | common | 0.0 | 洞穴/尸骸产，驱邪解蛊(library 矿物录) |

#### 容器与装具

| ID | 名称 | category | grid | weight | rarity | sq | 描述 |
|----|------|----------|------|--------|--------|----|----|
| herb_vial | 药瓶 | container | 1×1 | 0.15 | common | 0.0 | 灵晶碎封口的小瓶，密封保灵 |
| sealed_vial | 密封药瓶 | container | 1×1 | 0.2 | uncommon | 0.4 | 蛛丝绳缠封灵力加固（cord sq=0.5→sealed sq=0.4≤0.5×0.95）保质期翻倍 |
| herb_pouch | 灵草囊 | container | 1×2 | 0.3 | common | 0.0 | 粗布缝成的草药袋，采集时直接塞入减少磨损 |
| coin_box | 骨币匣 | container | 1×1 | 0.8 | common | 0.0 | 木匣内壁涂丹砂隔灵，骨币少漏 |
| projectile_bag | 暗器袋 | container | 1×2 | 0.4 | common | 0.0 | 兽皮内衬的暗器袋，骨刺不戳破 |
| spirit_seal_box | 封灵匣 | container | 1×1 | 1.0 | uncommon | 0.0 | 灵铁箍的密封小匣，高灵物短存最佳 |
| dead_drop_box | 死信箱 | container | 2×2 | 3.0 | uncommon | 0.0 | 埋地交易箱，内设防砸阵纹 |
| moisture_guard | 防潮包 | misc | 1×1 | 0.1 | common | 0.0 | 灰烬苔裹布包，吸湿防霉 |
| ore_sack | 矿石袋 | container | 1×2 | 0.3 | common | 0.0 | 粗布加底皮的矿石袋，耐磨 |
| water_skin | 水囊 | misc | 1×1 | 0.4 | common | 0.0 | 兽皮缝的水袋，远征不渴 |
| trade_crate | 货箱 | container | 2×2 | 2.0 | common | 0.0 | 六面木板钉成的大箱，集市搬运用 |
| herb_crate | 灵草箱 | container | 2×2 | 1.5 | common | 0.0 | 木板内衬粗布的存放箱，灵草批量少磨损 |
| sealed_envelope | 密封信封 | misc | 1×1 | 0.05 | common | 0.0 | 鼠尾油封口的粗布信封，拆即毁 |

#### 工具与装备

| ID | 名称 | category | grid | weight | rarity | sq | 描述 |
|----|------|----------|------|--------|--------|----|----|
| stone_pickaxe | 石镐 | tool | 1×2 | 2.0 | common | 0.0 | 碎石绑木柄，挖矿入门 |
| stone_axe | 石斧 | tool | 1×2 | 1.8 | common | 0.0 | 碎石夹木柄，伐木入门 |
| stone_hoe | 石锄 | tool | 1×2 | 1.5 | common | 0.0 | 碎石绑木柄，翻土入门 |
| mortar_stone | 研钵 | tool | 1×1 | 2.0 | common | 0.0 | 石块凿成的药臼，碾草磨粉 |
| heat_gloves | 绝热手套 | tool | 1×1 | 0.3 | uncommon | 0.3 | 蛛丝内衬的厚手套，采焦脉藤不灼伤 |

#### 急救与修炼

| ID | 名称 | category | grid | weight | rarity | sq | 描述 |
|----|------|----------|------|--------|--------|----|----|
| arm_splint | 夹板·臂 | misc | 1×1 | 0.3 | common | 0.0 | 木板布条绑的夹板，固定臂骨折 |
| leg_splint | 夹板·腿 | misc | 1×1 | 0.5 | common | 0.0 | 加长木板夹板，固定腿骨折 |
| bandage | 止血绷带 | misc | 1×1 | 0.1 | common | 0.0 | 粗布撕成条裹伤口，压迫止血 |
| meridian_salve | 养经膏 | pill | 1×1 | 0.1 | uncommon | 0.6 | 养经苔熬油制膏，外敷修复微裂经脉 |
| anti_gu_powder | 解蛊散 | pill | 1×1 | 0.1 | uncommon | 0.7 | 解蛊蕊雄黄研末，紧急解毒蛊 |
| qingzhuo_powder | 清浊散 | pill | 1×1 | 0.1 | uncommon | 0.6 | 清浊草丹砂研末，克染色浊乱 |
| calming_tea | 安神茶 | pill | 1×1 | 0.2 | common | 0.4 | 安神果煮水，镇心安神 |
| meditation_mat | 蒲团 | misc | 2×2 | 1.5 | common | 0.0 | 干草粗布叠成的打坐垫，坐上多静半分 |
| qi_guide_talisman | 灵气引导符 | misc | 1×1 | 0.05 | uncommon | 0.13 | 丹砂画引灵符（zhenfa sq=0.15→×0.95=0.14，取0.13） |
| meridian_rubbing | 经脉图拓片 | misc | 1×1 | 0.05 | uncommon | 0.4 | 灵草汁拓印的经脉走向参考图 |
| ningmai_prep_kit | 凝脉散预制包 | misc | 1×1 | 0.3 | uncommon | 0.5 | 凝脉散所需原料预包装，送炼丹炉即用 |
| huiyuan_decoction | 回元芷煎汤 | pill | 1×1 | 0.3 | uncommon | 0.5 | 回元芷简易煎法制剂，非炼丹产物 |
| rat_bait | 鼠群诱饵 | misc | 1×1 | 0.2 | common | 0.1 | 盐蓬晶裹粗布的诱鼠包 |
| spirit_stone_rack | 灵石架 | misc | 1×1 | 1.0 | common | 0.0 | 木架铁钉挂灵石，减接触磨损 |

#### 阵法与防御

| ID | 名称 | category | grid | weight | rarity | sq | 描述 |
|----|------|----------|------|--------|--------|----|----|
| array_flag_basic | 阵旗·凡 | misc | 1×2 | 0.4 | common | 0.0 | 粗布丹砂的入门阵旗，凡物无灵质 |
| array_eye_basic | 阵眼·凡 | misc | 1×1 | 0.5 | uncommon | 0.5 | 灵晶碎镶铁的阵法核心，入门级 |
| trip_wire | 预警绊线 | misc | 1×1 | 0.1 | common | 0.0 | 蛛丝绳挂铁针，碰断即响 |
| beast_trap | 困兽圈 | misc | 1×1 | 1.2 | common | 0.0 | 铁夹绳套的陷阱，夹住野兽腿 |
| qi_scatter_bead | 散真元珠 | misc | 1×1 | 0.1 | uncommon | 0.3 | 灵晶碎粉混骨粉封珠，散布真元干扰追踪 |
| gather_array_base | 聚灵阵基座 | misc | 2×2 | 4.0 | rare | 0.5 | 灵铁阵纸灵晶组成的聚灵阵核心 |
| camouflage_net | 伪装网 | misc | 2×2 | 0.8 | common | 0.0 | 粗布编灵草的遮蔽网，藏人藏物 |

#### 基础武器

| ID | 名称 | category | grid | weight | rarity | sq | 描述 |
|----|------|----------|------|--------|--------|----|----|
| stone_spearhead | 石矛头 | misc | 1×1 | 0.5 | common | 0.0 | 碎石磨尖的矛头，绑木棍成矛 |
| sling_weapon | 弹弓 | weapon | 1×1 | 0.3 | common | 0.0 | 兽皮兜木叉弹弓，甩石打鼠 |
| sling_stone | 飞石 | misc | 1×1 | 0.3 | common | 0.0 | 磨圆的弹弓石，打不远但打得准 |
| wooden_shield | 木盾 | armor | 1×2 | 3.0 | common | 0.0 | 木板钉铁的粗盾，挡一两下 |
| bone_shield | 骨盾 | armor | 1×2 | 2.5 | common | 0.0 | 兽骨编绳的圆盾，轻但脆 |
| iron_sword_blank | 凡铁剑胎 | misc | 1×2 | 2.5 | common | 0.0 | 凡铁粗锻的剑形坯料，送锻造台精锻 |
| iron_dagger | 凡铁匕首 | weapon | 1×1 | 0.8 | common | 0.0 | 近身防御短刃，小巧好藏 |
| stone_knife | 石刃 | weapon | 1×1 | 0.5 | common | 0.0 | 碎石磨薄的切割器，最入门 |
| bone_spike_crude | 粗骨刺 | weapon | 1×1 | 0.15 | common | 0.0 | 兽骨粗削尖的低档投掷物，打不死但打得疼 |
| wooden_club | 木棍 | weapon | 1×2 | 0.8 | common | 0.0 | 灵木削成的凡物木棍，近战入门（非灵器） |

#### 经济

| ID | 名称 | category | grid | weight | rarity | sq | 描述 |
|----|------|----------|------|--------|--------|----|----|
| bone_coin_blank | 骨币胚 | misc | 1×1 | 0.1 | common | 0.0 | 兽骨切片阵纸贴面的空白骨币，灌真元才有价 |
| trade_scale | 交易秤 | tool | 1×1 | 0.8 | common | 0.0 | 铁杆木盘的简易秤，称骨币灵草 |
| waymark_stone | 标记石 | misc | 1×1 | 0.4 | common | 0.0 | 丹砂画记号的石头，标路不迷 |
| disguise_wrap | 伪装包裹 | misc | 1×1 | 0.2 | common | 0.0 | 灰烬苔裹布的包装，掩盖灵物气息 |
| trade_scale_stand | 交易秤台 | tool | 1×1 | 1.5 | common | 0.0 | 秤固定在木架上，摊位交易免手持 |
| trade_puppet_frame | 交易傀儡骨架 | misc | 2×2 | 5.0 | rare | 0.3 | 兽骨蛛丝铁关节的人形骨架，注灵后可充当游商 |
| niche_repair_kit | 灵龛修补料 | misc | 1×1 | 1.5 | uncommon | 0.3 | 碎石灵铁混合料，修补损坏灵龛 |
| price_tag | 标价签 | misc | 1×1 | 0.01 | common | 0.0 | 丹砂写价的粗布签，贴在交易品上 |

#### 住所

| ID | 名称 | category | grid | weight | rarity | sq | 描述 |
|----|------|----------|------|--------|--------|----|----|
| torch_item | 火把 | misc | 1×1 | 0.2 | common | 0.1 | 木柄缠油炭的火把，照明一时辰 |
| lantern_item | 灯笼 | misc | 1×1 | 0.6 | common | 0.2 | 铁框灵晶芯的长明灯，灵气耗尽才灭 |
| door_bolt | 门闩 | misc | 1×1 | 1.5 | common | 0.0 | 铁杆木板的门闩，堵门挡贼 |
| moisture_base | 防潮地基 | misc | 2×1 | 3.0 | common | 0.0 | 石块灰烬苔铺的地基，架上不潮 |
| simple_bed | 简易床铺 | misc | 2×2 | 4.0 | common | 0.0 | 木板干草粗布叠的床，总比地上强 |
| window_grate | 窗栅 | misc | 1×1 | 2.0 | common | 0.0 | 凡铁条焊的栅栏窗，通风防贼 |
| niche_base | 灵龛基座 | misc | 2×2 | 6.0 | rare | 0.5 | 龛石灵铁木台组合的永久复活点基座 |

#### 炼丹 / 炼器预备

| ID | 名称 | category | grid | weight | rarity | sq | 描述 |
|----|------|----------|------|--------|--------|----|----|
| furnace_kit_fantie | 凡铁炉组件 | misc | 2×2 | 8.0 | common | 0.0 | 凡铁拼焊石块砌底的炉胎。放置后消耗此 item 并 spawn furnace ECS entity（灵质由 zone 灵气注入，不凭空生成） |
| forge_station_kit | 锻造台组件 | misc | 2×2 | 10.0 | common | 0.0 | 凡铁砧灵木架石块基的锻造台料包 |

---

### P1.1 — 100 配方清单

> **物理推导原则**：
> 1. qi_cost = 0 → 纯凡物物理操作（切割/编织/研磨/组装），不涉及真元
> 2. qi_cost > 0 → 真元灌注步骤（画符/封灵/注阵/渡性），走 qi_physics ledger
> 3. 产出灵质 ≤ 投入灵质总和 × 0.95 — 对齐 worldview §八 灵物磨损（加工必损 ≥5%）
> 4. 时间 ∝ 复杂度：切割(10-20s) < 研磨(20-30s) < 编织(30-60s) < 组装(45-120s) < 真元灌注(25-180s，含微灵操作)
> 5. 凡物配方（qi=0）默认解锁；涉真元配方需残卷/师承/顿悟解锁

**表格列定义**：
- `#` = 序号
- `ID` = recipe_id（workbench. 前缀省略）
- `名称` = display_name
- `材料` = `(template_id × count)` 列表
- `qi` = qi_cost（真元投入）
- `t` = 制作时间（秒）
- `产出` = `template_id × count`
- `cat` = CraftCategory：T=Tool, M=Misc, C=Container, A=ArmorCraft, AnqiCarrier=暗器, DuguPotion=毒蛊, ZhenfaTrap=阵法, TuikeSkin=退壳伪装
- `unlock` = 🔓=默认解锁, 📜=残卷, 👤=师承, 💡=顿悟
- `物理依据` = worldview / library 出处

---

#### 一、生存凡器 (12)

| # | ID | 名称 | 材料 | qi | t | 产出 | cat | unlock | 物理依据 |
|---|-----|------|------|-----|---|------|-----|--------|---------|
| 1 | tool.stone_pickaxe | 石镐 | stone_chunk×3 + wood_handle×1 | 0 | 25 | stone_pickaxe×1 | T | 🔓 | 凡物组装，§十四 基础采矿 |
| 2 | tool.stone_axe | 石斧 | stone_chunk×2 + wood_handle×1 | 0 | 25 | stone_axe×1 | T | 🔓 | 凡物组装，§十四 基础伐木 |
| 3 | tool.stone_hoe | 石锄 | stone_chunk×1 + wood_handle×1 | 0 | 20 | stone_hoe×1 | T | 🔓 | 凡物组装，灵田翻土 |
| 4 | tool.pickaxe_iron | 铁镐 | iron_ingot×3 + wood_handle×1 | 0 | 45 | pickaxe_iron×1 | T | 🔓 | 凡铁重锻需时长，§十 矿石采集 |
| 5 | tool.axe_iron | 铁斧 | iron_ingot×2 + wood_handle×1 | 0 | 40 | axe_iron×1 | T | 🔓 | 凡铁锻制 |
| 6 | tool.hoe_iron | 铁锄 | iron_ingot×1 + wood_handle×1 | 0 | 35 | hoe_iron×1 | T | 🔓 | 灵田精耕 |
| 7 | tool.mortar | 研钵 | stone_chunk×4 | 0 | 40 | mortar_stone×1 | T | 🔓 | 石块凿空，§十四 药粉研磨工具 |
| 8 | tool.sickle | 草镰 | iron_ingot×2 + wood_handle×1 | 0 | 40 | cao_lian×1 | T | 🔓 | 收割灵草专用，library 末法药材十七种 |
| 9 | tool.trade_scale | 交易秤 | iron_ingot×2 + wood_plank×1 | 0 | 45 | trade_scale×1 | T | 🔓 | §九 经济，面对面交易辅具 |
| 10 | tool.heat_gloves | 绝热手套 | ash_spider_silk×3 + tanned_hide×2 | 0 | 60 | heat_gloves×1 | T | 📜 | 需残卷知蛛丝耐热编法(library 绝地草木 焦脉藤采集需此) |
| 11 | tool.ice_gauntlet | 冰甲手套 | tanned_hide×2 + iron_ingot×1 | 0 | 50 | bing_jia_shou_tao×1 | T | 🔓 | 凡铁薄片衬皮(tools.toml 现有 ID)，采寒系植物防冻伤 |
| 12 | tool.scraper | 刮刀 | iron_ingot×1 + bone_chip_mat×1 | 0 | 30 | gua_dao×1 | T | 🔓 | 凡铁刃骨柄，兽皮刮毛用 |

#### 二、材料加工 (18)

| # | ID | 名称 | 材料 | qi | t | 产出 | cat | unlock | 物理依据 |
|---|-----|------|------|-----|---|------|-----|--------|---------|
| 13 | process.wood_handle | 木柄 | spirit_wood×1 | 0 | 15 | wood_handle×4 | M | 🔓 | 灵木削制，sq 损失(灵→凡) |
| 14 | process.wood_plank | 木板 | spirit_wood×1 | 0 | 15 | wood_plank×2 | M | 🔓 | 灵木锯制 |
| 15 | process.rope | 草绳 | grass_fiber×3 | 0 | 20 | grass_rope×1 | M | 🔓 | 草根搓绳 |
| 16 | process.spider_cord | 蛛丝绳 | ash_spider_silk×4 | 0 | 30 | spider_silk_cord×1 | M | 🔓 | 蛛丝搓绳，强度远超草绳(library 异兽三形考) |
| 17 | process.rough_cloth | 粗布 | ash_spider_silk×5 | 0 | 45 | rough_cloth×1 | M | 🔓 | 蛛丝编织，灰烬蛛丝韧极(§七) |
| 18 | process.tanned_hide | 熟皮 | raw_beast_hide×1 + hui_jin_tai×1 | 0 | 60 | tanned_hide×1 | M | 🔓 | 灰烬苔鞣制(library 异兽三形考 苔鞣法) |
| 19 | process.bone_chip | 骨片 | shu_gu×1 | 0 | 10 | bone_chip_mat×4 | M | 🔓 | 兽骨劈碎 |
| 20 | process.bone_meal | 骨粉 | shu_gu×2 | 0 | 25 | bone_meal_mat×1 | M | 🔓 | 兽骨碾磨 |
| 21 | process.iron_ingot | 粗铁锭 | cu_tie×2 | 0 | 50 | iron_ingot×1 | M | 🔓 | 粗铁粗炼(library 矿物录·凡铁)，凡物加热去杂 |
| 22 | process.spirit_charcoal | 灵木炭 | spirit_wood×2 | 0 | 60 | spirit_charcoal×3 | M | 🔓 | 灵木闷烧(§二 灵气挥发：炭保留微灵 30%) |
| 23 | process.dried_grass | 干草 | grass_fiber×5 | 0 | 20 | dried_grass×3 | M | 🔓 | 野草晾晒 |
| 24 | process.rat_tail_oil | 鼠尾油 | rat_tail×3 | 0 | 45 | rat_tail_oil×1 | M | 🔓 | 噬元鼠尾熬油(library 异兽三形考 鼠尾压差储真元→油脂含微灵) |
| 25 | process.salt_crystal | 盐蓬晶 | bai_yan_peng×3 | 0 | 35 | salt_crystal×1 | M | 🔓 | 白盐蓬析晶(library 绝地草木·白盐蓬 白昼析晶) |
| 26 | process.clay_pot | 陶罐 | stone_chunk×3 | 0 | 40 | clay_pot×1 | M | 🔓 | 碎石碾泥晒干(现有 ID clay_pot) |
| 27 | process.herb_bundle | 灵草束 | spirit_grass×5 | 0 | 10 | herb_bundle×1 | M | 🔓 | 扎束延缓散灵(§八 磨损减免：扎束=一次操作，后续取用算束不算草) |
| 28 | process.dan_sha_powder | 丹砂粉 | dan_sha×1 | 0 | 15 | powder_dan_sha×3 | M | 🔓 | 研钵碾制(library 矿物录·丹砂 Mellow 调和) |
| 29 | process.zhu_sha_powder | 朱砂粉 | zhu_sha×1 | 0 | 25 | powder_zhu_sha×2 | M | 🔓 | 研钵碾制(library 矿物录，产出为 plan-alchemy-v1 高阶丹药引料预留) |
| 30 | process.needle_batch | 铁针 | iron_ingot×1 | 0 | 20 | iron_needle×5 | M | 🔓 | 粗铁拉丝敲针(现有 ID iron_needle) |

#### 三、容器与存储 (12)

| # | ID | 名称 | 材料 | qi | t | 产出 | cat | unlock | 物理依据 |
|---|-----|------|------|-----|---|------|-----|--------|---------|
| 31 | container.herb_vial | 药瓶 | stone_chunk×2 + iron_ingot×1 | 0 | 35 | herb_vial×2 | C | 🔓 | 石磨瓶+铁扣盖(§八 灵物磨损：密封=减磨损) |
| 32 | container.sealed_vial | 密封药瓶 | herb_vial×1 + spider_silk_cord×1 | 1 | 25 | sealed_vial×1 | C | 📜 | 蛛丝缠封+微灵加固(shelflife 保质期×2) |
| 33 | container.herb_pouch | 灵草囊 | rough_cloth×2 + grass_rope×1 | 0 | 40 | herb_pouch×1 | C | 🔓 | 采集直塞减磨损(§八 灵物磨损) |
| 34 | container.herb_crate | 灵草箱 | wood_plank×4 + rough_cloth×1 | 0 | 45 | herb_crate×1 | C | 🔓 | 灵草批量存放(§八 灵物磨损：整箱少碰=减磨) |
| 35 | container.projectile_bag | 暗器袋 | tanned_hide×2 + grass_rope×1 | 0 | 40 | projectile_bag×1 | C | 🔓 | §五 器修·暗器流 载体存储 |
| 36 | container.seal_box | 封灵匣 | wood_plank×4 + ling_tie×1 | 3 | 90 | spirit_seal_box×1 | C | 📜 | 灵铁箍封(library 矿物录·灵铁 储灵兵胎→密封灵物) |
| 37 | container.dead_drop | 死信箱 | wood_plank×6 + iron_ingot×2 + zhenfa_blank_array×1 | 4 | 120 | dead_drop_box×1 | C | 📜👤 | §九 盲盒死信箱(阵纹=防砸自爆) |
| 38 | container.moisture_guard | 防潮包 | rough_cloth×1 + hui_jin_tai×2 | 0 | 25 | moisture_guard×2 | C | 🔓 | 灰烬苔吸湿(library 异兽三形考 苔鞣法) |
| 39 | container.ore_sack | 矿石袋 | rough_cloth×2 + tanned_hide×1 | 0 | 30 | ore_sack×1 | C | 🔓 | 粗布加底皮耐磨 |
| 40 | container.water_skin | 水囊 | tanned_hide×1 + grass_rope×1 | 0 | 30 | water_skin×1 | C | 🔓 | 远征基础(§十四 赶路补水) |
| 41 | container.trade_crate | 货箱 | wood_plank×6 + iron_ingot×2 | 0 | 65 | trade_crate×1 | C | 🔓 | §九 面对面交易 批量搬运 |
| 42 | container.sealed_envelope | 密封信封 | rough_cloth×1 + rat_tail_oil×1 | 0 | 20 | sealed_envelope×3 | C | 🔓 | 油封口(§十一 信息传递) |

#### 四、基础护甲 (10)

| # | ID | 名称 | 材料 | qi | t | 产出 | cat | unlock | 物理依据 |
|---|-----|------|------|-----|---|------|-----|--------|---------|
| 43 | armor.armor_straw_helmet | 草甲·头 | dried_grass×5 + grass_rope×1 | 0 | 30 | armor_straw_helmet×1 | A | 🔓 | 最低档防护(§四 16部位 HEAD) |
| 44 | armor.straw_chest | 草甲·胸 | dried_grass×8 + grass_rope×2 | 0 | 45 | armor_straw_chestplate×1 | A | 🔓 | 草甲聊胜于无(§四 CHEST) |
| 45 | armor.straw_legs | 草甲·腿 | dried_grass×6 + grass_rope×2 | 0 | 40 | armor_straw_leggings×1 | A | 🔓 | (§四 THIGH/CALF) |
| 46 | armor.armor_straw_boots | 草甲·脚 | dried_grass×4 + grass_rope×1 | 0 | 25 | armor_straw_boots×1 | A | 🔓 | (§四 FOOT) |
| 47 | armor.armor_hide_helmet | 皮甲·头 | tanned_hide×2 + grass_rope×1 | 0 | 45 | armor_hide_helmet×1 | A | 🔓 | 兽皮护头(§四 HEAD) |
| 48 | armor.hide_chest | 皮甲·胸 | tanned_hide×4 + grass_rope×2 | 0 | 65 | armor_hide_chestplate×1 | A | 🔓 | (§四 CHEST) |
| 49 | armor.hide_legs | 皮甲·腿 | tanned_hide×3 + grass_rope×2 | 0 | 55 | armor_hide_leggings×1 | A | 🔓 | (§四 THIGH/CALF) |
| 50 | armor.armor_hide_boots | 皮甲·脚 | tanned_hide×2 + grass_rope×1 | 0 | 40 | armor_hide_boots×1 | A | 🔓 | (§四 FOOT) |
| 51 | medical.arm_splint | 夹板·臂 | wood_plank×2 + rough_cloth×1 | 0 | 15 | arm_splint×2 | M | 🔓 | §四 FRACTURE 急救(骨折固定) |
| 52 | medical.leg_splint | 夹板·腿 | wood_plank×3 + rough_cloth×2 | 0 | 20 | leg_splint×2 | M | 🔓 | §四 FRACTURE 急救 |

#### 五、基础武器组件 (10)

| # | ID | 名称 | 材料 | qi | t | 产出 | cat | unlock | 物理依据 |
|---|-----|------|------|-----|---|------|-----|--------|---------|
| 53 | weapon.iron_sword_blank | 凡铁剑胎 | iron_ingot×3 | 0 | 65 | iron_sword_blank×1 | M | 🔓 | 锻造预制件(plan-forge-v1 进料) |
| 54 | weapon.bone_spike_crude | 粗骨刺 | shu_gu×2 + stone_chunk×1 | 0 | 40 | bone_spike_crude×3 | AnqiCarrier | 🔓 | 兽骨粗削(与 materials.toml bone_spike 高阶暗器区分) |
| 55 | weapon.wooden_club | 木棍 | spirit_wood×2 | 0 | 20 | wooden_club×1 | M | 🔓 | 凡木棍(§五 凡器边界，与 weapons.toml wooden_staff 灵器区分) |
| 56 | weapon.stone_spearhead | 石矛头 | stone_chunk×2 | 0 | 30 | stone_spearhead×1 | M | 🔓 | 石磨矛头 |
| 57 | weapon.sling | 弹弓 | wood_handle×1 + tanned_hide×1 + grass_rope×1 | 0 | 35 | sling_weapon×1 | M | 🔓 | 入门远程凡器(§四 距离衰减：凡物不受真元衰减) |
| 58 | weapon.sling_stone | 飞石 | stone_chunk×3 | 0 | 10 | sling_stone×5 | M | 🔓 | 圆石弹药 |
| 59 | weapon.wooden_shield | 木盾 | wood_plank×4 + iron_ingot×1 | 0 | 60 | wooden_shield×1 | A | 🔓 | §四 体表防护(护体凡器) |
| 60 | weapon.bone_shield | 骨盾 | shu_gu×6 + grass_rope×2 | 0 | 55 | bone_shield×1 | A | 🔓 | 兽骨编成(轻而脆) |
| 61 | weapon.iron_dagger | 凡铁匕首 | iron_ingot×1 + wood_handle×1 | 0 | 40 | iron_dagger×1 | M | 🔓 | 近身凡器(§四 距离衰减：贴脸最高效→短兵利器) |
| 62 | weapon.stone_knife | 石刃 | stone_chunk×1 + wood_handle×1 | 0 | 20 | stone_knife×1 | M | 🔓 | 最入门切割器 |

#### 六、修炼辅材 (12)

| # | ID | 名称 | 材料 | qi | t | 产出 | cat | unlock | 物理依据 |
|---|-----|------|------|-----|---|------|-----|--------|---------|
| 63 | cultivation.meditation_mat | 蒲团 | dried_grass×6 + rough_cloth×2 | 0 | 35 | meditation_mat×1 | M | 🔓 | §三 静坐冲击 辅具(减地面散热→灵气聚集效率+) |
| 64 | cultivation.qi_talisman | 灵气引导符 | zhenfa_blank_array×1 + powder_dan_sha×1 | 2 | 50 | qi_guide_talisman×1 | M | 📜 | 丹砂画符引灵(§三 修炼主流程：引导灵气→经脉) |
| 65 | cultivation.meridian_rub | 经脉图拓片 | zhenfa_blank_array×1 + spirit_grass×1 | 1 | 35 | meridian_rubbing×1 | M | 📜 | 灵草汁拓印(§六 经脉路径 学习辅具) |
| 66 | cultivation.ningmai_prep | 凝脉散预制包 | ning_mai_cao×3 + powder_dan_sha×1 + herb_vial×1 | 0 | 45 | ningmai_prep_kit×1 | M | 📜 | 凝脉散原料预包装(library 末法药材 凝脉草→凝脉散引药) |
| 67 | cultivation.huiyuan_soup | 回元芷煎汤 | hui_yuan_zhi×2 + clay_pot×1 + spirit_charcoal×1 | 0 | 60 | huiyuan_decoction×1 | M | 🔓 | library 末法药材 回元芷→临阵急用(简易煎法非炼丹) |
| 68 | cultivation.meridian_salve | 养经膏 | yang_jing_tai×2 + rat_tail_oil×1 + herb_vial×1 | 1 | 50 | meridian_salve×1 | M | 📜 | library 末法药材 养经苔→爆脉后修复(外敷制剂) |
| 69 | cultivation.anti_gu | 解蛊散 | jie_gu_rui×3 + xiong_huang×1 | 1 | 45 | anti_gu_powder×1 | DuguPotion | 📜 | library 末法药材 解蛊蕊+雄黄→解毒蛊(§五 毒蛊流克制) |
| 70 | cultivation.qingzhuo | 清浊散 | qing_zhuo_cao×2 + powder_dan_sha×1 | 1 | 40 | qingzhuo_powder×1 | DuguPotion | 📜 | library 末法药材 清浊草→克染色浊乱(§六 真元染色) |
| 71 | cultivation.calming_tea | 安神茶 | an_shen_guo×2 + clay_pot×1 | 0 | 30 | calming_tea×1 | M | 🔓 | library 末法药材 安神果→镇心(composure 回复) |
| 72 | cultivation.rat_bait | 鼠群诱饵 | salt_crystal×2 + rough_cloth×1 | 0 | 20 | rat_bait×1 | M | 🔓 | library 异兽三形考 白盐蓬波动谱同修士打坐→诱鼠 |
| 73 | cultivation.spirit_rack | 灵石架 | wood_plank×2 + iron_ingot×1 | 0 | 45 | spirit_stone_rack×1 | M | 🔓 | 灵石存放减接触磨损(§八 灵物操作磨损) |
| 74 | cultivation.bandage | 止血绷带 | rough_cloth×2 | 0 | 10 | bandage×4 | M | 🔓 | §四 16部位伤口 ABRASION/LACERATION 急救 |

#### 七、阵法基础 (8)

| # | ID | 名称 | 材料 | qi | t | 产出 | cat | unlock | 物理依据 |
|---|-----|------|------|-----|---|------|-----|--------|---------|
| 75 | array.blank_paper | 阵法白纸 | rough_cloth×1 + powder_dan_sha×1 + rat_tail_oil×1 | 1 | 35 | zhenfa_blank_array×2 | M | 🔓 | 丹砂油浸布(§五 地师·阵法 基础载体) |
| 76 | array.flag_basic | 阵旗·凡 | wood_handle×1 + rough_cloth×1 + powder_dan_sha×1 | 2 | 50 | array_flag_basic×1 | ZhenfaTrap | 📜 | §五 地师流 阵法标界(真元灌注标记范围) |
| 77 | array.eye_basic | 阵眼·凡 | ling_jing×1 + iron_ingot×2 | 3 | 65 | array_eye_basic×1 | ZhenfaTrap | 📜 | library 矿物录·灵晶→法宝阵眼(灵晶镶铁为核) |
| 78 | array.trip_wire | 预警绊线 | spider_silk_cord×2 + iron_needle×1 | 0 | 20 | trip_wire×3 | M | 🔓 | 物理机关(蛛丝韧+铁针触发，§十一 安全) |
| 79 | array.decoy_stake | 欺天阵木桩 | spirit_wood×2 + rough_cloth×1 + shu_gu×1 | 2 | 65 | decoy_stake×1 | ZhenfaTrap | 📜👤 | §八 欺天阵(复用 materials.toml 现有 ID, sq=0.9) |
| 80 | array.scatter_bead | 散真元珠 | ling_jing×1 + bone_meal_mat×1 | 2 | 35 | qi_scatter_bead×2 | ZhenfaTrap | 📜 | 灵晶碎粉混骨粉封珠(散布真元干扰追踪) |
| 81 | array.gather_base | 聚灵阵基座 | ling_tie×2 + zhenfa_blank_array×2 + ling_jing×1 | 5 | 130 | gather_array_base×1 | ZhenfaTrap | 📜👤 | §三 灵气浓度对修炼速度的影响→人工聚灵(灵铁+阵纸+灵晶组阵) |
| 82 | array.beast_trap | 困兽圈 | iron_ingot×3 + grass_rope×2 | 0 | 50 | beast_trap×1 | M | 🔓 | 凡物机关(§七 噬元鼠群/灰烬蛛 捕获) |

#### 八、经济与交易 (8)

| # | ID | 名称 | 材料 | qi | t | 产出 | cat | unlock | 物理依据 |
|---|-----|------|------|-----|---|------|-----|--------|---------|
| 83 | economy.coin_blank | 骨币胚 | shu_gu×1 + zhenfa_blank_array×1 | 0 | 30 | bone_coin_blank×3 | M | 🔓 | §九 骨币制造(骨+阵纸=空白胚，灌真元才有价) |
| 84 | economy.coin_box | 骨币匣 | wood_plank×4 + iron_ingot×1 + powder_dan_sha×1 | 0 | 55 | coin_box×1 | C | 🔓 | §九 骨币半衰(匣内丹砂隔灵→减缓真元流失) |
| 85 | economy.trade_scale_stand | 交易秤台 | trade_scale×1 + wood_plank×2 | 0 | 40 | trade_scale_stand×1 | T | 🔓 | 交易秤固定架(§九 面对面交易：放在摊位上免手持) |
| 86 | economy.waymark | 标记石 | stone_chunk×1 + powder_dan_sha×1 | 0 | 10 | waymark_stone×4 | M | 🔓 | 丹砂画记号→标路(§十三 世界尺度大→标记重要) |
| 87 | economy.disguise_wrap | 伪装包裹 | rough_cloth×2 + hui_jin_tai×1 | 0 | 25 | disguise_wrap×2 | TuikeSkin | 🔓 | 灰烬苔裹布(灰烬苔≈残灰方块气息→掩盖灵物) |
| 88 | economy.price_tag | 标价签 | rough_cloth×1 + powder_dan_sha×1 | 0 | 10 | price_tag×5 | M | 🔓 | 丹砂写价(§九 面对面交易 标注物品价值) |
| 89 | economy.puppet_frame | 交易傀儡骨架 | shu_gu×8 + spider_silk_cord×3 + iron_ingot×2 | 4 | 150 | trade_puppet_frame×1 | M | 📜👤 | §九 游商傀儡(骨骼+蛛丝绳+凡铁关节) |
| 90 | economy.niche_repair | 灵龛修补料 | stone_chunk×3 + ling_tie×1 | 2 | 65 | niche_repair_kit×1 | M | 📜 | 灵龛损坏修复(龛石碎+灵铁补) |

#### 九、住所与防御 (8)

| # | ID | 名称 | 材料 | qi | t | 产出 | cat | unlock | 物理依据 |
|---|-----|------|------|-----|---|------|-----|--------|---------|
| 91 | shelter.torch | 火把 | wood_handle×1 + spirit_charcoal×1 + rat_tail_oil×1 | 0 | 10 | torch_item×4 | M | 🔓 | 灵木炭+鼠尾油=灵火引(照明时辰级) |
| 92 | shelter.lantern | 灯笼 | iron_ingot×2 + spirit_charcoal×1 + ling_jing×1 | 1 | 45 | lantern_item×1 | M | 📜 | 灵晶芯灵火(library 矿物录·灵晶 封入即长明) |
| 93 | shelter.door_bolt | 门闩 | iron_ingot×3 + wood_plank×2 | 0 | 45 | door_bolt×1 | M | 🔓 | 凡物防御 |
| 94 | shelter.camo_net | 伪装网 | rough_cloth×3 + spirit_grass×2 | 0 | 55 | camouflage_net×1 | TuikeSkin | 🔓 | 灵草编入布→气息混入环境(§十一 匿名/隐蔽) |
| 95 | shelter.simple_bed | 简易床铺 | wood_plank×4 + dried_grass×4 + rough_cloth×2 | 0 | 60 | simple_bed×1 | M | 🔓 | 基础住所(§十四 修士也要睡觉) |
| 96 | shelter.moisture_base | 防潮地基 | stone_chunk×4 + hui_jin_tai×2 | 0 | 35 | moisture_base×4 | M | 🔓 | 石+苔→防地面潮气(灵物受潮加速散灵) |
| 97 | shelter.window_grate | 窗栅 | iron_ingot×4 | 0 | 35 | window_grate×2 | M | 🔓 | 凡铁栅栏 |
| 98 | shelter.niche_base | 灵龛基座 | spirit_niche_stone×1 + ling_tie×2 + wood_plank×4 | 5 | 180 | niche_base×1 | M | 📜👤💡 | §十一 灵龛放置(龛石+灵铁+木台→永久复活点) |

#### 十、炼丹 / 炼器预备 (2)

| # | ID | 名称 | 材料 | qi | t | 产出 | cat | unlock | 物理依据 |
|---|-----|------|------|-----|---|------|-----|--------|---------|
| 99 | prep.furnace_kit | 凡铁炉组件 | iron_ingot×6 + spirit_charcoal×2 + stone_chunk×4 | 0 | 120 | furnace_kit_fantie×1 | T | 🔓 | plan-alchemy-v1 §1.2(凡铁炉 tier 1 预制件，放下即成炉) |
| 100 | prep.forge_station | 锻造台组件 | iron_ingot×4 + spirit_wood×2 + stone_chunk×6 | 0 | 100 | forge_station_kit×1 | T | 🔓 | plan-forge-v1(锻造台预制件，放下即成锻造站) |

---

### P1.2 配方物理推导索引

每条配方的物理依据来源汇总：

| 来源 | 配方# |
|------|-------|
| worldview §二 灵压/挥发 | 22, 27, 29, 32, 36, 64, 73 |
| worldview §三 修炼体系 | 63-68, 81, 98 |
| worldview §四 战斗系统 | 43-52, 53-62, 74 |
| worldview §五 战斗流派 | 10, 11, 35, 57, 69, 70, 75-82 |
| worldview §七 生物生态 | 16-18, 24, 25, 72, 82 |
| worldview §八 天道/磨损 | 27, 31-33, 38, 73, 79, 84, 87 |
| worldview §九 经济 | 9, 37, 41, 83-90 |
| worldview §十 资源 | 1-8, 21, 40 |
| worldview §十一 安全 | 78, 88, 94 |
| worldview §十三 地理 | 86 |
| worldview §十四 一天 | 1-12, 40, 67, 71, 91, 95 |
| library 末法药材十七种 | 8, 66-71 |
| library 矿物录 | 21, 29, 36, 77, 92 |
| library 异兽三形考 | 16-18, 24, 72 |
| library 绝地草木拾遗 | 10, 11, 25 |
| library 灵物磨损笔记 | 27, 31-33, 38, 73, 84 |

---

## P2：Server 配方注册

### P2.1 CraftRecipe 扩展

```rust
// server/src/craft/recipe.rs 新增字段
pub struct CraftRecipe {
    // ... 现有字段 ...
    pub station: Option<CraftStationKind>,  // None = 手搓, Some(Workbench) = 制作台
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CraftStationKind {
    Workbench,
    // 未来可扩展：AlchemyBench, ForgeBench, ...
}
```

### P2.2 配方注册函数

```rust
// server/src/craft/workbench_recipes.rs (新文件)
pub fn register_workbench_recipes(registry: &mut CraftRegistry) -> Result<(), RegistryError> {
    // 100 配方注册，分 10 组函数
    register_survival_tools(registry)?;      // #1-12
    register_processing(registry)?;           // #13-30
    register_containers(registry)?;           // #31-42
    register_basic_armor(registry)?;          // #43-52
    register_weapon_components(registry)?;    // #53-62
    register_cultivation_support(registry)?;  // #63-74
    register_array_basics(registry)?;         // #75-82
    register_economy(registry)?;             // #83-90
    register_shelter(registry)?;             // #91-98
    register_alchemy_forge_prep(registry)?;  // #99-100
    Ok(())
}
```

### P2.3 制作台交互系统

```rust
// server/src/craft/workbench.rs 系统注册
pub fn register_workbench(app: &mut App) {
    app.add_systems(Update, (
        handle_workbench_place,       // 放置制作台
        handle_workbench_interact,    // 右键交互
        handle_workbench_break,       // 拆除
    ));
}
```

### P2.4 Session 校验扩展

在 `start_craft()` 新增校验：若 `recipe.station == Some(Workbench)` → 校验玩家 3 格内是否存在 `WorkbenchBlock` entity。不在范围 → `StartCraftError::StationOutOfRange`。

---

## P3：Client UI

### P3.1 WorkbenchScreen

**触发**：右键制作台方块 → Server 发 `WorkbenchOpenPayload` → Client 打开 `WorkbenchScreen`。

**布局**：复用 CraftScreen 框架，但：
- 左侧配方列表按 10 大类分组（不是现有 9 CraftCategory）
- 顶部标题「末法制作台」
- 右侧材料预览 + 制作按钮

**Java 文件**：
- `client/src/main/java/com/bong/client/craft/WorkbenchScreen.java`
- `client/src/main/java/com/bong/client/craft/WorkbenchScreenBootstrap.java`

### P3.2 bbmodel 渲染

**模型注册**：Client 通过 resource pack 机制将 `minecraft:crafting_table` block model 替换为 `bong:models/block/workbench.json`。

**粒子效果**：制作台放置后顶面阵纹有微弱的丹砂红粒子脉冲（每 60 tick 一次 burst，3 个 `BongSpriteParticle`，lifetime 10 tick，颜色 #8B3A3A，alpha 0.3→0）。

### P3.3 音效/视觉规格

#### 音效（SFX）

| 事件 | 音效 ID | 描述 | pitch | volume |
|------|---------|------|-------|--------|
| 放置制作台 | `bong:block.workbench.place` | 木块重落+铁钉微响（wood_thud + metal_clink 叠层） | 0.9-1.1 | 0.8 |
| 拆除制作台 | `bong:block.workbench.break` | 木板裂响 | 0.8-1.0 | 0.7 |
| 打开制作台 | `bong:block.workbench.open` | 木盖推开声（短促 wood_slide） | 1.0-1.2 | 0.5 |
| 开始制作 | `bong:craft.workbench.start` | 工具碰撞声（stone_strike 或 metal_tap） | 0.9-1.1 | 0.6 |
| 制作进行中 | `bong:craft.workbench.tick` | 每 40 tick 一次轻响（砧锤 / 研磨 / 编织随 category） | 0.8-1.2 | 0.3 |
| 制作完成 | `bong:craft.workbench.done` | 清脆落物声（item_drop + 微光音 sparkle） | 1.0 | 0.7 |
| 制作失败 | `bong:craft.workbench.fail` | 沉闷断裂声 | 0.7 | 0.5 |

#### 视觉效果（VFX）

| 事件 | 效果 | 参数 |
|------|------|------|
| 制作进行中 | 制作台顶面粒子频率从 60tick/次 加速到 10tick/次 | 颜色 #8B3A3A，count 3→8，lifetime 10→15 tick |
| 制作完成 | 顶面 burst 12 个丹砂粒子 + 1 个产出物品 3D icon 浮现并淡出 | burst lifetime 20 tick，icon 上浮 0.5 block 后 alpha 1.0→0 |
| 真元消耗 | qi_cost>0 时玩家→制作台方向有半透明灵气丝流动 | `BongBeamParticle` 蓝白 #A8C4E0，width 1px，lifetime 30 tick |

---

## P4：饱和测试

### P4.1 配方校验测试（100 条 pin 测试）

每条配方一个专属 test case：
- 材料 ID 存在于 ItemTemplate 注册表
- 产出 ID 存在于 ItemTemplate 注册表
- qi_cost 为有限非负数
- time_ticks > 0
- 产出灵质 ≤ 投入灵质总和 × 0.95（磨损律）
- station == Some(Workbench)

### P4.2 守恒律测试

- 所有 qi_cost > 0 的配方走 ledger QiTransfer
- QiTransfer.amount == recipe.qi_cost
- QiTransfer.reason == Crafting
- zone qi_density 变化 = +qi_cost（真元回流 zone）

### P4.3 Session 测试

- 手搓配方（station=None）不受制作台距离限制
- 制作台配方（station=Workbench）距离 > 3 → StartCraftError::StationOutOfRange
- 制作台被拆除中途 → CraftFailedEvent + 材料返还 70%

### P4.4 物理推导 pin 测试

抽检 10 条配方的物理推导正确性：
- #22 灵木炭：spirit_wood sq=0.8 × 2 = 1.6 → charcoal sq=0.3 × 3 = 0.9 ≤ 1.6×0.95 ✓
- #27 灵草束：spirit_grass sq=0.85 × 5 = 4.25 → bundle sq=0.80 × 1 = 0.80 ≤ 4.25×0.95=4.04 ✓（束=聚合，非倍增）
- #32 密封药瓶：vial sq=0 + cord sq=0.5 + qi=1 → sealed sq=0.4 ≤ 0.5×0.95=0.475 ✓（蛛丝灵力部分封入瓶壁）
- 更多...

### P4.5 集成测试

- 玩家放置制作台 → 右键打开 → 选配方 → 投入材料 → 等待 → 产出 → 拆除回收
- e2e 从原材料采集到成品使用的完整链路

---

## P5：归档

### P5.1 Finish Evidence 模板

```
## Finish Evidence
- 落地清单：workbench.rs / workbench_recipes.rs / workbench_materials.toml / WorkbenchScreen.java / Workbench.bbmodel
- 关键 commit：...
- 测试结果：100 pin + 守恒律 + session + 物理推导 + e2e
- 跨仓库核验：server WorkbenchBlock + client WorkbenchScreen + agent bong:craft/outcome station
```

---

## §8 开放问题（P0 决策门前需收口）

1. **制作台是否支持批量制作？** 现有 CraftSession 支持 quantity 1-64 批量。制作台配方是否沿用？
2. **制作台与现有手搓配方的关系？** 现有手搓配方（流派专属）是否可在制作台上做？还是严格隔离？
3. **制作台放置限制？** 是否限制每个区块/chunk 内制作台数量？（天道忌满类推？）
4. **bbmodel 纹理风格**：末法残土美术风格尚未统一，bbmodel 纹理暂用 placeholder 还是直接定稿？
5. ~~**hui_jin_tai vs 灰烬苔**~~ **已关闭**：core.toml line 261 确认 `id = "hui_jin_tai"`，拼写正确。

全部已在 §8.1 收口。原表保留以备追溯，**实施时以 §8.1 决议为准**。

---

## §8.1 决议（pre-P0 收口，2026-05-21）

### #1 制作台批量制作

**决议**：
1. 沿用现有 CraftSession batch 系统（quantity 1-64），workbench 配方无特殊限制
2. 材料 × quantity 预扣、qi_cost × quantity 预扣、time 不乘（与 `session.rs:306,320,389` 现有行为一致）
3. 无需新增字段或验证逻辑——`MAX_CRAFT_QUANTITY=64` 上限对制作台同样适用

**落点**：`server/src/craft/session.rs:31`（MAX_CRAFT_QUANTITY）/ `session.rs:253-410`（start_craft 全量复用）/ plan §P2 无需修改

### #2 制作台与手搓配方关系

**决议**：
1. 严格隔离：`recipe.station == None` → 手搓（流派专属），`recipe.station == Some(Workbench)` → 制作台（通用入门）
2. 手搓配方不可在制作台做，制作台配方不可手搓——UI 按 station 上下文过滤
3. CraftCategory 9 variant 不变，station 维度正交于 category（同 category 可有手搓和制作台配方）
4. `start_craft()` 校验：若 recipe.station=Workbench 则检查 3 格内 WorkbenchBlock entity，否则跳过

**落点**：`server/src/craft/recipe.rs:149-168`（CraftRecipe 新增 station 字段）/ `session.rs:253`（start_craft 新增 station 校验）/ plan §P2.1 + §P2.4

### #3 制作台放置限制

**决议**：
1. 不设 per-chunk 数量限制。制作台是凡物（sq=0.0），天道忌满针对灵气聚集非凡物密度
2. 自然约束已足够：每台消耗 spirit_wood×4 + iron_ingot×2 + shu_gu×2，材料成本限制滥放
3. 与现有 WeaponForgeStation / AlchemyFurnace / SpiritNiche 一致——均无 per-chunk 限制

**落点**：`server/src/craft/workbench.rs`（新文件，不加限制逻辑）/ plan §P0.1 无需修改

### #4 bbmodel 纹理风格

**决议**：
1. 直接定稿，不用 placeholder。项目已有 113 个 bbmodel + 8 个正式 block texture，asset pipeline 成熟
2. 纹理按 plan §P0.2 设计规格制作（16×16 px，骨白/褐色/丹砂红配色）
3. 遵循现有 block model 模式：JSON parent 引用 + 贴图映射

**落点**：`local_models/Workbench.bbmodel`（新文件）/ `client/src/main/resources/assets/bong/models/block/workbench.json`（新文件）/ `client/src/main/resources/assets/bong/textures/block/workbench_*.png`（新文件）/ plan §P0.2 + §P3.2

---

## §10 实施工作流

### §10.1 PR 序列

1. **PR-1 数据层**：新增 ItemTemplate（workbench_materials.toml）+ workbench_item 定义 + 配方 JSON
2. **PR-2 Server 逻辑**：CraftRecipe.station 扩展 + workbench.rs + workbench_recipes.rs + 100 配方注册 + 测试
3. **PR-3 bbmodel + Client**：Workbench.bbmodel + 纹理 + WorkbenchScreen + 渲染
4. **PR-4 集成 + 归档**：e2e 测试 + 数值校准 + Finish Evidence

### §10.2 subagent 配置

```
Agent(
  subagent_type: "claude",
  model: "opus",
  prompt: "...任务...\n\nultrathink"
)
```

### §10.3 视觉资产 3 轮自审 + `<PROMISE>` 担保

PR-3（bbmodel + Client）包含视觉资产（Workbench.bbmodel、纹理、item icon），按 `docs/CLAUDE.md` §6.1 强制 3 轮自我打磨：

1. **Round 1** first cut → commit `(round 1/3)`
2. **Round 2** 自我 review（与 §P0.2 spec 一致性 + 配色校验）→ 修 → commit `(round 2/3)`
3. **Round 3** 终轮 review → 修 → commit `(round 3/3)`，commit message 末尾写 `<PROMISE>` 块

纯逻辑代码（WorkbenchScreen.java 等）不适用，按常规 atomic commit。

### §10.4 CodeRabbit 等待协议

每 PR 提交后 ScheduleWakeup 1200s 等 CR review，最多 3 回合。

### §10.5 单次 consume-plan 全自动到 merge

用户提交 `/consume-plan` 后即可下班。

---

## Finish Evidence

### 落地清单

| 阶段 | 模块/文件 |
|------|-----------|
| P0 | `server/src/craft/workbench.rs` — WorkbenchBlock component + is_within_workbench_range |
| P0 | `local_models/Workbench.bbmodel` — Blockbench 源模型 |
| P0 | `client/.../textures/block/workbench_{top,side,front,bottom}.png` — 4 张 16×16 纹理 |
| P0 | `client/.../models/block/workbench.json` — MC JSON model |
| P1 | `server/assets/items/workbench_materials.toml` — 80 个新 ItemTemplate |
| P1 | `server/assets/items/minerals.toml` — 7 个矿物 ItemTemplate |
| P1 | `server/assets/items/core.toml` — workbench_item 定义 |
| P2 | `server/src/craft/recipe.rs` — CraftStationKind 枚举 + station 字段 |
| P2 | `server/src/craft/workbench_recipes.rs` — 100 条制作台配方注册 |
| P2 | `server/src/craft/session.rs` — start_craft station 校验 + StationOutOfRange |
| P2 | `server/src/network/craft_emit.rs` — ECS WorkbenchBlock 距离查询 |
| P3 | `client/.../craft/WorkbenchScreen.java` — 制作台 UI |
| P3 | `client/.../craft/WorkbenchScreenBootstrap.java` — payload 路由注册 |
| P3 | `client/.../craft/WorkbenchConstants.java` — 7 SFX + 3 VFX 常量 |
| P3 | `client/.../blockstates/crafting_table.json` — blockstate 替换 |
| P4 | `server/src/craft/workbench_recipes.rs` tests — 100 pin + physics spot-check |

### 关键 commit

| Hash | 日期 | 描述 |
|------|------|------|
| 8d701c71e | 2026-05-21 | PR-1: 数据层——制作台 + 88 ItemTemplate (#298) |
| cc89a2533 | 2026-05-21 | PR-2: Server 逻辑层——station 字段 + 100 配方 + session 校验 (#301) |
| 53ebf3ce8 | 2026-05-22 | PR-3: Client UI + bbmodel + 视觉资产 (#302) |
| 96dd8e3d0 | 2026-05-22 | PR-4: P4 饱和测试 |

### 测试结果

```
cd server && cargo test
test result: ok. 5988 passed; 0 failed; 0 ignored

cd client && ./gradlew test build
BUILD SUCCESSFUL (WorkbenchScreenTest 14 cases + WorkbenchConstantsTest 18 cases)
```

### 跨仓库核验

| 层 | Symbol | 命中数 |
|----|--------|--------|
| server | `WorkbenchBlock` | 8 |
| server | `CraftStationKind` | 19 |
| client | `WorkbenchScreen` | 26 |

### 遗留 / 后续

- item icon 图标生成（需 `/gen-image item` skill，不在代码 PR 范围）
- 实际音效资产（.ogg）录制（SFX ID 已定义在 WorkbenchConstants）
- VFX 粒子渲染 system 实装（参数已定义，渲染逻辑待 plan-vfx-v1）
- 制作台被拆除中途取消 session 的完整 Bevy system（依赖 ChunkLayer/BlockState API）
- agent 层 `bong:craft/outcome` 增加 `station: "workbench"` 字段（当前 Agent IPC 已有 craft outcome，字段扩展待 agent plan）
