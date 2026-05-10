# Bong · plan-armor-visual-v1 · 凡物盔甲视觉 + craft 接入

凡物盔甲基础视觉系统。当前 `plan-armor-v1` ✅ finished 建立了盔甲 server 逻辑（防御值/耐久/附着槽），但**游戏内没有实际盔甲物品可穿**。本 plan 做三件事：① 创建 6 套凡物盔甲（不同材质 × 换色 vanilla 甲模型暂用）② 接入 craft 系统（手搓制作配方）③ 基础视觉差异（颜色/材质 tint 区分）。不做自定义 3D 模型——用 vanilla leather armor 染色方案暂时撑住，后续 plan 再做 BlockBench 模型。

**世界观锚点**：`worldview.md §九` 经济（凡物盔甲用骨币/兽骨/铁矿手搓，不是灵器）· `§四` 战斗近身肉搏 → 盔甲是防崩拳/断肢的物理防护 · `§十` 资源匮乏 → 盔甲破了只能修不能随便换

**前置依赖**：
- `plan-armor-v1` ✅ → ArmorSlot / ArmorStat / ArmorDurability / ArmorComponent server 全套
- `plan-craft-v1` ✅ → CraftRegistry / CraftRecipe / 手搓通用系统
- `plan-inventory-v2` ✅ → 装备栏 / 物品系统
- `plan-weapon-v1` ✅ → 武器材质分类（复用材质 enum：骨/铁/铜/兽皮/灵布/残卷缠）
- `plan-shelflife-v1` ✅ → 耐久退化（盔甲耐久走 shelflife 同源）
- `plan-client` ✅ → Fabric client 基础

**反向被依赖**：
- `plan-forge-v1` ✅ → 未来高级盔甲走锻造而非手搓（本 plan 只做凡物手搓级）
- `plan-item-visual-v1` ✅ → 物品图标系统已就绪（本 plan 产出的盔甲 icon 用 gen.py 生成）

---

## 接入面 Checklist

- **进料**：`armor::ArmorComponent { slot, material, defense, durability }` / `craft::CraftRegistry` / `craft::CraftRecipe` / `inventory::EquipmentSlot` / MC `ArmorItem` / `DyeableArmorItem`
- **出料**：6 套凡物盔甲物品注册（server `armor::mundane::*`）+ 6 craft 配方注册 + client 6 套 armor tint 配置（复用 vanilla leather armor 染色 API）+ 6 张物品 icon（gen.py 生成）+ 装备时视觉效果
- **跨仓库契约**：server 物品注册 → client `ArmorTintRegistry`（material → color 映射）

---

## §0 设计轴心

- [ ] **凡物级**：这些盔甲是普通修士日常防护，不是灵器——防御值低、耐久有限、手搓可得
- [ ] **暂用 vanilla 换色**：用 MC leather armor 的 `DyeableArmorItem` API 做颜色区分，不做自定义模型
- [ ] **6 种材质 = 6 种颜色**：骨甲(灰白) / 兽皮甲(棕) / 铁甲(深灰) / 铜甲(古铜) / 灵布衫(淡青) / 残卷缠甲(暗黄)
- [ ] **接 craft 系统**：每套甲 = 对应材料 ×4-6 手搓产出，配方注册到 CraftRegistry
- [ ] **耐久可见**：装备后 armor icon tooltip 显示耐久条（复用 MC durability bar）

---

## 6 套凡物盔甲规格

| 材质 | 颜色 Hex | 防御值 | 耐久 | craft 材料 | 适用 |
|------|----------|--------|------|-----------|------|
| 骨甲 | #D0C8B8 灰白 | 3 | 80 | 骨币 ×6 | 醒灵新手起步 |
| 兽皮甲 | #8B6914 棕 | 5 | 120 | 兽皮 ×4 + 骨币 ×2 | 引气日常 |
| 铁甲 | #555555 深灰 | 8 | 200 | 铁矿 ×5 + 骨币 ×3 | 凝脉标配 |
| 铜甲 | #B87333 古铜 | 7 | 160 | 铜矿 ×4 + 兽皮 ×2 | 凝脉轻装 |
| 灵布衫 | #88BBCC 淡青 | 4 | 100 | 灵布 ×3 + 灵草 ×2 | 修炼用（轻便） |
| 残卷缠甲 | #A08030 暗黄 | 6 | 140 | 残卷碎片 ×4 + 骨币 ×4 | TSY 探索用 |

每套包含 4 件：头 / 胸 / 腿 / 靴（防御值按 MC 比例分配：胸 40% / 腿 30% / 靴 15% / 头 15%）

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|----|------|----|
| P0 | server 6 套盔甲物品注册 + ArmorComponent 填充 + craft 配方注册 + 基础测试 | ⬜ |
| P1 | client armor tint 系统（material → color 映射）+ 装备穿戴视觉验证 | ⬜ |
| P2 | 6 张物品 icon 生成（gen.py item 档）+ tooltip 耐久条 + 材质名显示 | ⬜ |
| P3 | 装备增强视觉：穿戴时微粒子 flash + 破损警告（耐久 < 20% 闪红）+ 破碎音效 | ⬜ |
| P4 | 6 套 × 4 件 × craft/equip/durability 饱和化测试 | ⬜ |

---

## P0 — server 物品 + craft 配方 ⬜

### 交付物

