# Bong · plan-skill-v1

**子技能系统专项**。统一管理非境界的"职业熟练度"（采药 / 炼丹 / 锻造 ...），让这些活动在不破坏 worldview "境界为根本"的前提下有独立成长曲线。本 plan 是 alchemy / forge / botany / lingtian 等动作系 plan 的**共同依赖层**——那些 plan 只负责"触发 +XP"，效果如何作用回去看本 plan。

**世界观锚点**：
- `worldview.md §三` 材料/丹药/器物为辅助，技艺再熟也不能替代境界
- `worldview.md §十二` 玩家死透后 skill 归零（经验在玩家脑子里而非角色身上）
- `worldview.md §八` 升级 narration 走冷漠古意语调

**本 plan 不管**：境界系统（cultivation）· 战斗武学粒度设计（v2+）

**交叉引用**：`plan-botany-v1.md §1.3`（采药触发）· `plan-lingtian-v1.md §3.3`（herbalism XP 数值已给）· `plan-alchemy-v1.md`（火候容差 / 残缺池权重）· `plan-forge-v1.md`（Tempering 命中窗口）· `plan-inventory-v1.md`（skill_scroll item）· `plan-cultivation-v1.md`（境界软挂钩）· `plan-death-lifecycle-v1.md`（死透归零）。

---

## §0 设计轴心

- [ ] **子技能 ≠ 境界**——境界决定真元上限/经脉/神识；skill 只影响"做这件事有多熟"
- [ ] **做中学**主路径：每次完成对应动作 +XP（失败给得少）；**残卷顿悟**加速（大额跳级）
- [ ] **XP → Lv 线性累积**（Lv.0-10），上限由境界**软挂钩**（可超 cap 硬练，效率 ×0.3）
- [ ] **不衰退**（worldview"一学不忘"基调）
- [ ] **死透归零**：新角色与旧角色无机制关联（worldview §十二）
- [ ] **效果线性非指数**：避免 Lv.10 碾压 Lv.1 的 power creep

---

## §1 首批技能清单（MVP）

| skill_id | 中文 | 来源 plan | 影响概述 |
|---|---|---|---|
| `herbalism` | 采药（含种植）| botany §1.3 · lingtian §3.3 | 自动采集解锁 / 品质分布 / 时长 / 种子掉率 |
| `alchemy` | 炼丹 | alchemy | 火候容差 / 残缺池 side_effect 权重 / 丹毒抗性 |
| `forging` | 锻造 | forge | Tempering 命中窗口 / 允许失误数 / 铭文失败率 |

**不做**（v2+，见 §11 TODO）：战斗武学（剑术/刀法/拳法粒度难定）· 修炼本身（境界已有独立系统）· 阵法（zhenfa 自带进阶）· 叙事影响力

---

## §2 升级模型

### §2.1 XP 曲线

```rust
fn xp_to_next(lv: u8) -> u32 {
    100 * ((lv as u32) + 1).pow(2)
}

// Lv.0 → 1 :   100 XP
// Lv.1 → 2 :   400 XP  (累计 500)
// Lv.2 → 3 :   900 XP  (累计 1_400)
// Lv.3 → 4 : 1_600 XP  (累计 3_000)
// Lv.5 → 6 : 3_600 XP  (累计 11_000)
// Lv.9 → 10: 10_000 XP (累计 38_500)
```

- [ ] Lv.3 通常是"解锁自动化动作"的门槛（botany/lingtian 已采用）
- [ ] Lv.10 是硬上限，不可超

### §2.2 XP 来源类型

| 来源 | 典型 | 数量级 |
|---|---|---|
| 做中学 | 动作完成（采集/播种/炼丹成功）| +1 ~ +6 / 次 |
| 失败给 | 炼丹炸炉 / 锻造废品 | +0 ~ +2 / 次（按努力计）|
| 残卷顿悟 | 吞下 `skill_scroll` | +100 ~ +2000 / 个（相当于跳半级到 2 级）|
| 境界突破 | 突破到新境界 | **不给 XP**，只解锁新 cap（已学等级可"激活"未曾发挥的效果）|
| 师承（v2+） | 师父授艺 session | +N（师父消耗自身 qi）|

