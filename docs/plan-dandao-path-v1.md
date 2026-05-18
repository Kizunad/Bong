# Bong · plan-dandao-path-v1

**丹道流派——从醒灵到化虚的辅助/变异双轨骨干**。不是第八个战斗流派（worldview §五 攻防七流已闭合），而是**横向辅助流派**——任何修士都能吃丹，但深度走丹道的人自己变成了**活丹炉**，身体因长期丹毒积累而不可逆地异化。核心身份：**以身试药，药即是我**——修士用自己的身体做实验田，丹毒不是排掉的废物，而是改造血肉的原料。化虚终极 = 完全变异体（暴龙王形态），放弃人形换取极致身体属性，代价是经脉系统永久退化、社会身份不可逆暴露。

**定位**：worldview §五 七流框架外的**第三类**（非攻非防，辅助/体质改造），与七流自由叠加。一个丹道修士同时可以是体修或剑修——丹道不替代战斗流派，而是**在战斗流派之上叠加身体改造层**。

---

## 世界观锚点

- `worldview.md §三:79` — 材料/丹药**永远是辅助**，绝非必需——丹道流派恰好是这条规则的**极端化身**：丹药对普通修士是辅助，对丹道修士是本体
- `worldview.md §三:155-167` — 真元极易挥发 + 极度排他——丹毒就是"非自身真元的异种残留"在经脉内累积（与 §四 异体排斥同源）
- `worldview.md §五:613` — 温润色（Mellow）炼丹师染色：自疗+、中和异种真元、攻击力极弱——丹道流派的正典色
- `worldview.md §六.2:605-631` — 染色规则：长期修习的物理沉淀，不进战斗公式——变异是染色之上的**第二层物理沉淀**（染色改真元性质，变异改肉体结构）
- `worldview.md §十二:1010-1011` — "续命成瘾者：被丹药/秘术吊着却原地不动的存在"——暴龙王是此原型的极端化身
- `worldview.md §十二:1048` — "续命没有免费午餐"——变异代价 = 经脉效率永久降低 + 社会身份暴露
- `worldview.md §四:211-259` — 体表 16 部位 × 6 档伤口——变异形态在此模型上**增设附加部位**（角/鳞/多臂不替代原 16 位，而是额外 slot）
- `worldview.md §四:260-290` — 经脉 20 条 × 4 档损伤 + 污染——丹毒就是 contamination 的丹药特化来源

**library 锚点**：
- `docs/library/peoples/战斗流派源流.json` — 需补充"丹道流"条目
- `docs/library/ecology/` — 变异灵草/暴龙王生态条目待新建

---

## Worldview 扩展：§六.4 丹体异化（Pill-Body Transmutation）

> 正式写入 worldview.md 需在 plan 归档前完成。以下是待写入内容。

### 丹体异化机制

末法残土的正典修炼路径是经脉拓扑变化。但存在一条**代价高昂的旁路**——

长期大量服用高浓度丹药的修士，体内丹毒不是简单地"排异消散"，而是**与血肉融合**。温润色真元有"中和异种真元"的特性（worldview §六:613），但当丹毒积累超过经脉的消化上限时，溢出的丹毒开始改造肉体。这不是修炼，是**被动适应**——身体为了不被毒死，开始按丹毒的"模板"重塑自己。

**变异阶梯**（按累计丹毒总量，单位与现有 `toxin_amount` 一致：0.x 量级）：

> 基线参照：现有普通丹药 toxin_amount 中位数 ~0.5/颗。正常修士醒灵→化虚约吃 100-150 颗辅助丹（累计 ~50-75），永远不会触发微变线。只有**刻意大量服药**（2 倍以上正常用量）的丹道修士才会进入变异轨道。

| 阶段 | 累计丹毒阈值 | 约需普通丹(0.5/颗) | 对应游戏时段 | 表现 | 功能 | 代价 |
|------|------------|-------------------|------------|------|------|------|
| 0 — 常人 | 0-30 | <60 颗 | — | 无外观变化 | 无 | 无 |
| 1 — 微变 | 30-100 | 60-200 颗 | 引气中期~凝脉（~15h） | 虹膜变色（金/银/赤）、肤色微变、指甲硬化 | 暗视（负灵域视距+30%）+ 抗毒（contamination tick 排毒 +10%） | 经脉效率 -3%（所有经脉 contamination baseline +0.03） |
| 2 — 显变 | 100-250 | 200-500 颗 | 凝脉后期~固元（~30h） | 额头骨脊隆起、前臂鳞片、脊椎突起 | 天然护甲（**仅对应部位** ABRASION→BRUISE 降档）+ 近战附加 | 经脉效率 -8%、NPC 好感度 -20、inspect 可见 |
| 3 — 重变 | 250-500 | 500-1000 颗 | 通灵期（~50h） | 双角/尾巴/背部甲壳 | 角冲撞（额外近战 slot）+ 甲壳（**仅背部**等效中甲）+ 尾击 | 经脉效率 -15%、大部分 NPC 拒绝交易、全服广播"异化者出没" |
| 4 — 兽化 | 500+ | 1000+ 颗 | 化虚后长期积累（100h+） | 多臂 / 体型膨胀 ×1.5 / 面部完全非人 | 副手 slot +2（快速切换，**非同时攻击**）+ 体质 +50%（hitbox ×1.5 更易命中） | 经脉效率 -30%、人形装备不可穿、NPC 敌对、天道注视 |

**阶段 4 能力限制（防 OP）**：
- 多臂 = 持握 slot +2 + 切换武器无延迟，**不是同时挥 4 把武器**——一次仍只挥一把，但可瞬间切换
- 体质 +50% 而非 ×2——搭配 hitbox ×1.5 = 更大目标、更容易被范围命中
- **全力一击仍触发虚脱**（worldview §四:388 不可违反）——但变异体可在虚脱期用「服丹急行」缩短恢复
- 天然甲仅覆盖**变异生长部位**（背甲=背部 only / 前臂鳞=前臂 only），不是全身

**关键规则**：
- 变异**不可逆**——不同于染色可洗（worldview §六:631），变异一旦发生就是永久肉体改造。降境不回退变异
- 变异有**功能性**——不只是外观，每个变异 slot 提供实际战斗/生存能力
- 变异有**经脉代价**——每次变异永久提升 contamination baseline（经脉被丹毒"改写"了一部分）
- 变异在 inspect 中**完全可见**——无法遮蔽（神识遮蔽对变异无效，肉体是物理存在）
- **社会代价**：变异体被大部分修士/NPC 视为"禁忌"/"不洁"，影响交易/社交/声望
- 变异发生时**触发顿悟选择**——与 worldview §六.3 顿悟机制接入
- **丹毒阈值不看"当前丹毒"而看"历史累计丹毒总量"**——排干净丹毒不能避免变异，因为变异是身体已经发生的物理改造

### 暴龙王：丹体异化的终极形态

末法残土最古老的续命成瘾者之一。他曾是上古大能的炼丹弟子，在末法降临后靠不间断吞服自炼丹药存活了数千年——寿元一次次即将耗尽，又被续命丹续回来（每次都以经脉效率为代价）。数千年的累积，他的身体已经完全非人：暴龙形态、双翼、多肢。