1. **`armor::mundane` 模块**（`server/src/armor/mundane.rs`）
   - `MundaneArmorMaterial` enum：`Bone / Hide / Iron / Copper / SpiritCloth / ScrollWrap`
   - `MundaneArmorItem` struct：material + slot + 对应 ArmorComponent
   - `register_mundane_armors(registry)` → 注册 24 个物品（6 材质 × 4 部位）
   - 每个物品有唯一 `item_id`：`bong:armor_bone_helmet` / `bong:armor_hide_chestplate` 等

2. **craft 配方注册**（`server/src/armor/mundane_recipes.rs`）
   - 6 套完整配方注册到 `CraftRegistry`
   - 每套 4 件分别注册（头 = 材料 ×2 / 胸 = ×3 / 腿 = ×2 / 靴 = ×1.5 向上取整）
   - 配方 category: `ArmorCraft`

3. **20 单测**
   - 6 材质 × 防御值正确 / 耐久正确 / craft 配方输入输出正确 / 装备槽正确

### 验收抓手
- 测试：`server::armor::tests::mundane_bone_defense_3` / `server::armor::tests::craft_recipe_iron_chestplate` / `server::armor::tests::all_24_items_registered`
- 手动：`/give` 骨甲 → 装备 → 防御值生效

---

## P1 — client armor tint ⬜

### 交付物

1. **`ArmorTintRegistry`**（`client/src/main/java/com/bong/client/armor/ArmorTintRegistry.java`）
   - `HashMap<String, Integer>` material_id → ARGB color
   - 6 颜色映射注册
   - 实现：hook `ArmorFeatureRenderer`，对 bong armor item 应用 `DyeableArmorItem` 染色 API

2. **vanilla leather armor 复用**
   - 所有 bong armor 在 client 侧伪装为 leather armor + tint（不需要自定义模型文件）
   - 通过 `ItemStack.getOrDefault(DataComponentTypes.DYED_COLOR, color)` 设置颜色

3. **装备穿戴同步**
   - server 装备变化 → client 收到 equipment update → 按 material 查 tint → 渲染对应颜色甲

### 验收抓手
- 测试：`client::armor::tests::bone_armor_tint_matches` / `client::armor::tests::all_6_materials_distinct_color`
- 手动 WSLg：穿骨甲 → 灰白色 / 穿兽皮 → 棕色 / 穿铁甲 → 深灰 → 每套明显可区分

---

## P2 — 物品 icon + tooltip ⬜

### 交付物

1. **6 张物品 icon**（通过 `scripts/images/gen.py` item 档生成）
   - 每套盔甲一张代表 icon（胸甲形态，对应颜色）
   - 输出到 `client/src/main/resources/assets/bong-client/textures/gui/items/armor/`
   - 命名：`armor_bone.png` / `armor_hide.png` / `armor_iron.png` / `armor_copper.png` / `armor_spirit_cloth.png` / `armor_scroll_wrap.png`

2. **tooltip 增强**
   - 物品名称颜色按材质（灰白/棕/深灰/古铜/淡青/暗黄）
   - 防御值行：`防御: +3`（绿色）
   - 耐久条：MC 原生 durability bar 复用
   - 材质描述行：`凡物·骨制` / `凡物·兽皮` / `凡物·铁制` 等（灰色小字）

### 验收抓手
- gen.py 产出 6 张 icon + client 正确加载显示
- tooltip 信息完整 + 颜色正确

---

## P3 — 装备视觉增强 ⬜

### 交付物

1. **穿戴 flash**：装备瞬间全身微白闪 0.1s（`OverlayQuadRenderer` white alpha 0.1）
2. **破损警告**：耐久 < 20% → 装备 icon 闪红 + HUD 角落 toast "甲胄将破"（一次性）
3. **破碎音效**：耐久归零 → `armor_break.json`（`minecraft:entity.item.break`(pitch 0.7, volume 0.4)）+ 碎片粒子（`BongSpriteParticle` × 4 金属碎片向外飞散）
4. **修复提示**：破碎后 tooltip 显示"已损坏·不可穿戴"（红色）+ 修复需要同材质 ×2 hand-craft

### 验收抓手
- 测试：`client::armor::tests::low_durability_flash_red` / `server::armor::tests::broken_armor_unequippable`
- 手动：穿盔甲 → 白闪 → 被打很多次 → 闪红警告 → 破碎 → 碎片粒子 + 音效

---

## P4 — 饱和化测试 ⬜

### 交付物

1. **全矩阵**：6 材质 × 4 部位 × craft/equip/break = 72 基础 case
2. **craft 链验证**：从原材料 → 手搓 → 获得甲 → 装备 → 战斗消耗耐久 → 破碎 → 修复
3. **视觉验证**：6 套穿在身上颜色互相可区分（截图对比）

### 验收抓手
- 全 24 物品 craft + equip + break 链路 e2e
- 6 套颜色区分截图

---

## Finish Evidence（待填）

- **落地清单**：server `armor::mundane::*` / 24 物品 / 6 craft 配方 / client `ArmorTintRegistry` / 6 icon / tooltip / 穿戴 flash / 破损警告 / 破碎音效+粒子
- **关键 commit**：P0-P4 各自 hash
- **遗留 / 后续**：自定义 3D 盔甲模型（需 plan-armor-model-v1 + BlockBench）/ 灵器级盔甲走 forge 而非 craft / 盔甲附着（灵核/符文）视觉
