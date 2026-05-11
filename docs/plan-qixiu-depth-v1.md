# Bong · plan-qixiu-depth-v1 · 器修深化——法器即外挂经脉

法器不是工具，是修士的**外化经脉系统**。锻造创建初始铭纹（法器版经脉），长期使用冲刷加深铭纹、积累真元染色、产生使用者共鸣。法器会成长、会特化、会过载龟裂——物理规律与修士经脉完全同构。本 plan 把器修从"锻造+扔暗器"变成**一条完整的修炼路线**。

**物理推演链**（worldview 三公理 → 法器活化）：
1. 真元极易挥发（§二）+ 载体能锁真元（§五）→ 载体价值 = 微观结构与真元的锁合度
2. 锻造 = 改变微观结构提升锁合度 → 开光 = 首次激活真元回路（铭纹）
3. 真元长期被某种方式调用会沉淀出性质（§六.2）→ 法器内真元反复流过铭纹 → 铭纹被冲刷加深 → 法器"成长"
4. 修士真元有染色 → 反复流过法器 → 法器积累染色 → 法器有"性格"
5. 法器染色 ↔ 使用者染色匹配 → 共鸣 → 效率加成

**世界观锚点**：
- `worldview.md §四` 距离衰减——载体封存比 = 材质微观结构的物理体现
- `worldview.md §五` 器修/暗器流——真元附着在物理载体上，材质分级决定保留率
- `worldview.md §五` 凡器边界——凡铁不锁真元，日用工具，非法器
- `worldview.md §六.2` 真元染色——长期修习沉淀出性质，这是物理变化不是 tag
- `worldview.md §六.2` 凝实色——器修原生匹配色，真元有形质感、易附着物体、衰减 -0.03/格
- `worldview.md §四` 过载撕裂——经脉有流量上限，法器铭纹同理
- `worldview.md §十五` 万物皆有成本——换法器 = 失去共鸣 = 战力回退

**前置依赖**：
- `plan-forge-v1` ✅ → 锻造四步状态机（坯料→淬炼→铭文→开光）、BilletProfile、品阶四级
- `plan-weapon-v1` ✅ → Weapon component、7 类武器、品质乘数、耐久度
- `plan-anqi-v1` ✅ → CarrierImprint、CarrierKind 6 档、充能→投掷→衰减闭环
- `plan-anqi-v2` ✅ → 五招完整包、磨损税、容器系统
- `plan-qi-physics-v1` ✅ → 距离衰减公式、逸散算子、环境场、守恒账本
- `plan-shelflife-v1` ✅ → 三路径衰减（Decay/Spoil/Age）、容器封存倍率
- `plan-style-vector-integration-v1` ✅ → PracticeLog、QiColor、evolve_qi_color
- `plan-craft-v1` ✅ → 手搓配方框架
- `plan-cultivation-v1` ✅ → 经脉系统（MeridianSystem）、InsightModifiers

**反向被依赖**：
- `plan-anqi-v2` → 载体铭纹深度影响暗器五招效率（凝魂注射密度 ∝ 铭纹深度）
- `plan-combat-gamefeel-v1` ⬜ skeleton → 法器共鸣视觉反馈（高共鸣 = 武器发光）
- `plan-insight-alignment-v1` ⬜ active → CONVERGE(凝实) 顿悟可加速法器铭纹成长

---

## 接入面 Checklist

- **进料**：`forge::ForgeSession`（锻造产出品阶+铭文结果）/ `combat::Weapon`（武器 component）/ `combat::CarrierImprint`（暗器载体烙印）/ `cultivation::QiColor + PracticeLog`（使用者染色）/ `qi_physics::excretion`（逸散公式复用）/ `qi_physics::channeling`（真元导流）/ `shelflife::DecayProfile`（半衰期框架）/ `inventory::ItemInstance`（物品实例持久化）
- **出料**：新增 `ArtifactMeridian` component（法器铭纹）/ 新增 `ArtifactColor` component（法器染色）/ 新增 `ArtifactResonance` 计算模块 / 修改 `Weapon::damage_multiplier` 接入共鸣系数 / 修改 `CarrierImprint` 接入铭纹深度 / 新增铭纹龟裂事件 / client `InspectScreen` 法器铭纹可视化
- **共享类型 / event**：复用 `ColorKind`（10 色 enum）/ 复用 `MeridianId`（铭纹命名复用经脉命名空间但不共享数据）/ 新增 `ArtifactMeridianCracked` event / 新增 `ArtifactResonanceChangedS2c` packet / 复用 `WeaponBroken` event（铭纹全裂时触发）
- **跨仓库契约**：
  - server：`server/src/forge/artifact_meridian.rs` + `server/src/forge/artifact_color.rs` + `server/src/forge/resonance.rs`（新文件）
  - agent：无改动（天道不干预法器成长——这是修士自己的事）
  - client：`InspectScreen` 法器页 + 铭纹可视化 + 共鸣度指示器
