# Bong · plan-woliu-v4 · 涡流派全流程：获取→修炼→天赋路线

涡流功法全生命周期：从「修士如何第一次学会涡流招式」到「练到极致触发顿悟成为涡流大师」。四条获取渠道（残卷 / 观摩 / NPC 传功 / 垂死大能）、熟练度成长与效果缩放、涡流专属天赋路线。承接 `plan-woliu-v1/v2/v3` 已落地的 11 招 + 战斗物理 + 视听体系——v4 补全从「0 招」到「涡流大师」的玩家旅程。

| 阶段 | 内容 | 状态 |
|---|---|---|
| **P0** | 涡流残卷获取闭环（item + 掉落 + 研读 + 学招） | ⬜ |
| **P1** | 观摩领悟 + NPC 传功路径 | ⬜ |
| **P2** | 垂死大能事件 + 熟练度成长 | ⬜ |
| **P3** | 涡流天赋路线 + 全流程验收 | ⬜ |

**世界观锚点**：
- `worldview.md §五:442-455` 涡流核心原理（掌心负灵域 / 真空吸扯 / 反噬致残）
- `worldview.md §五:535-557` 流派由组合涌现 / 五维适配度 / 专精档位代价
- `worldview.md §六:654-666` 顿悟触发时机（功法练满 / 濒死 / 天劫等）
- `worldview.md §九:880` 残卷 = 击杀道伥 / 遗迹探索 → 学习术法
- `worldview.md §七:727` 道伥 = 获取残卷的唯一途径（之一）
- `worldview.md §七:742` 垂死的大能 = 传功路径（极稀有 + 陷阱）

**library 锚点**：`cultivation-0004 涡流散人手札`（真空场与灵气压差的关系）· `peoples/战斗流派源流`（涡流防御三流定位）

**前置依赖**：
- `plan-woliu-v1/v2/v3` ✅ → 涡流 11 招注册 + combat module + 视听
- `plan-skill-v1` ✅ → SkillRegistry / Casting / cooldown
- `plan-cultivation-v1` ✅ → Realm / Cultivation / Meridian / MeridianSystem
- `plan-combat-no_ui` ✅ → CombatEvent / resolve
- `plan-craft-v1` ✅ → RecipeUnlockState / unlock_via_scroll 三渠道模式（残卷/师承/顿悟）
- `plan-style-vector-integration-v1` ✅ → PracticeLog / QiColor / evolve_qi_color
- `plan-multi-style-v1` ✅ → is_hunyuan / 混元色
- `plan-meridian-severed-v1` ✅ → SkillMeridianDependencies
- `plan-vfx-v1` ✅ / `plan-particle-system-v1` ✅ → 粒子基类
- `plan-audio-v1` ✅ → SoundRecipePlayer
- `plan-HUD-v1` ✅ → BongHudOrchestrator
- `plan-npc-ai-v1` ✅ → big-brain Utility AI / NPC dialog

**反向被依赖**：
- 其他 6 流派的 vN+1 plan → 复用本 plan 建立的「战斗功法残卷研读」+ 「观摩领悟」+ 「传功」通道
- `plan-style-balance-v1` ⬜ → 涡流天赋参数进平衡矩阵
- `plan-gameplay-journey-v1` ⬜ skeleton → §A.5 涡流获取节奏锚点

---

## 接入面 Checklist

- **进料**：
  - `npc::loot::NpcLootTable` / `npc::tsy_hostile::*` → 道伥 + 遗迹掉落残卷
  - `inventory::ItemInstance` → 残卷物品实例（scroll_kind="combat_technique", scroll_skill_id="woliu.*"）
  - `cultivation::Cultivation` / `Realm` → 研读 / 观摩前置境界检查
  - `cultivation::known_techniques::KnownTechniques` → 学招写入
  - `cultivation::components::MeridianSystem` → 经脉前置检查
  - `combat::woliu_v2::VortexCastEvent` → 观摩事件源
  - `cultivation::color::QiColor` / `PracticeLog` → 领悟概率 / 天赋关联
  - `cultivation::insight::*` → 顿悟触发 + InsightEffect
  - `cultivation::generic_talent::GenericTalentRegistry` → 新增天赋条目
- **出料**：
  - 新增 `server/src/cultivation/technique_scroll.rs` — 战斗功法残卷研读系统
  - 新增 `server/src/cultivation/technique_observe.rs` — 观摩领悟系统
  - 新增 `server/src/cultivation/technique_mentor.rs` — NPC 传功系统
  - 新增 `server/src/cultivation/technique_proficiency.rs` — 熟练度成长系统
  - 修改 `KnownTechniques::default()` — 涡流招式不再默认给出
  - 新增 `InsightEffect::VortexBackfireResist` / `VortexRadiusBonus` / `VortexFlowSpeed` — 涡流专属顿悟
  - 新增 `server/assets/insight/generic_talents.json` 涡流条目
  - 新增 `server/assets/items/woliu_scrolls.toml` — 涡流残卷物品模板
  - client `TechniqueScrollReadScreen.java` — 研读动画 + 学招通知
  - client `TechniqueObserveHud.java` — 观摩领悟 HUD
  - `TechniqueLearnedEvent` / `TechniqueScrollReadEvent` / `TechniqueMasteredEvent` — server event
  - `bong:technique/learned` / `bong:technique/scroll_read` / `bong:technique/mastered` — IPC channel
