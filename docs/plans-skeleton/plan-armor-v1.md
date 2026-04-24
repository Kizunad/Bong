# Armor Mitigation · plan-armor-v1（骨架）

> 装备护甲对战斗伤害的减免系统：`DerivedAttrs.defense_power` 骨架已在 `combat/components.rs` 存在但闭环未通——装备无 armor 数值字段、没人写入 `defense_power`、`Wound` 结算不读防御。**骨架阶段**：列接口 + 决策点 + `WoundKind × BodyPart` 防护矩阵，不下笔实装。
> 交叉引用：`worldview.md §四`（战斗系统）· `worldview.md §五`（战斗流派 — 体修"不依赖外物"态度）· 预期 `worldview.md §四.零`（战力分层 — 护甲如何嵌入三层模型，待起稿）· `plan-cultivation-v1 §2`（修炼侧不涉防护，接口隔离）

---

## §-1 前提（现有代码基线）

### Server 端已有能力

| 能力 | 位置 | 备注 |
|------|------|------|
| `DerivedAttrs { attack_power, defense_power, move_speed_multiplier }` component | `server/src/combat/components.rs:222-237` | **默认全 1.0**，目前无装备写入 |
| `Wounds { entries, health_current, health_max }` + 7 段 `BodyPart` | `components.rs:32-72` | Head/Chest/Abdomen/ArmL/ArmR/LegL/LegR |
| `WoundKind` 5 种 | `components.rs:43-50` | Cut / Blunt / Pierce / Burn / Concussion |
| 部位状态阈值（`HEAD_STUN_SEVERITY_THRESHOLD=0.5` / `LEG_SLOWED_SEVERITY_THRESHOLD=0.3`） | `components.rs:27-30` | 护甲降档后这些阈值自动联动 |
| `JIEMAI_DEFENSE_WINDOW_MS=200` 截脉主动防御 | `components.rs:21-25` | 与被动护甲并行 |
| `DefenseStance` + `UnlockedStyles`（截脉/替尸/绝灵涡流）| `components.rs:270-321` | 主动防御流派，与本 plan 并行（§4.1 定顺序） |
| `TICKS_PER_SECOND: u64 = 20` | `components.rs:11` | 所有 tick 相关数值以 20TPS 为准 |

### Client 端已有能力

| 能力 | 位置 | 备注 |
|------|------|------|
| `EquipSlotType.HEAD/CHEST/LEGS/FEET` 护甲槽 | `client/.../InventoryEquipRules.java:72` | 已做槽位资格判定 |
| `canEquip()` 类型/占用校验 | `InventoryEquipRules.java:49-74` | 只判资格，**不管数值** |
| 16 细段 `BodyPart` | `inventory/model/BodyPart.java` | 通过 `WoundLayerBinding` 从 server 7 粗段展开 |
| `WoundLevel` 6 档（INTACT/BRUISE/ABRASION/LACERATION/FRACTURE/SEVERED） | `inventory/model/WoundLevel.java` | **护甲降档作用于此轴** |

### 关键空缺（本 plan 要补的）

1. **装备 schema 无 armor 数值字段** — 护甲目前只有外观与种类，没有 `ArmorProfile`
2. **`DerivedAttrs.defense_power` 无人写入** — 默认 1.0，缺少 `装备 → DerivedAttrs` 同步 system
3. **Wound 结算不读 `defense_power`** — 攻击输出直接入 `Wounds.entries`，没有减免层
4. **按 `WoundKind × BodyPart` 分型的矩阵未建** — "板甲挡 Cut 挡 Pierce、但挡 Burn 差" 的常识没编码
5. **护甲耐久/破损未建** — 装备无 durability 字段

---

## §0 设计轴心（骨架阶段已定稿，不再动）

1. **护甲作用于"降档 `WoundLevel`"，不直接减 HP 数值** — 对齐 `worldview.md §四` + 预期 `§四.零` 的"伤口是定性状态"设计（FRACTURE → LACERATION 比 `-20 HP` 有游戏意义）
2. **`WoundKind × BodyPart` 二维矩阵** — 不做"一个 defense 数值减所有伤"的简化；板甲挡 Cut/Pierce，皮甲缓 Blunt，无甲面对 Burn/Concussion 都弱
3. **装备作用点是 `DerivedAttrs`**（不新建组件链路），新增 `defense_profile` 字段承载二维矩阵
4. **体修流派绕过装备**（对齐 `worldview.md §五 体修`"不依赖外物"）— 体修 buff 作用于 `defense_power` 基础乘数，相当于"内力替代护甲"
5. **主动防御（`DefenseStance`）与被动护甲顺序固定**：`attacker output → DefenseStance → armor_mitigation → Wound 写入`（§4.1 展开）
6. **骨架不引入"法宝级"分支** — 上古护甲（`§十六` 遗物）作为 `ArmorProfile` 数值极端点，走相同接口
7. **护甲耐久独立维度**（不与真元/经脉混）— 破损护甲防护降级而非失效；修复渠道归锻造 / 灵草 plan

