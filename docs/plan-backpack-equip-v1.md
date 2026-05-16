# Plan: Backpack Equipment — 背包装配系统 v1

> **状态**：⬜ 待启动
> 作者：Claude Code + Kiz

背包从硬编码占位容器变为**可装备、可制作、有耐久**的物品。玩家出生只有贴身口袋 2×3 + 一个破草包 3×3（耐久 0.3），更多空间靠制作/拾取背包获得。新增 InspectScreen "行囊" tab 管理背包装备与负重。

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|------|------|------|
| **P0** | Server 端 ContainerSpec + 动态容器重建 + 新装备槽 + 负重公式 | ⬜ |
| **P1** | Schema 扩展 + 网络协议适配（ContainerIdV1 → String） | ⬜ |
| **P2** | 背包物品模板 + 制作配方 + default.toml 重写 | ⬜ |
| **P3** | 背包耐久 system + 破损溢出 | ⬜ |
| **P4** | Client "行囊" tab + 背包装备拖拽 + 负重 HUD | ⬜ |
| **P5** | 饱和化测试 + 端到端验收 | ⬜ |

---

## 接入面 Checklist

- **进料**：
  - `inventory::PlayerInventory` 的 `containers` / `equipped` / `max_weight`（`server/src/inventory/mod.rs:357-364`）
  - `inventory::ItemTemplate` + `ItemRegistry`（`mod.rs:96-118`）
  - `craft::CraftRegistry` + `register_basic_processing_recipes`（`server/src/craft/mod.rs:787`）
  - `inventory::calculate_current_weight`（`mod.rs:2753`）
  - `inventory::validate_move_semantics`（`mod.rs:3082`）
  - `schema::inventory::ContainerIdV1`（`server/src/schema/inventory.rs:11-15`）
  - `schema::inventory::EquipSlotV1`（`server/src/schema/inventory.rs:19-36`）

- **出料**：
  - 动态 `ContainerState` 列表 → 推给 client 的 `inventory_snapshot`
  - `max_weight` 由装备背包实时派生 → 影响 `OverloadedMarker`（`mod.rs:403`）
  - 背包破损 → `DroppedItemEvent`（溢出物品落地）
  - 新制作配方 `basic.grass_pouch` → 接入 `CraftRegistry`

- **共享类型 / event**：
  - 复用 `DroppedItemEvent`（背包破损溢出）
  - 复用 `InventoryDurabilityChangedEvent`（背包耐久变化）
  - 新增 `ItemCategory::Container`
  - 扩展 `EquipSlotV1`（+3 背包槽）
  - `ContainerIdV1` 从 enum 改 String

- **跨仓库契约**：
  - server：`inventory/mod.rs`（核心改动）、`schema/inventory.rs`（ContainerIdV1 / EquipSlotV1）、`network/inventory_snapshot_emit.rs`（适配 String container id）、`craft/mod.rs`（新配方）
  - client：`InventoryModel`（动态 containers）、`EquipSlotType`（+3 槽）、`InspectScreen`（新 tab）、`BackpackGridPanel`（无改动，已支持动态 id/rows/cols）
  - agent：无变更

- **worldview 锚点**：§九 经济（灵物操作磨损 1-5%）→ 背包耐久磨损是其物理实现之一；§十四 "你快速采集...天道的磨损税" → 背包操作扣耐久

- **qi_physics 锚点**：无直接调用。背包耐久扣减是凡物磨损，不走真元物理。

---

## P0 — Server 端数据模型 + 动态容器

### P0.1 `ContainerSpec` 新字段

`server/src/inventory/mod.rs` 内 `ItemTemplate` 新增：

```rust
pub struct ContainerSpec {
    pub rows: u8,
    pub cols: u8,
    pub weight_capacity: f64,
    pub equip_slot: String,          // "back_pack" | "waist_pouch" | "chest_satchel"
    pub durability_cost_per_op: f64, // 每次包内增删操作扣耐久，默认 0.005
}
```

`ItemTemplate` 新增 `pub container_spec: Option<ContainerSpec>`。

`ItemCategory` 新增 variant `Container`。