- **共享类型 / event**：
  - 复用 `VortexCastEvent`（观摩检测源）/ `KnownTechniques`（写入目标）/ `InsightEffect`（新增 variant）
  - 复用 `RecipeUnlockState` 模式（三渠道设计），但**不复用 craft unlock 代码**——战斗功法学招是独立系统（功法 ≠ 配方）
  - 新增 `TechniqueLearnedEvent { player, technique_id, source: LearnSource }` — 通知 agent / LifeRecord
- **跨仓库契约**：
  - server: `cultivation::technique_scroll` / `technique_observe` / `technique_mentor` / `technique_proficiency`
  - client: `bong:technique/learned` CustomPayload → 学招通知 HUD / `bong:technique/proficiency_up` → 熟练度提升通知
  - agent: `tiandao` 订阅 `bong:technique/learned` → narration（"某人于此处悟得涡流之法"）
- **worldview 锚点**：§五 涡流核心 + §九 残卷来源 + §六 顿悟 + §七 道伥/大能
- **qi_physics 锚点**：熟练度影响涡流 `stir_99_1` 的 `vortex_delta` 参数（worldview §五:547 "Δ 上限 +0.2"）→ 调用 `combat::woliu_v2::physics::stir_99_1(delta_override)`。不自建 physics 子系统
- **经脉依赖声明**：复用 v3 已声明的 `SkillMeridianDependencies::declare(woliu_*, vec![LU, HT])`；研读残卷时校验经脉未 SEVERED

---

## P0 — 涡流残卷获取闭环

### P0.1 KnownTechniques 涡流招式移除默认

当前 `KnownTechniques::default()` 将 33 招全部以 `proficiency: 0.5, active: true` 给出——这是 dev 占位。本阶段将 11 个 `woliu.*` 从 `TECHNIQUE_IDS` default 列表移除。其他流派暂保持 default（各自 vN+1 plan 处理）。

**改动**：`server/src/cultivation/known_techniques.rs`
- `KnownTechniques::default()` 改为返回空 `Vec`（全局，不只涡流）
- 新增 `#[cfg(feature = "dev-techniques")]` 模块：`KnownTechniques::dev_default()` 恢复全部 33 招（proficiency=0.5, active=true），`cargo run --features dev-techniques` 时启用
- 生产路径：玩家初始 0 招，所有招式必须通过获取机制学会
- `/technique add <id>` / `/technique reset_all` dev 命令不受影响（走 `DevCommand` 路径，不经 default）

**Cargo.toml 变更**：
```toml
[features]
dev-techniques = []
```

**测试**：
- `known_techniques::default_is_empty` — default 返回空 Vec
- `known_techniques::dev_default_has_all_33` — `#[cfg(feature = "dev-techniques")]` 时 dev_default 返回全部 33 招
- `cmd::dev::technique_add_woliu_works` — dev 命令仍能添加涡流招式

### P0.2 涡流残卷物品模板

新建 `server/assets/items/woliu_scrolls.toml`，定义涡流功法残卷：

```toml
# 涡流功法残卷。研读后学会对应招式。
# scroll_kind = "combat_technique" 标识战斗功法残卷（区别于 craft 配方残卷）。
# scroll_skill_id = 招式 ID。

[[item]]
id = "scroll_woliu_vortex"
name = "涡流残卷·绝灵涡流"
category = "scroll"
grid_w = 1
grid_h = 2
base_weight = 0.05
rarity = "uncommon"
spirit_quality_initial = 0.5
description = "泛黄残页，记载着如何在掌心强造微小负灵域的初始心法。字迹模糊，需静心研读。"
scroll_kind = "combat_technique"
scroll_skill_id = "woliu.vortex"

# ... 其余 10 招同结构，rarity 按品阶递增：
# 黄阶(vortex/hold/burst) = uncommon
# 玄阶(mouth/pull/vacuum_palm/vortex_shield) = rare
# 地阶(heart/vacuum_lock/vortex_resonance/turbulence_burst) = epic
```

共 11 个 item 模板。品阶 → rarity 映射：黄阶 `uncommon` / 玄阶 `rare` / 地阶 `epic`。

**测试**：
- `item_template::woliu_scrolls_load` — 11 个模板全部成功解析
- `item_template::woliu_scroll_skill_ids_valid` — 每个 `scroll_skill_id` 都能在 `TECHNIQUE_DEFINITIONS` 找到匹配

### P0.3 残卷研读机制

新建 `server/src/cultivation/technique_scroll.rs`：

```rust
pub struct TechniqueScrollReadEvent {
    pub player: Entity,
    pub technique_id: String,
    pub source_item: String,
}

pub struct TechniqueLearnedEvent {
    pub player: Entity,
    pub technique_id: String,
    pub source: LearnSource,
}

pub enum LearnSource {
    Scroll { item_id: String },
    Observe { observed_entity: Entity },
    Mentor { npc_entity: Entity },
    DyingMaster { npc_entity: Entity },
    DevCommand,
}

pub enum ScrollReadOutcome {
    Learned,
    AlreadyKnown,
    RealmTooLow { required: Realm, current: Realm },
    MeridianSevered { channel: MeridianId },
    MeridianMissing { channel: MeridianId },
    InvalidScroll,
}
```

**接线**：client inventory 右键物品时发送 `ClientRequestV1` payload（`kind: "technique_scroll_use"`，`item_slot: usize`）→ server `network::client_request_handler` 新增分支路由到 `technique_scroll::handle_scroll_use()`。复用现有 `ClientRequestV1` JSON payload 路径（同 craft scroll 的 `unlock_via_scroll` 走法），不新增 CustomPayload channel。

