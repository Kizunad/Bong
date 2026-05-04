# Bong · plan-dugu-v1

**毒蛊流**（攻击）。毒蛊真元慢性侵蚀经脉 → **永久 qi_max 下降**。任何武器（拳/剑/暗器/飞针）都可灌毒蛊真元 → 触发暴露与社会反噬。

**Primary Axis**（worldview §五:464 已正典）：**毒素真元累积速率 + 解毒难度**

## 阶段总览

| 阶段 | 状态 | 验收 |
|---|---|---|
| **P0** 凝针 + 灌毒蛊 + 经脉损伤 + 自解（B 30% 失败）+ inspect hook stub | ⬜ | — |
| **P1** 主动遮蔽（plan-perception hook 注入）+ 部位→经脉映射 + agent narration | ⬜ | — |
| P2 v1 收口（饱和测试 + LifeRecord） | ⬜ | — |

> **vN+1 (plan-dugu-v2)**：求医解 + 越级解 + 多经脉同时受毒 + 毒针变种（不同毒草不同毒效）+ 完整 NPC 信誉度系统（接 plan-identity-v1）

---

## 世界观 / library / 交叉引用

**worldview 锚点**：
- §五.4 毒蛊流（line 421-426：恶性寄生者 / 1 点脏真元 / 永久下降 / 专克高境 / 全陆追杀）
- §五:464 流派 primary axis 表（毒素真元累积速率 + 解毒难度）
- §五 末土后招原则（**2026-05-03 正典化**）："毒 vs 毒蛊"边界正典化 / 流派不是字段是行为涌现 / 毒蛊流是后招原则极端化
- §四 异体排斥（line 342-349：侵染 + 排异反应 + 交换比亏损）
- §四 越级原则（line 360-391：池子差距矩阵）— 毒蛊"专克高境"通过 D 公式实现
- §六:546 阴诡色 × 毒蛊师（毒蛊流附加效果+ / 自身经脉慢性侵蚀）
- §十一 身份与信誉（**2026-05-03 正典化**）：毒蛊师社会默认（被识破 → -50 baseline / 高境 NPC 追杀 / 中境拒交易）

**library 锚点**：
- `peoples-0006 战斗流派源流`（毒蛊流四 / "余识此流者未有善终"）
- `ecology-0002 末法药材十七种`：
  - **解蛊蕊**（幽暗地穴紫花，解蛊必备）— Q48 自解必需材料
  - **噬脉根**（负灵域浅层剧毒，毒蛊原料）— vN+1 高阶毒料
  - **终焉藤**（毒蛊终极原料，烧三日不熄）— vN+1 蛊养
  - **清浊草**（专克染色浊乱）— vN+1 自救阴诡色失控
- `cultivation/真元十一色考`（line 25：阴诡色 = 毒蛊师真元墨绿带腐臭）
- `ecology/绝地草木拾遗`：
  - 断戟刺（毒蛊飞针复合污染载体）— vN+1 飞针变种
  - 血色脉草夜面叶（侵骨毒）— vN+1 高阶毒料
- `geography/血谷残志`（cross-ref 毒蛊流）

**交叉引用**：
- `plan-cultivation-v1` ✅（已落地）— **直接接入 `Meridian.flow_capacity` 字段**（components.rs:124）+ qi_max recompute 链路
- `plan-combat-no_ui-v1` ✅（已落地）— 复用 §3.4 部位倍率（Q54: B 部位→经脉映射）+ `Contamination` 系统（区分 `DuguPoisonState` vs 普通 contam）
- `plan-perception-v1.1` ✅（已落地）— **直接接入 `obfuscate_sense_kind` hook**（plan 文件 line 6 已留 "留给 plan-stealth 替换"）
- `plan-anqi-v1` 🟡（active P0）— "毒性真元"特性给 anqi 暗器附毒**不挂 DuguPoisonState**（仅 contam tag）
- `plan-zhenfa-v1` 🟡（active P0/P1）— 同上，"毒性真元"特性给 zhenfa 诡雷加伤**不挂 DuguPoisonState**
- `plan-identity-v1` ⬜（**未立 plan，建议立**）— 身份切换 / NPC 信誉度系统（v1 dugu 仅留 hook）
- `plan-baomai-v1` ✅（已落地）— vN+1 求医解时接入 NPC 服务

## 接入面 checklist（防孤岛 — 严格按 docs/CLAUDE.md §二）

- **进料**：`Cultivation.qi_current` 扣凝针成本 + 灌毒蛊成本 → `cultivation::Meridian` (target) 写入 `flow_capacity` 永久减损 → `combat::Lifecycle` 校验非死亡态
- **出料**：`combat::AttackResolved`（命中）→ 新增 `cultivation::DuguPoisonState` component → 新增 `cultivation::DuguPoisonProgressEvent`（每 5 min tick 触发）→ agent narration / LifeRecord
- **共享类型 / event**：复用 `AttackIntent` / `CombatEvent::AttackResolved` / `Wounds` / `Stamina`；**新增** `ShootNeedleIntent` / `InfuseDuguPoisonIntent` / `DuguPoisonState` component / `DuguPoisonTick` system
- **跨仓库契约**：
  - server: `combat::needle::QiNeedle` component / `combat::needle::ShootNeedleIntent` event / `cultivation::dugu::InfuseDuguPoisonIntent` event / `cultivation::dugu::DuguPoisonState` component / `cultivation::dugu::dugu_poison_tick` system / `cultivation::dugu::resolve_self_antidote_intent` / `combat::stealth::DuguObfuscation` impl
  - schema: `agent/packages/schema/src/dugu.ts` → `DuguPoisonStateV1` / `DuguPoisonProgressEventV1` / `DuguObfuscationStateV1`
  - client: `bong:combat/shoot_needle` (inbound, 新) / `bong:combat/infuse_dugu_poison` (inbound, 新) / `bong:cultivation/self_antidote` (inbound, 新) / `bong:cultivation/dugu_poison_state` (outbound, 新, 仅自己可见自己受毒情况) HUD payload
- **特性接入面（v1 仅留 hook 不实装 — worldview §五"特性偏泛型"已正典）**：
  - **毒性真元**（特性，泛型）→ 让 anqi/zhenfa/拳的 contam 加 poison tag — **不挂 DuguPoisonState**（worldview "毒 vs 毒蛊" 边界）
  - **真元逆逸散效率**（zhenfa primary axis 泛型化）→ 飞针半衰期延长（v1 飞针不存载体故无效，留 hook）
  - **阴诡色**（worldview §六:546 dugu × 阴诡色原生匹配）→ 灌毒蛊真元成本 -X% / 主动遮蔽境界差容忍 +1（vN+1 实装）
