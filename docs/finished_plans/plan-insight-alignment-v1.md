# Bong · plan-insight-alignment-v1 · 顿悟三轨对齐

顿悟选项不再是静态池随机抽取，而是**按玩家当前真元向量动态生成三轨选项**：靠近（专精深化）/ 中性（通用增益）/ 远离（转型拓宽）。每次顿悟都变成"加深 vs 安全 vs 拓宽"的人生岔路口——玩家的每一次选择都在真元色谱上留下不可逆的印记。

**世界观锚点**：
- `worldview.md §六.2` 真元染色是"长期修习的物理沉淀"——专精 10h 出主色调，换路径旧色慢慢褪。顿悟是加速这个过程的"质变接口"
- `worldview.md §六.2` 染色规则——1 主色满效、1 主+1 副打折~70%、三种以上杂色全失效。三轨选项直接操控这个光谱
- `worldview.md §六.3` 顿悟是"关键时刻的人生选择"——情境化、不可重选、天道可感知
- `worldview.md §五` 流派由组合涌现（无系统门禁），染色是流派的物理副产物——选"靠近"是加深流派认同，选"远离"是开辟第二流派可能性
- `worldview.md §五` 末土后招原则——你的顿悟历史暴露你的倾向，在 PvP 信息战中是弱点也是伪装素材

**前置依赖**：
- `plan-cultivation-v1` ✅ → 顿悟核心系统（7 类 19 效果 + InsightQuota + Arbiter + Fallback）
- `plan-style-vector-integration-v1` ✅ → PracticeLog + QiColor + evolve_qi_color
- `plan-multi-style-v1` ✅ → 多流派切换 / 双流派修炼时间线
- `plan-skill-v1` ✅ → 招式系统 / 流派关联
- `plan-craft-v1` ✅ → 配方顿悟解锁渠道

**反向被依赖**：
- `plan-gameplay-journey-v1` ⬜ skeleton → 100h 路径中"流派分化点"引用本 plan 的三轨设计
- `plan-combat-gamefeel-v1` ⬜ skeleton → PvP 中"后招原则"信息差——你的顿悟选择暴露了你的倾向

---

## 接入面 Checklist

- **进料**：`cultivation::QiColor`（main/secondary/is_chaotic/is_hunyuan）/ `cultivation::PracticeLog`（weights: HashMap\<ColorKind, f64>）/ `cultivation::InsightQuota`（剩余额度 per realm）/ `cultivation::insight::InsightCategory + InsightEffect`（7 类 19 变体白名单）/ `cultivation::Realm`（当前境界）/ `cultivation::life_record::LifeRecord`（insights_taken 历史）
- **出料**：修改 `insight_fallback.rs` 生成逻辑（static → dynamic 三轨）/ 修改 Agent `insight.md` prompt / 新增 `InsightAlignment` enum（server + schema + client 三端）/ client `InsightOfferScreen` 三轨视觉差异 / 新增 `insight::color_affinity` 色系→效果映射模块
- **共享类型 / event**：复用 `InsightEffect` 全部 19 变体（**不新建**）/ 复用 `InsightOffer`（**不新建**）/ `InsightChoiceV1` 新增 `alignment` 字段（`"converge" | "neutral" | "diverge"`）/ 新增 `ColorAffinityMap`（纯数据表，不是 component）
- **跨仓库契约**：
  - server：`InsightChoiceV1.alignment: String`（新增字段）+ `server/src/cultivation/color_affinity.rs`（色系→效果映射）+ `insight_fallback.rs` 重构
  - agent：`agent/packages/schema/src/insight-offer.ts` 加 `alignment` 字段 + `agent/packages/tiandao/src/skills/insight.md` prompt 改造
  - client：`InsightOfferScreen` 按 alignment 渲染卡片底色 + 图标
- **worldview 锚点**：§六.2 染色规则 + §六.3 顿悟 + §五 流派涌现 + §五 末土后招原则
- **qi_physics 锚点**：不涉及（顿悟不操作真元物理层，不触及守恒律/衰减/逸散）

---

## §0 设计轴心

- [ ] **每次顿悟 = 色谱上的一个抉择**：三个选项不再是"A/B/C 随机 buff"，而是"加深你是谁 / 保持安全 / 尝试成为别人"的三条路
- [ ] **靠近 = 越走越深**：CONVERGE 效果匹配当前 main color 关联流派，magnitude ×1.2（比 NEUTRAL 高 20%），但累计上限消耗更快（在有限额度内更早触顶）
- [ ] **远离 = 代价换空间**：DIVERGE 效果推向 PracticeLog 中权重最低的色系方向，magnitude ×0.9（比 NEUTRAL 低 10%），但打开新流派通道——选了 DIVERGE 的效果会在 PracticeLog 中注入 +2.0 权重到目标色（相当于 2 次实践量，加速染色偏移）
- [ ] **中性 = 安全牌**：NEUTRAL 效果不绑定任何 ColorKind，走通用类（QiRegenFactor / ComposureRecover / BreakthroughBonus 等），magnitude ×1.0 标准
- [ ] **混元修士特殊规则**：`is_hunyuan = true` 时，三轨变为"维持混元（HunyuanThreshold 加成）/ 通用 / 打破混元走专精（ColorCapAdd 目标色 + 该色 PracticeLog +5.0）"
- [ ] **杂色修士特殊规则**：`is_chaotic = true` 时，三轨变为"走向混元（ChaoticTolerance + 均匀补齐最低色 PracticeLog）/ 通用 / 回归主色（ColorCapAdd 当前权重最高色 + 抑制次高色 PracticeLog ×0.8）"
- [ ] **选项文案感知向量**：flavor_text 不再是通用文案，而是根据当前 main color 生成——"你的真元已染锋锐之意…… 是更磨利刃，还是另寻他路？"
- [ ] **万物皆有代价（强约束）**：**每个顿悟选项必须同时携带一项增益（gain）和一项代价（cost）**。纯增益的选项不存在——这是末法残土"万物皆有成本"的直接表达。玩家不是在选"哪个 buff 最大"，而是在选"我愿意付什么代价换什么能力"。代价必须是**同量级的、可感知的、影响日常的**，不能是无关痛痒的 -0.1%
- [ ] **代价的三轨分化**：
  - CONVERGE 代价 = **缩窄**：增益强（×1.2）但代价也绑定色系——你在靠近的方向更强，但远离的方向变弱（对立色效率 -15%）
  - NEUTRAL 代价 = **均摊**：增益和代价分布在不相关的轴上（如：真元回复 +5% → 但过载容忍 -3%）
  - DIVERGE 代价 = **割舍**：增益弱（×0.9）且代价是当前主色能力衰退（主色对应效果 -10%）——你在离开旧路的过程中，旧路的"肌肉记忆"在褪去