- **worldview 锚点**：§四 过载撕裂 + §五 器修暗器流 + §六.2 染色规则 + §十五 万物皆有成本
- **qi_physics 锚点**：法器铭纹的真元流过使用 `qi_physics::channeling::qi_channeling()`；铭纹龟裂的过载判定使用 `qi_physics::constants::QI_DECAY_PER_BLOCK` 同源常数体系；法器逸散走 `qi_physics::excretion::qi_excretion()`。**不自定物理公式**

---

## §0 设计轴心

- [ ] **法器 = 外挂经脉**：法器内部有铭纹（真元回路），与修士经脉的物理规律同构——打通要时间、加深要使用、损伤要修复、过载会撕裂
- [ ] **铭纹深度 = 使用时间的物理沉淀**：不是经验值，是真元冲刷铭纹的自然结果。铭纹加深速度 ∝ `使用频率 × 单次真元流量 × 材质锁合度`
- [ ] **法器染色 = 真元长期印染**：使用者的 QiColor 会慢慢"染"到法器上。法器有自己的 `ArtifactColor`，形成机制与修士 PracticeLog 同源
- [ ] **共鸣 = 染色匹配度**：法器染色与使用者染色越匹配，伤害/效率越高。捡别人的法器 = 共鸣从零开始
- [ ] **万物皆有代价**：
  - 法器成长需要**时间**（铭纹加深是慢过程）
  - 换法器需要**割舍**（失去旧法器的共鸣积累）
  - 法器过载需要**修复**（铭纹龟裂后降效或碎裂）
  - 高品阶法器需要**养护**（铭纹越深，维护消耗越高）
- [ ] **不搞"法器等级"**：法器没有 Lv.1→Lv.99。铭纹深度是连续值，品阶进化是铭纹深度到阈值后的自然结果（同修士经脉打通→境界突破）

---

## §1 铭纹物理模型

### 铭纹 = 法器内部的真元回路

锻造"铭文"步骤在法器内部刻画初始真元回路。开光时真元首次灌入激活回路。此后每次使用（攻击/施法/暗器投掷/御器操控），真元流过铭纹 → 冲刷效应 → 铭纹加深。

### 铭纹数据结构

```rust
#[derive(Debug, Clone, Component, Serialize, Deserialize)]
pub struct ArtifactMeridian {
    pub grooves: Vec<Groove>,
    pub total_depth: f64,
    pub depth_cap: f64,
    pub overload_cracks: u8,
    pub created_at_tick: u64,
    pub last_flow_tick: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Groove {
    pub id: GrooveId,
    pub depth: f64,
    pub depth_cap: f64,
    pub flow_capacity: f64,
    pub crack_severity: f64,
}
```

### 铭纹数量与材质天花板

| 材质档 | 初始铭纹数 | 深度上限 | 品阶天花板 | 对应武器/载体 |
|--------|----------|---------|----------|-------------|
| 凡铁/木石 | 1 | 10.0 | 凡器（不可进化） | 凡器工具 |
| 普通兽骨 | 2 | 30.0 | 法器 | BoneChip |
| 异变兽骨 | 3 | 60.0 | 法器 | YibianShougu |
| 灵木 | 4 | 100.0 | 灵器 | LingmuArrow |
| 凝实色骨 | 5 | 150.0 | 灵器 | DyedBone |
| 封灵匣骨 | 6 | 200.0 | 道器 | FenglingheBone |
| 上古残骨 | 8 | 300.0 | 道器 | ShangguBone |

凡铁只有 1 条浅铭纹——这就是凡器"不锁真元"的物理解释：回路太少太浅，真元流过几乎不留痕迹。

