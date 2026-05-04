# Bong · plan-alchemy-v2 · Active

> **状态**：✅ 完成（2026-05-04）—— P0–P4 全部代码落地。代码在工作分支 `auto/plan-alchemy-v2`（commits `2dba5b27` / `3ddae947` / `114a74ac`），PR 合并 main 后将归档至 `docs/finished_plans/`。前置 plan-alchemy-v1 / plan-alchemy-client-v1 / plan-combat-no_ui 全 ✅ finished。

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

- [x] **side_effect_pool 从字符串映射到真实效果**——当前 `tag` 只是字符串，P0 落地后丹药副作用才真正生效
- [x] **丹方残卷体现末法残缺感**——丹方不是完整知识，玩家只能从残卷学到有限配方；残缺版成品品阶受限
- [x] **品阶/开光是进深系统**——晋升炼丹路线的进深层，不影响 v1 基础炼丹（铭文不在本 plan，已并入 forge）
- [x] **AutoProfile 炉子有自己的 qi 储量**（user Q-A3）——`FurnaceQiReserve` 挂在 station 上，玩家通过 `InjectQiIntent` 注入；战斗不抽 player；炉 qi 与 PlayerQi 完全隔离
- [x] **丹心识别对应 worldview §九 情报换命**——消耗材料换情报，不是免费获取配方

---

## §1 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ✅ 2026-05-04 | `side_effect_pool` tag → `StatusEffectKind` 枚举映射 + 丹药服用触发副作用 | 单元：各 tag 映射不缺失；`ApplyStatusEffectIntent` 正确发出 |
| **P1** ✅ 2026-05-04 | 丹方残卷损坏（`RecipeFragment` 物品 + 残缺版学习路径） | 残缺丹方品阶上限 < 完整版；无法学到残缺段之外 |
| **P2** ✅ 2026-05-04 | 品阶 / 开光系统（v2 炼丹结果分层；~~铭文~~ 已并入 forge） | 品阶 1-5 对应不同效果幅度；开光为可选附加 |
| **P3** ✅ 2026-05-04 | AutoProfile 自动炼丹（傀儡绑炉，读火候曲线，高境界解锁） | 傀儡绑炉后 AutoProfile 输出品质 ≥ 手工 85%（平衡目标） |
| **P4** ✅ 2026-05-04 | 丹心识别（worldview §九 情报换命） | 消耗一颗丹药 → `RecipeHint` 物品入背包；agent narration 触发 |

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

## §4 P2：品阶 / 开光（铭文 → forge）

> **2026-05-04 范围修正**（user Q-A2）：~~铭文~~ 不属于本 plan。铭文是 forge 体系（`server/src/forge/session.rs:67 InscriptionState` + `forge/events.rs:39 InscriptionScrollSubmit` + 铭文残卷 / 失败率已实装）。**alchemy v2 不再涉及铭文**——丹药只走品阶 + 开光。

**品阶（1-5）**：
- 由炼丹结果的 deviation 分布决定（plan-alchemy-v1 已落 deviation 逻辑）
- 品阶 1 = 劣品，品阶 5 = 极品；效果幅度与品阶线性/指数映射

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
    pub qi_feed_rate: f32,             // 每秒真元消耗量（从 FurnaceQiReserve 抽）
    pub max_sessions: u32,             // 单次绑炉最多自动炼几炉
}

// 炉子专属真元储量（user Q-A3 决策：与 PlayerQi 完全隔离）
pub struct FurnaceQiReserve {
    pub current: f32,
    pub capacity: f32,                 // 由炉子品阶决定
    pub injection_rate_cap: f32,       // 玩家 InjectQiIntent 时的每秒注入上限
}

