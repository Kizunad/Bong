# Armor Mitigation · plan-armor-v1

> 装备护甲对战斗伤害的减免系统：`DerivedAttrs.defense_power` 骨架已在 `combat/components.rs` 存在但闭环未通——装备无 armor 数值字段、没人写入 `defense_power`、`Wound` 结算不读防御。本 plan 把装备数值化、同步到 `DerivedAttrs`、插入 `resolve_attack_intents` 的 wound 写入前层、补 `WoundKind × BodyPart` 二维防护矩阵。
> 交叉引用：`worldview.md §四`（战斗系统）· `worldview.md §四 战力分层`（2026-04-24 merged，三层多血条模型：护甲作用于"体表"轴的伤口降档，不直接减真元）· `worldview.md §五`（战斗流派 — 体修"不依赖外物"态度）· `plan-cultivation-v1 §2`（修炼侧不涉防护，接口隔离）

---

## §-1 现有代码基线（2026-04-24 audit 完成）

### Server 端已有能力

| 能力 | 位置 | 备注 |
|------|------|------|
| `DerivedAttrs { attack_power, defense_power, move_speed_multiplier }` | `combat/components.rs:223-229` | **默认全 1.0**，目前无装备写入 |
| `BodyPart` 7 段 enum | `combat/components.rs:33-42` | Head/Chest/Abdomen/ArmL/ArmR/LegL/LegR |
| `WoundKind` 5 种 enum | `combat/components.rs:44-50` | Cut / Blunt / Pierce / Burn / Concussion |
| `Wound { location, severity, kind }` | `combat/components.rs:56-66` | severity 是 f32 |
| `Wounds { entries, health_current, health_max }` | `combat/components.rs:68+` | Vec\<Wound\> |
| 部位状态阈值常量 | `combat/components.rs:27,29` | LEG_SLOWED=0.3 / HEAD_STUN=0.5 |
| `JIEMAI_DEFENSE_WINDOW_MS=200` | `combat/components.rs:21` | 截脉主动防御窗口 |
| `JIEMAI_CONTAM_MULTIPLIER=0.2` | `combat/components.rs:23` | 截脉污染降低系数 |
| `UnlockedStyles { jiemai, tishi, jueling }` | `combat/components.rs:275-283` | 三档主动防御解锁 flag（截脉/替尸/绝灵涡流） |
| `TICKS_PER_SECOND = 20` | `combat/components.rs:11` | 20TPS |
| 截脉实际实装：`CombatState.incoming_window` + `apply_defense_intents` + jiemai_success 分支 | `combat/resolve.rs:77,404-434` | **截脉是本项目唯一已实装的主动防御流派**；替尸 / 绝灵涡流仅有 UnlockedStyles flag，主动逻辑未落地 |
| 主攻击结算入口 `resolve_attack_intents` | `combat/resolve.rs:97` | severity 在 `:342,429,443` 多处生成；armor_mitigation 层需插入此处 |
| `body_part_multipliers` / `wound_kind_profile` | `combat/resolve.rs:534,544` | 现有部位倍率 / kind 特性档；本 plan 的 `defense_profile` 叠加在其上 |
| `PlayerInventory.equipped: HashMap<String, ItemInstance>` | `inventory/mod.rs:196` | 装备槽实际载体；Changed\<PlayerInventory\> 即装备变动 |

### Client 端已有能力

| 能力 | 位置 | 备注 |
|------|------|------|
| `EquipSlotType.HEAD/CHEST/LEGS/FEET` 护甲槽 | `client/.../inventory/model/EquipSlotType.java` | 槽位枚举 |
| `canEquip()` 类型/占用校验 | `client/.../inventory/InventoryEquipRules.java:49-74` | 只判资格，**不管数值** |
| 16 细段 `BodyPart` | `client/.../inventory/model/BodyPart.java` | Server 7 粗段通过 `WoundLayerBinding`（`client/.../combat/inspect/WoundLayerBinding.java`）展开到 16 细段 |
| `WoundLevel` 6 档 | `client/.../inventory/model/WoundLevel.java` | INTACT→BRUISE→ABRASION→LACERATION→FRACTURE→SEVERED；**护甲降档作用于此轴** |

### 关键空缺（本 plan 要补的）

