# Bong · plan-food-v1 · 骨架

**灵食与陈化系统**——接入 `shelflife` 为食物类物品（凡俗食物 / 灵果 / 陈酒 / 陈醋）注册 DecayProfile，实装 `consume_food` 接口给玩家临时修炼加速效果，并接入冰窖容器降低腐败速率。"末法时代一切都在衰败，但陈化酒等上古封印产物反例 = 时间亦可积淀"——这是 shelflife 存在意义的文化根基。

**来源**：`plan-shelflife-v1.md` §M7 DecayProfile 定稿节"plan-food-v1 注册 `chen_jiu_v1` + `chen_cu_v1`"

**交叉引用**：`plan-shelflife-v1.md` ✅（DecayProfileRegistry / consume_with_shelflife / 陈化曲线 / 冰窖容器钩子）· `plan-botany-v1.md` ✅（灵草采集，灵草是部分灵食原材料）· `plan-alchemy-v1.md` ✅（丹药老坛模式参考）· `plan-inventory-v1.md` ✅（item 系统 / NBT 存储）· `plan-social-v1.md` ✅（骨币 / 以物易物场景中食物作为交易品）

**worldview 锚点**：
- **§十 资源与匮乏**：灵草是"区域灵气 > 0.3 才会生长"的稀缺资源；灵果/灵食是灵草的食用形态，末法时代灵食极稀；凡俗食物腐败=世界观物质匮乏的物理表达
- **§九 陈化经济**（shelflife 描述）：陈酒越久越贵，陈醋作为独立 item ID 切换，两者是末法世界"时间就是资产"的经济支柱之一
- **§九 交易方式**：食物/灵酒作为交易品在以物易物场景中流通——炼丹疯子不收骨币只收毒草，类比食物流通的非货币经济位置

**qi_physics 锚点**：
- 食用灵果/灵酒给临时 qi_regen 加速系数，走 `cultivation::session::apply_food_bonus(entity, bonus_factor, duration_ticks)`；**不直接修改 `qi_current`**，只修改 regen 速率（守恒律不受影响）
- 食物腐败/陈化走 `shelflife::consume_with_shelflife`，不引入新物理常数

**前置依赖**：
- `plan-shelflife-v1` ✅ — DecayProfileRegistry + consume_with_shelflife + 陈化曲线框架
- `plan-inventory-v1` ✅ — item NBT + ContainerSlot + item_id 注册
- `plan-botany-v1` ✅ — 灵草 item 体系（灵食原材料来源）

**反向被依赖**：
- `plan-shelflife-v1` §M7 — `build_default_registry()` 缺 food profiles（`chen_jiu_v1` / `chen_cu_v1` / 凡俗食物 Spoil）
- `plan-economy-v1` ✅（finished）— 死物/腐败品次级市场（陈醋 / 败食）需本 plan 先落 item ID

---

## 接入面 Checklist

- **进料**：`botany::PlantRegistry`（灵草材料 / 灵果 spawn 点）/ `inventory::ItemRegistry`（item_id 注册入口）/ `shelflife::DecayProfileRegistry`（注册 food profiles）/ `cultivation::session`（regen hook）
- **出料**：`item.food.*` item ID 集合 → inventory 可持有；`shelflife` profile 注册（`chen_jiu_v1` / `chen_cu_v1` / `food_spoil_v1`）→ shelflife 消费链路；`FoodBonus` Component → cultivation session 临时效果
- **共享类型**：新增 `FoodBonus { factor: f32, remaining_ticks: u32 }` Component（不新增 Event，消费点触发 `ApplyFoodBonusCommand`）
- **跨仓库契约**：server 纯食用逻辑，无 IPC；client 食用动作复用 vanilla eating animation 或添加简单粒子效果（归 P3 client wiring）；agent 可从 `world_state` 的 zone 灵气字段间接感知灵食消耗
- **worldview 锚点**：§十 资源与匮乏（灵草/灵食）/ §九 陈化经济（陈酒陈醋）

---

## 阶段总览

| 阶段 | 内容 | 状态 | 验收 |
|------|------|------|------|
| **P0** | 食物 item 注册 + shelflife Spoil profile（凡俗食物 + 灵果） | ⬜ | 单测：item_id `food.mundane.*` / `food.spirit_fruit.*` 注册；consume_with_shelflife 返回正确 SpoilCheckOutcome |
| **P1** | 陈酒/陈醋 Age profile（`chen_jiu_v1` → 峰值窗口 → `chen_cu_v1` item ID 切换） | ⬜ | 单测：age 曲线 peak_tick 前 quality_factor >1.0；过峰触发 item ID 切换为 `food.chen_cu` |
| **P2** | `consume_food` 接口 + `FoodBonus` 临时修炼加速 | ⬜ | 单测：食用灵果后 FoodBonus Component 挂载；cultivation session 读取 bonus_factor；duration_ticks 耗尽后自动移除 |
| **P3** | 冰窖容器接入（`ContainerType::IceCellar` → Spoil rate ×0.3） | ⬜ | 单测：同一 item 在冰窖内 spoil 速率 vs 常温差异 ≥70% |

---

## §0 物品清单（P0 实装）

| item_id | 类型 | shelflife | 备注 |
|---------|------|-----------|------|
| `food.mundane.cooked_meat` | 凡俗熟肉 | Spoil, half_life ≈ 3d | 野兽肉烹制 |
| `food.mundane.chen_bing` | 硬饼干 | Spoil, half_life ≈ 30d | 无灵气含量 |
| `food.spirit_fruit.ling_guo` | 灵果 | Spoil, half_life ≈ 2d | 临时 qi_regen +20% |
| `food.spirit_wine.chen_jiu` | 新酿陈酒 | Age: PeakAndFall, peak ≈ 365d | 过峰 → `food.spirit_wine.chen_cu` |
| `food.spirit_wine.chen_cu` | 陈醋（陈酒过峰变质） | Spoil | 独立 item ID，可作炼丹辅料 |

---

## §1 开放问题（P0 决策门前需收口）

1. **灵果 spawn**：直接在 botany 灵草 spawn 中新增一个 `PlantKind::LingGuo` 结点，还是从采集 botany 现有植物获得？——建议归 botany PlantRegistry 扩展，本 plan 只注册 item，不修改 worldgen
2. **冰窖触发方式**：P3 冰窖走特殊 block tag（`bong:ice_cellar`）还是用 `ContainerType` enum 扩展？——建议复用 shelflife `ContainerType` 扩展（避免新增 block tag 系统）
3. **骨币/真元是否适用于酿酒原料**：陈酒酿造需要骨币支付启动真元消耗吗？——初版不引入骨币消耗，P2 只走 inventory craft 消耗灵草材料