---

## §1 通用天赋与特殊天赋分层

天赋分两类：**通用天赋**走 JSON 数据驱动，运行时加载、热可扩展；**特殊天赋**留 Rust 硬编码，需要专属逻辑。

### 分类标准

| 类型 | 特征 | 存储 | 扩展方式 |
|------|------|------|---------|
| **通用天赋** | 参数统一：`stat + op(mul/add) + value`，增益/代价都是数值修正 | `server/assets/insight/generic_talents.json` | 改 JSON，不改代码 |
| **特殊天赋** | 参数不统一或需专属逻辑（解锁能力/触发事件/布尔开关/多参数组合） | `InsightEffect` enum 硬编码 | 改 Rust 代码 |

### 通用天赋的 stat 白名单

**增益 stat（gain）**：

| stat ID | 中文 | op | 范围 | 对应 InsightModifiers 字段 |
|---------|------|-----|------|--------------------------|
| `qi_regen_factor` | 真元回复倍率 | mul | 1.01-1.10 | `qi_regen_mul` |
| `composure_recover` | 心境恢复倍率 | mul | 1.01-1.15 | `composure_recover_mul` |
| `breakthrough_bonus` | 突破成功率 | add | 0.01-0.05 | `next_breakthrough_bonus` |
| `meridian_flow_rate` | 经脉流速倍率 | mul | 1.01-1.08 | `MeridianSystem.flow_rate` (需 `meridian_group`) |
| `overload_tolerance` | 过载容忍 | add | 0.01-0.05 | `MeridianSystem.overload_threshold` (需 `meridian_group`) |
| `color_cap` | 染色上限 | add | 0.01-0.05 | `QiColor` 对应色槽 (需 `color`) |
| `chaotic_tolerance` | 杂色容忍 | add | 0.01-0.05 | `chaotic_tolerance_add` |
| `hunyuan_threshold` | 混元阈值 | mul | 0.95-0.99 | `hunyuan_threshold_mul` |

**代价 stat（cost）**：

| stat ID | 中文 | op | 范围 |
|---------|------|-----|------|
| `qi_volatility` | 真元挥发加速 | add | 0.01-0.05 |
| `shock_sensitivity` | 心境冲击敏感 | add | 0.01-0.05 |
| `opposite_color_penalty` | 对立色效率惩罚 | add | 0.05-0.20 |
| `main_color_penalty` | 主色效率惩罚 | add | 0.05-0.15 |
| `overload_fragility` | 过载脆性 | add | 0.01-0.05 |
| `meridian_heal_slowdown` | 经脉修复减速 | mul | 0.85-0.95 |
| `breakthrough_failure_penalty` | 突破失败代价 | mul | 1.05-1.20 |
| `sense_exposure` | 被感知暴露度 | add | 0.01-0.05 |
| `reaction_window_shrink` | 反应窗口缩短 | mul | 0.90-0.97 |
| `chaotic_tolerance_loss` | 杂色容忍下降 | sub | 0.01-0.03 |

### 通用天赋 JSON 模板

文件位置：`server/assets/insight/generic_talents.json`

```json
{
  "version": 1,
  "talents": [
    {
      "id": "qi_regen_harmony",
      "category": "qi",
      "color_affinity": ["mellow", "gentle"],
      "alignment": "any",
      "gain": {
        "stat": "qi_regen_factor",
        "op": "mul",
        "base_value": 1.03
      },
      "cost": {
        "stat": "qi_volatility",
        "op": "add",
        "base_value": 0.015
      },
      "gain_flavor": "{color_name}之息调和——真元回复 +{gain_pct}%",
      "cost_flavor": "灵气更活——战斗中真元挥发 +{cost_pct}%"
    },
    {
      "id": "meridian_surge_arm_yin",
      "category": "meridian",
      "color_affinity": ["sharp", "light"],
      "alignment": "converge",
      "gain": {
        "stat": "meridian_flow_rate",
        "op": "mul",
        "base_value": 1.05,
        "meridian_group": "arm_yin"
      },
      "cost": {
        "stat": "overload_fragility",
        "op": "add",
        "base_value": 0.025
      },
      "gain_flavor": "{color_name}之意贯穿经脉——手三阴流速 +{gain_pct}%",
      "cost_flavor": "脉壁变薄——过载耐受 -{cost_pct}%"
    },
    {
      "id": "color_deepen_self",
      "category": "coloring",
      "color_affinity": ["*"],
      "alignment": "converge",
      "gain": {
        "stat": "color_cap",
        "op": "add",
        "base_value": 0.03,
        "color": "$main"
      },
      "cost": {
        "stat": "chaotic_tolerance_loss",
        "op": "sub",
        "base_value": 0.02
      },
      "gain_flavor": "{color_name}之色更纯——染色深度 +{gain_pct}%",
      "cost_flavor": "越纯越排异——杂色容忍 -{cost_pct}%"
    },
    {
      "id": "composure_calm",
      "category": "composure",
      "color_affinity": ["*"],
      "alignment": "neutral",
      "gain": {
        "stat": "composure_recover",
        "op": "mul",
        "base_value": 1.06
      },
      "cost": {
        "stat": "shock_sensitivity",
        "op": "add",
        "base_value": 0.03
      },
      "gain_flavor": "心如止水——心境恢复 +{gain_pct}%",
      "cost_flavor": "水面更易起波——心境冲击敏感 +{cost_pct}%"
    },
    {
      "id": "overload_iron_will",
      "category": "meridian",
      "color_affinity": ["heavy", "violent"],
      "alignment": "converge",
      "gain": {
        "stat": "overload_tolerance",
        "op": "add",
        "base_value": 0.03
      },
      "cost": {
        "stat": "meridian_heal_slowdown",
        "op": "mul",
        "base_value": 0.90
      },
      "gain_flavor": "经脉如铁——过载容忍 +{gain_pct}%",
      "cost_flavor": "铁难弯更难修——经脉修复速度 -{cost_pct}%"
    },
    {
      "id": "breakthrough_gamble",
      "category": "breakthrough",
      "color_affinity": ["*"],
      "alignment": "neutral",
      "gain": {
        "stat": "breakthrough_bonus",
        "op": "add",
        "base_value": 0.03
      },
      "cost": {
        "stat": "breakthrough_failure_penalty",
        "op": "mul",
        "base_value": 1.10
      },
      "gain_flavor": "冲关之诀——突破成功率 +{gain_pct}%",
      "cost_flavor": "愿赌服输——突破失败惩罚 +{cost_pct}%"
    }
  ]
}
```

### JSON 字段说明

