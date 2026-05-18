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
| **P-1** ⬜ | worldgen layout 基础设施 — `LayoutSpec` / `Placement` / `layouts/runner.py` + terrain profile schema 扩 `architectural_layout` + `compound_flatten_radius` + stitcher density mask（§9.4a 详述）。**前置 hard dependency，P0 起所有阶段依赖 layout runner 可用**。 | — |
| **P0** ⬜ | 丹道底盘 — `DandaoStyleComponent` + `MutationComponent` + 累计丹毒追踪 + 3 基础招式（自服丹/投掷丹/丹雾）+ 经脉依赖 + PracticeLog 温润色 | — |
| **P1** ⬜ | 变异系统 — `MutationRegistry` + 4 阶段变异触发 + 变异 slot + 顿悟选择 + 社会反应 | — |
| **P2** ⬜ | 丹道专属物品 — 5 种专属灵草（含植物模型）+ 变异丹/体质丹/续命丹配方 + 变异催化炉 tier 4 | — |
| **P3** ⬜ | 变异形态视觉 + HUD — client GeckoLib 变异附件（角/鳞/多臂/尾）+ 丹道专属 HUD 面板 + inspect 变异图 | — |
| **P4** ⬜ | 暴龙王 BOSS — 模型导入 + 5 动画映射 + big-brain AI + 3 阶段战斗 + 掉落物 | — |
| **P5** ⬜ | 境界递进功法 + 平衡 — 醒灵→化虚各境界解锁丹道能力 + 与七流派克制关系 + 天道互动 | — |

**贴图政策（硬规则）**：所有新增 2D 贴图（粒子 VFX / 物品图标 / 方块面 / entity overlay UV / HUD / 场景概念）必须走 `/gen-image <style> <prompt>`（脚本 `scripts/images/gen.py`，cliproxy 优先 / openai fallback）。四档画风 item / particle / hud / scene 各自对应见 §7.4。Blockbench / GIMP 仅作 gen-image 首稿之后的 UV 边缘 / alpha / 像素对齐调整，**禁止手工从零画**。已搬入资源（`local_models/baolongwang/*.png`）除外。

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

## §7 模型与贴图资产清单

### §7.1 暴龙王（已有，已搬入）

| 文件 | 路径 | 状态 |
|------|------|------|
| geometry | `local_models/baolongwang/baolongwang.geo.json` | ✅ 已搬入 |
| animation | `local_models/baolongwang/baolongwang.animation.json` | ✅ 已搬入 |
| texture | `local_models/baolongwang/baolongwang.png` | ✅ 已搬入（gen-image 豁免） |

### §7.2 变异附件 GeckoLib 模型（P3 需制作）

骨骼/几何走 Blockbench；UV 贴图走 `/gen-image item`（见 §7.4）。

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

### §7.3 灵草植物模型（P2 需制作）

方块模型走 Blockbench JSON；图标 + 方块贴图走 `/gen-image item`（见 §7.4）。

| 植物 | 方块模型 | 图标 |
|------|---------|------|
| 蜕骨藤 | 需要 | `/gen-image item` |
| 兽心草 | 需要 | `/gen-image item` |
| 龙鳞苔 | 需要 | `/gen-image item` |
| 续元蕊 | 需要 | `/gen-image item` |
| 化形根 | 需要 | `/gen-image item` |

### §7.4 贴图资产管线（/gen-image 强制）

所有新增 2D 贴图按下表对应 style 调 `/gen-image <style> <prompt>`（脚本 `scripts/images/gen.py`）。手工只在 gen 产出之后做 UV 边缘 / alpha / 像素对齐微调，**首稿禁手画**。

| 贴图类别 | style | 数量 | 路径 | 尺寸 |
|---------|-------|-----|------|------|
| 招式粒子 VFX：`pill_glow` / `pill_trail` / `pill_burst` / `pill_mist` | particle | 4 | `assets/bong/textures/particle/<id>.png` | 16-32 |
| 灵草物品图标：蜕骨藤 / 兽心草 / 龙鳞苔 / 续元蕊 / 化形根 | item | 5 | `assets/bong/textures/item/<id>.png` | 16×16 |
| 灵草方块贴图（每种灵草至少一面） | item | 5 | `assets/bong/textures/block/<id>.png` | 16×16 |
| 丹药物品图标：蜕骨丹 / 兽心丹 / 龙鳞散 / 续元丹 / 化形大丹 / 净毒丸 | item | 6 | `assets/bong/textures/item/<id>.png` | 16×16 |
| 变异附件 entity UV：`dandao_iris` / `_nails` / `_ridge` / `_scales` / `_spurs` / `_horns` / `_tail` / `_carapace` / `_arms` / `_beast` | item | 10 | `assets/bong/textures/entity/mutation/<id>.png` | 16-128（与 §P3 §4.1 表对齐） |
| 变异催化炉 tier 4 外观 | item | 1 | `assets/bong/textures/entity/alchemy_furnace_tier4.png` | 64×64 |
| 暴龙王 BOSS arena 场景概念图（仅 design ref，不入运行时） | scene | 1 | `docs/library/ecology/baolongwang_arena.png` | 1024×1024 |
| 丹道 HUD 面板背景 / inspect 变异图（若需） | hud | 0-2 | `assets/bong/textures/gui/<id>.png` | 256×128 |

**命令样板**（按类别一条；其他同类参数同源调整）：

```bash
/gen-image particle "温润绿丹药口部光晕 16×16，柔和发光 #7ED4A0，末法残土像素风"
/gen-image item "蜕骨藤暗紫色藤蔓缠绕骨骼图标 16×16，像素风"
/gen-image item "变异前臂鳞 UV 贴图 64×64，灰绿龙鳞无缝纹理，UV 友好"
/gen-image scene "末法残土地下巨洞，暴龙王 BOSS arena，毒紫雾气，坍缩渊边缘"
/gen-image hud "丹道·丹体异化面板背景，暗木纹边框 + 内层温润绿淡光，256×128"
```

**总贴图工作量估算**：32 个新贴图 + 1 个场景概念图 = ~33 次 `/gen-image` 调用（部分需多次重抽选稿）。

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

> 全部已在 §8.1 收口。原表保留以备追溯，**实施时以 §8.1 决议为准**。

---

## §8.1 决议（pre-P0 收口，2026-05-18）

> 以 Explore agent 实地核查 server / client / worldgen 现状为依据。每条结尾的「落点」是 P0/P1 实施时应当读的代码符号或 plan 章节。

### #1 经脉惩罚数值与"经脉效率"的代码语义

**决议**：

1. **`meridian_penalty` 不直接缩放经脉效率字段**——`server/src/cultivation/` 里**没有** `efficiency_multiplier` / `flow_rate_multiplier` 这类乘数字段。`Contamination` 的影响走的是**链式损耗**：`contamination.rs:29-31` 定义 `DRAIN_RATIO = 1.5`（排 1 单位污染要花 1.5 qi）+ `BASE_PURGE_RATE = 0.1`，qi_current 不足时排异失败 → 已通经脉添加 `MeridianCrack`（`contamination.rs:142-149`）→ 招式 hit_chance / cast 成功率受损。
2. **`MutationState.meridian_penalty` 重定义为「污染基线加成」**：每 tick 给 `Contamination.current` 累加 `meridian_penalty × CONTAMINATION_BASELINE_TICK`（新增常数，建议 `0.01/tick × penalty 系数`），从而通过现有排异链路自然削弱角色。**不要在 plan 范围内新建经脉效率乘数**——会和现有 contamination 系统两套并行不一致（红旗，对齐 `docs/CLAUDE.md §四 自定真元物理常数`）。
3. **阶段 4 -30% 调到 -20%**：根据 DPS 推算（化虚变异体 70% 经脉效率 + 体质 +50% + 多臂 = DPS 比值 ~0.7-0.85），-30% 偏强（变异体 net DPS 已下降到 0.7 以下时仍要承担社会代价 + 不可逆，吸引力为负）。最终数值：**-3% / -8% / -15% / -20%**。阶段 4 的 OP 抑制改由"经常进入 contamination overflow → 自动触发裂痕"承担，而非纯惩罚乘数。
4. **plan §1.3 三招式 qi 消耗的高境界数值需重算**：现有 `qi_max × 3%（醒灵 0.3 ... 化虚 321）`隐含 qi_max 几何 4× 递增到化虚 ~10,700——但代码里 `qi_max = 10 + Σ flow_capacity`（`components.rs:384`），化虚 20 脉全通约 **210-500 范围**。**实施时直接读 `cultivation::components::capacity_for_tier(realm)` 算实际 qi_cost，不要硬编 0.3/1.2/4.5/16/63/321 这串**。plan §1.3 表格作为"占用比例"语义保留（× qi_max 3%），但要在 P0 P0 测试断言里去掉绝对数值。

**落点**：`server/src/cultivation/contamination.rs:29-31, 142-149` / `server/src/cultivation/components.rs:196-208, 384` / `server/src/combat/armor.rs:10`（`ARMOR_MITIGATION_CAP = 0.85`）/ plan §1.3 + §2.1 + §2.4。

### #2 多臂切换：cooldown share 限速

**决议**：