**研读流程**（inventory 内右键残卷）：
1. 校验 `scroll_kind == "combat_technique"` 且 `scroll_skill_id` 存在
2. 查 `TECHNIQUE_DEFINITIONS` 获取 `required_realm` + `required_meridians`
3. 校验玩家 `Realm >= required_realm`
4. 校验 `required_meridians` 均已开通且未 SEVERED
5. 校验 `KnownTechniques` 中无该招
6. 全部通过 → 消耗残卷 → `KnownTechniques.entries.push(KnownTechnique { id, proficiency: 0.0, active: true })` → emit `TechniqueLearnedEvent`
7. 失败 → 不消耗残卷 → inventory 内 tooltip 显示对应错误

注意 `proficiency: 0.0`（从零开始，不再是 dev 占位的 0.5）。

**研读视听**（右键即时 + 成功瞬间）：

- **研读成功（瞬间，inventory 内右键后触发）**：
  - 粒子：残卷碎裂成灰烬向上飘散（`BongSpriteParticle` ash 贴图 × 16 burst，lifetime 40 tick，速度向上 0.05 + 随机 spread 0.03，颜色 `#8B7355` → `#C4A94D` gradient）+ 玩家身周淡紫涡旋闪现（复用 `VortexSpiralPlayer` 0.5s 淡出，颜色 `#9B7ED8`）
  - 音效：`scroll_read_success.json`（`minecraft:entity.player.levelup`(pitch 1.2, volume 0.6) + `minecraft:block.portal.trigger`(pitch 2.0, volume 0.15, delay 5)）
  - HUD：屏幕中央金色大字"习得·{technique_display_name}"（`HudRenderLayer::OVERLAY`，颜色 `#C4A94D`，opacity 1.0 → 0.0 over 60 tick，字号 24，easing=ease_out_quad）
  - narration（scope=player, style=perception）：
    - "残卷化灰，掌心隐隐有凉意涌动——这便是涡流的起手。"
    - "墨字入眼，法理入心。你感到肺经末端多了一丝从未有过的牵引。"
    - "纸页碎裂的瞬间，你看见了——灵气的缝隙。"

- **研读失败**：
  - 境界不足：HUD toast 红色 "境界不足——需{required_realm}方可参悟此卷"，60 tick 淡出
  - 经脉未通：HUD toast 红色 "经脉未通——需先开通{meridian_name}"
  - 经脉断裂：HUD toast 红色 "经脉已损——{meridian_name}永久断裂，无法修习此法"
  - 已知：HUD toast 黄色 "你已通晓此法"

### P0.4 道伥 + 遗迹掉落注册

修改 `server/src/npc/loot.rs`：

现有道伥（`Daoxiang`）loot table 已有 `item.daoxiang.tattered_scroll`（10% 概率）。**改造方案**：将 `tattered_scroll` 拆为流派残卷池——命中 10% 后再内部 roll 流派（7 流派均分 + 品阶权重）。这样不改总掉率，只让残卷有具体功法含义。

- **道伥残卷池**（替换原 `item.daoxiang.tattered_scroll` 10%）：
  - 内部 roll：70% 黄阶 / 25% 玄阶 / 5% 空卷（`tattered_scroll_generic`，无功法，10 骨币卖价）
  - 黄/玄阶命中后再 roll 流派：当前只有涡流有 item template，其他 6 流派暂出 `tattered_scroll_generic` 占位（等各流派 plan 注册自己的残卷后替换）
  - 涡流命中率：10% × 70% × (1/7) = 1% 黄阶 / 10% × 25% × (1/7) = 0.36% 玄阶
  - 地阶不由普通道伥掉落（worldview §七:727 "破碎法宝" ≠ 完整地阶功法）

- **TSY 遗迹 loot**（`server/src/npc/tsy_hostile.rs` 已有 `blueprint_scroll_spec` / `inscription_scroll_spec` 字段）：
  - `tsy_zongmen_ruin` 浅层：黄阶涡流残卷 5%（宗门遗迹 = 功法残卷主产地）
  - `tsy_zongmen_ruin` 深层：玄阶涡流残卷 2%
  - `tsy_gaoshou_hermitage`：地阶涡流残卷 0.5%（高手隐居处 = 近代功法来源）

**概率设计哲学**：worldview §九:880 "极稀" = 单次 < 5%。道伥是主要来源但单流派命中率 ~1%，需要反复刷。TSY 遗迹更集中但进入成本高。

**测试**：
- `loot::daoyang_can_drop_woliu_scroll` — 道伥 loot table 包含涡流残卷
- `loot::daoyang_woliu_probabilities` — 黄阶 ≤ 3% / 玄阶 ≤ 1%
- `loot::tsy_ruin_woliu_scroll_tiers` — 浅层只出黄阶，深层出玄阶，隐居处出地阶
- `technique_scroll::read_scroll_success` — 满足条件研读成功 + 残卷消耗 + KnownTechniques 写入 + event emit
- `technique_scroll::read_scroll_realm_too_low` — 境界不足 → 不消耗 + 返回错误
- `technique_scroll::read_scroll_meridian_severed` — 经脉断裂 → 不消耗 + 返回错误
- `technique_scroll::read_scroll_already_known` — 已知 → 不消耗
- `technique_scroll::read_scroll_invalid` — scroll_kind 不是 combat_technique → reject
- `technique_scroll::proficiency_starts_at_zero` — 研读后 proficiency=0.0 非 0.5

---

## P1 — 观摩领悟 + NPC 传功路径

### P1.1 观摩领悟系统

新建 `server/src/cultivation/technique_observe.rs`：

