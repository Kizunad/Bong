# Bong · plan-alchemy-advanced-v1 · 骨架

**高级炼丹**。接入丹药副作用 StatusEffect、残卷机制（残缺版学习）、品阶系统、铭文/开光强化、丹心识别（逆向配方）、AutoProfile 自动化炼丹。

**世界观锚点**：`worldview.md §三`（"丹药永远是辅助，万物皆有代价"——品阶越高副作用越强）· `worldview.md §九`（"情报换命"："花时间和灵石换取情报，再以情报换命或换物"——丹心识别是"情报"的具体形式）· `worldview.md §十一`（秘法传承：残卷是传承碎片，符合末法稀缺叙事）

**接入面**：
- **进料**：`RecipeRegistry`（已加载 JSON）→ 残卷扩展 · `AlchemySession.outcomes`（已产出）→ 品阶注入 · `ApplyPillRequest`（已实装）→ 识别路径
- **出料**：`ApplyStatusEffectIntent` → `combat::status` tick（副作用接入）· `PartialRecipeScroll` item → `inventory`（识别产出）· `ItemInstanceV1.grade` → 客户端 tooltip
- **共享类型**：`StatusEffectKind`（`server/src/combat/events.rs:60`）· `ItemEffectV1`（inventory schema）· `AlchemySession`（`server/src/alchemy/`）
- **worldview 锚点**：§三 代价系统 + §九 情报换命

**交叉引用**：`plan-alchemy-v1`（完成，基座）· `plan-alchemy-client-v1`（Fabric 客户端接入）· `plan-combat-no_ui`（StatusEffectKind 基座）· `plan-inventory-v1`（物品实例化）· `plan-botany-v1/v2`（药材来源）

---

## §0 设计轴心

- [ ] **副作用先接入**（P0）——解除现有 `side_effect_pool` 字符串占位，让已有系统闭合
- [ ] **残卷 = 传承稀缺**——末法世界完整配方是奢侈品，学到一半就能凑合炼，但有代价
- [ ] **品阶 = 工艺精度**——不引入新材料，而是把现有火候精准度映射为品阶差异
- [ ] **丹心识别 = 情报**——吃一颗别人炼的丹，可以得到部分配方碎片；与 worldview §九"情报换命"呼应
- [ ] **AutoProfile = 傀儡手艺**——末法傀儡没有心神，炼丹成品率比人工低；自动化不免费

## §1 副作用 → StatusEffectKind 映射（P0）

**阶段状态**：⬜

**可核验交付物**：
- 扩展 `StatusEffectKind` enum（`server/src/combat/events.rs:60`）——新增：
  ```rust
  QiRegenBoost,       // 真元自然恢复 ×1.5，持续 N ticks
  InsightFlash,       // 本次境界推进经验 +10%，单次触发
  QiCapPermMinus,     // 真元上限永久 -X%（写入 cultivation component）
  StaminaBoost,       // 体力恢复加速，持续 N ticks
  BreathingDisorder,  // 真元自然恢复 ×0.5，持续 N ticks（毒）
  ```
- `server/src/combat/status.rs`：为上述 variant 实现 tick 效果
  - `QiCapPermMinus`：tick 首帧写入 `cultivation.qi_max *= (1.0 - delta)`，立即消费不循环
  - `InsightFlash`：tick 首帧 emit 自定义 event，由 cultivation 消费加经验
- `server/src/alchemy/brew.rs`：成丹结算遍历 `recipe.side_effect_pool` tags → 映射 `StatusEffectKind` → emit `ApplyStatusEffectIntent`
  - 映射表：`"minor_qi_regen_boost"→QiRegenBoost`、`"rare_insight_flash"→InsightFlash`、`"qi_cap_perm_minus_1"→QiCapPermMinus(0.01)`、`"stamina_boost"→StaminaBoost`、`"breathing_disorder"→BreathingDisorder`
- schema pin 测试：每个 StatusEffectKind variant 有正反 sample 对拍
- 测试 `combat::status::*`：`qi_regen_boost_applies`、`qi_cap_perm_minus_permanent`、`breathing_disorder_halves_regen`、`insight_flash_triggers_once`、`status_effect_kind_serde_all_variants`（5 单测）

## §2 丹方残卷损坏（P1）

**阶段状态**：⬜

**可核验交付物**：
- `RecipeFragment { recipe_id: RecipeId, known_stages: Vec<u8>, completeness: f32 }` struct（`server/src/alchemy/recipe.rs`）
- `PartialRecipeScroll` 物品模板（`server/assets/items/` JSON）：持有 `RecipeFragment`
- `RecipeKnowledgeStore` resource：`HashMap<PlayerId, HashMap<RecipeId, RecipeFragment>>`
  - `learn_fragment(player, fragment)` → 合并已有碎片（`completeness = max(old, new)`）
  - `is_complete(player, recipe_id) -> bool`（completeness ≥ 1.0）
- 炼制路径分支：
  - `is_complete == false`：只能走 `flawed_fallback` outcome + 缺失 stage 导致炸炉概率 +50%
  - `is_complete == true`：正常全分支
