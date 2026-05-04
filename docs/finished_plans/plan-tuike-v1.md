# Bong · plan-tuike-v1

**替尸·蜕壳流**（防御）。以蛛丝、朽木等制"伪灵皮"穿在真甲下层——**只过滤真元污染，不防物理伤害**。受击 contam 累积达伪皮承受上限自动脱一层，连污染一起带走。**物资派**：靠 inventory loadout 决定，但**长期专精涌现凝实色**（与器修同源——伪皮亦载体；worldview §五"流派 ⇄ 染色" 2026-05-03 正典化）。

**Primary Axis**（worldview §五:466 已正典）：**伪皮档位（轻/中/重） + 材料色克 + 单层吸收上限**

## 阶段总览

| 阶段 | 状态 | 验收 |
|---|---|---|
| **P0** 单层蛛丝伪皮 + 自动 contam 累积脱壳 + 制作流程 + plan-armor 装备槽 + plan-zhenmai 重量耦合 | ⬜ | — |
| **P1** 朽木甲 3 层 + 暗器溢出 contam 自然实现"连壳带人"+ agent narration | ⬜ | — |
| P2 v1 收口（饱和 testing + 数值平衡） | ⬜ | — |

> **vN+1 (plan-tuike-v2)**：三层蛛丝伪皮 + 灵纹伪皮（异变兽皮高阶）+ 凝实色器修加速制作 + NPC 商人售卖 + 蛛丝产能调参

---

## 世界观 / library / 交叉引用

**worldview 锚点**：
- §五.防御.2 替尸/蜕壳流（line 437-440：拟态灰烬蛛丝/死域朽木 + 注真元模拟气息 + **优先承受污染** + 主动切断 + 把攻击连同污染带走 + 适合器修/仓鼠玩家）
- §五:466 流派 primary axis 表（伪皮档位 + 材料色克 + 单层吸收上限）
- §五:451 克制关系："**器修重狙→蜕壳**"（重狙 contam 量大撑爆伪皮 → 一次脱光 + 溢出 contam 进玩家）
- §五:386-389 "防御三流皆克制不了真正的体修爆脉"（爆脉 wound 直接打玩家，伪皮不防 wound）
- §四 异体排斥（line 342-349：侵染 + 排异反应 + 交换比亏损 — 伪皮过滤的物理依据）
- §五:471 "蜕壳流是物资派——它的 primary axis 不绑修士身体，只看带了什么材料、几层伪皮。代价在钱包，不在真元"（**正典化的物资派定位**）

**library 锚点**：
- `peoples-0005 异变图谱·残卷`（**拟态灰烬蛛**——蜕壳流必备载体，一具产蛛丝半套；2-3 骨币 / 具，咬伤粘液封经脉三息）
- `peoples-0006 战斗流派源流`（防御二·替尸/蜕壳流原文："**所打者，纯烧材料；几层壳打光后比纸还脆**"）

**交叉引用**：
- `plan-armor-v1` ✅（已落地）— **核心接入**：扩展 `ArmorKind::FalseSkin` + 伪皮在真甲下层（Q74: B），共存装备
- `plan-zhenmai-v1` 🟡（active P0 已落地，P1 在做）— 装备重量耦合：蛛丝伪皮 = 轻 ×1.0 / 朽木甲 = 重 ×0.6（zhenmai-v1 Q63 表）
- `plan-combat-no_ui-v1` ✅（已落地）— 复用 `Contamination.entries` 写入路径；伪皮**截胡** contam 写入（先扣伪皮，伪皮空再进玩家 Contamination）
- `plan-anqi-v1` 🟡（active P0）— 重狙 hit_qi 大量 → contam 量大 → 一次撑爆伪皮（"连壳带人"自然实现，**不需 bypass**）
- `plan-baomai-v1` ✅（已落地）— 爆脉 wound 直接打玩家（伪皮不防 wound），**不需特判**
- `plan-cultivation-v1` ✅（已落地）— 制作扣 `Cultivation.qi_current` + qi_color 染色读取（vN+1 凝实色加成 hook）
- `plan-inventory-v1` ⬜（未立 plan）— 多套伪皮库存（v1 简化为单套装备 + 背包堆叠）
- `plan-tsy-hostile-v1` ✅（已落地）— 拟态灰烬蛛丝来源（vN+1 调参产能）

## 接入面 checklist（防孤岛 — 严格按 docs/CLAUDE.md §二）

- **进料**：`Cultivation.qi_current` 扣制作成本（5 / 30 qi）→ `inventory` 取蛛丝 / 死域朽木 → `armor::ArmorSlot` 装备伪皮 in 真甲下层 → `combat::Lifecycle` 校验非死亡态
- **出料**：受击 `Contamination.entries.push` 之前 — **由 `tuike_filter_contam` 系统截胡**：contam 累积进 `FalseSkin.absorbed_contam` → 达 capacity 触发 `ShedEvent` → 移除一层 → 溢出 contam 才进 `Contamination`
- **共享类型 / event**：复用 `Contamination` / `Cultivation` / `Armor` / `ArmorKind`；新增 `FalseSkin` component / `ShedEvent` event / `ArmorKind::FalseSkin` variant
- **跨仓库契约**：
  - server: `combat::tuike::FalseSkin` component / `combat::tuike::tuike_filter_contam` system / `combat::tuike::shed_layer` 函数 / `armor::ArmorKind::FalseSkin` variant 扩展 / `crafting::false_skin_recipe` (新)
  - schema: `agent/packages/schema/src/tuike.ts` → `FalseSkinStateV1` / `ShedEventV1`
  - client: `bong:armor/equip_false_skin` (inbound, 新) / `bong:tuike/false_skin_state` (outbound, 新) HUD payload
- **特性接入面**（worldview §五"流派由组合涌现" 2026-05-03 正典 — tuike 长期专精涌现**凝实色**，与器修同源；worldview §五:471 物资派定位仍成立）：
  - **凝实色加成** vN+1 接入：长期专精涌现凝实色后，伪皮制作 qi cost -10% / contam_capacity +20%
  - **真元逆逸散效率特性** vN+1 接入（伪皮 absorbed_contam 不漏失）— v1 contam 累积永不漏失，不需要

