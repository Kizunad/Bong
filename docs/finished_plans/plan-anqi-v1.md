# Bong · plan-anqi-v1

**器修·暗器流**（攻击）。把真元封入异变兽骨等高级载体——20 秒充能后 30-50 格冷距离狙击。"载体击破之时，封存真元注入敌身。"

**Primary Axis**（worldview §五:462 已正典）：**载体封存比例 + 命中距离**

## 阶段总览

| 阶段 | 状态 | 验收 |
|---|---|---|
| **P0** 狙射闭环（封 → 投 → 衰减 → 命中注射 → 射空蒸发） | ⬜ | — |
| **P1** inventory 决策面（hand-slot 持骨 + 自然漏失 tick + HUD） | ⬜ | — |
| P2 v1 收口（饱和测试 + agent narration） | ⬜ | — |

> **vN+1 (plan-anqi-v2)**：6 档载体全开 + 多发齐射 + 诱饵战术 + 封灵匣容器 + 磨损税 + 凝魂/破甲注射功法 + 多容器（箭袋 / 裤袋）

---

## 世界观 / library / 交叉引用

**worldview 锚点**：
- §五.2 器修/暗器流（line 405-413：载体材质分级 50 格保留 80%）
- §五:462 流派 primary axis 表（载体封存比例 + 命中距离）
- §四 距离衰减（line 332-340：贴脸 100% / 10 格 40% / 50 格归零；异变兽骨 50 格 80%）
- §四 越级原则与全力一击（line 360-391：暗器封 30% qi_max 是"半全力击"，**不触发**战后虚脱）
- §六 染色谱（line 542：凝实色 × 器修原生匹配；line 480 毒性真元/凝实色泛型化）
- §十一 封灵匣 / 负灵袋（line 1416：异兽骨骼/灵木编制；不入箱使用不扣次数 — vN+1 接入）

**library 锚点**：
- `peoples-0005 异变图谱·残卷`（兽爪 10 骨币 / 兽核 80 骨币 / 缝合兽来源）
- `ecology-0004 灵物磨损笔记`（"暗器载体每入囊出囊扣一成真元 / 故器修持骨于手不入囊" — **v1 已知偏离**，vN+1 必须接入）
- `peoples-0006 战斗流派源流`（攻击三·器修/暗器流原文）
- `ecology/绝地草木拾遗`（云顶兰 / 悬根薇 / 渊泥红玉 — vN+1 高阶载体来源）

**交叉引用**：
- `plan-combat-no_ui-v1` ✅（已落地）— 复用 `AttackIntent` / `CombatEvent::AttackResolved` / `Wounds` / `Contamination`；P0 **首次实装** spec 已定义但未落地的 `ThrowCarrierIntent` + `decay_factor` 公式
- `plan-zhenfa-v1` 🟡（active P0/P1 已落地）— 与诡雷共享底层 `CarrierImprint` 字段（封存机制同源），任一 plan 修改字段必须双侧同步
- `plan-perception-v1.1` ✅（已落地，2026-05-01）— 命中后 `CombatEvent::AttackResolved` 已走 perception 边缘指示器
- `plan-weapon-v1` ✅（已落地）— Bow 第 496 行 "v1 不做 ranged"，**anqi 是仓库第一个 ranged combat 实装**
- `plan-cultivation-v1` ✅（已落地）— `Cultivation.qi_current` 扣封真元 / `QiColor` 决定染色加成 / `Contamination.entries` 接污染
- `plan-inventory-v1` ⬜（未立 plan）— P1 hand-slot 持骨 vs 入囊取舍；vN+1 接磨损税 + 箭袋容器

## 接入面 checklist（防孤岛 — 严格按 docs/CLAUDE.md §二）

- **进料**：`Cultivation.qi_current` 扣封真元 → `QiColor` 决定衰减加成 → `Lifecycle` 校验非死亡态 → `inventory` 取/放 charged 异变兽骨 item
- **出料**：`CombatEvent::AttackResolved`（命中）/ 新增 `CombatEvent::ProjectileDespawned`（射空 / 飞出 max_range）→ `Contamination.entries.push` 写污染 → `Wounds` 写实体伤 → `CombatSummary` 节流推 agent → `bong:combat/carrier_*` outbound payload 同步 client HUD
- **共享类型 / event**：复用 `AttackIntent` / `CombatEvent` / `Wounds` / `Contamination` / `Stamina`；首次实装 `ThrowCarrierIntent`；新增 `ChargeCarrierIntent` / `CarrierImprint` component
- **跨仓库契约**：
  - server: `combat::carrier::Carrier` component / `combat::carrier::ChargeCarrierIntent` event / `combat::carrier::ThrowCarrierIntent` event / `combat::projectile::QiProjectile` component / `combat::carrier::carry_decay_tick` system
  - schema: `agent/packages/schema/src/combat-carrier.ts` → `CarrierStateV1` / `CarrierChargedEventV1` / `CarrierImpactEventV1` / `ProjectileDespawnedEventV1`
  - client: `bong:combat/charge_carrier` (inbound, 新) / `bong:combat/throw_carrier` (inbound, plan-combat-no_ui §604 已定义) / `bong:combat/carrier_state` (outbound, 新) HUD payload
- **特性接入面（v1 仅留 hook 不实装 — worldview §五"特性偏泛型"已正典）**：
  - **真元逆逸散效率**（zhenfa primary axis 泛型化）→ 载体封存半衰期 × (1 + 加成) — 直接延长可用时长 + 间接延长有效射程
  - **毒性真元**（dugu primary axis 泛型化）→ 命中注射时 contam 附毒 tag — 凝实色 + 毒性 = 远射毒针组合
  - **凝实色 (Solid)**（worldview §六:542）→ 飞行衰减系数 -0.03/格（已在 plan-combat-no_ui §3.2 公式内）