TOML 解析层 `ItemTemplateToml` 新增对应 `ContainerSpecToml`，`parse_container_spec` 辅助函数校验 rows/cols > 0、weight_capacity > 0、equip_slot 合法。

### P0.2 新增装备槽常量

```rust
pub const EQUIP_SLOT_BACK_PACK: &str = "back_pack";
pub const EQUIP_SLOT_WAIST_POUCH: &str = "waist_pouch";
pub const EQUIP_SLOT_CHEST_SATCHEL: &str = "chest_satchel";

pub const BODY_POCKET_CONTAINER_ID: &str = "body_pocket";
pub const BODY_POCKET_ROWS: u8 = 2;
pub const BODY_POCKET_COLS: u8 = 3;
```

### P0.3 动态容器重建

新增 `fn rebuild_containers_from_equipment(inventory: &mut PlayerInventory, registry: &ItemRegistry)`：

1. 始终保留 `body_pocket` 2×3（如果不存在就创建空的）
2. 遍历 `EQUIP_SLOT_BACK_PACK` / `EQUIP_SLOT_WAIST_POUCH` / `EQUIP_SLOT_CHEST_SATCHEL` 三个装备槽：
   - 有装备且 `container_spec.is_some()` → 确保 `containers` 里有对应 id 的 `ContainerState`，尺寸匹配
   - 无装备 → 如果 `containers` 里有该 id 的容器**且容器非空**，拒绝卸除（或溢出，由 P3 处理）；为空则移除
3. 移除不属于以上四种 id 的任何 container（清理旧硬编码）

调用时机：
- 玩家 join 初始化（`instantiate_inventory_from_loadout` 后）
- `apply_inventory_move` 涉及背包槽装备/卸除时
- 背包破损时（P3）

### P0.4 负重公式重写

```rust
pub const BASE_CARRY_CAPACITY: f64 = 15.0;

pub fn compute_max_weight(inventory: &PlayerInventory, registry: &ItemRegistry) -> f64 {
    let mut max = BASE_CARRY_CAPACITY;
    for slot in [EQUIP_SLOT_BACK_PACK, EQUIP_SLOT_WAIST_POUCH, EQUIP_SLOT_CHEST_SATCHEL] {
        if let Some(item) = inventory.equipped.get(slot) {
            if let Some(template) = registry.get(&item.template_id) {
                if let Some(spec) = &template.container_spec {
                    max += spec.weight_capacity;
                }
            }
        }
    }
    max
}
```

`PlayerInventory.max_weight` 改为 **只读派生值**，不再存储在 struct 里——每次需要时调 `compute_max_weight`。或者保留字段但由 `rebuild_containers_from_equipment` 每次同步刷新。

**选方案 B（保留字段，rebuild 时刷新）**——改动最小，已有的 `sync_overloaded_marker` system 无需改签名。

### P0.5 装备校验扩展

`validate_move_semantics` 的 `InventoryLocationV1::Equip { slot }` 分支新增：

```rust
EquipSlotV1::BackPack | EquipSlotV1::WaistPouch | EquipSlotV1::ChestSatchel => {
    let spec = template.container_spec.as_ref().ok_or_else(|| {
        format!("item `{}` is not a container; cannot equip to {slot:?}", item.template_id)
    })?;
    let expected_slot = match slot {
        EquipSlotV1::BackPack => "back_pack",
        EquipSlotV1::WaistPouch => "waist_pouch",
        EquipSlotV1::ChestSatchel => "chest_satchel",
        _ => unreachable!(),
    };
    if spec.equip_slot != expected_slot {
        return Err(format!(
            "container `{}` equip_slot `{}` does not match target slot `{expected_slot}`",
            item.template_id, spec.equip_slot
        ));
    }
    Ok(())
}
```

**卸除背包校验**：当 `from` 是背包槽、`to` 是容器/hotbar 时，检查对应容器是否为空。非空则拒绝移动并返回 `"cannot unequip backpack: container not empty"`。

### P0 验收