pub struct InjectQiIntent {
    pub furnace_entity: Entity,
    pub amount_per_sec: f32,           // 玩家选择的注入速率（≤ injection_rate_cap）
}
```

**约束**：
- AutoProfile 炼丹品质上限 = 手工该配方最高历史品质 × 85%（防全自动满品质）
- **真元由 `FurnaceQiReserve` 持有**（user Q-A3）：玩家通过 `InjectQiIntent` 主动注入；战斗时**不抽 PlayerQi**（炉子是独立账户）；储量耗尽 → 停炉
- 不能同时设定多个炉子（绑炉单一）
- **离场不停炉**：玩家走开后炉子继续按 AutoProfile 跑，直到 reserve 耗尽（核心价值——离场炼丹）

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

- [x] **Q-A1 ✅**（user 2026-05-04）：~~`InsightFlash` tag log+stub~~——顿悟系统已完整实装（`server/src/cultivation/insight.rs`：InsightCategory 7 类 + InsightQuota + InsightRequest/Offer/Chosen + InsightTriggerRegistry + apply_choice）。**P0 直接发 InsightRequest，不 stub**；具体走哪个 InsightCategory 由 tag 决定（如 `rare_insight_flash` → `Composure` 类一次性 +5%）。
- [x] **Q-A2 ✅**（user 2026-05-04）：~~铭文操作 UI~~——铭文不在本 plan，已并入 forge（`server/src/forge/session.rs:67 InscriptionState`）。alchemy v2 P2 仅做品阶 + 开光。
- [x] **Q-A3 ✅**（user 2026-05-04）：AutoProfile 真元储量 = 炉子专属 component `FurnaceQiReserve`，与 PlayerQi 完全隔离；玩家通过 `InjectQiIntent` 主动注入；战斗不抽。详 §5。
- [x] **Q-A4 ✅**（user 2026-05-04 D）：丹方残卷掉落来源留 P1 实施时拍（候选：tsy loot / NPC 交易 / 顿悟事件，三选 1 或全开）。

---

## §8 进度日志

- 2026-05-01：从 plan-alchemy-v1 reminder 整理立项。现有代码：`SideEffect { tag, duration_s, weight, perm, color, amount }` 结构已落（`server/src/alchemy/recipe.rs:134`）；`StatusEffectKind` enum 已有 7 个 variant（`server/src/combat/events.rs:60`）；side_effect → StatusEffect 映射、丹方残卷、品阶/铭文/开光、AutoProfile、丹心识别全部未实装。
- **2026-05-04**：skeleton → active 升级（user 拍板）。前置 plan-alchemy-v1 / plan-alchemy-client-v1 / plan-combat-no_ui 全 ✅ finished，依赖闭合。下一步起 P0 worktree（StatusEffectKind 扩展 + side_effect_apply.rs）。
- **2026-05-04**：§7 全部 4 决策闭环（Q-A1/A2/A3/A4 详 §7）。范围修正：删除 P2 铭文章节（移交 forge），新增 §5 `FurnaceQiReserve` + `InjectQiIntent` 结构。P0 直接接 InsightRequest（顿悟系统已实装）。
- **2026-05-04**：P0–P4 全部代码落地（commits `2dba5b27` server / `3ddae947` agent / `114a74ac` client，工作分支 `auto/plan-alchemy-v2`）。文档先标完成（user 拍板 B 选项，接受"文档先到代码后到"窗口）；待 PR 合并 main 后归档至 `docs/finished_plans/`。

---

## Finish Evidence

> **注意**：本 plan 的代码 commits 当前在工作分支 `auto/plan-alchemy-v2`，尚未合并到 main。在 main 上 grep 以下文件路径会扑空，属于已知"文档先到"窗口（user 拍板 B），不是虚标。PR 合并后此节即为标准 Finish Evidence。

### 落地清单

| 阶段 | 模块 / 文件 | 行数 | 关键 symbol |
|---|---|---|---|
| **P0** | `server/src/alchemy/side_effect_apply.rs` | 158 | `apply_side_effect()` · `SideEffectApplyError` · 5 个 `StatusEffectKind` 新 variant（`QiRegenBoost` / `InsightFlash` / `QiCapPermMinus` / `ContaminationBoost` / `AlchemyBuff(String)`，详 `server/src/combat/events.rs`） |
| **P1** | `server/src/alchemy/recipe_fragment.rs` | 152 | `RecipeFragment` · `FragmentLearnError` · 残缺规则（`UsablePartial` / `SeverelyDamaged`） |
| **P2** | `server/src/alchemy/quality.rs` | 105 | `QualityTier`（1–5）· `void_consecration()` 化虚祝圣开光逻辑 |
| **P3** | `server/src/alchemy/auto_profile.rs` | 214 | `AlchemyAutoProfile` · `FurnaceQiReserve`（独立账户）· `InjectQiIntent` |
| **P4** | `server/src/alchemy/danxin.rs` | 227 | `DanxinIdentifyIntent` · `RecipeHint` · `accuracy = min(1.0, (realm_tier / pill_tier) × random(0.5, 1.0))` · `AlchemyInsightEvent`（agent narration 触发） |

`server/src/alchemy/mod.rs` +82 行登记 5 个新 mod；`server/assets/items/pills.toml` +22 行新增样例丹药条目。

### 关键 commits

- `2dba5b27` 2026-05-04 — `feat(alchemy): 落地炼丹 v2 服务端扩展`（server，36 文件 +1100 行）
- `3ddae947` 2026-05-04 — `feat(agent): 接入炼丹洞察契约`（agent，schema + IPC）
- `114a74ac` 2026-05-04 — `feat(client): 展示炼丹物品元数据`（client，3 文件 +174 行）

### 测试结果

- **Server**：5 模块 × 3–4 单测 = **18 个 Rust 单元测试**
  - `side_effect_apply.rs` 4 / `recipe_fragment.rs` 3 / `quality.rs` 4 / `auto_profile.rs` 4 / `danxin.rs` 3
- **Agent**：`agent/packages/schema/tests/schema.test.ts` + `agent/packages/tiandao/tests/redis-ipc.test.ts` 各 +1，共 **2 个新 vitest case**
- **Client**：随 `InventorySnapshotHandler` / `InventoryItem` 扩展更新，无新增独立 test（依赖既有 inventory 集成测试）

### 跨仓库核验

- **server ↔ agent**：`AlchemyInsightEvent`（server）↔ `agent/packages/schema/src/alchemy.ts` +17 行 `AlchemyInsightSchema` ↔ `agent/packages/schema/samples/alchemy-insight.sample.json` ↔ `generated/alchemy-insight-v1.json`（53 行 JSON Schema）
- **server ↔ client**：`server/src/network/inventory_event_emit.rs` +9 行扩展 inventory snapshot payload（品阶 / 铭文 / 残卷标记）↔ `client/.../inventory/model/InventoryItem.java` +80 行解析新 metadata ↔ `ItemTooltipPanel.java` 渲染品阶
- **agent IPC channel**：`agent/packages/schema/src/channels.ts` +3 行注册新 channel；`tiandao/src/redis-ipc.ts` +22 行订阅 alchemy-insight

### 遗留 / 后续

- **PR 合并 main**：工作分支 `auto/plan-alchemy-v2` 待合并；`sync/main-alchemy-v2-20260504` 是 sync 中间分支。合并后人工 / `/consume-plan` 在末尾 commit 内 `git mv docs/plan-alchemy-v2.md docs/finished_plans/`
- **§5 离场不停炉的实战平衡**：AutoProfile 输出品质 ≤ 手工 85% 的平衡阈值已写入代码，但需要长程游玩验证（属 plan 验收范围之外，归 balance 调参）
- **Q-A4 残卷掉落来源**：plan §7 留待 P1 实施时拍——目前代码仅落 `RecipeFragment` 结构体，掉落表 / 兑换表挂接到 `tsy loot` / NPC 交易 / 顿悟事件需后续 plan（候选 plan 名 `plan-recipe-fragment-source-v1`，未立）