| 字段 | 类型 | 说明 |
|------|------|------|
| `id` | string | 全局唯一 ID，不可重复 |
| `category` | string | 7 类之一（`meridian/qi/composure/coloring/breakthrough/style/perception`） |
| `color_affinity` | string[] | 亲和色系（`["sharp","light"]`）；`["*"]` = 全色通用 |
| `alignment` | string | `"converge"` / `"neutral"` / `"diverge"` / `"any"`（any = 三轨均可选） |
| `gain.stat` | string | 增益 stat ID（见上表白名单） |
| `gain.op` | string | `"mul"` / `"add"` / `"sub"` |
| `gain.base_value` | number | 基础值（会被 alignment 系数调整：CONVERGE ×1.2 / NEUTRAL ×1.0 / DIVERGE ×0.9） |
| `gain.meridian_group` | string? | 可选，`"arm_yin"` / `"arm_yang"` / `"leg_yin"` / `"leg_yang"` / `"ren_du"` |
| `gain.color` | string? | 可选，`"$main"` = 当前主色 / `"$diverge"` = 目标色 / 具体色名 |
| `cost.stat` | string | 代价 stat ID（见上表白名单） |
| `cost.base_value` | number | 代价基础值（≥ gain.base_value × 0.5） |
| `gain_flavor` | string | 增益文案模板（支持 `{color_name}` `{gain_pct}` `{target_color_name}`） |
| `cost_flavor` | string | 代价文案模板（支持 `{cost_pct}`） |

### 特殊天赋（保留 Rust 硬编码）

以下效果参数不统一或需要专属逻辑，**不进 JSON**：

| InsightEffect 变体 | 原因 |
|-------------------|------|
| `UnlockPractice { name }` | 解锁命名能力，需查 TECHNIQUE_DEFINITIONS |
| `UnlockPerception { kind }` | 解锁感知类型，需写 UnlockedPerceptions 集合 |
| `LifespanExtensionEnlightenment` | 一次性延寿，需跨模块调 lifespan 合同 |
| `ComposureImmuneDuringBreakthrough` | 布尔开关，无数值参数 |
| `TribulationPredictionWindow` | 布尔开关 |
| `BreakthroughEventConditionDrop { realm }` | 需要 Realm enum 参数 |
| `ZhenfaConcealment { add }` | 阵法子系统专属 |
| `ZhenfaDisenchant { add }` | 阵法子系统专属 |
| `DualForgeDiscount { id, mul }` | 锻造子系统专属 |
| `ColorMaterialAffinity { color, material, add }` | 三参数组合 |
| `PurgeEfficiency { color, mul }` | 排毒需指定 color |
| `UnfreezeQiMax { mul }` | 专属机制 |

特殊天赋仍在 `insight_fallback.rs` 中硬编码，但**同样必须遵守代价铁律**——每个特殊天赋也要手写对应的 `InsightCost`。

### 色系→天赋亲和映射

三轨选取时，按 `color_affinity` 字段过滤：
- **CONVERGE**：取 `color_affinity` 包含当前 `QiColor.main` 或 `"*"` 的天赋，且 `alignment ∈ {"converge", "any"}`
- **NEUTRAL**：取 `alignment ∈ {"neutral", "any"}` 的天赋
- **DIVERGE**：取 `color_affinity` 包含 `diverge_target` 色或 `"*"` 的天赋，且 `alignment ∈ {"diverge", "any"}`

**设计约束**：
- CONVERGE 从当前 `QiColor.main` 亲和池选取，magnitude = `base_value × 1.2`
- DIVERGE 从 `diverge_target` 亲和池选取，magnitude = `base_value × 0.9`
- NEUTRAL 从中性池选取，magnitude = `base_value × 1.0`
- 全部效果仍受 Arbiter 白名单 + 单次 cap + 累计 cap 校验（不绕过）
- 若某轨无可选天赋（全部触顶或池为空），降级为 NEUTRAL + 提示"此道已臻顶"
- 通用天赋 + 特殊天赋在三轨选取时混合候选——特殊天赋优先级不高于通用天赋

---

## §2 代价对轴表（万物皆有成本）

**铁律**：每个顿悟选项**必须**同时携带 `gain`（增益）和 `cost`（代价）。代价必须是可感知的、影响日常的、与增益同量级的。不存在纯增益选项。

### 代价结构

```rust
pub struct InsightTradeoff {
    pub gain: InsightEffect,       // 你得到什么
    pub gain_magnitude: f64,       // 增益幅度
    pub cost: InsightCost,         // 你失去什么
    pub cost_magnitude: f64,       // 代价幅度
}
```

### 三轨代价分化

| 轨道 | 增益特征 | 代价特征 | 设计意图 |
|------|---------|---------|---------|
| **CONVERGE** | 强（×1.2），绑当前色系 | **对立色效率衰退**：对立色系的关联效果永久 -15% | 越专精越强，但越偏科——剑修顿悟后用爆脉拳更加别扭 |
| **NEUTRAL** | 标准（×1.0），通用 | **正交轴削弱**：增益和代价在不相关的轴上对冲 | 安全但并非无痛——真元回复快了但过载变脆了 |
| **DIVERGE** | 弱（×0.9），绑目标色系 | **当前主色效率衰退**：主色关联效果永久 -10% | 转型有代价——旧路的"肌肉记忆"在褪色 |

### 代价对轴（10 对）

每对轴是**此消彼长**的关系——gain 在左边，cost 在右边（反之亦可）：

| # | 轴 A（正向） | 轴 B（反向） | 叙事逻辑 |
|---|------------|------------|---------|
| 1 | 经脉流速 +（MeridianRate） | 经脉过载容忍 -（OverloadTolerance） | 流速快的脉更脆——水管压力大了管壁薄了 |
| 2 | 真元回复 +（QiRegenFactor） | 真元挥发 +（QiVolatility） | 回复快 = 体质对灵气更敏感 = 战斗中真元也散得更快 |
| 3 | 心境恢复 +（ComposureRecover） | 心境冲击敏感 +（ShockSensitivity） | 恢复快 = 情绪波动大 = 受冲击时反应也更剧烈 |
| 4 | 染色加深 +（ColorCapAdd） | 杂色容忍 -（ChaoticTolerance） | 色越纯越排斥异色——主色强了但双修更难 |
| 5 | 突破成功 +（BreakthroughBonus） | 突破失败代价 +（FailurePenalty） | 愿赌服输——天道给你更大的窗口，但关窗时砸得更狠 |
| 6 | 感知范围 +（UnlockPerception） | 被感知暴露 +（SenseExposure） | 看得远 = 灵识外放 = 别人也更容易定位你 |
| 7 | 流派效率 +（当前色系招式） | 跨流派效率 -（非当前色系招式） | 深化专精 = 偏科——一条路走到黑，其他路走不动 |
| 8 | 排毒效率 +（PurgeEfficiency） | 自疗速度 -（HealRate） | 排异快 = 免疫系统过激 = 自身修复也被"排异"了 |
| 9 | 阵法持久 +（ZhenfaConceal/Disenchant） | 即时反应 -（ReactionWindow） | 算计深 = 思虑重 = 危机时靠肌肉记忆反应慢了 |
| 10 | 过载容忍 +（OverloadTolerance） | 经脉修复速度 -（MeridianHealRate） | 扛得住 ≠ 好得快——硬抗过后经脉纤维化更难愈合 |