- 单测：`ContainerSpec` TOML 解析正/反例（rows=0 / weight_capacity<0 / equip_slot 非法 → Err）
- 单测：`rebuild_containers_from_equipment` 装上背包 → containers 多一个、卸下 → 减一个、body_pocket 始终存在
- 单测：`compute_max_weight` 无背包=15.0、一个草包=23.0、三个包=累加
- 单测：`validate_move_semantics` 拒绝非 Container 物品进背包槽、拒绝 equip_slot 不匹配、拒绝卸除非空背包
- 单测：已有 `add_item_to_player_inventory` 在新容器布局下仍正确分配格子

---

## P1 — Schema + 网络协议适配

### P1.1 `ContainerIdV1` 从 enum 改 String

`server/src/schema/inventory.rs`：

```rust
// 旧
pub enum ContainerIdV1 { MainPack, SmallPouch, FrontSatchel }

// 新：直接用 String，serde 序列化为 snake_case 字符串
pub type ContainerIdV1 = String;
```

所有 match `ContainerIdV1::MainPack` 的代码改为字符串比较。

`InventoryLocationV1::Container { container_id, row, col }` 的 `container_id` 类型从 enum 变 String，反序列化加 `deserialize_non_empty_string_up_to_64` 校验。

**兼容性**：旧 client 发 `"main_pack"` / `"small_pouch"` / `"front_satchel"` 的 JSON 字符串格式与新 String 类型天然兼容（serde `rename_all = "snake_case"` 的 enum 序列化就是这些字符串）。

### P1.2 `EquipSlotV1` 扩展

```rust
pub enum EquipSlotV1 {
    // ... 已有 12 个 ...
    BackPack,
    WaistPouch,
    ChestSatchel,
}
```

client `EquipSlotType` 同步新增三个值。

### P1.3 `inventory_snapshot_emit` 适配

`server/src/network/inventory_snapshot_emit.rs` 里的 `container_id_to_v1` 转换函数从 match-enum 改为直接传 String。snapshot 中 `containers` 数组顺序改为：body_pocket 在首位，后续按装备槽顺序。

### P1.4 `InventorySnapshotV1` container 定义动态化

snapshot payload 的 `containers` 字段已是数组，每项包含 `id` / `name` / `rows` / `cols` / `items`——天然支持动态。只需确保 server 推新 id 时 client 不 crash。

### P1 验收

- 单测：`ContainerIdV1` 序列化/反序列化 round-trip（`"body_pocket"` / `"back_pack"` / 旧 `"main_pack"` 均可解析）
- 单测：snapshot 包含 body_pocket + 动态背包容器
- 单测：`EquipSlotV1::BackPack` serde 为 `"back_pack"`
- 单测：`inventory_move_intent` 用新 container_id 发送 → server 正确处理

---

## P2 — 背包物品模板 + 制作配方

### P2.1 物品模板

`server/assets/items/core.toml` 新增：

```toml
# ─── 背包 ─────────────────────────────────────────────────────────────────

[[item]]
id = "worn_grass_pouch"
name = "破草包"
category = "container"
grid_w = 2
grid_h = 2
base_weight = 0.25
rarity = "common"
spirit_quality_initial = 0.0
description = "不知哪个死人身上扒下来的草包。勉强能用，但缝线快散了。"

[item.container_spec]
rows = 3
cols = 3
weight_capacity = 8.0
equip_slot = "back_pack"
durability_cost_per_op = 0.008

[[item]]
id = "grass_pouch"
name = "小草包"
category = "container"
grid_w = 2
grid_h = 2
base_weight = 0.3
rarity = "common"
spirit_quality_initial = 0.0
description = "草根编的粗糙小包。能多带几样东西，但别指望装太多。"

[item.container_spec]
rows = 3
cols = 3
weight_capacity = 8.0
equip_slot = "back_pack"
durability_cost_per_op = 0.005
```

### P2.2 制作配方

`server/src/craft/mod.rs` 的 `register_basic_processing_recipes` 追加：

```rust
("basic.grass_pouch", "编草包", &[("grass_rope", 3)], 20 * 20, ("grass_pouch", 1)),
```

完整获取链：grass_fiber ×12 → grass_rope ×3（已有配方）→ grass_pouch ×1。

### P2.3 default.toml 重写