**叙事定位**：
- 他不是"邪恶 BOSS"，他是"续命成瘾的悲剧"——一个不愿意死的老人，用了太多药
- 他的防御随寿命增长（壳层越来越厚），但经脉效率已降至 5% 以下——真元几乎不能流通
- 他依赖丹药维持身体运转，而非真元——这是对正统修炼路径的彻底背叛
- 击杀他的核心策略：切断他的丹药来源（打碎他的炉/储物），等他身体自行崩溃

---

## 交叉引用（已完成 plan）

- `plan-alchemy-v1` ✅ — 核心炼丹链路（配方/火候/熔炉/五桶输出/ContamSource 复用）
- `plan-alchemy-v2` ✅ — 副作用映射 + 丹方残卷 + 品阶开光 + 自动炼丹 + 丹心识别
- `plan-alchemy-client-v1` ✅ — Fabric 端炼丹 UI
- `plan-alchemy-combat-v1` ✅ — 战斗中使用丹药
- `plan-alchemy-recycle-v1` ✅ — 炼丹回收
- `plan-botany-v1/v2` ✅ — 植物系统 + 采集（丹道专属灵草走此底盘）
- `plan-forge-v1` ✅ — 炼器底盘（丹道炼器扩展此系统）
- `plan-combat-no_ui` ✅ — `AttackIntent` / `CombatEvent` / `StatusEffectKind`（变异攻击新增变体）
- `plan-vfx-v1` ✅ — VfxEventRouter / VfxPlayer / BongParticles 管线
- `plan-armor-v1` ✅ — 护甲系统（变异天然护甲走此底盘但跳过装备 slot）
- `plan-armor-visual-v1` ✅ — GeckoLib geo.json 自定义模型渲染（变异形态复用此管线）
- `plan-npc-ai-v1` ✅ — big-brain Utility AI（暴龙王 AI 走此底盘）
- `plan-npc-skin-v1` ✅ — NPC 外观系统（暴龙王模型渲染参考）
- `plan-skill-v1` ✅ — 招式系统 + SkillConfigSchema 注册
- `plan-style-vector-integration-v1` ✅ — `PracticeLog.add()` 染色权重
- `plan-meridian-severed-v1` ✅ — 经脉永久 SEVERED + `SkillMeridianDependencies::declare()`
- `plan-qi-physics-v1` ✅ — 守恒律 / `QiTransfer` / 距离衰减
- `plan-shelflife-v1` ✅ — 物品腐败（丹药保质期）
- `plan-death-lifecycle-v1` ✅ — 死亡/重生（暴龙王击杀事件链）
- `plan-cultivation-v1` ✅ — 修炼核心（contamination / MeridianSystem / QiColor）

**交叉引用（skeleton / active）**：
- `plan-sword-path-v1` skeleton — 同为"横向流派骨干"，结构参考
- `plan-craft-v1` skeleton — 变异体部位强化配方走通用手搓
- `plan-yidao-v1` skeleton — 医道可治疗变异副作用（经脉效率恢复，但不可逆转变异外观）

---

## 接入面 Checklist

- **进料**：
  - `alchemy::pill::Pill` + `alchemy::outcome::PillOutcome` — 丹药服用 → 触发丹毒累积
  - `cultivation::contamination::Contamination` + `ContamSource` — 丹毒直接复用污染系统
  - `cultivation::components::Cultivation { qi_current, qi_max, realm }` — 境界判定
  - `cultivation::components::QiColor` + `PracticeLog` — 温润色练习记录
  - `cultivation::meridian::MeridianSystem` — 经脉状态 + SEVERED 检查
  - `cultivation::meridian::severed::SkillMeridianDependencies` — 招式经脉依赖注册
  - `inventory::InventoryComponent` — 丹药/材料消耗
  - `botany::PlantRegistry` — 灵草查询
  - `combat::armor::ArmorComponent` — 变异天然护甲叠加
  - `npc::lifecycle::NpcKind` — 暴龙王 NPC 类型注册
- **出料**：
  - `combat::events::CombatEvent` — 命中结算（新增 `AttackSource::MutationStrike` / `PillBomb` / `PillMist`）
  - `combat::events::StatusEffectKind` — 新增丹道相关状态（`PillBuffActive` / `MutationSurge` / `PillPoisonCloud`）
  - `network::VfxEventPayloadV1` — 变异视觉 + 丹药投掷粒子
  - `schema::combat_hud::TechniqueEntryV1` — HUD 同步
  - `cultivation::insight::InsightTrigger` — 变异阶段触发顿悟
  - `cultivation::life_record::LifeRecord` — 变异事件记入一生记录
  - `network::agent_bridge` — 天道感知变异体（变异阶段 3+ 触发注视）
- **共享类型/event**：
  - **复用** `AttackIntent`（新增 3 变体：MutationStrike / PillBomb / PillMist）
  - **复用** `CombatEvent` / `StatusEffectKind`
  - **复用** `Contamination` / `ContamSource`（丹毒 = ContamSource 的 pill 特化）
  - **复用** `PracticeLog.add(ColorKind::Mellow, ...)` 
  - **复用** `AlchemyFurnace` tier 系统（新增变异催化炉 tier 4+）
  - **新增** `MutationComponent { stage: MutationStage, cumulative_toxin: f64, slots: Vec<MutationSlot> }` — 变异状态
  - **新增** `MutationSlot { kind: MutationKind, level: u8 }` — 单个变异部位
  - **新增** `MutationRegistry` — 变异类型注册表（JSON 加载）
  - **新增** `DandaoStyleComponent` — 丹道修习记录（累计炼丹次数/服药次数/变异选择历史）
- **跨仓库契约**：
  - server: `server/src/dandao/` 新模块 + `server/src/alchemy/` 扩展
  - agent: `bong:mutation_event` narration（天道对变异体的反应）
  - client: GeckoLib 变异附件渲染 + HUD 变异状态面板 + 暴龙王模型
  - schema: `MutationStateV1` / `MutationEventV1` IPC payload
- **worldview 锚点**：§三 丹药辅助 / §五 温润色 / §六 染色+顿悟 / §十二 续命成瘾者 / **新增 §六.4 丹体异化**
- **qi_physics 锚点**：
  - `qi_physics::contamination` — 丹毒 = ContamSource 特化来源
  - `qi_physics::ledger::QiTransfer` — 炼丹/服药真元流动走守恒
  - **不新增物理常数** — 丹毒排异沿用现有 10:15 排异比（worldview §四 异体排斥）
  - 变异 contamination baseline 提升是 **MutationComponent 内状态**，不是 qi_physics 新常数

---

## §0 阶段总览

| 阶段 | 内容 | 状态 |
|---|---|---|
| **P0** ⬜ | 丹道底盘 — `DandaoStyleComponent` + `MutationComponent` + 累计丹毒追踪 + 3 基础招式（自服丹/投掷丹/丹雾）+ 经脉依赖 + PracticeLog 温润色 | — |
| **P1** ⬜ | 变异系统 — `MutationRegistry` + 4 阶段变异触发 + 变异 slot + 顿悟选择 + 社会反应 | — |
| **P2** ⬜ | 丹道专属物品 — 5 种专属灵草（含植物模型）+ 变异丹/体质丹/续命丹配方 + 变异催化炉 tier 4 | — |
| **P3** ⬜ | 变异形态视觉 + HUD — client GeckoLib 变异附件（角/鳞/多臂/尾）+ 丹道专属 HUD 面板 + inspect 变异图 | — |
| **P4** ⬜ | 暴龙王 BOSS — 模型导入 + 5 动画映射 + big-brain AI + 3 阶段战斗 + 掉落物 | — |
| **P5** ⬜ | 境界递进功法 + 平衡 — 醒灵→化虚各境界解锁丹道能力 + 与七流派克制关系 + 天道互动 | — |