### CONVERGE 的"对立色"定义

每种 ColorKind 有一个**对立色**（代价方向）：

| 色系 | 对立色 | 对立逻辑 |
|------|-------|---------|
| Sharp（锋锐/剑） | Heavy（沉重/体修） | 锋利 vs 钝重 |
| Heavy（沉重/体修） | Light（飘逸/御物） | 重 vs 轻 |
| Mellow（温润/丹） | Violent（暴烈/雷） | 温和 vs 激烈 |
| Solid（凝实/器修） | Light（飘逸/御物） | 凝实 vs 飘逸 |
| Light（飘逸/御物） | Heavy（沉重/体修） | 轻盈 vs 厚重 |
| Intricate（缜密/阵法） | Violent（暴烈/雷） | 精密 vs 暴力 |
| Gentle（柔和/医道） | Insidious（阴诡/毒蛊） | 温柔 vs 阴毒 |
| Insidious（阴诡/毒蛊） | Gentle（柔和/医道） | 阴毒 vs 温柔 |
| Violent（暴烈/雷法） | Intricate（缜密/阵法） | 暴力 vs 精密 |
| Turbid（浊乱/魔功） | Mellow（温润/丹） | 混沌 vs 醇正 |

### 代价的具体实现

代价通过 `InsightModifiers` 新增负向字段存储：

```rust
pub struct InsightModifiers {
    // ... 现有正向字段 ...
    // 新增代价字段
    pub opposite_color_efficiency_penalty: f64,  // 对立色效率惩罚（CONVERGE 代价）
    pub qi_volatility_add: f64,                  // 真元挥发加速（NEUTRAL 代价之一）
    pub shock_sensitivity_add: f64,              // 心境冲击敏感度（NEUTRAL 代价之一）
    pub main_color_efficiency_penalty: f64,      // 主色效率惩罚（DIVERGE 代价）
    pub reaction_window_penalty: f64,            // 即时反应窗口缩短（阵法代价）
    pub breakthrough_failure_penalty_mul: f64,   // 突破失败惩罚倍率
    pub sense_exposure_add: f64,                 // 被感知暴露度
}
```

### 代价可感知性约束

- 代价幅度 ≥ 增益幅度 × 0.5（不允许"增益 +10% 代价 -0.5%"这种象征性代价）
- 代价必须在日常 gameplay loop 中被碰到（不能是"只有化虚渡劫时才触发"的极端场景代价）
- UI 中代价用红色小字显示在增益下方，不隐藏

### 代价的 flavor_text 示例

| 选项 | 增益描述 | 代价描述 |
|------|---------|---------|
| CONVERGE（Sharp 主色） | "锋锐之意贯穿经脉，手三阴流速 +6%" | "厚重之道渐远——沉重色招式效率 -15%" |
| NEUTRAL | "真元回复加速 +5%" | "灵气更活——战斗中真元挥发加速 +3%" |
| DIVERGE（目标 Intricate） | "缜密之理初窥——阵法效率 +4.5%" | "锋锐之忆淡去——锋锐色招式效率 -10%" |

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|----|------|----|
| P0 | `InsightAlignment` + `InsightTradeoff` + `InsightCost` + `GenericTalentRegistry`（JSON 加载）+ 对立色表 + 三轨选取 | ⬜ |
| P1 | 三轨 Fallback 生成器（重构 `insight_fallback.rs`）+ 代价配对逻辑 + DIVERGE PracticeLog 注入 | ⬜ |
| P2 | Agent prompt 改造 + InsightChoiceV1 schema 加 `alignment` + `cost_desc` 字段 + agent Arbiter 适配 | ⬜ |
| P3 | Client UI 三轨视觉（卡片底色 / 对齐图标 / 增益绿字+代价红字 / flavor 文案） | ⬜ |
| P4 | 饱和化测试（10 色 × 16 trigger × 3 alignment × gain/cost 配对校验 + 代价幅度 ≥ gain×0.5） | ⬜ |

---

## P0 — 核心数据结构 + 三轨选取逻辑 ⬜

### 交付物

1. **`InsightAlignment` enum + `InsightCost` enum + `InsightTradeoff` struct**（`server/src/cultivation/insight.rs`）

   ```rust
   #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
   pub enum InsightAlignment {
       Converge,  // 靠近当前真元向量——专精深化
       Neutral,   // 中性——通用增益
       Diverge,   // 远离当前真元向量——转型拓宽
   }

   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub enum InsightCost {
       OppositeColorPenalty { color: ColorKind, penalty: f64 },  // 对立色效率 -N%
       QiVolatility { add: f64 },                                // 真元挥发加速
       ShockSensitivity { add: f64 },                            // 心境冲击敏感
       MainColorPenalty { color: ColorKind, penalty: f64 },      // 主色效率 -N%（DIVERGE）
       OverloadFragility { add: f64 },                           // 过载耐受降低
       MeridianHealSlowdown { mul: f64 },                        // 经脉修复减速
       BreakthroughFailurePenalty { mul: f64 },                   // 突破失败加重
       SenseExposure { add: f64 },                               // 被感知暴露度
       ReactionWindowShrink { mul: f64 },                         // 即时反应窗口缩短
       ChaoticToleranceLoss { sub: f64 },                         // 杂色容忍度下降
   }

   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct InsightTradeoff {
       pub alignment: InsightAlignment,
       pub gain: InsightEffect,
       pub gain_magnitude: f64,
       pub cost: InsightCost,
       pub cost_magnitude: f64,
       pub gain_flavor: String,
       pub cost_flavor: String,
   }
   ```

   在 `InsightChoice` 中新增 `alignment: InsightAlignment` + `cost: InsightCost` + `cost_flavor: String` 字段。