- **plan-perception hook 接入**：`obfuscate_sense_kind` 由 dugu 师替换为 `DuguObfuscation`（plan-perception-v1.1 line 6 已留接口）

**Hotbar 接入声明**（2026-05-03 user 正典化"所有技能走 hotbar"）：
- **`bong:combat/shoot_needle`**（即时凝针 1 qi）= **Technique** → P0 实装时改走 hotbar 1-9（`Technique::DuguShootNeedle` 绑战斗·修炼栏 + UseQuickSlot 触发）
- **`bong:combat/infuse_dugu_poison`**（灌毒蛊真元）= **Technique** → P0 实装时改走 hotbar 1-9（`Technique::DuguInfusePoison`）
- **`bong:cultivation/self_antidote`**（服解蛊蕊 + 20 真元）= **Consumable**（含 cast time）→ 走 **F1-F9 快捷使用栏**（跟丹药同槽位机制），不是 1-9
- 详见 `plan-woliu-v1.md §8 跨 plan hotbar 同步修正备注`。

---

## §A 概览（设计导航）

> 毒蛊流 = **慢性侵蚀经脉的真元修炼形态**。表面玩任何流派（拳/剑/暗器），关键瞬间灌毒蛊真元 → 命中即挂 `DuguPoisonState` → target 经脉每 5 min `flow_capacity` 永久 ↓ → qi_max 永久 ↓。**触发即暴露**——主动遮蔽神识失效 5s。

### A.0 v1 实装范围（2026-05-03 拍板）

| 维度 | v1 实装 | 搁置 vN+1 |
|---|---|---|
| 凝针基础 | **任何流派可用，1 qi → spawn QiNeedle**（Q51: C, 即时凝针）| 多种针型（断戟刺 / 血色脉草夜叶）|
| 灌毒蛊机制 | **dugu 师专属，独立异步动作把毒蛊真元覆盖到下次出手载具**（Q51: C 类比 anqi 注入）| 多毒草毒效变种 |
| 毒针物理 | **复用 anqi 的 QiProjectile** + 1 qi 固定 + **不走衰减公式**（worldview §五:422 锚定）+ hitbox inflation 0.6m | — |
| 毒针 = 毒蛊？ | **❌ 否**——毒针是普通真元投射，灌毒蛊真元才是 dugu 流派核心 | — |
| 经脉损伤 | **`(flow_capacity × 1%) × poisoner_realm_tier`** 每 5 min（Q52: D）| 毒源叠加 / 多经脉同时 |
| 受毒经脉选择 | **击中部位映射经脉**（Q54: B；命中胸→心经 / 命中手→手经 / ...）| 高境毒蛊师可指定经脉 |
| 解毒（自解）| **解蛊蕊 + 20 真元 + 30% 失败概率**（Q48: B 先做，worldview 已正典 20 点）| 求医解 + 越级解 |
| 解毒失败后果 | **flow_capacity = 0**，该经脉永久废 | — |
| 主动遮蔽 | **always-on**（Q53: A）— 身份隐蔽不消耗真元 | 阴诡色 +1 境界差容忍 |
| 暴露触发 | **灌毒蛊真元出手时 = 暴露 5s**（普通毒不触发）| 完整 NPC 信誉度反应 |
| inspect 神识 | **plan-perception hook stub**（Q55: B v1 不深入实装）| 完整 inspect UI / 神识等级 |
| NPC 反应 | **v1 留 hook 不实装**（Q55: B）| plan-identity-v1 + plan-baomai 联动 |

### A.1 dugu 与 anqi/zhenfa 的"载具复用"

毒蛊真元可附着的载具：

| 载具 | 复用 plan | 灌毒蛊后行为 |
|---|---|---|
| **拳头**（无载具）| plan-combat-no_ui §3.1 AttackIntent | 命中即挂 DuguPoisonState |
| **凡剑 / 法剑** | plan-weapon-v1 | 同上 |
| **anqi 暗器**（兽骨）| plan-anqi-v1 | 命中即挂 DuguPoisonState（**取代** carrier 默认 contam profile）|
| **zhenfa 诡雷**（vN+1）| plan-zhenfa-v1 | 范围内全部挂 DuguPoisonState（高烈度但暴露范围大）|
| **凝针**（毒蛊师方便武器）| **本 plan 新建** `QiNeedle` | 标准毒蛊形态 |

**v1 范围**：仅做凝针 + 拳头 + anqi 暗器 三种灌毒蛊路径。剑（plan-weapon）vN+1。zhenfa 灌毒蛊（plan-dugu-v2 + plan-zhenfa-v2）。

### A.2 凝针 ≠ dugu 流派

凝针是**任何流派可用的真元投射方式**——

| | 普通玩家凝针 | dugu 师凝针 + 灌毒蛊 |
|---|---|---|
| 触发 | `ShootNeedleIntent` | `ShootNeedleIntent` + `InfuseDuguPoisonIntent` |
| 真元成本 | 1 qi | 1 qi（凝针）+ 灌毒蛊真元成本 |
| 命中效果 | 普通 contam（1 点）| `DuguPoisonState` 挂载 |
| 暴露神识 | 否 | **是**（5s 暴露窗口）|

**这就是 worldview "末土后招原则"的物理化**：凝针只是工具，毒蛊师藏在"普通飞针手"身份里，关键瞬间才灌毒蛊。

### A.3 v1 实施阶梯

```
P0  核心闭环（最 close-loop，定 dugu 数值标尺）
       ShootNeedleIntent (任何玩家) → spawn QiNeedle →
       InfuseDuguPoisonIntent (仅 dugu 师，覆盖下次出手) →
       命中 → 挂 DuguPoisonState (binds Meridian by 部位映射) →
       DuguPoisonTick 每 5 min: meridian.flow_capacity -= (cap × 1% × poisoner_tier) →
       qi_max recompute (调 cultivation 已有链路)
       自解：解蛊蕊 + 20 qi + 30% 失败
       inspect hook stub: obfuscate_sense_kind 默认透传 (无 dugu obfuscation)
       ↓ 测试饱和（毒 vs 毒蛊边界 / 经脉映射 / 解毒分支）
P1  主动遮蔽 + 部位映射 + agent narration
       DuguObfuscation impl 注入 plan-perception hook
       命中部位 → 经脉映射表（plan-combat-no_ui §3.4 联动）
       agent narration: DuguPoisonProgressEvent → "X 真元上限又少了 1"
       ↓ 饱和化 testing
P2  v1 收口
       LifeRecord 写入"X 在 Y 处灌毒蛊给 Z"事件
       身份/信誉 stub（v1 不实装 NPC 反应）
```

