# Bong · plan-alchemy-v1

**炼丹专项**（**不含炼器**——炼器另立 `plan-forge-v1`）。Worldview §三"材料/丹药永远是辅助"，本 plan 把丹药做成**有代价的加速器**，不做"吃了就变强"的 power creep。

**世界观锚点**：
- `worldview.md §三` — 丹药可加速/保险，绝非必需；万物皆有成本
- `worldview.md §六 染色谱` — 10 种真元色是本 plan 的代价/效果共同语言
- `worldview.md §十` — 灵气零和，灵草需区域灵气 > 0.3 才生长

**关键复用**（避免重复造轮子）：
- `cultivation::Contamination / ContamSource` — **丹毒直接复用**，药入体 = ContamSource 入 entries
- `cultivation::contamination_tick` — **代谢直接复用**（10:15 qi 排异比）
- `cultivation::MeridianSystem` — 代谢速率随经脉升级天然加快（sum_rate / opened_count / integrity 提升）

**交叉引用**：`plan-inventory-v1.md` · `plan-cultivation-v1.md`（经脉/真元/污染）· `plan-HUD-v1.md §10`（快捷使用栏）· `plan-botany-v1.md`（植物/采集，待立）· `plan-forge-v1.md`（炼器，待立）。

---

## §0 设计轴心

- [ ] **三大子系统完全解耦**（为后续自动化留口）：
  - 配方系统（JSON 加载，纯数据）
  - 熔炉系统（世界实体，玩家/NPC/未来傀儡皆可绑）
  - 火候进程（运行时状态机，玩家可干预或设定曲线）
- [ ] **药三分毒**：每颗丹产出一条 `ContamSource`（色 + 量），进入 contamination 系统
- [ ] **代谢 = 现有经脉能力**：不新增代谢字段；吃下一颗前必须等当前丹毒 purge 到阈值以下
- [ ] 多结果输出（成丹/副产/废丹/炸炉），**非二元**
- [ ] **品阶 / 铭文 / 开光 先不做**（TODO）——MVP 只做单一成功度轴
- [ ] **灵草 MVP 用 placeholder registry**，等 `plan-botany-v1` 落地后替换

---

## §1 子系统拆解

### §1.1 配方系统（纯数据，JSON 加载）

路径：`server/assets/alchemy/recipes/*.json` · JSON Schema 放 `agent/packages/schema`（与其他 IPC schema 同栈，双端契约）。**测试示例见 §3.2**（不进生产）。

配方结构要点：
- `stages[]` — 分段式投料；首段 = 起炉投料，后续段 = 中途要求（到 tick X 时须追加材料 Y）
- `fire_profile` — 目标温度 + 时长 + 容差带
- `outcomes` — perfect/good/flawed/waste/explode 五桶；`flawed_fallback` 专用于**残缺匹配**路径（见 §1.3 投错料规则）
- `furnace_tier_min` — 限最低炉阶

- [ ] 加载器：启动期扫目录 → `RecipeRegistry` resource
- [ ] 未来 NPC/agent 可以**读同一张表**做自动炼丹

### §1.2 熔炉系统（世界实体）

```rust
#[derive(Component)]
pub struct AlchemyFurnace {
    pub tier: u8,              // 决定可开火候精度 + 最高配方
    pub owner: Option<Entity>,
    pub session: Option<AlchemySessionId>,
    pub integrity: f32,        // 炸炉会扣
}
```

- [ ] 放置为方块或实体（MVP 用方块 + BlockEntity）
- [ ] tier 由材料决定（凡铁炉 / 灵铁炉 / ...）
- [ ] `owner` 只影响启动权限；session 持有当前炼制任务
- [ ] **多炉并行**：一玩家可绑多个炉（未来自动化入口）

### §1.3 火候进程（运行时状态机）

```rust
pub struct AlchemySession {
    pub recipe: RecipeId,
    pub furnace: Entity,
    pub caster: Entity,
    pub elapsed_ticks: u32,
    pub temp_track: Vec<(u32, f32)>,   // (tick, temp) 曲线记录
    pub qi_injected: f64,
    pub interventions: Vec<Intervention>,
}

pub enum Intervention {
    AdjustTemp(f32),      // 手动滑块
    InjectQi(f64),        // 灌真元
    AutoProfile(ProfileId),  // 后续：绑定预设曲线
}
```

