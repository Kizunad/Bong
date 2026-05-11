# plan-alchemy-combat-v1：战场丹药十方（肢体恢复 / 部位硬化 / 移速 / 体力）

> 炼丹体系从"修炼辅助"扩展到"战场消耗品"。10 颗**凡物丹药**覆盖四类战斗需求，**每颗都带负面副作用**——末法残土没有免费午餐。
>
> **凡物丹药定位**：原料皆为凡间草药、兽骨、矿粉——不含灵石、兽核等高阶灵材。药力作用于**肉身层面**（伤口、体力、筋骨），不触及经脉/真元池本身。因此**前三境（醒灵/引气/凝脉）满效，固元过渡衰减，通灵/化虚断崖式失效**——高境修士的肉身已被真元重塑，凡物药力如隔靴搔痒。

## 阶段总览

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | StatusEffectKind 扩展 + 伤口恢复通道 + 部位抗击打框架 | ⬜ |
| P1 | 10 颗丹药 server 实现（item.toml + recipe JSON + effect handler） | ⬜ |
| P2 | HUD 状态栏接入（buff/debuff 图标 + 部位高亮 + 负面闪烁） | ⬜ |
| P3a | Item icon 生成（10 张 64×64 PNG，gen.py --style item） | ⬜ |
| P3b | 服用动画（10 份 PlayerAnimator JSON，各自独立骨骼姿态） | ⬜ |
| P3c | 粒子效果（10 张粒子贴图 + 10 个 VfxPlayer + VfxBootstrap 注册） | ⬜ |
| P3d | 音效 recipe（10 份 audio_recipes JSON，vanilla 音效分层组合） | ⬜ |
| P4 | 饱和测试（10 颗 × 正面 + 负面 + 叠加冲突 + shelflife 腐败） | ⬜ |

---

## 接入面

### 进料

- `alchemy::Recipe` — 10 份新配方 JSON，走现有 `recipe.rs` 加载管线
- `alchemy::PillEffect` — 扩展 effect kind，新增 `wound_heal` / `body_part_resistance` / `speed_boost` / `stamina_boost`
- `combat::Wounds` — 读取伤口等级，写入恢复结果
- `combat::StatusEffects` — 注入正面 buff + 负面 debuff（双 `ApplyStatusEffectIntent`）
- `combat::Stamina` — 读写体力上限 / 恢复速率
- `cultivation::Cultivation` — 读 qi_current（部分负面效果持续抽真元）
- `shelflife` — 10 颗丹药各自注册 `DecayProfile`（腐败后负面加重）
- `botany::PlantRegistry` + `mineral::MineralRegistry` + `fauna::FaunaTag` — 配方原料

### 出料

- `combat::StatusEffects` — 写入 buff/debuff → 同步到客户端 `StatusSnapshotHandler`
- `combat::Wounds` — 伤口等级变更 → 触发 `InventorySnapshotHandler` 同步到客户端 `MiniBodyHudPlanner`
- `combat::DerivedAttrs` — move_speed_multiplier / defense_power 变更
- `alchemy::PillConsumeOutcome` — 消耗结果（含毒素注入 `Contamination`）
- `network::StatusSnapshotHandler` → 客户端 `StatusEffectHudPlanner`（buff/debuff 8 栏显示）
- `skill::SkillXpGain` — 炼制经验
- `narrative` — agent narration 可感知（"某人服了断续散，断臂竟在愈合……"）

### 共享类型 / event

- 复用 `StatusEffectKind` — 新增 7 个变体（见 P0），不另建 enum
- 复用 `ApplyStatusEffectIntent` — 正面负面各一条 intent
- 复用 `PillConsumeOutcome` — 扩展消耗结果类型
- 复用 `WoundLevel` / `BodyPart` — 伤口操作走现有 `Wounds` 组件方法
- **不新建** event / component，全部接现有管线

### 跨仓库契约

| 层 | 新增 symbol |
|----|------------|
| server | `StatusEffectKind::WoundHeal` / `BodyPartResist` / `SpeedBoost` / `StaminaBoost` / `StaminaCrash` / `LegStrain` / `QiDrainForStamina` |
| server | `pill.rs` 新增 `apply_wound_heal()` / `apply_body_part_resist()` 消耗处理函数 |
| server | `assets/items/pills.toml` 10 条 `[[item]]` |
| server | `assets/alchemy/recipes/` 10 份 `*_v1.json` |
| client | `StatusEffectStore` 扩展 7 个 kind → displayName + icon tint + tooltip |
| client | `MiniBodyHudPlanner` 扩展：部位硬化时该部位 dot 额外画蓝框 |
| client | `bong-client/textures/gui/items/` 10 张丹药图标 PNG（64×64，gen.py --style item） |
| client | `bong/player_animation/` 10 份服用动画 JSON（pill_huo_xue ~ pill_hu_gu） |
| client | `bong/textures/particle/` 10 张粒子贴图 + `bong/particles/` 10 份粒子注册 JSON |
| client | `bong/audio_recipes/` 10 份音效 recipe JSON（pill_*_consume） |
| client | `visual/particle/alchemy/` 10 个 VfxPlayer 类 + VfxBootstrap 10 条注册 |
| client | `BongAnimations.java` 10 个动画 Identifier + `BongParticles.java` 10 个粒子类型 |
| agent | 无新增 schema（丹药消耗是本地事件，不走 Redis IPC） |

### worldview 锚点