### §2.3 升级 narration

每升一级弹一条冷漠古意 narration（`worldview §八`）：

- 好例：`"你摘得百草渐熟，今已识八分。（herbalism Lv 3 → 4）"`
- 坏例：`"恭喜！采药升级！"`（太游戏化）

走 `bong:skill/lv_up` channel，agent 消费生成 narration。

---

## §3 获得路径详解

### §3.1 做中学

动作完成即自动判 +XP，**无需玩家确认**。具体数值由各 plan 定义（见 §7 汇总表）。

- [ ] **失败也给**：例如炼丹炸炉 +1（比成功 +3 少）· 锻造废品 +0（没努力够）
- [ ] **动作多样性奖励**：连续重复同一配方/动作，XP 递减 10% / 次，最多扣到 50%（防宏磨）
- [ ] **触发去重**：同一 session 内多阶段的 XP 合并成一条事件

### §3.2 残卷顿悟

`skill_scroll` item 定义（跨 plan 统一规格）：

```rust
pub struct SkillScroll {
    pub skill_id: SkillId,
    pub xp_grant: u32,     // 100 ~ 2000 按稀有度
    pub title: String,     // 例："《百草图考·残》"
    pub flavor: String,    // worldview 古意短句
}
```

- [ ] **形状**：通用 1×1（某些高阶珍本 1×2，如"《百草图考·残》"，由具体 scroll 定义）· 栈上限 1（独本）
- [ ] **item 上携带唯一 `scroll_id`**（非仅 skill_id），用于判"已学"
- [ ] 获取：散修掉落 / 遗迹宝箱 / 高阶 NPC 交易 / 图书馆档案
- [ ] **使用**：从**任意容器**（主背包/小口袋/前挂/腰包）拖到 InspectScreen 技艺 tab 残卷槽 → 残卷消失 + XP 进账 + `consumed_scrolls.insert(scroll_id)`
- [ ] **已学判定**：`SkillSet.consumed_scrolls: HashSet<ScrollId>` 记录一生读过的每份 scroll_id；相同 scroll_id 再次拖入 → **不消耗，不进 XP**，tooltip 提示"此卷已悟"
- [ ] **拖拽合法性校验**（client 拦截 + server 二次校验）：
  - `skill_scroll` → 残卷槽 ✓
  - `recipe_scroll`（丹方残卷）/ `blueprint_scroll`（图谱残卷）/ 其他任何 item → 槽 ✗ · 拖拽 hover 时红框 + tooltip "此物非 skill，不可入"
  - 未知 skill_id 的残卷（未来新 skill 的 scroll，老客户端读不到）→ ✗ · 提示"不识此技，暂不能悟"
- [ ] **重复阅读处理**：scroll_id 命中 consumed_scrolls → 残卷**不消耗**（玩家可重卖/转手）· 若想强制消耗（例：空出背包位）→ 右键 drop 即可
- [ ] 未来"复制残卷" / "抄写"走抄写系统（v2+）

### §3.3 境界突破（只开 cap，不给 XP）

突破到新境界 → `Lv.cap` 上调，已达当前 cap 的 skill 解锁被压制的效果（见 §4）。

---

## §4 境界软挂钩

```
醒灵 → cap 3   · 引气 → cap 5   · 凝脉 → cap 7
固元 → cap 8   · 通灵 → cap 9   · 化虚 → cap 10
```

- [ ] **超 cap 硬练**：Lv 仍可涨，但每次 XP 进账 ×0.3（慢）
- [ ] **cap 压制效果**：Lv > cap 时，`effective_lv = min(real_lv, cap)`——展示给玩家的仍是 `real_lv`（你知道自己熟，但经脉不够用不出来）
- [ ] **境界跌落**：skill real_lv 不扣，但 cap 下修 → effective_lv 打折 → narration 冷漠提示（"经脉萎缩，往日手艺施展不开"）
- [ ] **inspect 面板显示**：`Lv.7 / cap 5`（灰掉超出部分），点击展开解释