**触发条件**（全部满足）：
1. 观摩者与施法者在 16 格内且有视线（ray cast 无遮挡）
2. 施法者成功释放涡流招式（`VortexCastEvent` emit）
3. 观摩者尚未习得该招式
4. 该招式品阶 ≤ 玄阶（地阶无法观摩领悟——worldview "算计型"流派的高阶功法需要理论基础）
5. 观摩者 `Realm >= required_realm`
6. 观摩者 `required_meridians` 均已开通且未 SEVERED

**领悟概率公式**：

```rust
fn observe_learn_chance(
    technique_grade: Grade,      // 黄阶/玄阶
    observer_color: &QiColor,    // 观摩者当前染色
    practice_log: &PracticeLog,  // 观摩者修炼履历
    insight_modifiers: &InsightModifiers, // 顿悟加成
) -> f64 {
    let base = match technique_grade {
        Grade::Yellow => 0.05,   // 黄阶 5% 基础
        Grade::Profound => 0.01, // 玄阶 1% 基础
        _ => 0.0,                // 地阶以上不可观摩
    };

    // 缜密色亲和加成（涡流 ↔ 缜密色同源）
    let color_bonus = if observer_color.dominant_kind() == Some(ColorKind::Intricate) {
        1.5  // +50%
    } else {
        1.0
    };

    // PracticeLog 涡流维度累积加成
    let practice_bonus = (practice_log.weight(ColorKind::Intricate) / 100.0).min(0.5) + 1.0;

    // 顿悟 insight modifier（"识规则" 等加成）
    let insight_bonus = 1.0 + insight_modifiers.observe_chance_bonus;

    (base * color_bonus * practice_bonus * insight_bonus).min(0.15) // 硬上限 15%
}
```

**冷却**：同一招式同一施法者，60s 内只判定一次（防刷）。

**领悟视听**：

- 粒子：观摩者头顶冒出一枚淡紫色"悟"字符（`BongSpriteParticle` glyph 贴图 `bong:textures/particle/insight_glyph.png`，lifetime 60 tick，缓慢上浮 0.01 block/tick + 旋转，颜色 `#9B7ED8` opacity 0.8 → 0.0，burst × 1）
- 音效：`technique_observe_insight.json`（`minecraft:entity.experience_orb.pickup`(pitch 0.6, volume 0.4) + `minecraft:block.amethyst_block.chime`(pitch 1.2, volume 0.2, delay 10)）
- HUD：屏幕下方金色 toast "你望着那掌心涡旋，隐约明白了什么——习得·{technique_display_name}"（`HudRenderLayer::TOAST`，颜色 `#C4A94D`，80 tick fade out）
- narration（scope=player, style=perception）：
  - "旁人掌中涡旋转瞬即逝，你却看到了灵气的走向——原来如此。"
  - "他人的涡流只是一闪，你的肺经却自行模仿了那个频率。"

### P1.2 NPC 传功路径

新建 `server/src/cultivation/technique_mentor.rs`：

**前置**：当前 `NpcArchetype` 无流派标签（只有 Zombie/Commoner/Rogue/Beast/Disciple/Daoxiang 等 9 种）。本阶段在 `NpcArchetype` 或 NPC entity 上新增 `combat_style_tags: Vec<String>` 可选字段（如 `["woliu", "zhenmai"]`），生成 NPC 时按区域/遭遇随机填入。不做独立 plan——只是给 NPC lifecycle 加一个 tag 字段，其他流派 plan 复用。

**触发场景**：散修 NPC（`NpcArchetype::Rogue` / `NpcArchetype::Disciple`）对话时，若该 NPC 的 `combat_style_tags` 包含 `"woliu"`，且关系度足够：

- **NPC 对话选项**："教我涡流之法"（需 NPC 好感度 ≥ 50 / 20 骨币 / 境界 ≥ Condense）
- **NPC 愿意教的条件**：
  - 玩家境界 ≥ 该招式 `required_realm`
  - 玩家经脉满足 `required_meridians`
  - NPC `combat_style_tags` 包含 `"woliu"`
  - NPC 好感度 ≥ 50（`social::RelationScore`）
- **NPC 可教范围**：仅黄阶 + 玄阶（NPC 也是散修，地阶功法他们自己都不会）
- **代价**：20 骨币（黄阶）/ 50 骨币（玄阶）+ NPC 好感度 -10（"传功耗元气"）
- **成功率**：100%（付了钱就教，NPC 不骗人——区别于垂死大能的风险）

**传功仪式视听**（5s channel）：

- 动画：NPC 伸掌贴玩家后背（`mentor_channel.json`，UPPER_BODY 100 tick loop），NPC 右臂前伸 pitch=-0.3rad / 玩家站立不动
- 粒子：NPC 手掌 → 玩家背部淡紫色真元丝线（`BongLineParticle` × 3 线条，NPC hand bone → player spine，lifetime 100 tick loop，颜色 `#9B7ED8` opacity 0.4，速度沿线方向 0.03 block/tick）
- 音效：`technique_mentor_channel.json`（`minecraft:block.beacon.ambient`(pitch 1.5, volume 0.08) loop + `minecraft:block.enchantment_table.use`(pitch 1.0, volume 0.1, delay 40)）
- 成功瞬间：复用 P0.3 研读成功的 HUD + 粒子（涡流闪现 + "习得"大字）
- narration（scope=player, style=dialogue）：
  - "他掌心贴上你背脊的一刻，你感到肺经被一股外力轻轻拨弄——像调弦。"
  - ""记住这个频率，"散修收手，"别练过头——涡流反噬不是说着玩的。""