---

## P0：丹道底盘

### §1.1 DandaoStyleComponent

```rust
/// 丹道修习记录——挂在 player entity 上，首次炼丹/服药时 lazy insert。
#[derive(Component, Debug, Clone, Serialize, Deserialize)]
pub struct DandaoStyle {
    /// 累计成功炼丹次数（含任何品阶）
    pub brew_count: u32,
    /// 累计服药次数
    pub pill_intake_count: u32,
    /// 历史累计丹毒总量（只增不减——排毒不降此值）
    pub cumulative_toxin: f64,
    /// 当前变异阶段（0-4）
    pub mutation_stage: u8,
    /// 丹道熟练度（累计炼丹/服药/投掷操作时长 tick）
    pub mastery_ticks: u64,
}
```

**初始化时机**：玩家首次执行以下任一操作时 lazy insert：
- 成功炼出一颗丹药（`AlchemySession` 完成）
- 服用一颗丹药（`PillIntakeEvent`）
- 使用丹道招式

**PracticeLog 集成**：每次炼丹/服药，调用 `practice_log.add(ColorKind::Mellow, amount)` 累积温润色。

### §1.2 累计丹毒追踪

现有 `Contamination` 组件跟踪**当前**污染值（可排毒降低）。丹道需要额外追踪**历史累计**丹毒——因为变异不看当前丹毒（排干净也不能逆转已发生的肉体改造），而看你一辈子吃了多少毒。

```rust
// 在 DandaoStyle 内的 cumulative_toxin 字段
// 每次 ContamSource::Pill { .. } 进入 contamination 系统时：
//   dandao_style.cumulative_toxin += source.amount;
//   dandao_style.pill_intake_count += 1;
```

**钩入点**：`alchemy::pill::apply_pill_effects` system（已有）→ 在 `ContamSource::Pill` 写入后，同步更新 `DandaoStyle.cumulative_toxin`。

### §1.3 三基础招式

丹道不是"法术"，而是**用丹药作为战斗道具**。三个基础招式对应三种丹药战术应用。

#### 招式一：「服丹急行」 — 自服战斗丹（零距离，buff 自身）

- **机制**：从 hotbar 选择一颗丹药 → 0.5s cast → 服下 → 立即触发丹药效果（绕过正常消化延迟）+ 额外 buff（丹道熟练度越高，紧急服丹效率越高）
- **vs 正常服药**：正常服药有 3s 消化延迟 + 效果缓释。服丹急行 = 战斗中快速用药的专属能力
- **真元消耗**：qi_max × 3%（醒灵 0.3，引气 1.2，凝脉 4.5，固元 16，通灵 63，化虚 321）走 `QiTransfer`
- **冷却**：15s（mastery ≥ 中级降至 8s）
- **经脉依赖**：足太阴脾经 `SP`（消化吸收）+ 足少阴肾经 `KI`（韧性）
- **SkillMeridianDependencies**: `declare("dandao_pill_rush", vec![MeridianId::SP, MeridianId::KI])`

**视听规格**：
- **动画**：gen_dandao_pill_rush.py — 右手抛丹入口 endTick=10, right_arm pitch -1.2rad → 0rad (easing: ease_out_quad), head pitch -0.3rad（微仰头）
- **粒子**：`BongSpriteParticle` × 8, lifetime 15 tick, 从口部向外扩散, 颜色 `#7ED4A0`（温润绿）, spawn burst, 贴图 `bong:particle/pill_glow`, VfxPlayer `DandaoPillRushVfx`, event ID `bong:vfx_dandao_pill_rush`
- **音效**：audio_recipe `{"layers": [{"sound": "entity.generic.drink", "pitch": 1.3, "volume": 0.8, "delay_ticks": 0}, {"sound": "entity.player.burp", "pitch": 0.7, "volume": 0.4, "delay_ticks": 8}]}`
- **HUD**：左下角状态区弹出「服丹」图标 2s, 颜色 `#7ED4A0` opacity 0.9 → 0.0 fade out 20 tick

#### 招式二：「投丹」 — 投掷丹药弹（5-15 格中距离，范围效果）

- **机制**：选择一颗丹药 → 真元包裹 → 投出 → 命中后碎裂释放丹药效果（对命中点 3 格范围内敌人施加 50% 效力的丹药副作用 contamination）
- **真元消耗**：丹药封存真元 = `pill.qi_cost × 1.5`（额外 0.5 倍是封存衰减税，走 `qi_physics::container` 衰减）
- **距离衰减**：真元包裹 = 凡铁级载体（worldview §五:409 "飞 10 格损失 75%"），但丹药碎裂释放不依赖残余真元——只要物理命中就触发
- **冷却**：8s
- **经脉依赖**：手太阴肺经 `LU`（气，利远程）+ 足太阴脾经 `SP`
- **SkillMeridianDependencies**: `declare("dandao_pill_bomb", vec![MeridianId::LU, MeridianId::SP])`

**视听规格**：
- **动画**：gen_dandao_pill_bomb.py — 右手上扬 → 前抛 endTick=12, right_arm pitch 0 → -1.5rad → 0.5rad (easing: ease_in_out_cubic), body yaw 跟随目标方向
- **粒子（投掷阶段）**：`BongRibbonParticle` × 1 trail, lifetime 20 tick, 跟随弹道, 颜色 `#A8E6CF`（浅绿）, continuous, 贴图 `bong:particle/pill_trail`
- **粒子（碎裂阶段）**：`BongSpriteParticle` × 24, lifetime 40 tick, 球形 burst, 半径 3 格, 颜色 `#7ED4A0` → `#4A7A5C`（绿→暗绿）, spawn burst, 贴图 `bong:particle/pill_burst`, VfxPlayer `DandaoPillBombVfx`, event ID `bong:vfx_dandao_pill_bomb`
- **音效**：`{"layers": [{"sound": "entity.witch.throw", "pitch": 1.1, "volume": 0.7, "delay_ticks": 0}, {"sound": "block.glass.break", "pitch": 0.6, "volume": 0.9, "delay_ticks": 6}]}`
- **HUD**：无特殊 HUD（命中反馈走通用战斗 hit marker）

#### 招式三：「丹雾」 — 丹药蒸发（0-5 格近距离，持续 AoE）

- **机制**：消耗一颗丹药 + 10 qi → 在脚下 5 格范围制造丹雾区域 → 持续 15s → 区域内效果按丹药类型：
  - 回复类丹药 → 区域持续回复（友方真元 +1/s）
  - 毒性类丹药 → 区域持续毒伤（敌方 contamination +0.5/s）
  - 增益类丹药 → 区域增益光环