### A.4 v1 已知偏离正典（vN+1 必须修复）

- [ ] **NPC 厌恶反应**（worldview §十一 已正典化的"-50 baseline / 高境追杀 / 中境拒交易"）—— v1 留 hook 不实装，意味着被识破后 NPC 反应仍是默认中性
- [ ] **求医解 / 越级解**（Q48 用户要求"都要，先做自解"）—— v1 仅自解，求医路径搁置
- [ ] **多毒蛊源叠加**（同 target 被多个 dugu 师命中）—— v1 同一 target 同一 meridian 仅最新 DuguPoisonState 生效，后续叠加规则 vN+1
- [ ] **dugu 师自身阴诡色 + 经脉污染**（worldview §六:546 "自身经脉慢性侵蚀，需持续养"）—— v1 不实装 dugu 师自身代价，搁置 vN+1
- [ ] **养蛊巢 / 终焉藤**（library `末法药材十七种`）—— v1 不做养蛊系统

### A.5 v1 关键开放问题

**已闭合**（Q45-Q55）：
- Q45 reframe → 没有 fake_style 字段；走 plan-perception obfuscate_sense_kind hook
- Q47 reframe → 经脉永久 flow_capacity ↓（接 cultivation::Meridian.flow_capacity 直接修改）
- Q48 → B 自解（解蛊蕊 + 20 真元 + 30% 失败）先做；vN+1 求医 / 越级
- Q50 reframe → 飞针 = 真元凝结武器，**任何流派可用**；dugu 灌毒蛊才是流派核心
- Q51 → C 凝针即时 + 灌毒蛊异步
- Q52 → D `(flow_capacity × 1%) × poisoner_realm_tier`
- Q53 → A always-on 主动遮蔽，灌毒蛊出手时暴露 5s
- Q54 → B 部位映射经脉
- Q55 → B v1 留 hook 不实装 NPC 反应

**仍 open**（v1 实施时拍板）：
- [ ] **Q56. 解毒失败的 NearDeath 处理**：失败 = 经脉永久废 + 进入 NearDeath 30s 自救窗口？还是仅经脉废不进 NearDeath？建议**仅经脉废**（避免双重惩罚）
- [ ] **Q57. 主动遮蔽境界差识破阈值**：plan-perception 默认 Δ≥2 完全识破 / Δ=1 模糊化为 AmbientLeyline。dugu 是否调整？建议**沿用默认**（v1 不特化）
- [ ] **Q58. 部位 → 经脉映射表完整度**：worldview §六.1 列了 12 正经，plan-combat-no_ui §3.4 部位 7 档（头/胸/腹/臂/手/腿/脚）— 多对一映射规则留 P0 实装时拟（建议头→督脉 / 胸→心经 / 腹→脾经 / 臂→大肠经 / 手→肺经 / 腿→膀胱经 / 脚→肾经）
- [ ] **Q59. dugu 师自身阴诡色累积**：v1 是否接入"灌毒蛊次数 → 染色推进"？建议**搁置 vN+1**（plan-color-v1 落地后做）

---



## §0 设计轴心

- [ ] 毒蛊 = **慢性侵蚀真元上限**，不求即时威能
- [ ] 物理本质 = "本音恶意失谐"——毒蛊师主动让自己一点真元失谐附在毒草
- [ ] 末法约束：毒蛊师**自身经脉慢性侵蚀**，养蛊代价不可逆
- [ ] 社交约束：暴露 = 全陆追杀；通灵期罕见（道德底线）

## §1 第一性原理（烬灰子四论挂点）

- **音论·恶意失谐**：脏真元 = 修士主动让一点本音"失谐"，附在毒草特性上。这种失谐音入敌经脉后，**对方本音和被毒污染的失谐音会产生持续不可愈的冲突**——所以是慢性侵蚀（不像普通污染可排，毒蛊污染不消）
- **影论·镜面腐蚀**：脏真元每次扰动对方心镜都带走一点投影力 → **真元上限缓慢永久下降**（书里原话"真元上限缓慢永久下降"= 镜身被慢性磨损）
- **缚论·养蛊代价**：毒蛊师长期养"脏真元"，自己的本音也在缓慢失谐 → 自身经脉污染累积、镜身缓慢失谐
- **噬论·终焉藤**：终焉藤"烧三日不熄"——其本身已是高度有序的恶意真形，故难分解

## §2 招式 / 形态分级

| 形态 | 触发 | 持续 | 真元成本 | 战后代价 |
|---|---|---|---|---|
| **微针刺** | 1 点脏真元入敌经脉 | 6-24 小时 | 1 真元 + 1 毒针 | 自身污染 +0.01 |
| **多针埋伏** | 3-5 针布在路径 | 同上 | 多倍材料 | 同上叠加 |
| **蛊养**（长期）| 培育终焉藤毒巢 | 永久 | 持续养元 | 经脉每月 +0.05 污染 |
| **解蛊**（自救）| 强逼出他人毒蛊 | 一次性 | 自身真元 20+ | 解药消耗 |

## §3 数值幅度梯度（按境界）

```
醒灵：不能修毒蛊（无法操控失谐音）
引气：单针，脏真元 1 点，持续 6 小时，每小时扣对方真元上限 1%
凝脉：双针 / 三针，脏真元 2-3 点，持续 12 小时
固元：可培育"养蛊"，脏真元 5+ 点，持续 24 小时
通灵：罕见——通灵修士道德底线高，毒蛊修士极少能修到通灵
化虚：几乎不存在
```

**对方真元上限永久折损公式**（提案）：
```
permanent_loss = poison_qi × duration_hours × 0.5%
（1 点脏真元持续 24 小时 → 对方真元上限永久 -12%）
```

## §3.1 毒蛊·v1 规格（P0 阶段）

> worldview §五:421-426 锚定（"恶性寄生者 / 1 点脏真元 / 永久下降 / 专克高境"）+ §五 末土后招原则（**2026-05-03 正典化**："毒 vs 毒蛊"边界）。v1 收敛到**凝针基础**（任何流派可用）+ **灌毒蛊机制**（dugu 师专属）+ **经脉永久损伤 tick**（接 cultivation::Meridian.flow_capacity）+ **自解（30% 失败）**。

### 3.1.A 凝针基础（任何流派可用，QiNeedle）

**`ShootNeedleIntent`** 是普适基础能力——任何境界 ≥ 引气的修士都可用：