**Hotbar 接入声明**（2026-05-03 user 正典化"所有技能走 hotbar"）：
- **`bong:combat/charge_carrier`**（静坐 20s 充能封真元）= **Technique** → P0 实装时改走 hotbar 1-9（`Technique::AnqiChargeCarrier` 绑战斗·修炼栏 + UseQuickSlot 触发）
- **`bong:combat/throw_carrier`**（持骨抛掷）= **物品 use action**（attack 键复用）→ **不走 hotbar**，保留物品 packet
- 详见 `plan-woliu-v1.md §8 跨 plan hotbar 同步修正备注`。

---

## §A 概览（设计导航）

> 暗器流 = 真元唯一的"离体存活"路径：必须靠物理载体（兽骨 / 灵木）锁住真元，命中前以一定保留率送达，命中瞬间载体破碎"注射"进敌经脉。**v1 实装最小狙射闭环**——单档异变兽骨 + 单发狙射 + 飞行衰减 + 命中注射 + 射空蒸发。

### A.0 v1 实装范围（2026-05-03 拍板）

| 维度 | v1 实装 | 搁置 vN+1 |
|---|---|---|
| 载体档位 | **异变兽骨单档**（Q30: A）| 凡铁 / 普骨 / 灵木 / 道伥残骨 / 异变兽爪 |
| 封真元过程 | **20 秒主动可中断（暂存"半封态"）**（Q31: D + 20s）| 凝实色加速封 / 长封 30 min |
| 真元投入 | **滑块自选 [0, min(qi_max × 30%, 80)]**（Q35: D）| 超载 (>30%) 功法接入 |
| 持有方式 | **主手 / 副手任一 hand slot**（Q32: A∪B 合并）| 箭袋 / 裤袋多容器 |
| 同时持有 | **单根**（Q40: A）| 多根 stack + 箭袋 |
| 飞行衰减 | **§3.2 公式调系数对齐 worldview 80%**（Q36: A 校准）| 功法/特性改逸散效率 |
| 命中分配 | **50% wound + 50% contam（异变兽骨 default profile）**（Q37: 子弹参考）| profile 表 + 凝魂/破甲注射功法 |
| 自然漏失 | **半衰期 120 min（基础材质 2h）**（Q38: C + 基础）| 真元逆逸散特性 / 封灵匣容器 |
| 瞄准 | **client crosshair raycast + hitbox 等效偏大**（Q39: A + 容错）| server soft-lock |
| 射空 | **70% 蒸发 + 30% 残留落地（5s 归零，期间他人可捡）**（Q33: B）| 二次拾取降级利用 |
| 磨损税 | **v1 不做（已知偏离 ecology-0004）**（Q34: deferred）| vN+1 必须接入 align worldview |

### A.1 跨流派"载体封存"机制同源

| 维度 | zhenfa 异变兽核 | anqi 异变兽骨 |
|---|---|---|
| 形态 | 镶嵌进白名单方块 | 持手 / 投出 / 碎裂 |
| 时长 | 24h 朽坏 | 半衰期 120 min |
| 投入上限 | 50% qi_max | 30% qi_max（Q35 拍） |
| 触发 | 踩入 / 主动引爆 | 抛投 + raycast 命中 |
| 命中机制 | 单格 / 3×3 / 5×5 范围伤害 | 单 entity 注射（§3.1.E） |

**共用底层 component**（v1 P0 落地时定义）：

```rust
pub struct CarrierImprint {
    pub qi_amount: f32,                // 当前残留真元
    pub qi_color: QiColor,             // 来源色（决定 §3.2 衰减加成）
    pub source_realm: Realm,           // 封存者境界（决定越级加成）
    pub half_life_min: f32,            // 半衰期分钟数（anqi 默认 120, zhenfa 不用此字段填 f32::INFINITY）
    pub decay_started_at: GameTime,    // ContaminationTick 切片基准
    pub bond_kind: BondKind,           // ::HandheldCarrier (anqi) | ::EmbeddedTrap (zhenfa)
}
```

**修改任一 plan 的 `CarrierImprint` 字段必须双侧同步**——红旗事件。

### A.2 跨流派"距离衰减公式"首次落地

plan-combat-no_ui §3.2 `decay_factor(distance, color, grade)` spec 存在但**实测代码未实装**（grep 验证：`server/src/combat/resolve.rs` 仅有 reach-based 近战 decay）。

**anqi P0 = 仓库首次实装** ranged decay 公式。落地后：
- zhenfa 主动触发距离（worldview §六:421 缜密色 +50% 距离）也复用同公式
- 后续所有 ranged 流派（暗器 / 飞针 / 御物）共享此公式

**校准目标**（worldview §四 line 332-340 锚定）：
- 0 格：100% 保留
- 10 格：40% 保留（普通真元）
- 50 格：~80% 保留（异变兽骨 + 凝实色）

**plan-combat-no_ui §3.2 原公式系数实测对不上**：base_decay 0.06 + carrier_bonus 0.04 + color_bonus 0.03 → 50 格 clamp 到 100%。**P0 落地时调系数**（建议 base_decay 0.07-0.10，carrier_bonus 0.04-0.05），通过单测拟合 worldview 三个数据点。具体系数留 P0 实装时拟定（Q41 见下）。

### A.3 v1 实施阶梯

```
P0  狙射闭环（最 close-loop，定 anqi 数值标尺）
       ChargeCarrierIntent (20s) → CarrierImprint 写入兽骨 →
       持手 → ThrowCarrierIntent → QiProjectile spawn →
       client raycast hint + server raycast (hitbox 偏大) →
       命中 → 50% wound + 50% contam → CombatEvent::AttackResolved →
       射空 → 70% 蒸发 + 30% 残留落地 → 5s 后 ProjectileDespawned
       ↓ 调系数对齐 worldview §四 80%
P1  inventory 决策面 + 自然漏失 + HUD
       carry_decay_tick (半衰期 120 min)
       hand-slot 持骨绑定（主手/副手都可）
       client HUD: charge progress bar + sealed_qi 数字 + half-life timer
       ↓ 饱和化 testing
P2  v1 收口 + agent narration
       agent 接 CarrierImpactEvent / ProjectileDespawned 生成 narration
       LifeRecord 写入"X 在 N 格外狙射 Y"事件
```

