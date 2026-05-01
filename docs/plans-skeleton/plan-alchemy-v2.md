# Bong · plan-alchemy-v2 · 骨架

炼丹系统扩展：side_effect_pool → StatusEffect 映射 / 丹方残卷（残缺学习）/ 品阶-铭文-开光系统 / AutoProfile 自动炼丹 / 丹心识别（玩家逆向配方）。`plan-alchemy-v1`（finished）的后续扩展，不重复已落地的核心炼丹链路。

**世界观锚点**：`worldview.md §九 情报与谈判`（丹心识别 = "情报换命"钩子）· `worldview.md §五 炼丹流`（五炼境炼丹师 / 品阶 / 傀儡绑炉 AutoProfile）

**交叉引用**：
- `plan-alchemy-v1`（finished）— 核心炼丹链路（FlawedFallback / side_effect_pool 结构已落 `server/src/alchemy/recipe.rs`）
- `plan-alchemy-client-v1`（finished）§7 P5 — 正式配方名称正典化已交由此 plan，v2 不重叠
- `plan-alchemy-recycle-v1`（active）— 炼丹回收；v2 不重叠
- `plan-combat-no_ui`（finished）`StatusEffectKind` enum（`server/src/combat/events.rs`）— v2 P0 需新增 side_effect tag 对应的 variant

---

## 接入面 Checklist

- **进料**：
  - 现有 `side_effect_pool: Vec<SideEffect>` 中的 `tag: String`（`server/src/alchemy/recipe.rs:134`）
  - 炼丹成品 `pill.rs` 进 `inventory` → `StatusEffects` component
  - worldview §九"情报换命"—丹心识别：玩家分析丹药 → 获取配方碎片
- **出料**：
  - `ApplyStatusEffectIntent { kind: StatusEffectKind::AlchemySideEffect(_), ... }` 进战斗系统
  - 丹方残卷：`RecipeFragment` 进 `inventory`，学习后解锁残缺版配方
  - 丹心识别：消耗一颗丹药 → 产出 `RecipeHint`（配方碎片）进 `inventory`
- **共享类型**：复用 `StatusEffectKind`（新增 alchemy 相关 variant）· 复用 `PlayerInventory` / `ItemInstance`
- **跨仓库契约**：
  - server：`server/src/alchemy/` 扩展
  - agent：丹心识别结果可触发 narration（`bong:alchemy_insight`）
  - client：品阶/铭文展示（inspect 面板 tooltip 扩展）
- **worldview 锚点**：§五 炼丹流 + §九 情报换命

---

## §0 设计轴心

- [ ] **side_effect_pool 从字符串映射到真实效果**——当前 `tag` 只是字符串，P0 落地后丹药副作用才真正生效
- [ ] **丹方残卷体现末法残缺感**——丹方不是完整知识，玩家只能从残卷学到有限配方；残缺版成品品阶受限
- [ ] **品阶/铭文/开光是进深系统**——晋升炼丹路线的进深层，不影响 v1 基础炼丹
- [ ] **AutoProfile 是低频高效工具**——傀儡绑炉，玩家设定曲线后可半自挂机炼丹；高境界特权
- [ ] **丹心识别对应 worldview §九 情报换命**——消耗材料换情报，不是免费获取配方

---

## §1 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ⬜ | `side_effect_pool` tag → `StatusEffectKind` 枚举映射 + 丹药服用触发副作用 | 单元：各 tag 映射不缺失；`ApplyStatusEffectIntent` 正确发出 |
| **P1** ⬜ | 丹方残卷损坏（`RecipeFragment` 物品 + 残缺版学习路径） | 残缺丹方品阶上限 < 完整版；无法学到残缺段之外 |
| **P2** ⬜ | 品阶 / 铭文 / 开光系统（v2 炼丹结果分层） | 品阶 1-5 对应不同效果幅度；铭文 / 开光为可选附加 |
| **P3** ⬜ | AutoProfile 自动炼丹（傀儡绑炉，读火候曲线，高境界解锁） | 傀儡绑炉后 AutoProfile 输出品质 ≥ 手工 85%（平衡目标） |
| **P4** ⬜ | 丹心识别（worldview §九 情报换命） | 消耗一颗丹药 → `RecipeHint` 物品入背包；agent narration 触发 |

---

## §2 P0：side_effect_pool 映射

现有 `SideEffect.tag` 字符串（`server/src/alchemy/recipe.rs::SideEffect`）与战斗 `StatusEffectKind` enum 的对应关系需要在此 phase 建立：

```rust
// 建议新增 StatusEffectKind variant（server/src/combat/events.rs）
pub enum StatusEffectKind {
    // 现有...
    Bleeding, Slowed, Stunned, DamageAmp, DamageReduction, BreakthroughBoost, Humility,
    // alchemy v2 新增：
    QiRegenBoost,          // tag: "minor_qi_regen_boost" / "qi_regen_boost"
    InsightFlash,          // tag: "rare_insight_flash"（触发一次顿悟机会）
    QiCapPermMinus,        // tag: "qi_cap_perm_minus_1"（永久，amount=1%）
    ContaminationBoost,    // tag: "contam_boost"（施毒类副作用）
    AlchemyBuff(String),   // 兜底：未知 tag 作为字符串保留，log warn
}
```