- **真元消耗**：10 qi + 持续 0.5 qi/s 维持（走 `QiTransfer`）
- **冷却**：30s
- **经脉依赖**：足太阴脾经 `SP` + 足厥阴肝经 `LR`（韧性 + 持久）
- **SkillMeridianDependencies**: `declare("dandao_pill_mist", vec![MeridianId::SP, MeridianId::LR])`

**视听规格**：
- **动画**：gen_dandao_pill_mist.py — 双手合十碎丹 endTick=16, left_arm + right_arm 对称向中心合拢 pitch ±0.8rad → 0, body z -0.1（微前倾）
- **粒子**：`BongGroundDecalParticle` × 1 区域底贴 + `BongSpriteParticle` × 40 continuous, lifetime 30 tick, 从地面上升 0.5-1.5 格, 速度 0.02/tick 向上, 颜色根据丹药类型：回复 `#7ED4A0` / 毒性 `#8B5A8B` / 增益 `#FFD700`, spawn continuous 2/tick, 贴图 `bong:particle/pill_mist`, VfxPlayer `DandaoPillMistVfx`, event ID `bong:vfx_dandao_pill_mist`
- **音效（起手）**：`{"layers": [{"sound": "block.brewing_stand.brew", "pitch": 0.5, "volume": 0.6, "delay_ticks": 0}]}`
- **音效（持续）**：`{"layers": [{"sound": "entity.puffer_fish.blow_up", "pitch": 0.3, "volume": 0.3, "delay_ticks": 0}]}` loop every 40 tick
- **HUD**：丹雾区域边界用 `BongGroundDecalParticle` 圆形标示, 颜色对应丹药类型, opacity 0.4

### §1.4 经脉依赖总览

丹道依赖**足三阴**（脾/肾/肝 = 韧/持久/抗毒）+ 肺经 LU（气，用于远程投掷）。

| 招式 | 依赖经脉 | 理由 |
|------|---------|------|
| 服丹急行 | SP + KI | 消化吸收 + 药效韧性 |
| 投丹 | LU + SP | 远程投掷 + 丹药制备 |
| 丹雾 | SP + LR | 持久释放 + 毒素控制 |
| P5 高阶功法 | 递增（详见 P5） | — |

**SP（脾经）是全丹道的核心经脉**——断了脾经的丹道修士直接废一半。这与 worldview §四:286 "断了肺经的飞剑手就废了" 对等。

### §1.5 测试要求（P0）

- 每个招式：happy path + 冷却检查 + 经脉 SEVERED 拒绝 + 真元不足拒绝
- `DandaoStyle` 初始化：首次炼丹触发 / 首次服药触发 / 重复操作不重复初始化
- `cumulative_toxin` 追踪：3 种丹药各服 1 颗 → 累计值 = 三颗丹毒之和
- PracticeLog 集成：炼丹/服药后 Mellow 权重增加
- 守恒律：所有 qi 消耗走 `QiTransfer`，断言 zone qi 变化量 = 玩家消耗量

---

## P1：变异系统

### §2.1 MutationComponent

```rust
/// 变异状态——挂在 player/NPC entity 上。
/// 首次 cumulative_toxin 超过阈值 500 时 insert。
#[derive(Component, Debug, Clone, Serialize, Deserialize)]
pub struct MutationState {
    pub stage: MutationStage,
    pub slots: Vec<ActiveMutation>,
    /// 经脉效率惩罚（0.0-1.0，叠加到所有经脉的 contamination baseline）
    pub meridian_penalty: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MutationStage {
    None,      // 0: cumulative < 30.0
    Subtle,    // 1: 30.0-100.0
    Visible,   // 2: 100.0-250.0
    Heavy,     // 3: 250.0-500.0
    Bestial,   // 4: 500.0+
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveMutation {
    pub kind: MutationKind,
    pub slot: BodySlot,
    pub level: u8,        // 1-3（同 slot 可强化）
    pub acquired_tick: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MutationKind {
    // 阶段 1 — 微变
    GoldenIris,       // 金瞳：暗视
    HardenedNails,    // 硬甲指：近战附加
    ToughSkin,        // 糙皮：抗毒

    // 阶段 2 — 显变
    BoneRidge,        // 额骨脊：撞击
    ForearmScales,    // 前臂鳞：轻甲
    SpineSpurs,       // 脊突：背部护甲

    // 阶段 3 — 重变
    Horns,            // 双角：冲撞近战 slot
    Tail,             // 尾：尾击 + 平衡
    BackCarapace,     // 背甲：中甲等效

    // 阶段 4 — 兽化
    ExtraArms,        // 多臂：额外武器 slot（最多 +2 手）
    BodyEnlarge,      // 体型膨胀：体质 ×2
    BeastFace,        // 兽面：恐吓光环 + 完全非人
}

/// 身体挂载位置（不替代原 16 部位，而是附加 slot）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BodySlot {
    Head,       // 角/兽面/金瞳
    Forearm,    // 鳞/硬甲指
    Back,       // 脊突/背甲
    Torso,      // 体型/多臂
    Lower,      // 尾
}
```

### §2.2 变异触发

`mutation_check_system` 每 600 tick（30s）检测一次 `DandaoStyle.cumulative_toxin`：

```
if cumulative_toxin >= threshold[next_stage] && current_stage < next_stage:
    emit MutationAdvanceEvent { entity, from: current_stage, to: next_stage }
```

**MutationAdvanceEvent 处理链**：
1. 触发顿悟选择（`InsightTrigger::MutationAdvance`）→ 玩家选择该阶段的 2-3 个变异 slot
2. 写入 `MutationState.slots` + 更新 `stage`
3. 更新 `meridian_penalty`
4. 写入 `LifeRecord`（"修士于某地首次显现丹体异化"）
5. 阶段 3+: emit `AgentBridge::MutationEvent` → 天道 narration
6. 阶段 3+: 全服广播（"有人现了变异之相"）

### §2.3 变异顿悟选择

每次变异阶段提升触发一次顿悟（接 `cultivation::insight` 系统）：

```
[变异阶段 1 — 微变]
你的身体开始排斥你吃下的第 X 颗丹药。
但你没有吐出来——反而，身体在适应。
你感到——

  A. 瞳孔灼烫。（金瞳：暗视 + 负灵域视距 +30%）
  B. 指甲发痒。（硬甲指：空手近战伤害 +15%）
  C. 皮肤收紧。（糙皮：排毒速率 +10%）

这不可逆。
```

```
[变异阶段 2 — 显变]
镜中的你已经不太像人了。
额头隆起的骨脊在阳光下泛着白光。
你接受了这副新模样——

  A. 骨脊硬化。（额骨脊：撞击招式解锁 + 近战附加 slot）
  B. 前臂覆鳞。（前臂鳞：等效轻甲 ABRASION→BRUISE 降档）
  C. 脊椎外突。（脊突：背部减伤 20%）

从此 inspect 你的人都能看见变化。
```

```
[变异阶段 3 — 重变]
你照了照水面。水面里的东西不再是人。
是什么？你不确定。但你确定——更强了。

  A. 双角顶出。（角：冲撞近战 slot + 5 qi 蓄力冲击）
  B. 尾椎延伸。（尾：尾击 + 坠落减伤 50% + 平衡加成）
  C. 背壳覆盖。（背甲：等效中甲 + 仰攻抗性）
```