### 铭纹加深公式

```
depth_increment = qi_flow_amount × material_lock_coefficient × (1.0 - depth / depth_cap)
```

- `qi_flow_amount`：本次使用流过铭纹的真元量（攻击伤害 / 暗器封存量 / 施法消耗）
- `material_lock_coefficient`：材质锁合度（凡铁 0.01 → 上古残骨 0.15）
- `(1.0 - depth / depth_cap)`：递减因子——越深越难加深，趋近上限时接近停滞

**关键设计**：铭纹加深是**对数增长**，前期快后期极慢。从零到 50% 深度可能只要 5h 使用，50% 到 90% 要 20h，90% 到 100% 几乎不可能（类比修士打通最后几条奇经）。

### 铭纹与锻造的关系

锻造四步对铭纹的影响：
- **坯料**：材质决定 `groove_count` + `depth_cap` + `material_lock_coefficient`
- **淬炼**：淬炼品质（Perfect/Good/Flawed）影响初始 `flow_capacity`（Perfect = cap×0.8 / Good = cap×0.5 / Flawed = cap×0.3）
- **铭文**：铭文卷轴决定铭纹的**初始图案**——不同图案对不同流派有亲和（剑纹 → Sharp 亲和 / 锤纹 → Heavy 亲和）。跳过铭文 = 无图案 = 铭纹加深速度 ×0.5（随机冲刷低效）
- **开光**：首次真元灌注激活铭纹，灌注量决定初始 depth（开光真元 × 0.01 = 初始 depth）

---

## §2 法器染色

### 染色积累机制

法器有自己的 `ArtifactColor`，形成机制与修士 `PracticeLog` 同源：

```rust
#[derive(Debug, Clone, Component, Serialize, Deserialize)]
pub struct ArtifactColor {
    pub practice_log: PracticeLog,
    pub main: ColorKind,
    pub secondary: Option<ColorKind>,
    pub is_chaotic: bool,
}
```

每次使用法器时（攻击/充能/施法），使用者的 `QiColor.main` 写入法器的 `practice_log.add(user_color, flow_amount × 0.1)`。

- 法器染色的衰减速率 = 修士的 1/10（法器内部环境密封，染色褪得慢）
- 法器染色演化使用同一个 `evolve_qi_color()` 函数（复用，不重写）
- 法器染色无"混元"状态（`is_hunyuan` 始终 false——法器没有意识，无法"均衡修炼"）

### 染色的物理效果

法器的染色不是装饰——它影响真元流通效率：

| 法器主色 | 使用者主色 | 效率修正 |
|---------|----------|---------|
| 同色 | 同色 | +0% ~ +15%（按染色深度递增） |
| 无色（新法器） | 任意 | ±0%（中性） |
| 异色 | 异色 | -5% ~ -20%（按染色深度递增） |

**物理解释**：同色真元流过同色铭纹 = 共振，流通阻力小；异色真元流过异色铭纹 = 干涉，流通阻力大。

---

## §3 共鸣度

### 定义

```rust
pub fn compute_resonance(
    artifact_color: &ArtifactColor,
    user_color: &QiColor,
    groove_total_depth: f64,
    groove_depth_cap: f64,
) -> f64
```

共鸣度 = `染色匹配度 × 铭纹成熟度`

- **染色匹配度**：法器 main == 使用者 main → 1.0 / 法器 main == 使用者 secondary → 0.6 / 法器无色 → 0.5 / 法器 main ≠ 使用者任一色 → 0.2
- **铭纹成熟度**：`total_depth / depth_cap`（0.0 ~ 1.0）
- 共鸣度范围：0.0 ~ 1.0

### 共鸣度对战斗的影响

共鸣度作为乘数接入 `Weapon::damage_multiplier()`：

```
final_damage = base_damage × attack_mul × quality_mul × durability_mul × resonance_mul

resonance_mul = 0.7 + 0.6 × resonance
```

- 共鸣 0.0 → ×0.70（新武器/异色武器，七折惩罚）
- 共鸣 0.5 → ×1.00（及格线，等于现有基线）
- 共鸣 1.0 → ×1.30（满共鸣，三成加成）