```rust
#[derive(Event)]
pub struct ShootNeedleIntent {
    pub shooter: Entity,
    pub dir_unit: Vec3,                   // 客户端 crosshair 方向
    pub source: IntentSource,
}
```

**实装流程**：
```
玩家发 bong:combat/shoot_needle { dir_unit } →
server 校验：
  - shooter.realm >= Realm::YinQi（醒灵不能凝针）
  - shooter.cultivation.qi_current >= 1.0
  - shooter.lifecycle 非死亡态
  - shooter.stamina >= 2 (轻型动作)
通过 →
  shooter.cultivation.qi_current -= 1.0
  shooter.stamina -= 2
  spawn QiNeedle entity:
    QiNeedle {
        qi_payload: 1.0,                  # 固定 1 点（worldview §五:422）
        qi_color: shooter.cultivation.qi_color,
        infused_dugu: false,              # 默认 false，由 InfuseDuguPoisonIntent 翻为 true
        shooter,
        spawn_pos: shooter.eye + dir_unit × 0.3,
        velocity: dir_unit × NEEDLE_SPEED (90 格/s, 比 anqi 暗器更快),
        max_distance: 50 格,
        hitbox_inflation: 0.6,
    }
emit ChargeNeedleEvent → client (轻型粒子, 不暴露)
```

**关键设计**：
- **不走 §3.2 距离衰减公式**——worldview §五:422 锚定"穿透力极强"，0-50 格全程 100% 保留
- **不复用 anqi 的 `CarrierImprint`**——凝针不需要载体物质，直接由真元凝结
- **粒子隐蔽**：client 视觉飞针极细（2 像素），不发光（区别于 anqi 暗器的可见拖尾）

### 3.1.B 灌毒蛊真元（dugu 师专属，独立异步动作）

**`InfuseDuguPoisonIntent`** 仅 dugu 师可发——**修炼条件 v1 简化**：玩家在 `Cultivation` 添加 `dugu_practice_level: u8`（v1 默认任何玩家可手动设为 1+ 即可，vN+1 接修炼系统）：

```rust
#[derive(Event)]
pub struct InfuseDuguPoisonIntent {
    pub infuser: Entity,
    pub target_carrier: InfuseTarget,      // ::NextNeedle | ::NextAttack | ::CarrierSlot(slot)
    pub source: IntentSource,
}

pub enum InfuseTarget {
    NextNeedle,                             // 下一个 ShootNeedleIntent 灌毒蛊
    NextMeleeAttack,                        // 下一次 AttackIntent 灌毒蛊（拳/剑等）
    CarrierSlot(SlotId),                    // 灌入 anqi 暗器槽位
}
```

**实装流程**：
```
玩家发 bong:combat/infuse_dugu_poison { target } →
server 校验：
  - infuser.cultivation.dugu_practice_level >= 1
  - infuser.cultivation.qi_current >= INFUSE_DUGU_COST (5 qi/次, vN+1 接修炼降低)
  - infuser.lifecycle 非死亡态
  - infuser.combat.pending_dugu_infusion 为 None（不可叠加）
通过 →
  infuser.cultivation.qi_current -= INFUSE_DUGU_COST
  infuser.combat.pending_dugu_infusion = Some(InfusionState {
      target,
      infused_at: GameTime,
      ttl: 60s,                            # 60s 内必须出手，否则失效（真元自然散去）
  })
  emit InfuseDuguEvent → client（仅 infuser 自己看到 HUD 提示）
  ⚠️ 关键：触发 5s 暴露窗口
  emit DuguObfuscationDisruptedEvent { infuser, until: GameTime + 5s }
    → plan-perception 的 obfuscate_sense_kind 在该窗口期内对此玩家透传 SenseKindV1::DuguPoison
    → 5s 内被任何施神识者 inspect → 该玩家 dugu_revealed
```

**关键设计**：
- **灌毒蛊触发即暴露 5s** —— Q53 A "always-on 主动遮蔽，但灌毒蛊出手暴露"
- **InfuseDuguPoisonIntent 与 ShootNeedleIntent 解耦** —— 玩家可"灌毒蛊后 60s 内任意出手时机"释放，是核心战术押注
- **"毒"≠"毒蛊"边界**：仅 `InfuseDuguPoisonIntent` 触发暴露与 DuguPoisonState 挂载；普通玩家点"毒性真元"特性给 contam 加 poison tag **不触发**该机制

### 3.1.C 出手命中：`DuguPoisonState` 挂载（Q54: B 部位映射）

命中事务（基于 plan-combat-no_ui §3.1.4 命中判定）触发后：

```rust
fn on_attack_resolved(
    mut commands: Commands,
    mut events: EventReader<CombatEvent>,
    attackers: Query<&Combat>,
    targets: Query<&MeridianSystem>,
    mut poison_states: Query<&mut DuguPoisonStateMap>,
) {
    for ev in events.read() {
        let CombatEvent::AttackResolved { attacker, target, body_part, .. } = ev else { continue };
        let combat = attackers.get(*attacker).ok();
        let Some(infusion) = combat.and_then(|c| c.pending_dugu_infusion.as_ref()) else { continue };

        // 1. 命中部位 → 经脉映射（Q54: B；Q58 留实装拍板，建议下表）
        let meridian_id = body_part_to_meridian(body_part);

        // 2. 校验该经脉已开
        let target_meridians = targets.get(*target).ok();
        let Some(meridian) = target_meridians.and_then(|ms| ms.get(meridian_id)) else { continue };
        if !meridian.opened { continue; }                    // 未开经脉无法挂毒蛊（药力无依附）

        // 3. 挂 DuguPoisonState（同 target 同 meridian 仅最新生效，后续覆盖）
        commands.entity(*target).insert(DuguPoisonState {
            meridian_id,
            attacker: *attacker,
            attached_at: time.now,
            poisoner_realm_tier: combat.cultivation.realm.tier(),
            loss_per_tick: meridian.flow_capacity * 0.01,    // 基础 1%
        });

        // 4. 消费 pending_dugu_infusion
        commands.entity(*attacker).insert(Combat { pending_dugu_infusion: None, .. });
    }
}
```

#### Q58 部位 → 经脉映射表（建议起手值，P0 实装时拟）

