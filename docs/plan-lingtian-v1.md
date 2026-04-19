# Bong · plan-lingtian-v1

**灵田专项**（人工种植）。与 `plan-botany-v1`（野生生态）职责分离，共用 `PlantKindRegistry`。作为长线经营循环：开垦 → 种植 → 补灵 → 收获 → 翻新。

**世界观锚点**：
- `worldview.md §十` SPIRIT_QI_TOTAL = 100 灵气零和
- `worldview.md §七` 灵物密度阈值（大型灵田触发天道注视）
- `worldview.md §六` 真元只有染色谱，**不得**引入"雷竹/火稻"五行属
- `worldview.md §十一` 玩家之间默认敌对——**不做**大农庄/NPC 雇工

**关键复用**：
- `PlantKindRegistry`（botany 定义）—— 灵田作物是 PlantKind 的**亚种**，共享 item/品质机制
- 采集浮窗（`harvest-popup.svg`）—— 收获走同一 UI 范式，手动/自动二选（熟练度解锁）
- `BotanySkillStore` —— 种田与采药共用 `herbalism` skill（统一 herbalism 技艺）

**交叉引用**：`plan-botany-v1.md`（PlantKind + 浮窗）· `plan-alchemy-v1.md`（作物 → 丹方材料）· `plan-skill-v1.md`（herbalism）· `plan-zhenfa-v1.md`（欺天阵保田）· `plan-inventory-v1.md`（锄头 / 骨币 / 兽核 item）。

**进度**（2026-04-19，分支 `plan-lingtian-v1`）：
- ✅ **P0 骨架已落**：`server/src/botany/`（PlantKindRegistry + plants.toml 含 §3.1 测试三作物 + 1 野生 only 回归样本）+ `server/src/lingtian/plot.rs`（LingtianPlot Component + CropInstance + 翻新方法）+ `register(&mut app)` 双双接入 main.rs
- ✅ **P1 数据 / 状态机已落**：`hoe.rs`（HoeKind 三档 + uses_max + 耐久成本）+ `assets/items/lingtian.toml`（hoe_iron / hoe_lingtie / hoe_xuantie）+ `terrain.rs`（地形适合性 / 拒绝原因）+ `session.rs`（TillSession 手动 40t / 自动 100t · RenewSession 100t · cancel · 重复 tick no-op）+ `events.rs`（StartTill / TillCompleted / StartRenew / RenewCompleted）。**纯状态机层就绪，ECS system / 方块 ↔ 玩家输入桥未接**
- ⏳ **P1 收尾未做**：ECS 驱动 system（事件 → session 推进 → plot 落地）· valence BlockKind ↔ TerrainKind 适配 · 玩家主手锄读取 · inventory.durability 扣减
- ⏳ **P0/P1 共同收尾**：BlockEntity 持久化（依 plan-persistence-v1）· 方块放置 e2e 验收
- ⏳ **P2+ 全未动**：生长 tick / plot_qi / 区域漏吸 / 补灵浮窗 / 收获 / 偷菜偷灵 / 密度阈值 / 客户端 UI

测试：546/546 全过（含 botany + lingtian 17 单测）；我的文件 clippy 0 警告。

---

## §0 设计轴心

- [ ] **混合灵气模型**：基础生长微量吸区域 · 补灵（骨币/兽核/区域抽吸）加速与提品
- [ ] **长线低频经营**：补灵 3-7 天（72-168h real time）一次；不做"每日上线"强度
- [ ] **灵气零和**：所有入 plot_qi 的灵气都记在区域账本上（从哪吸就从哪扣）
- [ ] **密度阈值**：大型灵田（多 plot 聚集 + 高 plot_qi）触发天道注视（worldview §七）
- [ ] **收获复用采集浮窗**：熟后右键作物走 harvest-popup 手动/自动路径
- [ ] **NPC 种田**不做（v2+ 借 npc-ai）

---

## §1 子系统拆解

### §1.1 田块模型（Plot）