**暗器共鸣**：载体的共鸣度影响封存效率而非伤害——`seal_ratio = base_seal × (0.8 + 0.4 × resonance)`。高共鸣 = 充能时真元损失更少。

### 共鸣度的代价

- **换武器 = 共鸣归零**：新法器的铭纹没被你的真元冲刷过，从 0.0 开始培养
- **捡别人的法器**：法器已有前主人的染色 → 异色惩罚 → 需要长期使用"洗染"才能提高共鸣
- **借用代价**：短期借用高品阶法器，共鸣低 → 发挥不出全部性能。"拿到了屠龙刀但舞不动"

---

## §4 法器品阶进化

### 进化条件

法器品阶不完全由锻造决定。锻造设定品阶**起点**，使用中铭纹加深到阈值后自然进化：

| 进化 | 铭纹深度阈值 | 共鸣度阈值 | 额外条件 | 效果 |
|------|-----------|----------|---------|------|
| 凡器→法器 | avg_depth ≥ 15.0 | ≥ 0.4 | 使用 ≥ 3h | `quality_tier` 0→1 |
| 法器→灵器 | avg_depth ≥ 50.0 | ≥ 0.6 | 使用 ≥ 15h + 铭纹无龟裂 | `quality_tier` 1→2 |
| 灵器→道器 | avg_depth ≥ 120.0 | ≥ 0.8 | 使用 ≥ 40h + 材质 ≥ 封灵匣骨 + 主人亲手锻造 | `quality_tier` 2→3 |

**材质天花板**：凡铁最多法器、灵木最多灵器、封灵匣骨/上古残骨可到道器。材质不够 → 铭纹深度到上限就停了。

**进化瞬间**：品阶跳变时触发全服 narration（灵器以上）+ VFX 光柱 + 法器外观微变（铭纹发光更亮）。天道会注意到——高品阶法器的灵气特征更明显，等于暴露位置。

**进化代价**：
- 品阶进化瞬间法器真元消耗 = 使用者 `qi_current × 30%`（铭纹重构需要大量真元）
- 进化后法器的**养护消耗**上升——高品阶铭纹需要定期真元灌注维持，否则缓慢退化（铭纹浅化 0.1/day）

---

## §5 铭纹过载与龟裂

### 过载判定

法器铭纹有流量上限（`groove.flow_capacity`）。单次使用流过铭纹的真元超过上限 → 过载 → 铭纹龟裂。

```
overload_check: qi_flow_this_use > sum(groove.flow_capacity × (1.0 - groove.crack_severity))
```

过载来源：
- 过载撕裂（§四）传导到法器——修士爆脉时法器也承受过载
- 全力一击——池子一次性灌出时法器铭纹承受全部流量
- 暗器凝魂注射——高密度注入超过载体铭纹容量
- 环境过载——在高灵压区使用低品阶法器（灵压差挤压铭纹）

### 龟裂分级

| 龟裂等级 | crack_severity | 效果 | 修复方式 |
|---------|---------------|------|---------|
| 微裂 | 0.01-0.15 | flow_capacity -15%，几乎无感 | 静养（不使用 2h 自然愈合） |
| 裂纹 | 0.16-0.40 | flow_capacity -40%，共鸣度 ×0.8 | 炼器台修复（消耗材料 + 真元） |
| 深裂 | 0.41-0.70 | flow_capacity -70%，品阶强制降一级 | 炼器台 + 稀有材料 + 3h 修复 |
| 断裂 | 0.71-1.0 | 该铭纹永久报废，不可修复 | 无（该铭纹永久 flow_capacity = 0） |

全部铭纹断裂 → `WeaponBroken` event → 法器碎裂。

**设计意图**：法器不是一次性消耗品（那是暗器载体的定位），但也不是永生的——它在使用中损耗、在过载中受伤。器修不止是"打造"，还包括"养护"。

### 龟裂与战斗的博弈

- 保守使用 = 法器寿命长但铭纹加深慢（每次流过的真元少 → 冲刷弱）
- 激进使用 = 铭纹加深快但龟裂风险高（大流量冲刷 → 深得快但容易裂）
- 全力一击 = 铭纹可能直接深裂/断裂（赌命式使用，法器可能是一次性的）

---

## §6 法器养护

### 养护机制

品阶 ≥ 法器的法器需要定期养护——持有者在安全区（灵龛内）花 `qi_current` 灌入法器铭纹：