2. **`GenericTalentRegistry`**（`server/src/cultivation/generic_talent.rs`，新文件）

   启动时加载 `server/assets/insight/generic_talents.json`，解析为 `Vec<GenericTalentDef>`：

   ```rust
   #[derive(Debug, Clone, Deserialize)]
   pub struct GenericTalentDef {
       pub id: String,
       pub category: InsightCategory,
       pub color_affinity: Vec<String>,       // ["sharp","light"] 或 ["*"]
       pub alignment: String,                  // "converge"/"neutral"/"diverge"/"any"
       pub gain: StatModifier,
       pub cost: StatModifier,
       pub gain_flavor: String,                // 模板，含 {color_name} {gain_pct}
       pub cost_flavor: String,                // 模板，含 {cost_pct}
   }

   #[derive(Debug, Clone, Deserialize)]
   pub struct StatModifier {
       pub stat: String,                       // stat ID（见 §1 白名单）
       pub op: String,                         // "mul"/"add"/"sub"
       pub base_value: f64,
       pub meridian_group: Option<String>,      // 经脉组（可选）
       pub color: Option<String>,               // "$main"/"$diverge"/具体色名（可选）
   }
   ```

   - `GenericTalentRegistry::load(path) -> Result<Self>`：启动时一次性加载，校验 stat ID ∈ 白名单 + base_value ∈ 允许范围
   - `query(color_affinity, alignment) -> Vec<&GenericTalentDef>`：按亲和色 + 轨道过滤
   - `resolve_flavor(template, color_name, pct) -> String`：填充文案模板
   - `to_insight_tradeoff(def, alignment_coeff, main_color, diverge_color) -> InsightTradeoff`：从 JSON 定义转换为运行时 tradeoff

3. **特殊天赋池**（`server/src/cultivation/special_talent.rs`，新文件）

   保留 Rust 硬编码的特殊天赋（UnlockPractice / UnlockPerception / LifespanExtension 等），同样返回 `InsightTradeoff`（手写 cost）。

   ```rust
   pub fn special_converge_pool(color: ColorKind) -> Vec<InsightTradeoff> { ... }
   pub fn special_neutral_pool() -> Vec<InsightTradeoff> { ... }
   ```

4. **`diverge_target` 函数**

   返回 PracticeLog 中权重最低且不等于 current_main 的 ColorKind。
   若所有色系权重均为 0（醒灵新人），随机选一种非 main 色。

5. **`select_aligned_tradeoffs` 函数**（`server/src/cultivation/color_affinity.rs`）

   输入：`trigger_id`, `QiColor`, `PracticeLog`, `InsightQuota`, `Realm`, `&GenericTalentRegistry`
   输出：`[InsightTradeoff; 3]`（CONVERGE / NEUTRAL / DIVERGE 各一）

   逻辑：
   - **构建候选池**：合并通用天赋（`registry.query()`）+ 特殊天赋（`special_*_pool()`），按 alignment 分三组
   - **CONVERGE**：从候选池的 converge 组中选一个未触顶的天赋；通用天赋的 magnitude = `base_value × 1.2`；通用天赋的 cost 已在 JSON 中定义；特殊天赋的 cost 在 Rust 中硬编码
   - **NEUTRAL**：从候选池的 neutral 组中选一个未触顶的天赋；magnitude = `base_value × 1.0`
   - **DIVERGE**：从 diverge_target 色的候选池中选一个未触顶的天赋；magnitude = `base_value × 0.9`
   - 若某轨无可选天赋（全部触顶或池为空），退化为 NEUTRAL
   - 三个 gain 效果不重复
   - **代价校验**：assert `cost_magnitude >= gain_magnitude * 0.5`（铁律）

6. **`opposite_color` 函数**

   返回 §2 对立色表中的对立 ColorKind。10 色全覆盖。

4. **混元 / 杂色特殊路径**（`select_aligned_choices` 内分支）

   - `is_hunyuan`：CONVERGE = `HunyuanThreshold { mul: 0.97 * 1.2 }`（维持混元更容易）/ NEUTRAL = 通用 / DIVERGE = `ColorCapAdd { color: strongest_single, add: 0.05 }` + PracticeLog 注入该色 +5.0（打破混元走专精）
   - `is_chaotic`：CONVERGE = `ChaoticTolerance { add: 0.03 * 1.2 }` + 最低色 PracticeLog +2.0（走向混元）/ NEUTRAL = 通用 / DIVERGE = `ColorCapAdd { color: highest_weight, add: 0.04 }` + 次高色 PracticeLog ×0.8（回归主色）

### 验收抓手

- 测试：`server::cultivation::generic_talent::tests::load_json_valid`（加载 generic_talents.json 成功，stat ID 全在白名单内）
- 测试：`server::cultivation::generic_talent::tests::load_json_rejects_unknown_stat`（未知 stat ID 报错）
- 测试：`server::cultivation::generic_talent::tests::load_json_rejects_out_of_range`（base_value 超范围报错）
- 测试：`server::cultivation::generic_talent::tests::query_by_color_and_alignment`（按 color_affinity + alignment 过滤正确）
- 测试：`server::cultivation::generic_talent::tests::wildcard_affinity_matches_all`（`["*"]` 匹配全部 10 色）
- 测试：`server::cultivation::generic_talent::tests::resolve_flavor_template`（模板变量填充正确）
- 测试：`server::cultivation::color_affinity::tests::converge_pool_all_10_colors`（通用 + 特殊合并后每种色至少 3 个亲和效果）
- 测试：`server::cultivation::color_affinity::tests::diverge_target_picks_lowest_weight`
- 测试：`server::cultivation::color_affinity::tests::diverge_target_new_player_random`（醒灵新人不 panic）
- 测试：`server::cultivation::color_affinity::tests::select_aligned_no_duplicate_effects`
- 测试：`server::cultivation::color_affinity::tests::converge_magnitude_1_2x`
- 测试：`server::cultivation::color_affinity::tests::diverge_magnitude_0_9x`
- 测试：`server::cultivation::color_affinity::tests::hunyuan_special_converge_maintains`
- 测试：`server::cultivation::color_affinity::tests::chaotic_special_converge_to_hunyuan`
- 测试：`server::cultivation::color_affinity::tests::cap_exhausted_falls_back_to_neutral`
- 测试：`server::cultivation::color_affinity::tests::every_choice_has_nonzero_cost`（**铁律：无纯增益**）
- 测试：`server::cultivation::color_affinity::tests::cost_magnitude_gte_half_gain`（代价 ≥ 增益 ×0.5）
- 测试：`server::cultivation::color_affinity::tests::converge_cost_targets_opposite_color`（CONVERGE 代价命中对立色）
- 测试：`server::cultivation::color_affinity::tests::diverge_cost_targets_main_color`（DIVERGE 代价命中当前主色）
- 测试：`server::cultivation::color_affinity::tests::opposite_color_map_symmetric`（对立色表存在且 10 色全覆盖）

---

## P1 — 三轨 Fallback 生成器 ⬜