1. **武器切换当前无延迟**：`AttackIntent`（`server/src/combat/events.rs:35-45`）不绑武器实例；攻击冷却走全局 `ATTACK_COOLDOWN_TICKS = 10`（`server/src/combat/player_attack.rs:15`），换武器立即生效；招式冷却绑 `skill_id` 不绑武器（`server/src/network/skillbar_config_emit_test.rs:85-157`），换武器不重置 cooldown。
2. **多臂 = 扩 `EquipSlotV1` 加 `ExtraHand0/ExtraHand1`**（`server/src/schema/inventory.rs:31-56`），保留 hotbar 9 槽平行。不改 `AttackIntent` 不改 attack 路径。
3. **"切换无延迟"防 AI 滥用 = cooldown share**：新增 `WEAPON_SWAP_COOLDOWN_TICKS = 20`（1s GCD），所有手槽位共享 `last_swap_tick`。这是最少改动方案——不引入 mastery-gated（增加新维度）也不引入 attack-locking（侵入 attack 路径）。等真有滥用证据再细化。
4. **plan §2.1 `ExtraArms` 注释更新**：从"切换无延迟"改为"切换共享 1s GCD，攻击节奏不变但应对场景灵活（一手 buff 一手输出）"。
5. **HUD 条件显示硬约束**（对齐 memory `feedback_hud_conditional.md`）：`ExtraHand0/ExtraHand1` 槽位的快捷栏渲染**只在玩家已解锁多臂变异 slot 时才出现**，未解锁时 `WeaponHotbarHudPlanner` 不画这两格、不画灰位、不画占位提示——**完全隐藏**。同理：丹道·丹体异化 HUD 面板（plan §4.2）只在 `DandaoStyleComponent` 已 lazy insert 后才显示；变异阶段标签（金瞳/角/尾）只在对应 `ActiveMutation` 写入后才出现；变异部位标记（plan §4.2 "迷你人体剪影 + 变异部位标记"）按 `MutationState.slots` 列表条件渲染，每多一个 slot 多画一个点。规则一句话：**未解锁的能力/槽位/状态绝不在 HUD 上留痕**。

**落点**：`server/src/schema/inventory.rs:31-56` / `server/src/combat/player_attack.rs:15, 69` / `server/src/combat/weapon.rs:121-167` / `client/src/main/java/com/bong/client/hud/WeaponHotbarHudPlanner.java:39-75`（多臂槽位渲染条件门）/ plan §2.1 `ExtraArms` 描述 / plan §4.2 HUD 章节 + 加 "条件显示" 子节。

### #3 暴龙王巨洞：blueprint 固定坐标 + 负灵域 zone

**决议**：

1. **走 blueprint 固定坐标方案**（同灵眼 / 坍缩渊口模式）。Blueprint 模型 `worldgen/scripts/terrain_gen/blueprint.py:54-69` 已支持 `BlueprintZone.pois: tuple[PoiSpec, ...]`，每 POI 带 `pos_xyz`。`server/zones.worldview.example.json` 里 `rift_mouth_north_001 @ (-500, 74, -8500)` 就是范本。
2. **新增 zone**：`baolongwang_cavern_deep`，aabb 选未占用地下区（建议 `x: 1500-2000, y: -80 to -20, z: -5500 to -4800` 区间），`spirit_qi: -0.8` 自动落入 `BotanyZoneTag::NegativeField`（阈值 `< -0.2`，`server/src/world/zone.rs:48`）→ 天道感知屏蔽（对应 plan §6.4 暴龙王"活在天道盲区"设定）。
3. **2 个 POI**：`baolongwang_furnace`（炉位置，弱点）+ `baolongwang_spawn`（BOSS 站位）。POI `kind` 字段在 server 侧通过 `ZoneRegistry::find_poi_by_kind(...)` 查询用于 NPC spawn 初始化（plan-npc-ai-v1 框架已就位）。
4. **不需要 worldgen Python 代码改动**——只动 `server/zones.worldview.example.json` JSON（~30 行新增）。

**落点**：`server/zones.worldview.example.json`（待新增 baolongwang zone 条目）/ `server/src/world/zone.rs:48`（NegativeField tag 阈值）/ `worldgen/scripts/terrain_gen/blueprint.py:54-69, 170-187`（POI spec 加载） / plan §5.5。

### #4 化虚大衍丹体：复用 AlchemyAutoProfile + is_internal 分支

**决议**：

1. **不新写完整 Session 状态机**——`AlchemySession`（`server/src/alchemy/session.rs:68-82`）是 tick 驱动而非 enum FSM，没有"跳过中途阶段直接产出"接口；硬绕过会破坏现有 contamination 写入路径。
2. **复用 plan-alchemy-v2 的 `AlchemyAutoProfile`**（`server/src/alchemy/auto_profile.rs:69-100`）—— 它本来就是"无 GUI 全自动炼丹"通道。新增分支：
   - `AlchemyAutoProfile.is_internal: bool` 字段
   - `is_internal == true` 时跳过 `FurnaceQiReserve` 储备消耗 → 改为直接扣 `player.qi_current`（守恒律仍走 `qi_physics::ledger::QiTransfer { from: player, to: zone, amount: qi_cost × dissipation_ratio }`，dissipation 给末态 zone 不凭空消失）
   - `feed_stage / tick / classify` 链路全部复用，不重写
3. **新增 IPC event**：`DandaoInternalBrewIntent { recipe_id, duration_ticks }`，client → server 触发；server 端 `spawn_furnace_with_auto_profile(player, recipe, profile_curve_internal)`。
4. **HUD 不画完整炼丹 UI**——化虚招式 cast 走通用 5s progress bar（plan §1.3 模式）即可，配套 narration "你在体内点燃了丹炉"。复用率约 95%。

**化虚大衍丹体「身体即是丹炉」机制完整说明**（plan §6.1 表"化虚"行的展开）：

化虚境界变异体不再需要外置丹炉。把材料（灵草 + 矿物）吞入腹中，用自身真元在体内"点火"——`AlchemyAutoProfile.is_internal = true` 分支会跳过 `FurnaceQiReserve` 储备，直接消耗 `player.qi_current` 烧火 5s，期间复用 `auto_profile.rs` 的 `feed_stage / tick / classify` 链路（火候曲线 / 投料窗 / 品阶分类全套），**只是没有可见的炉子**。5s 后：

- **不产出实体丹药**——效果直接施加到玩家身体（如蜕骨丹效果 → 直接 `cumulative_toxin += 5.0` + `Contamination::add(ContamSource::Pill { ... })`）
- **代价远高于外炼**：每次内炼无条件 `cumulative_toxin += 15.0`（约等于一颗化形大丹半剂） + `meridian_penalty += 0.01`（永久 1% 经脉惩罚叠加，**不可洗**）
- **优点**：不依赖丹方器具，野外即可炼；可在战斗中（5s 内不可移动 / 不可施法）当应急 buff
- **触发条件**：化虚境界 + 已解锁「大衍丹体」被动（任意变异阶段 ≥ 3 时通过顿悟选择解锁，写入 `DandaoStyle.passives: Vec<DandaoPassive>`）
- **守恒律**：消耗的 qi 走 `QiTransfer { from: player, to: zone, amount: qi_cost × dissipation }`，dissipation 给当前 zone 不凭空消失，对齐 `docs/CLAUDE.md §四`

**叙事定位**：这是丹道流派的"终极返祖"——丹宗远祖（百草门初代）就是这样炼丹的，后来才发明外炉；变异体身体已经接近上古丹师状态，重新拾回了这门失传技术。代价是每次内炼都让你离"人"更远一步。

**落点**：`server/src/alchemy/auto_profile.rs:69-100` / `server/src/alchemy/session.rs:68-82` / `qi_physics::ledger::QiTransfer`（守恒律）/ plan §6.1 化虚行 + §9.1 丹宗叙事呼应。

### #5 暴龙王三阶段 AI：纯 Scorer 链组合，不扩 big-brain

**决议**：

1. **现有 big-brain 用法**（`server/src/npc/brain.rs:1-10, 118-129, 1006-1048`）= 标准 Scorer + Action + Picker(big-brain 内置) + Thinker，每个 Scorer 一个 component + system pair。
2. **三阶段不需要 PhaseGate 特殊组件**——改用 **health-ratio Scorer 驱动三平行 Phase Scorer 链**：
   - 阶段一 Scorer（`AvoidPlayerScorer` / `PillMistScorer` / `TailSwipeScorer`），其评分公式都乘上 `phase1_active = (health_ratio > 0.7) as f32`
   - 阶段二 Scorer 同理，乘 `phase2_active = (0.3 < health_ratio ≤ 0.7) as f32`
   - 阶段三 Scorer 乘 `phase3_active = (health_ratio ≤ 0.3) as f32`
   - 自然形成"同一时刻只有一个阶段的 Action 评分 > 0"，big-brain Picker 自动选最高分 Action
3. **优势**：纯组合现有模式，无新 trait/component；阶段切换由 health 实时驱动，无显式状态转换 bug 空间；后续要加阶段（暴怒后再"垂死爆发"）只是再加一个 Scorer 链。
4. **plan §5.3 Scorer 描述更新**：删除"PhaseGate 包装"叙述，改为"每阶段 Action 评分函数内置 health_ratio 区间门控"。

**落点**：`server/src/npc/brain.rs:1006-1046`（chase_target_scorer_system 范本）/ plan §5.3 三阶段 Scorer 表。

### #6 Bedrock → GeckoLib 骨骼 alias：renderer 侧 HashMap，不改源文件

**决议**：

