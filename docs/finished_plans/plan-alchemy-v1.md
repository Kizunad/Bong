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

- [x] **三大子系统完全解耦**（为后续自动化留口）：
  - 配方系统（JSON 加载，纯数据）
  - 熔炉系统（世界实体，玩家/NPC/未来傀儡皆可绑）
  - 火候进程（运行时状态机，玩家可干预或设定曲线）
- [x] **药三分毒**：每颗丹产出一条 `ContamSource`（色 + 量），进入 contamination 系统
- [x] **代谢 = 现有经脉能力**：不新增代谢字段；吃下一颗前必须等当前丹毒 purge 到阈值以下
- [x] 多结果输出（成丹/副产/废丹/炸炉），**非二元**
- [ ] **品阶 / 铭文 / 开光 先不做**（TODO）——MVP 只做单一成功度轴
- [x] **灵草 MVP 用 placeholder registry**，等 `plan-botany-v1` 落地后替换

---

## §1 子系统拆解

### §1.1 配方系统（纯数据，JSON 加载）

路径：`server/assets/alchemy/recipes/*.json` · JSON Schema 放 `agent/packages/schema`（与其他 IPC schema 同栈，双端契约）。**测试示例见 §3.2**（不进生产）。

配方结构要点：
- `stages[]` — 分段式投料；首段 = 起炉投料，后续段 = 中途要求（到 tick X 时须追加材料 Y）
- `fire_profile` — 目标温度 + 时长 + 容差带
- `outcomes` — perfect/good/flawed/waste/explode 五桶；`flawed_fallback` 专用于**残缺匹配**路径（见 §1.3 投错料规则）
- `furnace_tier_min` — 限最低炉阶

- [x] 加载器：启动期扫目录 → `RecipeRegistry` resource
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

- [x] 放置为方块或实体（2026-04-21 server-only：`ClientRequestV1::AlchemyFurnacePlace` → `PlaceFurnaceRequest` 事件 → spawn ECS entity + 刷 `BlockState::FURNACE`。Fabric 客户端拦截右键发 payload 未接，见 reminder.md）
- [x] tier 由材料决定（`alchemy::furnace::furnace_tier_from_item_id`：`furnace_fantie` → tier 1；灵铁炉 tier 2 / 仙铁炉 tier 3 等 forge-v1 品阶落地后补）
- [x] `owner` 只影响启动权限；session 持有当前炼制任务
- [x] **多炉并行**：同玩家可在多坐标放多个炉（每炉独立 ECS entity；多炉 session 路由 / intervention 按炉分发仍未接，见 reminder.md）

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

- [x] 进程每 tick 比对当前 temp/qi 与 `fire_profile` → 累积偏差
- [x] 结束时按偏差分桶到 `outcomes.{perfect,good,flawed,waste,explode}`
- [x] **AutoProfile 是预留口**：未来用 JSON 定义的标准曲线替代人工，实现"自动化炼丹"

#### 材料消耗规则

- [ ] **投入即消耗**：材料从背包拖入炉槽瞬间扣库存（不等起炉）
- [ ] **起炉前可右键取回**（退回背包，无损）
- [x] **起炉后锁定**，失败（waste/explode）**不返还**任何材料
- [x] 中途投料（`stages[].at_tick`）错过窗口 → 本阶段判失败，直接走 flawed/waste 分桶

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

- [x] `stages[]` 定义多阶段投料窗口：`{ at_tick: 80, required: [{material, count}], window: 20 }`
- [x] 到 tick 时 UI 提示"该下 X 了"（投料槽闪烁）
- [x] window 内未投 / 投错 → 走残缺匹配或失败

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

- [x] 初始玩家无已学方子（或仅"开脉丹"作为教学）
- [x] **学习**：从背包拖【丹方残卷】item 到卷轴区 → `LearnedRecipes.ids.push(id)` + 残卷消耗
- [x] **翻页**：卷轴 UI 左右箭头切换 `current_index`
- [x] 重复残卷（已学）→ 提示"此方已悟"，不消耗
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

- [x] `attacker_id: None` 用作"丹毒来源"标签（未来可加 `Source` enum 精确区分）
- [x] 丹毒色由配方决定，多为 `Mellow` / `Turbid`（平和 / 浊）

### §2.2 重复服药约束

```
can_take(pill) = Contamination.entries
  .filter(|e| e.color == pill.toxin_color && e.attacker_id.is_none())
  .map(|e| e.amount).sum() < THRESHOLD
```