| body_part (plan-combat-no_ui §3.4) | 映射经脉 | worldview 依据 |
|---|---|---|
| Head | 督脉 | 头部统御真元 |
| Chest | 手少阴心经 | 心居胸 |
| Belly | 足太阴脾经 | 脾居腹 |
| Arm | 手阳明大肠经 | 臂偏力（worldview §六.1 手三阳偏力）|
| Hand | 手太阴肺经 | 手末为肺经起 |
| Leg | 足太阳膀胱经 | 腿偏速（足三阳偏速）|
| Foot | 足少阴肾经 | 足底为肾经起 |

**未开经脉的兜底**：v1 简化为"该次毒蛊失效"（不挂 DuguPoisonState）。vN+1 可改为"挂到最近的已开经脉"。

### 3.1.D 经脉损伤 tick + qi_max 永久下降（Q52: D）

**`DuguPoisonState` component**：

```rust
#[derive(Component, Debug, Clone, Serialize, Deserialize)]
pub struct DuguPoisonState {
    pub meridian_id: MeridianId,
    pub attacker: Entity,
    pub attached_at: GameTime,
    pub poisoner_realm_tier: u8,           // 0=醒灵 / 1=引气 / 2=凝脉 / 3=固元 / 4=通灵 / 5=化虚
    pub loss_per_tick: f64,                // 基础 = flow_capacity × 0.01
}
```

**`dugu_poison_tick` 系统（每 5 min 触发）**：

```rust
fn dugu_poison_tick(
    time: Res<GameTime>,
    mut targets: Query<(&mut MeridianSystem, &mut Cultivation, &DuguPoisonState)>,
    mut events: EventWriter<DuguPoisonProgressEvent>,
) {
    if time.now.tick_5min_boundary() {        // 仅 5 min 边界触发
        for (mut ms, mut cult, state) in targets.iter_mut() {
            let Some(meridian) = ms.get_mut(state.meridian_id) else { continue };

            // Q52: D 公式
            let actual_loss = state.loss_per_tick * (state.poisoner_realm_tier as f64).max(1.0);
            // 注意：tier=0 醒灵不能修毒蛊（worldview §五.4 line 421-426 的"通灵罕见"逻辑），
            //       v1 简化为 tier=0 折损为 0；vN+1 接修炼系统更精细

            meridian.flow_capacity -= actual_loss;
            meridian.flow_capacity = meridian.flow_capacity.max(0.0);

            // 触发 qi_max recompute（cultivation 已有链路）
            cult.recompute_qi_max(&ms);

            // 经脉废 → 解除 DuguPoisonState（毒已发尽，不再 tick）
            if meridian.flow_capacity <= 0.0 {
                meridian.opened = false;       // 经脉永久废
                // commands.remove::<DuguPoisonState>(target);  （由调用方处理）
            }

            events.send(DuguPoisonProgressEvent {
                target,
                meridian_id: state.meridian_id,
                flow_capacity_after: meridian.flow_capacity,
                qi_max_after: cult.qi_max,
                actual_loss_this_tick: actual_loss,
            });
        }
    }
}
```

#### 数值表（v1 异变兽骨参考，poisoner_tier=2 凝脉期）

| 受毒者境界 | meridian flow_capacity | 每 5 min 损耗 | 该经脉废所需 | qi_max 总损耗（境界）|
|---|---|---|---|---|
| 醒灵 (poisoner=2) | 10 | `10 × 0.01 × 2 = 0.2` | 250 min ≈ 4 小时 | -10 (该经脉容量) |
| 引气 | 30-40 | `~0.6 / 5min` | ~5 小时 | -30 |
| 凝脉 | 60-90 | `~1.2 / 5min` | ~6 小时 | -60 |
| 固元 | 130-180 | `~2.4 / 5min` | ~7 小时 | -130 |
| 通灵 | 250-350 | `~4.8 / 5min` | ~7 小时 | -250 |
| 化虚 | 500 | `~10 / 5min` | ~4 小时 | -500 |

**关键观察**：高境 target 单经脉绝对损耗大（worldview "专克高境"自然成立），但**该经脉变废所需时间相对稳定 4-7 小时**（"慢性侵蚀" narrative）。

### 3.1.E 自解机制（Q48: B 先做自解）

**`SelfAntidoteIntent`**：

```rust
#[derive(Event)]
pub struct SelfAntidoteIntent {
    pub healer: Entity,                    // 自解：healer == target
    pub target: Entity,
    pub antidote_item: ItemId,             // 必须 == bong:dugu/jiegurui (解蛊蕊)
    pub source: IntentSource,
}
```

**实装流程**（worldview §五:424 + §四:349 锚定 20 真元）：

```
玩家发 bong:cultivation/self_antidote { ... } →
server 校验：
  - healer.target 有 DuguPoisonState component
  - healer.inventory 含 解蛊蕊 ≥ 1
  - healer.cultivation.qi_current >= 20.0
  - healer.lifecycle 非死亡态
通过 →
  healer.inventory 消耗解蛊蕊 1
  healer.cultivation.qi_current -= 20.0

  # Q48: B 30% 失败概率
  if rng.random_f64() < 0.30:
    # 失败：经脉永久废
    target.meridians[poison.meridian_id].flow_capacity = 0.0
    target.meridians[poison.meridian_id].opened = false
    target.cultivation.recompute_qi_max(...)
    commands.remove::<DuguPoisonState>(target)
    emit AntidoteResultEvent { result: Failed, meridian_destroyed: poison.meridian_id }
    # Q56 设计：仅经脉废，不进 NearDeath（避免双重惩罚）
  else:
    # 成功：解除 DuguPoisonState，flow_capacity 已损失部分**不恢复**（永久）
    commands.remove::<DuguPoisonState>(target)
    emit AntidoteResultEvent { result: Success, residual_flow_capacity: target.meridians[...].flow_capacity }
```

**关键语义**：
- **解毒成功**：DuguPoisonState 移除；`flow_capacity` 已损失部分**不恢复**（这是 worldview "永久下降"的精神）
- **解毒失败**：经脉永久废 + DuguPoisonState 移除（毒已发尽）—— **不进 NearDeath**（Q56 设计避免双重惩罚）

### 3.1.F 主动遮蔽（plan-perception hook 实装，always-on）

**`DuguObfuscation` impl**（注入 plan-perception-v1.1 的 `obfuscate_sense_kind` hook）：