---

## §5 UI

### §5.1 InspectScreen 新 tab "技艺"（layer B · 三列布局）

> 草图 `docs/svg/inspect-skill.svg`。整体 1920×1080，三列 380 + 1020 + 420。

**InspectScreen 级约定**（跨所有 tab 一致）：
- [ ] **右侧常驻背包**：任何 tab 选中，右侧 420 列始终渲染塔科夫式背包（多容器 tab：主 5×7 / 小口袋 3×3 / 前挂 3×4 / 腰包 v2+）
- [ ] **左侧换 tab 内容**：装备 / 修仙 / 伤口 / 状态 / **技艺** 切换只刷新左侧
- [ ] **DragState 跨 tab 持续**：拖拽中切 tab 不重置，松开时按目标 tab 的当前 drop target 判定合法性（残卷槽只在技艺 tab 内存在 → 切到其他 tab 拖拽相当于无目标 → 回弹原位）

**技艺 tab 三列**：
- [ ] **左 380**：skill 列表 · 每行 `{ 图标 · 中文名 · Lv.X / cap Y · XP 进度条 · 最近 +XP }`；**灰显未学**（v2+ 战斗武学 / 阵法 / 师承）；底部**残卷拖入槽**（单格 1×2 容纳珍本）
- [ ] **中 1020**：选中 skill 详情 · 四象限：XP 累计曲线 / Lv 效果表（当前 Lv 行高亮）/ 近期流水 / 里程碑 · 底部"当前 Lv 实际生效"大横条 + 品质分布 mini 对比
- [ ] **右 420**：常驻背包（见上）

**自定义组件**（非通用）：
- [ ] `SkillRowComponent`（左列每行）
- [ ] `SkillCurveComponent`（XP 曲线，Canvas）
- [ ] `SkillMilestoneListComponent`（里程碑条目）
- [ ] 背包仍复用 `BackpackGridPanel`

### §5.2 场景浮窗就地显示

各 plan 自己的 Screen / 浮窗顶栏显示本 skill 的 `Lv.X`（已在 `harvest-popup.svg` 采用）：

- alchemy-furnace.svg → 中央栏顶部加 "炼丹 Lv.X · 本次火候容差 +Y%"
- forge-station.svg → 同上，"锻造 Lv.X · 命中窗口 +Y tick"
- harvest-popup.svg → **已有**"采药经验 Lv.4"

---

## §6 各 skill 效果表（线性曲线，非指数）

### §6.0 插值规则

下列表格只列关键 Lv（端点）。**Lv 之间一律按线性插值**计算实际效果：

```rust
fn interp(lv: u8, pts: &[(u8, f32)]) -> f32 {
    // pts 按 lv 升序；返回 lv 在相邻端点间的线性插值
    for w in pts.windows(2) {
        let (l0, v0) = w[0]; let (l1, v1) = w[1];
        if lv >= l0 && lv <= l1 {
            let t = (lv - l0) as f32 / (l1 - l0) as f32;
            return v0 + (v1 - v0) * t;
        }
    }
    pts.last().unwrap().1
}
```

- [ ] 示例：herbalism Lv.4 自动采时长 = interp(4, [(3, 8.0), (5, 6.0)]) = **7.0s**
- [ ] 未显式写端点的项，Lv.0 为"基础无加成"（自动 ✗ / 时长 +0 / 率 +0）
- [ ] cap 压制：`effective_lv = min(real_lv, cap)`，实际生效用 effective_lv 插值

### §6.1 herbalism

| Lv | 自动采集 | 手动时长 | 种子掉率加成 | 品质偏移 |
|----|---------|---------|------------|---------|
| 0 | ✗ | +0 | +0% | 0 |
| 1 | ✗ | -0.2s | +2% | +5% 品 |
| 3 | ✓ 解锁（8s） | -0.5s | +5% | +10% 品 |
| 5 | ✓（6s） | -1.0s | +10% | +15% 品 |
| 7 | ✓（5s） | -1.2s | +15% | +20% 品 |
| 10 | ✓（5s）| -1.5s | +25% | +30% 品 + 5% 极 |