- [x] 同色丹毒未代谢到阈值 → 禁止再服（或强吃触发过量 debuff）
- [x] **不新增字段**：代谢快慢完全由经脉 `sum_rate × integrity` 决定（contamination_tick 本就这么算）
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

> 详见 `docs/svg/alchemy-furnace.svg` 草图（草图为放大示意，实际尺寸以代码为准）。

- [x] 层级：**BaseOwoScreen&lt;FlowLayout&gt;**（B 层，硬编码，右键炉方块打开）
- [x] **紧凑面板 600×340**（panel padding 6 / gap 4），目标 UI scale 3 完整可见：
  - 1080p · scale 3 → GUI 640×360 ✓
  - 2K · scale 3 → GUI 853×480 ✓ 充裕
  - 历史教训：1560×900 旧版在 scale 3 下只能看见 ~2/3，已弃用
- [x] 三列布局（高 244）：
  - 左 150：**方子手札**（◀ 标题 ▶ + 副标题 + 正文 140 高 + 残卷拖入区 46 高）
  - 中 220：4 投料槽行 + 状态行 + 进度条 + 温度条 + qi 条 + 干预 log（截 2 条）+ 炉信息一行
  - 右 212：**复用塔科夫背包** `BackpackGridPanel` 5×7（196×140）+ 重量条
- [x] 底栏 60：5 个 36×36 outcome 方块（perf/good/flaw/wast/boom）+ Mellow/Violent 双色丹毒条
- [ ] **删除项**（旧版冗余）：炉体 ASCII 可视化（lid/body/flame/base 共 200 高）· 多 tab 行 · hotbar 预览 · 大 tooltip 占位框 · 键位提示框 · 各种 plan §X.Y meta-label
- [ ] **不做**火候曲线编辑器（留 v2）
- [ ] **不做**品阶显示（只显示 quality %）
- [ ] **不做**配方匹配校验 UI（投错自然走残缺 / waste / explode）
- [x] 尺寸常量集中在 `AlchemyScreen.PANEL_W/PANEL_H/LEFT_W/MID_W/RIGHT_W/BODY_H/BOTTOM_H`，方便后续调参

---

## §4 数据契约

### Server 侧

- [x] `RecipeRegistry` resource（启动期加载 JSON）
- [ ] `AlchemyFurnace` component + BlockEntity（持久化 session_id）
- [ ] `AlchemySession` resource（map: session_id → state，含 stages_progress / staged_materials_in）
- [x] `LearnedRecipes` component（挂玩家，含 ids + current_index）
- [x] `LifeRecord` 扩展：追加 `alchemy_attempts: Vec<AlchemyAttempt>`（含时间/配方/残缺版/抽到的 side_effect）
- [x] Events：`StartAlchemyRequest` / `InterventionRequest`（调温/注 qi/中途投料）/ `AlchemyOutcome`
- [ ] Channel：`bong:alchemy/start` · `bong:alchemy/tick`（session 状态广播）· `bong:alchemy/intervention` · `bong:alchemy/outcome`
- [x] IPC Schema（agent/packages/schema）：recipe JSON Schema（为 agent 推演 NPC 炼丹准备）

### Client 侧（新增 Store）

- [x] `AlchemyFurnaceStore` — 当前打开炉体状态（tier / integrity / owner）
- [x] `AlchemySessionStore` — 实时 session（elapsed / temp_track / qi_injected / staged_materials / stage_hints）
- [x] `RecipeScrollStore` — 当前卷轴上显示的 recipe 文案 + `LearnedRecipes` 列表 + current_index
- [x] 复用：`InventoryStateStore` / `BackpackGridPanel` / `DragState` / `ItemTooltipPanel` / `Contamination`（HUD 预警读取）

---

## §5 阶段划分