```toml
max_weight = 23.0   # BASE_CARRY 15.0 + worn_grass_pouch 8.0

# ─── 贴身口袋 2×3（固定，不可卸） ──────────────────────────────────────────
[[containers]]
id = "body_pocket"
name = "贴身口袋"
rows = 2
cols = 3

  [[containers.items]]
  row = 0
  col = 0
  template_id = "spirit_niche_stone"
  stack_count = 1

  [[containers.items]]
  row = 0
  col = 1
  template_id = "fengling_bone_coin"
  stack_count = 5

# ─── 背部破草包 3×3（装备产生的容器） ────────────────────────────────────────
[[containers]]
id = "back_pack"
name = "破草包"
rows = 3
cols = 3

  [[containers.items]]
  row = 0
  col = 0
  template_id = "spirit_grass"
  stack_count = 3

  [[containers.items]]
  row = 0
  col = 1
  template_id = "ningmai_powder"
  stack_count = 2

  [[containers.items]]
  row = 0
  col = 2
  template_id = "guyuan_pill"
  stack_count = 1

  [[containers.items]]
  row = 1
  col = 0
  template_id = "bone_spike"
  stack_count = 1

  [[containers.items]]
  row = 1
  col = 1
  template_id = "ash_spider_silk"
  stack_count = 1

  [[containers.items]]
  row = 2
  col = 0
  template_id = "ci_she_hao_seed"
  stack_count = 4

  [[containers.items]]
  row = 2
  col = 1
  template_id = "ning_mai_cao_seed"
  stack_count = 4

bone_coins = 7

# ─── 装备 ─────────────────────────────────────────────────────────────────
[[equip]]
slot = "back_pack"
template_id = "worn_grass_pouch"
durability = 0.3

[[equip]]
slot = "chest"
template_id = "fake_spirit_hide"
durability = 0.7

[[equip]]
slot = "main_hand"
template_id = "iron_sword"
durability = 0.5

# ─── 战斗 hotbar ─────────────────────────────────────────────────────────
[[hotbar]]
index = 0
template_id = "ningmai_powder"
```

**起手物品精简**：body_pocket 6 格只放龛石+骨币，其余全在破草包里。iron_sword 直接装备 main_hand 不占格子。铁锄/灵木苗等较重的起手物品移除——空间有限，玩家需要自己去获取。

### P2 验收

- 单测：`load_item_registry` 成功加载 `worn_grass_pouch` / `grass_pouch`，`container_spec` 字段正确
- 单测：`load_default_loadout` 成功加载新 loadout，body_pocket 和 back_pack 容器各有正确物品
- 单测：制作配方 `basic.grass_pouch` 在 `CraftRegistry` 中注册且材料列表 = `[("grass_rope", 3)]`
- 单测：`instantiate_inventory_from_loadout` 产出的 inventory 有 body_pocket + back_pack 两个容器

---

## P3 — 背包耐久 + 破损溢出

### P3.1 耐久扣减

在 `add_item_to_player_inventory_inner` / `detach_instance` / `apply_inventory_move` 中，当操作涉及某个**背包槽展开的容器**时（container_id ∈ {back_pack, waist_pouch, chest_satchel}），找到对应装备槽的背包物品，扣减耐久：

```rust
fn apply_backpack_wear(
    inventory: &mut PlayerInventory,
    registry: &ItemRegistry,
    container_id: &str,
) -> Option<BackpackBreakEvent> {
    let slot = container_id_to_equip_slot(container_id)?;
    let backpack = inventory.equipped.get_mut(slot)?;
    let template = registry.get(&backpack.template_id)?;
    let cost = template.container_spec.as_ref()?.durability_cost_per_op;
    backpack.durability = (backpack.durability - cost).max(0.0);
    if backpack.durability <= 0.0 {
        Some(BackpackBreakEvent { slot: slot.to_string() })
    } else {
        None
    }
}
```

`body_pocket` 操作**不扣耐久**（没有对应装备）。

### P3.2 破损溢出

背包耐久归零时触发 `handle_backpack_break`：

1. 从装备槽移除背包物品
2. 将对应容器内所有物品转为 `DroppedItemEvent`（掉在玩家脚下）
3. 移除该容器
4. 重算 `max_weight`（`rebuild_containers_from_equipment`）
5. 向 client 推 `inventory_snapshot`（全量刷新，因为容器结构变了）