- §三 真元（不是蓝条，是命）：负面效果涉及真元流失 / qi_max 永降
- §三 境界本质是经脉拓扑变化 + 真元池倍率累乘：醒灵 10 → 化虚 10700 的 1074 倍膨胀是"凡物丹药对高境无效"的物理依据——凡草药分子无法渗透真元重塑后的灵肌
- §四 战力分层（体表 16 部位 × 6 档伤口）：伤口恢复 / 部位硬化直接操作此层
- §四 越级原则"境界差距是物理事实"：凡丹衰减曲线是同一物理的消费品表达——低境凡丹对高境无意义，如同§四"醒灵对化虚连一掌都扛不了"
- §九 经济与交易：丹药是高价值消耗品，半衰期逼迫流转；凡丹市场被自然限定在低境经济圈
- §十 资源与匮乏："万物皆有成本"——每颗丹的负面效果是 worldview 的直接体现
- §十五 设计哲学 #1："强大不是终点，是新的危险的开始"——高境修士失去凡丹依赖，必须找到替代方案
- §十五 设计哲学 #7："战斗是真元汇率兑换"——战场丹药是兑换的加速器
- §十五 设计哲学 #8："碰一下物品都有代价"——服药即有成本

### qi_physics 锚点

- **真元流失类负面效果**：回力丹的 qi_drain 2.0/s 走 `qi_physics::ledger::QiTransfer { from: player, to: zone, amount }`——真元归还环境，不凭空消失
- **qi_max 永降**：断续散的 -3% 走 `qi_physics` 的 `qi_cap_adjust`（与现有 `QiCapPermMinus` 同源）
- 不新增物理常数，不自定衰减公式

---

## 凡物丹药·境界衰减

### 物理依据

凡物丹药的药力作用于**肉身**——加速血液循环、强化筋膜、刺激肌腱——本质是把草药的化学物质灌入凡人级别的生物体。

问题在于：修士的肉身随境界**不可逆地真元化**。引气期的肌肉还是 70% 凡肉 + 30% 真元浸润；到了通灵期，肌纤维已被真元完全重铸为"灵肌"——凡草药的分子对灵肌就像盐水泼钢板，渗不进去。

> "固元以上的修士嘲笑凡丹，不是因为高傲。是因为他们的血管里已经不流血了。"

### 衰减曲线

| 境界 | 正面效果倍率 | 负面效果倍率 | 设计意图 |
|------|------------|------------|---------|
| 醒灵（0） | **×1.0**（满效） | ×1.0 | 凡物丹药的黄金用户 |
| 引气（1） | **×1.0**（满效） | ×1.0 | 仍是凡躯，药力完整 |
| 凝脉（2） | **×1.0**（满效） | ×1.0 | 经脉成环但肉身未质变 |
| 固元（3） | **×0.5**（半效） | ×0.8 | 真元核成形，肉身开始质变；药力减半但副作用仍在八成 |
| 通灵（4） | **×0.15**（残效） | ×0.6 | 灵肌已成，凡药几乎无用；副作用因代谢加速也减弱 |
| 化虚（5） | **×0.05**（聊胜于无） | ×0.4 | 肉身已非凡物，活血丹对化虚老怪相当于喝水 |

### 关键设计决策

**正面衰减 > 负面衰减**：高境修士吃凡丹不仅没用，还要承受大部分副作用——这不是 bug 而是 feature。药力进不去灵肌（正面衰减猛），但药毒仍在血液循环里作祟（负面衰减慢）。**固元修士吃活血丹：伤口只好一半，出血加速照样 ×2.0**。这自然劝退高境修士使用凡丹，让稀缺资源留给真正需要的低境散修。

**断续散（#3）的特殊处理**：断续散的 qi_max 永降效果不受境界衰减——这是对真元池本身的操作，不是对肉身的操作。永久代价不打折，但 SEVERED → FRACTURE 的恢复效果按境界衰减（通灵修士用断续散：花了 3% qi_max，断肢只恢复到 FRACTURE 和 SEVERED 之间的某个中间态，需要额外手段补完）。

**HUD 提示**：固元+修士服用凡丹时，事件流显示橙色警告："凡药药力不足，仅恢复 [X]%。"——明确告知衰减比例，不让玩家误以为 bug。

### 代码实现（P0 扩展）

```rust
// server/src/alchemy/pill.rs — 新增
pub fn mortal_pill_realm_scale(realm: Realm) -> (f32, f32) {
    // 返回 (positive_scale, negative_scale)
    match realm {
        Realm::Awaken | Realm::Induce | Realm::Condense => (1.0, 1.0),
        Realm::Solidify => (0.5, 0.8),
        Realm::Spirit => (0.15, 0.6),
        Realm::Void => (0.05, 0.4),
    }
}

// consume_pill() 内调用：
// let (pos_scale, neg_scale) = mortal_pill_realm_scale(cultivation.realm);
// wound_heal_grades = (base_grades as f32 * pos_scale).round() as u8;  // 可能 round 到 0
// bleeding_multiplier = base_bleed_mult * neg_scale;  // 负面仍在
```

### 衰减作用方式（逐丹药）

| # | 丹药 | 正面效果衰减方式 | 负面效果衰减方式 |
|---|------|----------------|----------------|
| 1 | 活血丹 | 恢复等级数 × pos_scale（round，通灵时 1×0.15=0 → 无效） | 出血倍率 × neg_scale |
| 2 | 续骨膏 | 恢复等级数 × pos_scale | 脆弱系数 × neg_scale；移速减免 × neg_scale |
| 3 | 断续散 | SEVERED 恢复概率 × pos_scale（通灵=15%概率成功）；**qi_max 永降不衰减** | 行动限制时长 × neg_scale |
| 4 | 铁壁散 | 减免百分比 × pos_scale（通灵: 40%×0.15=6%） | 四肢脆弱 × neg_scale |
| 5 | 金钟丹 | 减伤百分比 × pos_scale | 真元恢复停止时长 × neg_scale |
| 6 | 凝甲散 | 减免百分比 × pos_scale | 功能系数降幅 × neg_scale |
| 7 | 疾风丹 | 移速加成 × pos_scale（通灵: 35%×0.15=5%） | 体力消耗倍率 × neg_scale；Exhausted 时长 × neg_scale |
| 8 | 缩地散 | 移速加成 × pos_scale | 腿伤等级 × neg_scale（round，通灵时 1×0.6=1 → 仍受伤） |
| 9 | 回力丹 | 恢复倍率 × pos_scale | qi_drain 速率 × neg_scale |
| 10 | 虎骨散 | 体力上限增幅 × pos_scale；即时恢复比例 × pos_scale | 虚脱时长 × neg_scale；上限降幅 × neg_scale |