```rust
#[derive(Component)]
pub struct LingtianPlot {
    pub pos: BlockPos,
    pub owner: Option<Entity>,
    pub crop: Option<CropInstance>,    // 当前作物（PlantKind 亚种）
    pub plot_qi: f32,                  // 田块独立灵气池（0 ~ plot_qi_cap）
    pub plot_qi_cap: f32,              // 由地形 / 水源 / 阵法修饰
    pub harvest_count: u32,            // 累计收获次数
    pub last_replenish_at: Tick,       // 最近补灵时间（tick）
}

pub struct CropInstance {
    pub kind: PlantId,                 // 与 botany PlantKindRegistry 共用
    pub growth: f32,                   // [0, 1]
    pub quality_accum: f32,            // 生长过程累积品质修饰
}
```

- [ ] plot_qi_cap 基线 1.0；水源相邻 +0.3；处于湿地 biome +0.5；聚灵阵内 +1.0（上限 3.0）
- [ ] `harvest_count ≥ N_RENEW`（默认 5）→ 田块进入"贫瘠"状态，必须翻新才能再种

### §1.2 锄头 · 开垦 · 种植

#### §1.2.1 锄头 item

- [ ] 1×2 形状，耐久 N 次（铁 20 / 灵铁 50 / 玄铁 100）
- [ ] **必须主手持握**（不在主手不触发开垦/翻新交互）
- [ ] 消耗 1 耐久 / 开垦 · 1 耐久 / 翻新

#### §1.2.2 开垦流程（空地 → 空 plot）

1. 主手持锄 + 右键空地（grass / dirt / swamp_mud） → 弹开垦浮窗（复用采集浮窗范式）
2. 2s 手动 / 5s 自动（自动需 `herbalism` Lv.3+）
3. 完成 → spawn `LingtianPlot` block（空田，无作物）+ 锄头耐久 −1 + herbalism XP +1
4. 非适合地形（沙 / 石 / 冰 / 死域）→ 浮窗直接灰掉"开始"

#### §1.2.3 种植流程（空 plot → 有作物）

空 plot 右键（不需要主手锄头）→ 弹**种植浮窗**（UI 见 `docs/svg/lingtian-planting.svg`，种子 item 定义见 §1.2.4）：

1. 浮窗列出玩家背包内所有"种子 item"（从 `SeedRegistry` 匹配）
2. 玩家选 1 种种子 → 点击"播种"（1s session，可取消）
3. 完成 → 种子 −1 · `LingtianPlot.crop = CropInstance { kind, growth: 0, quality_accum: 0 }` · XP +1
4. 已有作物的 plot → 右键直接进生长/补灵浮窗（§1.4），不提供种植

#### §1.2.4 种子 item

- [ ] **种子是独立 item**（非作物本体）：`{canonical_plant_id}_seed`（例：`ci_she_hao_seed` / `ning_mai_cao_seed`）· 1×1 · 栈上限 32
- [ ] 获取：采集成熟作物时有 10-30% 掉落种子（按作物稀有度，通用 30% / 区域专属 20% / 极稀 10%）· 或散修交易 / 残卷
- [ ] 不是所有正典植物都有种子——**野生 only** 物种（负灵域噬脉根、伪灵脉天怒椒、灵眼石芝等）**无种子**，无法人工种植（`cultivable: false`）
- [ ] MVP 可种清单见 §2 表格 `灵田可种` 列

#### §1.2.5 锄头不在主手？

右键空地但主手非锄 → 不弹浮窗，**actionbar** 提示"需持锄"（2s 自动消失，不进事件流）

### §1.3 灵气模型（混合 · 用户决策 C）

```
每 tick：
  base_drain = crop.kind.growth_cost × BASE_DRAIN_RATE
  if plot_qi >= base_drain:
      crop.growth += GROWTH_PER_TICK × quality_multiplier(plot_qi)
      plot_qi -= base_drain
  else if zone.spirit_qi >= base_drain × ZONE_LEAK_RATIO:
      # plot 没灵气时，微量从区域漏吸（慢）
      crop.growth += GROWTH_PER_TICK × 0.3
      zone.spirit_qi -= base_drain × ZONE_LEAK_RATIO
  else:
      crop.growth 停滞
```