---

## §1 数据模型扩展

### 1.1 装备 `ArmorProfile`（新）

```rust
// server/src/combat/armor.rs（新文件）
pub struct ArmorProfile {
    pub slot: EquipSlot,                           // HEAD / CHEST / LEGS / FEET
    pub body_coverage: Vec<BodyPart>,              // 一件甲可覆盖多部位（Q1）
    pub kind_mitigation: HashMap<WoundKind, f32>,  // 0.0=不减, 1.0=完全免疫
    pub durability_cur: u32,
    pub durability_max: u32,
    pub broken_multiplier: f32,                    // durability=0 时的衰减系数（e.g., 0.3）
}
```

### 1.2 `DerivedAttrs` 扩展

```rust
pub struct DerivedAttrs {
    pub attack_power: f32,
    pub defense_power: f32,                         // 既有，通用乘数（体修 buff 作用点）
    pub move_speed_multiplier: f32,
    pub defense_profile: HashMap<(BodyPart, WoundKind), f32>,  // 新增
}
```

### 1.3 装备 → `DerivedAttrs` 同步 system（新）

每当 `PlayerEquipment` 变动（`Changed<PlayerEquipment>`），遍历装备护甲 → 聚合 `body_coverage × kind_mitigation` → 写入 `defense_profile`。多件护甲覆盖同 `(BodyPart, WoundKind)` 时**取最大**（不叠加——避免三件甲叠成无敌）。

### 1.4 Wound 结算插入点（`combat/resolve.rs`）

```
// 现有（推断）: attacker.output → wound → Wounds.entries.push
// 新流程: attacker.output → DefenseStance → armor_mitigation → wound → push
//
// apply_armor_mitigation(wound, derived):
//   let m = derived.defense_profile.get(&(wound.location, wound.kind)).copied().unwrap_or(0.0);
//   wound.severity *= 1.0 - m;
//   emit ArmorDurabilityChanged event（耐久消耗）
```

---

## §2 护甲档次 × `WoundKind` 二维矩阵（骨架示意）

骨架阶段 4 档 × 5 kind。数值是 `kind_mitigation`（0-1，受击 severity × (1-mitigation)）。

| 档次 / 类型 | Cut | Blunt | Pierce | Burn | Concussion |
|-------------|-----|-------|--------|------|------------|
| 布甲 | 0.10 | 0.20 | 0.05 | 0.00 | 0.10 |
| 皮甲 | 0.25 | 0.30 | 0.20 | 0.10 | 0.15 |
| 板甲 | 0.50 | 0.40 | 0.55 | 0.15 | 0.20 |
| 灵纹甲（法宝） | 0.35 | 0.35 | 0.35 | 0.40 | 0.35 |

- 板甲偏物理、灵纹甲均衡但上限低
- 体修"自带护甲"不用矩阵，走 `defense_power` 基础加成（§4.2）
- 具体数值平衡是 Q2（playtest 校准）

---

## §3 护甲耐久与破损（骨架）

- 每次受击消耗耐久：`durability_cur -= max(1, round(wound.severity × armor_cost_factor × 10))`（`armor_cost_factor` 初值 0.1-0.3，Q4）
- `durability_cur = 0` → 破损状态（不完全失效，`kind_mitigation *= broken_multiplier`，e.g., 0.3）
- 修复渠道（Q5）：锻造重淬（`plan-forging-v1` 未来）/ 灵草外敷（局部）/ 特殊 NPC
- 新 event：`ArmorDurabilityChanged { entity, slot, cur, max, broken }` 推给 agent（天道叙事用）

---

## §4 与现有战斗流派 / 修炼的耦合

### 4.1 `DefenseStance` 与装备的优先级（定顺序）

```
attacker.output
  ↓
DefenseStance 检查（截脉 / 替尸 / 绝灵涡流）
  ├─ 截脉命中（200ms 窗口）→ severity ×= 0 或 ×= JIEMAI_CONTAM_MULTIPLIER（装备减免不再叠加）
  ├─ 替尸 fake_skin_layers > 0 → 完全吸收，装备耐久**保留**（replace kill）
  ├─ 绝灵涡流激活 → severity ×= 0.5 → **继续走装备减免**
  └─ None → 直接走装备减免
  ↓
装备减免（apply_armor_mitigation）
  ↓
Wound 写入 Wounds.entries
```