### A.4 v1 已知偏离正典（vN+1 必须修复）

- [ ] **磨损税**（ecology-0004 已确立）—— v1 不实装意味玩家可自由入囊；跟"故器修出门多直接持骨于手"不一致
- [ ] **6 档载体**（worldview §五:408 已列）—— v1 仅异变兽骨单档，凡铁/灵木/道伥残骨缺席
- [ ] **封灵匣容器**（worldview §1416 已列）—— v1 不实装；charged 兽骨在背包/箱子内一律按"持身"处理（不漏失，但 vN+1 必须区分"持身 vs 入箱 vs 入封灵匣"）

### A.5 v1 关键开放问题

**已闭合**（Q30-Q40 见 §A.0 表）。

**仍 open**（v1 实施时拍板）：
- [ ] **Q41. 距离衰减公式具体系数**：base_decay / carrier_bonus / color_bonus 三个数 — 留 P0 实装时单测拟合 worldview §四 三个数据点（0/10/50 格）
- [ ] **Q42. hitbox 等效偏大具体范围**：0.3m / 0.5m / 0.8m — 建议起手 0.4m，对齐 MC 默认箭支 hit margin，留 P0 调参
- [ ] **Q43. 半衰期归零阈值**：当 sealed_qi 衰减到多少 % 时载体自动碎裂 / 真元被天地吞干？建议 5%（120 × log2(20) = ~520 min ≈ 8.7h 后归零）
- [ ] **Q44. 残留落地的 5s 窗口期**：他人路过能不能"误触发"？建议**否**（5s 内仅"实体捡起"动作触发归零，进入触发格不触发）—— 与 zhenfa 诡雷"踩雷触发"互斥语义需明确

---

## §0 设计轴心

- [ ] 暗器 = **真元唯一的离体存活方式**，靠外置载体承载临时镜印
- [ ] 载体材质决定真元保存时长 + 飞行衰减
- [ ] 末法约束：磨损税强迫"持骨于手"——影响 inventory loadout
- [ ] 射空一枚 = 50%-100% 真元蒸发；"三发不中被人贴上来一掌打死"

## §1 第一性原理（烬灰子四论挂点）

- **影论·外置镜面**：兽骨/灵木 = **外置临时镜面**——异变兽骨保留兽类生前真元的镜印，可替修士本心镜分担投影压力。这是真元唯一可以"离体存活"的物理路径
- **音论·凝实色专项**：长期封真元的器修，真元染上凝实色（真元如附薄膜，易附外物）。书里"灵泉湿地一器修，真元封入三根兽骨飞五十格不散"
- **噬论·载体破即时机**：载体一破，真元再无依托 → 立刻被天地吞掉。所以**必须命中后碎裂**才能注射真元——碎裂瞬间有"无主时刻"，趁此塞进敌经脉
- **缚论**：高阶载体（道伥残骨）含强镜印，可暂借大量"镜材"——故越稀有越能封真元

## §2 载体材质分级

| 材质 | 镜印强度 | 50 格保留 | 封真元时长 | 来源 |
|---|---|---|---|---|
| 凡铁 / 木石 | 极弱 | 25% | 10 分钟 | 普通采集（不推荐）|
| 普通兽骨 | 弱 | 50% | 30 分钟 | 普通野兽 |
| **异变兽爪** | 中 | 80% | 2 小时 | 异变缝合兽（10 骨币 / 爪）|
| **异变兽骨** | 中 | 80% | 2 小时 | 异变图谱-残卷 |
| 灵木 | 中-高 | 85% | 4 小时 | worldgen 稀有树种 |
| 道伥残骨 | 高 | 90% | 6 小时 | 道伥掉落（极稀，需识字配残卷）|

## §3 数值幅度梯度（按境界）

```
醒灵：不能修暗器流（封真元能力不够）
引气：单发狙射，载体仅普通兽骨，封真元 ≤ 10
凝脉：双发齐射，可用异变兽骨，封 ≤ 30
固元：三发齐射 + 诱饵战术，封 ≤ 60
通灵：凝实色持续加成，载体保真元时间 +50%
化虚：理论上"远施法术千里"——但化虚已断绝
```

## §3.1 暗器·v1 规格（P0 阶段）

> worldview §五:407 锚定："花半小时将 80% 真元封入 3 根骨刺，30 格外冷酷狙击。载体命中碎裂时，封存真元直接'注射'进敌人体内"。v1 收敛到**单根**（Q40: A）+ **20 秒充能**（Q31: D + 20s）+ **30% qi_max 投入**（Q35: D）。

### 3.1.A 载体（Q30: A 异变兽骨单档 + Q34 deferred 备注）

**v1 唯一载体**：异变兽骨（来源 peoples-0005 异变图谱·残卷）

| 字段 | 值 | 说明 |
|---|---|---|
| `item_id` | `bong:anqi/yibian_shougu` | 新增 item，对接 `plan-inventory` 当前槽位系统 |
| `tier` | Beast (中) | 对应 plan-combat-no_ui §3.2 `carrier_grade::Beast` |
| `qi_color_affinity` | Solid (凝实色) | worldview §六:542 锚定，凝实色玩家天然搭配 |
| `qi_承载上限` | `min(qi_max × 30%, 80)` | Q35: D，玩家滑块在此区间自选 |
| `half_life_min` | 120 min | Q38: C + 基础材质 2h |
| `获取来源` | 异变缝合兽掉落（plan-tsy-hostile） | v1 默认掉落率 30%（vN+1 调参） |

**白名单外**：v1 **不接受**任何其他材质（凡铁/普骨等）—— 玩家手持其他物品按 `bong:combat/charge_carrier` 触发时 server 直接 reject 并 `IntentRejected { reason: InvalidCarrier }`。