- [ ] `ZONE_LEAK_RATIO` 默认 0.2（plot 空时生长速率 30%，区域扣 20% 的 base_drain）
- [ ] quality_multiplier：plot_qi 越满，成长 tick 的 quality_accum 越高
- [ ] 补灵后 plot_qi 回满 → 进入"丰沛期"，crop 额外得到品质加成

### §1.4 补灵交互（session）

玩家右键 plot，若 `plot_qi < plot_qi_cap × 0.3` → 弹补灵浮窗：

| 来源 | 代价 | plot_qi 回补 | 扣除目标 |
|---|---|---|---|
| 区域抽吸 | 0（但慢 8s）| +0.5 | zone.spirit_qi -0.5 |
| 骨币（worldview §九）| 骨币 1 枚 | +0.8 | 物品栏骨币 |
| 异兽核（worldview §十）| 1 个 | +2.0（直接拉满）| 物品栏兽核 |
| 灵水 | 1 瓶 | +0.3 | 物品栏灵水 |

- [ ] 同范式浮窗（2-8s session，可打断）
- [ ] 补灵冷却：同一 plot 每 3-7 天真实时间（72-168h）补一次
- [ ] **过早补 / 超 cap 溢出规则**：溢出量 = `replenish_amount − (plot_qi_cap − plot_qi_current)`，**溢出部分回馈环境**（`zone.spirit_qi += overflow`），不是亏损 · 但来源材料（骨币/兽核/灵水）**不退**（代价仍付）
- [ ] 补灵事件走 `bong:lingtian/replenish` channel，大规模补灵触发天道密度阈值（见 §5）

### §1.5 收获（复用采集浮窗）

- [ ] `crop.growth >= 1.0` → plot 上方显示"熟"标记（顶部小图标）
- [ ] 右键熟作物 → 弹 **harvest-popup**（同 botany §1.3 范式）
  - 手动 2-3s
  - 自动 5-8s（`herbalism` Lv.3+ 解锁）
  - XP / 品质规则完全相同
- [ ] 收获后 `harvest_count += 1` · 作物消失 · plot 进入空田状态
- [ ] 作物采走 = 灵气永远流出（与 botany §2 闭环一致）

### §1.6 翻新

- [ ] `harvest_count >= N_RENEW (5)` → plot 变"贫瘠"：外观灰化、不能种
- [ ] 翻新交互：主手持锄 + 右键 plot → 5s session
  - 消耗 1 把锄头耐久 · +小量骨币 / 兽骨粉之类的"肥料"（MVP 占位）
  - 完成后 plot 重置 harvest_count = 0，plot_qi 清零 · herbalism XP +2
- [ ] 不翻新的 plot 永久留存但不可种——玩家可主动拆除回收位置

### §1.7 所有权 · 偷菜 / 偷灵（worldview §十一 默认敌对）

玩家之间匿名敌对 → plot 所有权**不提供强制保护**，可被他人操作：

| 行为 | 结果 | 记录 |
|---|---|---|
| 非 owner **补灵** | 生效（plot_qi 上去，来源材料从**操作者**背包扣）| `LifeRecord` 记一笔（owner 可读到"某人替我补了灵"匿名化为"有修士"）|
| 非 owner **偷收获**（熟后）| 作物 drop 到**操作者**背包；`harvest_count` 照加；作物采走灵气流出规则不变 | LifeRecord 双方都记（owner 一条"被偷"，操作者一条"偷人"）|
| 非 owner **偷灵**（plot_qi 未空时右键"吸灵"新动作）| plot_qi 直接减，80% 注入**操作者** qi_current，**20% 散逸回馈 zone.spirit_qi**（保持零和）| 同上 |
| 非 owner **破坏 plot**（铲除）| 可执行，但触发全服 narration "有人铲了谁的田"（匿名） | LifeRecord 记 "毁田" tag |

- [ ] **防护手段**（玩家主动成本）：
  - 灵龛方圆 5 格内的 plot 他人无法破坏（但补灵 / 偷收获仍可，灵龛只挡方块破坏）
  - 聚灵阵 / 欺天阵（plan-zhenfa）覆盖可减少被天道盯 + 可选加"禁他人操作"flag（代价高）
  - 大量 plot 聚集 = 肥肉，密度阈值触发天道清算