- [ ] 进程每 tick 比对当前 temp/qi 与 `fire_profile` → 累积偏差
- [ ] 结束时按偏差分桶到 `outcomes.{perfect,good,flawed,waste,explode}`
- [ ] **AutoProfile 是预留口**：未来用 JSON 定义的标准曲线替代人工，实现"自动化炼丹"

#### 材料消耗规则

- [ ] **投入即消耗**：材料从背包拖入炉槽瞬间扣库存（不等起炉）
- [ ] **起炉前可右键取回**（退回背包，无损）
- [ ] **起炉后锁定**，失败（waste/explode）**不返还**任何材料
- [ ] 中途投料（`stages[].at_tick`）错过窗口 → 本阶段判失败，直接走 flawed/waste 分桶

#### 投错料规则（不做配方校验，走残缺匹配）

> 设计原则：玩家投错 = 玩家的事（plan §0 "玩法深度"）。系统不弹"材料不对"，只走结果。

1. **精确匹配**：投入材料集合 == `recipes[X].ingredients` → 按 fire_profile 偏差正常分桶
2. **残缺匹配**：投入材料是某配方 `ingredients` 的**子集**（缺一种或多种），且存在该配方的 `flawed_fallback` 定义 → 走残缺版
3. **乱投**：既不精确也无 fallback → `outcomes.waste` 或（温度/qi 严重偏离时）`outcomes.explode`

**残缺版产出**：
- 丹效 ×0.3~0.6（按缺失比例线性）
- 固定副作用 toxin_amount ×1.5
- **追加随机一条 side_effect**（从 `side_effect_pool` 抽，可好可坏，见 §3.2 示例）
- 该次残缺配方 + 抽到的 side_effect **记入 `LifeRecord`**（见 §4 数据契约），玩家在 inspect 面板可看自己历次残缺尝试，相当于"试药史"

#### 中途投料

- [ ] `stages[]` 定义多阶段投料窗口：`{ at_tick: 80, required: [{material, count}], window: 20 }`
- [ ] 到 tick 时 UI 提示"该下 X 了"（投料槽闪烁）
- [ ] window 内未投 / 投错 → 走残缺匹配或失败

#### 离线 / 持续性

- [ ] **服务器常驻**，session 不因玩家下线暂停；tick 照推
- [ ] BlockEntity 持久化 session 状态（炉与 session 双向引用）
- [ ] 重启服务器 → 从 BlockEntity 快照恢复 session

### §1.4 方子学习与切换

```rust
#[derive(Component)]
pub struct LearnedRecipes {
    pub ids: Vec<RecipeId>,
    pub current_index: usize,  // 当前卷轴翻到第几张
}
```

- [ ] 初始玩家无已学方子（或仅"开脉丹"作为教学）
- [ ] **学习**：从背包拖【丹方残卷】item 到卷轴区 → `LearnedRecipes.ids.push(id)` + 残卷消耗
- [ ] **翻页**：卷轴 UI 左右箭头切换 `current_index`
- [ ] 重复残卷（已学）→ 提示"此方已悟"，不消耗
- [ ] 残卷内容损坏（未来扩展）→ 只能学到残缺版，进残缺匹配池

---

## §2 丹毒模型（复用 Contamination）

### §2.1 服药流程

```
useItem(pill) →
  查 pill.toxin_amount / toxin_color →
  Contamination.entries.push(ContamSource {
    amount: toxin_amount,
    color: toxin_color,
    attacker_id: None,    // None 表示丹毒而非战斗污染
    introduced_at: tick,
  }) →
  应用 pill.effect（回血 / 加 qi / 推进经脉进度 ...）
```

- [ ] `attacker_id: None` 用作"丹毒来源"标签（未来可加 `Source` enum 精确区分）
- [ ] 丹毒色由配方决定，多为 `Mellow` / `Turbid`（平和 / 浊）

### §2.2 重复服药约束

```
can_take(pill) = Contamination.entries
  .filter(|e| e.color == pill.toxin_color && e.attacker_id.is_none())
  .map(|e| e.amount).sum() < THRESHOLD
```

- [ ] 同色丹毒未代谢到阈值 → 禁止再服（或强吃触发过量 debuff）
- [ ] **不新增字段**：代谢快慢完全由经脉 `sum_rate × integrity` 决定（contamination_tick 本就这么算）
- [ ] 经脉升级 → 代谢加快 = worldview "升级经脉获取更强代谢"

### §2.3 过量惩罚（可选，放 §7 开放）