```
maintenance_cost_per_day = quality_tier × 2.0 × groove_count
```

| 品阶 | 铭纹数（典型） | 日养护消耗 | 不养护后果 |
|------|-------------|----------|----------|
| 法器 | 3 | 6 qi/day | 铭纹每日浅化 0.05 |
| 灵器 | 5 | 20 qi/day | 铭纹每日浅化 0.10 |
| 道器 | 7 | 42 qi/day | 铭纹每日浅化 0.20 |

**物理解释**：铭纹是真元冲刷出的"河床"——不持续有水流过，河床会淤塞。品阶越高铭纹越深，需要的"流水量"越大。

**设计意图**：高品阶法器是"奢侈品"——你必须有足够的真元池来养它。引气期修士拿到灵器 = 养不起 → 铭纹退化 → 品阶跌回法器。这与修士境界维护成本（§三 "高境界维护成本极高"）同构。

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|----|------|----|
| P0 | `ArtifactMeridian` component + 铭纹数据结构 + 锻造→铭纹初始化桥接 | ⬜ |
| P1 | 铭纹加深 tick 系统 + 使用→冲刷逻辑 + `material_lock_coefficient` 常数表 | ⬜ |
| P2 | `ArtifactColor` component + 法器染色积累 + 共鸣度计算 + `Weapon::damage_multiplier` 接入 | ⬜ |
| P3 | 铭纹过载判定 + 龟裂分级 + 修复机制 + `ArtifactMeridianCracked` event | ⬜ |
| P4 | 法器品阶进化（凡→法→灵→道）+ 养护 tick 系统 + narration 联动 | ⬜ |
| P5 | Client 法器铭纹可视化（InspectScreen 法器页 + 共鸣度 HUD 指示 + 龟裂警告） | ⬜ |
| P6 | 饱和化测试（7 材质 × 4 品阶 × 10 染色 × 龟裂分级 × 共鸣度范围 + 进化阈值 pin） | ⬜ |

---

## P0 — ArtifactMeridian component + 锻造桥接 ⬜

### 交付物

1. **`ArtifactMeridian` component**（`server/src/forge/artifact_meridian.rs`，新文件）

   §1 中定义的完整数据结构。`ArtifactMeridian` 作为 Bevy Component 挂在 `Weapon` 所在 entity 上。

2. **`GrooveId` 类型**

   铭纹 ID 使用字符串命名（`"primary"` / `"secondary_1"` / `"tertiary_2"` 等），不复用 `MeridianId`（法器铭纹与修士经脉是类比关系，不是同一个东西）。

3. **材质常数表**（`server/src/forge/artifact_meridian.rs` 内 `const` block）

   ```rust
   pub fn material_spec(carrier_kind: CarrierKind) -> MaterialSpec {
       match carrier_kind {
           CarrierKind::BoneChip       => MaterialSpec { grooves: 2, depth_cap: 30.0,  lock_coeff: 0.03 },
           CarrierKind::YibianShougu   => MaterialSpec { grooves: 3, depth_cap: 60.0,  lock_coeff: 0.06 },
           CarrierKind::LingmuArrow    => MaterialSpec { grooves: 4, depth_cap: 100.0, lock_coeff: 0.08 },
           CarrierKind::DyedBone       => MaterialSpec { grooves: 5, depth_cap: 150.0, lock_coeff: 0.10 },
           CarrierKind::FenglingheBone => MaterialSpec { grooves: 6, depth_cap: 200.0, lock_coeff: 0.12 },
           CarrierKind::ShangguBone    => MaterialSpec { grooves: 8, depth_cap: 300.0, lock_coeff: 0.15 },
       }
   }
   ```

   凡铁 / 木石类武器（非 CarrierKind）：`grooves: 1, depth_cap: 10.0, lock_coeff: 0.01`。

4. **锻造→铭纹桥接**

   修改 `forge/inventory_bridge.rs`，在锻造成品写入 inventory 时同步创建 `ArtifactMeridian` component：
   - `groove_count` = `material_spec(billet_carrier).grooves`
   - `groove.depth` = `consecration_qi_amount × 0.01`（开光真元的 1% 作为初始深度）
   - `groove.flow_capacity` = `depth_cap × tempering_quality_factor`（Perfect=0.8 / Good=0.5 / Flawed=0.3）
   - 铭文卷轴类型 → `groove.id` 的命名后缀（剑纹/锤纹/阵纹等，影响后续染色亲和）