### 战术含义

- **前三境（醒灵/引气/凝脉）**：凡丹是战场必需品。散修背包里永远有几颗活血丹和疾风丹，它们是低境修士弥补真元池不足的核心手段。
- **固元（过渡期）**：凡丹药效减半，开始不划算。固元修士要学会不依赖凡丹——靠经脉流量和真元池本身抗伤。这是"凡→仙"的心理断奶期。
- **通灵/化虚（断崖）**：凡丹基本无用。活血丹恢复 0 级伤口（取整为零），疾风丹加速 5%（聊胜于无）。高境修士的战场恢复手段来自**经脉循环自愈**（真元灌注创口）、**高阶灵丹**（未来 plan，需灵石/兽核等高阶灵材）、或**流派特性**（温润色自疗、截脉流自我中和）。凡丹市场被自然限定在低境经济圈内。

> 这创造了末法残土特有的"丹药鄙视链"：通灵老怪不屑凡丹，却可能在坍缩渊深层捡到一颗引气散修留下的活血丹——对他无用，但可以丢给身边的低境搬运工续命。**凡丹是低境的命，高境的零钱。**

---

## 丹药清单（10 颗）

### A. 肢体伤口恢复（3 颗）

---

#### 1. 活血丹（Blood-Quickening Pill）

> "血行则伤退。只是血行太快时，你最好别再挨刀了。"

**定位**：廉价广谱疗伤丹，散修随身携带的基础消耗品。

| 维度 | 值 |
|------|-----|
| 正面效果 | 全身所有部位伤口等级 -1（如 LACERATION → ABRASION），不影响 SEVERED |
| 负面效果 | `Bleeding` 加重：60s 内所有伤口出血速率 ×2.5 |
| 持续时间 | 正面即时生效；负面 60s |
| 染色 | Gentle（柔） |
| 毒素量 | 0.15 |
| 稀有度 | common |
| 炉阶 | 1 |
| 配方原料 | 止血藤 ×2 + 活血草 ×1 |
| 服用时间 | 1.5s |
| 冷却 | 30s |

**战术博弈**：伤口降级很好，但如果你刚服完又挨了一刀，加速出血可能比原伤更致命。最佳时机是脱离战斗后服用。

**腐败加重**：过期活血丹出血速率 ×4.0（血液不凝反而更稀），强制服用可能直接 Bleeding 致死。

---

#### 2. 续骨膏（Bone-Mending Paste）

> "骨头接上了，但接骨的半刻钟里，你连苍蝇都扛不住。"

**定位**：定向修复单个部位的重伤（FRACTURE → BRUISE），野外急救用。

| 维度 | 值 |
|------|-----|
| 正面效果 | 指定一个 `BodyPart`，该部位伤口等级 -2（如 FRACTURE → BRUISE） |
| 负面效果 | 该部位 120s 内功能系数额外 -0.3（修复期脆弱）+ 全身移速 -15% |
| 持续时间 | 正面即时；负面 120s |
| 染色 | Solid（实） |
| 毒素量 | 0.25 |
| 稀有度 | uncommon |
| 炉阶 | 2 |
| 配方原料 | 白玉骨粉 ×1 + 续断草 ×1 + 兽骨胶 ×1 |
| 服用时间 | 3.0s（外敷，需要长操作窗口） |
| 冷却 | 120s |

**战术博弈**：3 秒服用时间意味着战斗中几乎不可能使用，必须找掩护。120s 修复期内该部位反而更脆——急于归队会付出更大代价。

**腐败加重**：过期续骨膏修复期延长到 300s，且有 30% 概率修复失败（伤口不变但毒素照吃）。

---

#### 3. 断续散（Severed-Mending Powder）

> "天道不许断肢再生。你逆天而行，代价是命本身。"

**定位**：唯一能从 SEVERED 恢复的丹药，极端稀有，代价永久。

| 维度 | 值 |
|------|-----|
| 正面效果 | 指定一个 SEVERED 部位 → 恢复到 FRACTURE（仍需续骨膏 / 自然愈合继续恢复） |
| 负面效果 | **永久** qi_max -3% + 服用后 300s 全身移速 -50% + 300s 内不可战斗（Stunned） |
| 持续时间 | 正面即时；永久 qi_max 损失 + 300s 行动限制 |
| 染色 | Turbid（浊） |
| 毒素量 | 0.80（极高） |
| 稀有度 | epic |
| 炉阶 | 3 |
| 配方原料 | 千年骨参 ×1 + 活化兽核碎片 ×1 + 灵泉水 ×2 + 续断草 ×2 |
| 服用时间 | 5.0s |
| 冷却 | 600s（10 分钟） |

**战术博弈**：断臂可以接回来，但永久丢 3% 真元上限——对高境修士来说代价极重（化虚 qi_max 10700 的 3% = 321 点）。5 分钟的行动限制意味着你必须在绝对安全的地方服用。多次使用累计扣 qi_max 会越来越致命。

**腐败加重**：过期断续散 qi_max 永降 6%，且 50% 概率恢复失败（SEVERED 不变，毒素和 qi_max 照扣）。

---

### B. 部位抗击打增强（3 颗）

---

#### 4. 铁壁散（Iron Wall Powder）

> "真元凝于胸腹如铁壁。只是铁壁之外，你赤裸如婴。"

**定位**：躯干专防丹，适合正面硬刚时护住要害。