- [ ] 阈值以上强吃：立刻施加 debuff + 额外经脉裂痕
- [ ] 对应 worldview "万物皆有成本"

---

## §3 MVP 范围（先跑通框架）

### §3.1 测试丹药（3 种，验证三条路径）

| 丹 | 效果 | 丹毒色/量 | 验证意图 |
|---|---|---|---|
| 回元丹 | qi_current +20 | Mellow 0.3 | 基础服药 → 污染 → 代谢闭环 |
| 开脉丹 | 推进当前未通经脉进度 +30% | Mellow 0.5 | 与 meridian_open 联动 |
| 赌命散 | qi_current +50 + 瞬回 | Violent 1.2 | 高毒高效，验证过量约束 |

### §3.2 测试配方 JSON（仅测试，不进生产）

三份示例见下。覆盖：单阶/多阶投料 · 残缺 fallback · side_effect_pool · 炸炉。

```json
// recipes/kai_mai_pill_v0.json — 开脉丹（单阶，演示残缺匹配）
{
  "id": "kai_mai_pill_v0",
  "name": "开脉丹（测试）",
  "furnace_tier_min": 1,
  "stages": [
    { "at_tick": 0, "required": [
      { "material": "kai_mai_cao", "count": 3 },
      { "material": "ling_shui",   "count": 1 }
    ], "window": 0 }
  ],
  "fire_profile": {
    "target_temp": 0.60, "target_duration_ticks": 200, "qi_cost": 15.0,
    "tolerance": { "temp_band": 0.10, "duration_band": 30 }
  },
  "outcomes": {
    "perfect": { "pill": "kai_mai_pill",         "quality": 1.0, "toxin_amount": 0.30, "toxin_color": "Mellow" },
    "good":    { "pill": "kai_mai_pill",         "quality": 0.7, "toxin_amount": 0.50, "toxin_color": "Mellow" },
    "flawed":  { "pill": "kai_mai_pill_flawed",  "quality": 0.4, "toxin_amount": 0.80, "toxin_color": "Turbid" },
    "waste":   null,
    "explode": { "damage": 20.0, "meridian_crack": 0.15 }
  },
  "flawed_fallback": {
    "pill": "kai_mai_pill_flawed",
    "quality_scale": 0.5,
    "toxin_scale": 1.5,
    "side_effect_pool": [
      { "tag": "minor_qi_regen_boost",   "duration_s": 300, "weight": 1 },
      { "tag": "meridian_itch_debuff",   "duration_s": 300, "weight": 2 },
      { "tag": "random_color_shift",     "color": "Insidious", "amount": 0.2, "weight": 1 }
    ]
  }
}
```

```json
// recipes/hui_yuan_pill_v0.json — 回元丹（单阶，最简）
{
  "id": "hui_yuan_pill_v0",
  "name": "回元丹（测试）",
  "furnace_tier_min": 1,
  "stages": [
    { "at_tick": 0, "required": [
      { "material": "bai_cao",   "count": 2 },
      { "material": "ling_shui", "count": 1 }
    ], "window": 0 }
  ],
  "fire_profile": {
    "target_temp": 0.45, "target_duration_ticks": 120, "qi_cost": 8.0,
    "tolerance": { "temp_band": 0.12, "duration_band": 20 }
  },
  "outcomes": {
    "perfect": { "pill": "hui_yuan_pill", "quality": 1.0, "toxin_amount": 0.20, "toxin_color": "Mellow", "effect": { "qi_gain": 24 } },
    "good":    { "pill": "hui_yuan_pill", "quality": 0.7, "toxin_amount": 0.30, "toxin_color": "Mellow", "effect": { "qi_gain": 18 } },
    "flawed":  { "pill": "hui_yuan_pill_flawed", "quality": 0.4, "toxin_amount": 0.50, "toxin_color": "Turbid", "effect": { "qi_gain": 10 } },
    "waste":   null,
    "explode": { "damage": 8.0, "meridian_crack": 0.05 }
  },
  "flawed_fallback": {
    "pill": "hui_yuan_pill_flawed",
    "quality_scale": 0.5, "toxin_scale": 1.5,
    "side_effect_pool": [
      { "tag": "stamina_boost",       "duration_s": 180, "weight": 1 },
      { "tag": "blurred_vision_15s",  "duration_s": 15,  "weight": 2 }
    ]
  }
}
```