5. **持久化**

   `ArtifactMeridian` 走现有 `plan-persistence-v1` ✅ 的 component 序列化框架。铭纹数据必须跨 session 持久化。

### 验收抓手

- 测试：`forge::artifact_meridian::tests::material_spec_all_6_carriers`（6 种载体全有 spec）
- 测试：`forge::artifact_meridian::tests::forge_creates_artifact_meridian`（锻造产出自动创建铭纹 component）
- 测试：`forge::artifact_meridian::tests::initial_depth_from_consecration`（开光真元 100 → 初始深度 1.0）
- 测试：`forge::artifact_meridian::tests::flow_capacity_by_tempering_quality`（Perfect/Good/Flawed 三档）
- 测试：`forge::artifact_meridian::tests::mundane_weapon_single_shallow_groove`（凡器只有 1 条浅铭纹）

---

## P1 — 铭纹加深 tick 系统 ⬜

### 交付物

1. **`artifact_meridian_deepen_on_use` system**

   监听 `CombatEvent::AttackResolved` / `CarrierChargedEvent` / `CarrierImpactEvent`，从中提取 `qi_flow_amount`，调 §1 加深公式更新 `ArtifactMeridian.grooves[].depth`。

   ```
   depth_increment = qi_flow_amount × lock_coeff × (1.0 - depth / depth_cap)
   ```

   每条 groove 均匀分摊（flow 平均分配到所有未断裂 groove）。

2. **`artifact_meridian_maintenance_tick` system**

   每 in-game day tick 一次。品阶 ≥ 法器的法器检查：持有者 `qi_current` 是否足够养护。够 → 扣 qi + 铭纹不变；不够 → 铭纹浅化（§6 公式）。

3. **铭纹深度变化 event**

   `ArtifactMeridianDepthChanged { entity, groove_id, old_depth, new_depth }` → 用于 client 同步 + narration 触发。

### 验收抓手

- 测试：`forge::artifact_meridian::tests::deepen_on_attack`（攻击后铭纹加深）
- 测试：`forge::artifact_meridian::tests::deepen_diminishing_returns`（靠近上限时增量趋近 0）
- 测试：`forge::artifact_meridian::tests::mundane_deepen_extremely_slow`（凡铁 lock_coeff=0.01）
- 测试：`forge::artifact_meridian::tests::maintenance_drains_qi`（养护扣 qi）
- 测试：`forge::artifact_meridian::tests::no_maintenance_causes_shallowing`（不养护 → 浅化）
- 测试：`forge::artifact_meridian::tests::cracked_groove_excluded_from_flow`（龟裂铭纹不参与分摊）

---

## P2 — 法器染色 + 共鸣度 ⬜

### 交付物

1. **`ArtifactColor` component**（`server/src/forge/artifact_color.rs`，新文件）

   §2 中定义的数据结构。每次使用法器时 `practice_log.add(user_main_color, flow_amount × 0.1)`。衰减速率 = 修士的 1/10。

2. **`artifact_color_evolve_tick` system**

   复用 `evolve_qi_color()` 函数（不重写）。每 tick 演化法器染色。

3. **`compute_resonance` 函数**（`server/src/forge/resonance.rs`，新文件）

   §3 中定义的公式。输入 ArtifactColor + 使用者 QiColor + 铭纹深度 → 输出 0.0~1.0。

4. **接入 `Weapon::damage_multiplier`**

   修改 `combat/weapon.rs`，`damage_multiplier()` 新增 resonance 系数：

   ```rust
   pub fn damage_multiplier_with_resonance(&self, resonance: f64) -> f32 {
       let resonance_mul = 0.7 + 0.6 * resonance as f32;
       self.attack_multiplier() * self.quality_multiplier() * self.durability_factor() * resonance_mul
   }
   ```

   修改 `combat/resolve.rs` 中的伤害计算调用此新函数。

5. **接入 `CarrierImprint` 封存效率**

   暗器充能时 `seal_ratio = base_seal × (0.8 + 0.4 × resonance)`。高共鸣 = 充能损失更少。

### 验收抓手