### 交付物

1. **重构 `insight_fallback.rs`**

   现有 `fallback_for_trigger()` 返回 `Vec<InsightChoice>`（3 个静态选项）。重构为：
   - 新签名：`fallback_for_trigger(trigger_id, qi_color, practice_log, quota, realm) -> Vec<InsightTradeoff>`
   - 内部调 `select_aligned_tradeoffs()`
   - 每个 tradeoff 携带 `alignment` + `gain` + `cost` + 双段 flavor
   - gain_flavor 模板化：根据 alignment + trigger_id + main color 生成
   - cost_flavor 模板化：明确告诉玩家"你将失去什么"

2. **flavor_text 模板**（`server/src/cultivation/insight_flavor.rs`，新文件）

   每种 alignment × trigger 类型有专属文案模板：

   | alignment | trigger 类别 | 文案模板示例 |
   |-----------|-------------|------------|
   | Converge | first_breakthrough | "你的真元已染{main_color_name}之意。此刻，它渴望更深。{effect_desc}" |
   | Neutral | first_breakthrough | "突破的余韵尚在。天地给了你一个平淡的馈赠。{effect_desc}" |
   | Diverge | first_breakthrough | "你感到体内有一缕不属于{main_color_name}的真元在涌动。{target_color_name}之意……{effect_desc}" |
   | Converge | survived_negative_zone | "负灵域没有杀死你——你的{main_color_name}之气比你以为的更韧。{effect_desc}" |
   | Diverge | killed_higher_realm | "击杀强者的瞬间，你感到对方的真元余韵——{target_color_name}……你从未触碰过的力量。{effect_desc}" |

   - `color_kind_to_chinese()`：Sharp→"锋锐"、Heavy→"沉重"、Mellow→"温润"...（10 种中文名映射）
   - 模板用 `format!` 填充，不走 LLM

3. **DIVERGE 的 PracticeLog 副作用**

   当玩家选择 DIVERGE 类顿悟后，`apply_choice()` 除了应用 InsightEffect 外，额外：
   - `practice_log.add(diverge_target_color, 2.0)`（注入 2 次实践量到目标色）
   - 这个副作用**不**走 `record_style_practice()`（不触发 CultivationSessionPracticeEvent），直接写 PracticeLog
   - 在 LifeRecord 中标记 `BiographyEntry::InsightDiverge { from_color, to_color }`

4. **向后兼容**

   - 已有的 `InsightOffer`（来自 Agent LLM）如果没有 `alignment` 字段，默认 `Neutral`
   - Agent LLM 未激活时（当前状态），全部走新 fallback 三轨逻辑
   - 已持有的 `InsightModifiers` 结构不变——三轨只影响**选择生成**，不影响效果应用

### 验收抓手

- 测试：`server::cultivation::insight_fallback::tests::fallback_produces_three_alignments`（每个 trigger 返回恰好 CONVERGE+NEUTRAL+DIVERGE 各一）
- 测试：`server::cultivation::insight_fallback::tests::every_tradeoff_has_cost`（**所有返回的 tradeoff 都有 cost ≠ None**）
- 测试：`server::cultivation::insight_fallback::tests::converge_cost_is_opposite_color_penalty`
- 测试：`server::cultivation::insight_fallback::tests::diverge_cost_is_main_color_penalty`
- 测试：`server::cultivation::insight_fallback::tests::neutral_cost_matches_pair_axis`（NEUTRAL 代价从代价对轴表配对）
- 测试：`server::cultivation::insight_fallback::tests::diverge_injects_practice_log`（DIVERGE 选择后 PracticeLog 目标色 +2.0）
- 测试：`server::cultivation::insight_fallback::tests::cost_flavor_not_empty`（代价文案非空）
- 测试：`server::cultivation::insight_fallback::tests::gain_flavor_contains_color_name`（增益文案包含当前主色中文名）
- 测试：`server::cultivation::insight_fallback::tests::backward_compat_no_alignment_field`（无 alignment 字段默认 Neutral + 默认代价 QiVolatility）
- 手动：触发 `first_breakthrough_to_Induce` → 弹出 3 选项 → 确认每个选项同时显示绿色增益 + 红色代价

---

## P2 — Agent prompt + Schema 适配 ⬜

### 交付物

1. **`InsightChoiceV1` schema 扩展**（`agent/packages/schema/src/insight-offer.ts`）

   新增字段：
   ```typescript
   export interface InsightChoiceV1 {
     category: InsightCategory;
     effect_kind: string;
     magnitude: number;
     flavor_text: string;         // 增益描述
     narrator_voice?: string;
     alignment?: "converge" | "neutral" | "diverge";  // 新增
     cost_kind?: string;          // 新增：代价效果类型
     cost_magnitude?: number;     // 新增：代价幅度
     cost_flavor?: string;        // 新增：代价描述（红色显示）
   }
   ```

   - Rust 侧 `server/src/schema/cultivation.rs` 同步加 `alignment: Option<String>` + `cost_kind: Option<String>` + `cost_magnitude: Option<f64>` + `cost_flavor: Option<String>`
   - JSON Schema 重新生成（`npm run generate`）

2. **Agent prompt 改造**（`agent/packages/tiandao/src/skills/insight.md`）

   在 insight 系统 prompt 中追加三轨 + 代价指导：
   - "你必须返回恰好 3 个选项，每个选项标注 alignment 字段"
   - "**铁律：每个选项必须同时包含增益（flavor_text + effect_kind + magnitude）和代价（cost_flavor + cost_kind + cost_magnitude）。纯增益选项会被 Arbiter 拒绝**"
   - "第一个选项（converge）：增益加深当前 main_color 流派能力（magnitude ×1.2）；代价 = 对立色效率 -15%"
   - "第二个选项（neutral）：通用增益（magnitude ×1.0）；代价 = 正交轴上的另一个能力削弱"
   - "第三个选项（diverge）：增益推向权重最低色系方向（magnitude ×0.9）；代价 = 当前主色效率 -10%"
   - "cost_flavor 必须写清楚玩家会失去什么——不允许模糊措辞。示例：'厚重之道渐远——沉重色招式效率 -15%'"
   - "如果 is_hunyuan=true：converge 维持混元（代价=专精能力上限降低），diverge 打破混元走专精（代价=混元容忍度永久降低）"
   - "如果 is_chaotic=true：converge 走向混元（代价=主色效率降低），diverge 回归主色（代价=次色被抑制）"

3. **Agent Arbiter 适配**（`agent/packages/tiandao/src/insight-runtime.ts`）

   `applyInsightArbiter()` 新增：
   - 验证 3 个 choice 的 alignment 字段分别为 converge/neutral/diverge（无重复）
   - alignment 缺失 → 按顺序填充 converge/neutral/diverge
   - alignment 有重复 → 去重后从 fallback 补足