narration 模板（接 agent，scope = player）：
- `"草包的缝线终于散了。几样东西滚落在地。"`
- `"腰间的小袋磨出了洞，里面的零碎掉了一地。"`

### P3.3 死亡 + 背包交互

- 死亡掉落（`apply_death_drop_to_inventory`）：照常按 50% 规则从所有容器里选物品掉落。**背包物品本身也参与死亡掉落 roll**——如果背包被选中掉落，内容物一起掉（整包掉落）
- 终结掉落（`apply_termination_drop_on_terminate`）：全部掉落，含背包

### P3 验收

- 单测：`apply_backpack_wear` 每次调用扣减正确值，body_pocket 不扣
- 单测：背包耐久从 0.01 扣到 ≤0 → 返回 `BackpackBreakEvent`
- 单测：`handle_backpack_break` 移除容器 + 物品转 DroppedItemEvent + max_weight 下降
- 单测：背包卸除校验——容器非空时 `apply_inventory_move` 拒绝卸除
- 单测：死亡掉落 roll 中背包 + 内容物联动掉落
- 单测：背包耐久=0.3 的 worn_grass_pouch，durability_cost_per_op=0.008，操作 38 次后破损

---

## P4 — Client "行囊" Tab + 背包装备

### P4.1 `EquipSlotType` 扩展

`client/src/main/java/com/bong/client/inventory/model/EquipSlotType.java` 新增：

```java
BACK_PACK("背包"),
WAIST_POUCH("腰包"),
CHEST_SATCHEL("前挂");
```

### P4.2 InspectScreen "行囊" Tab

在 InspectScreen 的容器 tab 栏新增 "行囊" tab。切换到此 tab 时，右侧面板内容替换为：

**布局**（自上而下）：

1. **三个背包装备槽**——每个槽位显示：
   - 2×2 格的物品图标（空槽显示半透明轮廓 + 槽位名称）
   - 槽位标签（"背部" / "腰间" / "胸前"）
   - 已装备时：背包名称、容量使用率（`已用/总格`）、耐久条
   - 可从其他 tab 的网格拖入 Container 类物品

2. **负重详情栏**——
   - 总负重条：`████████░░░  12.3 / 23.0`
   - 分项：`基础 15.0` + 各背包 `+8.0` 的 breakdown
   - 超重时负重条变红 + 文字 `"负重过载"`

3. **贴身口袋 2×3 网格**——
   - 始终可见（不受 tab 切换影响的固定容器）
   - 和其他容器 tab 的 BackpackGridPanel 行为一致（拖拽、tooltip）

### P4.3 动态容器 Tab 渲染

已有的容器 tab 切换机制（`containerGrids[]` + tab 栏）改为从 server snapshot 动态构建：

- server 推过来几个 container 就渲染几个 tab（body_pocket 除外——它在行囊 tab 里显示）
- tab 名称 = `ContainerState.name`（"破草包" / "小草包" 等）
- 旧的 `DEFAULT_CONTAINERS` 作为 fallback 仅在 server 未推送时使用

### P4.4 背包耐久 HUD 反馈

背包耐久 < 20% 时：
- 对应容器 tab 标签闪烁橙色（每 40 tick 切换一次 opacity 0.6↔1.0）
- "行囊" tab 里该槽位的耐久条变橙
- 耐久 < 5% 时变红 + tab 标签持续闪烁

背包破损时：
- 事件流显示 `"[背包] 草包散了，物品掉落"`
- 对应容器 tab 消失（server 推新 snapshot，tab 数量减少）

### P4 验收

- 客户端单测：`EquipSlotType.BACK_PACK` / `WAIST_POUCH` / `CHEST_SATCHEL` 存在且 displayName 正确
- 客户端单测：`InventoryModel.Builder.containers()` 接收不同数量 ContainerDef 后 build 正确
- 客户端单测：行囊 tab 负重计算 breakdown 正确（mock 数据）
- 实机验证（需人工）：
  - 打开 InspectScreen → 看到"行囊"tab → 点击进入 → 三个槽位正确显示
  - 破草包已装备 → back_pack 槽有图标 + 耐久条
  - 从网格拖背包物品到行囊 tab 的空槽 → 装备成功 → 新容器 tab 出现
  - 背包内操作反复进行 → 耐久下降 → 耐久 <20% 闪橙 → 耐久归零 → 物品散落 + tab 消失