- 测试：`forge::artifact_color::tests::color_accumulates_on_use`（使用后法器 practice_log 增长）
- 测试：`forge::artifact_color::tests::decay_rate_one_tenth`（法器染色衰减 = 修士 1/10）
- 测试：`forge::resonance::tests::same_color_max_resonance`（同色满深度 = 1.0）
- 测试：`forge::resonance::tests::no_color_half_resonance`（无色法器 = 0.5）
- 测试：`forge::resonance::tests::different_color_low_resonance`（异色 = 0.2 × 成熟度）
- 测试：`forge::resonance::tests::damage_with_zero_resonance_is_70_pct`（共鸣 0 = 七折）
- 测试：`forge::resonance::tests::damage_with_full_resonance_is_130_pct`（共鸣 1 = 1.3 倍）
- 测试：`forge::resonance::tests::carrier_seal_efficiency_scales_with_resonance`

---

## P3 — 铭纹过载与龟裂 ⬜

### 交付物

1. **`overload_check` 函数**

   §5 过载判定公式。每次使用后检查 `qi_flow > total_flow_capacity`。

2. **龟裂应用逻辑**

   过载时选择流量分摊最高的 groove 施加龟裂。龟裂量 = `(qi_flow - capacity) / capacity × 0.3`。

3. **`ArtifactMeridianCracked` event**

   触发时机：任何 groove 的 `crack_severity` 从 < 0.16 跃升到 ≥ 0.16（微裂→裂纹）。用于 client 警告 + narration。

4. **修复机制**

   在炼器台（`WeaponForgeStation`）发起修复：消耗材料 + 真元 + 时间 → `crack_severity` 下降。微裂可自愈（不使用 2h）。断裂不可修复。

5. **全铭纹断裂 → WeaponBroken**

   所有 groove 的 `crack_severity >= 0.71` → 触发现有 `WeaponBroken` event。

### 验收抓手

- 测试：`forge::artifact_meridian::tests::overload_causes_crack`
- 测试：`forge::artifact_meridian::tests::micro_crack_self_heals`（2h 不使用 → 微裂消失）
- 测试：`forge::artifact_meridian::tests::deep_crack_drops_quality_tier`
- 测试：`forge::artifact_meridian::tests::severed_groove_permanent`（断裂不可修）
- 测试：`forge::artifact_meridian::tests::all_grooves_severed_triggers_weapon_broken`
- 测试：`forge::artifact_meridian::tests::repair_at_station_reduces_crack`

---

## P4 — 法器品阶进化 + 养护 ⬜

### 交付物

1. **品阶进化检查 system**

   每 100 tick 检查一次：`avg_depth >= threshold && resonance >= threshold && usage_hours >= threshold && material_allows`。满足 → `quality_tier += 1` + 进化 event。

2. **进化代价**

   进化瞬间扣 `qi_current × 30%`。进化后养护消耗上升（§6 公式）。

3. **进化 narration**

   灵器以上进化 → 天道 narration "某处有法器通灵之兆……"。暴露位置——与突破广播同理。

4. **养护退化**

   品阶 ≥ 法器且连续 3 天未养护 → 品阶降一级。铭纹深度不变但 `quality_tier` 回退。重新养护后可再次进化（不用从零开始，铭纹还在）。

### 验收抓手

- 测试：`forge::artifact_meridian::tests::evolution_mundane_to_magic`（满足条件 → 凡→法）
- 测试：`forge::artifact_meridian::tests::evolution_blocked_by_material_cap`（凡铁不能超法器）
- 测试：`forge::artifact_meridian::tests::evolution_costs_30pct_qi`
- 测试：`forge::artifact_meridian::tests::no_maintenance_3days_drops_tier`
- 测试：`forge::artifact_meridian::tests::re_evolution_after_maintenance_resume`

---

## P5 — Client 法器铭纹可视化 ⬜

### 交付物

1. **InspectScreen 法器页**

   在装备 inspect 界面新增法器详情页：
   - 铭纹图（每条 groove 画一条弧线，深度 = 线宽，龟裂 = 红色裂纹 overlay）
   - 法器染色指示（小色块，与修士 QiColor 相同的渲染方式）
   - 共鸣度圆弧（0-100%，颜色从灰→绿→金）
   - 品阶标签 + 进化进度条（当前深度 / 下一品阶阈值）
   - 养护状态（"良好" / "需养护" / "退化中"）