**容器规则（Q34 deferred）**：
- v1 **charged 兽骨在背包/箱子里都不漏失**（除自然漏失 tick），跟"持手"无差别 —— 这是 ecology-0004 的 known divergence，§A.4 已记录
- vN+1 接入磨损税后：入囊扣 10% 真元 + 入封灵匣不扣 + 持身不扣

**同时持有（Q40: A）**：v1 hand-slot 同时只能持 **1 根 charged 异变兽骨**。背包内可以堆多根**未充能**素材，但 charged 兽骨**不可堆叠**（每根独立 `CarrierImprint` 实例）。

### 3.1.B 封真元过程（Q31: D + 20s 主动可中断 + 暂存半封态）

**触发**：玩家手持**素材兽骨**（未充能） + `bong:combat/charge_carrier` packet 入站

**流程**（server 权威）：
```
玩家发 ChargeCarrierIntent { carrier_slot, qi_target } →
server 校验：
  - carrier 是异变兽骨素材？
  - qi_target ∈ [0, min(qi_max × 30%, 80)]？
  - Cultivation.qi_current >= qi_target？
  - 玩家不在 NearDeath / 不在另一次充能中？
通过 → 进入"充能态" CarrierCharging { started_at, qi_target, accumulated, slot }
       Cultivation.qi_current 立即扣 qi_target × 0.5（已支付一半）
       client HUD 显示 charge progress bar (0..20s)

每 tick：accumulated += qi_target / 20s × dt
       超过 20s → 完成：
         CarrierImprint 写入兽骨：{ qi_amount: qi_target, ... }
         Cultivation.qi_current 再扣剩下 50%（首次扣 50%，完成扣 50%，总 qi_target）
         emit CarrierChargedEventV1 → client / agent
         CarrierCharging 移除

中断条件：玩家移动到 1 格外 / 玩家攻击 / 玩家被攻击命中 / 玩家切换 hand-slot →
       充能进入"半封态"暂存（worldview §五:407 "花半小时" 的精神延续 — 不浪费）
       半封态结构：CarrierImprint { qi_amount: qi_target × accumulated_ratio × 0.5, ... }
       说明：
         - 真元投入：玩家已扣 50% qi_target，"半封态"按 accumulated_ratio 实际有效
         - 比如充能 10s（50%）就中断 → 兽骨上写入 qi_target × 0.5 × 0.5 = qi_target × 25%
         - 玩家剩下的 50% qi_target 仍可继续充能（恢复 ChargeCarrierIntent 时不重新扣，只追充剩余）
       这就是 Q31 D 的"材料不浪费"语义 — 玩家失败一半也能拿到一半成品

完整态 vs 半封态：CarrierImprint.bond_kind 都是 ::HandheldCarrier，差异仅在 qi_amount 实际值
```

**充能时的 qi_color**：完成时刻取玩家**当前** `Cultivation.qi_color`（由染色系统决定）。半封态被 reject 后中途换色再补充能 → **以最后一次完成时刻的色为准**（设计避免跨色叠加复杂度）。

### 3.1.C 真元投入与上限（Q35: D 滑块 + 超载预留）

`qi_target` 玩家自选，client 滑块 UI 锁定到：

```
qi_target_range = [0, min(qi_max × 30%, 80)]
```

**境界对应实际上限**（按 worldview §三 真元池正典数值）：

| 境界 | qi_max | 30% 比例 | 80 hard cap | 实际上限 |
|---|---|---|---|---|
| 醒灵 | 10 | 3 | — | **3** |
| 引气 | 40 | 12 | — | **12** |
| 凝脉 | 150 | 45 | — | **45** |
| 固元 | 540 | 162 | 80 | **80**（撞硬顶）|
| 通灵 | 2100 | 630 | 80 | **80** |
| 化虚 | 10700 | 3210 | 80 | **80** |

**含义**：
- 醒灵期玩家最多封 3 qi/根（弱）
- 凝脉期成熟玩家可封 45 qi/根（甜区）
- 固元+ 玩家受 80 硬顶限制 — 单根威力被钉死，**强迫高境玩家"出门多封几根才有量"** —— 跟 vN+1 多发齐射 + 箭袋接入面对齐

**超载（vN+1 接口预留）**：
- v1 **不实装**超载功法，但 server 端 `qi_target` 校验留 hook：
  ```rust
  let cap = if has_overload_skill(player) {
      (qi_max * 0.5).min(160.0)  // 假设超载功法解锁 50% / 硬顶 160
  } else {
      (qi_max * 0.3).min(80.0)
  };
  ```
- vN+1 落地超载功法时只改 `has_overload_skill` 入参逻辑，不动 v1 数值
- "全力一击"（worldview §四 line 380）问题：anqi 不触发战后虚脱（封 30% 是"半全力击"）—— **重要**：超载功法解锁后封 50%+ 是否触发虚脱留 vN+1 拍

### 3.1.D 飞行衰减 + 命中判定（Q36: A 校准 + Q39: A + hitbox 偏大）

#### 3.1.D.1 飞行衰减公式（首次实装 plan-combat-no_ui §3.2）

**调系数对齐 worldview §四 锚点**（line 332-340）：
- 0 格：100%
- 10 格：40%（普通真元，无 carrier 加成）
- 50 格：~80%（异变兽骨 Beast + 凝实色 Solid）

公式形态（保 §3.2 spec）：
```
hit_qi_ratio(distance, color, grade) = clamp(
    base_decay
    + color_bonus_per_block × distance
    + carrier_bonus_per_block × distance,
    0.0, 1.0
)

base_decay      = 1.0 - 0.10 × distance         # 比 §3.2 原值 0.06 更陡
color_bonus     = match qi_color {
                    Solid => +0.04,             # 凝实色 +0.04/格
                    Sharp => +0.03,
                    Light => +0.035,
                    Mellow => -0.02,
                    _ => 0.0,
                }
carrier_bonus   = match grade {
                    Beast => +0.05,             # 异变兽骨 +0.05/格
                    Spirit => +0.06,            # vN+1 灵木
                    Relic => +0.08,             # vN+1 道伥残骨
                    _ => 0.0,
                }
```