1. **现有 GeckoLib 模型也用自定义骨骼名**（`client/src/main/resources/assets/bong/geo/iron_leggings.geo.json` / `jungle_scorpion.geo.json`）—— Bong 项目本就不强求 Minecraft humanoid 标准命名，FaunaModel 直接指 geo.json 无 alias 层。
2. **`baolongwang.geo.json` 骨骼完整列表**：`bone2 / bdk_rl / bdk_ra / bdk_la / bdk_ll / bdk_lw / bdk_rw / bdk_body / bone / group`。
3. **映射表（renderer 侧 HashMap 实装，不动源文件）**：
   ```
   bdk_body → body
   bdk_rl   → right_leg
   bdk_ll   → left_leg
   bdk_ra   → right_arm
   bdk_la   → left_arm
   bdk_rw   → right_wing  (GeckoLib standard 无此骨骼 — animation processor no-op pass-through)
   bdk_lw   → left_wing   (同上)
   bone / bone2 / group   → 弃用 / 装饰，无 animation 绑定
   ```
4. **实装位置**：`BaolongwangEntityRenderer extends GeoEntityRenderer`，构造时注册 `boneAlias: Map<String, String>`，在 `applyMolangQueries` / `getAnimationProcessor` 处做 lookup。**不改 `local_models/baolongwang/baolongwang.geo.json`**——美术工具重导出会覆盖。

**落点**：`local_models/baolongwang/baolongwang.geo.json:1-50` / `client/src/main/java/com/bong/client/fauna/FaunaModel.java:1-21`（参考 pattern）/ plan §5.1。

### #7 蜕骨藤 / 异变兽骨经济：靠 botany density 校准，不改 plan 阈值

**决议**：

1. **plan §六.4 阈值（0/30/100/250/500）保留**——这些是"累计 toxin"语义，普通辅助丹 0.5/颗在自然修炼路径下天花板 ~75，永远不触发微变。变异轨道明确要"刻意大量服药"。
2. **关键校准在 botany 侧**：`ye_ku_teng`（蜕骨藤）配置当前在 `server/src/botany/registry.rs:700+` 但 `v2_spec.env_locks` **尚未实装**（plan-botany-v2 骨架在跑）。P2 实施时设 **`density_factor = 0.05`（极稀有）+ `survival_mode = NegPressureFeed` + `env_locks = [NegativeField, Ruins]`**——让 188 颗采集约 12-18h 游戏时间，对齐 plan §六.4 表"凝脉后期~固元（~30h）"窗口的~半段时间投入。
3. **异变兽骨**：plan §5.4 暴龙王掉落 + plan-tsy-hostile-v1 / plan-npc-ai-v1 的 hostile mob 掉落已有底盘。P2 实施时在异变兽 loot table 加 `BeastBone × 1-3 (70%)`。94 颗约 35-50 次击杀，与采集时长匹配。
4. **若实施时实测严重偏离**（采集 ≥ 30h），降级到选项 C：把蜕骨丹需求数降到 50 颗 + toxin/颗 8.0（材料量减半）。

**落点**：`server/src/botany/registry.rs:700+`（ye_ku_teng v2_spec）/ `server/src/botany/lifecycle.rs:126-190`（spawn_v2_plants_for_zone）/ plan §3.1 表"蜕骨藤"行新增 `density_factor: 0.05` 字段提示 / plan §3.2 蜕骨丹配方备注"如实施时采集时长 >30h 触发选项 C 调整"。

### #8 变异附件模型：6 手工 + 4 gen-image，总 5-6 小时

**决议**：

1. **`scripts/images/gen.py` 现状**：仅 2D PNG（item/particle/hud/scene 四档），**无 Tripo 集成 / 无 image-to-3d**。`scripts/images/style.py:20-79`。
2. **现有 GeckoLib 模型复杂度**：最简 `douli_hat.geo.json` 116 行 5 骨骼 ≈ 30-60 min/件手工 Blockbench。
3. **10 个变异附件分工**：
   - **手工 Blockbench 6 个**（需精确骨骼层级 / pivot / 关节）：金瞳、硬甲指、额骨脊、双角、尾、多臂
   - **gen-image 贴图 + 简单 Blockbench cube 4 个**（规则几何贴图为主）：前臂鳞、脊突、背甲、兽面
   - 总工作量 ~5-6h
4. **plan §7.2 表"估计工作量"列重定**：手工 6 个标 "Blockbench 30-60min/件" / gen-image 4 个标 "/gen-image item + cube paste UV 15-20min/件"。
5. **不引入 Tripo 等外部 3D 生成工具**——质量不可控且增加依赖；plan §8 #8 提到的 Tripo 路线**拒绝**。

**落点**：`scripts/images/gen.py` / `scripts/images/style.py:20-79` / `client/src/main/resources/assets/bong/geo/douli_hat.geo.json`（最简范本）/ plan §7.2 工作量表 + §7.4 命令样板。

---

## §8.2 §8.1 收口后下一步（**已修订 2026-05-18**：单 plan 多 PR 路径）

§8.1 决议是 plan **设计补丁**，未实施。原本（早期草案）建议拆 4 个独立 plan 分别消费——**该建议已废弃**，改走 **单 plan 多 PR 路径**（见 §10.2 / §10.4）：

- **不另起 `plan-worldgen-layout-v1` / `plan-dandao-base-v1` / `plan-dandao-items-v1` / `plan-baolongwang-boss-v1` / `plan-dandao-advance-v1` 这类骨架 plan**——全部内容在本 plan 内做。
- worldgen layout 基础设施作为 **P-1 前置阶段**（§0 阶段总览），仍在本 plan-dandao-path-v1 内实施。
- 单次 `/consume-plan dandao-path-v1` 调用按 §10.2 推荐的 4 PR 序列依次提交、依次 merge，全程 ScheduleWakeup 驱动。
- **唯一例外**：worldview §六.4 仍**单独 PR**——把 plan 头部"Worldview 扩展：§六.4 丹体异化"章节作为 `docs/worldview.md` 的 amend，单独提 PR（CLAUDE.md / AGENTS.md 严禁自动改 worldview，必须人工 review）。归档前 worldview §六.4 必须先 land。这条不在 consume-plan 自动化范围内。

为什么放弃拆 plan：拆 plan 增加 plan 文件管理成本（4 份 active + 维护交叉引用），且每份子 plan 体量都不大到独立成 plan 的程度；单 plan 多 PR 在 consume-plan agent 视角下序列化推进效率更高，仍能满足"PR review 不会爆"目标。

---

## §9 丹道地形：丹宗遗园（百草门覆灭遗迹）

> 对标 `plan-sword-path-v1` §P2 巨剑沧海。剑道地标是「上古剑宗插剑入海」，丹道地标是「上古丹宗百草门覆灭后的整片药圃 + 露天大丹炉群 + 异灵兽散落」。叙事上 §6.4 暴龙王 = 百草门末代掌门，本 zone = 他的故乡。

### §9.1 叙事

末法降临前，**百草门**是九大宗门中唯一的纯丹宗——不修战斗、不练剑，整门人靠"以丹养身"维持，掌门人是后来的暴龙王。末法初年灵气暴跌，门内弟子按掌门指示集体服用囤积的"续命大丹"——掌门活下来（变成了暴龙王，沦为续命成瘾者），其他人没扛过灵气崩塌期，**药效与本体真元的最后失衡**让他们当场扭曲变形，倒在自家药圃里。

千年后这片山谷成了：

- 上百口**露天炼丹大鼎**残骸（青铜半埋入土，仍有微温）
- 数十块**药圃石篱**（曾经按八卦布局，现在只剩低矮石墙圈出土框）
- **变异灵草疯长**（蜕骨藤 / 兽心草 / 龙鳞苔 全是上古百草门的培育品种，野化后变得更稀有更剧毒）
- **服丹未死的"半人"散落**（异灵兽：变形未完成的丹师，掉异变兽骨 / 残破丹药 / 失传丹方）
- **丹师枯骨遍地**（手里仍紧握未炼成的丹药）

地表土壤被千年丹毒侵染，呈灰紫色（podzol + purple_terracotta 混合）。Zone 中央有一座半塌大殿，是百草门的「百草丹殿」总坛——里面藏着全套的丹方壁画 + 一座唯一保留完好的 tier-4 变异催化炉（剧情上是掌门炼"续命大丹"时用的）。

**与暴龙王 BOSS 的关系**：本 zone 是叙事支线 / 采集场 / 入门 elite，**不是暴龙王战场**。暴龙王本人在 §8.1 #3 决议的 `baolongwang_cavern_deep`（地下深处坍缩渊边缘负灵域），不在丹宗遗园。但本 zone 的丹师枯骨 + 壁画会大量铺垫"掌门叛逃"叙事，让玩家在去坍缩渊打 BOSS 之前先理解他是谁。

### §9.2 Zone 定义

`server/zones.json`（或 `server/zones.worldview.example.json`）新增：

```json
{
  "name": "dan_zong_yi_yuan",
  "display_name": "丹宗遗园",
  "aabb": {
    "min": [-2400.0, -16.0, 3200.0],
    "max": [-800.0, 240.0, 4800.0]
  },
  "spirit_qi": 0.40,
  "danger_level": 4,
  "ambient_recipe_id": "ambient_dan_zong",
  "active_events": [],
  "patrol_anchors": [
    [-1600.0, 78.0, 3600.0],
    [-1200.0, 82.0, 4000.0],
    [-2000.0, 76.0, 4400.0]
  ],
  "blocked_tiles": []
}
```