- [ ] 偷灵 / 偷菜不自动通知 owner（匿名系统）；owner 需下次上线右键 plot 时才看到"plot_qi 蒸发"或"被偷收获"记录

---

## §2 作物表（**全对齐正典**，来自 `docs/library/ecology/末法药材十七种.json` + `辛草试毒录.json`）

| 作物 ID | 野生 biome / 条件 | `cultivable` | plot_qi 消耗 | 生长时长 | 种子掉率 | 用途 |
|---|---|---|---|---|---|---|
| `ci_she_hao` 刺舌蒿（微辛）| 初醒原外围 | ✓ | 低 | 8h | 30% | 凝脉散引 / 临时提速 |
| `ning_mai_cao` 凝脉草 | 馈赠区 · > 0.4 | ✓ | 中 | 16h | 30% | 凝脉散主料 |
| `hui_yuan_zhi` 回元芷 | 湿地 | ✓ | 低中 | 12h | 30% | 回元丹主料 |
| `gu_yuan_gen` 固元根 | 深处 · > 0.6 | ✓（需补灵保）| 高 | 96h（4 天）| 20% | 固元丹主料 |
| `chi_sui_cao` 赤髓草（烈辛）| BloodValley 红砂岩 | ✓（需红砂岩基底替代物——灵田种代价 +50%）| 中高 | 36h | 20% | 凝脉散高阶 / 固元丹辅 |
| `zhen_jie_zi` 针芥子（烈辛）| 青云残峰 / 灵泉湿地 | ✓（需水源相邻）| 中 | 24h | 20% | 凝脉散疏通药 |
| `qing_zhuo_cao` 清浊草 | 馈赠区/负灵域交界 | ✓（难，需特殊阵法）| 中 | 48h | 15% | 中和异种真元 |
| `an_shen_guo` 安神果 | 青云残峰外门梯田 | ✓（果树型，长期）| 中 | 240h（10 天）| 15% | 镇心 / 顿悟辅 |
| `jie_gu_rui` 解蛊蕊 | 幽暗地穴 | ✓（需低光）| 低 | 12h | 25% | 解毒蛊 |
| `ling_mu_miao` 灵木苗 | LingquanMarsh（静态点）| ✓（极慢 20 天）| 高 | 480h | 10%（稀）| forge 载体 |

**野生 only**（`cultivable: false`，无种子，无法人工种）：
- `yang_jing_tai` 养经苔（死域边缘）
- `hui_jin_tai` 灰烬苔（残灰方块表面）
- `shi_mai_gen` 噬脉根（负灵域裂缝）
- `ling_yan_shi_zhi` 灵眼石芝（灵眼上方 ⏳）
- `ye_ku_teng` 夜枯藤（幽暗深处）
- `shao_hou_man` 烧喉蔓（地穴洞壁）
- `kong_shou_hen` 空兽痕（负灵域残灰）
- `tian_nu_jiao` 天怒椒（伪灵脉陷阱）
- **毒性五味**：蜉蝣花 · 无言果 · 黑骨菌 · 浮尘草 · 终焉藤（禁种）

**不加**（worldview 禁）：雷竹 / 火稻 / 水莲 等五行属作物。

---

## §3 MVP

### §3.1 测试三作物（正典名）

| 作物 ID | 生长时长 | 验证意图 |
|---|---|---|
| `ci_she_hao` 刺舌蒿 | 8h | 短周期基线循环（验证种植→生长→收获闭环）|
| `ning_mai_cao` 凝脉草 | 16h | 需补灵，验证 plot_qi 管理 |
| `ling_mu_miao` 灵木苗 | 480h（20 天）| 极长线，验证补灵节奏 3-7 天 + forge 接入 |

### §3.2 MVP 范围