### 验收抓手

- 测试：`agent::insight-runtime::tests::arbiter_validates_alignment_uniqueness`
- 测试：`agent::insight-runtime::tests::missing_alignment_defaults_to_sequence`
- Schema 测试：`agent::schema::tests::insight_choice_alignment_field_optional`
- 手动：mock LLM 返回三轨选项 → Arbiter 通过 → server 正确解析 alignment

---

## P3 — Client UI 三轨视觉差异化 ⬜

### 交付物

1. **卡片底色差异**（`InsightOfferScreen.java`）

   三张选项卡片按 alignment 染色：
   - CONVERGE：底色 = 当前 main color 对应色调（见下表），透明度 20%
   - NEUTRAL：底色 = 灰色 `#808080`，透明度 15%
   - DIVERGE：底色 = diverge target color 对应色调，透明度 20%

   色调映射（ColorKind → ARGB hex）：

   | ColorKind | 色调 hex | 视觉 |
   |-----------|---------|------|
   | Sharp | #C0C0E0 | 银蓝 |
   | Heavy | #8B6914 | 古铜 |
   | Mellow | #A0D0A0 | 淡绿 |
   | Solid | #B0A090 | 岩色 |
   | Light | #D0E8FF | 天蓝 |
   | Intricate | #C0B0D0 | 淡紫 |
   | Gentle | #F0D0E0 | 粉白 |
   | Insidious | #408040 | 墨绿 |
   | Violent | #D08040 | 橙红 |
   | Turbid | #606060 | 暗灰 |

2. **对齐图标**（卡片左上角 8×8 小图标）

   - CONVERGE：向内收拢的双箭头 `»«`（专精）
   - NEUTRAL：水平横线 `──`（平衡）
   - DIVERGE：向外发散的双箭头 `«»`（拓宽）
   - 图标用 `DrawContext.drawText` 渲染（不做贴图，ASCII 足够）

3. **增益/代价双行文案**

   每张卡片内容分两段：
   - **增益行**（绿色 `#80FF80`）：`gain_flavor`（如 "锋锐之意贯穿经脉，手三阴流速 +6%"）
   - **代价行**（红色 `#FF6060`）：`cost_flavor`（如 "厚重之道渐远——沉重色招式效率 -15%"）
   - 代价行字号比增益行小 1px，但**不隐藏、不折叠、不需要 hover 才能看到**
   - 增益行前缀 `▲`，代价行前缀 `▼`

4. **文案风格差异**

   - CONVERGE 增益字体颜色 = 当前主色色调（略亮）
   - NEUTRAL 增益字体颜色 = 标准白
   - DIVERGE 增益字体颜色 = 目标色色调（略亮）
   - 所有代价字体颜色统一为暗红 `#FF6060`

5. **tooltip 提示**

   hover 卡片时显示两行 tooltip：
   - 第一行："深化{main_color_name} / 通用增益 / 转向{target_color_name}"
   - 第二行（红色）："代价：{cost 一句话总结}"

### 验收抓手

- 手动：触发顿悟 → 3 张卡片 → 每张同时显示绿色增益行 + 红色代价行 → 代价描述清晰可读 → hover tooltip 正确
- 测试：`client::insight::tests::card_color_matches_alignment`（mock offer → 验证 3 张卡底色）
- 测试：`client::insight::tests::cost_line_rendered_red`（代价行颜色 = #FF6060）
- 测试：`client::insight::tests::cost_line_always_visible`（代价行不折叠、不隐藏）
- 测试：`client::insight::tests::tooltip_contains_cost_summary`

---

## P4 — 饱和化测试 ⬜

### 交付物

1. **三轨效果完备性矩阵**

   自动化测试覆盖：
   - 10 ColorKind × 3 alignment = 30 种选取路径，每条至少返回一个合法 InsightTradeoff（gain + cost 均非空）
   - 16 trigger × 3 alignment = 48 种 fallback 组合，全部返回有效 InsightTradeoff
   - 混元 is_hunyuan=true × 3 轨 = 3 种特殊选取
   - 杂色 is_chaotic=true × 3 轨 = 3 种特殊选取
   - 总计 ≥ 84 个测试 case

2. **magnitude 校验矩阵**

   - CONVERGE gain magnitude = base × 1.2（容差 ±0.001）
   - NEUTRAL gain magnitude = base × 1.0
   - DIVERGE gain magnitude = base × 0.9
   - 全部通过 Arbiter `validate_offer()` 校验（single_cap + cumulative_cap）
   - 特殊情况：magnitude × 1.2 超过 single_cap 时 → clamp 到 single_cap

3. **代价校验矩阵（铁律）**

   - **全覆盖无纯增益**：84 个 case × assert cost != None（任何 cost 为空的 tradeoff = test failure）
   - **代价幅度 pin**：assert `cost_magnitude >= gain_magnitude * 0.5`（30 case × 3 alignment = 90 断言）
   - **CONVERGE 代价命中对立色**：10 ColorKind × assert cost.color == opposite_color(main)
   - **DIVERGE 代价命中当前主色**：10 ColorKind × assert cost.color == main
   - **NEUTRAL 代价从对轴表配对**：6 种 neutral effect × assert cost_kind == paired_axis[gain_kind]
   - **代价效果可执行**：全部 InsightCost 变体在 `apply_tradeoff_cost()` 中有对应分支（无 unreachable!）

4. **PracticeLog 副作用验证**

   - 选 DIVERGE → 目标色 PracticeLog +2.0，其他色不变
   - 选 CONVERGE / NEUTRAL → PracticeLog 不变
   - 混元修士选 DIVERGE → 目标色 +5.0
   - 杂色修士选 CONVERGE → 最低色 +2.0

5. **端到端闭环测试**

   - 创建玩家（Sharp 主色，weights: {Sharp: 50, Heavy: 5}）
   - 触发 `first_breakthrough_to_Induce`
   - 验证 3 个 tradeoff 分别为：
     - CONVERGE: gain=Sharp 亲和效果（magnitude ×1.2），cost=OppositeColorPenalty(Heavy, 0.15)
     - NEUTRAL: gain=通用效果（magnitude ×1.0），cost=对轴配对代价
     - DIVERGE: gain=某低权重色亲和效果（magnitude ×0.9），cost=MainColorPenalty(Sharp, 0.10)
   - 选择 CONVERGE → 验证 InsightModifiers.opposite_color_efficiency_penalty += 0.15
   - 选择 DIVERGE → PracticeLog 目标色 +2.0 + InsightModifiers.main_color_efficiency_penalty += 0.10
   - 后续战斗中 Sharp 招式效率 = base × (1.0 - main_color_penalty)（DIVERGE 后打剑变弱）
   - 后续战斗中 Heavy 招式效率 = base × (1.0 - opposite_color_penalty)（CONVERGE 后打拳变弱）