**Hotbar 接入声明**（2026-05-03 user 正典化"所有技能走 hotbar"）：
- **`bong:armor/equip_false_skin`**（装备伪皮）= **装备操作**（armor inventory path）→ **不走 hotbar**，保留装备 packet
- tuike 整个流派**无 technique cast**（worldview §五:471 "物资派" 正典定位 — 不需技能；染色由专精涌现，非门禁）
- 详见 `plan-woliu-v1.md §8 跨 plan hotbar 同步修正备注`。

---

## §A 概览（设计导航）

> 蜕壳流 = **真元污染过滤器**——伪皮**不防 wound**，只防 contam。受击 contam 累积进伪皮，达伪皮 `contam_capacity` 自动脱一层 + 溢出 contam 进玩家。**物资派**：全靠 inventory loadout 决胜（worldview §五:471）；长期专精涌现**凝实色**（与器修同源；worldview §五"流派 ⇄ 染色"）。worldview "器修重狙→蜕壳"自然实现（高 contam 撑爆 + 溢出），不需 bypass / 特判。

### A.0 v1 实装范围（2026-05-03 拍板）

| 维度 | v1 实装 | 搁置 vN+1 |
|---|---|---|
| **核心模式** | **物资派**（worldview §五:471 正典）| 染色加速 / 特性加成 |
| 伪皮档位 | **2 档：单层蛛丝伪皮 + 朽木甲（3 层）**（Q78: B）| 三层蛛丝 / 灵纹伪皮 |
| 装备槽 | **plan-armor 共存**（伪皮内层 + 真甲外层）（Q74: B）| 多套切换 |
| 触发机制 | **自动 contam 累积**（达 capacity 自动脱壳）（Q75 reframe）| 手动 prep / 玩家选择时机 |
| **关键设计** | **伪皮不防 wound，只防 contam**（Q77 reframe，worldview §五.防御.2 锚定）| — |
| 赤裸期 | **无赤裸期**（脱完直接换装新的）（Q76: D）| 紧急再装 / 缩短机制 |
| 暗器穿透 | **不做特判**（contam 量大自然撑爆）（Q77 reframe）| — |
| 爆脉穿透 | **不做特判**（爆脉 wound 直接进玩家，伪皮不防）| — |
| 与 zhenmai 重量耦合 | **复用 zhenmai-v1 Q63**：蛛丝伪皮 = 轻 / 朽木甲 = 重 | — |
| 制作 | **玩家自制**（plan-armor crafting 框架扩展）（Q79: A）| NPC 商人售卖 |
| 染色 / 特性 | v1 不实装；vN+1 凝实色加成（长期专精涌现，worldview §五"流派 ⇄ 染色"）| 完整凝实色×tuike 加成 |

### A.1 物理模型（关键反转）

**普通流派的攻击事务**（plan-combat-no_ui §3.1）：
```
attack → hit_qi 计算 → wound 进 target.Wounds + contam 进 target.Contamination
```

**面对 tuike 装备伪皮的 target**：
```
attack → hit_qi 计算 →
  wound 进 target.Wounds（不变）
  contam 走 tuike_filter_contam 拦截：
    if target.has FalseSkin (伪皮还有层):
      FalseSkin.absorbed_contam += contam_amount
      if absorbed_contam >= layer.contam_capacity:
        # 一次撑爆
        overflow = absorbed_contam - layer.contam_capacity
        shed_layer(target)  # 移除一层 + emit ShedEvent
        # 递归处理溢出
        if overflow > 0 && 还有下一层:
          repeat with next layer
        else:
          target.Contamination.entries.push(overflow)
    else:
      target.Contamination.entries.push(contam_amount)  # 没伪皮，照常
```

**worldview 克制关系自然实现**：
- "器修重狙→蜕壳"：anqi 单针 hit_qi = 80 凝脉档（worldview §四 例子），命中后 contam 量按 §3.1.E profile 50/50 = 40 contam → 单层蛛丝 capacity 10 → 一次撑爆 + 溢出 30 contam 进玩家（"连壳带人秒杀"）
- "爆脉→蜕壳"：爆脉 wound 极重直接进玩家（伪皮不防 wound），contam 进伪皮（如有）但 wound 已经致命

### A.2 物资经济（worldview "仓鼠玩家友好"）

| 伪皮档 | 主料 | 辅料 | 真元 cost | 单层 contam_capacity | 总层数 | 重量 (zhenmai 耦合) |
|---|---|---|---|---|---|---|
| 单层蛛丝伪皮 | 蛛丝 1 卷（=2 具蛛 ≈ 4 骨币）| — | 5 | 10 | 1 | 轻 ×1.0 |
| 朽木甲 | 死域朽木 1 块 | 蛛丝 2 卷 | 30 | 30 / 层 | 3 | 重 ×0.6 |

**总 contam 防御**：
- 蛛丝伪皮：10 contam 单次 → 性价比战 1-2 拳后换装
- 朽木甲：90 contam 总（30×3）→ 撑久但 zhenmai 弹反窗口缩 40%

**经济压力**：
- 蛛丝伪皮 = 4 骨币（蛛丝） + 5 qi → 撑一次 10 contam → 高消耗品
- 朽木甲 = 8+ 骨币 + 30 qi → 撑 90 contam → 性价比高但 jiemai trade-off

### A.3 v1 实施阶梯