- [ ] 锄头 item（铁 / 灵铁 / 玄铁三档）+ 种子 item（三种）
- [ ] 开垦 session（空地 → 空 plot）
- [ ] **种植 session**（空 plot → 选种子 → 有作物）
- [ ] 三作物 PlantKind 亚种注册（`cultivable: true`）
- [ ] 生长 tick + plot_qi 消耗 + 区域漏吸（30% 慢速兜底）
- [ ] 补灵浮窗（4 来源）+ 溢出回馈环境规则
- [ ] 收获复用 harvest-popup
- [ ] 翻新 session
- [ ] 偷菜 / 偷灵（非 owner 可操作 + 匿名记录到 LifeRecord）
- [ ] herbalism XP 数值表（见 §3.3）

### §3.3 herbalism XP 数值（刻意偏低，不膨胀）

| 动作 | XP | 备注 |
|---|---|---|
| 开垦 | +1 | 一次一 |
| 种植 | +1 | 放种子 |
| 补灵（区域抽吸 8s）| +1 | 只第 1 来源给 |
| 收获（手动）| +2 | 与采药一致 |
| 收获（自动，需 Lv.3）| +5 | 熟练加成 |
| 翻新 | +2 | 一次性 |
| 偷菜 / 偷灵 | 0 | 不给 XP（动作有利，不该再奖励熟练）|

对照 `plan-skill-v1.md` 曲线 `XP_to_next(lv) = 100 × (lv+1)²`：
- Lv.0 → 1 需 100 XP ≈ 30-50 次动作（可控）
- Lv.3 解锁自动 ≈ 1000 XP ≈ 500 次动作（长线）
- Lv.10 封顶 ≈ 38500 XP（不膨胀，老玩家也要磨）

---

## §4 数据契约

### Server

- [ ] `LingtianPlot` component + BlockEntity（持久化 crop / plot_qi / harvest_count）
- [ ] `SeedRegistry` resource（启动期从 PlantKindRegistry 派生，筛 `cultivable: true` → 自动生成 seed item 定义）
- [ ] `LingtianTick` system（与 `BotanyTick` 同调度但独立，period = 1 min）
- [ ] Session 池（各 session 共用同一 resource map，按 kind 区分）：
  - `TillSession`（开垦 2-5s）
  - `PlantingSession`（种植 1s）
  - `ReplenishSession`（补灵 2-8s）
  - `RenewSession`（翻新 5s）
- [ ] Events：`PlotTilled` / `PlotPlanted` / `PlotReplenish` / `PlotHarvest` / `PlotRenew` / `PlotStolen` / `PlotQiDrained`（偷灵）
- [ ] Channel：`bong:lingtian/tick` · `bong:lingtian/plant` · `bong:lingtian/replenish` · `bong:lingtian/harvest` · `bong:lingtian/steal`
- [ ] 接入 `zone.spirit_qi` 双向流动（区域漏吸 / 补灵扣 / 溢出回馈 / 偷灵 20% 散逸）

### Client

- [ ] `LingtianPlotStore`（打开浮窗时填充当前 plot 完整状态）
- [ ] `PlantingSessionStore`（种植浮窗：列背包内匹配 `SeedRegistry` 的 item + 选中的 seed_id）
- [ ] 复用：`HarvestSessionStore`（收获/开垦/翻新/补灵共用 session 进度条）· `InventoryStateStore` · `BotanySkillStore`（herbalism）

---

## §5 阶段划分

| Phase | 内容 | 验收 |
|---|---|---|
| P0 | LingtianPlot 组件 + BlockEntity + PlantKind `cultivable` flag | 方块放置持久化 |
| P1 | 锄头 item + 开垦 session + 翻新 session | 可开可翻 |
| P2 | 生长 tick + plot_qi 消耗 + 区域漏吸 | `ci_she_hao` 8h 成熟闭环 |
| P3 | 补灵浮窗 4 来源 + 冷却 72-168h | 三种作物都能补出丰沛 |
| P4 | 收获走 harvest-popup（复用）+ herbalism XP 联动 | 自动收获熟后触发 |
| P5 | 密度阈值公式（见下）+ 天道密度事件 | 10 plot 聚集达阈值触发 narration + 灵气扰动 |
| P6 | 偷菜 / 偷灵 · 非 owner 操作 · LifeRecord 匿名记录 | 他人可补/偷，owner 次日上线可见记录 |