1. **装备 schema 无 armor 数值字段** — 护甲只有外观与种类，无 `ArmorProfile`
2. **`DerivedAttrs.defense_power` 无人写入** — 默认 1.0，缺 `装备 → DerivedAttrs` 同步 system
3. **`resolve_attack_intents` 不读 defense** — severity 直接入 `Wounds.entries`，没有减免层
4. **`WoundKind × BodyPart` 分型矩阵未建** — "板甲挡 Cut/Pierce、但挡 Burn 差" 未编码
5. **护甲耐久/破损未建** — 装备无 durability 字段

### Audit 修正（骨架 → active）

原骨架基线表中几处行号与实际偏 ±1（已按实际回填）；**`DefenseStance` struct 实际不存在**（仅 `client_request_handler.rs:988,1054` 测试代码残留引用，无定义），原骨架 §4.1 的"DefenseStance 作为独立 Component"假设已推翻 —— 截脉依靠 `CombatState.incoming_window` + jiemai_success 分支实现，替尸 / 绝灵涡流尚未落地（仅解锁 flag 存在）。`§1.3` 同步 system 的 `Changed<PlayerEquipment>` 改为 `Changed<PlayerInventory>`（无 PlayerEquipment Component）。

---

## §0 设计轴心（active 阶段已锁）

1. **护甲作用于"降档 `WoundLevel`"，不直接减 HP 数值** — 对齐 `worldview.md §四 战力分层` 的"伤口是定性状态"设计（FRACTURE → LACERATION 比 `-20 HP` 有游戏意义）
2. **`WoundKind × BodyPart` 二维矩阵** — 不做"一个 defense 数值减所有伤"的简化；板甲挡 Cut/Pierce，皮甲缓 Blunt，无甲面对 Burn/Concussion 都弱
3. **装备作用点是 `DerivedAttrs`**（不新建组件链路），新增 `defense_profile` 字段承载二维矩阵
4. **体修流派绕过装备**（对齐 `worldview.md §五 体修`"不依赖外物"）— 体修 buff 作用于 `defense_power` 基础乘数，相当于"内力替代护甲"
5. **主动防御与被动护甲顺序固定**：`attacker output → 截脉窗口判定 → armor_mitigation → Wound 写入`（§4.1 展开；替尸 / 绝灵涡流未实装，不在本 plan 链路中）
6. **骨架不引入"法宝级"分支** — 上古护甲（`§十六` 遗物）作为 `ArmorProfile` 数值极端点，走相同接口；cap 0.85（不完全免疫，Q7 已决）
7. **护甲耐久独立维度**（不与真元/经脉混）— 破损护甲防护降级而非失效（`broken_multiplier=0.3`）；修复渠道归锻造 / 灵草 plan

---

## §1 数据模型扩展

### 1.1 装备 `ArmorProfile`（新）

```rust
// server/src/combat/armor.rs（新文件）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArmorProfile {
    pub slot: EquipSlot,                           // HEAD / CHEST / LEGS / FEET
    pub body_coverage: Vec<BodyPart>,              // 一件甲可覆盖多部位（Q1 MVP 见下）
    pub kind_mitigation: HashMap<WoundKind, f32>,  // 0.0=不减, 0.85=cap（Q7）
    pub durability_cur: u32,
    pub durability_max: u32,
    pub broken_multiplier: f32,                    // durability=0 时的衰减系数，默认 0.3
}
```

`ItemInstance`（`inventory/mod.rs:134-150`）扩一个 `Option<ArmorProfile>` 字段，或独立表 `armor_profiles: HashMap<instance_id, ArmorProfile>` —— 后者避免 ItemInstance 肥胖，active 起稿时二选一。

**`body_coverage` MVP 映射表（Q1 已决）**：

| EquipSlot | 覆盖 BodyPart (server 7 粗段) |
|---|---|
| HEAD | [Head] |
| CHEST | [Chest, Abdomen] |
| LEGS | [LegL, LegR] |
| FEET | [LegL, LegR]（与 LEGS 重叠，取 max） |
| 手套（未来扩 WRIST 槽）| [ArmL, ArmR] |

16 细段 → 7 粗段的展开由 client 侧 `WoundLayerBinding` 已处理，本 plan 不动。

### 1.2 `DerivedAttrs` 扩展

```rust
// combat/components.rs:223 → 扩
pub struct DerivedAttrs {
    pub attack_power: f32,
    pub defense_power: f32,                                      // 既有，通用乘数（体修 buff 作用点）
    pub move_speed_multiplier: f32,
    pub defense_profile: HashMap<(BodyPart, WoundKind), f32>,    // 新增：二维矩阵查询表
}
```

`Default` 同步更新：`defense_profile` 默认空 map（查询 miss 时 `unwrap_or(0.0)` = 不减免）。