```
P0  单层蛛丝伪皮闭环（最 close-loop）
       FalseSkin component { absorbed_contam, contam_capacity, layers, kind }
       tuike_filter_contam 系统（截胡 Contamination 写入）
       自动累积 + 达 capacity 脱壳
       FalseSkinForge 配方（plan-armor 扩展）
       plan-zhenmai-v1 装备重量耦合（轻 ×1.0）
       ↓
P1  朽木甲 3 层 + 暗器溢出 + agent narration
       朽木甲制作（蛛丝 + 死域朽木）
       多层 shed 递归（一次大 contam 撑爆多层）
       agent narration: ShedEvent 触发
       plan-zhenmai-v1 重 ×0.6 耦合
       ↓ 饱和 testing
P2  v1 收口
       数值平衡（fight room：anqi vs tuike / dugu vs tuike）
       LifeRecord "X 在 Y 战中脱了 N 层壳"
```

### A.4 v1 已知偏离正典（vN+1 必须修复）

- [ ] **三层蛛丝伪皮 / 灵纹伪皮**（worldview §五.防御.2 + skeleton §2 高阶款）—— v1 仅做单层 + 朽木甲
- [ ] **染色加速制作**（worldview §六:542 凝实色 × 器修原生匹配）—— v1 不实装
- [ ] **NPC 商人售卖**（plan-baomai 联动）—— v1 仅自制
- [ ] **蛛丝产能调参**（plan-tsy-hostile-v1 拟态灰烬蛛刷新率）—— v1 默认值
- [ ] **赤裸期机制**（worldview "几层壳光后比纸还脆"）—— Q76 D 选项 v1 简化为无赤裸期
- [ ] **多套伪皮快切**（plan-inventory 未立 plan）—— v1 单套装备

### A.5 v1 关键开放问题

**已闭合**（Q74-Q79，6 个决策）：
- Q74 → B 共存（伪皮内层 + 真甲外层；contam 先扣伪皮，wound 直接进玩家）
- Q75 reframe → 自动 contam 累积达 capacity 脱壳，不是命中即扣 / 不是手动按键
- Q76 → D 无赤裸期（v1 简化）
- Q77 reframe → 伪皮不防 wound 只防 contam；worldview 克制（重狙 / 爆脉）自然实现
- Q78 → B 单层 + 朽木甲（轻 + 重两档）
- Q79 → A 玩家自制（NPC 商人 vN+1）

**仍 open**（v1 实施时拍板）：
- [ ] **Q80. 制作时间**：skeleton "1 时辰静心制单层" 实装时是否真做"静坐 N 分钟"？建议**简化为即时制作**（5 qi 扣完即得伪皮 item，符合 v1 plan-armor 配方框架）—— P0 拟
- [ ] **Q81. 单层 vs 多层 ShedEvent 是否合并**：朽木甲一次撑爆多层时是 emit 1 个 ShedEvent { layers_shed: 3 } 还是 3 个 ShedEvent？建议**合并 1 个**（agent narration 更顺）
- [ ] **Q82. 伪皮 contam 是否会自然漏失**：伪皮长期穿戴，absorbed_contam 是否随时间衰减？建议**否**（伪皮静态，contam 锁死直到脱壳；若漏失会让"长期穿戴慢慢清空"成为 trick）
- [ ] **Q83. 装备朽木甲时是否能装其他真甲**：worldview / plan-armor 没明说"朽木甲属于 FalseSkin 子类还是独立 ArmorKind"。建议**朽木甲也是 FalseSkin 子类**（穿在真甲下层，重量分类为重）

### A.6 物资派的"无特性"声明

worldview §五:471 已正典化："**蜕壳流是物资派——它的 primary axis 不绑修士身体，只看带了什么材料、几层伪皮。代价在钱包，不在真元**"

v1 严格遵守：
- ❌ 不实装真元逆逸散特性给伪皮 absorbed_contam 漏失（也不会漏失）
- ❌ 不实装毒性真元给脱壳"附毒效果"
- ❌ 不实装染色加成（凝实色 / 沉重色）
- ❌ 不实装顿悟 effect 给伪皮防御
- ✅ 唯一影响伪皮的"修士属性"：制作时的 `Cultivation.realm`（醒灵不可制；引气+ 才能注真元入伪皮）

vN+1 可考虑加"伪皮制作工艺 skill"——属技能熟练度系统而非真元修炼，仍符合"物资派"定位。

---



## §0 设计轴心

- [ ] 蜕壳 = **用假心镜（伪灵皮）替修士分担一次污染**
- [ ] 烧材料经济 → "仓鼠玩家"友好
- [ ] 末法约束：脱壳后赤裸 30 秒无防御
- [ ] 主动脱壳 ≠ 被动护盾——需要修士自主判断时机

## §1 第一性原理（烬灰子四论挂点）

- **影论·伪镜面**：伪灵皮 = 用蛛丝/朽木编织的"假心镜薄片"，注入少量真元模拟真镜的本音
- **音论·骗音**：当异种真元打过来时，先撞上伪灵皮的假本音——**误以为已经入侵成功**——这时主动脱壳，把假镜面连同已入侵的污染一起切断
- **噬论·脱皮即销**：脱下来的伪皮立刻被天地吞掉（连污染一起带走）
- **缚论·假不是真**：伪灵皮虽能承污染，但不能承真正的"镜身崩"——所以爆脉流燃命可以打穿一切伪皮（缚论的物理优先级）

## §2 形态分级

| 形态 | 材料 | 防御层数 | 真元成本 |
|---|---|---|---|
| **单层伪皮** | 蛛丝 1 卷 + 真元 5 | 1 次脱壳 | 装备消耗 |
| **三层伪皮** | 蛛丝 3 卷 + 真元 15 | 3 次脱壳（递减效果）| 同上 |
| **朽木甲** | 死域朽木 + 蛛丝 + 真元 30 | 5 次脱壳，负重 +20% | 同上 |
| **灵纹伪皮**（高阶）| 异变兽皮 + 蛛丝 + 真元 60 | 5 次 + 单次承受量 +50% | 通灵期才能制 |

**蛛丝来源**：拟态灰烬蛛（peoples-0005，2-3 骨币 / 具，一具产蛛丝半套）→ 三层伪皮需 6 具蛛

## §3 数值幅度梯度（按境界）