**品质偏移 → 四档分布的具体映射**（当基础分布为 `[劣 20 / 普 30 / 良 40 / 极 10]` 时）：

```
bias_pts = +N%  （来自 skill_effect.quality_bias）
新 劣 = max(0, 基础劣 - bias × 0.6)     // 劣最减
新 普 = max(0, 基础普 - bias × 0.3)     // 普减少
新 良 = 基础良 + bias × 0.7              // 良加最多
新 极 = 基础极 + bias × 0.2              // 极微加
// 归一化到 100%
```

- [ ] 例：herbalism Lv.4 品质偏移 +12% → `劣 12.8 / 普 26.4 / 良 48.4 / 极 12.4`
- [ ] Lv.10 额外 "+5% 极" 是**独立项**，先做四档偏移，再把 5% 从 劣/普 平均扣给 极
- [ ] 基础分布由 PlantKind / Recipe / Blueprint 各自定义，skill 只修饰

### §6.2 alchemy

| Lv | 火候容差 | 残缺池坏 side_effect 权重 | 丹毒抗性 |
|----|---------|------------------------|---------|
| 0 | ×1.0 | ×1.0 | +0% |
| 1 | ×1.05 | ×0.95 | +2% |
| 3 | ×1.15 | ×0.85 | +5% |
| 5 | ×1.25 | ×0.75 | +10% |
| 7 | ×1.35 | ×0.60 | +15% |
| 10 | ×1.50 | ×0.40 | +25% |

- [ ] 丹毒抗性 = Contamination 的 `purge_rate` 额外加成（见 alchemy §2 复用 contamination_tick）

### §6.3 forging

| Lv | Tempering 窗口 +tick | 允许失误 +次 | 铭文失败率 - |
|----|---------------------|-------------|-------------|
| 0 | +0 | +0 | -0% |
| 1 | +1 | +0 | -3% |
| 3 | +3 | +1 | -10% |
| 5 | +5 | +1 | -15% |
| 7 | +6 | +2 | -22% |
| 10 | +8 | +3 | -30% |

---

## §7 XP source 汇总表（跨 plan）

所有触发点都应同步这张表；各 plan 内部数值应引用此处（source of truth）。

### §7.1 herbalism

| 动作 | XP | 来源 plan |
|---|---|---|
| 开垦 | +1 | lingtian §3.3 |
| 种植 | +1 | lingtian |
| 野外采集 手动 | +2 | botany |
| 野外采集 自动（Lv.3+）| +5 | botany |
| 灵田收获 手动 | +2 | lingtian |
| 灵田收获 自动（Lv.3+）| +5 | lingtian |
| 补灵（区域抽吸）| +1 | lingtian |
| 翻新 | +2 | lingtian |
| 偷菜 / 偷灵 | 0 | lingtian §1.7（动作已占便宜不再奖熟练）|

### §7.2 alchemy

| 动作 | XP | 备注 |
|---|---|---|
| 炼成（perfect）| +6 | 顶级奖励 |
| 炼成（good）| +3 | 常态 |
| 炼出残次（flawed / fallback）| +2 | 试错也学到 |
| 炸炉（explode）| +1 | 失败付代价同时给 1 XP |
| 完全废料（waste）| +0 | 投错料乱搞不给 |
| 读懂丹方残卷（学习新方）| +1 | 轻奖励 |

### §7.3 forging

| 动作 | XP | 备注 |
|---|---|---|
| 坯料成 | +1 | 单步奖励 |
| 淬炼 perfect | +4 | |
| 淬炼 good | +2 | |
| 铭文成 | +3 | |
| 开光成 | +5 | 高阶奖励 |
| 炸砧 | +1 | 失败也学 |
| 废品（waste）| +0 | |

---

## §8 数据契约

### Server