| Phase | 内容 | 验收 |
|---|---|---|
| P0 | RecipeRegistry + JSON 加载 + IPC Schema + 单测 | 启动加载 3 份测试 recipe 无错 ✅ |
| P1 | AlchemyFurnace BlockEntity + LearnedRecipes component + 右键打开 Screen | 方块持久化 session_id · 客户端拿到 RecipeScrollStore |
| P2 | 投料槽 + 材料消耗规则（投入即消耗/不返还）+ stages 多阶支持 | 赌命散三阶投料流程跑通 |
| P3 | 火候进程 tick + 偏差计算 + 精确匹配 outcome 分桶 | 三种测试丹能 perfect/good/flawed/explode ✅ |
| P4 | 残缺匹配 + side_effect_pool 抽取 + LifeRecord 记录 | 缺一料能走 fallback，inspect 看得到试药史 ✅ |
| P5 | 服药 → ContamSource 注入 + 重复服药阈值 + 过量 debuff | 过量服赌命散触发禁服 / 强吃 debuff |
| P6 | BaseOwoScreen 接入（卷轴手札 + 塔科夫背包 + 拖拽投料 + 翻页/拖残卷学习） | 全流程端到端可玩 |

---

## §6 跨 plan 钩子

- [x] **plan-botany-v1**（待立）：替换 §3.2 的 placeholder 材料，接入真实灵草采集
- [x] **plan-forge-v1**（待立）：炼器走同一炉体抽象？或独立？留待 forge plan 决策
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

---

## §9 进度日志

- 2026-04-25：P0 落地确认（server alchemy/ 3909 行 + 3 份测试 recipe）

---

## Finish Evidence

> 验收日期 2026-04-27 · 阶段 P0–P6 全部 ✅（P6 仅 UI 部分接入，世界右键流程留 §1.2 reminder.md 待办）。

### 落地清单（plan §5 阶段表 ↔ 代码）

| Phase | plan 验收点 | 代码落地 |
|---|---|---|
| P0 | RecipeRegistry + JSON 加载 + IPC Schema | `server/src/alchemy/recipe.rs`（503 行，启动期扫 `server/assets/alchemy/recipes/*.json`）· 三份测试 recipe（kai_mai_pill_v0 / hui_yuan_pill_v0 / du_ming_san_v0）· `agent/packages/schema/src/alchemy.ts`（101 行）+ 10 个 sample JSON |
| P1 | AlchemyFurnace + LearnedRecipes + Screen 客户端拿到 RecipeScrollStore | `server/src/alchemy/furnace.rs`（含 `AlchemyFurnace::placed` / `furnace_tier_from_item_id`）· `learned.rs`（93 行 `LearnedRecipes` component）· `client/.../alchemy/state/RecipeScrollStore.java`（103 行）· `ClientRequestV1::AlchemyFurnacePlace` payload + `PlaceFurnaceRequest` 事件 |
| P2 | 投料槽 + stages 多阶投料 + 起炉后锁定不返还 | `server/src/alchemy/session.rs`（315 行，`StagedMaterials.completed_stages / missed_stages`，`apply_tick` 多阶 window 判定） |
| P3 | 火候进程 tick + 偏差计算 + 五桶分桶 | `outcome.rs`（295 行）`DeviationSummary` / `classify_precise` / `ResolvedOutcome::{Perfect,Good,Flawed,Waste,Explode}` |
| P4 | 残缺匹配 + side_effect_pool + LifeRecord | `resolver.rs`（395 行 `resolve()` 入口）· `outcome.rs::build_flawed_result` · `cultivation/life_record.rs` 扩 `AlchemyAttempt` |
| P5 | 服药 → ContamSource + 阈值禁服 + 过量 debuff（基础） | `pill.rs`（189 行 `consume_pill`，`TOXIN_THRESHOLD = 1.0`，`SPOIL_TOXIN_MULT`，`AgePeakCheck`，复用 `cultivation::contamination_tick`） |
| P6 | BaseOwoScreen 全流程（卷轴手札 + 塔科夫背包 + 拖拽投料 + 翻页 / 残卷学习） | `client/src/main/java/com/bong/client/alchemy/AlchemyScreen.java`（814 行，三列 600×340，复用 `BackpackGridPanel` 5×7）· 7 个 store（`AlchemyFurnaceStore` / `AlchemySessionStore` / `RecipeScrollStore` / `AlchemyAttemptHistoryStore` / `ContaminationWarningStore` / `AlchemyOutcomeForecastStore` / `InventoryMetaStore`） |

### 关键 commit