| 维度 | 值 |
|------|-----|
| 正面效果 | Chest + Abdomen 伤害减免 +40%，持续 90s |
| 负面效果 | 四肢（ArmL / ArmR / LegL / LegR）伤害减免 -25%，持续同期 90s |
| 持续时间 | 90s |
| 染色 | Heavy（厚） |
| 毒素量 | 0.30 |
| 稀有度 | uncommon |
| 炉阶 | 2 |
| 配方原料 | 铁砂草 ×2 + 重土 ×1 + 异兽甲片 ×1 |
| 服用时间 | 2.0s |
| 冷却 | 120s |

**战术博弈**：躯干结实了但四肢更脆。暗器流可以瞄你手臂打废武器手，截脉流可以集中攻击你的腿让你跑不掉。正面硬刚有利，被侧翼包抄就是灾难。

**HUD 表现**：MiniBodyHudPlanner 躯干部位画蓝色加固框（2px），四肢部位画红色虚线框（脆弱警示）。

---

#### 5. 金钟丹（Golden Bell Pill）

> "金钟一响，刀枪不入。只是钟声散去后，你连呼吸都觉得累。"

**定位**：全身短爆发硬化，用于扛过关键一击或撤离窗口。

| 维度 | 值 |
|------|-----|
| 正面效果 | 全身 `DamageReduction` +30%，持续 30s |
| 负面效果 | 真元恢复速率 -100%（完全停止），持续 180s（远超 buff 时长） |
| 持续时间 | 正面 30s；负面 180s |
| 染色 | Heavy（厚） |
| 毒素量 | 0.45 |
| 稀有度 | rare |
| 炉阶 | 2 |
| 配方原料 | 金铃果 ×1 + 铁砂草 ×1 + 固元草 ×2 |
| 服用时间 | 1.0s（紧急服用） |
| 冷却 | 300s |

**战术博弈**：30 秒无敌感，但之后 3 分钟真元不回——你扛住了一波，但真元用完就真的空了。最适合撤离时服用：硬吃伤害跑出战场，到安全点再慢慢等恢复。

**HUD 表现**：StatusEffectHudPlanner 正面 buff 金色边框（30s 倒计时），负面 debuff 灰色脉搏图标（180s 倒计时），两个同时可见。

---

#### 6. 凝甲散（Coagulation Armor Powder）

> "那条手臂硬得像铁，但也笨得像铁。"

**定位**：单肢硬化丹，定向保护一条手臂或腿。

| 维度 | 值 |
|------|-----|
| 正面效果 | 指定一个肢体（ArmL / ArmR / LegL / LegR），该部位伤害减免 +60%，持续 60s |
| 负面效果 | 该部位功能系数 -0.35，持续同期（硬化的肢体不灵活）；若为手臂则攻击力 -20%，若为腿则移速 -15% |
| 持续时间 | 60s |
| 染色 | Solid（实） |
| 毒素量 | 0.20 |
| 稀有度 | common |
| 炉阶 | 1 |
| 配方原料 | 石肤苔 ×2 + 兽骨胶 ×1 |
| 服用时间 | 2.0s |
| 冷却 | 60s |

**战术博弈**：保护武器手（ArmR）不被打废，代价是攻击力下降；保护腿不被打瘸，代价是跑得慢。你选择保护哪条，就暴露了你的战斗策略——对手会改打未保护的部位。

**HUD 表现**：MiniBodyHudPlanner 硬化部位画蓝色实心覆盖层（半透明），被削弱的功能用橙色小三角标注。

---

### C. 移速提升（2 颗）

---

#### 7. 疾风丹（Gale Pill）

> "跑得快是好事。跑到最后跑不动，就不是好事了。"

**定位**：中等移速提升，持续时间较长，适合赶路和撤退。

| 维度 | 值 |
|------|-----|
| 正面效果 | 移速 +35%（`move_speed_multiplier` ×1.35），持续 60s |
| 负面效果 | 体力消耗速率 ×2.5，持续同期；效果结束后进入 `Exhausted` 状态 20s（移速 ×0.6） |
| 持续时间 | 正面 60s + 负面尾巴 20s |
| 染色 | Light（轻） |
| 毒素量 | 0.20 |
| 稀有度 | uncommon |
| 炉阶 | 1 |
| 配方原料 | 风行草 ×2 + 灵泉水 ×1 |
| 服用时间 | 1.0s |
| 冷却 | 90s |

**战术博弈**：60 秒高速移动很爽，但体力消耗 2.5 倍意味着全程冲刺约 16 秒就耗尽体力。必须间歇冲刺。效果结束后 20 秒 Exhausted 是致命窗口——如果你没跑到安全点，追兵正好在你最慢的时候到。

---

#### 8. 缩地散（Earth-Shrinking Powder）

> "缩地成寸，一息千里。只是你的腿还是凡人的腿。"

**定位**：极短极猛的爆发移速，紧急逃命或突进用。

| 维度 | 值 |
|------|-----|
| 正面效果 | 移速 +80%（`move_speed_multiplier` ×1.80），持续 10s |
| 负面效果 | 效果结束后双腿（LegL + LegR）各受 1 级伤口恶化（如 INTACT → BRUISE），**不可被护甲减免**；10s 内体力不恢复 |
| 持续时间 | 正面 10s；负面即时（腿伤永久直到自然愈合） |
| 染色 | Violent（烈） |
| 毒素量 | 0.35 |
| 稀有度 | rare |
| 炉阶 | 2 |
| 配方原料 | 烈风根 ×1 + 缩骨藤 ×1 + 灵泉水 ×1 |
| 服用时间 | 0.5s（紧急吞服） |
| 冷却 | 180s |

**战术博弈**：0.5 秒服用 + 10 秒 ×1.80 速度 = 终极逃命手段。但双腿必定受伤——连续使用两次就 ABRASION，三次 LACERATION，四次 FRACTURE，五次 SEVERED。**这不是可以无限吃的逃命药，是透支双腿的赌命选择。**