```
醒灵：不能制（无法注真元入伪皮）
引气：单层伪皮，承受 ≤ 10 异种真元
凝脉：三层伪皮，承受 ≤ 30
固元：朽木甲（5 层），承受 ≤ 60
通灵：灵纹伪皮，承受 ≤ 120
化虚：理论上"无穷叠层"——已断绝
```

**单次承受公式**（提案）：
```
shed_capacity = base_layer × (1 + integrity_buff)
```

## §3.1 蜕壳·v1 规格（P0 阶段）

> worldview §五.防御.2（line 437-440）+ §五:471（"物资派" 正典）+ §五"流派由组合涌现"（2026-05-03 正典）锚定：**伪皮只过滤真元污染**，受击 contam 累积达 capacity 自动脱壳。v1 收敛到**单层蛛丝伪皮**（P0）+ **朽木甲 3 层**（P1，§3.2）。**v1 不绑特性 / 不实装染色加成**，但 tuike 长期专精会涌现凝实色（vN+1 加成接入）。

### 3.1.A FalseSkin component + ArmorKind 扩展（Q74: B 共存）

**plan-armor-v1 扩展**（`server/src/combat/armor.rs`）：

```rust
pub enum ArmorKind {
    None,              // 已有
    Light,             // 已有 (轻甲)
    Medium,            // 已有 (中甲)
    Heavy,             // 已有 (重甲)
    /// **新增** plan-tuike-v1：替尸/蜕壳流伪灵皮
    /// 与真甲共存（Q74: B）—— 伪皮在内层先扣 contam，真甲在外层照常防 wound
    FalseSkin(FalseSkinKind),
}

pub enum FalseSkinKind {
    /// 单层蛛丝伪皮（v1 P0）
    SpiderSilk,
    /// 朽木甲 3 层（v1 P1，§3.2）
    RottenWoodArmor,
    // vN+1: ThreeLayerSilk / SpiritWeavedArmor / ...
}
```

**新增 component**：

```rust
#[derive(Component, Debug, Clone, Serialize, Deserialize)]
pub struct FalseSkin {
    pub kind: FalseSkinKind,
    pub layers_remaining: u8,           // 单层=1; 朽木甲=3
    pub contam_capacity_per_layer: f64, // 单层蛛丝=10.0; 朽木甲=30.0
    pub absorbed_contam: f64,           // 当前层累积，达 capacity 触发脱壳
    pub equipped_at: GameTime,
}
```

**装备槽冲突规则（Q74: B 共存）**：
- 玩家可同时装备：1 件 `Armor` (真甲) + 1 件 `FalseSkin` (伪皮)
- 受击时序：先扣 FalseSkin.absorbed_contam（截胡 contam 写入）→ FalseSkin 脱壳 / 溢出 contam → 真甲照常处理 wound（plan-armor-v1 已有 wound 减免逻辑）

### 3.1.B Contam 拦截系统：`tuike_filter_contam`（Q75 reframe + Q77 reframe）

**核心机制**：在 `resolve_attack_intents` 写入 `Contamination.entries.push` 之前，由 tuike 系统截胡：

```rust
/// 攻击命中后、Contamination 写入前的拦截。
/// Q77 reframe: 伪皮只防 contam，wound 直接进玩家。
pub fn tuike_filter_contam(
    incoming_contam: f64,
    qi_color: QiColor,
    target_skin: Option<&mut FalseSkin>,
) -> ContamFilterResult {
    let Some(skin) = target_skin else {
        return ContamFilterResult { passes_through: incoming_contam, shed_layers: 0 };
    };

    if skin.layers_remaining == 0 {
        return ContamFilterResult { passes_through: incoming_contam, shed_layers: 0 };
    }

    let mut remaining = incoming_contam;
    let mut shed_count = 0;

    while remaining > 0.0 && skin.layers_remaining > 0 {
        let cap = skin.contam_capacity_per_layer;
        let space = cap - skin.absorbed_contam;

        if remaining >= space {
            // 撑爆当前层
            skin.absorbed_contam = 0.0;
            skin.layers_remaining -= 1;
            shed_count += 1;
            remaining -= space;
            // worldview "重狙连壳带人" 自然实现：高 contam 量 → 多层撑爆 → 溢出进玩家
        } else {
            // 部分填充，不脱壳
            skin.absorbed_contam += remaining;
            remaining = 0.0;
        }
    }

    ContamFilterResult { passes_through: remaining, shed_layers: shed_count }
}
```

**调用点**（`server/src/combat/resolve.rs` jiemai 分支后、Contamination.push 前）：

```rust
// 现有：last_contam.amount = emitted_contam_delta;
// 现有：contamination.entries.push(...);

// 改写：先走 tuike 拦截
let mut filter_result = tuike_filter_contam(
    emitted_contam_delta,
    intent.qi_color,
    false_skins.get_mut(target_entity).ok(),
);

if filter_result.shed_layers > 0 {
    // 触发脱壳事件
    out_events.send(CombatEvent::Shed {
        target: target_id.clone(),
        layers_shed: filter_result.shed_layers,
        kind: false_skins.get(target_entity).map(|s| s.kind),
        contam_absorbed: emitted_contam_delta - filter_result.passes_through,
        contam_overflow: filter_result.passes_through,
    });
    // Q81 拍板：合并为 1 个 ShedEvent，layers_shed 字段记次数
}

if filter_result.passes_through > 0.0 {
    // 溢出 contam 才进玩家
    contamination.entries.push(ContamSource {
        attacker_id: Some(attacker_id.clone()),
        amount: filter_result.passes_through,
        color: intent.qi_color.into(),
        introduced_at: clock.tick,
    });
}
// wound 不变，继续 plan-armor wound 减免 + Wounds.push（Q77 reframe: 伪皮不防 wound）
```

### 3.1.C 单层蛛丝伪皮数值（v1 P0）