```
[变异阶段 4 — 兽化]
你听见自己的骨头在说话。
它们要长出来。
你拦不住——

  A. 多臂。（+2 手臂 slot：可同时持 4 件装备）
  B. 膨胀。（体型 ×1.5：体质 ×2 + hitbox 扩大）
  C. 兽面。（恐吓光环 5 格：低 2 境界敌人 composure -30% + 面部完全非人）
```

### §2.4 变异功能性

每个变异 slot 提供实际战斗/生存能力：

| 变异 | 功能 | 数值 |
|------|------|------|
| 金瞳 | 负灵域视距 + 暗处感知 | 视距 +30%，暗处亮度 +2 |
| 硬甲指 | 空手近战附加 | base_attack +3（空手时） |
| 糙皮 | 排毒速率加成 | contamination_tick 排毒 +10% |
| 额骨脊 | 近战撞击 slot | 解锁「骨撞」招式（P5） |
| 前臂鳞 | 前臂部位天然轻甲 | ABRASION → BRUISE 降档 |
| 脊突 | 背部减伤 | 背部受击伤害 -20% |
| 双角 | 冲撞近战 slot | 解锁「角冲」招式（P5）+ 5 qi 蓄力 |
| 尾 | 尾击 + 平衡 | 解锁「尾击」+ 坠落减伤 50% |
| 背甲 | 天然中甲 | LACERATION → ABRASION 降档（背部） |
| 多臂 | +2 手臂 slot | 可同持 4 件装备/武器 |
| 膨胀 | 体质 +50% | HP +50% + hitbox ×1.5（更易被命中） |
| 兽面 | 恐吓光环 | 5 格内低 2 境界敌人 composure -30% |

### §2.5 社会反应系统

变异阶段 → NPC 反应变化（接 `npc::social` 系统）：

| 阶段 | NPC 反应 | 机制 |
|------|---------|------|
| 0 — 常人 | 正常 | — |
| 1 — 微变 | 不明显 | NPC 对话偶尔提及"你的眼睛不太对" |
| 2 — 显变 | 警觉 | 好感度 -20，部分商人拒绝交易 |
| 3 — 重变 | 恐惧/敌对 | 好感度 -50，多数 NPC 拒绝交易，天道注视 |
| 4 — 兽化 | 完全敌对 | 所有非变异体 NPC 主动敌对，装备限制 |

### §2.6 测试要求（P1）

- MutationStage 转换：5 个阶段各自阈值精确断言
- 每个 MutationKind 功能性至少 1 条 happy path
- 经脉惩罚叠加正确（3% / 8% / 15% / 30%）
- 顿悟触发链：cumulative_toxin 跨阈值 → InsightTrigger::MutationAdvance 发出
- 不可逆性：降境后 mutation_stage 不变、slots 不清除
- LifeRecord 写入
- 社会反应：阶段 2+ NPC 好感度变化断言

---

## P2：丹道专属物品

### §3.1 专属灵草（5 种，需植物模型）

走 `botany::PlantRegistry` 底盘新增。每种灵草对应一个变异丹配方的核心材料。

| 灵草名 | 生长区域 | 灵气需求 | 用途 | 稀有度 | 模型备注 |
|--------|---------|---------|------|--------|---------|
| **蜕骨藤** | 负灵域边缘 | -0.1 ~ 0.2 | 变异丹（骨系变异催化） | 稀有 | 藤蔓缠绕骨骼造型，暗紫色 |
| **兽心草** | 异变兽出没区 | > 0.5 | 变异丹（肌肉/体质系） | 稀有 | 心形叶片，暗红脉络 |
| **龙鳞苔** | 坍缩渊入口 | -0.3 ~ 0 | 变异丹（鳞/甲系） | 极稀有 | 扁平苔藓，表面鳞片状纹路，灰绿色 |
| **续元蕊** | 灵眼附近 | > 0.8 | 续命丹（寿元恢复） | 极稀有 | 发光花蕊，金黄色，微弱脉动 |
| **化形根** | 死域深处 | = 0 | 高阶变异丹（阶段 4 兽化） | 传说 | 人形根茎，触碰时微动，乳白色 |

**植物模型规格**（每种需 1 个 Blockbench JSON + 1 张 16×16 贴图）：
- 使用项目 `scripts/images/gen.py` 生成 item 风格图标（`/gen-image item <描述>`）
- 方块模型走 `client/src/main/resources/assets/bong/models/block/` 现有管线

### §3.2 专属丹药配方（6 种）

走 `server/assets/alchemy/recipes/` 现有 JSON 配方系统扩展。

| 丹药名 | 核心材料 | 效果 | 丹毒（ContamSource） | 品阶范围 |
|--------|---------|------|---------------------|---------|
| **蜕骨丹** | 蜕骨藤 ×2 + 异变兽骨 ×1 | 加速骨系变异进度 (+5.0 cumulative_toxin) | 高（amount: 5.0） | 2-4 |
| **兽心丹** | 兽心草 ×2 + 变异核心 ×1 | 体质增幅（HP +10% 持续 1h）+ 肌肉系变异进度 | 中（amount: 3.0） | 2-4 |
| **龙鳞散** | 龙鳞苔 ×3 + 灵铁粉 ×1 | 天然护甲增幅（持续 30min）+ 鳞/甲系变异进度 | 中（amount: 4.0） | 3-5 |
| **续元丹** | 续元蕊 ×1 + 回元芷 ×2 | 寿元恢复 +50 年 | 低（amount: 1.5） | 3-5 |
| **化形大丹** | 化形根 ×1 + 蜕骨藤 ×1 + 兽心草 ×1 + 龙鳞苔 ×1 | 直接触发下一变异阶段（跳过自然累积） | 极高（amount: 30.0） | 4-5 |
| **净毒丸** | 回元芷 ×3 + 固元根 ×2 | 降低当前 contamination 50%（但不降 cumulative_toxin） | 无（amount: 0） | 1-3 |

### §3.3 变异催化炉（tier 4）

```rust
// 扩展 AlchemyFurnace tier 系统
// tier 4 = 变异催化炉，仅限炼制变异丹系列
// 需要 forge 系统锻造（灵铁+变异核心+异变兽骨）
```

- 变异催化炉 = tier 4 `AlchemyFurnace`
- 炼制变异丹时成功率 +20%（vs 普通 tier 3 炉）
- 炼制非变异丹时无加成
- 外观：比普通炉大一圈，表面有骨骼纹路

### §3.4 炼器——变异部位强化器

走 `forge::station::WeaponForgeStation` 扩展。变异体可以锻造自己的变异部位：

| 锻造对象 | 材料 | 效果 | 说明 |
|---------|------|------|------|
| 角强化 | 异变兽骨 ×3 + 灵铁 ×1 | 角冲撞伤害 +30% | 需变异 slot: Horns |
| 鳞甲淬炼 | 龙鳞苔 ×2 + 矿石 ×2 | 鳞片护甲升一档 | 需变异 slot: ForearmScales / BackCarapace |
| 多臂协调 | 兽心草 ×2 + 固元根 ×1 | 多臂武器切换速度 -50% delay | 需变异 slot: ExtraArms |
| 尾刃化 | 灵铁 ×2 + 蜕骨藤 ×1 | 尾击伤害 +20% + 附带真元 | 需变异 slot: Tail |