---

### D. 体力恢复增强（2 颗）

---

#### 9. 回力丹（Vigor Recovery Pill）

> "用真元换体力。修仙的人干凡人的蠢事。"

**定位**：战斗间歇快速恢复体力，代价是持续消耗真元。

| 维度 | 值 |
|------|-----|
| 正面效果 | 体力恢复速率 ×3.0（5.0/s → 15.0/s），持续 90s |
| 负面效果 | 持续期间 qi_current 以 2.0/s 速率流失（真元归还环境，走 `QiTransfer`） |
| 持续时间 | 90s |
| 染色 | Mellow（醇） |
| 毒素量 | 0.15 |
| 稀有度 | common |
| 炉阶 | 1 |
| 配方原料 | 参须 ×2 + 活血草 ×1 |
| 服用时间 | 1.5s |
| 冷却 | 60s |

**战术博弈**：90 秒消耗 180 点真元换满体力。引气期 qi_max 才 40——吃一颗回力丹 20 秒就空了。高境修士负担得起，低境修士是在拿命换体力。**境界越低越不敢吃**，但境界低的人恰恰最缺体力。

---

#### 10. 虎骨散（Tiger Bone Powder）

> "虎骨入体，力大无穷。虎骨散去，你比猫还弱。"

**定位**：体力上限爆发提升 + 即时回体力，但药效退去后严重虚脱。

| 维度 | 值 |
|------|-----|
| 正面效果 | 最大体力 +50%（100 → 150），立即恢复当前体力到新上限的 80%（即 120），持续 120s |
| 负面效果 | 效果结束后 `StaminaCrash` 60s：体力上限 -30%（100 → 70），恢复速率 -60%（5.0/s → 2.0/s） |
| 持续时间 | 正面 120s；负面尾巴 60s |
| 染色 | Heavy（厚） |
| 毒素量 | 0.30 |
| 稀有度 | uncommon |
| 炉阶 | 2 |
| 配方原料 | 虎骨粉 ×1（异变虎兽核碎片）+ 参须 ×1 + 重土 ×1 |
| 服用时间 | 2.0s |
| 冷却 | 180s |

**战术博弈**：120 秒的高体力窗口足够打完一场中等战斗。但之后 60 秒虚脱期——体力只有 70 且恢复极慢——是你最脆弱的时刻。如果战斗拖到虚脱期，你连逃跑的体力都没有。

---

## P0：StatusEffectKind 扩展 + 基础框架

### 新增 StatusEffectKind 变体

```rust
// server/src/combat/events.rs
pub enum StatusEffectKind {
    // ... 现有 21 个 ...
    
    // plan-alchemy-combat-v1 新增
    WoundHeal,            // 正面：伤口恢复（即时，magnitude = 恢复等级数）
    BodyPartResist,       // 正面：部位硬化（magnitude = 减免百分比）
    BodyPartWeaken,       // 负面：部位脆弱（magnitude = 功能系数降幅）
    SpeedBoost,           // 正面：移速提升（magnitude = 倍率增量）
    StaminaRecovBoost,    // 正面：体力恢复加速（magnitude = 倍率）
    StaminaCrash,         // 负面：体力虚脱（magnitude = 上限降幅百分比）
    QiDrainForStamina,    // 负面：真元换体力持续流失（magnitude = 每秒流失量）
    LegStrain,            // 负面：腿部应力伤（即时，magnitude = 伤口恶化等级）
}
```

### 伤口恢复通道

```rust
// server/src/combat/wound_heal.rs（新文件）
pub fn apply_wound_heal(wounds: &mut Wounds, target: Option<BodyPart>, grades: u8) { ... }
// - target = None → 全身每部位 -grades
// - target = Some(part) → 仅该部位 -grades
// - SEVERED 不受普通 heal 影响（需 断续散 专属路径）
// - 发 WoundHealEvent 给 narrative + HUD 同步
```

### 部位抗击打框架

```rust
// server/src/combat/status.rs — attribute_aggregate_tick() 扩展
// BodyPartResist：存储 (BodyPart, magnitude, remaining_ticks)
// → resolve_attack_intents() 读取目标部位的 resist 值，damage *= (1 - resist)
// BodyPartWeaken：同理，damage *= (1 + weaken)
```

## P1：10 颗丹药实现

每颗丹药需要：

1. `server/assets/items/pills.toml` — 1 条 `[[item]]`（id / name / category / rarity / effect / cast_duration_ms / cooldown_ms）
2. `server/assets/alchemy/recipes/{pill_id}_v1.json` — 1 份配方（stages / fire_profile / outcomes / side_effect_pool）
3. `server/src/alchemy/pill.rs` — `consume_pill()` 扩展分支，按 effect.kind 走对应 handler
4. `server/src/alchemy/side_effect_apply.rs` — 新 tag → StatusEffectKind 映射

### 丹药 ID 与毒素/稀有度速查

| # | ID | 名称 | 分类 | 染色 | 毒素 | 稀有度 | 炉阶 |
|---|-----|------|------|------|------|--------|------|
| 1 | `huo_xue_dan` | 活血丹 | wound_heal | Gentle | 0.15 | common | 1 |
| 2 | `xu_gu_gao` | 续骨膏 | wound_heal_targeted | Solid | 0.25 | uncommon | 2 |
| 3 | `duan_xu_san` | 断续散 | severed_mend | Turbid | 0.80 | epic | 3 |
| 4 | `tie_bi_san` | 铁壁散 | body_resist_torso | Heavy | 0.30 | uncommon | 2 |
| 5 | `jin_zhong_dan` | 金钟丹 | body_resist_all | Heavy | 0.45 | rare | 2 |
| 6 | `ning_jia_san` | 凝甲散 | body_resist_limb | Solid | 0.20 | common | 1 |
| 7 | `ji_feng_dan` | 疾风丹 | speed_boost | Light | 0.20 | uncommon | 1 |
| 8 | `suo_di_san` | 缩地散 | speed_burst | Violent | 0.35 | rare | 2 |
| 9 | `hui_li_dan` | 回力丹 | stamina_recov | Mellow | 0.15 | common | 1 |
| 10 | `hu_gu_san` | 虎骨散 | stamina_burst | Heavy | 0.30 | uncommon | 2 |