| 字段 | 值 | 备注 |
|---|---|---|
| `kind` | `FalseSkinKind::SpiderSilk` | — |
| `layers_remaining` | 1 | 单层 |
| `contam_capacity_per_layer` | 10.0 | worldview §五:466 "单层吸收上限" + skeleton §3 引气期 ≤10 |
| 制作真元 cost | 5.0 | skeleton §2 + worldview §三 真元池 |
| 主料 | 拟态灰烬蛛丝 1 卷 | peoples-0005 |
| 辅料 | — | 无 |
| 重量 (zhenmai 耦合) | Light ×1.0 | zhenmai-v1 Q63 |

**含义**：
- 受击单次 contam ≤ 10 → 累积进伪皮，不脱壳
- 受击单次 contam > 10 → 一次撑爆 + 溢出 (e.g. 受击 contam=40 → 蛛丝 capacity 10 撑爆 + 溢出 30 进玩家 = "重狙连壳带人")
- 累积达 10 → 脱壳，伪皮失效
- 玩家可背包再换一层（v1 简化）

### 3.1.D 制作流程（Q79: A 玩家自制）

**plan-armor-v1 crafting 框架扩展**（`server/src/crafting/false_skin.rs` 新文件）：

```rust
pub struct FalseSkinRecipe {
    pub kind: FalseSkinKind,
    pub main_material: ItemId,         // bong:tuike/spider_silk
    pub aux_materials: Vec<ItemId>,    // 朽木甲: vec![bong:tuike/rotten_wood]
    pub qi_cost: f64,
    pub min_realm: Realm,              // 引气+ 才能制（worldview "醒灵不能制"）
    pub instant: bool,                 // Q80 拟：v1 即时制作（不做"1 时辰静心"）
}

pub fn forge_false_skin(
    crafter: Entity,
    recipe: &FalseSkinRecipe,
    cult: &mut Cultivation,
    inventory: &mut Inventory,
) -> Result<ItemInstance, CraftError> {
    // 1. 校验境界
    if cult.realm.tier() < recipe.min_realm.tier() {
        return Err(CraftError::RealmTooLow);
    }
    // 2. 校验材料
    inventory.consume(&recipe.main_material, 1)?;
    for aux in &recipe.aux_materials {
        inventory.consume(aux, 1)?;
    }
    // 3. 扣真元
    if cult.qi_current < recipe.qi_cost {
        return Err(CraftError::NotEnoughQi);
    }
    cult.qi_current -= recipe.qi_cost;
    // 4. 产出 ItemInstance
    Ok(ItemInstance::new(recipe.kind.into_item_id()))
}
```

**v1 P0 配方表**：
- `RECIPE_SPIDER_SILK_FALSE_SKIN`：1 卷蛛丝 + 5 qi → 1 件 SpiderSilk 伪皮，min_realm = YinQi
- `RECIPE_ROTTEN_WOOD_ARMOR`（P1 §3.2）：1 块朽木 + 2 卷蛛丝 + 30 qi → 1 件 RottenWoodArmor，min_realm = NingMai

### 3.1.E 装备 / 脱卸 / 切换（Q76: D 无赤裸期）

**装备**（client packet `bong:armor/equip_false_skin { slot, item_instance_id }`）：
```
玩家装备 FalseSkin item →
server 校验：
  - target.armor_slot 已 equip 的 FalseSkin? 报错（一件 slot 只能一件 FalseSkin）
  - target 持有该 item? 否报错
  - target.realm >= recipe.min_realm? 否报错
通过 →
  inventory 移除 item
  attach FalseSkin component { layers_remaining: recipe.layers, ... } 到 target
  emit FalseSkinEquippedEvent
```

**主动脱卸**（v1 不实装；vN+1 加 unequip packet）：
- v1 玩家无法主动脱伪皮，只能等 contam 撑光自然脱壳

**全部脱光后**（layers_remaining = 0）：
- FalseSkin component 移除
- 玩家可立即 equip 新一件（Q76: D **无赤裸期**）
- 后续 contam 直接进 Contamination

### 3.1.F worldview 克制关系自然实现（Q77 reframe）

**"器修重狙→蜕壳"自动化**（worldview §五:451）：
- anqi 单针 hit_qi（凝脉 50 / 固元 80）
- contam profile 50/50 → contam = 25-40
- 蛛丝 capacity 10 → 一次撑爆 + 溢出 15-30 contam
- "连壳带人秒杀" 物理实现 ✅

**"爆脉→蜕壳"自动化**（worldview §五:386-389）：
- 爆脉 wound 直接进 target.Wounds（伪皮不防 wound）
- 爆脉 contam 进伪皮（如有）
- 爆脉的杀伤靠 wound 不靠 contam，伪皮无效 ✅

**v1 不需 anqi/baomai 特判代码**——`tuike_filter_contam` 通用逻辑覆盖所有攻击源。

### 3.1.G 与 plan-zhenmai-v1 装备耦合

zhenmai-v1 Q63（已闭合）+ tuike Q78 = 自动联动：

```rust
fn jiemai_armor_modifier(weight: WeightClass) -> f32 {
    match weight {
        WeightClass::Light => 1.0,
        WeightClass::Medium => 0.9,
        WeightClass::Heavy => 0.6,
    }
}

// FalseSkinKind → WeightClass mapping
impl FalseSkinKind {
    pub fn weight_class(&self) -> WeightClass {
        match self {
            FalseSkinKind::SpiderSilk => WeightClass::Light,
            FalseSkinKind::RottenWoodArmor => WeightClass::Heavy,
        }
    }
}
```

**含义**：穿朽木甲玩 zhenmai parry → prep window 1000ms × 0.6 = 600ms（仅穿伪皮场景；若同时穿真甲则取较重值）

### 3.1.H 物资派声明（worldview §五:471 严格遵守）

**v1 不实装**：
- ❌ 真元逆逸散特性 → absorbed_contam 不漏失（设计上也不漏失，特性无作用）
- ❌ 毒性真元 → 脱壳不附毒
- ❌ 染色加成（凝实色 / 沉重色）
- ❌ 顿悟 effect 给伪皮防御
- ✅ 唯一"修士属性"接入：制作时的 `Cultivation.realm` 校验（醒灵不可制）