6. **LifeRecord 审计**

   - 每次顿悟选择记录 alignment + cost 信息
   - `BiographyEntry::InsightTaken` 包含 alignment tag + cost_kind
   - `BiographyEntry::InsightDiverge` 仅 DIVERGE 时写入

### 验收抓手

- `cargo test cultivation::color_affinity` + `cultivation::insight_fallback` 全绿
- `npm test` agent schema + insight-runtime 全绿
- client 测试全绿
- 84+ case 矩阵全绿

---

## Finish Evidence

**归档时间**：2026-05-11

### 落地清单

- P0/P1：`server/src/cultivation/insight.rs` 新增 `InsightAlignment`、`InsightCost`、`InsightTradeoff`，`InsightChoice` 扩展 `alignment` / `cost` / `cost_magnitude` / `cost_flavor` / `target_color`，`validate_offer()` 拒绝纯增益和低代价选项。
- P1：`server/src/cultivation/generic_talent.rs` + `server/assets/insight/generic_talents.json` 落地 JSON 驱动通用天赋池，覆盖 gain/cost stat 白名单、倍率缩放、颜色 token、模板渲染和数据校验。
- P1/P4：`server/src/cultivation/color_affinity.rs`、`special_talent.rs`、`insight_flavor.rs`、`insight_fallback.rs` 重构 fallback 为 CONVERGE / NEUTRAL / DIVERGE 三轨生成，按 `QiColor` + `PracticeLog` 选择靠近、通用和转向选项，并覆盖混元/杂色特殊分支。
- P1/P4：`server/src/cultivation/insight_apply.rs` 新增代价累计字段，应用 `OppositeColorPenalty`、`MainColorPenalty`、`QiVolatility`、`ShockSensitivity`、`MeridianHealSlowdown` 等 cost；DIVERGE 写入目标色 PracticeLog +2.0，混元打破专精 +5.0。
- P1/P4：`server/src/cultivation/life_record.rs`、`lifespan.rs`、`possession.rs`、`persistence/mod.rs`、`schema/cultivation.rs`、`world/poi_novice.rs` 对齐新 `InsightTaken` 兼容字段与 `InsightDiverge` 审计事件。
- P2：`agent/packages/schema/src/insight-offer.ts` 与 generated schema 增加 `alignment`、`cost_kind`、`cost_magnitude`、`cost_flavor`；`agent/packages/tiandao/src/insight-runtime.ts` Arbiter 校验三轨唯一性、自动补齐缺失 alignment，并拒绝缺代价/低代价选项。
- P2：`agent/packages/tiandao/src/skills/insight.md` 明确要求 LLM 返回三轨选项和可感知代价，纯增益输出会被 Arbiter 拒绝。
- P3：`client/src/main/java/com/bong/client/insight/InsightAlignment.java`、`InsightChoice.java`、`InsightOfferScreen.java`、`MockInsightOfferData.java`、`HeartDemonOfferHandler.java` 展示三轨图标/底色、绿色增益行、红色代价行和 tooltip；旧构造器默认 NEUTRAL + 保底代价文案。
- P4：server 覆盖 fallback 三轨、全 trigger 有三选项、代价非空、代价幅度不少于增益 50%、混元/杂色分支、DIVERGE PracticeLog 注入、LifeRecord 审计；agent 覆盖 alignment 去重/补齐和 cost 校验；client 覆盖 describe 输出与代价常显。

### 关键 commit

- `01f7765e8` · 2026-05-11 · `plan-insight-alignment-v1: 落地顿悟三轨代价闭环`
- `22e050f23` · 2026-05-11 · `plan-insight-alignment-v1: 对齐 agent 顿悟契约`
- `b0ddfc4ed` · 2026-05-11 · `plan-insight-alignment-v1: 展示顿悟三轨代价`

### 测试结果

- `cd server && CARGO_BUILD_JOBS=1 cargo fmt --check && CARGO_BUILD_JOBS=1 cargo clippy --all-targets -- -D warnings && CARGO_BUILD_JOBS=1 cargo test` ✅ `4239 passed; 0 failed`
- `cd agent && npm run build && npm run check -w @bong/schema && npm test -w @bong/tiandao` ✅ schema artifacts fresh；tiandao `51 passed (51)` / `350 passed (350)`
- `cd client && JAVA_HOME="$HOME/.sdkman/candidates/java/17.0.18-amzn" ./gradlew test build` ✅ `BUILD SUCCESSFUL`
- `git diff --check origin/main..HEAD` ✅
- 评审收口增量验证：`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test cultivation::color_affinity` / `cultivation::generic_talent` / `cultivation::insight_apply` / `cultivation::insight_fallback`、`npm run build`、`npm run check -w @bong/schema`、`npm test -w @bong/tiandao -- insight-runtime`、`git diff --check origin/main..HEAD` ✅

### 跨仓库核验

- server：`InsightAlignment`、`InsightCost`、`InsightTradeoff`、`GenericTalentRegistry`、`select_aligned_tradeoffs`、`fallback_for_context`、`apply_choice`、`BiographyEntry::InsightDiverge`。
- agent/schema：`InsightChoiceV1.alignment`、`InsightChoiceV1.cost_*`、`applyInsightArbiter()`、`agent/packages/tiandao/src/skills/insight.md`。
- client：`InsightAlignment`、`InsightChoice.costSummary/costFlavor`、`InsightOfferScreen.describe()`、`HeartDemonOfferHandler.readChoices()`。

### 遗留 / 后续

- 代价字段已写入并累计到 `InsightModifiers`，但 combat / PvP / inspect 等 downstream 消费尚未全量接线；本 plan 落地的是三轨契约、持久字段、审计记录与 UI 可见性。
- Agent schema / prompt / Arbiter 已收紧为 3 个三轨选择和非零代价校验；当前 gameplay 执行权威仍是 server fallback，现有 agent IPC bridge 尚未把 LLM choice payload 直接作为可执行选择落地。
- Agent fallback 路径目前仍缺玩家 `QiColor` / `PracticeLog` 上下文；直到 bridge 升级前，真实玩家向量感知由 server fallback 侧保证。
- inspect 中的"顿悟倾向历史"与 PvP 侧模糊感知不在本 plan 范围，保留给后续 UI / 信息战 plan。
- 连续 DIVERGE 触发特殊叙事"你已不是你"未在本 plan 内落地，需结合后续杂色/混元叙事切片实现。