### §3.5 测试要求（P2）

- 5 种灵草注册到 PlantRegistry 且生长条件正确（灵气阈值）
- 6 种丹药配方 JSON 解析通过 + RecipeRegistry 加载
- 变异催化炉 tier 4 限制：非变异丹配方不享受加成
- 4 种锻造配方：无对应变异 slot 时拒绝锻造
- shelflife：变异丹走现有腐败系统（half_life 按 qi_physics 统一常数）

---

## P3：变异形态视觉 + HUD

### §4.1 Client GeckoLib 变异附件

变异不替换玩家模型，而是**在原模型上叠加 GeckoLib 附件**（类似 `plan-armor-visual-v1` 的护甲渲染管线）。

**附件模型列表**（需制作 Blockbench JSON + 贴图）：

| 附件 | 挂载骨骼 | 模型文件 | 贴图 | 说明 |
|------|---------|---------|------|------|
| 金瞳 | head | `dandao_golden_iris.geo.json` | `dandao_iris.png` (16×16) | 眼睛发光 overlay |
| 硬甲指 | right_hand / left_hand | `dandao_hardened_nails.geo.json` | `dandao_nails.png` (32×32) | 手指变利爪 |
| 额骨脊 | head | `dandao_bone_ridge.geo.json` | `dandao_ridge.png` (32×32) | 额头隆起骨脊 |
| 前臂鳞 | right_arm / left_arm | `dandao_forearm_scales.geo.json` | `dandao_scales.png` (64×64) | 前臂覆鳞 |
| 脊突 | body | `dandao_spine_spurs.geo.json` | `dandao_spurs.png` (32×32) | 背部脊椎外突 |
| 双角 | head | `dandao_horns.geo.json` | `dandao_horns.png` (64×64) | 从额头长出的弯角 |
| 尾 | body (下方) | `dandao_tail.geo.json` | `dandao_tail.png` (64×64) | 尾椎延伸 |
| 背甲 | body | `dandao_carapace.geo.json` | `dandao_carapace.png` (128×128) | 整背覆盖的甲壳 |
| 多臂 | body (两侧) | `dandao_extra_arms.geo.json` | `dandao_arms.png` (128×128) | 腰侧额外一对手臂 |
| 兽面 | head | `dandao_beast_face.geo.json` | `dandao_beast.png` (64×64) | 完全覆盖头部的兽面 |

**渲染管线**：
- 走 `plan-armor-visual-v1` 建立的 `BongArmorFeatureRenderer` 管线
- 每个附件 = 一个 `GeoModel` + `AnimatableTexture`
- `MutationVisualSyncPayload`（CustomPayload `bong:mutation_visual`）：server → client 同步当前变异 slot 列表
- Client 端 `MutationFeatureRenderer extends LivingEntityFeatureRenderer` 按 slot 列表叠加渲染

### §4.2 HUD——丹道面板

参照 `feedback_hud_immersive_minimal.md`（常驻仅极简，其他按需）：

**常驻变化**：
- 变异阶段 1+: 左下角迷你人体剪影增加变异部位标记（用变异 slot 对应颜色点标注）
- 变异阶段 3+: 剪影轮廓变形（非人形 silhouette）

**按需面板（I 键打开 inspect UI 内）**：

```
┌─────────────────────────────────────────┐
│ 丹道·丹体异化                             │
├─────────────────────────────────────────┤
│ 变异阶段：[██████░░░░] 显变 (2/4)        │
│ 累计丹毒：3,247 / 5,000                  │
│ 经脉惩罚：-8%                             │
│                                         │
│ 已获变异：                                │
│  ◆ 金瞳 Lv.1    [HEAD]   暗视 +30%      │
│  ◆ 前臂鳞 Lv.2  [FOREARM] 轻甲 ABRASION→BRUISE │
│                                         │
│ 下一阶段(重变)需：丹毒 5,000             │
│  可选：双角 / 尾 / 背甲                   │
└─────────────────────────────────────────┘
```

**HUD 渲染层**：`HudRenderLayer::DandaoMutation`（新增，优先级低于战斗层）

### §4.3 Inspect 他人变异

inspect 另一个修士时：
- 阶段 1: 无特殊标记（外观变化微小）
- 阶段 2+: 显示「丹体异化·显变」标签 + 可见变异列表
- 阶段 4: 显示「兽化体」红色大字标签

### §4.4 测试要求（P3）

- `MutationVisualSyncPayload` 序列化/反序列化 round-trip
- 每个变异 slot 的 GeckoLib model 加载不 crash（client render test）
- HUD 面板数据正确显示（mock MutationState → 渲染断言）
- inspect 他人：阶段 2+ 显示标签、阶段 1 不显示

---

## P4：暴龙王 BOSS

### §5.1 模型与动画

**资源位置**：`local_models/baolongwang/`（已搬入）
- `baolongwang.geo.json` — Bedrock 格式 geometry（`geometry.bdk_head`），10 骨骼，1024×1024 贴图
- `baolongwang.animation.json` — 5 动画：`idle`(4.76s loop) / `walk`(1.12s loop) / `attack`(1.64s) / `skill1`(3.2s) / `skill2`(1.64s)
- `baolongwang.png` — 贴图

**格式转换需求**：
- Bedrock geometry → GeckoLib `.geo.json`（格式兼容，需验证骨骼命名规范）
- Bedrock animation → GeckoLib `.animation.json`（格式基本兼容）
- 贴图直接使用

**client 渲染**：
- 走 `plan-npc-skin-v1` 建立的 NPC 自定义模型管线
- `BaolongwangEntityRenderer extends MobEntityRenderer` + GeckoLib `AnimatableInstanceCache`
- 体型 = 4×4×6 blocks（远大于普通修士）

### §5.2 暴龙王身份设定

```rust
// server 端 NPC 注册
pub struct BaolongwangBoss {
    pub age_years: u32,           // 存活年数（影响防御）
    pub pill_reserve: Vec<Pill>,  // 携带丹药储备
    pub mutation_stage: u8,       // 固定 4（完全兽化）
    pub furnace_intact: bool,     // 炉是否完好（核心弱点）
}
```

**核心特征**：
- **无限寿命**：通过持续服用续命丹维持。但这不违反 worldview §十二"没有无限续命路径"——暴龙王有一个"炉"，炉内自动炼续命丹，切断炉 = 切断续命
- **防御随寿命**：`defense_power = 0.15 + 0.05 × (age_bracket)`（age_bracket 0-4 按存活千年分档）——满档 `defense_power = 0.35` = 受伤仅 35%（对齐现有 `combat::resolve` 乘数模型，`ARMOR_MITIGATION_CAP = 0.85` 是护甲上限，BOSS 特权可突破）
- **HP 基线**：等效通灵境体质（qi_max ~2000 级别的 HP pool）× 变异 +50% = ~3000 effective HP。不是化虚级——他经脉废了，境界实质已退化
- **经脉效率极低（5%）**：完全依赖丹药维持身体运转，几乎不能主动运真元
- **丹药储备 = 弹药库**：身上携带大量丹药，战斗中自服/投掷。储备 30 颗，用完就只能物理攻击
- **炉 = 生命线**：暴龙王身旁有一个变异催化炉，持续炼制续命丹。摧毁炉后暴龙王进入倒计时（120s real time / 2400 tick）——没有续命丹，寿元耗尽。倒计时期间攻击 +30% / defense_power 退化 0.35 → 0.6（壳在碎）