**校验对齐 worldview**：
- 普通真元 10 格：1.0 - 0.10 × 10 = 0.0 → ❌ 应是 0.4

**单测拟合需求**：v1 P0 落地时跑单测拟合，以下面三组样本为目标：

| distance | color | grade | 期望 hit_qi_ratio | worldview 锚点 |
|---|---|---|---|---|
| 0 | Any | Any | 1.00 | 贴脸 100% |
| 10 | Mellow/Default | Mundane | 0.40 | "普通玩家 10 格火球损失 60%" §四:336 |
| 50 | Solid | Beast | 0.80 | "异变兽骨 50 格保留 80%" §四:410 |

**Q41**：base_decay / color_bonus / carrier_bonus 三个数最终怎么调，留 P0 实装时单测驱动。建议起手 `base_decay = 0.06`（保 §3.2 原值）+ `color_bonus(Solid) = 0.04` + `carrier_bonus(Beast) = 0.05`，用 Mellow + Mundane 校 10 格 0.40，用 Solid + Beast 校 50 格 0.80。

#### 3.1.D.2 命中判定（Q39: A + hitbox 偏大）

**轨迹**：服务端权威 swept-volume tick（参 plan-combat-no_ui §3.4）。每 tick 把 projectile AABB 从 `prev_pos` 扫到 `next_pos`，与所有候选 `entity.aabb_inflated` 求 swept 相交。

**hitbox inflation**（Q39: A + 容错）：
```rust
let aabb_inflated = entity.aabb.expanded_by(0.4);  // 0.4m 等效偏大（Q42 留实装调）
```
- 不修改玩家瞄准灵敏度，但**子弹判定 hitbox 比 entity 实际大 0.4m**
- 视觉效果：玩家"擦边"也算命中，给慢速狙射玩家容错
- 跟 MC 默认箭支 hit margin 一致（Q42 起手 0.4m）

**raycast 流程**：
```
ThrowCarrierIntent { carrier_slot, dir_unit, power } →
server spawn QiProjectile entity {
    velocity: dir_unit × base_throw_speed × power_factor (60-90 格/s)
    qi_payload: CarrierImprint.qi_amount
    qi_color: CarrierImprint.qi_color
    grade: CarrierImprint.tier  // Beast
    spawn_pos: player.eye_pos + dir_unit × 0.5
    max_distance: 80 格（v1 硬顶）
}
进入 ProjectileTickSet:
  for tick in 0..max:
    next_pos = pos + velocity × dt
    if next_pos.distance_from_spawn > max_distance:
      → ProjectileDespawnedEvent { reason: OutOfRange }; break
    swept_aabb 与所有 entity.aabb_inflated 求相交
    if 命中 entity X:
      hit_distance = pos.distance_to(spawn_pos)
      hit_qi = qi_payload × hit_qi_ratio(hit_distance, qi_color, grade)
      → CombatEvent::AttackResolved { ... }（按 §3.1.E 注射）
      despawn
      break
    if 命中方块（地面/墙）:
      → ProjectileDespawnedEvent { reason: HitBlock, residual_pos: hit_block_pos }
      触发 §3.1.G 射空残留逻辑
      break
    pos = next_pos
```

**主动瞄准强迫**：client 必须先 `bong:combat/throw_carrier { dir_unit }`，server 根据 dir_unit 计算轨迹。client 不发 dir_unit 则 reject。这就是 Q39 A "完全 client crosshair raycast"。

### 3.1.E 命中注射（Q37: 子弹 profile + default 50/50）

#### 3.1.E.1 默认 profile（v1 单档异变兽骨）

> worldview §四:344-349 已锚定"侵染 / 排异反应 / 交换比亏损"机制。

参考现实子弹学，对应 v1 异变兽骨形态：

| 参考类比 | 形变度 | 穿透度 | 临时空腔 | 永久创伤 | 对应 anqi 行为 |
|---|---|---|---|---|---|
| 软铅圆头弹 | 中 | 中 | 中 | 中 | **v1 异变兽骨 default** |
| 空尖弹 | 高 | 浅 | 大 | 大 | vN+1 灵木 (大形变 + 大 wound) |
| 钢芯穿甲 | 低 | 深 | 小 | 小（深部脏器损伤） | vN+1 道伥残骨 (低 wound + 大 contam) |
| 钝铅 | 高 | 浅 | 小 | 大表面伤 | vN+1 凡铁 (大 wound + 小 contam) |

**v1 异变兽骨 default profile**（中形变 + 中穿透 + 镜印释放 → 各占一半）：

```rust
fn anqi_carrier_profile(carrier_kind: CarrierKind) -> InjectProfile {
    match carrier_kind {
        // v1
        CarrierKind::YibianShougu => InjectProfile {
            wound_ratio: 0.5,
            contam_ratio: 0.5,
        },
        // vN+1 (preview only, not impl in v1)
        CarrierKind::LingMu => InjectProfile { wound_ratio: 0.7, contam_ratio: 0.3 },
        CarrierKind::DaoYangCanGu => InjectProfile { wound_ratio: 0.3, contam_ratio: 0.7 },
        CarrierKind::FanTie => InjectProfile { wound_ratio: 0.6, contam_ratio: 0.4 },
        // ...
    }
}
```

#### 3.1.E.2 注射数值