```rust
pub struct DuguObfuscation;

impl ObfuscateSenseKind for DuguObfuscation {
    fn obfuscate(
        &self,
        sense_entry: &SenseEntryV1,
        observer: &Cultivation,
        observed: &Cultivation,
        observed_combat: &Combat,
    ) -> SenseEntryV1 {
        // 仅对 dugu 真元修炼者生效（observed.cultivation.dugu_practice_level >= 1）
        if observed.dugu_practice_level == 0 {
            return sense_entry.clone();    // 透传，不遮蔽
        }

        // 检查暴露窗口（灌毒蛊出手 5s 内）
        let in_disrupted_window = observed_combat
            .obfuscation_disrupted_until
            .map_or(false, |t| t > now);

        if in_disrupted_window {
            return sense_entry.clone();    // 暴露窗口期，不遮蔽
        }

        // always-on 主动遮蔽逻辑（Q53: A）
        let realm_diff = observer.realm.tier() as i32 - observed.realm.tier() as i32;
        match realm_diff {
            d if d >= 2 => sense_entry.clone(),                      // Δ≥2 完全识破
            1 => SenseEntryV1 {                                       // Δ=1 模糊化
                kind: SenseKindV1::AmbientLeyline,                    // 看作普通灵脉波动
                ..sense_entry.clone()
            },
            _ => SenseEntryV1::masked(),                              // Δ≤0 完全屏蔽
        }
    }
}
```

**实装时点**：
- v1 P0 仅 stub（`DuguObfuscation` 默认 enable，但 `obfuscation_disrupted_until` 字段写入但不影响实际行为）
- v1 P1 完整接入 plan-perception hook + 暴露窗口期 5s 真正生效

**关键语义**：
- 普通玩家无神识 → 永远看不到任何 dugu 师 SenseEntry（plan-perception 已有逻辑）
- 高境玩家施神识 + 与 dugu 师境界差 ≥ 2 → 完全识破
- dugu 师灌毒蛊出手 5s 内 → obfuscation 失效，被任何施神识者识破

### 3.1.G 反噬与失败 / NPC stub（Q55: B v1 不实装 NPC 反应）

#### 3.1.G.1 dugu 师自身代价（v1 简化版）

worldview §五:422 + §六:546 锚定 dugu 师"自身经脉慢性侵蚀，需持续养"——v1 不实装染色养成系统，仅做最简化代价：

- 每次 `InfuseDuguPoisonIntent` 消耗 5 qi（无法回收）
- 每次 `ShootNeedleIntent` 消耗 1 qi（无法回收）
- v1 不做 dugu 师自身经脉污染（vN+1 plan-color 落地后接入）

#### 3.1.G.2 NPC 反应 hook（v1 仅留 stub）

worldview §十一 已正典化"毒蛊师社会默认（-50 baseline / 高境追杀 / 中境拒交易）"——v1 不实装 NPC 反应。预留 stub：

```rust
// dugu_revealed 事件（v1 仅 emit，无 consumer）
#[derive(Event, Serialize)]
pub struct DuguRevealedEvent {
    pub revealed_player: Entity,
    pub witness: Entity,                   // 谁识破的
    pub witness_realm: Realm,              // NPC vN+1 反应分级
    pub at_position: Vec3,
    pub at_time: GameTime,
}
```

vN+1 plan-identity-v1 / plan-baomai 落地后：
- 添加 consumer：DuguRevealedEvent → identity 信誉度 -X / NPC 间传话概率 / 高境 NPC 主动追杀

#### 3.1.G.3 失败模式

| 失败模式 | 处理 |
|---|---|
| 灌毒蛊真元 60s 内未出手 | `pending_dugu_infusion = None`；qi 已扣不退 |
| 命中经脉未开 | 灌毒蛊真元浪费（pending 消费但无 DuguPoisonState 挂载）|
| 自解失败 | 经脉永久废（Q48: B）|
| 多次 InfuseDuguPoisonIntent 同 target | 仅最新覆盖，前一个 pending 失效 |

#### 3.1.G.4 与 stamina / qi 池耦合

- `ShootNeedleIntent`：1 qi + 2 stamina
- `InfuseDuguPoisonIntent`：5 qi + 0 stamina（静态修炼）
- `SelfAntidoteIntent`：20 qi + 0 stamina

---

## §4 材料 / 资源链

| 阶段 | 材料 | library 来源 | 用途 |
|---|---|---|---|
| 基础原料 | **噬脉根**（负灵域浅层剧毒）| ecology-0002（稀见五味）| 单针毒料 |
| 高阶原料 | **终焉藤** | ecology-0002（毒性五味·烧三日不熄）| 养蛊巢 |
| 载体 | 异变兽爪 | peoples-0005 | 毒针实体 |
| 解药基础 | **解蛊蕊**（幽暗地穴紫花，3 骨币 / 颗）| ecology-0002 | 解蛊丹 |
| 中和染色 | 清浊草 | ecology-0002（专克染色浊乱者）| 自身阴诡色失控时使用 |

**采集风险**：
- 噬脉根采集需进负灵域 → 涡流装备 / 抗灵压丹辅助
- 终焉藤"养藤者多自身先死"（书里原话）

## §5 触发 / 流程

```
养蛊：终焉藤巢 → 持续注入失谐真元 → 产出毒针（1 颗 / 24h）
出招：毒针抛射 → 命中 → 脏真元入敌经脉 →
  对方 contamination_special_dugu = 1（不可被普通排异 tick 中和）→
  逐小时扣对方 max_qi
对方解药：解蛊蕊煎服 → 强消耗自身真元 20+ → 解除
养蛊代价：每月自身经脉污染 +0.05（持续养蛊不可逆）
```

## §6 反噬 / 失败代价

- [ ] 自身经脉慢性污染（每月 +0.05），不可逆累积
- [ ] 阴诡色染色，inspect 一眼可见 → **暴露 = 全陆追杀**（worldview / 战斗流派源流原话）
- [ ] 终焉藤养蛊失败 = 反噬自身污染 +0.5
- [ ] 解蛊药材稀缺，自身遭蛊解不出会死
- [ ] 道德路径：通灵期天道注意力上升，毒蛊师极易被劫

## §7 克制关系

- **克**：高境对手（专攻真元上限——境越高越亏）；持久战 / 追踪战
- **被克**：解蛊蕊普及 → 毒蛊难成（社交对抗）；近战流（爆脉/震爆 三掌之内拍死）
- **反向克涡流流**：为一点毒开黑洞，涡流自身反噬代价更大（worldview "毒蛊→涡流"）
- **染色亲和**：阴诡色（毒蛊流原生匹配，加成幅度待定）
- **错配**：温润色（丹师）走毒蛊——本音中和性强，养不出"恶意失谐"

## §8 数据契约

### v1 P0 落地清单（按 §3.1 规格）