- `575bd982` 2026-04-15 — `feat(alchemy): implement plan-alchemy-v1 server slice (P0–P5)`（14 文件 +2159 行；server alchemy/ 主体 + 3 recipe JSON + 50 单测）
- `2b7d99c7` 2026-04-17 — `feat(HUD): plan-HUD-v1 §11.4 channels end-to-end + textured loadout`（agent schema + 10 sample + `client/alchemy/AlchemyScreen.java` 763 行 + 7 个 store）
- `f862a6e2` 2026-04-21 — `feat(alchemy): plan-alchemy-v1 §1.2 世界放置炉 server-only MVP`（`AlchemyFurnacePlace` payload + 5 集成测试 + `furnace_fantie` item）
- `aba0c3e2` 2026-04-21 — `fix(alchemy): codex P1 race + P2 inventory sync + schema artifact`（mod.rs +72 / schema 同步）
- `e0bf8247` / `3433ae48` / `892503b3` shelflife M5b/c/d — Spoil/Decay/Age 三路径接入 `consume_pill`（plan-shelflife-v1 跨 plan hook）
- `a7050089` 2026-04-24 — `feat(alchemy): plan-mineral-v1 M5 — IngredientSpec.mineral_id 矿物辅料校验`（recipe.rs +76）

### 测试结果

- `grep -rc '#\[test\]' server/src/alchemy/` → **97 个 `#[test]`**（recipe 13 / session 11 / outcome 14 / pill 19 / resolver 16 / furnace 7 / mod 9 / learned 4 / skill_hook 4）
- `client/src/test/java/com/bong/client/alchemy/AlchemyScreenSkillHeaderTest.java` — 2 个 `@Test`（卷轴技艺标题渲染）
- `agent/packages/schema/tests/schema.test.ts`（55 行）含 alchemy schema 正反 sample 对拍

### 跨仓库核验

- **server**：`AlchemyFurnace` / `AlchemySession` / `LearnedRecipes` / `RecipeRegistry` / `consume_pill` / `IngredientSpec.mineral_id` / `ResolvedOutcome` / `DeviationSummary` / `ClientRequestV1::AlchemyFurnacePlace` / `cultivation::Contamination::entries`（复用）
- **agent**：`agent/packages/schema/src/alchemy.ts`（`RecipeEntry` / `OutcomeBucket` / `StageHint` / `ContaminationLevel`）· `client-request.ts` 的 `AlchemyOpenFurnace` / `AlchemyIgnite` / `AlchemyFeedSlot` / `AlchemyIntervention` / `AlchemyTakeBack` / `AlchemyTakePill` / `AlchemyTurnPage` / `AlchemyLearnRecipe` 8 个 variant
- **client**：`AlchemyScreen` + `AlchemyScreenBootstrap` + `state/{AlchemyFurnaceStore,AlchemySessionStore,RecipeScrollStore,AlchemyAttemptHistoryStore,ContaminationWarningStore,AlchemyOutcomeForecastStore,InventoryMetaStore}` · `ClientRequestSender.sendAlchemyOpenFurnace`

### 遗留 / 后续（本 plan 不处理，已记 `docs/plans-skeleton/reminder.md`）

- **Fabric 客户端右键炉方块开 Screen**：当前 `AlchemyScreenBootstrap` 仅绑 K 键 debug 打开；vanilla `UseItemOnC2s` 拦截 → `AlchemyOpenFurnace` payload 未接（server 侧已就绪）
- **多炉 session 路由按 BlockPos**：现 `AlchemyIntervention` 仍按"每玩家一炉"假设，需给相关 payload 加 `furnace_pos`
- **BlockEntity 持久化**：炉是纯内存对象，重启即丢——等 `plan-persistence-v1` 落地
- **Redis channel `bong:alchemy/*`**：未接 agent 推演（server↔client 直接 payload 已能跑）
- **炸炉真正结算**：`ResolvedOutcome::Explode` 的 damage / meridian_crack 还没应用到 caster 实体
- **side_effect tag 映射真实 buff/debuff**：`minor_qi_regen_boost` / `rare_insight_flash` / `qi_cap_perm_minus_1` 等仍是字符串，等 StatusEffect 系统统一
- **测试 recipe 进生产**：三份 placeholder 等 `plan-botany-v1` 落地后替换真实灵草采集
- **跨 plan 钩子尚未接通**：`plan-cultivation-v1` 开脉丹推进经脉进度 · `plan-HUD-v1 §10` 快捷使用栏消费 pill · `plan-inventory-v1` 丹方残卷 item 1×2 + 操作磨损
- **v2+ 设计开放项**（plan §7）：品阶 / 铭文 / 开光 / 自动化炼丹 AutoProfile / NPC agent 侧炼丹 / 丹心识别全部未实装