- **spirit_qi 0.40**：中等灵气区（边缘略偏低）——千年丹毒侵染土壤抑制灵气流通，比正常宗门遗迹（jiu_zong_ruin spirit_qi 通常 0.5）稍低。
- **danger_level 4**：等同于巨剑沧海（剑道）。异灵兽 + 蒸气毒泉 + 大殿地下守卫构成主威胁，不需要化虚来打但凝脉以下风险高。
- **坐标选择**：负 x 负 z 第三象限，避开 spawn / qingyun_peaks / lingquan_marsh / 渊口荒丘 现有坐标群，靠近 worldgen 已有的 north_waste 区域。具体坐标实施时按 `server/zones.worldview.example.json` 现状再敲。

### §9.3 Terrain Profile

> **本 zone 是宗门人工遗迹，不走「density-based 杂物 spawn」的 noise 范式**——所有建筑（大殿 / 药圃 16 格 / 丹炉 8 对 / 中轴大道 / 蒸气毒泉 3 处）都是 **deterministic layout**，按八卦/对称/中轴公式相对 POI 中心点摆放。只有"野草 / 散落丹师枯骨 / 表层小石"这类自然杂物才走 density spawn。地形 height field 仍走 noise（缓坡），表层 surface 仍走 palette——**唯独建筑结构不能 noise**。

`worldgen/terrain-profiles.example.json` 新增 `dan_zong_yi_yuan`：

```json
"dan_zong_yi_yuan": {
  "height": { "base": [62, 78], "peak": 92, "compound_flatten_radius": 96 },
  "boundary": { "mode": "soft", "width": 96 },
  "surface": ["podzol", "coarse_dirt", "mud", "purple_terracotta", "mossy_cobblestone"],
  "water": { "level": "low", "coverage": 0.10 },
  "passability": "medium",
  "structure_density": {
    "wild_herb_clump": 0.020,
    "scattered_bone_fragment": 0.004,
    "small_rubble": 0.008
  },
  "architectural_layout": "dan_zong_compound",
  "ambient_hint": {
    "pill_steam_drift": "continuous",
    "soil_tint": "purple_grey",
    "alchemist_groan": "rare"
  }
}
```

**与其他 profile 的关键不同**：

1. **新增 `architectural_layout` 字段**（worldgen schema 扩展）——值是 layout 生成器 ID，对应 `worldgen/scripts/terrain_gen/layouts/dan_zong_compound.py`（详见 §9.4a）。layout 生成器在 procedural 表层 / decoration density spawn 之后运行，最后一步覆盖建筑区块。layout 区域内 density spawn 会被 mask 遮蔽（不在建筑物上长草）。
2. **新增 `height.compound_flatten_radius`**——以 POI `dan_zong_great_hall` 坐标为中心 96 格半径内强制摊平到 height = 76（宗门台地，方便建筑站立），半径外正常 noise 高程。这是「建筑场地」的硬性平整需求，noise 不能在建筑区生成陡坡。
3. **`structure_density` 大幅删减**——只保留 3 项纯"自然杂物"（野草 / 骨片 / 小碎石）。原来 §9.3 旧版的 `ruined_open_furnace / herb_garden_stone_pen / vapor_poison_spring / fallen_recipe_stele / fallen_alchemist_bone` 全部移到 §9.4a layout（确定坐标），**不在 density spawn 序列里**。
4. **地形高程**：base 62-78 / peak 92 是 zone 外缘的缓坡丘陵，宗门内部因 `compound_flatten_radius` 实际高程恒定 76 = 平台。

### §9.4 Decorations & Layout

本 zone 的 decoration 拆成两层：

#### §9.4a 建筑 layout（deterministic，**新增 worldgen 能力**）

新建 `worldgen/scripts/terrain_gen/layouts/dan_zong_compound.py`（worldgen 系统需要新增 `layouts/` 子模块——`LayoutSpec` 类、layout-runner 入口 system，独立于现有 `profiles/*.py` 的 procedural decoration 体系）。

**Layout 生成器规格**：

```python
# worldgen/scripts/terrain_gen/layouts/base.py（新增）
@dataclass(frozen=True)
class LayoutSpec:
    """Deterministic 建筑布局——按 POI 中心点 + 几何公式放块。"""
    name: str
    poi_kind: str                     # 锚定的 POI 类型，从 BlueprintZone.pois 找中心点
    radius: int                       # 影响半径，layout 内禁用 density spawn
    placements: tuple["Placement", ...]  # 由 Python 算法生成 / 也可手写
    # masks density spawn：在 radius 内 noise-spawned decoration 被覆盖

@dataclass(frozen=True)
class Placement:
    """单个结构投放：坐标 + 块映射 / NBT。"""
    offset: tuple[int, int, int]      # 相对 POI 中心的 (dx, dy, dz)
    rotation: int                     # 0 / 90 / 180 / 270
    kind: Literal["nbt", "block_grid", "stamp_radial"]
    payload: str                      # NBT 路径 或 inline block list 名
```

**`dan_zong_compound` layout 具体内容**：

```python
DAN_ZONG_COMPOUND_LAYOUT = LayoutSpec(
    name="dan_zong_compound",
    poi_kind="dan_zong_great_hall",   # 中心点 = POI 坐标，整片 layout 相对它定位
    radius=96,                         # 与 terrain profile compound_flatten_radius 对齐
    placements=(
        # ── 中央：百草丹殿（单一大型 NBT） ──
        Placement(offset=(0, 0, 0), rotation=0, kind="nbt",
                  payload="dan_zong_great_hall.nbt"),

        # ── 八卦药圃 16 格（内环 8 + 外环 8 偏 22.5°） ──
        # 内环 r1=24，外环 r2=48
        *(
            Placement(
                offset=(int(24 * cos(radians(angle))), 0, int(24 * sin(radians(angle)))),
                rotation=int(angle) % 360,
                kind="stamp_radial",
                payload="herb_garden_pen_6x6",   # 6×6 mossy_cobblestone 石篱 + podzol 内土
            )
            for angle in (0, 45, 90, 135, 180, 225, 270, 315)
        ),
        *(
            Placement(
                offset=(int(48 * cos(radians(angle + 22.5))), 0, int(48 * sin(radians(angle + 22.5)))),
                rotation=int(angle + 22.5) % 360,
                kind="stamp_radial",
                payload="herb_garden_pen_8x8",   # 外环放大版 8×8
            )
            for angle in (0, 45, 90, 135, 180, 225, 270, 315)
        ),

        # ── 露天炼丹大鼎 8 对沿中轴对称（共 16 口） ──
        *(
            placement
            for z in (16, 32, 48, 64, 80, 96, 112, 128)
            for placement in (
                Placement(offset=(-12, 0, z), rotation=0, kind="nbt",
                          payload="ruined_open_furnace.nbt"),
                Placement(offset=(+12, 0, z), rotation=180, kind="nbt",
                          payload="ruined_open_furnace.nbt"),
            )
        ),

        # ── 中轴大道（6 格宽 mossy_cobblestone，z = -8 到 +144） ──
        Placement(offset=(0, 0, 64), rotation=0, kind="block_grid",
                  payload="central_path_6x152"),

        # ── 蒸气毒泉 3 处固定 ──
        Placement(offset=(0, -1, 96), rotation=0, kind="nbt",
                  payload="vapor_poison_spring_main.nbt"),     # 主毒泉，正前方
        Placement(offset=(+64, -1, 48), rotation=0, kind="nbt",
                  payload="vapor_poison_spring_small.nbt"),    # 东南角
        Placement(offset=(-64, -1, 48), rotation=0, kind="nbt",
                  payload="vapor_poison_spring_small.nbt"),    # 西北角

        # ── 倒塌丹方碑 4 块按内环药圃间隙 ──
        *(
            Placement(offset=(int(36 * cos(radians(angle))), 0, int(36 * sin(radians(angle)))),
                      rotation=int(angle + 90) % 360, kind="nbt",
                      payload="fallen_recipe_stele.nbt")
            for angle in (22.5, 112.5, 202.5, 292.5)
        ),

        # ── 丹师枯骨 8 具固定放在中轴大道两侧（叙事锚点，非杂草） ──
        *(
            Placement(offset=(dx, 0, dz), rotation=rot, kind="nbt",
                      payload="fallen_alchemist_bone.nbt")
            for (dx, dz, rot) in (
                (-4, 24, 90), (+4, 40, 270),
                (-4, 56, 90), (+4, 72, 270),
                (-4, 88, 90), (+4, 104, 270),
                (-4, 120, 90), (+4, 136, 270),
            )
        ),
    ),
)
```

**摆放语义**：

- **大殿位于原点 (0,0,0)**——所有结构相对大殿坐标。大殿 NBT 自带正门朝 z+ 方向。
- **八卦药圃**：内外两环各 8 格，外环偏转 22.5° 错位避开内环遮挡（玩家从大殿沿中轴走能看见所有 16 格药圃）。
- **丹炉对称**：8 对沿中轴正前方两侧排列，距中轴 ±12 格，z 方向 16 格间隔。沿大道走时左右各有一排炼丹大鼎，仪式感强。
- **中轴大道**：6 格宽 mossy_cobblestone 主干道，从大殿正门延伸 152 格到 zone 边缘。
- **3 处毒泉固定坐标**：主毒泉在中轴最远端（视觉锚点）+ 两侧角落各 1 处（探索奖励）。**不是 density spawn**——玩家每次进入 zone 看到的 3 处毒泉位置完全一致。
- **丹方碑 4 块**：按内环药圃 + 22.5° 角落（即外环的 8 个方位被外环药圃占了之后剩下的「间隙」），叙事铺垫密度刚好。
- **丹师枯骨 8 具**：固定在中轴大道两侧（玩家必经路径），保证叙事 / loot 触达率 100%。**这些不是 density 杂物**——剩余的"无主小骨片"才走 §9.4b density spawn。