### 1.3 装备 → `DerivedAttrs` 同步 system（新）

每当 `Changed<PlayerInventory>`（装备变动），遍历 `player.equipped` 各槽位装备的 `ArmorProfile` → 聚合 `body_coverage × kind_mitigation` → 写入 `defense_profile`。

**多件护甲覆盖同 `(BodyPart, WoundKind)` 时取最大**（不叠加 —— 避免三件甲叠成无敌）。

```rust
pub fn sync_armor_to_derived_attrs(
    mut query: Query<(&PlayerInventory, &mut DerivedAttrs), Changed<PlayerInventory>>,
    armor_profiles: Res<ArmorProfileRegistry>,  // instance_id → ArmorProfile
) {
    for (inv, mut derived) in &mut query {
        let mut profile = HashMap::new();
        for item in inv.equipped.values() {
            let Some(ap) = armor_profiles.get(item.instance_id) else { continue };
            let effective_mitigation = if ap.durability_cur == 0 {
                ap.broken_multiplier
            } else { 1.0 };
            for body in &ap.body_coverage {
                for (kind, mitigation) in &ap.kind_mitigation {
                    let final_m = (mitigation * effective_mitigation).min(0.85);  // Q7 cap
                    profile
                        .entry((*body, *kind))
                        .and_modify(|existing| *existing = existing.max(final_m))
                        .or_insert(final_m);
                }
            }
        }
        derived.defense_profile = profile;
    }
}
```

### 1.4 Wound 结算插入点（`combat/resolve.rs:97`）

现有 `resolve_attack_intents` 内流程（精简骨架理解）：
1. 计算 attacker output → incoming severity
2. 查 defender `CombatState.incoming_window` → 是否在 jiemai 窗口内 命中 (`:404-434`)
   - 命中：severity = 0，contamination ×= `JIEMAI_CONTAM_MULTIPLIER`（0.2）
   - 未命中：severity 按常规
3. `severity` 写入 `Wounds.entries` (`:342,429,443`)

**新插入点**：在步骤 2 之后、步骤 3 之前：

```rust
// 新增在 resolve.rs:~435（jiemai_success 分支结束后，Wounds.entries.push 之前）
fn apply_armor_mitigation(
    wound: &mut Wound,
    derived: &DerivedAttrs,
    contam: &mut f64,
) -> bool {  // 返回是否命中护甲（供耐久消耗判定）
    let Some(&m) = derived.defense_profile.get(&(wound.location, wound.kind)) else {
        return false;
    };
    if m <= 0.0 { return false; }
    wound.severity *= 1.0 - m as f32;
    // Q10 已决：contamination 同步按 (1-m) 线性衰减
    *contam *= 1.0 - m as f64;
    true
}
```

耐久消耗在调用点 emit `ArmorDurabilityChanged` event（§3 展开）。

---

## §2 护甲档次 × `WoundKind` 二维矩阵（MVP 骨架值）

4 档 × 5 kind 作为 MVP 起点，active 阶段 playtest 后扩。数值是 `kind_mitigation`（0-1，cap 0.85）。

| 档次 / 类型 | Cut | Blunt | Pierce | Burn | Concussion |
|-------------|-----|-------|--------|------|------------|
| 布甲 | 0.10 | 0.20 | 0.05 | 0.00 | 0.10 |
| 皮甲 | 0.25 | 0.30 | 0.20 | 0.10 | 0.15 |
| 板甲 | 0.50 | 0.40 | 0.55 | 0.15 | 0.20 |
| 灵纹甲（法宝级） | 0.35 | 0.35 | 0.35 | 0.40 | 0.35 |
| 上古遗物（cap 示例） | 0.70 | 0.60 | 0.75 | 0.50 | 0.55 |

- 板甲偏物理、灵纹甲均衡但上限低
- 上古遗物单件接近 cap 0.85（未达），但 `durability_max` 极低（3-5 次即碎，对齐 worldview §十六.三 "脆化"）
- 体修"自带护甲"不用矩阵，走 `defense_power` 基础加成（§4.2）

**具体数值平衡是 playtest 迭代（Q3）**，骨架数值保证能跑就行。

---

## §3 护甲耐久与破损