**测试**：
- `technique_observe::observe_learn_chance_yellow` — 黄阶 base=0.05，缜密色 ×1.5
- `technique_observe::observe_learn_chance_profound` — 玄阶 base=0.01
- `technique_observe::observe_learn_chance_earth_zero` — 地阶 = 0
- `technique_observe::observe_learn_chance_cap` — 上限 0.15
- `technique_observe::observe_cooldown_60s` — 同招同人 60s 内不重复判定
- `technique_observe::observe_requires_line_of_sight` — 无视线不触发
- `technique_observe::observe_requires_realm` — 境界不足不触发
- `technique_mentor::mentor_dialog_option_appears` — NPC 有涡流标签 + 好感 ≥ 50 → 对话选项出现
- `technique_mentor::mentor_teaches_technique` — 付费 + 传功 → KnownTechniques 写入 + event emit
- `technique_mentor::mentor_cost_deducted` — 20 骨币扣除 + 好感 -10
- `technique_mentor::mentor_refuses_low_affinity` — 好感 < 50 → 拒绝
- `technique_mentor::mentor_refuses_earth_grade` — NPC 不教地阶
- `technique_mentor::mentor_refuses_severed_meridian` — 经脉断裂 → 拒绝 + 对话提示

---

## P2 — 垂死大能事件 + 熟练度成长

### P2.1 垂死大能随机事件

当前无通用 world_event 模块。在 `server/src/npc/` 下新建 `dying_master.rs`——作为 NPC 生成的特殊路径（垂死大能本质是一个特殊 NPC entity + 对话树 + 30s 倒计时 despawn），挂在 chunk 加载 system 上检测负灵域洞穴条件。不建独立 world_event 框架——本 plan 只做涡流相关的垂死大能，通用随机事件框架留给后续 plan。

**事件规格**（worldview §七:742 锚定）：

- **触发条件**：玩家进入负灵域（zone_qi < -0.3）洞穴区块 + 随机判定（每 chunk 首次进入 0.5%）
- **NPC 规格**：`NpcArchetype::DyingMaster`，化虚境外表，真元 < 5%（濒死态），位于洞穴深处
- **对话流程**：
  1. NPC 主动开口："救……给我回元丹……传你一门地阶功法……"
  2. 玩家选项：
     - A. 给丹（消耗 5 颗回元丹）→ NPC 恢复 → **50% 概率传功 + 50% 概率夺舍攻击**
     - B. 拒绝 → NPC 继续衰弱
     - C. 言语拖延（等他在负压下自然死亡，30s 倒计时）→ 死后舔包掉落地阶残卷（100%）
  - **正确解法**（worldview 锚定）：选 C 或布陷阱，等他死后拾取
- **传功（A 路径成功时）**：
  - 教一门地阶涡流招式（随机从 heart/vacuum_lock/vortex_resonance/turbulence_burst 中选）
  - 需玩家满足该招式 `required_realm` + 经脉——否则"传你也接不住"
- **夺舍（A 路径失败时）**：
  - NPC 真元暴涨 → 发起 PvE 战斗（化虚境 NPC，极难）
  - 击杀奖励 = 同样掉地阶残卷（但战斗成本极高）
- **舔包（C 路径 / 击杀后）**：
  - 掉落 1 张地阶涡流残卷（随机）+ 1-3 颗丹药残渣 + 1 个破碎法器

**视听**：

- **NPC 濒死外观**：使用 `tsy_hostile` 的道伥贴图变体（苍白皮肤 + 龟裂纹路），额外附加 `BongSpriteParticle` 身周灰色烟气（lifetime 40 tick，continuous 1/s，颜色 `#4A4A4A` opacity 0.3，半径 1 block 缓慢上飘）
- **对话时**：无粒子变化，仅 NPC 嘴部 animation 微动（`npc_speak_weak.json`，HEAD 20 tick loop，jaw pitch=0.05rad oscillate）
- **传功成功**：复用 P1.2 传功仪式视听（NPC 手贴背 + 真元丝线），但颜色更浓（`#7B5EA8` opacity 0.7）+ 传功结束后 NPC 倒地碎裂（`minecraft:entity.zombie.death`(pitch 0.5, volume 0.6)）
- **夺舍攻击**：NPC 眼睛突然亮红光（`BongSpriteParticle` 眼部 glow × 2，颜色 `#FF2200`，burst）+ `minecraft:entity.warden.emerge`(pitch 1.0, volume 0.8) + NPC 真元条瞬间回满 + 进入战斗
- **自然死亡（C 路径）**：NPC 身体逐渐透明（30s fade，opacity 1.0 → 0.0 linear）+ 最终碎裂成灰烬粒子（`BongSpriteParticle` ash × 32 burst + `minecraft:entity.generic.death`(pitch 0.3, volume 0.4)）+ 掉落物品散落

**narration**（scope=zone, style=narrative）：
- "洞穴深处，一具衣衫破碎的身影半靠岩壁。他还活着——但不会太久。"
- "他的真元已经被这片负灵域吸得见底，剩下的只够维持心跳。"
- 传功成功后："他把最后的真元灌入你的经脉，然后像一盏油尽的灯，灭了。"
- 夺舍时："他的眼睛突然亮了。那不是感激——是猎物落网的喜悦。"

### P2.2 熟练度成长系统

新建 `server/src/cultivation/technique_proficiency.rs`：

**proficiency 范围**：`0.0`（刚学会）→ `1.0`（精通 / 练满）

**成长公式**：