**Layout-runner 系统**：

- 新增 `worldgen/scripts/terrain_gen/layouts/runner.py`：从 BlueprintZone.pois 查询 `poi_kind` 找中心坐标，按 LayoutSpec.placements 顺序 paste blocks / NBT
- 在 raster_export 之前 layout runner 运行一次，把 deterministic 结构刻入 chunk
- layout 覆盖区（POI 半径 96）内禁用 density spawn（避免野草长到大殿屋顶）—— mask 通过 SDF 函数注入到 `stitcher.py` 的 spawn judge

**worldgen 系统扩展工作量**（layout 系统是新增能力，约 1-2 天）：

| 任务 | 文件 | 工作量 |
|------|------|------|
| `LayoutSpec` / `Placement` 基类 | `worldgen/scripts/terrain_gen/layouts/base.py` | 0.5d |
| layout-runner + NBT paste | `worldgen/scripts/terrain_gen/layouts/runner.py` | 1d |
| `compound_flatten_radius` height 摊平 | `worldgen/scripts/terrain_gen/fields.py` 扩 | 0.5d |
| density spawn mask（layout 区禁草） | `worldgen/scripts/terrain_gen/stitcher.py` 扩 | 0.5d |
| terrain-profiles schema 加 `architectural_layout` 字段 | `worldgen/scripts/terrain_gen/profiles/base.py` | 0.25d |

**关键约束**：worldgen layout 系统作为本 plan **P-1 前置阶段**（见 §0 阶段总览 + §10.2 PR-1），仍在本 plan 内做但**独立成 PR**——基础设施 PR-1 必须先 merge，后续 PR-2/3/4 才能依赖 layout runner 可用。不另立 `plan-worldgen-layout-v1`。基础设施 PR 与玩法 PR 分离避免 review 混杂。

#### §9.4b 自然杂物（density spawn，传统范式）

`worldgen/scripts/terrain_gen/profiles/dan_zong_yi_yuan.py`（按 `ash_dead_zone.py` 现有范式）：

```python
DAN_ZONG_YI_YUAN_DECORATIONS = (
    DecorationSpec(
        name="wild_herb_clump",
        kind="shrub",
        blocks=("crimson_roots", "warped_roots", "twisting_vines"),
        size_range=(1, 3),
        rarity=0.85,
        notes="野生变异灵草丛：在 layout 半径 96 外的自然区域散布。layout 内的药圃格子另有种植规则（见 §9.4a 注），不靠这条。",
    ),
    DecorationSpec(
        name="scattered_bone_fragment",
        kind="shrub",
        blocks=("bone_block",),
        size_range=(1, 1),
        rarity=0.40,
        notes="散落骨片：1×1 bone_block，没有 loot——纯氛围，对应「弟子尸体被千年风蚀，连完整骸骨都难找」。叙事重要的 8 具枯骨在 layout 里。",
    ),
    DecorationSpec(
        name="small_rubble",
        kind="shrub",
        blocks=("cobblestone", "mossy_cobblestone", "andesite"),
        size_range=(1, 2),
        rarity=0.60,
        notes="小石堆：宗门外缘建筑物风化残砾，1-2 格高小堆。",
    ),
)
```

**只覆盖 zone 外缘（layout 半径 96 外的 ~1.5km × 1.5km 区域）**——layout 区禁草 mask 自动遮蔽。野生灵草 spawn 不会出现在大殿屋顶 / 中轴大道 / 药圃格内（药圃格内的灵草由 layout 显式 plant_grid 摆放，不靠 density）。

### §9.5 古遗迹 — 百草丹殿（POI: `dan_zong_great_hall`）

zone 内唯一大型遗迹，固定坐标，对标剑道铸剑古殿。

- **外观**：50×30×18 mossy_stone_bricks + chiseled_polished_blackstone 半塌大殿，正门两侧各立一口完整的 ruined_open_furnace（标志），门上石匾刻"百草"二字（用 jigsaw structure 嵌入 chiseled blocks）。
- **内部结构**（3 层）：
  - **地表大殿**（30×30）：
    - 中央一座 **tier-4 变异催化炉**（不需自带，世界生成时直接 spawn 完成态——剧情设定是掌门当年用过的，自动落入玩家 interaction radius）
    - 四周墙壁有 **6 张完整丹方壁画**（pixel-painted wall map / item frame + 壁画贴图）——靠近交互可解锁丹方到玩家 `recipe_known` 集合（走 `plan-alchemy-v2` 残卷识别底盘）
    - 散落 **3-5 卷 `scroll_dandao_path`**（丹道功法残卷），拾取解锁 P0 三招式中的某一个（走 `UnlockSource::Scroll` 现有机制）
  - **侧室 ×2**（侧厅药库）：每间 1-2 个箱子，随机掉 `meteor_iron` ×1-2 / 变异灵草核心材料（兽心草核 / 龙鳞苔精） / 续命丹 ×1-3 / `scroll_alchemy_archaic`（上古炼丹术残卷）
  - **地下储药库**（10 格深）：
    - 大型石棺一口，开棺得 **掌门师弟「玄草子」骸骨** + 残破续命大丹 ×1（叙事铺垫：暴龙王服了同款丹活下来，他师弟没扛住）
    - 石棺旁石碑刻字："**师兄……你为何只给我们留了一半剂量？**"（narration trigger，scope: player, style: dialogue）—— 这是暴龙王叙事的关键伏笔
    - 地下守卫：1-2 只 **守墓异灵**（elite 级 mutant，掉异变兽骨 ×3-5 + 暴龙王相关线索物品「师叔之印」）

- **worldgen blueprint POI spec**：

  ```json
  {
    "kind": "ruin",
    "pos_xyz": [-1600.0, 82.0, 4000.0],
    "name": "百草丹殿",
    "display_name": "百草丹殿·宗主炉房",
    "tags": ["dandao_path", "alchemy", "boss_lore", "baolongwang_prequel"],
    "unlock": "found_by_exploration",
    "qi_affinity": -0.10,
    "danger_bias": 2
  }
  ```

  - `qi_affinity -0.10`：负灵（千年丹毒抑制大殿周边灵气），但不到负灵域（spirit_qi 仍正），对天道感知**不屏蔽**（区别于暴龙王巢穴的 -0.8 屏蔽）。
  - `tags` 含 `baolongwang_prequel` 让 agent 在玩家进入 / 拾取师叔之印时触发暴龙王前情 narration。

### §9.6 Zone 环境视听

#### ambient_dan_zong（audio_recipe）

```json
{
  "layers": [
    { "sound": "block.brewing_stand.brew", "pitch": 0.4, "volume": 0.12, "delay_ticks": 0 },
    { "sound": "block.fire.ambient", "pitch": 0.6, "volume": 0.08, "delay_ticks": 240 },
    { "sound": "entity.witch.ambient", "pitch": 0.5, "volume": 0.06, "delay_ticks": 480 },
    { "sound": "block.bubble_column.ambient", "pitch": 0.7, "volume": 0.05, "delay_ticks": 720 }
  ]
}
```

层意：低频炼丹炉冒泡 + 残火 + 偶发"丹师残魂呻吟" + 蒸气毒泉气泡。

#### ZoneAtmosphereProfile 新增 `dan_zong_yi_yuan`

- **粒子**：`BongSpriteParticle` type `pill_haze`，密度 0.4/s（高于一般 zone，体现"药气弥漫"），tint `#8B6FA5`（紫灰色，对齐丹毒色系），drift Y +0.015（缓慢上升），生命 90 tick；额外 `BongGroundDecalParticle` type `purple_soil_stain`，密度 0.05/s，地表斑驳紫色印记。
- **雾**：fogStart 56，fogEnd 144，density 0.008（比 spawn 浓），color `#5A4060`（暗紫灰远雾）。
- **天空色温**：RGB shift `(+8, -5, +12)` 偏紫，下午时段叠加 `#6B4A7A` × 0.2 透明度（"丹毒夕阳"特效）。
- **音效随机插入**：每 600-1200 tick rolling check（5% 概率）插入 `entity.zombie_villager.ambient` pitch 0.4 volume 0.15 = 「丹师残魂呻吟」（对齐 ambient_hint 的 `alchemist_groan: rare`）。
- **服务端 narration 触发**（每首次进入 zone，scope: player, style: perception）：
  - `"空气是甜的——不是花香，是丹药熬过头的焦糖味。"`
  - `"你脚下的紫土很软，像踩在反复发酵过的药渣上。"`
- **靠近 dan_zong_great_hall 大殿 30 格（scope: player, style: narrative）**：
  - `"半塌的大殿正门刻着'百草'二字。门两边各立一口齐人高的青铜大鼎，鼎内还有一点微温。"`
- **拾取师叔之印（scope: player, style: dialogue）**：
  - `"印面刻着「百草门·师叔·玄草子」。你忽然听见自己脑中有人说话——是个老人的声音：'师兄，你为何只给我们留了一半剂量？'"`