- 每次受击消耗耐久：`durability_cur -= max(1, round(wound.severity × ARMOR_COST_FACTOR × 10))`，`ARMOR_COST_FACTOR = 0.2`（Q4 已决：恒定值）
- `durability_cur = 0` → 破损状态（不完全失效，`kind_mitigation *= broken_multiplier`，默认 0.3）
- 破损不影响 equip 状态（玩家仍穿着，只是减免力打折）
- 修复渠道（Q5）：锻造重淬（归 `plan-forge-v1`）/ 灵草外敷（归 `plan-alchemy-v1`）/ NPC 维修（骨架阶段不定，active 时按已存在的 plan 决定）
- 新 event：`ArmorDurabilityChanged { entity, slot, cur, max, broken }` 推给 agent（天道叙事用）

---

## §4 与现有战斗流派 / 修炼的耦合

### 4.1 截脉（已实装）与装备护甲的顺序

```
attacker.output (severity 初值)
  ↓
CombatState.incoming_window 判定（resolve.rs:404-434）
  ├─ 截脉命中：severity = 0, contam ×= JIEMAI_CONTAM_MULTIPLIER (0.2)
  │              装备减免**不再叠加**（完全吸收）
  │              装备耐久**不消耗**（攻击没打在甲上）
  └─ 未命中：继续下一层
  ↓
apply_armor_mitigation（本 plan §1.4 新增）
  severity ×= (1 - defense_profile[(body, kind)])
  contam ×= (1 - mitigation)  // Q10
  命中护甲则 emit ArmorDurabilityChanged
  ↓
Wounds.entries.push（resolve.rs:342,429,443）
```

### 4.1.1 替尸 / 绝灵涡流（未实装，不影响本 plan）

现状：`UnlockedStyles.tishi` 和 `.jueling` 只是解锁 flag，主动流派逻辑未落地。本 plan 不负责实装这两种流派，落地后它们的接入点**应该在本 plan 的 `apply_armor_mitigation` 之前**（与截脉同层级，处理完才走装备）：

```
(未来 plan)
截脉 ─→ 替尸 ─→ 绝灵涡流 ─→ armor_mitigation ─→ Wound
```

active 阶段实装 armor 时预留这个插入点的文档注释即可。

### 4.2 体修流派（`worldview.md §五`）

- 体修"不依赖外物"——走 buff 路径：`defense_power` 基础 ×= 1.3（Q9 敲定 MVP 值，playtest 后调）
- 体修**仍可穿护甲**，但体修 buff 和护甲 `kind_mitigation` **独立相乘**（不叠加）：
  ```
  wound.severity *= (1 - kind_mitigation) × (1 / defense_power)
  ```
- 体修的基础加成暂按**所有部位均匀**应用（Q9 保留问题：是否按 BodyPart 区分"硬化四肢不护头"，留 follow-up）

### 4.3 剑修 / 毒蛊师 / 雷法 / 吞噬

- 均正常穿护甲
- **剑修**攻击方 `wound_kind` 偏 Cut/Pierce → 对板甲低效、对布甲高效（矩阵自然平衡）
- **雷法** `wound_kind` 偏 Concussion/Burn → 所有护甲都低效（对齐 §五 "雷法击穿护体真气"）
- **毒蛊师** 持续性侵染，单次 severity 低 → 护甲减免意义弱，改为被 `ContaminationTick` 消耗 qi 抗
- **吞噬魔功** 攻击 kind TBD（可能混类），留 design follow-up

---

## §5 HUD / Inspect 显示

- Inventory 装备槽 tooltip 显示 `kind_mitigation` 矩阵（hover 时），对齐现有装备 tooltip 结构
- 左下角剪影 HUD：破损护甲在对应部位画裂纹图标（与 16 部位伤口圆点同层渲染，用 Z 顺序区分）
- 战斗中：收到 `ArmorDurabilityChanged { broken: true }` → 1s toast "胸甲破损"（对齐现有 `ContaminationWarningStore` 风格）
- 实装路径：`client/src/main/java/com/bong/client/hud/` 新增 `ArmorBreakIndicator` 或复用现有伤口渲染通道

---

## §6 开放问题（audit 后 Q1/Q4/Q6/Q7/Q8/Q9/Q10 敲定 MVP 值，Q2/Q3/Q5 保留 playtest/follow-up）

### 已决（MVP 默认值）