```
hit_qi = qi_payload × hit_qi_ratio(...)        # §3.1.D 公式
profile = anqi_carrier_profile(carrier_kind)

wound_damage = hit_qi × profile.wound_ratio × body_part_mul(hit_point)  # §3.4 部位倍率
contam_amount = hit_qi × profile.contam_ratio                            # 直接进 contam，按 worldview §四:348 "排掉 10 要花 15" 由 cultivation::ContaminationTick 接管

CombatEvent::AttackResolved {
    attacker, target,
    hit: true,
    damage: wound_damage,
    body_part,
    body_color: target.qi_color,
    qi_color: imprint.qi_color,
    cause: AttackCause::AnqiCarrier { carrier_kind, hit_distance, sealed_qi_initial: qi_payload },
}
Wounds.entries.push(Wound {
    location: body_part,
    severity: severity_from(wound_damage),
    kind: WoundKind::Penetrating,
    bleeding_per_sec: bleed_from(wound_damage),
    inflicted_by: attacker,
})
Contamination.entries.push(ContamSource {
    attacker_id: attacker,
    amount: contam_amount,
    qi_color: imprint.qi_color,
})
```

**载体物理碎裂**：命中后 server despawn QiProjectile + spawn 客户端粒子（凝实色调色，参 plan-vfx-v1）。**v1 不掉落任何 item drop**（vN+1 残骸可拾取重炼留 hook）。

#### 3.1.E.3 vN+1 功法接口预留

`InjectProfile` 字段不动，仅暴露 hook：
- **凝魂注射**：`profile.wound_ratio = 0.0; profile.contam_ratio = 1.0`（全 contam）
- **破甲注射**：`profile.wound_ratio = 0.7; profile.contam_ratio = 0.3`（重 wound）

v1 时 hook 为 no-op，functional code 写成 `apply_skill_modifier(profile, skills)` 占位。

### 3.1.F 自然漏失 tick（Q38: 半衰期 + 特性 + 封灵匣 vN+1）

**v1 异变兽骨基础值**：`half_life_min = 120`（2h）

**衰减公式**（半衰期模型）：
```rust
fn carry_decay_tick(time: Res<GameTime>, mut imprints: Query<&mut CarrierImprint>) {
    for mut imprint in imprints.iter_mut() {
        if imprint.bond_kind != BondKind::HandheldCarrier { continue; }  // zhenfa 走 §3.1.F (zhenfa)
        let elapsed_min = (time.now - imprint.decay_started_at).as_secs_f32() / 60.0;
        let half_lives = elapsed_min / imprint.half_life_min;
        let new_qi = imprint.qi_amount_initial * 0.5_f32.powf(half_lives);
        imprint.qi_amount = new_qi;
        if new_qi / imprint.qi_amount_initial < 0.05 {
            // Q43 阈值 — 实用上限（建议 5%）
            // → 载体自动碎裂，触发 ProjectileDespawned { reason: NaturalDecay }
            // qi 100% 蒸发（不残留）
        }
    }
}
```

**衰减时间表**（异变兽骨基础值 120 min）：

| 经过时间 | 残留比例 | 实战可用？ |
|---|---|---|
| 0 min | 100% | ✅ 满血 |
| 60 min | 71% | ✅ 良好 |
| 120 min | 50% | ⚠️ 一半 |
| 240 min | 25% | ⚠️ 弱 |
| 360 min | 12.5% | ❌ 接近废 |
| 520 min ≈ 8.7h | ~5% | ❌ 自动碎裂 |

**特性 hook（vN+1 接口预留）**：
```rust
fn effective_half_life(base: f32, traits: &PlayerTraits) -> f32 {
    let mut hl = base;
    if traits.has(Trait::QiAntiDissipation) {
        hl *= 1.0 + traits.get_level(Trait::QiAntiDissipation) * 0.5;
        // 满级 +100% → 半衰期 240 min
    }
    hl
}
```

**封灵匣 hook（vN+1 接口预留）**：worldview §1416 锚定。
```rust
fn decay_paused_in_storage(item_storage: ItemStorageContext) -> bool {
    matches!(item_storage, ItemStorageContext::SealedSpiritBox(_))
    // v1 永远 false（封灵匣 plan-forge 还没立）
    // vN+1: 放进封灵匣的 charged 兽骨 carry_decay_tick 跳过
}
```

**bond_kind 隔离**：本系统**只处理** `BondKind::HandheldCarrier`。zhenfa 的 `BondKind::EmbeddedTrap` 走另一套朽坏 tick（plan-zhenfa-v1 §3.1.F-zhenfa 命名空间），**不互串**。

### 3.1.G 反噬与失败（Q33: 70% 蒸发 + 30% 残留落地 + 5s 归零）

#### 3.1.G.1 射空（命中方块 / 飞出 max_range）

```
ProjectileDespawned 触发时：
  qi_at_despawn = qi_payload × hit_qi_ratio(distance_traveled, ...)
  // (此时距离比命中略远，hit_qi_ratio 已衰减)

  // Q33: B 实装
  qi_evaporated = qi_at_despawn × 0.7   # 70% 立刻被天地吞
  qi_residual = qi_at_despawn × 0.3     # 30% 残留

  spawn 落地 item: ItemEntity {
    item: bong:anqi/yibian_shougu_charged,
    CarrierImprint { qi_amount: qi_residual, half_life_min: 0.083, ... }
    # 0.083 min = 5s 半衰期 → 5s 后归零（Q33 "5 秒后归零"语义）
  }
  emit CombatEvent::ProjectileDespawned { reason, residual_qi: qi_residual, pos: despawn_pos }
```

**残留兽骨 5s 窗口**（Q44 设计）：
- 5s 内**他人捡起**（pickup item）→ `CarrierImprint` 转移到该玩家 inventory，但 half_life_min 保留 0.083，5s 后无论如何归零（防止"路过捡漏"成为可靠战术）
- 5s 内**进入触发格**（路过踩上）→ **不触发任何事**（v1 与 zhenfa 诡雷"踩雷触发"互斥语义明确）
- 5s 后落地兽骨：CarrierImprint 移除 → 退化为普通**素材兽骨**（item 形态变为 `bong:anqi/yibian_shougu_素材`），可被任意玩家捡走当材料重新封

#### 3.1.G.2 充能失败 / 中断

参 §3.1.B 半封态语义 — **不算反噬**，而是"暂存"。玩家失血失态都是中断条件，但兽骨不浪费。