### §5.1 密度阈值公式（用户决策）

```
zone_pressure = Σ (crop.kind.growth_cost × crop_count)   // 作物需求
              − zone_natural_supply                       // 环境补充（区域自然回复）
              − Σ replenish_recent_7d                     // 最近 7 天所有补灵总量
```

- [ ] `zone_pressure > THRESHOLD_LOW` → 天道 narration 提示（冷漠古意）
- [ ] `> THRESHOLD_MID` → 该区异变兽刷新率 +30%
- [ ] `> THRESHOLD_HIGH` → 区域内所有 plot_qi 瞬时清零 + 3x3 范围生成道伥（worldview §八.1 注视规则）
- [ ] 阈值具体数值 MVP 用占位（`LOW=0.3 / MID=0.6 / HIGH=1.0`），Phase 上线后调参

---

## §6 跨 plan 钩子

- [ ] **plan-botany-v1**：共用 `PlantKindRegistry`，`cultivable: bool` flag 区分可种；共用 harvest-popup + BotanySkill
- [ ] **plan-skill-v1**：`herbalism` 技艺覆盖采集 + 种植 + 开垦 + 翻新（统一熟练度）
- [ ] **plan-alchemy-v1**：配方材料走正典（`ci_she_hao` / `ning_mai_cao` / `hui_yuan_zhi` 等 `cultivable: true` 作物，均可采集或种植）· alchemy JSON 的 placeholder 名（`kai_mai_cao` 等）需批量改正典，见 reminder.md
- [ ] **plan-forge-v1**：灵木苗种 20 天得灵木（forge 载体）
- [ ] **plan-zhenfa-v1**：聚灵阵围 plot → `plot_qi_cap +1.0`；欺天阵护田避天道注视
- [ ] **plan-inventory-v1**：锄头 item（铁/灵铁/玄铁）+ **种子 item**（每可种作物一种，`{id}_seed`）+ 肥料占位 item + 骨币/兽核（已存在）
- [ ] **天道系统**（§5 P5）：密度阈值触发 narration + 灵气清零事件
- [ ] **alchemy 废料反哺**（§7 开放，跨 plan 闭环）：炼丹废渣 → 肥料，减少翻新代价

---

## §7 TODO / 开放问题（v2+）

- [ ] NPC 散修种小块田（读同一 PlantKindRegistry）
- [ ] 作物二级加工（`ci_she_hao` → 凝脉散引 / `an_shen_guo` → 安神汤 等）
- [ ] 天气 / 季节影响生长速率
- [ ] 特殊作物变异（天劫余波 / 负灵域漂移种子 / 真元色沾染产生变种）
- [ ] 灵田与聚灵阵协同（plot_qi_cap 加成具体曲线）
- [ ] alchemy 废料反哺 lingtian 的具体配比
- [ ] 自动补灵 / 自动收获的傀儡（plan-alchemy §7 AutoProfile 同类扩展）
- [ ] 偷菜 / 偷灵的攻击性升级：连续偷同人是否触发 PVP flag？

---

## §8 风险与对策

| 风险 | 对策 |
|---|---|
| 灵田变成"放下就跑"的低操作 | 补灵节奏 3-7 天强制上线一次；不补就漏吸区域（慢 30%） |
| 大玩家屯田垄断灵气 | 密度阈值触发天道清算（worldview §七）· plot 上限可硬设（每玩家 ≤ 8 plot）|
| 灵木 20 天成熟太长 | 平衡时看玩家反馈；可加速卡（骨币换 -N 天）|
| 翻新卡关消耗过多锄头 | 锄头耐久按档分配（玄铁 100 次可翻新 20 次）|
| 补灵溢出 | 溢出量回馈 zone.spirit_qi 保持零和；但来源材料不退，代价仍付（UI 浮窗提示"将溢出 X，回馈本区"）|
| 偷菜/偷灵被滥用成人身攻击 | 匿名系统下只能通过 LifeRecord 间接感知，无法锁定具体"冤家"；§7 留 TODO 可升级为连续偷触发 PVP flag |