```rust
fn proficiency_gain(
    current: f32,
    source: ProficiencySource,
    color_match: bool,     // 缜密色 = true
    meridian_health: f32,  // 依赖经脉的平均 health
) -> f32 {
    let base = match source {
        ProficiencySource::CombatCast => 0.008,    // 实战施放
        ProficiencySource::PracticeSession => 0.003, // 修炼打坐
        ProficiencySource::BackfireSurvived => 0.015, // 反噬后存活（"吃亏长记性"）
    };

    // 颜色匹配加成
    let color_mul = if color_match { 1.5 } else { 1.0 };

    // 经脉健康度加成（经脉越健康，修炼越顺）
    let meridian_mul = 0.5 + meridian_health * 0.5; // 0.5-1.0 range

    // 递减收益（越高越难涨）
    let diminish = 1.0 - current * 0.8; // proficiency=0 → ×1.0, proficiency=0.9 → ×0.28

    (base * color_mul * meridian_mul * diminish).max(0.001) // 最低 0.001 保底
}
```

**关键时间节点**（预估，缜密色 + 全健康经脉）：
- 0.0 → 0.3（入门）：~40 次实战 cast（约 2-3h 战斗）
- 0.3 → 0.6（熟练）：~80 次实战 cast（约 5-6h）
- 0.6 → 0.9（精通）：~200 次实战 cast（约 15h）
- 0.9 → 1.0（练满）：~150 次实战 cast（约 10h，递减最陡）

对齐 worldview §A.5 "单流派 100h" 中的修炼分配。

**proficiency 效果缩放**（修改 `combat::woliu_v2::physics` + `combat::woliu_v2::backfire`）：

> **实地现状**：`backfire.rs`（73 行）和 `physics.rs`（135 行）目前**完全不读** `KnownTechnique.proficiency`——反噬概率只看 realm / qi 比值 / contamination / overflow，物理参数写死。本阶段在两个文件的核心函数签名中加入 `proficiency: f32` 参数，所有缩放通过该参数乘入。

| 属性 | proficiency=0.0 | proficiency=0.5 | proficiency=1.0 | 公式 |
|---|---|---|---|---|
| 反噬概率 | ×2.0（翻倍） | ×1.0（基线） | ×0.4（大幅降低） | `backfire_base * (2.0 - 1.6 * prof)` |
| 真元消耗 | ×1.3 | ×1.0 | ×0.85 | `qi_cost * (1.3 - 0.45 * prof)` |
| 涡流 Δ（vortex_delta） | 0.08（基线 80%） | 0.10（基线） | 0.12（+20%） | `0.08 + 0.04 * prof` |
| 吸引半径 | ×0.8 | ×1.0 | ×1.1 | `radius * (0.8 + 0.3 * prof)` |
| cast_ticks | ×1.2（更慢） | ×1.0 | ×0.9（更快） | `ticks * (1.2 - 0.3 * prof)` |

proficiency=1.0 时 `vortex_delta = 0.12` 即 worldview §五:547 "Δ 上限 +0.2" 的基座（+0.2 需天赋加成叠加，见 P3）。

**修炼 session**：

玩家蹲下 + 手持空手 + 已学涡流招式 + 非战斗状态 → 进入"修炼涡流"状态：
- 每 10s tick 一次 `ProficiencySource::PracticeSession`
- 消耗真元 2/10s（修炼也有成本）
- 有 0.5% 概率触发轻微反噬（proficiency < 0.3 时提高到 2%）——修炼也有风险
- 玩家移动 / 被攻击 / 真元 < 10% → 自动退出修炼
- **负灵域加速**：当前区域 zone_qi < 0 时，proficiency 成长 ×1.5（涡流本就是操控真空/负压的流派，在负灵域修炼更贴合物理直觉）。zone_qi < -0.5 时 ×2.0 但反噬概率也 ×2.0（深层负灵域风险收益并存）

**修炼视听**：
- 动画：复用 `stance_woliu.json`（涡流待机姿态 loop）
- 粒子：双掌间小型涡流旋转（复用 `VortexSpiralPlayer` 的 mini 模式，半径 0.3 block，颜色 `#9B7ED8` opacity 0.3）
- 音效：`minecraft:block.portal.ambient`(pitch 2.5, volume 0.03) loop（极轻的嗡鸣）

**练满事件**：

proficiency 到达 1.0 时 emit `TechniqueMasteredEvent`：
- 写入 `LifeRecord`（"某年某月某日，{player} 于{region} 练满{technique_name}"）
- 触发顿悟判定（与 P3 涡流天赋联动）
- narration（scope=zone, style=narrative）："此人掌心涡旋已如呼吸般自然——涡流一道，又多了一位通达者。"

**测试**：
- `technique_proficiency::gain_formula_combat` — 实战 cast proficiency 增长符合预期
- `technique_proficiency::gain_diminishing` — 高 proficiency 时增长率递减
- `technique_proficiency::color_match_bonus` — 缜密色 ×1.5
- `technique_proficiency::meridian_health_impact` — 经脉损伤降低增长
- `technique_proficiency::practice_session_gain` — 修炼 session 按 10s tick 成长
- `technique_proficiency::practice_session_qi_cost` — 修炼消耗真元 2/10s
- `technique_proficiency::practice_session_exits_on_move` — 移动退出修炼
- `technique_proficiency::backfire_chance_scales` — prof=0 反噬 ×2, prof=1 反噬 ×0.4
- `technique_proficiency::qi_cost_scales` — prof=0 消耗 ×1.3, prof=1 消耗 ×0.85
- `technique_proficiency::vortex_delta_scales` — prof=0 Δ=0.08, prof=1 Δ=0.12
- `technique_proficiency::mastered_event_at_1_0` — proficiency=1.0 时 emit TechniqueMasteredEvent
- `technique_proficiency::mastered_writes_life_record` — 练满写入 LifeRecord
- `dying_master::event_spawn_in_negative_zone` — 负灵域洞穴才触发
- `dying_master::event_probability_0_5_percent` — 0.5% 概率
- `dying_master::path_c_drop_earth_scroll` — 等死后 100% 掉地阶残卷
- `dying_master::path_a_50_50_split` — 给丹 50% 传功 / 50% 夺舍
- `dying_master::seize_body_triggers_combat` — 夺舍进入 PvE
- `dying_master::mentor_checks_realm` — 传功前校验境界