- **Q1** ✅ `body_coverage` 粒度：CHEST 覆盖 [Chest, Abdomen]，HEAD [Head]，LEGS/FEET [LegL, LegR]（见 §1.1 表）
- **Q4** ✅ `ARMOR_COST_FACTOR = 0.2` 恒定值。非线性留 follow-up（如果 playtest 发现"重甲耐久掉得比布甲还快"这种奇葩现象）
- **Q6** ✅ PVP 平衡：MVP 不做特殊限制，护甲 cap 0.85；低阶"命中未装甲部位（头 / 四肢手套缺位）"保留反杀空间（对齐 worldview §十六 分层悖论）
- **Q7** ✅ 法宝级护甲 cap 0.85（非 1.0）；上古遗物 `durability_max = 3-5`（脆化对齐 §十六.三）
- **Q8** ❌ 护甲**不改变 wound.kind**（例如 Cut → Blunt 钝力化）；故事感强但实装复杂，留 follow-up
- **Q9** ✅ 体修 `defense_power × 1.3` 基础加成，所有部位均匀应用（"按部位分区"留 follow-up）
- **Q10** ✅ 护甲吸收伤害同步减少 contamination：`contam ×= (1 - mitigation)` 线性

### 保留

- **Q2** 护甲档次数量：MVP 4 档（布/皮/板/灵纹）+ 上古 1 档 = 5 档够跑；active 阶段扩到 8-12 档覆盖"流民布衣 → 上古法宝"全谱，由 plan-forge / plan-weapon 的材料分级驱动
- **Q3** `WoundKind × 档次` 矩阵具体数值：骨架占位，playtest 校准
- **Q5** 修复渠道归属 plan：骨架指向 forge/alchemy，具体接入点（是否新增 Recipe / Tool / NPC dialogue）由相应 plan 决定

---

## §7 实施规模预估

| 模块 | 新增行数 |
|------|------|
| `server/src/combat/armor.rs`（新） | ~250 |
| `combat/resolve.rs` apply_armor_mitigation 插入 + 调用点 | ~100 |
| `combat/components.rs` `DerivedAttrs.defense_profile` 字段 + Default 更新 | ~20 |
| `server/src/inventory/mod.rs` ItemInstance 扩 `ArmorProfile` 关联 + sync_armor_to_derived_attrs system | ~150 |
| IPC schema 新增（TypeBox + Rust），`ArmorDurabilityChanged` event | ~120 |
| Client `InventoryEquipRules.java` / tooltip / 破损图标渲染 | ~140 |
| `armor_profiles.json`（blueprint data，~5 档 × 每档 1-2 件示例）| ~80 |
| Rust tests（unit + integration） | ~200 |
| Java tests | ~80 |
| **合计** | **~1140** |

触点约 15 文件，一次 worktree 吃得完。

---

## §8 Active 阶段执行检查表

骨架 → active 升级（2026-04-24 完成），以下项已就绪：

- [x] worldview §四 战力分层 merged（commit 1701aff0）—— 确认装备在"真元 / 经脉 / 体表"三层中作用于体表伤口档次
- [x] `combat/components.rs` / `combat/resolve.rs` 基线 audit 完成（§-1 全部行号实锤）
- [x] `DefenseStance` 不存在的事实确认，§4.1 调整为"截脉 → armor → Wound"
- [x] `PlayerInventory.equipped` 作为装备载体确认
- [x] Q1/Q4/Q6/Q7/Q8/Q9/Q10 敲定 MVP 默认值
- [x] `ARMOR_COST_FACTOR` / `broken_multiplier` / cap 等常量有初值
- [ ] `armor_profiles` 的 data file 位置约定：blueprint JSON vs Rust 硬编码表？active 起稿第一步决策

### active 阶段建议开工顺序

1. 先加 `ArmorProfile` struct + `ArmorProfileRegistry` resource（空壳），跑通编译
2. `DerivedAttrs.defense_profile` 字段扩展 + Default 初始化
3. `sync_armor_to_derived_attrs` system 注册到 `Update`
4. `apply_armor_mitigation` 插入 `resolve_attack_intents`，先不消耗耐久
5. 写 1-2 件测试装备 blueprint（布甲 / 板甲），mock `ArmorProfileRegistry` 填充
6. playtest：穿上板甲挨打 → HUD 显示 severity 降级；穿布甲挨雷法攻击 → 仍满伤
7. 补耐久消耗 + `ArmorDurabilityChanged` event
8. 补 broken 状态 + `broken_multiplier` 降级
9. Client tooltip + 破损图标渲染
10. IPC schema + Rust / Java / JSON fixture 测试
11. `cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test` + `./gradlew test build` 全绿
12. 实机 playtest：4 档护甲 × 5 kind 攻击的减伤曲线合理性

---

**下一步**：`/consume-plan armor` 启动 active 阶段。第一步先决定 `armor_profiles` 数据源（推荐 blueprint JSON 随 worldgen 一起走）。