#### 3.1.G.3 距离零超出

`max_distance = 80 格`（v1 硬顶）—— 飞出后视为 `OutOfRange` despawn，按 §3.1.G.1 残留逻辑执行。

#### 3.1.G.4 与 Stamina / qi 池子的耦合

- `ChargeCarrierIntent` 不消耗 stamina（封是静态过程）
- `ThrowCarrierIntent` 消耗 stamina **5**（同 plan-combat-no_ui §3.4 AttackIntent 默认值；强迫连射受体力限制）
- 封真元已扣 qi_target — 投出 / 射空都不再额外扣 qi（已是 sunk cost）

#### 3.1.G.5 凝实色养成失败惩罚（vN+1 留口）

skeleton §6 "凝实色养成失败（杂色）= 载体保真元时间砍半"—— v1 不实装染色养成系统（plan-color-v1 未立），凝实色加成纯靠玩家当前 `QiColor` 状态，不区分"养成失败的杂色"。vN+1 接入 `plan-color-v1` 后：
```rust
if qi_color == QiColor::Mixed_FailedSolid {
    half_life *= 0.5;
}
```

---

## §4 材料 / 资源链

| 阶段 | 材料 | library 来源 | 用途 |
|---|---|---|---|
| 入门 | 凡铁箭头 + 普通兽骨 | 自采 | 醒灵-引气期练手 |
| 主力 | **异变兽爪 / 兽骨** | peoples-0005（缝合兽）| 凝脉-固元主力载体 |
| 高阶 | 灵木 | worldgen | 远射载体（持续时间长）|
| 顶级 | 道伥残骨 | peoples-0005（道伥）| 通灵期 / 大战决招 |
| 配套 | 凝实色训练（长期）| 染色长期 | 真元保存加成（幅度待定）|

**磨损税**（ecology-0004）：
- 暗器载体**每入囊出囊扣 10% 真元**
- 强迫器修出门"持骨于手"——影响 plan-inventory-v1 的 hand-slot 设计
- 战略含义：暗器流玩家要在"出门前一次封够 + 全程持骨"和"路上才封 + 入囊保护"间取舍

## §5 触发 / 流程

```
平常：选材料 → 静坐封真元（凝实色加成；占用 5-30 分钟）→
  载体上 mirror_imprint = qi_charge ×（材质系数）
战斗：瞄准 → 抛射（飞行中按距离衰减真元）→
  命中 → 载体碎裂 → 注射真元至敌经脉（污染 = 注入量 × 0.7）
射空：载体落地 → 镜印失稳 → 真元在 5 秒内被天地吞干
```

## §6 反噬 / 失败代价

- [ ] 射空 = 50%-100% 真元蒸发（载体碎裂时无宿主）
- [ ] 载体超时未用 = 真元自然漏完（普通兽骨 30 分钟）
- [ ] 入囊出囊磨损税（每次 10%）
- [ ] 三发不中被贴脸 → 暗器流近战极弱（书里"老夫识一暗器流被人贴上来一掌打死"）
- [ ] 凝实色养成失败（杂色） = 载体保真元时间砍半

## §7 克制关系

- **克**：蜕壳流（一发穿透伪皮，连壳带人）；所有不擅长拉近距离的流派
- **被克**：涡流流（高爆发飞入涡流被天地法则没收）；爆脉流贴脸（书里原话）
- **染色亲和**：凝实色（器修原生匹配，真元保存加成幅度待定）
- **错配**：沉重色（体修）走暗器——封真元慢且漏，先天弱

## §8 数据契约

### v1 P0 落地清单（按 §3.1 规格）

| 模块 | 文件路径 | 核心内容 |
|---|---|---|
| Carrier component | `server/src/combat/carrier.rs` | `CarrierImprint` (与 zhenfa 共用) / `Carrier` / `CarrierCharging` / `BondKind` enum |
| Charge intent | `server/src/combat/carrier.rs` | `ChargeCarrierIntent` event + `charge_carrier_tick` system |
| Throw intent | `server/src/combat/carrier.rs` | `ThrowCarrierIntent` event（plan-combat-no_ui §457 spec 已定义，**首次实装**）|
| Projectile | `server/src/combat/projectile.rs` | `QiProjectile` component + `projectile_tick_system` swept-volume |
| Decay tick | `server/src/combat/carrier.rs` | `carry_decay_tick` 半衰期模型（v1 异变兽骨 120 min） |
| Distance decay | `server/src/combat/decay.rs` | `hit_qi_ratio(distance, color, grade)` —— **首次实装** plan-combat-no_ui §3.2 公式 |
| Inject profile | `server/src/combat/carrier.rs` | `anqi_carrier_profile` + `InjectProfile` (wound_ratio / contam_ratio) |
| Despawn 残留 | `server/src/combat/carrier.rs` | `spawn_residual_carrier` (Q33: 30% 残留 5s 归零) |
| Item registry | `server/assets/items/anqi.toml` | `bong:anqi/yibian_shougu` (素材) / `bong:anqi/yibian_shougu_charged` |
| Combat event 扩展 | `server/src/combat/events.rs` | 新增 `CombatEvent::ProjectileDespawned` / `AttackCause::AnqiCarrier` |
| Schema (TS) | `agent/packages/schema/src/combat-carrier.ts` | `CarrierStateV1` / `CarrierChargedEventV1` / `CarrierImpactEventV1` / `ProjectileDespawnedEventV1` |
| Inbound packet | `client/.../net/CarrierPackets.java` | `bong:combat/charge_carrier` / `bong:combat/throw_carrier` |
| Outbound packet | `client/.../net/CarrierPackets.java` | `bong:combat/carrier_state` (charge progress / sealed_qi / half-life timer) |
| Client HUD | `client/.../hud/CarrierHud.java` | charge progress bar + sealed_qi 数字 + half-life 进度 |

### v1 P1 落地清单