---

## P3 — 涡流天赋路线 + 全流程验收

### P3.1 涡流专属顿悟

新增 3 个 `InsightEffect` variant（`server/src/cultivation/insight.rs`）：

```rust
// F 流派类 — 涡流专属
VortexBackfireResist {
    mul: f64,  // 反噬概率乘数，如 0.5 = 反噬概率减半
},
VortexDeltaBonus {
    add: f64,  // vortex_delta 加成，如 +0.08 = Δ 从 0.12 → 0.20
},
VortexFlowSpeed {
    mul: f64,  // 涡流真元流速乘数，影响持涡/护体持续时间
},
```

**顿悟触发条件 + 选项**：

#### 顿悟一：「识规则」（首次练满任一涡流招式时触发）

```
[某门功法练满 — 涡流]
你第一次完整掌握了涡流之法。
掌心涡旋已如呼吸，你对灵气的理解又深了一层——

  A. 你看到了法则的骨架。涡流反噬减半——因为你知道何时该停。
     （VortexBackfireResist { mul: 0.5 }）
     代价：对非涡流招式的真元消耗 +8%（专精代价）

  B. 你看到了更深的空洞。涡流 Δ +0.08——你的真空场更强。
     （VortexDeltaBonus { add: 0.08 }）
     代价：涡流持续消耗 +15%（深层真空更耗元气）

  C. 你看到了流动本身。涡流持续时间 ×1.3——你的真元流更稳。
     （VortexFlowSpeed { mul: 1.3 }）
     代价：涡流冷却 +20%（稳流需要恢复期）
```

选项 B 选中时 `vortex_delta = 0.12 + 0.08 = 0.20`，精确对齐 worldview §五:547 "Δ 上限 +0.2"。

#### 顿悟二：「负压直觉」（涡流 proficiency ≥ 0.6 时首次反噬存活触发）

```
[首次反噬存活 — 涡流]
涡流反噬的痛还在手骨里跳。
但你活了下来，而且你看清了那一刻的错误——

  A. 你记住了断裂的瞬间。此后反噬时经脉损伤 -30%。
     （MeridianOverloadTolerance { id: MeridianId::LU, add: 0.03 }）
     代价：涡流启动延迟 +0.5s（你变谨慎了）

  B. 你学会了压制涡流。此后可主动中断涡流而不触发反噬。
     （UnlockPractice { name: "涡流中断".to_string() }）
     代价：中断后 5s 内无法再次施放涡流
```

#### 顿悟三：「涡心共鸣」（首次在 3 目标以上的战斗中成功释放涡流共振触发）

```
[群体涡流 — 涡心共鸣]
三道涡流同时旋转，汇聚成一个更深的涡心。
你感到了空间本身的振动——

  A. 涡流共振强度 ×1.4（每增加 1 目标的叠加系数从 +20% → +28%）
     代价：涡流共振真元消耗 ×1.3

  B. 涡流共振范围 +2 格（6 → 8 格球形）
     代价：涡流共振持续时间 -1s（4s → 3s）
```

### P3.2 涡流专属 generic talent

在 `server/assets/insight/generic_talents.json` 新增 3 条涡流天赋：

```json
{
  "id": "vortex_intricate_affinity",
  "category": "style",
  "color_affinity": ["intricate"],
  "alignment": "converge",
  "gain": {
    "stat": "vortex_backfire_resist",
    "op": "mul",
    "base_value": 0.85
  },
  "cost": {
    "stat": "opposite_color_penalty",
    "op": "add",
    "base_value": 0.10
  },
  "gain_flavor": "缜密色与涡流同源——反噬概率 -{gain_pct}%",
  "cost_flavor": "越精于涡流，越钝于蛮力——对立色效率 -{cost_pct}%"
},
{
  "id": "vortex_lung_heart_synergy",
  "category": "meridian",
  "color_affinity": ["intricate", "insidious"],
  "alignment": "converge",
  "gain": {
    "stat": "vortex_delta",
    "op": "add",
    "base_value": 0.02,
    "meridian_group": "arm_yin"
  },
  "cost": {
    "stat": "meridian_overload_risk",
    "op": "add",
    "base_value": 0.02,
    "meridian_group": "arm_yin"
  },
  "gain_flavor": "肺心二经协振——涡流 Δ +{gain_val}",
  "cost_flavor": "二经联动的代价——手三阴过载风险 +{cost_pct}%"
},
{
  "id": "vortex_flow_endurance",
  "category": "qi",
  "color_affinity": ["intricate"],
  "alignment": "converge",
  "gain": {
    "stat": "vortex_sustain_cost",
    "op": "mul",
    "base_value": 0.90
  },
  "cost": {
    "stat": "vortex_burst_damage",
    "op": "mul",
    "base_value": 0.92
  },
  "gain_flavor": "涡流持续消耗 -{gain_pct}%——持久博弈之道",
  "cost_flavor": "爆发力略减——紊流爆发伤害 -{cost_pct}%"
}
```

### P3.3 天赋 + proficiency + color 三维联动

worldview §五:546-548 锚定的完整公式：

