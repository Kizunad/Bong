# Bong · plan-longevity-v1 · 骨架

**续命体系**。以代价换寿——续命丹（业力）、夺舍（境界）、坍缩渊深潜（风险）；风烛 buff 数值定案；生平卷善终/横死分类字段；寿元时钟三方协调验证。

**世界观锚点**：`worldview.md §十三`（寿元宽裕上限 line 774+"续命是存在的但没有免费午餐"line 816）· `worldview.md §十二`（死亡/终结规则："老死 = 遗物按善终规则留容器"line 863）· `worldview.md §十六`（活坍缩渊深层"最顶级遗迹"→ 换寿语境）· `worldview.md §二`（夺舍原文："传你一门地阶功法 · 他恢复真元后可能夺舍你"line 547）

**接入面**：
- **进料**：`combat::lifecycle` 死亡/老死分支 → 风烛触发 · `alchemy::brew` 成丹注入 StatusEffect · `world::zone` 坍缩渊负压阈值 → 换寿事件
- **出料**：`StatusEffectKind::WindCandle` → `combat::status` tick · `LifeRecord.death_manner` → 亡者博物馆 · `ItemEffectV1.kind="extend_longevity"` → inventory consume
- **共享类型**：复用 `StatusEffectKind` enum（`server/src/combat/events.rs:60`）· `LifeRecord`（`server/src/combat/lifecycle.rs`）· `ApplyStatusEffectIntent`
- **worldview 锚点**：§十三 寿元 + §十二 终结 + §十六 坍缩渊

**交叉引用**：`plan-death-lifecycle-v1`（完成，基座）· `plan-alchemy-v1`（完成，续命丹配方接入）· `plan-cultivation-v1`（境界/真元上限永久修改）· `plan-tsy-dimension-v1`（坍缩渊位面）

---

## §0 设计轴心

- [ ] 续命存在，但**没有免费午餐**——每条路径都有不可逆代价（业力、境界、风险）
- [ ] **风烛** = 剩余寿元 < 10% 时的持续 debuff，不是立刻死亡；是"时限警告"
- [ ] **善终 vs 横死** = 生平卷分类，影响亡者博物馆展示风格与遗物规则
- [ ] **夺舍**是唯一允许占用他人角色的 mechanic——代价是攻击者境界退步（末法悖论）
- [ ] 坍缩渊换寿 = "以劫换命"，拿深渊风险兑换额外年岁；只能触发一次

## §1 风烛 StatusEffect 实装（P0）

**阶段状态**：⬜

**可核验交付物**：
- `StatusEffectKind::WindCandle` 加入 `server/src/combat/events.rs:60` enum
- 触发条件：`cultivation::life_remaining_ratio < 0.10` → emit `ApplyStatusEffectIntent { kind: WindCandle, duration_ticks: u32::MAX }`（持续型，不自动到期，随寿元回升而消除）
- tick 效果（`server/src/combat/status.rs`）：
  - 真元自然恢复速率 × 0.5
  - 每 `AGING_NARRATION_INTERVAL_TICKS`（= 1 服务器天 = 72000 ticks @ 20 TPS）emit `DeathInsightRequested { category: AgingWarning }`
- 消除条件：寿元回升 ≥ 10%（续命丹生效后）→ remove WindCandle from StatusEffects
- 测试 `server/src/combat/status.rs`：`wind_candle_applied_at_threshold`、`wind_candle_removed_when_healed`、`wind_candle_halves_qi_recovery`、`wind_candle_persistent_not_timed_out`、`wind_candle_narration_interval`（共 5 单测）

## §2 善终 / 横死 生平卷分类（P1）

**阶段状态**：⬜

**可核验交付物**：
- `DeathManner` enum（`server/src/combat/lifecycle.rs` 或独立 `life_record.rs`）：
  ```rust
  pub enum DeathManner {
      NaturalAging,           // 寿元耗尽
      Fallen { cause: CultivationDeathCause },   // 战斗/污染/域崩/天劫
      SoulSeized,             // 被夺舍
      Reclused,               // 主动终结（劝退 prompt 选"归隐"）
  }
  ```
- `LifeRecord` 新增 `death_manner: Option<DeathManner>`，`#[serde(default)]` 兼容旧档
- 死亡分流：`lifecycle.rs` 各死亡路径填写对应 DeathManner 并写入 LifeRecord
- **善终规则**（NaturalAging / Reclused）：不掉落物品，生成遗骸容器；`agent` narration 语调为"落叶归根"
- **横死规则**（Fallen / SoulSeized）：正常掉落；narration 语调为"草木皆兵"
- 亡者博物馆 `LifeRecord` JSON 包含 `death_manner` 字段可读
- 测试：`life_record::death_manner_serde`、`natural_aging_no_drop`、`fallen_normal_drop`、`reclused_no_drop`（4 单测）

## §3 续命丹（P2）

**阶段状态**：⬜