2. **共鸣度 HUD 微指示**

   在 MiniBodyHudPlanner 旁边加一个 3×3 像素的小方块：
   - 颜色 = 法器主色（无色 = 灰色）
   - 亮度 ∝ 共鸣度（低共鸣 = 暗淡，高共鸣 = 明亮）
   - 不占用 HUD 空间，极简

3. **龟裂警告**

   铭纹龟裂 ≥ 裂纹（0.16+）→ 武器 icon 上叠加红色裂纹 overlay + toast "法器铭纹龟裂"。

### 验收抓手

- 手动：锻造一把灵木剑 → InspectScreen 显示 4 条铭纹 + 初始深度 → 使用 5 分钟 → 铭纹加深可见 → 共鸣度上升 → 过载一次 → 裂纹 overlay 出现
- 测试：`client::inspect::tests::artifact_page_renders_grooves`
- 测试：`client::inspect::tests::resonance_indicator_color`

---

## P6 — 饱和化测试 ⬜

### 交付物

1. **铭纹物理矩阵**
   - 7 材质 × 加深公式 = 7 条增长曲线 pin（初始/中期/接近上限三点断言）
   - 6 CarrierKind × `material_spec` 全覆盖
   - 凡铁 lock_coeff=0.01 单独验证（极慢）

2. **染色矩阵**
   - 10 ColorKind × 法器初始无色 → 使用 N 次后法器主色 = 使用者主色
   - 法器换主人（异色）→ 旧色慢慢褪 + 新色慢慢上
   - 法器染色不出现 is_hunyuan（pin 行为）

3. **共鸣度矩阵**
   - 同色满深度 = 1.0 / 无色半深度 = 0.25 / 异色满深度 = 0.2
   - damage_multiplier 范围 pin：0.70 ~ 1.30
   - carrier seal_ratio 范围 pin

4. **龟裂矩阵**
   - 4 级龟裂各自的 flow_capacity 系数 pin
   - 微裂自愈 2h pin
   - 断裂不可修 pin
   - 全断裂 → WeaponBroken pin

5. **进化矩阵**
   - 3 次进化（凡→法→灵→道）各自阈值 pin
   - 材质天花板 pin（凡铁不超法器等）
   - 养护退化 3 天 pin
   - 进化真元消耗 30% pin

6. **端到端**
   - 锻造灵木剑 → 使用 10 分钟 → 铭纹从初始加深 → 染色开始积累 → 共鸣度上升 → 伤害乘数提升 → 过载一次 → 龟裂 → 修复 → 继续使用 → 凡→法进化触发

### 验收抓手

- `cargo test forge::artifact_meridian` + `forge::artifact_color` + `forge::resonance` 全绿
- 总 case 数 ≥ 60

---

## Finish Evidence（待填）

- **落地清单**：`ArtifactMeridian` component / `Groove` 数据结构 / `MaterialSpec` 常数表 / 铭纹加深 tick / 铭纹养护 tick / `ArtifactColor` component / 法器 PracticeLog / `compute_resonance` / `Weapon::damage_multiplier_with_resonance` / 暗器封存共鸣接入 / 过载判定 / 龟裂四级 / 修复机制 / 品阶进化（3 档）/ 进化 narration / 养护退化 / InspectScreen 法器页 / 共鸣度 HUD 微指示 / 龟裂警告 / 60+ 饱和化测试
- **关键 commit**：P0-P6 各自 hash
- **遗留 / 后续**：
  - 御器术（远程真元操控法器——距离衰减制约操控精度，飘逸色天然优势）→ 独立 plan
  - 法器铭纹图案对流派的亲和差异（剑纹 → Sharp 亲和 +15%，锤纹 → Heavy 亲和 +15%）→ P1 扩展或独立 plan
  - 法宝系统（法器 Entity 化——飞剑自主飞行/灵幡持续施法）→ plan-treasure-v1
  - 暗器载体铭纹（CarrierImprint 接入 ArtifactMeridian）→ plan-anqi-v2 P 阶段协同
  - 法器交易市场（共鸣度+染色+铭纹深度 = 法器"履历"，影响定价）→ plan-economy-v2