```rust
#[derive(Component, Default, Serialize, Deserialize)]
pub struct SkillSet {
    pub skills: HashMap<SkillId, SkillEntry>,
    pub consumed_scrolls: HashSet<ScrollId>,  // 一生读过的残卷 id，判"此卷已悟"
}

#[derive(Serialize, Deserialize)]
pub struct SkillEntry {
    pub lv: u8,           // real_lv（不含 cap 压制；effective_lv = min(lv, cap)）
    pub xp: u32,          // 当前 lv 内累积
    pub total_xp: u64,    // 终身总 XP（统计用）
    pub last_action_at: Tick,
    pub recent_repeat_count: u8,  // 连续重复同动作计数（§3.1 多样性奖励）
}

#[derive(Copy, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum SkillId { Herbalism, Alchemy, Forging }  // MVP 三种

// LifeRecord 扩展（plan-death-lifecycle §5）
pub struct SkillMilestone {
    pub skill: SkillId,
    pub new_lv: u8,
    pub achieved_at: Tick,
    pub narration: String,       // agent 生成的升级文本（worldview §八 语调）
    pub total_xp_at: u64,        // 达成时的 total_xp 快照
}

// 事件 source 枚举（agent 消费生成 narration + UI 流水显示）
pub enum XpGainSource {
    Action { plan: &'static str, action: &'static str },  // 例 plan="lingtian", action="harvest_auto"
    Scroll { scroll_id: ScrollId, xp_grant: u32 },
    RealmBreakthrough,                                     // 境界突破仅给 CapChanged，不经此
    Mentor { mentor_char: CharId },                        // v2+ 师承
}
```

- [ ] `SkillSet` 挂玩家 entity，BlockEntity 不需要
- [ ] `LifeRecord.skill_milestones: Vec<SkillMilestone>`（每升一级记一笔，亡者博物馆可见）
- [ ] Events：
  - `SkillXpGain { char, skill, amount, source: XpGainSource }`
  - `SkillLvUp { char, skill, new_lv, narration }`
  - `SkillCapChanged { char, skill, new_cap }`（境界突破/跌落触发）
  - `SkillScrollUsed { char, scroll_id, skill, xp_granted, was_duplicate: bool }`
- [ ] Channel：`bong:skill/xp_gain` · `bong:skill/lv_up` · `bong:skill/cap_changed` · `bong:skill/scroll_used`
- [ ] IPC Schema（agent/packages/schema）：`SkillId` enum + `XpGainSource` tagged union · 为 agent 读 NPC skill 画像 + 生成升级 narration 准备

### Client

- [ ] `SkillSetStore`（完整 SkillSet 快照，InspectScreen 技艺 tab 消费）
- [ ] `BotanySkillStore` **deprecated** → 改为从 `SkillSetStore` 派生 `herbalism` 单项视图（不独立同步）
- [ ] 复用：`InventoryStateStore`（skill_scroll 拖拽源）· `DragState`

---

## §9 MVP 阶段划分

| Phase | 内容 | 验收 |
|---|---|---|
| P0 | `SkillSet` component（含 `consumed_scrolls`）+ `SkillId` enum + 曲线函数 + 插值函数 + 单测 | Lv 0-10 XP 累积/升级公式正确，Lv.4 插值 = 7s |
| P1 | Events（4 种）+ Channel + IPC schema（含 `XpGainSource` tagged union）| 各 plan 发 XpGain 事件能到达 Client |
| P2 | SkillSetStore 接入 InspectScreen "技艺" tab（三列 MVP）+ **迁移 `BotanySkillStore` 为派生视图**（不再独立同步） | 看到三 skill 当前 Lv/XP 条；botany/lingtian/harvest-popup 仍显示 Lv 正常 |
| P3 | 境界软挂钩（cap 计算 + effective_lv 压制 + UX 灰显）| 超 cap 效果打折可观察；InspectScreen 显示 `Lv.7 / cap 5` 灰色 |
| P4 | 残卷 item 拖入学习 + 合法性校验 + consumed_scrolls 去重 | skill_scroll 首次 +XP 消耗，重复提示"此卷已悟"不消耗；丹方残卷拖入红框拒绝 |
| P5 | 升级 narration（agent 集成）· `SkillLvUp` channel 消费生成 | Lv up 时 agent 生成冷漠古意文本，记入 LifeRecord.skill_milestones |
| P6 | LifeRecord.skill_milestones + 亡者博物馆展示 + 品质分布映射接入 alchemy/forge/botany | 死透后残留生平可查技艺进程；botany 采药品质确实按 skill Lv 偏移四档 |
| P7 | 废弃 `BotanySkillStore` 代码（完全移除）+ 所有 plan 内 skill 引用走 `SkillSetStore` | 搜索代码无 `BotanySkillStore` 剩余引用 |