**可核验交付物**：
- 配方文件 `server/assets/alchemy/recipes/shoumingdan.json`：
  - 主效果：`ItemEffectV1.kind = "extend_longevity"`，参数 `{ years: N }`（N 按境界：醒灵 10 / 引气 20 / 凝脉 40 / 固元 80 / 通灵 150 / 化虚 300）
  - 副作用：`ContamSource.amount = 0.15`（药三分毒）+ `side_effect_pool: ["karma_increase_minor"]`
  - 容差：精准火候可轻微提升 N，粗糙火候有 30% 概率直接失效（产出废丹）
- **续命递减**：`KarmaComponent` 每次消费续命丹 +0.3，当累计 `karma ≥ 3.0` 时再服用续命丹效果减半
- `apply_pill_system` 识别 `extend_longevity` → 调 `cultivation::lifespan::extend_remaining`
- `cultivation::lifespan::extend_remaining`：将剩余寿元 += years，不超过当前境界上限
- 测试：`alchemy::shoumingdan_recipe_loads`、`extend_longevity_capped_by_realm`、`karma_tolerance_halves_effect`、`wind_candle_removed_after_extend`（4 单测）

## §4 夺舍（P3）

**阶段状态**：⬜

**可核验交付物**：
- `SoulSeizureIntent` event（`server/src/combat/events.rs`）
- 前置条件（`resolve.rs` 或独立 `soul_seize.rs`）：
  - 攻击者 realm = 化虚 (`CultivationRealm::HuaXu`)
  - 目标 `qi_current / qi_max < 0.20`
  - 攻击者与目标距离 ≤ 2 格（贴身）
  - 攻击者 `karma < 5.0`（过高业力无法稳住夺舍）
- 灵识对抗 roll：`attacker.qi_max vs target.qi_max * (1 + target.meridian_integrity)`，成功概率 P = attacker / (attacker + target)
- **成功**：目标角色 `Lifecycle.soul_seized = true` → 下一 tick 进入终结流程（`DeathManner::SoulSeized`）；攻击者 `realm` 退降一级 + `karma += 1.5`
- **失败**：攻击者 `contamination += 0.5`（反噬）
- **反制**：目标可在 1s 内消耗 `qi_invest = qi_max * 0.5` 触发 `SoulSeizureDefense` → roll 优势 (+30%)
- 测试：`soul_seize_requires_huaxu`、`soul_seize_success_terminates_target`、`soul_seize_attacker_demotes_realm`、`soul_seize_fail_contaminates_attacker`、`soul_seize_defense_advantage`（5 单测）

## §5 坍缩渊换寿（P4）

**阶段状态**：⬜

**可核验交付物**：
- `CraterLifeExchangeEvent` event（`server/src/world/tsy.rs` 或 `crater.rs`）
- 触发：玩家在活坍缩渊位面 + `zone.spirit_qi ≤ -0.80` + 交互特殊实体 `CraterRelicPillar`
- 效果：延寿 `+100 × (|spirit_qi| - 0.7) / 0.5` 年（深度越大越多，上限 300 年）；同时 roll：50% 境界退 1 级 / 30% 业力 +1.0 / 20% 仅污染 +0.3
- **每个角色生命周期只能触发一次**（`LifeRecord.crater_exchange_used: bool`）
- Agent narration：天道评语"以渊换命，天道见之，一笑而过"
- 测试：`crater_exchange_extends_life`、`crater_exchange_once_per_lifetime`、`crater_exchange_depth_scales`（3 单测）

## §6 寿元时钟协调验证（P5）

**阶段状态**：⬜

**可核验交付物**：
- 验证三处一致性（文档 + 集成测试）：
  1. `TICKS_PER_GAME_YEAR = 72000`（20 TPS × 3600s）在 `server/src/cultivation/longevity.rs` 或配置文件中为常量
  2. 醒灵→化虚 正常节奏 ≤ 51.5h（= 51.5 game years），远低于 通灵期上限 1000 年
  3. 死亡扣寿 5% 数值表与境界上限一致（凡人 4y / 醒灵 6y / ... 化虚 100y）
- 离线倍率 `OFFLINE_LIFESPAN_RATE = 0.1` 常量化并有测试
- 死域/负灵域加速 `DEATH_ZONE_LIFESPAN_RATE = 2.0` 常量化并有测试
- 测试：`longevity::clock_ticks_per_year`、`offline_multiplier_boundary`、`death_zone_multiplier`、`realm_transition_lifespan_update`、`overage_converts_to_elder_death`（5 单测）

## §7 开放问题

- [ ] 续命丹的 `years` 数值是否与当前 51.5h 化虚基线在"极端续命"场景下平衡（多次服用能否把凡人撑到通灵期）？
- [ ] 夺舍后攻击者降境 + 业力 → 是否触发定向天罚（plan-tribulation §1 定向天罚 threshold）？
- [ ] `DeathManner::Reclused` 与 plan-death-lifecycle §3 的"主动终结 / 劝退 prompt"如何精确对接（劝退 prompt 在前，归档在后）？
