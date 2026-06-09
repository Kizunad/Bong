# plan-container-filter-and-completion-v1 — 容器品类筛选 + 12 僵尸容器使用闭环补全

> **来源**：手搓 104 产出物僵尸审计「容器全死（12）」一类。给容器加品类筛选 + 把 12 个 `category=misc` 的占位容器全部补成可用。
> **依赖**：[[plan-nested-pack-base-v1]]（提供子容器打开/移动机制；随身子包接它）。可放置容器部分 → [[plan-placeable-container-blocks-v1]]。
> **状态**：骨架（草案）。筛选架构已定（新增 `ItemCategory` 变体），保鲜接现成 shelflife。

给容器加 `accept_filter` 品类筛选维度（只收矿石 / 只收草药等），把 12 个僵尸容器全部补成有真实使用闭环：随身子包接套包系统、保鲜类接 `ContainerFreshnessBehavior`、经济/情报类定行为。让每个容器都不再是孤岛。

## 阶段总览

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | 新增 `ItemCategory::Mineral/Anqi/Liquid` 变体 + `ContainerSpec.accept_filter` 数据模型 | ⬜ |
| P1 | 筛选执行：`validate_attach_fits` + 套包子容器 move 校验 | ⬜ |
| P2 | 保鲜行为映射：密封药瓶 / 防潮包 / 封灵匣 接 `ContainerFreshnessBehavior` | ⬜ |
| P3 | 随身子包闭环 + filter 落地到全部随身容器 | ⬜ |
| P4 | client filter 元数据同步 + 灰显非法拖拽 + 12 容器逐个验收 | ⬜ |

## 接入面（防孤岛）

- **进料**：[[plan-nested-pack-base-v1]] 的子容器开/移/关 + `ContainerState`；`ContainerSpec`（`inventory/mod.rs:169`）；`ItemCategory`（`mod.rs:225`，13 变体）+ `parse_item_category`（`mod.rs:1950`）；保鲜走 `shelflife/types.rs:248-270` 的 `ContainerFreshnessBehavior`（Normal/Halve/Freeze/DryingRack/SpoilOnly/AgeAccelerate）+ `shelflife/container.rs` 的 `container_storage_multiplier` / `enter_container`/`exit_container`；映射模板 `spiritwood/mod.rs:627` `item_freshness_behavior`（已把 ling_xia→Freeze、ling_mu_ban→SpoilOnly 落地）。
- **出料**：容器筛选拒绝非法 attach（`validate_attach_fits` `mod.rs:3641`）；保鲜容器内物品走 `enter/exit_container` 改 freshness 速率；12 容器各有真实行为。
- **共享类型 / event**：`ItemCategory` 加 `Mineral/Anqi/Liquid` 变体（连带 `parse_item_category` + **所有 match 穷举处**补全——穷举编译强约束天然防漏）；`ContainerSpec` 加 `accept_filter: Option<Vec<ContainerAcceptFilter>>`；新增 `ContainerAcceptFilter` 枚举（ItemCategory 粗筛 + 可选 template_id 前缀细筛）；**复用 `ContainerFreshnessBehavior` 不另造保鲜机制**。
- **跨仓库契约**：server `ContainerInfoV1`（或 `PackContainerOpenV1`）加 `accept_filter` 列表字段；client `SubContainerPanel`/`GridSlotComponent` 收 filter 后灰显非法拖拽目标格；**agent 无关**。
- **worldview 锚点**：容器描述里的「隔灵 / 保鲜」呼应 worldview §二「真元极易挥发」——保鲜容器=减缓挥发；骨币匣（骨币货币）；封灵匣（高灵物短存）。
- **qi_physics 锚点**：保鲜 = 减缓 `spirit_quality`/`freshness` 衰减，**走 shelflife 现有 `container_storage_multiplier`，不新增任何 `*_DECAY*` 常数 / 衰变函数**（孤岛红旗自查：同源现象一份实现，归 shelflife）。

## P0 — ItemCategory 升变体 + accept_filter 数据模型 ⬜

- `ItemCategory`（`mod.rs:225`）加 `Mineral` / `Anqi` / `Liquid` 三变体 + `parse_item_category`（`mod.rs:1950`）加别名 + **全仓所有 `match category` 穷举处补臂**（编译器强制完整）。
- `ContainerSpec`（`mod.rs:169`）加 `accept_filter: Option<Vec<ContainerAcceptFilter>>`；`ContainerSpecToml`（`mod.rs:1513`）加 `#[serde(default)] accept: Vec<String>`。
- 定义 `ContainerAcceptFilter`（按 `ItemCategory` 粗筛 + 可选 `template_prefix: String` 细筛，参照 `mod.rs:3851` TreasureBehavior 模式）。
- 测试：新 category 解析正反；`accept=[]` 默认全收 pin；旧无 accept 字段 TOML 向后兼容。

## P1 — 筛选执行 ⬜

- `validate_attach_fits`（`mod.rs:3641`）bounds-check 之前插 `accept_filter` 白名单校验（container_id 反查 spec → 比对 item category，拒绝返回 `Err`）。
- 装备型容器走 `container_id_to_equip_slot`（`mod.rs:3297`）→ equipped → template → spec 反查；**套包子容器（`pack_` 前缀）按 `instance_id` 找 `ItemInstance.sub_container` 的 spec**（依赖 [[plan-nested-pack-base-v1]] P3 的 container_id 规则）。
- 测试：矿石进 ore_sack 通过、草药进 ore_sack 拒绝；通用容器（accept=[]）全收；套包子容器筛选与装备容器一致。