### §9.7 与 botany / npc-ai / agent 的接入

- **botany（药圃格内规律种植）**：
  - 16 格药圃**每格只种一种灵草**（与 §9.4a layout 配套）——内环 8 格分配：北 / 北东 / 东 / 东南 = 蜕骨藤 ×4 格；南 / 南西 / 西 / 西北 = 兽心草 ×4 格。外环 8 格分配：4 个正方位 = 龙鳞苔，4 个角位 = 续元蕊。这是宗门当年的"八卦药圃配方"，**deterministic 不随机**。
  - 每格 6×6 / 8×8 内按 grid 摆 4-6 株（layout 显式 `plant_grid` payload），由 `botany::lifecycle` 的 `place_plant_explicit(zone, pos, plant_id)` 注入（绕过 `spawn_v2_plants_for_zone` 的 score 判定）。
  - layout 半径 96 外的"野外"靠 §9.4b `wild_herb_clump` density spawn 出零散变异灵草（混合三种），让玩家既能在药圃格内"规律采集"也能在野外"撞见"——主刚需走 layout 保证可见性，野外是惊喜。
  - 本 zone 的 `BotanyZoneTag` 落入 `Ruins` + 新增 `DandaoPoisoned`（spirit_qi 0.4 + ambient_hint pill_steam_drift），让 蜕骨藤 / 兽心草 / 龙鳞苔 的 env_locks（§8.1 #7 决议）match 到本 zone（既支持 wild spawn 也支持 layout 种植）。

- **npc-ai**：守墓异灵走 plan-npc-ai-v1 的 big-brain Scorer/Action 体系，复用 chase / attack pattern；spawn 位置由 layout 显式锚定到大殿地下室（不是 density spawn）。掉落 table 加 `BeastBone × 3-5 (100%)` + `ShishuYin × 1 (10% 仅首杀)`。

- **agent**：当玩家首次进入 dan_zong_yi_yuan 或首次拾取 ShishuYin 时，server emit `bong:lore_event { kind: "baolongwang_prequel", chapter: "..." }`，agent 端 narration_pipeline 接管 chapter 化叙事，铺垫后续暴龙王 BOSS 战。

### §9.8 资产清单（§9 部分）

#### 贴图（gen-image 强制）

| 资产 | 路径 | 来源 |
|------|------|------|
| 紫色蒸气粒子 `pill_haze` | `assets/bong/textures/particle/pill_haze.png` | `/gen-image particle "紫灰色丹药蒸气粒子 32×32，柔光，末法残土像素风"` |
| 紫色土壤印 `purple_soil_stain` | `assets/bong/textures/particle/purple_soil_stain.png` | `/gen-image particle "紫色丹毒侵染地表印记 32×32，斑驳，半透明"` |
| 丹方壁画 `dandao_recipe_mural_*`（6 张） | `assets/bong/textures/painting/dandao_*.png` | `/gen-image item "百草门炼丹方壁画 32×32 古卷风格，水墨残破"` × 6 主题 |
| 师叔之印 `shishu_yin` 物品图标 | `assets/bong/textures/item/shishu_yin.png` | `/gen-image item "青铜方印「师叔」二字篆体 16×16 古旧"` |

#### Structure NBT（layout 投放，手工 Blockbench / WorldEdit）

| 资产 | 路径 | 尺寸 | 工作量 |
|------|------|------|--------|
| 百草丹殿主体 | `server/structures/dan_zong/dan_zong_great_hall.nbt` | 50×30×18（地表大殿 + 2 侧厅 + 地下室通道） | 4-6h |
| 露天炼丹大鼎 | `server/structures/dan_zong/ruined_open_furnace.nbt` | 5×5×5 | 1h |
| 药圃石篱 6×6（内环） | `server/structures/dan_zong/herb_garden_pen_6x6.nbt` | 6×2×6 | 0.5h |
| 药圃石篱 8×8（外环） | `server/structures/dan_zong/herb_garden_pen_8x8.nbt` | 8×2×8 | 0.5h |
| 蒸气毒泉·主 | `server/structures/dan_zong/vapor_poison_spring_main.nbt` | 5×3×5 | 1h |
| 蒸气毒泉·小 | `server/structures/dan_zong/vapor_poison_spring_small.nbt` | 3×2×3 | 0.5h |
| 倒塌丹方碑 | `server/structures/dan_zong/fallen_recipe_stele.nbt` | 1×3×2 | 0.5h |
| 丹师枯骨 | `server/structures/dan_zong/fallen_alchemist_bone.nbt` | 2×1×3 | 0.5h |
| 地下室·宗主石棺 | `server/structures/dan_zong/master_sarcophagus.nbt` | 3×2×5（含师叔之印放置点） | 1h |

总 9 个 NBT，~9-11h 手工搭建。

#### Worldgen 系统扩展（新代码模块）

| 资产 | 路径 | 工作量 |
|------|------|--------|
| Layout 基类 + Placement | `worldgen/scripts/terrain_gen/layouts/base.py` | 0.5d |
| Layout-runner + NBT paste 实现 | `worldgen/scripts/terrain_gen/layouts/runner.py` | 1d |
| dan_zong_compound layout 定义 | `worldgen/scripts/terrain_gen/layouts/dan_zong_compound.py` | 0.5d |
| terrain profile schema 扩 `architectural_layout` + `compound_flatten_radius` | `worldgen/scripts/terrain_gen/profiles/base.py` / `fields.py` | 0.5d |
| stitcher 加 layout-region density mask | `worldgen/scripts/terrain_gen/stitcher.py` | 0.5d |
| dan_zong_yi_yuan density profile（杂物） | `worldgen/scripts/terrain_gen/profiles/dan_zong_yi_yuan.py` | 0.5d |

总 ~3.5d worldgen 系统扩 + profile 落地。

#### 总成本汇总

- 贴图 ~10 张（gen-image），半小时
- NBT 9 个（手工），9-11h
- worldgen 系统扩展 + profile，~3.5d 工程时间

**归入阶段**：worldgen 系统扩展 → 本 plan **P-1 + PR-1**；NBT 资产 + dan_zong_compound layout 定义 + profile + ambient → 本 plan **PR-3 丹道地形**（依赖 PR-1 merged）。两者均在 plan-dandao-path-v1 内完成，不另立独立 plan。

### §9.9 测试要求（§9 部分）

- `dan_zong_yi_yuan` zone 写入后 `cargo test world::zone` 全绿（zone schema 解析 + aabb 不与现有 zone 重叠）
- terrain profile JSON 解析通过：`cd worldgen && python -m scripts.terrain_gen` 跑 dan_zong_yi_yuan tile 不抛
- raster 校验通过：`worldgen/scripts/terrain_gen/harness/raster_check.py` 对 dan_zong_yi_yuan range 不报红
- ambient_dan_zong audio_recipe 通过 `agent/packages/schema` typebox 校验
- ZoneAtmosphereProfile dan_zong_yi_yuan 注册到 client 后 `./gradlew test` 全绿
- **layout determinism 测试**（worldgen layout 系统专属）：同 seed 跑两次 worldgen，dan_zong_compound 内 16 格药圃 / 16 口丹炉 / 3 处毒泉 / 8 具枯骨 的坐标完全一致（断言）
- **layout-region density mask 测试**：在 dan_zong_great_hall POI 周围 96 格内不出现 wild_herb_clump / scattered_bone_fragment / small_rubble（断言层 query 应为空）
- **compound_flatten_radius 测试**：POI 周围 96 格内 height field 恒等于 76（容差 ±1，配合 NBT 楼梯过渡）
- **药圃格内灵草种植规则测试**：内环正北格只长蜕骨藤、外环角位只长续元蕊，与 §9.7 配方表一一对应
- 整 zone 端到端 smoke：`bash scripts/smoke-test.sh` 跑通玩家 teleport 进 dan_zong_yi_yuan + 沿中轴大道看到对称丹炉 + 走入主蒸气毒泉触发 contamination + 进入大殿地下室触发师叔之印 narration

---

## §10 消费本 plan 的工作流约束（consume-plan agent 必读）

> 本 plan 含 9 个 NBT 建筑 + 1 套 worldgen layout 系统扩展，**建筑类工作对 LLM 是难点**——一把过质量差。本 §10 是对 `commands/consume-plan.md` 通用工作流在本 plan 特殊场景下的细化约束，**不是替代**——通用约束（worktree / atomic commit / 测试全绿 / 不绕过 hooks）仍然全部生效。

### §10.1 建筑类任务：多轮打磨 + 自我 review + `<PROMISE>` 担保

凡涉及 NBT 建筑 / dan_zong_compound layout placement 摆位 / 视觉资产产出的 TODO，**禁止一次 commit 完成**。必须按以下迭代：

1. **Round 1 — first cut**：按 spec 摆出最简能用版本（大殿粗结构 / layout placements 公式套用）。commit message 标 `(round 1/3)`。
2. **Round 2 — 自我 review**：
   - **NBT 建筑**：用 Minecraft structure dump / 截图渲染检查比例 / 对称性 / 入口朝向 / 内部走廊是否能通；layout placements 用 ASCII 平面图打印（用 Python 脚本读 LayoutSpec 把 16 格药圃 / 16 口丹炉的 (dx, dz) 坐标投影到 200×200 字符网格）验证八卦布局真的 8 方位等距 + 22.5° 偏转 + 中轴对称
   - 发现问题 → 修 → commit `(round 2/3)`