---

## §10 跨 plan 钩子

- [ ] **plan-botany-v1 / plan-lingtian-v1**：`BotanySkillStore` 被本 plan 替代（`SkillSetStore` 派生）；XP 数值表全部从本 plan §7.1 抓取
- [ ] **plan-alchemy-v1**：§6.2 效果表代入 `fire_profile.tolerance` 计算；炸炉 +1 XP 写入炉结算逻辑
- [ ] **plan-forge-v1**：§6.3 代入 Tempering 命中窗口；`LearnedBlueprints` 与本 plan `SkillSet` 互补（图谱是配方，skill 是手艺）
- [ ] **plan-inventory-v1**：`skill_scroll` item 独立 1×1 类型（与丹方残卷/图谱残卷并列）
- [ ] **plan-cultivation-v1**：境界突破触发 `SkillCapChanged` 事件；境界跌落联动
- [ ] **plan-death-lifecycle-v1 §4/§5**：重生**不扣 skill**；死透**清零**（与 worldview §十二 一致）
- [ ] **plan-HUD-v1**：场景浮窗顶栏展示当前相关 skill Lv（已在 harvest-popup 示范）
- [ ] **plan-narrative-v1**：`SkillLvUp` 加入 narration 触发表
- [ ] **plan-library-web-content-v1**：亡者博物馆展示 skill_milestones

---

## §11 TODO / 开放问题（v2+）

- [ ] **战斗武学**（剑术 / 刀法 / 拳法 / 掌法）—— 粒度难定，需要先做 combat §5 才好切；可能一招一 skill 或一流派一 skill
- [ ] **师承系统**（plan-social §2）—— 师父消耗 qi 传功，徒弟 +N XP；师父死徒弟残留"师传印记"tag
- [ ] **技艺专精**：Lv.10 后的特化选择（类似顿悟），例如 herbalism Lv.10 分支"丹圃派"or"野行派"
- [ ] **NPC skill 画像**：agent 如何读/用（散修 NPC 的炼丹水平影响其交易品质/态度）
- [ ] **成就式里程碑**：Lv.5 / Lv.10 达成时可选特殊 narration 或 title
- [ ] **跨 skill 联动**：alchemy Lv.7+ 解锁"药师之眼"，自动识别 botany 采到的稀有
- [ ] **抄写残卷**：自己写 skill_scroll 传给他人（v2+）

---

## §12 风险与对策

| 风险 | 对策 |
|---|---|
| skill 数值膨胀 → power creep | 硬 cap 10 + 效果线性非指数（§6 表均线性）|
| "自动采集 = 懒人奖励"印象 | 门槛 Lv.3 + 自动期间受击仍断 + XP 是结果而非动机（botany §1.3 已论证）|
| 技能多导致 UI 膨胀 | MVP 仅 3 项；InspectScreen tab 内按字母/类别排序，未解锁灰显 |
| 多 plan XP 表分散不一致 | §7 汇总表作为 single source of truth；各 plan 内部**引用**此处数值不自定义 |
| 跨玩家攀比 | skill Lv 匿名（默认不显示给他人）；仅死透后生平卷公开——"身前不炫耀，身后供人学" |
| 宏磨（脚本刷 XP）| §3.1 多样性奖励（重复扣 XP）+ session 最低时长（采集 2s 已够）|
| 境界跌落玩家懵 | UX：InspectScreen 灰出超 cap 部分 + narration 解释 + 跌境事件弹窗告知 skill 受限 |