vN+1 可加"伪皮制作工艺 skill"（plan-skill-v1 已落地）——技能熟练度系统而非真元修炼，仍符合"物资派"。

---

## §3.2 蜕壳·v1 规格（P1 阶段：朽木甲 3 层）

延续 §3.1 设计，扩展朽木甲档：

### 3.2.A 朽木甲数值

| 字段 | 值 |
|---|---|
| `kind` | `FalseSkinKind::RottenWoodArmor` |
| `layers_remaining` | 3 |
| `contam_capacity_per_layer` | 30.0 |
| 制作真元 cost | 30.0 |
| 主料 | 死域朽木 1 块 |
| 辅料 | 拟态灰烬蛛丝 2 卷 |
| 重量 | Heavy ×0.6（zhenmai 耦合） |

**总 contam 防御**：90.0（30×3 层）

### 3.2.B 多层 ShedEvent 合并（Q81）

朽木甲一次撑爆多层时（极高 contam 攻击）：
- emit 1 个 `ShedEvent { layers_shed: 3, contam_absorbed: 90, contam_overflow: ... }`
- 不分多次 emit，agent narration 一次性 "X 三层壳同时崩落"

### 3.2.C agent narration 触发档

| ShedEvent | narration 风格 |
|---|---|
| layers_shed = 1, overflow = 0 | "X 一层壳挡住污染，蛛丝化烬而去" |
| layers_shed = 1, overflow > 0 | "X 一层壳承不住，污染漫入身体" |
| layers_shed >= 2 | "X 多层壳同时崩落 — 重狙之下连壳带人" |
| layers_shed == 总层数 (光) | "X 已无壳防身，赤裸面对污染" |

### 3.2.D 与 plan-zhenmai-v1 重耦合后果

朽木甲 + zhenmai parry：
- prep window: 1000ms × 0.6 = 600ms（短）
- jiemai_qi_cost: 不变
- 玩家选择压力：朽木甲提供 90 contam 防御 vs zhenmai window 缩 40%
- 这就是 v1 trade-off matrix 的第一个 emergent gameplay

---

## §4 材料 / 资源链

| 阶段 | 材料 | library 来源 | 用途 |
|---|---|---|---|
| 主料 | **拟态灰烬蛛丝** | peoples-0005（一具半套）| 所有伪皮基础 |
| 辅料 | 死域朽木 | worldview 死域 | 朽木甲负重换层数 |
| 高阶 | 异变兽皮 | peoples-0005 缝合兽 | 灵纹伪皮 |
| 灵物磨损 | 同 plan-anqi-v1 | ecology-0004 | 入囊出囊扣 10%——伪皮一般贴身穿不入囊 |

**蛛丝采集**：拟态灰烬蛛在死域边缘，咬伤会粘液封经脉三息——采集本身有风险

## §5 触发 / 流程

```
准备阶段：制作伪皮（1 卷蛛丝 + 5 真元 + 1 时辰静心 → 单层）
装备：穿在外层（占用 armor 槽）
战斗中受击：
  攻击命中 → 检查"伪皮层数 > 0" →
    ✅ 玩家按 shed → 当前一层失败 + 污染 = 0 + 体表 +1 档
    ❌ 玩家未按 / 漏判 → 异种真元穿透伪皮入经脉
脱壳完成：剥离层数 -1 + 真元扣 5 + 30 秒"赤裸期"无伪皮防御
```

## §6 反噬 / 失败代价

- [ ] 脱壳失败（异种真元穿透伪皮）→ 污染照常 + 体表 +2 档
- [ ] 脱壳后赤裸 30 秒（材料消耗）
- [ ] 连续脱壳 3 层后体表伤口 +1（伪皮和真皮粘连）
- [ ] 制作失败（被打断 / 真元不足）→ 蛛丝浪费
- [ ] 朽木甲负重 → 移速 -20% / 弹反窗口（plan-zhenmai）-30%

## §7 克制关系

- **克**：毒蛊流（污染连壳带走，蜕壳就是脱毒）；持续侵染流派
- **被克**：暗器流（一发穿透伪皮，连壳带人；worldview "器修重狙→蜕壳"）；爆脉流（贴脸燃命直接打穿所有壳——缚论的优先级）
- **染色关联**：世界观 §六.2 明确防御三流是战术选择，不绑定染色。器修（凝实色）制伪皮天然顺手但非专属；锋锐色（剑修）伪皮挂不住

## §8 数据契约

### v1 P0 落地清单（按 §3.1 规格）

| 模块 | 文件路径 | 核心内容 |
|---|---|---|
| FalseSkin component | `server/src/combat/tuike.rs` (新文件) | `FalseSkin { kind, layers_remaining, contam_capacity_per_layer, absorbed_contam, equipped_at }` |
| ArmorKind 扩展 | `server/src/combat/armor.rs` | 新增 `ArmorKind::FalseSkin(FalseSkinKind)` variant + `FalseSkinKind::SpiderSilk` |
| Contam 拦截系统 | `server/src/combat/tuike.rs` | `tuike_filter_contam(incoming, color, &mut FalseSkin) -> ContamFilterResult` |
| Resolve 调用点 | `server/src/combat/resolve.rs` | jiemai 分支后、`Contamination.entries.push` 前嵌入 tuike 拦截 |
| 制作配方 | `server/src/crafting/false_skin.rs` (新) | `FalseSkinRecipe` / `forge_false_skin` / `RECIPE_SPIDER_SILK_FALSE_SKIN` |
| Item registry | `server/assets/items/tuike.toml` | `bong:tuike/spider_silk` (素材) / `bong:tuike/false_skin_silk` (成品) |
| ShedEvent | `server/src/combat/events.rs` | 新增 `CombatEvent::Shed { target, layers_shed, kind, contam_absorbed, contam_overflow }` |
| Schema | `agent/packages/schema/src/tuike.ts` | `FalseSkinStateV1` / `ShedEventV1` |
| Inbound packet | `client/.../net/TuikePackets.java` | `bong:armor/equip_false_skin { slot, item_instance_id }` |
| Outbound packet | `client/.../net/TuikePackets.java` | `bong:tuike/false_skin_state` HUD payload |
| Client HUD | `client/.../hud/TuikeHud.java` | 当前伪皮层数 + absorbed_contam 进度条 |
| 单测 | `server/src/combat/tuike_tests.rs` | 单层撑爆 / 累积不撑爆 / 装备校验 / wound 不被防 / 多 attacker 累积 |