## P2：HUD 状态栏接入

### StatusEffectHudPlanner 扩展

- 7 个新 StatusEffectKind 各自映射 `Kind` 分类：
  - WoundHeal / BodyPartResist / SpeedBoost / StaminaRecovBoost → `BUFF`（绿色框）
  - BodyPartWeaken / StaminaCrash / QiDrainForStamina / LegStrain → `DEBUFF`（橙色框）
- 正面 + 负面同时存在时**两个都显示**（占 2 栏），玩家随时看到代价
- 负面 debuff 倒计时条用红色（区别正面的绿色）

### MiniBodyHudPlanner 扩展

- **部位硬化**：硬化中的部位 dot 外画 2px 蓝色方框（叠加在伤口色点之上）
- **部位脆弱**：脆弱部位 dot 外画 1px 红色虚线框
- **伤口恢复动画**：伤口等级下降时，该 dot 闪烁 3 次（白→新颜色）

### EventStreamHudPlanner

- 服药时写入事件流："服下活血丹，血脉加速流转。"（简短一句）
- 负面触发时写入："活血丹药力反噬，血流不止。"

## P3a：Item Icon 生成

10 张丹药图标，走 `scripts/images/gen.py --style item --transparent`。

**输出路径**：`client/src/main/resources/assets/bong-client/textures/gui/items/<item_id>.png`（64×64 PNG，透明背景）

| # | 文件名 | gen.py prompt 描述 | 视觉特征 |
|---|--------|-------------------|---------|
| 1 | `huo_xue_dan.png` | 圆润红色药丸，表面有暗红色血脉纹路流动，微微发光 | 红底，血丝纹 |
| 2 | `xu_gu_gao.png` | 白色膏药罐，瓷质小钵，里面是乳白色粘稠膏体，瓶口有白色溢出 | 白瓷钵，膏体 |
| 3 | `duan_xu_san.png` | 深褐色粉末包在粗糙的灰色兽皮纸中，散发浊黄色微光，上方有碎骨纹裂痕 | 兽皮包，浊黄光 |
| 4 | `tie_bi_san.png` | 暗灰色方形药块，表面有铁锈色金属颗粒，质感粗糙沉重 | 铁灰色，金属粒 |
| 5 | `jin_zhong_dan.png` | 金色圆丹，表面光滑如铸铜，有极细的钟纹浮雕，散发淡金色光晕 | 金色，钟纹 |
| 6 | `ning_jia_san.png` | 青灰色粉末装在骨质小筒中，筒口有石化结晶凝结 | 骨筒，青灰粉 |
| 7 | `ji_feng_dan.png` | 浅绿色半透明丹丸，内部有气流旋涡纹，似乎在微微颤动 | 绿透明，旋涡 |
| 8 | `suo_di_san.png` | 紫黑色粉末包在树叶中，叶脉呈电弧纹，包裹紧致如爆竹 | 紫黑，电弧纹 |
| 9 | `hui_li_dan.png` | 暖黄色圆丹，质地温润如蜜蜡，表面有参须纤维嵌入 | 蜜黄，参须纹 |
| 10 | `hu_gu_san.png` | 橙色粗粒粉末装在虎骨小节中，骨管两端用兽筋扎紧，表面有虎纹刻痕 | 虎骨管，橙粒 |

**命名规范**：与 `pills.toml` 中 `id` 字段完全一致（snake_case），客户端 `ItemIconResolver` 自动按 id 查找同名 PNG。

## P3b：服用动画（PlayerAnimator JSON）

10 份独立动画文件，路径 `client/src/main/resources/assets/bong/player_animation/<anim_id>.json`。

遵循 `docs/player-animation-conventions.md` 8 条硬规则：radians 非 degrees、guard→anticipation→peak→overshoot→recovery、stopTick ≥ endTick+2、torso+legs 同向 pitch 补偿。

现有 `eat_food.json`（40 tick，单手举食物入口）作为基础模板，每颗丹药在此基础上**改骨骼姿态、时序、body 位移**以产出视觉区分。

| # | anim_id | 时长(tick) | 核心姿态区别 | cast_duration 对应 |
|---|---------|-----------|-------------|-------------------|
| 1 | `pill_huo_xue` | 30 (1.5s) | 右手举丹至口，头微仰吞服，左手自然垂 | 1.5s |
| 2 | `pill_xu_gu` | 60 (3.0s) | 双手合拢在目标部位抹敷，torso 前倾 0.3rad（低头看伤口），身体微蹲 | 3.0s |
| 3 | `pill_duan_xu` | 100 (5.0s) | body.y -0.4（跪地），双手按住断肢处，torso 前倾 0.5rad，头垂下；中段 body 微颤（tick 40-60 随机 ±0.01 y 抖动）| 5.0s |
| 4 | `pill_tie_bi` | 40 (2.0s) | 右手握散送口→吞服→右掌拍胸口（rightArm pitch 从 -1.0 → 0.2 → -0.5），torso 微后仰 | 2.0s |
| 5 | `pill_jin_zhong` | 20 (1.0s) | 右手极速送丹入口（10tick 内完成），随即双掌合十于胸前（双臂 pitch=-0.8, bend=1.2），head pitch=0（正视） | 1.0s |
| 6 | `pill_ning_jia` | 40 (2.0s) | 右手从骨筒倒粉至左手→左手握住目标肢体涂抹，身体侧倾向目标肢体方向 | 2.0s |
| 7 | `pill_ji_feng` | 20 (1.0s) | 右手送丹入口→单膝微蹲（leftLeg bend=0.5）→起身弓步（rightLeg pitch=-0.3），重心前倾 | 1.0s |
| 8 | `pill_suo_di` | 10 (0.5s) | 极速吞服（rightArm 5tick 到口），猛然弓步前冲姿态（body.z +0.15, 双腿大开），整个动画 10tick 完成 | 0.5s |
| 9 | `pill_hui_li` | 30 (1.5s) | 右手送丹→吞服→左手捂腹（leftArm pitch=-0.6, bend=1.0），torso 微前倾，深呼吸节奏（body.y ±0.02 缓呼吸） | 1.5s |
| 10 | `pill_hu_gu` | 40 (2.0s) | 右手送散入口→双拳握紧（双臂 bend=1.5, pitch=-0.3）→双臂外展爆喝（pitch=0.2, body.y +0.05 微跳），head pitch=-0.1（仰头） | 2.0s |