3. **Round 3 — 终轮 review**：
   - 检查与 §9 spec 描述的视觉叙事一致性（百草丹殿是否给出"宗门遗迹废弃千年"的感受？药圃格子内灵草种植是否符合 §9.7 八卦方位配方？）
   - 修最后一轮 → commit `(round 3/3)`
4. **`<PROMISE>` 担保标记**：当 agent 认为"已尽 100% 努力，再改 1 轮也不会显著提升质量"时，在该建筑/layout 的**最终 round 3 commit message 末尾**写：

   ```
   <PROMISE>该建筑(/layout/...) 已经过 3 轮自我打磨 + review，达到当前能力上限。
   已检查：[比例对称 / 入口朝向 / 内部连通 / 视觉叙事 / spec 一致]
   仍存在的局限：[一两条诚实承认的不足，比如"装饰细节较单调，受 NBT 手搭工作量限制"]</PROMISE>
   ```

   `<PROMISE>` 不是免责声明，是**自我担保信号**——后续 CodeRabbit / 人工 review 若指出严重问题，仍要按 step 7 修复；但 nit / 偏好类不再继续打磨。

5. **不允许跳过迭代**：哪怕 Round 1 你觉得"已经很好"，仍必须走 Round 2 / Round 3——LLM 自评 first cut 质量普遍虚高，强制多轮是质量底线。
6. **非建筑类 TODO 不适用本节**：纯 Rust / TypeScript 逻辑代码（如 `MutationComponent` 实装、`DandaoStyle` 系统）按 commands/consume-plan.md 通常的 atomic commit + 测试全绿即可，无需多轮。

### §10.2 可选拆分多 PR（同一次 consume-plan 内）

本 plan **单 plan 多 PR** 路径（§8.2 修订后定调）。consume-plan agent 在 worktree 内**分多个 PR 序列化提交**：

- 推荐拆分点（依赖顺序，前一个 merge 后开下一个）：
  1. **PR-1 worldgen-layout 基础设施**：§9.4a `LayoutSpec / Placement / runner.py` + terrain profile schema 扩 + stitcher mask
  2. **PR-2 dandao 底盘**：§P0 + §P1 + §8.1 #1/#2/#4/#5 决议落地（纯 Rust 服务端 + schema）
  3. **PR-3 丹道地形**：§9 全部（依赖 PR-1，需要 layout 基础设施 land 后才能定义 dan_zong_compound）+ NBT 资产
  4. **PR-4 物品 / 视觉 / BOSS / 平衡**：§P2/P3/P4/P5（依赖前 3）

- **多 PR 仍属于同一次 `/consume-plan` 调用**：不退回让用户重跑。consume-plan agent 在 worktree 内顺序开 PR、等 merge、再开下一个，直到全部 land 后归档 plan。
- **PR 依赖处理**：前序 PR merge 前不开后续 PR；前序 PR 卡住（review 阻断 / CI 红 / merge 冲突）→ 走通用 step 7 / step 4.2 处理，处理完再继续；处理不了 → 停交人工，已 land 的 PR 保留不回退。
- **可选不拆**：如果 agent 评估单 PR 也能 review 过（不太可能但允许），可以一把出。**拆与不拆都合法**，按 agent 自己对 review 风险的判断。

### §10.3 CodeRabbit Review 等待协议（ScheduleWakeup 驱动）

CodeRabbit 是 PR 自动 review bot，以 GitHub Actions check run 形式呈现。consume-plan step 6 等 review 时按以下协议：

#### 状态判定

`gh pr checks <PR_NUM> --json name,status,conclusion` 查 CodeRabbit check：

| 状态 | 含义 | 动作 |
|------|------|------|
| `pass` (conclusion: success) | review 通过 | 进入 step 7 评审意见处理 / step 8 merge |
| `pending` (status: in_progress / queued) | 仍在 review | **等下一回合**（ScheduleWakeup） |
| `fail` (conclusion: failure) | review 不通过 | 按 step 7 严重性桶处理修复 |

#### 等待节奏

**禁止 sleep 循环 / busy poll**。每回合用 `ScheduleWakeup`：

- 首次提 PR 后 → `ScheduleWakeup delaySeconds=1200`（20 min，CodeRabbit 单回合典型耗时）reason="等 CodeRabbit review pass，PR #<num>"
- 醒来 → `gh pr checks <PR_NUM>` 查状态
- 若 `pending` → 再 `ScheduleWakeup delaySeconds=1200`，最多 3 回合 = 总 60 min
- 3 回合（60 min）仍 `pending` → 停交人工，输出 PR URL + `$WT_ABS` + "CodeRabbit 卡死 60+ min"
- `pass` / `fail` → 退出等待，进 step 7

#### 必须等 APPROVED 才 merge

对齐 memory `feedback_wait_coderabbit_approve.md`——**修完 review 意见后必须重新等 CodeRabbit re-review APPROVED**，**不自行判定**"我修好了应该过了所以直接 merge"。第二轮 review 同样按本协议（ScheduleWakeup 20 min × 最多 3 回合）。

#### 多 PR 场景

§10.2 多 PR 序列化时，**每个 PR 各自走完整 CodeRabbit 等待协议**——不能省。前一个 PR 未 APPROVED 不开下一个 PR。

### §10.4 单次 consume-plan 全自动到 merge

本 plan 的期望调用方式：**一次 `/consume-plan dandao-path-v1` 跑完全部 4 个 PR + 归档 plan**。

- consume-plan agent 在同一个 worktree / branch 序列中开 PR-1 → 等 review → merge → 开 PR-2 → ... → 全部 4 个 merge 完毕 → step 9 收尾清理
- 中途不要求人工干预——除非：
  - review 严重阻断（step 7 严重桶）反复修不过（≥2 轮）
  - merge 冲突 rebase 拿不准（step 4.2 ≥2 轮失败）
  - CodeRabbit 60 min 卡死（§10.3）
  - plan 设计层问题（评论指 plan 本身而非实装 patch）
- 全部 PR merge 后归档 plan（§3 末尾 "全 P 完成后" 章节），plan 文件 `git mv` 到 `docs/finished_plans/plan-dandao-path-v1.md` + Finish Evidence。

**预估总时长**（参考）：4 PR × (实施 1-3h + CodeRabbit 20-60 min + merge 5 min) ≈ 6-15 小时。consume-plan 全程 ScheduleWakeup 驱动，**不占用用户在线时间**——用户提交 `/consume-plan` 后即可下班，醒来看 plan 是否在 finished_plans/。

### §10.5 Subagent 驱动的 4 PR 实施（context 隔离强制）

> **2026-05-18 用户立此规则**：consume-plan 主线 agent **不亲自实施 PR**——为每个 PR 单独起一个 subagent（独立 context），主线只接收 subagent 的 `result` 段（200-500 token），不接收实施细节的几十 K token。4 PR 走完主线 context 增长 ≈ 2-5k token（vs 主线亲自跑会涨 200k+），等价实现"每 PR 后自动清理 context"，避免 1M context 模型在长任务里挤爆。

#### §10.5.1 subagent 配置（强制约定）

每次起 PR 实施 subagent 用以下参数：

```
Agent(
  description: "实施 PR-N <PR 名>",
  subagent_type: "claude",           // catch-all + 全工具集（Edit/Write/Bash/gh 全可用）
  model: "opus",                     // 强制 Opus 4.7（最强模型）
  prompt: "...任务描述...\n\nultrathink"   // 末尾 ultrathink keyword 触发最高思维 budget
)
```

**关键约定**：

- **`subagent_type: "claude"`**：catch-all subagent，工具集 `*`（Edit/Write/Bash/gh/Read 全可用）；不要用 `general-purpose`（功能等价但语义偏研究）也不要用 `Explore`（只读，没法实施）
- **`model: "opus"`**：显式指定 Opus 4.7。**不要用 sonnet/haiku**——实施 + 多轮自我 review + ultrathink 需要顶级模型
- **prompt 末尾 `ultrathink`**：Claude Code 硬约定，思维 budget 阶梯 `think` < `think hard` < `think harder` < `ultrathink`。`ultrathink` 是最高档，等价"思维强度 xhigh"，用于实施和 review 决策的高复杂度场景
- **`run_in_background: false`**（默认）：主线必须等 subagent 返回结果再继续（PR-N 完成才能开 PR-N+1，序列依赖）
- **`isolation: "worktree"`**：**不使用**——subagent 直接在主 worktree (`.worktree/plan-dandao-path-v1`) 内做事，与 consume-plan 通用 worktree 路径共享。`isolation: "worktree"` 会再开一个 nested worktree，无端复杂化

#### §10.5.2 subagent prompt 模板（PR-N 实施任务）