| 模块 | 文件路径 | 核心内容 |
|---|---|---|
| QiNeedle component | `server/src/combat/needle.rs` | `QiNeedle` component / `ShootNeedleIntent` event / `needle_tick_system` |
| Dugu state | `server/src/cultivation/dugu.rs` | `DuguPoisonState` component / `InfuseDuguPoisonIntent` event / `Cultivation.dugu_practice_level` 字段 / `Combat.pending_dugu_infusion` 字段 |
| Poison tick | `server/src/cultivation/dugu.rs` | `dugu_poison_tick` system（按 `attached_at_tick` 每 5 min 触发 → flow_capacity 永久 ↓ → qi_max recompute） |
| 部位映射 | `server/src/combat/dugu_mapping.rs` | `body_part_to_meridian` 函数（Q58 起手值表）+ `on_attack_resolved_dugu_handler` |
| 自解 | `server/src/cultivation/dugu.rs` | `SelfAntidoteIntent` event / `resolve_self_antidote_intent` 系统 / 30% 失败概率 |
| 主动遮蔽 stub | `server/src/combat/stealth.rs` | `DuguObfuscation` impl（v1 stub，P1 接 plan-perception hook） |
| Item registry | `server/assets/items/core.toml` / `server/src/botany/registry.rs` | `jie_gu_rui` (解蛊蕊) |
| Combat event 扩展 | `server/src/combat/events.rs` | `DuguPoisonProgressEvent` / `DuguObfuscationDisruptedEvent` / `DuguRevealedEvent` (stub) / `AntidoteResultEvent` |
| Schema (TS) | `agent/packages/schema/src/dugu.ts` | `DuguPoisonStateV1` / `DuguPoisonProgressEventV1` / `DuguObfuscationStateV1` / `AntidoteResultEventV1` / `DuguRevealedEventV1` |
| Inbound packets | `client/.../net/DuguPackets.java` | `bong:combat/shoot_needle` / `bong:combat/infuse_dugu_poison` / `bong:cultivation/self_antidote` |
| Outbound packets | `client/.../net/DuguPackets.java` | `bong:cultivation/dugu_poison_state`（仅自己可见自己受毒情况） |
| Client HUD | `client/.../hud/DuguHud.java` | 自身受毒经脉 + 每 5 min 损耗预览 + 灌毒蛊 pending 提示 + 5s 暴露窗口倒计时 |

### v1 P1 落地清单

| 模块 | 文件路径 | 核心内容 |
|---|---|---|
| 主动遮蔽 impl | `server/src/combat/stealth.rs` | `DuguObfuscation` 完整接入 plan-perception 的 `obfuscate_sense_kind` hook |
| 部位映射完善 | `server/src/combat/dugu_mapping.rs` | Q58 完整 7 档部位 → 经脉表（Head/Chest/Belly/Arm/Hand/Leg/Foot）+ 未开经脉兜底 |
| Agent narration | `agent/packages/tiandao/src/dugu-narration.ts` | `DuguPoisonProgressEventV1` → "X 真元上限又少了 1 / X 在街上踉跄" |
| 单测饱和 | `server/src/cultivation/dugu_tests.rs` | 所有命中/部位/解毒分支 + 毒 vs 毒蛊边界 + 暴露窗口 + 越级公式 |

### v1 P2 落地清单

| 模块 | 文件路径 | 核心内容 |
|---|---|---|
| LifeRecord | `server/src/lore/life_record.rs` | "X 在 Y 处灌毒蛊给 Z"事件类型 |
| 身份系统 stub | `server/src/identity/stub.rs` | `DuguRevealedEvent` 写入 stub（无实际反应；vN+1 plan-identity-v1 接 consumer） |

## §9 实施节点

详见 §A.3 v1 实施阶梯。三阶段总结：

- [ ] **P0** 凝针 + 灌毒蛊 + 经脉损伤 + 自解（B 30% 失败）+ 部位映射 + inspect hook stub —— 见 §3.1
- [ ] **P1** 主动遮蔽完整接入 + agent narration + 部位映射完善 —— 见 §3.1.F + §8 P1 清单
- [ ] **P2** v1 收口（LifeRecord + 身份系统 stub） —— 见 §8 P2 清单

## §10 开放问题

### 已闭合（2026-05-03 拍板，11 个决策）

- [x] **Q45** reframe → 没有 fake_style 字段；走 plan-perception `obfuscate_sense_kind` hook
- [x] **Q47** reframe → 直接接 cultivation::Meridian.flow_capacity 永久 ↓
- [x] **Q48** → B 自解（解蛊蕊 + 20 真元 + 30% 失败概率）先做；vN+1 求医解 + 越级解
- [x] **Q50** reframe → 凝针是普适能力，灌毒蛊是 dugu 专属
- [x] **Q51** → C 凝针即时 + 灌毒蛊异步（`InfuseDuguPoisonIntent` 60s 内附着任意出手）
- [x] **Q52** → D `(flow_capacity × 1%) × poisoner_realm_tier`（高境毒蛊师毒更烈）
- [x] **Q53** → A always-on 主动遮蔽，灌毒蛊出手时暴露 5s
- [x] **Q54** → B 部位映射经脉（Q58 表 P0 实装时拟）
- [x] **Q55** → B v1 留 hook 不实装（`DuguRevealedEvent` 仅 emit，无 consumer）

### 仍 open（v1 实施时拍板）

- [ ] **Q56. 自解失败的 NearDeath 处理**：建议**仅经脉废，不进 NearDeath**（避免双重惩罚）—— P0 实装时确认
- [ ] **Q57. 主动遮蔽境界差识破阈值**：建议**沿用 plan-perception 默认**（Δ≥2 完全识破 / Δ=1 模糊化 AmbientLeyline）—— P1 实装时拟
- [ ] **Q58. 部位 → 经脉映射表**：见 §3.1.C 起手表（Head→督脉 / Chest→心经 / Belly→脾经 / Arm→大肠经 / Hand→肺经 / Leg→膀胱经 / Foot→肾经）—— P0 实装时确认
- [ ] **Q59. dugu 师自身阴诡色累积**：v1 不实装，搁置 vN+1 plan-color-v1 落地后接入

### vN+1 留待问题（plan-dugu-v2 时拍）

- [ ] 求医解（接 plan-baomai 的 NPC 服务 + 50% 成功率）
- [ ] 越级解（被毒者境界与毒蛊师境界差影响解毒成功率）
- [ ] 多个毒蛊源叠加（同 target 多个 meridian / 同 meridian 多个 attacker）
- [ ] 终焉藤养蛊巢（library `末法药材十七种` 蛊养体系）
- [ ] 完整 NPC 信誉度反应（接 plan-identity-v1）
  - [ ] 高境 NPC 神识识破 → 主动追杀
  - [ ] 中境 NPC 拒绝交易 + 传话
  - [ ] 玩家主动卖情报机制