### v1 P1 落地清单（按 §3.2 规格）

| 模块 | 文件路径 | 核心内容 |
|---|---|---|
| 朽木甲制作 | `server/src/crafting/false_skin.rs` | `RECIPE_ROTTEN_WOOD_ARMOR`（蛛丝 2 + 朽木 1 + 30 qi → 3 层）|
| 死域朽木 item | `server/assets/items/tuike.toml` | `bong:tuike/rotten_wood`（worldgen 死域投放，vN+1 调参） |
| 多层撑爆递归 | `server/src/combat/tuike.rs` | `tuike_filter_contam` 已含 while 循环（§3.1.B 落地即支持多层）|
| zhenmai 重耦合 | `server/src/combat/jiemai.rs` (zhenmai-v1) | `FalseSkinKind::RottenWoodArmor → WeightClass::Heavy ×0.6` 自动接入 |
| Agent narration | `agent/packages/tiandao/src/tuike-narration.ts` | `ShedEventV1` → 按 layers_shed 分档触发 |

### v1 P2 落地清单

| 模块 | 文件路径 | 核心内容 |
|---|---|---|
| 数值平衡 | `server/src/combat/tuike_balance.rs` (可选) | fight room 演练（anqi 重狙 vs 蛛丝 / dugu 长线 vs 朽木甲）|
| LifeRecord | `server/src/lore/life_record.rs` | "X 在 N 战中脱了 M 层壳" 事件类型 |

## §9 实施节点

详见 §A.3 v1 实施阶梯。三阶段：

- [ ] **P0** 单层蛛丝伪皮闭环 + tuike_filter_contam + 制作配方 + plan-zhenmai-v1 轻甲耦合 —— 见 §3.1
- [ ] **P1** 朽木甲 3 层 + 多层撑爆递归 + zhenmai-v1 重耦合 + agent narration —— 见 §3.2
- [ ] **P2** v1 收口（数值平衡 + LifeRecord）

## §10 开放问题

### 已闭合（2026-05-03 拍板，6 个决策 + 2 个 reframe）

- [x] **Q74** → B 共存（伪皮内层 + 真甲外层；contam 先扣伪皮，wound 直接进玩家）
- [x] **Q75 reframe** → 自动 contam 累积达 capacity 脱壳，**不是命中即扣 / 不是手动按键**
- [x] **Q76** → D 无赤裸期（v1 简化）
- [x] **Q77 reframe** → **伪皮不防 wound，只防 contam**；worldview 克制（重狙 / 爆脉）自然实现，不需 bypass / 特判
- [x] **Q78** → B 单层蛛丝 + 朽木甲（轻 + 重两档）
- [x] **Q79** → A 玩家自制（NPC 商人 vN+1）

### 仍 open（v1 实施时拍板）

- [ ] **Q80. 制作时间**：建议**即时制作**（5/30 qi 扣完即得伪皮 item）—— P0 拟，与 plan-armor crafting 对齐
- [ ] **Q81. 多层 ShedEvent 合并**：建议**合并 1 个 event**（layers_shed 字段记次数；agent narration 更顺）—— P1 拟
- [ ] **Q82. 伪皮 absorbed_contam 是否漏失**：建议**否**（伪皮静态锁住污染，避免"长期穿戴慢慢清空"成为 trick）—— P0 拟
- [ ] **Q83. 朽木甲是否仍归 FalseSkin 子类**：建议**是**（属物资派伪皮范畴；穿在真甲下层；重量分类 Heavy）—— P1 拟

### vN+1 留待问题（plan-tuike-v2 时拍）

- [ ] **三层蛛丝伪皮 / 灵纹伪皮**（worldview §五.防御.2 + skeleton §2 高阶款）
- [ ] **凝实色器修加速制作**（worldview §六:542 凝实色 × 器修原生匹配）
- [ ] **NPC 商人售卖**（plan-baomai 联动 / 蛛丝伪皮 8 骨币 / 朽木甲 50 骨币）
- [ ] **蛛丝产能调参**（plan-tsy-hostile-v1 拟态灰烬蛛刷新率）
- [ ] **赤裸期机制重启**（worldview "几层壳光后比纸还脆"暗示）—— v1 简化为无赤裸期
- [ ] **多套伪皮快切 / 库存**（plan-inventory 落地后）
- [ ] **伪皮制作工艺 skill**（plan-skill-v1 扩展，技能熟练度而非真元修炼）
- [ ] **真元逆逸散特性 vs 伪皮**（设计上 v1 不漏失，vN+1 是否引入特性 hook）

## §11 进度日志

- 2026-04-26：骨架创建。依赖 plan-armor-v1（装备槽 / 重量）+ peoples-0005（蛛丝产能）。无对应详写功法书，从 worldview + 战斗流派源流 推演。
- 2026-05-03：从 skeleton 升 active。§A 概览 + §3.1 P0 + §3.2 P1 蜕壳·v1 规格落地（6 个决策点闭环 Q74-Q79，2 个关键 reframe（Q75/Q77），4 个 v1 实装时拍板 Q80-Q83）。primary axis = 伪皮档位 + 材料色克 + 单层吸收上限（worldview §五:466）。**关键反转**：伪皮**只防 contam 不防 wound**（worldview §五.防御.2 锚定"优先承受污染"）。**worldview 克制关系自然实现**：重狙连壳带人（高 contam 撑爆 + 溢出）/ 爆脉打穿一切伪皮（爆脉 wound 直接进玩家），**不需 bypass / 特判**。**物资派声明**（worldview §五:471 已正典）：v1 严格不绑染色 / 不绑特性。直接接 plan-armor-v1 装备槽（FalseSkin 在真甲下层）+ plan-zhenmai-v1 重量耦合（蛛丝轻 / 朽木重）。