## P2 — 保鲜行为映射 ⬜

- 照 `spiritwood/mod.rs:627` 模板，把容器 template_id 映射到 `ContainerFreshnessBehavior`：
  - `sealed_vial`（`workbench_materials.toml:182`，「保质期翻倍」）→ `Halve`
  - `spirit_seal_box`（:226，「高灵物短存最佳」）→ `Freeze`（呼应 `inventory_snapshot_emit` 已有 Freeze 用法）
  - `moisture_guard`（:248，「吸湿防霉」）→ `SpoilOnly { rate }`（精确 rate 见 §8 #2）
- 容器内物品 enter 时套用 behavior，exit 时还原（`shelflife/container.rs` `enter/exit_container`）。
- 测试：物品进 sealed_vial 后 freshness 衰减减半；进 spirit_seal_box 冻结；moisture_guard SpoilOnly 只防霉不延灵气；exit 后恢复 Normal。

## P3 — 随身子包闭环 + filter 落地 ⬜

- 给 [[plan-nested-pack-base-v1]] P5 升级的 5 随身子包配 `accept_filter`：`herb_pouch`=Herb/Food、`ore_sack`=Mineral、`projectile_bag`=Anqi、`water_skin`=Liquid、`herb_crate`=Herb。
- 补其余随身容器：`sealed_vial`(:182, Pill/Food)、`coin_box`(:204, **BoneCoin** filter——唯一已有专属 category，筛选最易)、`sealed_envelope`(:303, RecipeFragment/RecipeHint/Scroll 情报袋)、`spirit_seal_box`(:226, Treasure/高灵物) 各升 container + filter。
- 测试：每个随身容器装合法/非法物品的接受/拒绝；coin_box 只收骨币。

## P4 — client filter 同步 + 灰显 + 12 容器验收 ⬜

- `schema/inventory.rs` 的 `ContainerInfoV1`（或 `PackContainerOpenV1`）加 `accept_filter` 列表字段，双端（Rust serde + Java）同步。
- client `SubContainerPanel`/`GridSlotComponent` 收 filter 后对非法拖拽目标格灰显（`HighlightState.INVALID`，复用现有 highlight 机制）。
- **视听**：非法拖拽格灰显 = 红 tint `#C04040` opacity 0.4（INVALID 高亮）；放入合法格 SFX = `item.bundle.insert`。
- 逐个核验 12 容器各自 kind/filter/behavior（验收清单见下表）。

## 12 容器分类验收表（调查综合产出）

| 容器 | kind | 容量 | filter | 行为 |
|------|------|------|--------|------|
| `herb_pouch` 灵草囊 | subbag | 3×3 | Herb/Food | 套包，Normal 保鲜 |
| `ore_sack` 矿石袋 | subbag | 3×3 | **Mineral** | 套包（filter 旗舰验收） |
| `projectile_bag` 暗器袋 | subbag | 3×4 | **Anqi** | 套包 |
| `water_skin` 水囊 | subbag | 1×2~2×2 | **Liquid** | 套包 |
| `herb_crate` 灵草箱 | subbag | 4×4 | Herb | 套包随身大筐（放置版见 plan-C §8 #2） |
| `sealed_vial` 密封药瓶 | subbag | 1×2~2×2 | Pill/Food | 套包 + **Halve** 保鲜 |
| `spirit_seal_box` 封灵匣 | subbag/intel | 2×2 | Treasure/高灵物 | 套包 + **Freeze** 保鲜 |
| `moisture_guard` 防潮包 | subbag | 3×3 | accept=[]/Herb | 套包 + **SpoilOnly** 防霉 |
| `coin_box` 骨币匣 | economy | 2×2~3×3 | **BoneCoin** | 套包 |
| `sealed_envelope` 封缄 | intel | 1×2 | RecipeFragment/Hint/Scroll | 套包情报袋 |
| `trade_crate` 货箱 | placeable | 4×4 | accept=[] | → [[plan-placeable-container-blocks-v1]] |
| `dead_drop_box` 死信箱 | placeable/intel | 2×2 | RecipeFragment/Hint/Scroll | → [[plan-placeable-container-blocks-v1]]（阵法防砸） |

## §8 开放问题（P0 决策门前需收口）

| # | 问题 | 推荐默认 |
|---|------|------|
| 1 | 矿石/液体筛选粒度？ | **升 `ItemCategory::Mineral/Anqi/Liquid` 变体（用户已决议）**；矿石必要时再用 `ItemInstance.mineral_id` NBT 二级细筛。 |
| 2 | `moisture_guard` SpoilOnly 精确 rate？ | 待校准（0.3? 0.5?）。未定则 `inventory_snapshot_emit` 永远 Normal 兜底，保鲜对玩家隐形——P0 前必拍数。 |
| 3 | `accept=[]` 语义？ | 默认全收（通用容器）。 |
| 4 | 各容器精确 grid？ | 见上验收表（待 P0 与 [[plan-nested-pack-base-v1]] P5 一并定稿）。 |

## §10 实施工作流

升 active 时按 docs/CLAUDE.md §6：P0 数据模型 / P1 筛选 / P2 保鲜 / P3 随身闭环 / P4 client+验收，约 4-5 PR。依赖 [[plan-nested-pack-base-v1]] merge 后开。纯逻辑+数据，§10.1 多轮不适用。

## Finish Evidence

（迁入前必填）