**Java 注册**：`BongAnimations.java` 新增 10 个 `Identifier`，`BongAnimationRegistry` 自动加载同名 JSON。

**触发时机**：`pill.rs` 的 `consume_pill()` 开始时（cast 开始帧），通过 `bong:vfx_event` 下发 `play_animation` 指令，客户端 `BongAnimationPlayer.play(player, anim_id, priority=250)` 播放。

## P3c：粒子效果

每颗丹药 1 张粒子贴图 + 1 个 VfxPlayer 实现 + VfxBootstrap 注册。

### 粒子贴图

路径：`client/src/main/resources/assets/bong/textures/particle/<particle_id>.png`
配套 JSON：`client/src/main/resources/assets/bong/particles/<particle_id>.json`（指向贴图）
生成：`scripts/images/gen.py --style particle`（黑底白/发光形态，64×64）

| # | particle_id | gen.py prompt | 形态 |
|---|-------------|--------------|------|
| 1 | `huo_xue_mist` | 红色血雾团，边缘散开如雾状，中心浓 | BongSpriteParticle |
| 2 | `xu_gu_band` | 白色光带条，中间亮边缘渐隐，长条形 | BongRibbonParticle |
| 3 | `duan_xu_vortex` | 浊黄色气旋团，漩涡纹，中心暗边缘亮 | BongSpriteParticle |
| 4 | `tie_bi_metallic` | 灰色金属碎片，棱角分明，有金属反光 | BongSpriteParticle |
| 5 | `jin_zhong_bell` | 金色半透明圆弧，钟形轮廓，光晕渐隐 | BongSpriteParticle |
| 6 | `ning_jia_crust` | 青灰色甲壳碎片，石化质感，边缘有裂纹 | BongSpriteParticle |
| 7 | `ji_feng_wind` | 绿色风线条，细长弧形，尾部渐隐 | BongLineParticle |
| 8 | `suo_di_arc` | 紫色电弧，锯齿形闪电纹，高亮中心 | BongLineParticle |
| 9 | `hui_li_breath` | 暖黄色气息团，柔和圆形，半透明 | BongSpriteParticle |
| 10 | `hu_gu_stripe` | 橙色虎纹条，粗壮弧形，有兽纹刻痕 | BongRibbonParticle |

### VfxPlayer 实现

路径：`client/src/main/java/com/bong/client/visual/particle/alchemy/`（新子包）

| # | VfxPlayer 类名 | vfx_event ID | 粒子行为描述 |
|---|---------------|-------------|-------------|
| 1 | `HuoXueDanPlayer` | `bong:pill_huo_xue` | 以玩家腰部为中心，向外扩散 8-12 个红色血雾 sprite，lifetime 15 tick，半径 1.5m，Y 随机 ±0.5 |
| 2 | `XuGuGaoPlayer` | `bong:pill_xu_gu` | 在目标部位（从 payload.direction 推算）生成 3 条白色 ribbon，缠绕旋转 1 圈，lifetime 40 tick |
| 3 | `DuanXuSanPlayer` | `bong:pill_duan_xu` | 断肢位置生成浊黄气旋，6 个 sprite 围绕中心旋转收缩，lifetime 60 tick，伴随 groundDecal 黄色光圈 |
| 4 | `TieBiSanPlayer` | `bong:pill_tie_bi` | 胸腹区域生成 10 个灰色金属碎片 sprite 由外向内贴合躯干，lifetime 20 tick，到达后 fade out |
| 5 | `JinZhongDanPlayer` | `bong:pill_jin_zhong` | 以玩家为中心生成金色半透明球壳（8 个弧形 sprite 围成球），expand → hold → shrink，lifetime 30 tick |
| 6 | `NingJiaSanPlayer` | `bong:pill_ning_jia` | 目标肢体位置生成 5 个青灰甲壳 sprite 逐片覆盖，lifetime 25 tick |
| 7 | `JiFengDanPlayer` | `bong:pill_ji_feng` | 脚底向上生成 6 条绿色风线 line particle，螺旋上升至头顶，lifetime 15 tick |
| 8 | `SuoDiSanPlayer` | `bong:pill_suo_di` | 双腿位置爆发 4 条紫色电弧 line particle，极短 lifetime 8 tick，伴随 flash 白屏 0.1s |
| 9 | `HuiLiDanPlayer` | `bong:pill_hui_li` | 丹田（腹部）扩散暖黄气息 sprite，8 个向外扩散 → 回收，呼吸节奏循环 2 次，lifetime 30 tick |
| 10 | `HuGuSanPlayer` | `bong:pill_hu_gu` | 四肢表面生成橙色虎纹 ribbon，从肩/髋向手/脚方向铺开，lifetime 30 tick |