---

## P5 — 饱和化测试

### P5.1 Server 单测矩阵

| 模块 | 覆盖要求 |
|------|---------|
| ContainerSpec TOML 解析 | 正例 ×3（back_pack/waist_pouch/chest_satchel）+ 反例 ×5（rows=0/cols=0/weight_capacity≤0/equip_slot 非法/equip_slot 空） |
| rebuild_containers_from_equipment | 零背包=只有 body_pocket；装一个=+1 container；装三个=+3；卸空背包=移除；卸非空=拒绝 |
| compute_max_weight | 无背包=15.0；一个 cap=8 → 23.0；三个 cap=8+12+15 → 50.0；卸背包后回到 15.0 |
| validate_move_semantics 背包槽 | Container 物品 → back_pack ✓；非 Container → back_pack ✗；equip_slot 不匹配 ✗；卸除非空 ✗ |
| apply_backpack_wear | body_pocket 操作不扣；back_pack 操作扣 cost_per_op；多次扣减到 0 返回 break event |
| handle_backpack_break | 容器清空 + 物品转 DroppedItemEvent + 装备移除 + max_weight 刷新 |
| 死亡掉落联动 | 背包被 roll 中 → 内容物一起掉；背包未 roll 中 → 内容物按自身 roll |
| 堆叠/格子分配 | 在 3×3 容器中放入各种尺寸物品 → 不重叠 → 满了 → Err |
| 制作配方 | grass_rope ×3 → grass_pouch ×1 注册正确 |
| loadout 加载 | body_pocket + back_pack 双容器 + worn_grass_pouch 装备 + 起手物品归位 |

### P5.2 Client 单测矩阵

| 模块 | 覆盖要求 |
|------|---------|
| EquipSlotType 扩展 | 新三个值存在 + displayName |
| InventoryModel 动态 containers | 0~4 个 ContainerDef build → 正确 |
| ContainerIdV1 String 解析 | "body_pocket" / "back_pack" / 旧 "main_pack" round-trip |
| 行囊 tab 负重 | mock 数据 breakdown 算对 |

### P5.3 集成测试

- 端到端：server 启动 → 玩家 join → snapshot 包含 body_pocket + back_pack → client 渲染 2 个容器 tab + 行囊 tab
- 端到端：`/give grass_pouch 1` → 玩家拖到 waist_pouch 槽 → 新容器出现 → 拖物品进去 → 拖出来 → 卸背包
- 端到端：反复操作 worn_grass_pouch 直到破损 → 物品掉地 → 容器消失 → 编新草包装上

---

## 迁移注意事项

### 旧 container id 兼容

`"main_pack"` / `"small_pouch"` / `"front_satchel"` 这三个 id 不再作为默认容器出现。但 **persistence 尚未实装**（plan-inventory-v1 已延后），所以不存在旧存档迁移问题——每次 server 重启都是 fresh loadout。

如果后续 persistence 在本 plan 之前实装，需要加迁移逻辑：检测旧 container id → 重映射为 body_pocket + 新背包容器。

### 影响面其他模块

- `forge/inventory_bridge.rs`：硬编码了 `MAIN_PACK_CONTAINER_ID` → 改为 `find_first_fit_container_location`（已有该函数）
- `network/client_request_handler.rs`：测试中硬编码 `"main_pack"` → 更新为 `"body_pocket"` 或参数化
- `network/weapon_equipped_emit.rs`：测试中硬编码 → 同上

### 后续可扩展（本 plan 不做）

- 兽皮包（fauna 掉落 + 草绳 → 4×4, cap=15, back_pack）
- 灵木匣（spiritwood + 铁钉 → 3×5, cap=20, back_pack, 保鲜加成）
- 储物袋（高阶炼器 → 5×7, cap=35, back_pack）
- 腰包/前挂类别的具体物品
- 背包外观渲染（玩家模型上显示背包）
- 背包内物品的灵气流失加速（worldview §九 操作磨损的深度实现）