**预估团战时长**：
- 3 名固元修士（qi_max ~540）集火 → 每人每秒输出 ~20 effective damage（扣掉 0.35 defense） → 60 DPS × 50s = 3000 HP → **约 50s 硬打** + 阶段切换间歇 → 总战斗 ~3-5 min
- 1 名通灵修士 solo → 每秒 ~40 effective damage → 75s 硬打 → 总 ~2-3 min（但 BOSS 攻击可能先杀你）
- 核心设计：**不需要化虚才能打**，固元团队即可挑战，通灵可 solo

### §5.3 战斗三阶段

#### 阶段一：驱逐（HP > 70%）

暴龙王不想打架——他只想安静炼丹续命。

- **行为**：远离玩家 + 偶尔丹雾驱逐（毒性）+ 尾击反击
- **AI Scorer**：`AvoidPlayer(distance < 20) → weight 0.8` / `PillMist(distance < 8) → weight 0.5` / `TailSwipe(distance < 3) → weight 0.9`
- **动画**：`idle` + `walk`（远离）+ `attack`（尾击）

**视听规格**：
- **丹雾**：`BongSpriteParticle` × 60 continuous, 颜色 `#8B5A8B`（毒紫）, 半径 8 格, lifetime 60 tick, spawn 3/tick
- **音效**：`{"layers": [{"sound": "entity.ender_dragon.growl", "pitch": 0.4, "volume": 1.0, "delay_ticks": 0}]}`
- **narration**：
  - `"一股腐臭的药气从暴龙王的鳞缝中涌出——它似乎不想搭理你。"` scope: zone, style: perception
  - `"那东西扭过身来，比你见过的任何异变兽都大三倍。但它的眼神不是凶残——是厌烦。"` scope: player, style: narrative

#### 阶段二：暴怒（HP 30%-70%）

你惹怒了它。

- **行为**：主动攻击 + 角冲撞 + 自服增益丹 + 投掷毒丹
- **AI Scorer**：`AttackMelee(distance < 5) → weight 0.7` / `HornCharge(distance 5-15) → weight 0.6` / `SelfPill(hp < 50%) → weight 0.8` / `PillBomb(distance 5-20) → weight 0.5`
- **动画**：`attack`（近战）+ `skill1`（角冲撞）+ `walk`（追击）

**视听规格**：
- **角冲撞**：直线 15 格冲刺 0.8s, 命中点 `BongSpriteParticle` × 30 burst, 颜色 `#FFD700`（金色撞击波）, lifetime 10 tick
- **音效**：`{"layers": [{"sound": "entity.ravager.attack", "pitch": 0.5, "volume": 1.2, "delay_ticks": 0}, {"sound": "block.anvil.land", "pitch": 0.3, "volume": 0.8, "delay_ticks": 4}]}`
- **自服丹**：吞服动画 1s, 全身 `BongSpriteParticle` × 20, 颜色 `#FF4444`（暗红增益光）, lifetime 20 tick
- **narration**：
  - `"暴龙王低下头——双角对准了你。地面在它冲刺的瞬间裂开了。"` scope: zone, style: narrative
  - `"它从鳞缝间掏出一颗暗红色的丹药塞进嘴里。空气中的药味更浓了。"` scope: player, style: perception

#### 阶段三：崩溃（HP < 30% 或 炉被摧毁）

身体开始崩解——但崩解中的暴龙王比完好时更危险。

- **行为**：狂暴模式 + 全力一击 + 自爆丹药储备（范围 AoE）
- **AI Scorer**：`BerserkAttack → weight 1.0` / `SelfDetonation(hp < 10%) → weight 0.9`
- **动画**：`skill2`（全力一击）+ `attack`（连续攻击）
- **特殊**：炉被毁后，180s 倒计时。期间暴龙王 attack +50% / defense -30% / 每 10s 自身 HP -5%

**视听规格**：
- **全力一击**：`skill2` 动画 + `BongSpriteParticle` × 80 burst, 颜色 `#FF6600` → `#CC0000`（橙→血红）, 半径 10 格, lifetime 30 tick
- **音效**：`{"layers": [{"sound": "entity.wither.break_block", "pitch": 0.3, "volume": 1.5, "delay_ticks": 0}, {"sound": "entity.generic.explode", "pitch": 0.4, "volume": 1.2, "delay_ticks": 6}]}`
- **崩溃外观**：鳞片脱落粒子（`BongSpriteParticle` continuous, `#5A5A5A` 灰色碎片, 2/tick）
- **narration**：
  - `"暴龙王的壳在碎裂。你看到壳下面的肉——那不是兽的肉。那是人的肉。很老很老的人的肉。"` scope: zone, style: narrative
  - `"它发出一声悲鸣——不像兽嚎，更像一个老人在哭。"` scope: zone, style: perception

### §5.4 掉落物

| 物品 | 概率 | 用途 |
|------|------|------|
| **暴龙王核心** | 100% | 传说级变异核心——制作化形大丹的替代材料 / 直接服用触发阶段 4 变异 |
| **上古丹方残卷** | 100% | 3 张随机变异丹系列丹方碎片 |
| **暴龙王角** | 50%（仅角冲撞阶段存活） | 制作武器材料（角矛：base_attack 极高） |
| **暴龙王鳞** ×3-8 | 80% | 制作护甲材料（鳞甲：最高 tier 天然护甲） |
| **变异催化炉残骸** | 100%（仅炉被摧毁） | 可修复为 tier 5 变异催化炉（当前最高） |
| **续命丹** ×5-10 | 70% | 直接使用（寿元 +50 年 / 颗） |

### §5.5 暴龙王生成与位置

- **固定坐标**：大地图某处地下巨洞（worldgen 需配合标记区域）
- **单实例**：全服仅 1 只，击杀后 72h real time 重生
- **不主动走出**：活动范围限制在巨洞内 200 格

### §5.6 测试要求（P4）

- 模型加载：GeckoLib geo + animation + texture 不 crash
- 三阶段 AI 切换：HP 阈值触发 phase transition
- 炉摧毁：180s 倒计时启动 + HP 衰减
- 掉落物：100% 概率物品必定掉落 + 50%/80%/70% 概率物品统计验证（Monte Carlo 10000 次 seed）
- 角冲撞：15 格直线检测命中
- 丹雾 AoE：8 格范围内 contamination 增加

---

## P5：境界递进功法 + 平衡

### §6.1 醒灵→化虚丹道能力递进