```json
// recipes/du_ming_san_v0.json — 赌命散（多阶投料，高毒高效）
{
  "id": "du_ming_san_v0",
  "name": "赌命散（测试）",
  "furnace_tier_min": 2,
  "stages": [
    { "at_tick": 0,   "required": [{ "material": "xue_cao",  "count": 4 }], "window": 0 },
    { "at_tick": 80,  "required": [{ "material": "shou_gu",  "count": 1 }], "window": 20 },
    { "at_tick": 160, "required": [{ "material": "huo_jing", "count": 1 }], "window": 10 }
  ],
  "fire_profile": {
    "target_temp": 0.85, "target_duration_ticks": 220, "qi_cost": 35.0,
    "tolerance": { "temp_band": 0.05, "duration_band": 10 }
  },
  "outcomes": {
    "perfect": { "pill": "du_ming_san", "quality": 1.0, "toxin_amount": 1.20, "toxin_color": "Violent", "effect": { "qi_gain": 60, "qi_cap_boost_30s": 1.5 } },
    "good":    { "pill": "du_ming_san", "quality": 0.7, "toxin_amount": 1.50, "toxin_color": "Violent", "effect": { "qi_gain": 40, "qi_cap_boost_30s": 1.3 } },
    "flawed":  { "pill": "du_ming_san_flawed", "quality": 0.3, "toxin_amount": 2.00, "toxin_color": "Violent" },
    "waste":   null,
    "explode": { "damage": 40.0, "meridian_crack": 0.30 }
  },
  "flawed_fallback": {
    "pill": "du_ming_san_flawed",
    "quality_scale": 0.3, "toxin_scale": 2.0,
    "side_effect_pool": [
      { "tag": "berserk_5s",              "duration_s": 5,   "weight": 2 },
      { "tag": "qi_cap_perm_minus_1",     "perm": true,      "weight": 1 },
      { "tag": "rare_insight_flash",      "duration_s": 0,   "weight": 1 }
    ]
  }
}
```

**测试意图**：
- 开脉丹 → 单阶 + 残缺匹配闭环
- 回元丹 → 最简基线，验证 qi_gain effect
- 赌命散 → 多阶投料 + 严苛容差（±0.05 / ±10 ticks）+ 高毒 Violent 验证过量约束

### §3.3 交互 UI（MVP，B 层硬编码 Screen）

> 详见 `docs/svg/alchemy-furnace.svg` 草图。

- [ ] 层级：**BaseOwoScreen&lt;FlowLayout&gt;**（B 层，硬编码，右键炉方块打开）
- [ ] 三列布局（1560×900 居中，留边给 MC 世界）：
  - 左：**方子手札**（卷轴底纹 + 手抄文案 + ◀▶ 翻页 + 拖【丹方残卷】入卷学新方）
  - 中：炉体可视化 + 4 个通用投料槽（drop target） + 温度滑块 + F 注真元
  - 右：**复用塔科夫背包**（`BackpackGridPanel` 5×7 + 多 tab + `ItemTooltipPanel`）
- [ ] 底栏：五结果桶实时概率 + 丹毒预警（Mellow / Violent 双色条）
- [ ] **不做**火候曲线编辑器（留 v2）
- [ ] **不做**品阶显示（只显示 quality %）
- [ ] **不做**配方匹配校验 UI（投错自然走残缺 / waste / explode）

---

## §4 数据契约

### Server 侧

- [ ] `RecipeRegistry` resource（启动期加载 JSON）
- [ ] `AlchemyFurnace` component + BlockEntity（持久化 session_id）
- [ ] `AlchemySession` resource（map: session_id → state，含 stages_progress / staged_materials_in）
- [ ] `LearnedRecipes` component（挂玩家，含 ids + current_index）
- [ ] `LifeRecord` 扩展：追加 `alchemy_attempts: Vec<AlchemyAttempt>`（含时间/配方/残缺版/抽到的 side_effect）
- [ ] Events：`StartAlchemyRequest` / `InterventionRequest`（调温/注 qi/中途投料）/ `AlchemyOutcome`
- [ ] Channel：`bong:alchemy/start` · `bong:alchemy/tick`（session 状态广播）· `bong:alchemy/intervention` · `bong:alchemy/outcome`
- [ ] IPC Schema（agent/packages/schema）：recipe JSON Schema（为 agent 推演 NPC 炼丹准备）

### Client 侧（新增 Store）