## Finish Evidence

### 落地清单

- P0 单层蛛丝伪皮闭环：`server/src/combat/tuike.rs` 新增 `FalseSkin` / `FalseSkinKind::SpiderSilk` / `tuike_filter_contam` / `forge_false_skin` / `FalseSkinForgeRequest`；`server/assets/items/tuike.toml` 新增蛛丝伪皮与材料；`server/src/combat/resolve.rs` 在截脉和护甲之后、`Contamination.entries.push` 之前执行伪皮过滤，wound 始终直进玩家。
- P0 装备槽与制作入口：`server/src/inventory/mod.rs` 新增 `false_skin` equip slot；`server/src/schema/client_request.rs` / `agent/packages/schema/src/client-request.ts` / `client/src/main/java/com/bong/client/network/ClientRequestProtocol.java` 接入 `equip_false_skin` 与 `forge_false_skin`；`server/src/network/client_request_handler.rs` 完成背包校验、境界校验和事务式扣料扣 qi。
- P1 朽木甲与多层脱壳：`FalseSkinKind::RottenWoodArmor` 为 3 层、单层 30 contam、制作消耗蛛丝 2 + 朽木 1 + 30 qi；`tuike_filter_contam` 对大污染量合并为一个 `ShedEvent { layers_shed }`，支持一次撑爆多层并把溢出污染继续传给玩家。
- P1 截脉重量耦合与叙事：`FalseSkinKind::jiemai_window_modifier` 对蛛丝为 1.0、朽木甲为 0.6，并在 `apply_defense_intents` 与 zhenmai 最新负重窗口合并；`server/src/network/tuike_event_bridge.rs` 发布 `bong:tuike/shed`，`agent/packages/tiandao/src/tuike-narration.ts` 订阅后写入 `bong:agent_narrate`。
- P1/P2 跨端状态与 HUD：`server/src/network/false_skin_state_emit.rs` 推送 `false_skin_state`；`client/src/main/java/com/bong/client/combat/handler/FalseSkinStateHandler.java` 更新 `DerivedAttrsStore.tuikeLayers`；`InventoryEquipRules` / `EquipmentPanel` / `ServerDataRouter` / `ClientRequestSender` 支持伪皮装备、制作和显示。
- P2 生平记录与持久化：`server/src/combat/tuike.rs` 新增 `record_shed_events_in_life_record`；`server/src/cultivation/life_record.rs` 新增 `BiographyEntry::FalseSkinShed`；`server/src/persistence/mod.rs` 新增 `false_skin_shed` 事件类型；`server/src/schema/cultivation.rs` 与 `agent/packages/schema/src/biography.ts` 对齐 schema。

### 关键 commit

- `1ea8ce12` · 2026-05-04 · `feat(tuike): 接入伪皮战斗与制作闭环`
- `686fdf01` · 2026-05-04 · `feat(tuike): 对齐 schema 与天道蜕壳叙事`
- `96476a48` · 2026-05-04 · `feat(tuike): 接入客户端伪皮装备与 HUD`
- `5a7a2097` · 2026-05-04 · `feat(tuike): 记录蜕壳生平战绩`
- `4f961539` · 2026-05-04 · `fix(tuike): 收敛防御查询类型`
- `28408fb9` · 2026-05-04 · `fix(tuike): 对齐生平记录格式化`

### 测试结果

- `cd server && cargo test tuike -- --nocapture`：通过；13 tests passed。
- `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`：通过；2238 Rust tests passed。
- `cd agent && npm run build && (cd "packages/tiandao" && npm test) && (cd "packages/schema" && npm test)`：通过；tiandao 33 files / 228 tests，schema 9 files / 267 tests。
- `cd client && JAVA_HOME="/usr/lib/jvm/java-17-openjdk-amd64" PATH="/usr/lib/jvm/java-17-openjdk-amd64/bin:$PATH" ./gradlew test build`：通过；Gradle BUILD SUCCESSFUL，JUnit report 803 tests / 0 failures / 0 errors。
- `git diff --check`：通过；无 whitespace error。

### 跨仓库核验

- server：`FalseSkin` / `FalseSkinKindV1` / `ShedEvent` / `tuike_filter_contam` / `FalseSkinForgeRequest` / `record_shed_events_in_life_record` / `ServerDataPayloadV1::FalseSkinState` / `CH_TUIKE_SHED` / `CH_TUIKE_FALSE_SKIN_STATE`。
- agent/schema：`FalseSkinStateV1` / `ShedEventV1` / `ClientRequestEquipFalseSkinV1` / `ClientRequestForgeFalseSkinV1` / `BiographyEntryV1.FalseSkinShed` / generated JSON artifacts。
- agent/tiandao：`TuikeNarrationRuntime` / `agent/packages/tiandao/src/skills/tuike.md` / `CHANNELS.TUIKE_SHED` / `CHANNELS.AGENT_NARRATE`。
- client：`ClientRequestProtocol.encodeEquipFalseSkin` / `encodeForgeFalseSkin` / `FalseSkinStateHandler` / `EquipSlotType.FALSE_SKIN` / `InventoryEquipRules` / `ServerDataRouter`。

### 遗留 / 后续

- v1 按 §A.4 明确不做三层蛛丝伪皮、灵纹伪皮、凝实色制作加成、NPC 商人售卖、蛛丝产能调参、多套伪皮快切和赤裸期机制；这些继续留给 `plan-tuike-v2` 或对应系统 plan。
- 数值验证已覆盖蛛丝 10 contam、朽木甲 3 层共 90 contam、重污染多层撑爆与 LifeRecord 入账；独立 fight room 可在后续平衡 plan 中扩展，不阻塞本 plan 归档。