- worldgen loot table 新增残卷散落（ruins / dead NPC drops）
- 测试：`recipe_knowledge::fragment_merge`、`incomplete_recipe_forces_flawed`、`missing_stage_increases_explosion`、`complete_recipe_normal_path`（4 单测）

## §3 品阶系统（P2）

**阶段状态**：⬜

**可核验交付物**：
- `AlchemyGrade: u8`（1=废丹 / 2=凡品 / 3=良品 / 4=佳品 / 5=极品）
- 品阶决定因子（`server/src/alchemy/brew.rs` 结算）：
  - `fire_accuracy`：实际温度曲线 vs 目标曲线的 RMSE，低→高分
  - `qi_efficiency`：`qi_injected / recipe.qi_required`，过多/过少均降分
  - `ingredient_freshness`：材料 `shelflife` 剩余比例（需 plan-shelflife 接入）
  - 综合评分 → grade bucket
- `ItemInstanceV1.grade: Option<u8>`（schema 新增字段，`#[serde(default)]`）
- 品阶影响效果强度：main_effect_potency × (grade / 3.0)（grade=3 为基准）
- 客户端 `ItemTooltipPanel`：展示品阶星级标记（接线在 plan-alchemy-client-v1）
- 测试：`alchemy::grade::perfect_accuracy_gives_5`、`grade::poor_qi_reduces_grade`、`grade::affects_potency`、`grade::serde_roundtrip`、`grade::boundary_bucket_mapping`（5 单测）

## §4 铭文 / 开光（P3）

**阶段状态**：⬜

**可核验交付物**：
- `InscriberStation` 实体（`server/src/alchemy/inscriber.rs`）：独立于 `AlchemyFurnace`
- **铭文**：
  - `InscribeRequest { station_entity, item_instance_id, rune_item_id }` → handler
  - 消耗 rune 材料（来自 plan-mineral 矿物体系）
  - 成功：目标 `ItemInstance.inscriptions.push(rune_tag)`，效果强化
  - 失败概率 = `1 - cultivation_realm_factor`；失败 → 品阶 -1（最低到 1）
- **开光**：
  - `ConsecrateRequest { item_instance_id, qi_invest: f32 }`
  - 临时效果：品阶 +1（不改 item 持久 grade），持续 `qi_invest × 100` ticks
  - `qi_invest > qi_max * 0.3` → 丹药爆炸（item 销毁 + 经脉 MICRO_TEAR）
- 测试：`inscribe_success_adds_tag`、`inscribe_fail_reduces_grade`、`consecrate_boost_temporary`、`consecrate_overcharge_destroys`（4 单测）

## §5 丹心识别（P4）

**阶段状态**：⬜

**可核验交付物**：
- `ApplyPillRequest` 新增可选字段 `attempt_identify: bool`
- `identify_pill` 路径（`server/src/alchemy/identify.rs`）：
  - 消耗真元 `qi_cost = qi_max * 0.1`
  - 识别成功条件：`cultivation.meridian_integrity > 0.5` 或拥有"神识感知"技能（plan-perception）
  - 成功 → 产出 `PartialRecipeScroll { completeness: 0.3~0.5, known_stages: [0] }`
  - 失败 → 得知主效果类型字符串，无配方
- agent narration hook：emit `bong:alchemy_identify` event，天道评语"道心识方，万物皆可逆"（概率触发）
- 测试：`identify_success_yields_fragment`、`identify_fail_yields_effect_type`、`identify_qi_cost`、`identify_requires_integrity`（4 单测）

## §6 AutoProfile / 傀儡绑炉（P5）

**阶段状态**：⬜

**可核验交付物**：
- `FurnacePuppetBinding { puppet_npc_entity: Entity, fire_profile: Vec<(u32, f32)> }` component on `AlchemyFurnace`
- 绑定路径：`BindFurnacePuppetRequest { furnace_entity, npc_entity }` → server handler → 若 NPC 为傀儡类型则绑定
- 自动 tick：每帧执行一次 `puppet_furnace_tick`，按 fire_profile 曲线发 `AdjustTemp` intervention
- 傀儡劣化：自动操作成品率比人工 -20%（`fire_accuracy_penalty = 0.20`），体现"心神专注"价值
- 傀儡离线继续运行（server 侧 tick 不依赖玩家在线）
- 解绑：玩家可随时发 `UnbindFurnacePuppetRequest` 取回傀儡
- 测试：`puppet_binding_works`、`puppet_reduces_accuracy`、`puppet_ticks_offline`、`unbind_releases_puppet`（4 单测）

## §7 开放问题

- [ ] `QiCapPermMinus` 累积到真元上限 → 0 时怎么处理（设地板 0.1 / 触发 plan-death?）
- [ ] 识别消耗真元与丹药本身的 qi_cost 是否冲突（先识别后服用，还是同步）？
- [ ] 品阶与炉等（`furnace.tier`）的关系——高阶炉是否开放更高品阶上限？
- [ ] 傀儡绑炉是否需要 NPC 具备特定技能（plan-npc-ai）还是任意傀儡都行？