**VfxBootstrap 注册**（10 条）：
```java
registry.register(id("pill_huo_xue"), new HuoXueDanPlayer());
registry.register(id("pill_xu_gu"), new XuGuGaoPlayer());
// ... 以此类推
```

### 负面效果粒子（复用现有）

负面 debuff 生效时不需新粒子——复用现有 StatusEffect HUD 视觉（橙色框 + 红色倒计时条）。但以下两颗丹药的负面效果有额外世界粒子：

| 丹药 | 负面粒子 | 复用 |
|------|---------|------|
| 活血丹 | 出血加速期间，伤口部位持续滴红色 sprite（已有 `Bleeding` 视觉） | 复用 Bleeding 粒子，count ×2 |
| 缩地散 | 腿伤发生时，双腿位置闪红 + 地面 groundDecal 裂纹 | 复用 `CLOUD_DUST`（改红色） |

## P3d：音效 Recipe

路径：`client/src/main/resources/assets/bong/audio_recipes/<recipe_id>.json`

每颗丹药 1 份独立 audio recipe，用 vanilla MC 音效分层组合（不引入外部音频文件）。

| # | recipe_id | 层 1（主体） | 层 2（质感） | 层 3（尾韵，可选） |
|---|-----------|------------|------------|-----------------|
| 1 | `pill_huo_xue_consume` | `entity.witch.drink` pitch=1.2 vol=0.5 | `entity.player.hurt` pitch=1.8 vol=0.15 delay=5 | `block.wet_grass.break` pitch=0.8 vol=0.1 delay=10（血液流动） |
| 2 | `pill_xu_gu_consume` | `block.bone_block.place` pitch=0.7 vol=0.6 | `block.stone.place` pitch=1.5 vol=0.3 delay=8（凝固）| `entity.skeleton.hurt` pitch=0.5 vol=0.1 delay=20 |
| 3 | `pill_duan_xu_consume` | `entity.warden.sonic_boom` pitch=0.3 vol=0.4（低频轰鸣）| `block.bone_block.break` pitch=0.6 vol=0.5 delay=10（骨裂逆响）| `entity.enderman.teleport` pitch=0.4 vol=0.2 delay=30 |
| 4 | `pill_tie_bi_consume` | `entity.iron_golem.hurt` pitch=0.8 vol=0.5 | `block.anvil.land` pitch=1.5 vol=0.2 delay=5 | — |
| 5 | `pill_jin_zhong_consume` | `block.anvil.use` pitch=0.4 vol=0.7（钟声）| `block.amethyst_block.chime` pitch=0.6 vol=0.3 delay=3 | `entity.experience_orb.pickup` pitch=0.3 vol=0.1 delay=15 |
| 6 | `pill_ning_jia_consume` | `block.stone.place` pitch=0.9 vol=0.5 | `block.dripstone_block.break` pitch=1.2 vol=0.3 delay=5 | — |
| 7 | `pill_ji_feng_consume` | `entity.phantom.flap` pitch=1.5 vol=0.5（风声）| `item.elytra.flying` pitch=1.8 vol=0.3 delay=3 | — |
| 8 | `pill_suo_di_consume` | `entity.lightning_bolt.thunder` pitch=2.0 vol=0.3（爆裂短促）| `entity.firework_rocket.launch` pitch=1.5 vol=0.2 delay=2 | — |
| 9 | `pill_hui_li_consume` | `entity.witch.drink` pitch=0.8 vol=0.4 | `entity.player.breath` pitch=0.9 vol=0.3 delay=8（深呼吸）| `block.beacon.activate` pitch=2.0 vol=0.1 delay=15 |
| 10 | `pill_hu_gu_consume` | `entity.ravager.roar` pitch=1.5 vol=0.4（虎啸）| `entity.player.attack.strong` pitch=0.8 vol=0.3 delay=5 | `block.bone_block.fall` pitch=0.6 vol=0.2 delay=10 |

**触发时机**：与粒子同步，在 `bong:vfx_event` payload 中同时携带 `audioRecipeId` 字段，客户端 VfxPlayer 播放粒子后立即触发 `AudioRecipePlayer.play(recipeId)`。

## P4：饱和测试

### 测试矩阵（每颗丹药 8+ case）

1. **正面效果验证**：服药前后对应属性数值对比
2. **负面效果验证**：debuff 触发时机、持续时间、数值精度
3. **叠加冲突**：同时服两颗同类丹（如活血丹 + 续骨膏）
4. **与现有丹药冲突**：回元丹 + 回力丹（真元恢复 vs 真元流失谁赢？）
5. **境界衰减（核心）**：6 境界 × 10 颗丹逐一验证 pos_scale / neg_scale；特别断言：
   - 通灵服活血丹 → 恢复等级 round(1×0.15)=0 → 伤口不变
   - 固元服铁壁散 → 减免 40%×0.5=20%（半效）
   - 化虚服缩地散 → 移速 +80%×0.05=+4%（聊胜于无），腿伤 round(1×0.4)=0 → 不受伤（衰减到底反而没副作用）
   - 任何境界服断续散 → qi_max 永降 3% 不衰减
   - 固元+服药时事件流出现橙色 "凡药药力不足" 警告
6. **shelflife 腐败**：新鲜 / 过期 / 严重过期三档效果对比
7. **战斗中服用**：combat 状态下 cast_duration 能否完整执行
8. **连续服用**：冷却期内尝试重复服用的拒绝逻辑
9. **SEVERED 边界**：活血丹对 SEVERED 部位不生效；断续散对非 SEVERED 部位不生效
10. **死亡边界**：服药过程中死亡 / 负面效果致死的结算

### 守恒断言

- 回力丹 qi_drain 2.0/s 的真元必须出现在对应 zone 的 spirit_qi 增量中
- 断续散 qi_max -3% 走 `QiCapPermMinus` 路径，与现有永降逻辑一致
- 所有丹药的毒素注入走 `Contamination` 现有管线，不新建通道