```
实战涡流效率 = base_delta
  × proficiency_scale(prof)             // P2.2 熟练度缩放
  × color_scale(qi_color)               // 缜密色 → ×1.15 / 阴诡色 → ×1.05 / 其他 ×1.0
  × insight_bonus(顿悟选项)             // P3.1 顿悟加成
  × talent_bonus(generic_talent)        // P3.2 天赋条目
  × meridian_health_factor(LU, HT)     // 经脉健康度
```

最优 build（worldview §五:547 "涡流大师"）：
- 缜密色主色 + 任督二脉 + "识规则"顿悟(选 B: Δ+0.08) + `vortex_lung_heart_synergy` 天赋(Δ+0.02) + proficiency=1.0(Δ=0.12)
- 最终 Δ = 0.12 + 0.08 + 0.02 = 0.22 ≈ worldview "Δ 上限 +0.2"（从基线 0.10 起算 = +0.12，更精确）

最差 build（worldview §五:548 "涡流劣手"）：
- 沉重色 + 手三阳 + 爆脉天赋 + proficiency=0.0
- Δ = 0.08 × 1.0 = 0.08，反噬 ×2.0

### P3.4 全流程串接验收

**E2E 测试场景**（`server/src/cultivation/tests/woliu_lifecycle_e2e.rs`）：

1. `e2e_scroll_to_master` — 完整路径：
   - 玩家初始 0 涡流招式 → 获得涡流残卷 → 研读 → 学会 woliu.vortex → proficiency=0.0
   - 实战 cast 50 次 → proficiency ≈ 0.3（入门）
   - 继续 cast → proficiency=1.0 → TechniqueMasteredEvent
   - 顿悟触发 → 选"识规则"选项 B → vortex_delta 验证 +0.08

2. `e2e_observe_to_learn` — 观摩路径：
   - 玩家 A 释放 woliu.hold → 玩家 B 在 16 格内 → 概率判定 → 学会 woliu.hold

3. `e2e_dying_master_ambush` — 垂死大能陷阱路径：
   - 给丹 → 50% 夺舍 → 击杀 → 掉落地阶残卷

4. `e2e_dying_master_patience` — 垂死大能等死路径：
   - 选 C → 等 30s → NPC 死亡 → 舔包 → 地阶残卷

5. `e2e_proficiency_scaling_combat` — 熟练度影响战斗：
   - prof=0 cast → 验证 qi_cost ×1.3 + backfire_chance ×2.0
   - prof=1 cast → 验证 qi_cost ×0.85 + backfire_chance ×0.4 + delta=0.12

6. `e2e_talent_stacking` — 天赋叠加：
   - 缜密色 + 识规则(B) + lung_heart_synergy → delta = 0.22

**Dev 测试命令扩展**：
- `/technique add woliu.vortex` — 仍可用（LearnSource::DevCommand）
- `/technique proficiency woliu.vortex 1.0` — 直写 proficiency（已有）

**测试总量预估**：P0 ~10 / P1 ~12 / P2 ~14 / P3 ~8 + 6 E2E = **~50 测试**

---

## 跨阶段共用数据契约

| 类型 | 位置 | 说明 |
|---|---|---|
| `TechniqueLearnedEvent` | `cultivation::technique_scroll` | 通用学招事件，含 `LearnSource` 枚举 |
| `TechniqueMasteredEvent` | `cultivation::technique_proficiency` | 练满事件 |
| `LearnSource` | `cultivation::technique_scroll` | Scroll / Observe / Mentor / DyingMaster / DevCommand |
| `ProficiencySource` | `cultivation::technique_proficiency` | CombatCast / PracticeSession / BackfireSurvived |
| `ScrollReadOutcome` | `cultivation::technique_scroll` | Learned / AlreadyKnown / RealmTooLow / ... |
| `InsightEffect::VortexBackfireResist` | `cultivation::insight` | 涡流反噬抗性 |
| `InsightEffect::VortexDeltaBonus` | `cultivation::insight` | 涡流 Δ 加成 |
| `InsightEffect::VortexFlowSpeed` | `cultivation::insight` | 涡流流速加成 |
| `CH_TECHNIQUE_LEARNED` | `schema::channels` | `"bong:technique/learned"` |
| `CH_TECHNIQUE_MASTERED` | `schema::channels` | `"bong:technique/mastered"` |
| `CH_TECHNIQUE_PROFICIENCY_UP` | `schema::channels` | `"bong:technique/proficiency_up"` |

---

## 开放问题（已决策）

1. ~~KnownTechniques::default()~~ → **全局改空 + `#[cfg(feature = "dev-techniques")]` dev flag**。生产路径 0 招，`cargo run --features dev-techniques` 恢复全给。
2. ~~观摩品阶上限~~ → **黄阶+玄阶**。地阶必须残卷/传功/垂死大能。
3. ~~修炼地点~~ → **任何地方可练，负灵域加速**。zone_qi < 0 → ×1.5；zone_qi < -0.5 → ×2.0（反噬也 ×2.0）。
4. ~~垂死大能频率~~ → **0.5%**。每 chunk 首次进入负灵域洞穴时判定。
5. ~~prof=0 可用性~~ → **能放但很烂**。反噬 ×2 / 消耗 ×1.3 / Δ 仅 0.08。worldview "你用得不好" 而非 "不让你用"。
6. ~~残卷占位~~ → **generic 占位卷**。其他 6 流派暂出 `tattered_scroll_generic`（10 骨币卖价），等各流派 plan 替换。
7. ~~combat_style_tags~~ → **Entity component**。per-instance，散修 NPC 生成时随机分配 1-2 个流派标签。