### 4.2 体修流派（`worldview.md §五`）

- 体修"不依赖外物"——走 buff 路径：`defense_power` 基础 ×= 1.3（具体系数 Q9）
- 体修**仍可穿护甲**，但体修 buff 和护甲 `kind_mitigation` **独立相乘**（不是叠加）
- Q9：体修的基础加成是否按 `BodyPart` 区分（硬化四肢但不护头）？

### 4.3 剑修 / 毒蛊师 / 雷法 / 吞噬

- 均正常穿护甲
- **剑修**攻击方 `wound_kind` 偏 Cut/Pierce → 对板甲低效、对布甲高效（矩阵自然平衡）
- **雷法** `wound_kind` 偏 Concussion/Burn → 所有护甲都低效（对齐 §五 "雷法击穿护体真气"）
- **毒蛊师** 持续性侵染，单次 severity 低 → 护甲减免意义弱，改为被 `ContaminationTick` 消耗 qi 抗
- **吞噬魔功** 攻击 kind TBD（可能混类）

---

## §5 HUD / Inspect 显示（骨架）

- Inventory 装备槽显示 `kind_mitigation` 矩阵（tooltip hover，对齐现有装备 tooltip）
- 左下角剪影 HUD：破损护甲在对应部位画裂纹图标（与 16 部位伤口圆点同层渲染）
- 战斗中：收到 `ArmorDurabilityChanged { broken: true }` → 1s toast "胸甲破损"（对齐现有 `ContaminationWarningStore` 风格）

---

## §6 开放问题（骨架阶段不答，进 active plan 前收敛）

- **Q1** `body_coverage` 粒度：CHEST 甲只覆盖 `BodyPart::Chest` 还是 Chest+Abdomen？7 粗段模型下要明确映射表
- **Q2** 护甲档次数值：骨架给 4 档示意，active 阶段需要 8-12 档覆盖"流民布衣 → 上古法宝"全谱
- **Q3** `WoundKind × 档次` 矩阵具体数值：骨架占位，active 阶段需要 playtest
- **Q4** `armor_cost_factor`：恒定 vs 随 `wound.severity` 非线性？非线性真实但调参难
- **Q5** 修复渠道归属 plan：锻造 / 灵草 / NPC 维修 —— 接入点怎么切
- **Q6** 多人服 PVP 平衡：高阶穿板甲是否让低阶完全无法伤到？需保留"未戴护甲部位是软肋"的 gameplay tension（对齐 `§十六` 分层悖论"低阶翻盘"逻辑）
- **Q7** 法宝级护甲数值上限：1.0（完全免疫）vs 保留 cap（e.g., 0.85）？`worldview.md §十六.三` "脆化" 映射到 `durability_max` 极低（3-5 次用即碎）
- **Q8** 护甲是否影响 `wound.kind` 转换？例如板甲让 Cut → Blunt（钝力化）——故事感强但实装复杂
- **Q9** 体修 `defense_power` 基础加成公式与叠加规则
- **Q10** 护甲吸收的伤害是否仍引起污染（`JIEMAI_CONTAM_MULTIPLIER` 语义扩展）？

---

## §7 实施规模预估（骨架，active plan 开工时修正）

| 模块 | 新增行数 |
|------|------|
| Rust `server/src/combat/armor.rs`（新） | ~250 |
| `combat/resolve.rs` apply_armor_mitigation 插入 | ~80 |
| `combat/components.rs` `DerivedAttrs` 扩展 + 同步 system | ~80 |
| `server/src/inventory/armor_profile.rs`（新） | ~150 |
| Client `InventoryEquipRules.java` tooltip + 破损图标 | ~140 |
| Schema 新增（TypeBox + Rust + IPC） | ~120 |
| Rust tests（unit + integration） | ~200 |
| **合计** | **~1020** |

和 `plan-tsy-worldgen-v1`（~1360 行）同量级，一次 worktree 吃得完。

---

## §8 升级条件（骨架 → active）

本 plan 从 `docs/plans-skeleton/` 移到 `docs/` 的触发：

1. `worldview.md §四.零`（战力分层）merged —— 需要确认装备在"真元 / 经脉 / 体表"三层中的位置
2. `worldview.md §五 体修` 对 `defense_power` 基础加成的定性描述 —— Q9 的前置
3. Q1 / Q2 / Q3 / Q6 收敛（覆盖粒度、数值分档、PVP 平衡）
4. `worldview.md §七` 或 §八 有装备章节（护甲作为物品的档次划分）

---

**下一步**：等 `§四.零` merged 后，回答 Q1/Q2/Q3/Q6，骨架升级为 active plan（移出 `plans-skeleton/`），`/consume-plan armor` 启动。