- [ ] 自身阴诡色 + 经脉慢性污染（接 plan-color-v1）
- [ ] 通灵期"道德底线"机制化（天劫加大 / NPC 阵营惩罚）
- [ ] 解蛊蕊全服稀缺度调价（毒蛊不至于被解药普及废掉）

## §11 进度日志

- 2026-04-26：骨架创建。依赖 plan-cultivation-v1 污染机制 + plan-alchemy-v1 反向炼丹路径 + plan-social-v1 暴露追杀社交事件。无对应详写功法书。
- 2026-05-03：从 skeleton 升 active。worldview 同步正典化"末土后招原则"+ "身份与信誉"+ "毒 vs 毒蛊边界"（commit fe00532c）。§A 概览 + §3.1 P0 毒蛊·v1 规格落地（11 个决策点闭环 Q45-Q55，4 个 v1 实装时拍板 Q56-Q59）。primary axis = 毒素真元累积速率 + 解毒难度（worldview §五:464）。**v1 关键设计**：凝针是普适能力（任何流派可用），灌毒蛊真元才是 dugu 专属（触发 5s 暴露窗口）。直接接 cultivation::Meridian.flow_capacity 字段实现"永久 qi_max 下降"。直接复用 plan-perception-v1.1 已留的 `obfuscate_sense_kind` hook 实装主动遮蔽。与 plan-anqi-v1 / plan-zhenfa-v1 共享凝针/暗器载具，但 dugu 灌毒蛊路径独立（"毒 vs 毒蛊"边界）。

## Finish Evidence

### 落地清单

- P0 凝针 / 灌毒蛊 / 经脉损伤 / 自解闭环：`server/src/combat/needle.rs`、`server/src/cultivation/dugu.rs`、`server/src/cultivation/mod.rs`，新增 `ShootNeedleIntent`、`QiNeedle`、`InfuseDuguPoisonIntent`、`PendingDuguInfusion`、`DuguPoisonState`、`dugu_poison_tick`、`SelfAntidoteIntent`，并接入 `SkillRegistry` / `known_techniques` hotbar 技能。
- Review 修复收口：`dugu_poison_tick` 改为按 `attached_at_tick` 满间隔触发，`actual_loss_this_tick` 只上报真实扣减量；`can_infuse_dugu` 显式要求 `Realm::Induce`；hotbar 凝针缺少 target 时拒绝为 `InvalidTarget`，不扣资源、不发事件。
- P0/P1 命中与遮蔽接入：`server/src/combat/events.rs` 增加 `AttackSource`；`server/src/combat/resolve.rs` 透传攻击来源；`server/src/cultivation/spiritual_sense/scanner.rs`、`server/src/cultivation/spiritual_sense/push.rs` 接入 `DuguPractice` + `DuguObfuscationDisrupted`，同境/近境遮蔽为 `AmbientLeyline`，高两境界差或暴露窗口透传识破。
- P1 agent narration：`server/src/network/dugu_event_bridge.rs`、`server/src/network/redis_bridge.rs`、`agent/packages/tiandao/src/dugu-narration.ts`、`agent/packages/tiandao/src/skills/dugu.md` 将 `DuguPoisonProgressEventV1` 发布到 `bong:dugu/poison_progress` 并生成玩家叙事。
- P0/P1 client HUD contract：`server/src/network/dugu_state_emit.rs`、`agent/packages/schema/src/dugu.ts`、`agent/packages/schema/src/server-data.ts`、`client/src/main/java/com/bong/client/combat/handler/DuguPoisonStateHandler.java`、`client/src/main/java/com/bong/client/combat/store/DuguPoisonStateStore.java` 接通 `dugu_poison_state` 自身可见状态；`client/src/main/java/com/bong/client/network/ClientRequestProtocol.java`、`ClientRequestSender.java` 接通 `self_antidote`。
- P2 LifeRecord / biography / persistence hook：`server/src/cultivation/life_record.rs` 新增 `BiographyEntry::DuguPoisonInflicted`；`server/src/persistence/mod.rs`、`server/src/schema/server_data.rs`、`agent/packages/schema/src/biography.ts`、generated schema 同步 Dugu biography 事件。

### 关键 commit

- `edc6d428`（2026-05-04）`plan-dugu-v1: 落地毒蛊 server 闭环`
- `cf4ed1ac`（2026-05-04）`plan-dugu-v1: 接通毒蛊 schema 与天道叙事`
- `5378aed0`（2026-05-04）`plan-dugu-v1: 接通毒蛊客户端状态与自解请求`
- `2c5db341`（2026-05-04）`fix(plan-dugu-v1): 修正毒蛊 tick 与凝针目标校验`

### 测试结果

- `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`：2287 passed
- `cd server && cargo test cultivation::dugu`：11 passed
- `cd server && cargo test network::dugu_event_bridge`：1 passed
- `cd agent && npm run build`
- `cd agent/packages/tiandao && npm test`：236 passed
- `cd agent/packages/schema && npm test`：268 passed（含 generated artifacts freshness）
- `cd client && JAVA_HOME="$HOME/.sdkman/candidates/java/17.0.18-amzn" PATH="$HOME/.sdkman/candidates/java/17.0.18-amzn/bin:$PATH" ./gradlew test build`：BUILD SUCCESSFUL
- `git diff --check`

### 跨仓库核验

- server：`QiNeedle`、`ShootNeedleIntent`、`InfuseDuguPoisonIntent`、`DuguPoisonState`、`DuguPoisonProgressEvent`、`SelfAntidoteIntent`、`AttackSource::QiNeedle`、`CH_DUGU_POISON_PROGRESS`
- schema/agent：`DuguPoisonStateV1`、`DuguPoisonProgressEventV1`、`DuguObfuscationStateV1`、`AntidoteResultEventV1`、`CHANNELS.DUGU_POISON_PROGRESS`、`DuguNarrationRuntime`
- client：`encodeSelfAntidote`、`sendSelfAntidote`、`DuguPoisonStateHandler`、`DuguPoisonStateStore`、`ServerDataRouter` 注册 `dugu_poison_state`

### 遗留 / 后续

- NPC 信誉度、求医解、越级解、多毒源叠加、毒针材料变种、毒蛊师自身阴诡色 / 经脉慢性污染仍按本 plan 的 vN+1 范围留给 `plan-dugu-v2` / `plan-identity-v1` / `plan-color-v1`。