映射函数位置：`server/src/alchemy/side_effect_apply.rs`（新文件）

**测试要求**：
- 每个 tag → variant 有专属正向测试
- `AlchemyBuff(String)` 兜底路径 log warn + 不 panic
- 永久副作用（`perm=true`）触发后不自动超时（duration=0）

---

## §3 P1：丹方残卷损坏

```rust
// server/src/alchemy/recipe_fragment.rs（新文件）

pub struct RecipeFragment {
    pub recipe_id: String,
    pub known_stages: Vec<u8>,        // 可学习的 stage 索引（残缺）
    pub max_quality_tier: u8,         // 残卷学到的品阶上限（1-3，完整版可达 5）
}

// ItemTemplate 新增 category：RecipeFragment
// 学习操作：消耗 RecipeFragment 物品 → AlchemyKnowledge 中加入 known_stages
```

**残缺版配方规则**：
- 残缺 stage 数 ≥ 50%：可正常炼丹，品阶上限 3
- 残缺 stage 数 < 50%：高频 deviation，品阶上限 1
- 丹方残卷本身是 `rare` 稀有度，只能从 tsy 副本 / NPC 交易 / 顿悟事件获得

---

## §4 P2：品阶 / 铭文 / 开光

**品阶（1-5）**：
- 由炼丹结果的 deviation 分布决定（plan-alchemy-v1 已落 deviation 逻辑）
- 品阶 1 = 劣品，品阶 5 = 极品；效果幅度与品阶线性/指数映射

**铭文**：
- 只有品阶 3+ 的丹药才可铭文
- 需要额外材料（灵石 / 特定草药）+ 手动"刻铭"操作
- 铭文效果：额外一条 `StatusEffect`（类似 bonus mod）

**开光**：
- 品阶 5 丹药 + 化虚修士在场祝圣
- 效果：duration ×2 + 一次额外药效爆发（强化版 BreakthroughBoost）

---

## §5 P3：AutoProfile 自动炼丹

**定位**：固元境+ 解锁；傀儡绑炉后玩家设定火候曲线模板，炉子按模板自动运行。

```rust
pub struct AlchemyAutoProfile {
    pub profile_id: String,
    pub fire_curve: Vec<(f32, f32)>,   // (time_pct, temperature) 序列
    pub qi_feed_rate: f32,             // 每秒真元注入量
    pub max_sessions: u32,             // 单次绑炉最多自动炼几炉
}
```

**约束**：
- AutoProfile 炼丹品质上限 = 手工该配方最高历史品质 × 85%（防全自动满品质）
- 真元消耗由玩家事先注入"真元储量"（不实时抽），储量耗尽后停炉
- 不能同时设定多个炉子（绑炉单一）

---

## §6 P4：丹心识别（worldview §九 情报换命）

**流程**：
1. 玩家持丹药 → 右键 → "丹心识别" 操作
2. 消耗该丹药一颗（原料不可回收）
3. server 根据玩家境界 + 丹药品阶 roll 识别精度（0–100%）
4. 输出 `RecipeHint` 物品（描述：可辨别的 1-3 个草药成分）进背包
5. 若识别精度 ≥ 80%：额外触发 `bong:alchemy_insight` → agent narration

**识别精度公式**：
```
accuracy = min(1.0, (realm_tier / pill_tier) × random(0.5, 1.0))
// realm_tier: 醒灵=1…化虚=6；pill_tier: 配方境界对应
```

**丹心识别不能还原完整配方**——只能得到"碎片线索"；复原全配方需多次识别 + 交叉推断（玩家脑内完成，不做自动合成）。

---

## §7 开放问题

- [ ] `InsightFlash` tag：触发顿悟机会的具体实现依赖 plan-cultivation 顿悟池，P0 可先 log + stub
- [ ] 铭文操作 UI：需要 client plan 承接；P2 先落 server 逻辑，client 后接
- [ ] AutoProfile 的"真元储量"：是新的 component 还是复用现有 PlayerQi？是否可被战斗抽干？
- [ ] 丹方残卷掉落来源：tsy 副本 loot table（`plan-tsy-loot-v1`）、NPC 交易、顿悟事件——需与对应 plan 协商

---

## §8 进度日志

- 2026-05-01：从 plan-alchemy-v1 reminder 整理立项。现有代码：`SideEffect { tag, duration_s, weight, perm, color, amount }` 结构已落（`server/src/alchemy/recipe.rs:134`）；`StatusEffectKind` enum 已有 7 个 variant（`server/src/combat/events.rs:60`）；side_effect → StatusEffect 映射、丹方残卷、品阶/铭文/开光、AutoProfile、丹心识别全部未实装。