| 模块 | 文件路径 | 核心内容 |
|---|---|---|
| Hand-slot 持骨 | `server/src/inventory/hand_slot.rs` (新 / 与 plan-inventory 协调) | 主手 / 副手任一可持 charged 兽骨 (Q32 A∪B) |
| HUD 完善 | `client/.../hud/CarrierHud.java` | 半衰期 timer 可视化 + sealed_qi 衰减预览 |
| 单测饱和 | `server/src/combat/carrier_tests.rs` | 半衰期 / 充能中断 / 残留 5s / hitbox inflation |

### v1 P2 落地清单

| 模块 | 文件路径 | 核心内容 |
|---|---|---|
| Agent narration | `agent/packages/tiandao/src/anqi-narration.ts` | `CarrierImpactEventV1` 触发 narration（"30 格外冷酷狙击"风格） |
| LifeRecord | `server/src/lore/life_record.rs` | "X 在 N 格外狙射 Y" 事件类型 |

## §9 实施节点

详见 §A.3 v1 实施阶梯。三阶段总结：

- [ ] **P0** 狙射闭环（封 → 投 → 衰减 → 命中注射 → 射空蒸发）—— 见 §3.1
- [ ] **P1** inventory 决策面 + 自然漏失 tick + HUD —— 见 §3.1.A 持有规则 + §3.1.F 漏失公式
- [ ] **P2** v1 收口（饱和 testing + agent narration） —— 见 §8 P2 清单

## §10 开放问题

### 已闭合（2026-05-03 拍板）

- [x] **Q30** 载体档位 → A 异变兽骨单档
- [x] **Q31** 封真元过程 → D + 20s 主动可中断
- [x] **Q32** 持骨方式 → A∪B 主手/副手任一
- [x] **Q33** 射空真元归宿 → B 70% 蒸发 + 30% 残留 5s
- [x] **Q34** 磨损税 → deferred（vN+1 align ecology-0004）
- [x] **Q35** 真元投入上限 → D 滑块 + 超载预留
- [x] **Q36** 飞行衰减公式 → A 调系数对齐 worldview 80%
- [x] **Q37** 命中分配 → 50/50 default profile + 子弹参考
- [x] **Q38** 自然漏失 → C 半衰期 120 min + 特性接口 + 封灵匣 vN+1
- [x] **Q39** 瞄准 → A + hitbox 等效偏大
- [x] **Q40** 同时持有 → A 单根（vN+1 箭袋 / 裤袋）

### 仍 open（v1 实施时拍板）

- [ ] **Q41** 距离衰减公式具体系数（base_decay / color_bonus / carrier_bonus 三个数）—— P0 单测拟合 worldview §四 三个数据点
- [ ] **Q42** hitbox 等效偏大具体范围 —— 起手 0.4m，P0 调参
- [ ] **Q43** 半衰期归零阈值 —— 建议 5%（载体自动碎裂；qi 100% 蒸发不残留）
- [ ] **Q44** 残留落地 5s 窗口期他人是否能"误触发" —— 建议**否**（仅 pickup 触发，不与 zhenfa 踩雷语义混淆）

### vN+1 留待问题（plan-anqi-v2 时拍）

- [ ] 凝实色染色养成系统（杂色 = 半衰期砍半惩罚）—— 接 plan-color-v1
- [ ] 道伥残骨稀有度是否对齐 plan-tsy-loot —— 接 plan-tsy-loot 落地后
- [ ] 多发齐射的命中判定（逐发独立 raycast vs 集体判定）
- [ ] 残留载体被他人拾取的"二次磨损" —— 与 ecology-0004 磨损税系统对齐
- [ ] 异变兽骨从 plan-tsy-hostile 异变缝合兽掉落率（v1 默认 30%，vN+1 调参）
- [ ] 超载功法（>30% qi_max 投入）是否触发 worldview §四 战后虚脱

## §11 进度日志

- 2026-04-26：骨架创建。库藏锚点 peoples-0005（异变图谱）+ ecology-0004（磨损笔记）已就位。依赖 plan-cultivation-v1 染色系统 + plan-weapon-v1 投射物框架。
- 2026-05-03：从 skeleton 升 active。§A 概览 + §3.1 P0 暗器·v1 规格落地（11 个决策点闭环 Q30-Q40，4 个 v1 实装时拍板 Q41-Q44）。primary axis = 载体封存比例 + 命中距离（worldview §五:462）。v1 收敛到异变兽骨单档 + 单发狙射 + 半衰期 120 min。**首次实装** plan-combat-no_ui §3.2 飞行衰减公式 + §457 ThrowCarrierIntent。与 plan-zhenfa-v1 共用底层 `CarrierImprint` component。

## Finish Evidence

- 2026-05-04：server 暗器闭环落地：异变兽骨素材/充能态、`CarrierImprint`、20s 封元、hand-slot 投掷、飞行衰减、命中 wound+contam 注射、射空蒸发/残留、半衰期漏失、LifeRecord、生平持久化、Redis 事件桥、`carrier_state` server-data。
- 2026-05-04：agent/schema 落地：`CarrierStateV1`、`CarrierChargedEventV1`、`CarrierImpactEventV1`、`ProjectileDespawnedEventV1`、client request 契约、生成 schema artifact、anqi narration runtime 与回归测试。
- 2026-05-04：client 落地：`ChargeCarrier` / `ThrowCarrier` request sender、`carrier_state` router/store/handler、HUD `CarrierHudPlanner` 与 `HudRenderLayer.CARRIER` 接入。
- 2026-05-04 验证：
  - `cd server && cargo fmt --check`
  - `cd server && cargo clippy --all-targets -- -D warnings`
  - `cd server && cargo test`（2209 passed）
  - `cd agent && npm run build`
  - `cd agent/packages/schema && npm test`（267 passed）
  - `cd agent/packages/tiandao && npm test`（225 passed）
  - `cd client && JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64 PATH=/usr/lib/jvm/java-17-openjdk-amd64/bin:$PATH ./gradlew test build`