| 境界 | 解锁能力 | 说明 |
|------|---------|------|
| 醒灵 | 服丹急行（P0 招式一） | 入门：快速用药 |
| 引气 | 投丹（P0 招式二） | 远程丹药投掷 |
| 凝脉 | 丹雾（P0 招式三）+ 变异阶段 1 | 区域控制 + 首次变异可能 |
| 固元 | 「丹体共鸣」被动 + 变异阶段 2 | 自服丹效率 +30%，服药无消化延迟 |
| 通灵 | 「化丹为血」+ 变异阶段 3 | 可将丹药直接转化为真元（1 pill = qi_max × 5%），代价 cumulative_toxin +10.0 |
| 化虚 | 「大衍丹体」+ 变异阶段 4 | 化虚专属：身体即是丹炉——可以不用炼丹炉，直接在体内"炼丹"（用自身真元 + 材料 → 体内生成丹药效果，无实体丹药）。代价：每次内炼，cumulative_toxin +15.0 + 经脉效率 -1% 永久叠加 |

### §6.2 丹道与七流派的互动

丹道不是战斗流派，而是**叠加在战斗流派之上的辅助层**。

| 主流派 | + 丹道辅助效果 | 代价 |
|--------|-------------|------|
| 体修·爆脉 | 变异体质 ×2 = 更能扛过载撕裂 | 经脉惩罚降低爆脉效率 |
| 器修·暗器 | 丹药弹 = 新型载体（不损耗真元） | 丹药有保质期 |
| 地师·阵法 | 丹雾 = 区域控制叠加 | 丹雾暴露位置 |
| 毒蛊 | 丹毒 ≈ 蛊毒同源——变异丹蛊师 | 双重经脉惩罚 |
| 截脉·震爆 | 变异甲壳增加截脉触发面积 | 甲壳区域无法精确截脉 |
| 替尸·蜕壳 | 变异外壳 vs 伪皮——两套壳叠加 | 变异壳不可蜕（永久） |
| 绝灵·涡流 | 丹道辅助真元恢复延长涡流持续 | 变异体真元效率低 |

### §6.3 克制关系

丹道变异体的核心弱点：
- **经脉效率下降** → 所有需要高经脉流量的招式（爆脉/涡流）效率低
- **无法隐藏** → inspect 暴露，毒蛊师伪装无效（阶段 2+ 肉体可见）
- **社会孤立** → NPC 交易受限，情报获取困难
- **不可逆** → 一旦变异无法回头

丹道变异体的核心优势：
- **身体韧性极高** → 天然护甲 + 体质加成 + 多攻击 slot
- **丹药供给** → 自产自用，不依赖外部交易
- **续命** → 变异体寿元可通过续命丹无限延长（代价是持续变异）
- **多持武器** → 阶段 4 多臂可同持 4 件装备

### §6.4 天道互动

- 变异阶段 3+: 天道注视概率 +20%（变异体 = "灵气异常聚集点"）
- 变异阶段 4: 天道**主动降劫**——变异体被视为"违背天地秩序"
- 暴龙王: 天道已放弃追踪它——它太老了，活在天道的"盲区"（炉子的位置恰好在坍缩渊边缘，负灵域屏蔽天道感知）

**narration 模板**：
- `"天道感知到一股浊乱的气息——有什么东西不应该是那个形状。"` scope: zone, style: perception
- `"你感到天穹的目光——不是因为你境界高，而是因为你的身体已经不像人了。"` scope: player, style: narrative

### §6.5 测试要求（P5）

- 每境界解锁能力正确判定（realm gate）
- 化虚「大衍丹体」：内炼流程 = 无炉炼丹但走相同 AlchemySession 状态机
- 七流派叠加：每个组合至少 1 条集成测试
- 天道注视：变异阶段 3+ 天道概率正确调整
- 克制矩阵：经脉惩罚 × 各流派核心经脉 → 效率降低断言

---

## §7 模型资源清单

### 暴龙王（已有，已搬入）

| 文件 | 路径 | 状态 |
|------|------|------|
| geometry | `local_models/baolongwang/baolongwang.geo.json` | ✅ 已搬入 |
| animation | `local_models/baolongwang/baolongwang.animation.json` | ✅ 已搬入 |
| texture | `local_models/baolongwang/baolongwang.png` | ✅ 已搬入 |

### 变异附件（P3 需制作）

| 模型 | 估计工作量 | 说明 |
|------|----------|------|
| 金瞳 overlay | 小 | 简单 overlay |
| 硬甲指 | 小 | 手部附件 |
| 额骨脊 | 中 | 头部附件 |
| 前臂鳞 | 中 | 臂部附件 |
| 脊突 | 中 | 背部附件 |
| 双角 | 大 | 大型头部附件 |
| 尾 | 大 | 独立骨骼 + 摆动动画 |
| 背甲 | 大 | 大面积覆盖 |
| 多臂 | 极大 | 额外骨骼 + 独立动画 |
| 兽面 | 大 | 完全覆盖头部 |

### 灵草植物模型（P2 需制作）

| 植物 | 方块模型 | 图标 |
|------|---------|------|
| 蜕骨藤 | 需要 | `/gen-image item` |
| 兽心草 | 需要 | `/gen-image item` |
| 龙鳞苔 | 需要 | `/gen-image item` |
| 续元蕊 | 需要 | `/gen-image item` |
| 化形根 | 需要 | `/gen-image item` |

---

## §8 开放问题（P0 决策门前需收口）

1. **经脉惩罚实战检验**：-3%/-8%/-15%/-30% contamination baseline 是否让变异体在高境界"不可战斗"？需要 mock 计算：化虚变异体（70% 经脉效率）× 体质 +50% × 多持优势 vs 正常化虚——如果 net DPS 差距 >40%，惩罚需要下调
2. **多臂切换无延迟的具体实现**：当前 `AttackIntent` 单武器模型不变，多臂只影响 hotbar 选中逻辑。但"切换无延迟"如何避免成为"每 tick 选最优武器"的 AI 优势？考虑 mastery-gated 切换速率
3. **暴龙王与 worldgen 协调**：巨洞位置需要 worldgen 侧标记。是作为固定 zone（blueprint 写死坐标）还是 poi 随机生成？建议固定坐标（类似灵眼/坍缩渊）
4. **化虚大衍丹体 vs 正常炼丹**：体内炼丹是否走完整 AlchemySession 状态机（含火候/投料/中途干预）？建议简化为"消耗 qi + 材料 → 5s cast → 直接产出效果"，不走完整 session——否则 HUD 设计过于复杂
5. **暴龙王 AI 的 big-brain 三阶段**：当前 big-brain 是 Scorer→Action 模式（无 FSM）。三阶段需要一个 `PhaseGate` Scorer 包装（根据 HP% 激活/禁用不同 Action 组）。确认现有框架是否支持或需扩展
6. **暴龙王模型 Bedrock→GeckoLib 转换**：骨骼命名 `bdk_*` 非标准（`bdk_la`=左臂、`bdk_rl`=右腿、`bdk_lw`/`bdk_rw`=左右翼）。需要映射表但不需重命名源文件——在 client renderer 里做 alias
7. **变异丹的经济投入 vs 正常修炼**：一颗蜕骨丹(+5.0 toxin)需要 2 蜕骨藤(稀有) + 1 异变兽骨。从微变(30)到兽化(500)纯吃蜕骨丹 = 94 颗 = 188 蜕骨藤 + 94 异变兽骨。这个采集量对应多少小时？需要对照 botany 产出速率校准
8. **变异附件模型制作方式**：10 个 GeckoLib 附件是手工 Blockbench 还是走 `scripts/images/gen.py` + Tripo 生成 + 手工调整？后者可大幅缩短工期但质量不可控