- [ ] `AlchemyFurnaceStore` — 当前打开炉体状态（tier / integrity / owner）
- [ ] `AlchemySessionStore` — 实时 session（elapsed / temp_track / qi_injected / staged_materials / stage_hints）
- [ ] `RecipeScrollStore` — 当前卷轴上显示的 recipe 文案 + `LearnedRecipes` 列表 + current_index
- [ ] 复用：`InventoryStateStore` / `BackpackGridPanel` / `DragState` / `ItemTooltipPanel` / `Contamination`（HUD 预警读取）

---

## §5 阶段划分

| Phase | 内容 | 验收 |
|---|---|---|
| P0 | RecipeRegistry + JSON 加载 + IPC Schema + 单测 | 启动加载 3 份测试 recipe 无错 |
| P1 | AlchemyFurnace BlockEntity + LearnedRecipes component + 右键打开 Screen | 方块持久化 session_id · 客户端拿到 RecipeScrollStore |
| P2 | 投料槽 + 材料消耗规则（投入即消耗/不返还）+ stages 多阶支持 | 赌命散三阶投料流程跑通 |
| P3 | 火候进程 tick + 偏差计算 + 精确匹配 outcome 分桶 | 三种测试丹能 perfect/good/flawed/explode |
| P4 | 残缺匹配 + side_effect_pool 抽取 + LifeRecord 记录 | 缺一料能走 fallback，inspect 看得到试药史 |
| P5 | 服药 → ContamSource 注入 + 重复服药阈值 + 过量 debuff | 过量服赌命散触发禁服 / 强吃 debuff |
| P6 | BaseOwoScreen 接入（卷轴手札 + 塔科夫背包 + 拖拽投料 + 翻页/拖残卷学习） | 全流程端到端可玩 |

---

## §6 跨 plan 钩子

- [ ] **plan-botany-v1**（待立）：替换 §3.2 的 placeholder 材料，接入真实灵草采集
- [ ] **plan-forge-v1**（待立）：炼器走同一炉体抽象？或独立？留待 forge plan 决策
- [ ] **plan-cultivation-v1**：`open_pill_progress` 字段或事件，让开脉丹推进经脉进度
- [ ] **plan-HUD-v1 §10**：快捷使用栏消费 pill item
- [ ] **plan-inventory-v1**：
  - pill item 定义 + 栈上限 + 操作磨损（worldview §七 "灵物操作磨损"）
  - **丹方残卷 item**：1×2 类书籍物品，携带 `recipe_id: RecipeId`，可拖到卷轴区学习（见 §1.4）
  - 材料类 item：`kai_mai_cao / ling_shui / bai_cao / xue_cao / shou_gu / huo_jing`（测试 placeholder）
- [ ] **plan-death-lifecycle-v1 §4c**：未来续命丹走本系统（可好可坏的 side_effect_pool 正好适合"续命总有代价"）

---

## §7 TODO / 开放问题（留给 v2+）

- [ ] **品阶系统**：下品/中品/上品/极品 → 目前先用 quality: f32 代替
- [ ] **铭文系统**：丹药铭文附加效果
- [ ] **开光 / 丹心**：高阶交互
- [ ] **丹毒过量 debuff** 具体效果曲线
- [ ] **丹方获取路径**：初始解锁 / NPC 散修 / 遗迹残卷（依赖 social/narrative plan）
- [ ] **自动化炼丹**（AutoProfile 曲线库 + 傀儡绑定炉）
- [ ] **NPC/agent 侧炼丹**（读同一 RecipeRegistry）
- [ ] **丹心识别**：玩家能否逆向配方（worldview §九 "情报换命"）

---

## §8 风险与对策

| 风险 | 对策 |
|---|---|
| 丹药成为数值 power creep | 丹毒 + 重复服药阈值强约束；所有丹都带 ContamSource |
| UI 复杂度失控 | 三列 BaseOwoScreen · 复用塔科夫背包 · 曲线编辑器留 v2 |
| 残缺匹配被玩家滥用当"试药赌博" | side_effect_pool 大多数是负面；`LifeRecord.alchemy_attempts` 公开到亡者博物馆（坏名声成本） |
| 中途投料 tick 漂移导致一直 miss window | window 默认 ≥ 10 ticks（0.5s）· 服务器权威 tick，不受客户端 FPS 影响 |
| 与 botany 耦合阻塞进度 | placeholder material 先行，botany 落地后无痛替换 |
| 配方 JSON 结构变更频繁 | schema 走 TypeBox（agent/packages/schema），双端契约 |