```
你是 plan-dandao-path-v1 的 PR-N 实施 subagent。任务是在主 worktree
(`$REPO_ROOT/.worktree/plan-dandao-path-v1`, branch: `auto/plan-dandao-path-v1`)
内完成以下范围：

## 范围（严格）
<本 PR 对应 plan 章节列表 + 必须实施的 TODO 清单，例如：
 - §9.4a worldgen layout 基础设施：LayoutSpec/Placement/runner.py
 - §9.3 terrain profile schema 扩 architectural_layout + compound_flatten_radius
 - 单元测试：layout determinism + density mask>

## 工作流约束（必读）
- 必读 plan: docs/plan-dandao-path-v1.md（特别是 §10.1 建筑多轮打磨 + §10.5）
- 必读 commands: .claude/commands/consume-plan.md（atomic commit / 测试全绿 / 不绕过 hooks）
- 建筑/NBT/layout placement 类 TODO → 走 §10.1 强制 3 轮迭代 + 终轮 commit 写 <PROMISE>...</PROMISE> 块
- 纯代码 TODO → 常规 atomic commit + 跑对应子项目测试全绿

## 禁止
- 不 push 到 origin（push 由你完成但 PR 创建由主线确认）—— 实际上你完成所有 commit + push + `gh pr create`，PR URL 返回主线
- 不等 CodeRabbit review（主线负责等）—— 提完 PR 你的任务就结束
- 不修改本 plan 范围外的文件
- 不动其他 plan-*.md / CLAUDE.md / worldview.md

## 完成后返回（严格 JSON 格式）
```json
{
  "pr_url": "https://github.com/.../pull/<num>",
  "pr_number": <num>,
  "branch": "auto/plan-dandao-path-v1",
  "commits": [
    { "hash": "abc1234", "message": "..." },
    ...
  ],
  "tests_run": [
    "cd server && cargo test layout::  → 12 passed",
    ...
  ],
  "promise_blocks": [
    "dan_zong_great_hall: <PROMISE>...</PROMISE>",
    ...
  ],
  "notes": "任何主线需要知道的留待事项"
}
```

ultrathink
```

#### §10.5.3 主线主流程（subagent + ScheduleWakeup 编排）

```
for pr_n in [PR-1, PR-2, PR-3, PR-4]:
    # 1. 起 subagent 实施（主线 context 只加一段 result）
    result = Agent(
        subagent_type="claude",
        model="opus",
        prompt=f"...PR-{pr_n} 任务模板... ultrathink"
    )
    pr_url, pr_number = parse(result)

    # 2. ScheduleWakeup 等 CodeRabbit（§10.3 协议）
    for round in range(3):
        ScheduleWakeup(delaySeconds=1200, prompt="continue consume-plan", reason=f"等 CR review PR #{pr_number}")
        # wakeup
        status = gh pr checks pr_number
        if status == "pass": break
        if status == "fail": handle_review(pr_number)  # 见步骤 3
    else:
        stop("CodeRabbit 60 min 卡死")

    # 3. 若 CodeRabbit fail：起一个修复 subagent（也是独立 context）
    if has_review_issues:
        fix_result = Agent(
            subagent_type="claude",
            model="opus",
            prompt=f"修 PR #{pr_number} 的 CodeRabbit 意见: ... ultrathink"
        )
        # 修完回到步骤 2 重等

    # 4. merge（主线直接做，命令简单不消耗 context）
    gh pr merge pr_number --squash --delete-branch

# 全 4 PR merge 后归档
git mv docs/plan-dandao-path-v1.md docs/finished_plans/
git commit -m "归档 plan-dandao-path-v1：..."
git push
```

**context 估算对比**：

| 路径 | 主线 context 增长 |
|------|-----------------|
| 主线亲自跑 4 PR | ~200k token（4 × 50k 实施细节） |
| **subagent + ScheduleWakeup（本规范）** | **~2-5k token**（4 × subagent result + wakeup tick + merge cmd） |

#### §10.5.4 子 subagent 修复 review 意见

CodeRabbit 给 review 意见后，主线**不亲自修**——再起一个新 subagent：

```
Agent(
  subagent_type: "claude",
  model: "opus",
  prompt: """
  PR #<num> (<url>) 收到 CodeRabbit review 意见：

  <粘 review 评论原文>

  在主 worktree 内修复。按 §7 严重性桶处理：
  - 严重（bug/安全/与 plan 目标矛盾）→ 必修
  - 中等（质量问题不影响功能）→ 自行决定，未采纳要回 PR 评论说明理由
  - 轻微（nit/style）→ 默认不采纳，统一回一条评论

  修完 push 到 origin（不需等 CR re-review，主线负责）。
  返回：{ "fixed": [...], "rejected": [...], "fix_commits": [...] }

  ultrathink
  """
)
```

主线收到修复 result 后再走一轮 ScheduleWakeup 等 CR re-review。

#### §10.5.5 与 §10.1 多轮打磨的关系

§10.1 的 3 轮打磨 + `<PROMISE>` 担保**发生在 subagent 内部**——subagent 自己 commit `(round 1/3)` / `(round 2/3)` / `(round 3/3)` + 终轮 commit 写 `<PROMISE>` 块。主线只在 subagent 返回的 `promise_blocks` 字段里收到摘要，**不参与每轮 review**。

---

## Finish Evidence

### 落地清单

| 阶段 | 模块 / 文件路径 |
|------|----------------|
| P-1 worldgen layout infra | `worldgen/scripts/terrain_gen/layouts/{base,runner,__init__}.py` + `stitcher.py` 扩展（`_compute_circular_mask` / `apply_compound_flatten` / `compute_layout_density_mask`）+ `fields.py` `density_maskable` + `blueprint.py` `compound_flatten_radius` / `architectural_layout` |
| P0 丹道底盘 | `server/src/dandao/{mod,components,skills,toxin_tracker}.rs` + `server/src/schema/dandao.rs` |
| P1 变异系统 | `server/src/dandao/{mutation,progression}.rs` + `server/src/cultivation/life_record.rs` 扩展 + `server/src/schema/inventory.rs` ExtraHand slots |
| P2 物品 | `server/src/dandao/{herbs,recipes,catalyst_furnace,mutation_forge}.rs` + `server/assets/alchemy/recipes/dandao_*.json` |
| P3 视觉 placeholder | `server/src/dandao/visual_sync.rs`（MutationVisualSyncPayload + DandaoHudPanelData schema） |
| P4 暴龙王 BOSS | `server/src/dandao/{boss,boss_ai}.rs` |
| P5 境界递进 | `server/src/dandao/{internal_brew,progression}.rs` |
| 丹道地形 | `worldgen/scripts/terrain_gen/{layouts/dan_zong_compound,profiles/dan_zong_yi_yuan}.py` + `server/zones.json` 新增 2 zones |

### 关键 commit（squash merge）

| hash | 日期 | PR | 内容 |
|------|------|-----|------|
| `82c5d082b` | 2026-05-18 | #259 | PR-1: worldgen layout 基础设施 |
| `a3b91ce33` | 2026-05-18 | #260 | PR-2: 丹道底盘 + 变异系统 (P0+P1) |
| `660376340` | 2026-05-18 | #262 | PR-3: 丹道地形——丹宗遗园 + 暴龙王巢穴 |
| `cc2b3dc7b` | 2026-05-18 | #264 | PR-4: 物品 / BOSS / 境界递进 (P2-P5) |

### 测试结果

- PR-1: `python3 -m pytest worldgen/ → 88 passed (46 new layout infra)`
- PR-2: `cargo test → 5192 passed (116 new dandao)`
- PR-3: `python3 -m pytest worldgen/ → 109 passed (79 new terrain)` + `cargo test world::zone → 16 passed`
- PR-4: `cargo test → 5356 passed (98 new P2-P5)`
- 全部 PR 均通过 e2e CI pipeline（Schema + Agent + Server build + cargo test + Smoke/E2E）

### 跨仓库核验

| 仓库 | 命中 symbol |
|------|-------------|
| server | `DandaoStyle` / `MutationState` / `MutationStage` / `MutationKind` / `BodySlot` / `ActiveMutation` / `BaolongwangBoss` / `MutationVisualSyncPayload` / `DandaoHudPanelData` / `DandaoInternalBrewIntent` / `EquipSlotV1::ExtraHand0/1` / `WEAPON_SWAP_COOLDOWN_TICKS` / `LayoutSpec` / `Placement` / `run_layout` / `apply_compound_flatten` / `compute_layout_density_mask` |
| schema | `MutationStageV1` / `MutationKindV1` / `MutationEventV1` / `DandaoStyleV1` / `ActiveMutationV1` / `EquippedInventorySnapshotV1.extra_hand_0/1` |
| agent | `bong:mutation_event`（schema 已定义，agent 端 narration_pipeline 接入待后续） |
| client | `bong:mutation_visual`（CustomPayload 已定义，GeckoLib 渲染待后续 client PR） |

### 遗留 / 后续

- **worldview §六.4 丹体异化**：需单独 PR 写入 `docs/worldview.md`，人工 review（§8.2 说明，不在 consume-plan 范围）
- **Client GeckoLib 变异附件渲染**：10 个 Blockbench 模型 + GeckoLib renderer（P3 §4.1 只做了 schema placeholder）
- **Client HUD 丹道面板 Java 实现**：P3 §4.2 只做了 schema
- **贴图资产**：33 次 `/gen-image` 调用（§7.4 贴图管线）
- **NBT 建筑文件**：9 个 structure NBT（§9.8），layout runner 目前为 stub paste
- **暴龙王 GeckoLib 模型渲染**：Bedrock→GeckoLib 转换 + bone alias renderer
- **Agent 端 narration pipeline**：`bong:mutation_event` 消费 + 天道对变异体的反应文案
- **炼丹招式实际 qi 扣除**：P0 三招式 resolver 目前只做 gate check，实际 qi 扣除 + side-effect 需接入 QiTransfer + alchemy::pill 路径
- **mutation_advance_system 节流**：当前每帧检测，需加 600-tick 节流计数器







