# Bong · plan-poison-trait-v1 · 骨架

「毒性真元」特性泛型路径——**任何流派**可通过吃毒丹累积 `PoisonToxicity` 特性，让暗器/阵法/爆脉拳等招式附毒。代价三条：① 寿命扣减（接 plan-lifespan-v1 ✅） ② 消化负荷（DigestionLoad，本 plan 新建**通用底盘**） ③ 经脉 MICRO_TEAR 累积（接 plan-meridian-severed-v1 ⏳）。**双层附毒**：① 吃丹长期 stat 修饰 ② 毒丹研磨为粉涂暗器瞬时单次 debuff（接 plan-craft-v1 ⏳）。**5 种初级毒丹 + 5 种对应毒粉**（§3.5 规格表，骨币 50→800 阶梯）。客户端补 **10 个 item icon**（gen-image transparent bg）+ **`bong:eat_food` 通用吃食物动画双视角**（第三人称 PlayerAnimator JSON + 第一人称 mixin override，参考 KosmX example）。worldview §五:480 + §五:525-535「毒（特性，泛型）vs 毒蛊（流派，专属）」边界正典物理化身，**不触发暴露 / 不影响信誉度 / 染色可洗**，与 plan-dugu-v2 ⬜ 严格隔离。

**世界观锚点**：`worldview.md §五:480 「毒性真元」特性正典（暗器附毒/阵法地雷加伤/爆脉拳带毒）`· `§五:525-535 毒 vs 毒蛊关键边界（谁能用/触发暴露/解毒/社会后果四列）`· `§六:618-625 染色物理（毒色走暗绿/紫，普通可洗，区别于阴诡色不可洗）`· `§四:280-307 经脉损伤 4 档（MICRO_TEAR/TORN/SEVERED）`· `§十二:1043 续命路径有代价`（毒丹路径是其反向 —— 短期增伤，长期透支寿元）

**library 锚点**：待补 `cultivation-XXXX 毒丹试药志`（poison-trait-v1 P3 配合写一篇散修吃毒丹的笔记类条目）+ 复用 `peoples-0007 散修百态`（试药者群像）

**前置依赖**：

- `plan-alchemy-v2` ✅ → side_effect_pool 已实装；五种初级毒丹注册为 alchemy 配方（火候简化档），各自带 `side_effect_tag = poison_dose_*` 特化
- `plan-craft-v1` ⏳ active → **毒丹研磨成毒粉**走 craft 通用手搓（`PoisonPowder` category），注册 5 种研磨配方（毒丹 ×1 → 对应毒粉 ×3）
- `plan-lifespan-v1` ✅ → `lifespan::deduct(amount, reason)` API 调用，reason: `PoisonOverload`
- `plan-meridian-severed-v1` ⏳ → `MeridianSeveredEvent`（4 档之一 MICRO_TEAR），等其 P1 ship；本 plan P0 待 meridian-severed P1 落 API 后接入
- `plan-multi-style-v1` ✅ → QiColor 染色累积（毒色）走已有机制
- `plan-cultivation-canonical-align-v1` ✅ → 经脉拓扑 + Realm 基础
- `plan-skill-v1` ✅ + `plan-hotbar-modify-v1` ✅ → 招式系统底盘
- `plan-alchemy-client-v1` ✅ → 服丹姿态动画基础；本 plan P3 扩为通用「吃食物」动画（第一人称 + 第三人称双视角）

**反向被依赖**（poison-trait 落地后给以下 plan 提供 hook）：

- `plan-anqi-v2` ⬜ → 双层接入：① 暗器命中时检查 caster `PoisonToxicity`（长期累积附毒）② 暗器**涂毒粉**消耗品接口（瞬时投放，单次命中 PoisonDebuff）
- `plan-zhenfa-v2` ⬜ → 阵法地雷节点检查布阵者 `PoisonToxicity`，>threshold 伤害加成；可选**预埋毒粉**（毒粉 ×N 加阵基础伤害）
- `plan-baomai-v3` ⬜ → 爆脉拳命中检查 caster `PoisonToxicity`，>threshold 破皮入毒
- `plan-craft-v1` ⏳ active → 注册 5 种**毒丹研磨**配方（PoisonPowder category）
- `plan-alchemy-v2` ✅ → 注册 5 种**初级毒丹**配方（火候简化档，side_effect_tag 特化）
- `plan-style-balance-v1` 🆕 → 数值校准（PoisonToxicity 转化为附毒 dmg 的曲线 + 毒粉 debuff 强度进矩阵）

---

## §0 与 plan-dugu-v2 的边界硬隔离（worldview §五:525-535 物理化身）

| 维度 | **本 plan（毒性真元特性）** | **plan-dugu-v2（毒蛊流派）** |
|---|---|---|
| 谁能用 | **任何流派**（包括毒蛊师可叠加） | 仅毒蛊师（专修毒蛊真元） |
| 累积方式 | **吃毒丹**（plan-alchemy-v2 产出） | **服毒草煎汤 + 自身真元淬炼**（自蕴） |
| 累积量级 | `PoisonToxicity ∈ [0, 100]`（服丹累积，可降） | 阴诡色 % ∈ [0, 90]（永久不可洗） |
| 触发暴露 | **否**（worldview §五:528） | **是**（每次主动招式 roll，被识破→全服追杀） |
| 中和方式 | **普通中和真元 + 蕴元丹**（worldview §五:528 普通解毒）| 专属解蛊药（毒蛊师专炼）+ 失败永久废经脉 |
| 形貌外观 | 重度服丹（>80）口齿微青，**停丹 30d 内消退** | 阴诡色 ≥ 60% 形貌异化**永久不可逆** |
| 社会反应 | 无 baseline 调整 | -50 baseline 永久（worldview §十一:962-970）|
| QiColor 染色 | 暗绿/紫色，**普通可洗**（worldview §六:631）| 阴诡色，**永久 lock**（dugu-v2 §0 自蕴） |
| 影响招式 | 给本就有的招式（暗器/阵法/爆脉拳）**附加毒效** | dugu-v2 五招独立（蚀针/自蕴/侵染/神识遮蔽/倒蚀）|
| 经脉代价 | MICRO_TEAR 累积（**可愈**，接 yidao 接经术）| 自蕴 ≥ 90% 触发自身经脉 SEVERED |
| 是否注册新招 | **否**（无新 SkillRegistry 招式，纯 stat hook）| 是（5 招新注册） |

**实装边界**：

- 毒蛊师**可以**同时走毒丹路径（叠加 PoisonToxicity 上限 100），**但**：阴诡色 lock 优先，毒色染色被阴诡色覆盖；毒蛊师吃毒丹的毒效 hook 只生效于"非 dugu 招式"（防止双重计算）
- 任何角色 PoisonToxicity 触发的招式附毒**不写 IdentityProfile**，**不 emit DuguRevealedEvent**（worldview §五:528 关键差异）
- DigestionLoad 是泛型机制，**不是毒丹专属**——未来 plan-food-v1 / plan-yangsheng-v1 也可复用此底盘（本 plan 落地时仅毒丹接入，但 component 设计为通用）

---

## §1 接入面 Checklist

- **进料**：
  - `alchemy::Pill { side_effect_tag: poison_dose_X, toxin_amount }`（plan-alchemy-v2 ✅ 产出毒丹）
  - `inventory::eat_pill(pill_id)` → 触发本 plan `consume_poison_pill` handler
  - `cultivation::Cultivation { qi_current, realm }`
  - `lifespan::Lifespan { current_years, max_years }`（plan-lifespan-v1 ✅）
  - `meridian::Meridians`（plan-cultivation-canonical-align-v1 ✅ 20 经脉 enum）

- **出料**：
  - `PoisonToxicity { level: f32, source_history: Vec<PoisonDoseRecord> }` component 写玩家
  - `DigestionLoad { current: f32, capacity: f32, decay_rate: f32 }` component（**新建底盘，非毒丹专属**）
  - emit `PoisonDoseEvent { player, dose_amount, side_effect_tag }`（吃毒丹时）
  - emit `PoisonOverdoseEvent { player, severity }`（超量服丹触发寿命扣减 + 经脉 MICRO_TEAR）
  - emit `lifespan::DeductEvent { reason: PoisonOverload, years }`（接 plan-lifespan-v1 已有 API）
  - emit `MeridianSeveredEvent { meridian: random, severity: MICRO_TEAR, source: PoisonAccum }`（接 plan-meridian-severed-v1 P1 待 ship）
  - **PoisonAttackHook**：注册 `combat::AttackModifier` trait impl，让暗器/阵法/爆脉拳命中时检查 caster `PoisonToxicity` 并附加毒 debuff（具体由反向被依赖 plan 调用）

- **共享类型 / event**：
  - 复用 `cultivation::QiColor` (plan-multi-style-v1 ✅) 累积毒色（暗绿/紫），**不扩字段**（区别于 dugu-v2 P1 加 `permanent_lock_mask`，本 plan 走可洗路径不需要）
  - 复用 `MeridianSeveredEvent`（meridian-severed-v1 通用底盘，本 plan 是 source 之一）
  - 复用 `lifespan::DeductEvent`（lifespan-v1 已实装）
  - **新建** `PoisonDoseEvent` / `PoisonOverdoseEvent` / `DigestionOverloadEvent`（DigestionLoad 是新底盘必须新建相关 event）

- **跨仓库契约**：
  - server: `cultivation::poison_trait::*` 主实装（component / handler / tick）+ `schema::poison_trait`
  - agent: `tiandao::poison_trait_runtime`（吃毒丹叙事 + 中毒形貌渐变叙事 + 服丹过量寿命扣减心理叙事 + 经脉 MICRO_TEAR 反噬叙事）
  - client: 3 HUD 组件（PoisonToxicityIndicator / DigestionLoadBar / LifespanWarningBlink）+ 2 粒子（POISON_PILL_EATEN_PUFF / POISON_BREATH_AURA）+ 1 音效 recipe（poison_pill_swallow）

- **worldview 锚点**：见头部

- **qi_physics 锚点**：
  - **不引入新真元类型**——「毒性真元」是真元上的**属性 tag**，非新流体（worldview §五:480 "让暗器附毒"是修饰，非新流）
  - 不调用 `qi_physics::ledger::QiTransfer`（吃毒丹本身是 inventory 行为，不涉及 zone qi 流动）
  - 不需要扩 `qi_physics` 常数表——`POISON_DECAY_RATE_PER_DAY`、`DIGESTION_DECAY_RATE_PER_HOUR` 等是**生理常数**（不是真元物理常数），归本 plan 私有
  - **若 P0 决策门拍板「毒性真元附毒招式命中时算 qi_physics ρ 修饰」**（即毒效附在 attack qi 上影响异体排斥率），则需扩 `qi_physics::collision` 加 poison ρ 修饰算子；默认推荐**不扩**（保持毒效=独立 dmg debuff，与 qi 物理解耦），见 §5 决策门 #4

---

## §2 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ⬜ | 决策门：§5 十问题收口（含 5 种毒丹设计 / 毒粉 vs PoisonToxicity 关系 / 吃食物动画范围）+ 数值表锁定 + 与 dugu-v2 边界文档化 + qi_physics 是否扩 poison ρ 决策 + meridian-severed-v1 P1 API 等齐 + 与 plan-craft-v1 维护者拍板 `PoisonPowder` category 收录方式 | 数值矩阵进 §3 / 边界 spec 进 §0 / qi_physics 决策记 §5 / 等 meridian-severed P1 ship + craft-v1 §5 #1 决策门 |
| **P1** ⬜ | server `cultivation::poison_trait::*` 主实装（PoisonToxicity / DigestionLoad component + consume_poison_pill handler + poison_decay_tick + digestion_decay_tick + lifespan/meridian event 接入 + 毒粉 PoisonPowderConsumedEvent + apply_powder_coating 接口）+ schema 定义 + ≥100 单测（涨 20 测覆盖 5 丹 + 5 粉差异化）| `cargo test cultivation::poison_trait` 全过 / `grep -rcE '#\[test\]' server/src/cultivation/poison_trait/` ≥ 100 / 守恒断言（PoisonToxicity 衰减曲线 + DigestionLoad capacity + 5 丹独有 side_effect_tag 触发）|
| **P2** ⬜ | 与 plan-alchemy-v2 ✅ 集成（注册 5 种初级毒丹 alchemy 配方 + side_effect_tag 特化）+ 与 plan-craft-v1 ⏳ 集成（注册 5 种毒丹研磨配方进 PoisonPowder category）+ lifespan/meridian event consumer 接入 + AttackModifier trait impl（双层：长期 stat 修饰 + 瞬时粉末涂抹）| `alchemy::pill::eat()` → PoisonToxicity 累积 e2e / `craft::start()` → 毒粉产出 e2e / lifespan 扣减实测 / meridian MICRO_TEAR 写入实测 / 涂粉命中 e2e |
| **P3** ⬜ | client 3 HUD（PoisonToxicityIndicator / DigestionLoadBar / LifespanWarningBlink）+ 2 粒子（POISON_PILL_EATEN_PUFF / POISON_BREATH_AURA）+ 1 音效 recipe（poison_pill_swallow）+ **10 个 item icon**（5 丹 + 5 粉，gen-image skill 调用，强制 transparent bg）+ **吃食物动画双视角**（第三人称 PlayerAnimator JSON `bong:eat_food` + gen_eat_food.py + 第一人称 mixin override，**参考 KosmX/fabricPlayerAnimatorExample + Theoness1/EatingAnimation 实装方式**）+ agent 吃毒丹/形貌渐变/寿命扣减心理/MICRO_TEAR 反噬 narration template | render_animation.py headless 验证 / WSLg 实跑双视角 / 10 个 icon transparent bg 检查 / narration-eval ✅ 4 类叙事过古意检测 |
| **P4** ⬜ | 给 plan-anqi-v2 / plan-zhenfa-v2 / plan-baomai-v3 提供 PoisonAttackHook 联调 demo（每个流派至少一招接入 PoisonToxicity 修饰 + 至少 plan-anqi-v2 接入毒粉涂抹消耗品接口）+ 与 plan-style-balance-v1 ρ/W 矩阵中 poison dmg 转化曲线对齐 | 三流派至少一招 e2e 实测毒效附加 + anqi-v2 涂粉命中 e2e / style-balance 矩阵更新 |
| **P5** ⬜ | telemetry 校准（PoisonToxicity 累积速率 / DigestionLoad 上限 / 寿命扣减比例 PVP 实测 / 5 丹用量分布 / 毒粉消耗速率）+ library 条目补（cultivation-XXXX 毒丹试药志）+ 与 plan-yidao-v1 接经术联调（吃毒丹累积 MICRO_TEAR → 求医 yidao NPC）| PVP 实测数据进 plan §6 进度日志 / library 条目通过 review-book |

**P0 决策门**：完成前 §5 七问题必须有答案，否则数值/边界/接入分裂。

---

## §3 数据契约（待 P0 决策门后定稿）

```
server/src/cultivation/poison_trait/
├── mod.rs                — Plugin 注册 + re-export + register_alchemy_hooks
├── components.rs         — PoisonToxicity { level: f32 ∈ [0, 100],
│                                            source_history: Vec<PoisonDoseRecord>,
│                                            last_dose_tick: u64 }
│                          DigestionLoad { current: f32 ∈ [0, capacity],
│                                          capacity: f32 (= 100 base, +realm 加成),
│                                          decay_rate: f32 (= 5%/h base) }
│                          PoisonDoseRecord { tick, dose_amount, side_effect_tag }
├── handlers.rs           — consume_poison_pill (alchemy::Pill 入口)
│                          calculate_overdose_severity (DigestionLoad 超 capacity 触发)
├── tick.rs               — poison_toxicity_decay_tick (1%/h base, 重度服丹 0.5%/h)
│                          digestion_load_decay_tick (5%/h)
│                          overdose_lifespan_meridian_tick (服丹累积超阈值时扣寿命 + emit MICRO_TEAR)
├── attack_hook.rs        — impl combat::AttackModifier for PoisonToxicityModifier
│                          (caster.poison_toxicity > threshold → attack 附 PoisonDebuff)
└── events.rs             — PoisonDoseEvent / PoisonOverdoseEvent /
                            DigestionOverloadEvent /
                            (复用 lifespan::DeductEvent + MeridianSeveredEvent)

server/src/schema/poison_trait.rs  — IPC schema (PoisonDoseEvent + PoisonOverdoseEvent payload)

agent/packages/schema/src/poison_trait.ts  — TypeBox 对齐
agent/packages/tiandao/src/poison_trait_runtime.ts
                                  — 吃毒丹叙事 +
                                    中毒形貌渐变叙事（口齿微青/皮色蜡黄）+
                                    服丹过量寿命扣减心理叙事 +
                                    经脉 MICRO_TEAR 反噬叙事

client/src/main/java/.../cultivation/poison_trait/
├── PoisonToxicityIndicator.java   — HUD 毒性 % 显示（颜色随等级渐深）
├── DigestionLoadBar.java          — HUD 消化负荷条（接近 capacity 时闪烁）
├── LifespanWarningBlink.java      — HUD 寿命扣减预警（超量服丹瞬间闪 1s）
├── PoisonPillEatenParticle.java   — POISON_PILL_EATEN_PUFF 吃丹瞬间一缕暗绿
└── PoisonBreathAuraParticle.java  — POISON_BREATH_AURA 重度服丹（>80）口部常驻微毒气

client/src/main/resources/assets/bong/
├── audio_recipes/poison_pill_swallow.json
└── (无新动画 — 复用 alchemy 已有"服丹"动画 priority 200)
```

**combat::AttackModifier trait 接入示例**（给反向被依赖 plan 用）：

```rust
// 反向被依赖 plan（如 plan-anqi-v2）调用范例：
fn anqi_dart_resolve(caster: Entity, target: Entity, ...) {
    let base_dmg = ...;

    // ① 长期累积路径：caster.PoisonToxicity 修饰
    let final_dmg = poison_trait::apply_modifier(caster, base_dmg, AttackKind::Anqi);

    // ② 瞬时投放路径：检查暗器是否涂毒粉（粉末从 inventory 消耗 1 份）
    let final_dmg = poison_trait::apply_powder_coating(caster, item_in_hand, final_dmg);
    // 涂粉时 emit PoisonPowderConsumedEvent，消耗 1 份对应毒粉
    ...
}
```

---

## §3.5 五种初级毒丹 + 毒粉规格表

> **设计意图**：低境（醒灵-引气）友好的毒丹路径起点，差异化轴 = `PoisonToxicity 累积量 × DigestionLoad 占用 × 寿命扣 × MICRO_TEAR 概率 × 独有 side_effect_tag`。每丹研磨为对应粉（craft tab 30s in-game），粉用于暗器/阵法瞬时附毒，区别于丹本身的长期累积路径。

### 毒丹（alchemy 注册，side_effect_pool 特化）

| ItemId | 名称 | 主材料 | PoisonToxicity | DigestionLoad | 寿命扣 | MICRO_TEAR | 独有 side_effect_tag | 骨币锚定 |
|---|---|---|---|---|---|---|---|---|
| `PoisonPill::WuSuiSanXin` | **乌髓散心丹** | 妖兽乌髓汁 + 散心草 | +5 | +20 | 0 年 | 0% | `qi_focus_drift_2h`（cast 准头 -5% × 2h） | 50 骨币 |
| `PoisonPill::ChiTuoZhiSui` | **赤陀蜘髓丹** | 蜘蛛骨髓 + 赤陀粉（赤髓草加工） | +8 | +25 | -1 年 | 0% | `rage_burst_30min`（atk +5% / spd -10% × 30min） | 100 骨币 |
| `PoisonPill::QingLinManTuo` | **青鳞曼陀丹** | 青鳞蜥皮 + 曼陀罗 | +10 | +35 | -2 年 | 2% | `hallucin_tint_6h`（HUD 偶发幻象 × 6h） | 200 骨币 |
| `PoisonPill::TieFuSheDan` | **铁腹蛇胆丹** | 蛇胆 + 铁腹砂 | +12 | +45 | -3 年 | 5% | `digest_lock_6h`（DigestionLoad 衰减半速 × 6h，叠丹困难） | 350 骨币 |
| `PoisonPill::FuXinXuanGui` | **腐心玄龟丹** | 玄龟壳 + 腐心藻 | +15 | +55 | -5 年（首次额外） | 10% | `toxicity_tier_unlock`（永久解锁 PoisonToxicity 转化曲线 +1 档：附毒 dmg 上限 +20%）| 800 骨币 |

**alchemy 注册示例**（plan-alchemy-v2 ✅ 配方框架）：

```rust
// server/src/cultivation/poison_trait/recipes.rs
pub fn register_alchemy_recipes(registry: &mut alchemy::RecipeRegistry) {
    registry.register(alchemy::Recipe {
        id: AlchemyRecipeId::new("poison_trait.wu_sui_san_xin"),
        category: alchemy::Category::PoisonPill,
        materials: vec![
            (ItemId::WuSuiYouseSap, 2),
            (ItemId::SanXinHerb, 3),
        ],
        fire_pattern: alchemy::FirePattern::Simple,  // 简化档，无复杂火候
        side_effect_tag: SideEffectTag::PoisonDoseLow,
        unique_tag: SideEffectTag::QiFocusDrift2h,
        output: (ItemId::PoisonPill_WuSuiSanXin, 1),
        ...
    });
    // ... 其余 4 种类似
}
```

### 毒粉（craft 注册，毒丹研磨产物）

| ItemId | 名称 | 来源（craft 配方） | PoisonDebuff（涂暗器命中时） | 副作用（victim） |
|---|---|---|---|---|
| `PoisonPowder::WuSuiSanXin` | **乌髓散心粉** | 乌髓散心丹 ×1 → 乌髓散心粉 ×3 | mild: 2 dmg/s × 3s | — |
| `PoisonPowder::ChiTuoZhiSui` | **赤陀蜘髓粉** | 赤陀蜘髓丹 ×1 → 赤陀蜘髓粉 ×3 | mild: 2 dmg/s × 5s | — |
| `PoisonPowder::QingLinManTuo` | **青鳞曼陀粉** | 青鳞曼陀丹 ×1 → 青鳞曼陀粉 ×3 | moderate: 4 dmg/s × 5s | victim 1s 视觉模糊 |
| `PoisonPowder::TieFuSheDan` | **铁腹蛇胆粉** | 铁腹蛇胆丹 ×1 → 铁腹蛇胆粉 ×3 | moderate: 5 dmg/s × 6s | — |
| `PoisonPowder::FuXinXuanGui` | **腐心玄龟粉** | 腐心玄龟丹 ×1 → 腐心玄龟粉 ×3 | severe: 8 dmg/s × 8s | 5% 概率 victim MICRO_TEAR（随机经脉） |

**craft 注册示例**（plan-craft-v1 ⏳ 配方框架）：

```rust
// server/src/cultivation/poison_trait/recipes.rs
pub fn register_craft_recipes(registry: &mut craft::CraftRegistry) {
    registry.register(craft::CraftRecipe {
        id: craft::RecipeId::new("poison_trait.grind.wu_sui_san_xin"),
        category: craft::CraftCategory::PoisonPowder,  // 🆕 新增类别（plan-craft-v1 §5 #1 决策门补一类）
        materials: vec![(ItemId::PoisonPill_WuSuiSanXin, 1)],
        qi_cost: 2.0,                  // 微量真元淬炼
        time_ticks: 30 * 20,           // 30s in-game
        output: (ItemId::PoisonPowder_WuSuiSanXin, 3),
        requirements: craft::CraftRequirements {
            realm_min: None,
            qi_color_min: None,
            skill_lv_min: None,
        },
        unlock_sources: vec![
            craft::UnlockSource::Scroll { item_id: ItemId::Scroll_PoisonGrind },
            craft::UnlockSource::Mentor { npc_archetype: "alchemist_quirk".into() },
        ],
    });
    // ... 其余 4 种类似
}
```

> **注**：`craft::CraftCategory::PoisonPowder` 是 plan-craft-v1 §5 #1 决策门「6 类够吗」需补的第 7 类（或归入 `Misc`）—— 本 plan P0 阶段联动 plan-craft-v1 维护者拍板。

### 毒丹研磨 vs 直接服丹的策略博弈

| 路径 | 用途 | 代价 | 适合谁 |
|---|---|---|---|
| **吃毒丹** | 长期 PoisonToxicity 累积 → 招式整体附毒（修饰所有暗器/阵法/爆脉拳） | 寿命/消化/MICRO_TEAR | 走"毒性流派路线"的玩家 |
| **磨毒粉** | 单次涂抹瞬时附毒（具体命中那一下） | 1 份粉 = 1 次命中（消耗品）| 偶尔需要附毒的非毒系玩家 / 暗杀类一锤子买卖 |

worldview 锚：§五:480 "毒性真元让暗器附毒"覆盖两条路径——**长期 stat 修饰**（吃丹）+ **瞬时载体涂抹**（毒粉）。两者**可叠加**（PoisonToxicity 修饰 + 毒粉 debuff 共存命中），但在 dmg 转化曲线上避免双重计算（毒粉为加性，PoisonToxicity 为乘性，不冲突）。

---

## §4 客户端新建资产

| 类别 | ID | 来源 | 优先级 | 备注 |
|---|---|---|---|---|
| HUD | `PoisonToxicityIndicator` | 新建 | P3 | 角色 sidebar：当前 PoisonToxicity % + 等级标签（轻度<30 / 中度 30-70 / 重度>70）|
| HUD | `DigestionLoadBar` | 新建 | P3 | 角色 sidebar：消化负荷条 + capacity 标尺；接近 capacity 80% 时闪烁警告 |
| HUD | `LifespanWarningBlink` | 新建 | P3 | 寿命扣减瞬间在屏幕中央闪 1s「-X 年寿元（毒丹反噬）」|
| 粒子 | `POISON_PILL_EATEN_PUFF` ParticleType + Player | 新建 | P3 | 吃丹瞬间一缕暗绿从口部冒出 |
| 粒子 | `POISON_BREATH_AURA` ParticleType + Player | 新建 | P3 | 重度服丹（PoisonToxicity > 80）口部常驻微毒气，半径 1 格（区别于 dugu 自蕴气息 5 格规模）|
| 音效 | `poison_pill_swallow` | recipe 新建 | P3 | layers: `[{ sound: "entity.witch.drink", pitch: 1.1, volume: 0.4 }, { sound: "block.fire.extinguish", pitch: 0.9, volume: 0.2, delay_ticks: 3 }]`（吞丹声 + 后味苦涩）|

### 物品 icon（gen-image skill `style=item`，transparent background 强制要求）

| ItemId | icon ID | 提示词锚点 | 优先级 |
|---|---|---|---|
| `PoisonPill::WuSuiSanXin` | `bong:item/poison_pill_wu_sui_san_xin` | 暗灰底丹 + 灰白纹，温和外观 | P3 |
| `PoisonPill::ChiTuoZhiSui` | `bong:item/poison_pill_chi_tuo_zhi_sui` | 赤红丹 + 蛛丝纹 | P3 |
| `PoisonPill::QingLinManTuo` | `bong:item/poison_pill_qing_lin_man_tuo` | 青绿丹 + 鳞片光泽 | P3 |
| `PoisonPill::TieFuSheDan` | `bong:item/poison_pill_tie_fu_she_dan` | 黑铁色丹 + 蛇胆青斑 | P3 |
| `PoisonPill::FuXinXuanGui` | `bong:item/poison_pill_fu_xin_xuan_gui` | 墨黑丹 + 龟甲纹（最重档） | P3 |
| `PoisonPowder::WuSuiSanXin` | `bong:item/poison_powder_wu_sui_san_xin` | 灰白细粉装小瓷瓶 | P3 |
| `PoisonPowder::ChiTuoZhiSui` | `bong:item/poison_powder_chi_tuo_zhi_sui` | 赤红粉装小瓷瓶 | P3 |
| `PoisonPowder::QingLinManTuo` | `bong:item/poison_powder_qing_lin_man_tuo` | 青绿粉装小瓷瓶 | P3 |
| `PoisonPowder::TieFuSheDan` | `bong:item/poison_powder_tie_fu_she_dan` | 黑铁色粉装小瓷瓶 | P3 |
| `PoisonPowder::FuXinXuanGui` | `bong:item/poison_powder_fu_xin_xuan_gui` | 墨黑粉装小瓷瓶 | P3 |

**生成命令规格**（P3 阶段调 gen-image skill）：

```
/gen-image item <丹/粉中文名> + transparent bg + 末法残土风格 + 16x16/32x32 icon resolution
（每个 ItemId 单独跑一遍，确认 transparent bg 生效）
```

### 吃食物动画（第三人称 + 第一人称，**通用底盘**非毒丹专属）

> **设计意图**：当前 21 个 PlayerAnimator JSON 无 eat/drink 类，本 plan 借机立通用底盘，未来 alchemy 服丹 / 食物饱腹 / 灵酒灵茶 等都可复用。

| 类别 | ID | 来源 | 优先级 | 备注 |
|---|---|---|---|---|
| 第三人称动画 | `bong:eat_food` | 新建 PlayerAnimator JSON | P3 | 端碗/掰丹送嘴 → 仰脖 → 吞咽，priority 200（姿态层），duration 40 ticks（2s）|
| 第三人称 generator | `gen_eat_food.py` | `client/tools/` 新建 | P3 | 沿用 21 个已有 generator 模式（`anim_common.py` 复用 + render_animation.py headless 验证）|
| 第一人称动画 | hand swing override | Java mixin | P3 | 替换 vanilla `ItemUseAnimation.EAT/DRINK`：手部抬起 + item model 贴口部 + 微抖 |

**网络参考代码**（P3 实装时按优先级查阅）：

1. **`KosmX/fabricPlayerAnimatorExample`** —— 主要参考。完整 PlayerAnimator JSON 框架 + Fabric 1.20.1 API 调用模式。**关键文件**：`minecraft/fabric/src/testmod/resources/assets/animatorTestmod/player_animation/*.json` 看 JSON schema，`AnimationTriggerExample.java` 看播放触发。**适配难度：中**（需自己写 eat/drink 骨架，但 schema 直接照搬）

2. **`Theoness1/EatingAnimation`** —— 客户端事件触发逻辑参考。**关键文件**：`src/main/java/com/theone/eatinganimationid/`（如何捕获 `LivingEntity.eatFood()` / `ClientPlayerInteractionManager` 食物使用事件）。**适配难度：低**（仅事件捕获逻辑可借鉴，sprite 动画格式不直接复用）

3. **`Fuzss/betteranimationscollection`** —— ⚠️ 1.20.1 仅维护，**跳过**。代码复杂度高，反向工程成本不值

**实装顺序（P3 内部）**：

```
a. 第三人称：复制 client/tools/gen_meditate_sit.py → gen_eat_food.py
   按 KosmX 示例 schema 调整 keyframes（端碗→仰脖→吞咽 3 阶段）
   render_animation.py headless 迭代姿态（用户记忆 reference_animation_render_tool.md）
   注意 PlayerAnimator 4 大坑（feedback_playeranimator_gotchas.md）：
     - 循环单帧衰减到 defaultValue
     - body 走 MatrixStack 非 updatePart
     - bend 需 bendy-lib 否则静默 no-op
b. 第一人称：mixin 替换 ItemUseAnimation 渲染分支，参考 KosmX example 的 mixin 入口
c. 触发集成：alchemy::pill::eat() / inventory::consume_food() 路径调 BongAnimationRegistry.play("eat_food")
```

**复用清单**：
- 服丹姿态：plan-alchemy-client-v1 ✅ 已存"服丹"动画（priority 200）—— 本 plan 不替换，**仅作参考姿态点位**；新动画 `eat_food` 是更通用版本
- QiColor 染色渲染：复用 plan-multi-style-v1 ✅ 暗绿/紫染色（**不**走 dugu 阴诡色路径）
- 经脉受损可视化：复用 plan-meridian-severed-v1 ⏳ inspect 经脉图 MICRO_TEAR 标识

---

## §4.5 P1 测试矩阵（饱和化测试）

下限 **100 单测**：

| 模块 | 测试组 | 下限 |
|---|---|---|
| `consume_poison_pill` | 3 档剂量（low/mid/high）× PoisonToxicity 累积 + DigestionLoad 累积 + 边界（满 capacity 触发 overdose）+ 跨境界差异 | 18 |
| `five_pill_unique_side_effects` | 5 种初级毒丹各自独有 side_effect_tag 触发（QiFocusDrift / RageBurst / HallucinTint / DigestLock / ToxicityTierUnlock）+ 持续时间 + 失效清除 | 15 |
| `apply_powder_coating` | 5 种毒粉各自 PoisonDebuff 强度 × 命中触发 PoisonPowderConsumedEvent + 库存消耗 1 份 + 涂粉与 PoisonToxicity 叠加（加性+乘性公式）+ 无粉时 noop | 13 |
| `poison_toxicity_decay_tick` | 1%/h base 衰减曲线 + 重度（>70）减半为 0.5%/h + 累积期间不衰减（last_dose_tick < 1h）+ 衰减到 0 不溢出 | 12 |
| `digestion_load_decay_tick` | 5%/h base 衰减 + 满 capacity 触发 DigestionOverloadEvent + 衰减到 0 不溢出 | 8 |
| `calculate_overdose_severity` | 三档 severity（mild/moderate/severe）× 寿命扣减 emit + 经脉 MICRO_TEAR 概率 | 12 |
| `attack_hook::apply_modifier` | 三档 PoisonToxicity（<30 不加毒/30-70 加 mild debuff/>70 加 severe debuff）× 三种 AttackKind | 12 |
| `dugu_boundary_exclusion` | 毒蛊师并发吃毒丹时阴诡色优先 lock 染色 + dugu 招式不双重计算毒效 + 不写 DuguRevealedEvent | 8 |
| `lifespan_meridian_event_emission` | 寿命 DeductEvent payload 校验（reason=PoisonOverload）+ MeridianSeveredEvent 严重度=MICRO_TEAR + 随机经脉抽取分布 | 10 |

**P1 验收**：`grep -rcE '#\[test\]' server/src/cultivation/poison_trait/` ≥ 100。
守恒断言：PoisonToxicity 衰减后总值不应超 100（上限 clamp）；DigestionLoad 衰减后不应负值；寿命扣减总值应等于所有 PoisonOverdoseEvent payload 累加（账本一致性）。

---

## §5 开放问题 / 决策门（P0 启动前必须收口）

### #1 PoisonToxicity 累积曲线

毒丹三档（low/mid/high）服食一次累积量？

- **A**：low +5 / mid +12 / high +25（当前默认推荐）
- **B**：low +3 / mid +8 / high +20（更慢节奏，要更多 dose 才能达到附毒阈值）
- **C**：low +8 / mid +20 / high +40（更快爽感，但 overdose 风险翻倍）

**默认推荐 A** —— 配合衰减 1%/h，单次 high 后 25h 自然降回 0；累积达到附毒阈值（30+）需要持续服食 ≥3 dose。

### #2 DigestionLoad 容量与超量触发

`DigestionLoad.capacity` 默认值 + 境界加成？

- **A**：base 100 + (realm_idx × 20)（醒灵 100 / 化虚 200，每境界 +20，符合"修为越高消化越强"直觉）
- **B**：固定 base 100 不随境界（低境玩家更受消化限制）
- **C**：base 100 + (lifespan.max_years / 50)（按寿元上限隐式加成）

**默认推荐 A**。境界与消化能力的 scaling 是常见网文设定（"练气期日食一斤丹，化虚期日食百斤"夸张化）。

### #3 寿命扣减阶梯

PoisonOverdoseEvent severity → 寿命扣减年数？

- **A**：mild -1 年 / moderate -5 年 / severe -20 年
- **B**：mild 0.1 年 / moderate -1 年 / severe -5 年（更温和，毒丹是日常路径不是孤注）
- **C**：按 PoisonToxicity 当前值线性 -X 年（X = level / 5）

**默认推荐 B** —— 毒丹路径设计上是"长期透支寿元换短期增伤"，不应该一次 overdose 就扣 20 年（那级别的代价应该留给毒蛊师 dugu-v2 倒蚀）。B 配合 worldview §十二:1043 续命路径有代价，是其反向 mirror。

### #4 经脉 MICRO_TEAR 触发条件

PoisonOverdoseEvent severity → MICRO_TEAR 抽取经脉概率？

- **A**：mild 0% / moderate 10% / severe 30%（保守，重度才有概率）
- **B**：mild 5% / moderate 20% / severe 50%（激进，频繁 overdose 玩家经脉很快受损）
- **C**：累积 PoisonToxicity > 80 时每小时 1% 概率自动 MICRO_TEAR（独立于 overdose 事件，常驻风险）

**默认推荐 A**。worldview §四:280-307 MICRO_TEAR 是 4 档中最轻，但累积多档转 TORN 后才严重。本 plan 应给玩家足够空间走"日常服丹不出大事，过量才反噬"的节奏。等 P5 telemetry 校准。

### #5 附毒 dmg 转化曲线

`PoisonToxicity` → 招式附毒 debuff 强度公式？

- **A**：threshold gate（<30 无效 / 30-70 mild 2dmg/s × 5s / >70 severe 5dmg/s × 8s）—— 简单清晰
- **B**：线性曲线（debuff_dmg = max(0, (level - 30) × 0.1) dmg/s × 5s）—— 平滑过渡
- **C**：分段线性（30-50 慢 / 50-70 加速 / 70-100 平稳，符合"中毒到瓶颈"直觉）

**默认推荐 A**。简单 gate 易理解 + telemetry 校准方便（只需调三档常数）。等 plan-style-balance-v1 联调时若需更平滑再切 B。

### #6 与 dugu-v2 阴诡色 lock 冲突处理

毒蛊师（已有阴诡色 ≥ X%）并发吃毒丹时，毒色染色（暗绿/紫）应当如何处理？

- **A**：阴诡色完全 override，毒丹的染色累积**不写入 QiColor**（毒蛊师吃毒丹只 PoisonToxicity 累积，颜色保持阴诡色）—— 推荐
- **B**：双色叠加（QiColor 同时记录两种色，inspect 时显示主色 = 占比高者）—— 视觉混乱
- **C**：阴诡色 ≥ 60% 时拒绝吃毒丹（hard reject，HUD「自蕴尚有，丹毒入腹必紊」）—— 过度限制

**默认推荐 A**。worldview §六:631 阴诡色不可洗 + §六:618 染色物理「主色优先」，A 最自然。dugu-v2 §0 自蕴 lock 已规定阴诡色 lock，本 plan 兼容即可。

### #7 是否扩 qi_physics 加 poison ρ 修饰算子

「毒性真元」附毒招式命中时，是否影响 qi_physics 异体排斥率（ρ）？

- **A**：**不扩** —— 毒效=独立 dmg debuff，与 qi 物理解耦；attack 的 ρ 仍按 caster 流派算（worldview §六 7 流派 ρ 矩阵）。推荐
- **B**：**扩** —— 在 plan-qi-physics-v1 加 `POISON_RHO_MODIFIER ∈ [-0.05, +0.05]`，让附毒招式的 ρ 微降（更易混入宿主），需先扩 qi_physics 再 import

**默认推荐 A**。worldview §五:480 "毒性真元让暗器附毒"是字面"招式上贴毒"，不是真元物理修饰。B 扩 qi_physics 会拉长 P0 决策门 + 增加 qi_physics 测试矩阵负担。若 P5 telemetry 实测附毒招式过弱（玩家不愿吃毒丹），再考虑 B。

### #8 五种初级毒丹的命名 + 数值差异化是否合理

§3.5 表已草拟（乌髓散心 / 赤陀蜘髓 / 青鳞曼陀 / 铁腹蛇胆 / 腐心玄龟），骨币 50→800 阶梯，PoisonToxicity 累积 +5→+15。

- **A**：保留（默认推荐——命名末法残土风 + 数值阶梯线性，5 档差异清晰）
- **B**：缩到 3 档（轻/中/重，对应 +5/+10/+15）—— 简化注册成本但失去阶梯差异
- **C**：扩到 7 档 —— 跟境界 6 档对齐，但首版过度复杂
- **D**：保留 5 档但**重命名**（用户审美调整）

**默认推 A**。5 档阶梯既给玩家选择空间又不过度复杂，命名风格与 worldview 末法残土锚定（`peoples-0007 散修百态`）一致。如果用户对具体命名有意见走 D。

### #9 毒粉与 PoisonToxicity 的双层设计是否合理

§3.5 提出双层：① 吃毒丹累积 PoisonToxicity（长期 stat 修饰）② 毒丹研磨为粉涂暗器（瞬时单次 debuff）。

- **A**：保留双层（默认推荐）—— 给非毒系玩家临时附毒选项 + 给毒系玩家累积主路径
- **B**：仅留累积路径（吃毒丹 → PoisonToxicity → 招式自动附毒），**砍毒粉系统** —— 简化但减少玩法广度
- **C**：仅留毒粉路径（暗器涂粉），**砍 PoisonToxicity 累积** —— 但跟"毒修"主题脱钩（worldview §五:480 是 stat 修饰主导）
- **D**：保留双层但**毒粉效果不与 PoisonToxicity 叠加**（互斥，玩家选一种）—— 防止双重伤害堆叠

**默认推 A**，叠加规则定为"毒粉为加性 + PoisonToxicity 为乘性"避免双重计算。若 P5 telemetry 发现叠加 OP 再切 D。

### #10 吃食物动画的范围

§4 已声明 `bong:eat_food` 是**通用底盘**，但当前仅本 plan 用于毒丹。

- **A**：通用底盘只供本 plan 用（其他系统未来按需接入）—— 默认推荐，保持本 plan 范围
- **B**：本 plan 直接迁现有 plan-alchemy-client-v1 ✅ 服丹动画（重命名为 eat_food），让其他系统都用统一动画 —— 改动现有 active code，需 alchemy 维护者协调
- **C**：本 plan 不立通用，仅在毒丹 ItemUse 时调 alchemy 已有服丹动画 —— 减少新增，但失去"未来通用"窗口

**默认推 A**。新建一个通用名（`bong:eat_food`）+ 优先在毒丹路径实装；alchemy 服丹动画保持原样不动，未来若 alchemy v3 想统一可走迁移 PR。这条决策不阻塞本 plan，但需 P3 阶段定调命名约定，避免后续迁移成本。

---

## §6 进度日志

- **2026-05-08** 骨架立项。worldview §五:480 + §五:525-535 「毒（特性，泛型）vs 毒蛊（流派，专属）」边界正典物理化身。
  - 设计轴心：吃毒丹累积 PoisonToxicity → 给非毒蛊招式（暗器/阵法/爆脉拳）附毒 → 代价寿命/消化/经脉 MICRO_TEAR
  - 严格隔离 plan-dugu-v2：不触发暴露 / 不写 IdentityProfile / 染色可洗 / 经脉 MICRO_TEAR 而非 SEVERED / 形貌可消退
  - 复用底盘：alchemy-v2 ✅ side_effect_pool（毒丹载体）/ lifespan-v1 ✅（寿元扣减）/ multi-style-v1 ✅（QiColor 暗绿/紫）/ meridian-severed-v1 ⏳（MICRO_TEAR）/ skill-v1 ✅（AttackModifier hook）
  - 新建：DigestionLoad component（**通用底盘**，未来 plan-food / plan-yangsheng 也可复用，本 plan 落地时仅毒丹接入）
  - qi_physics 默认不扩 —— 毒效=独立 dmg debuff，不修饰 ρ
  - 反向被依赖：plan-anqi-v2 / plan-zhenfa-v2 / plan-baomai-v3 / plan-style-balance-v1（毒效附加 hook）
  - 待补：等 plan-meridian-severed-v1 P1 ship（MeridianSeveredEvent API 落地）→ P0 决策门最后一项
- **2026-05-08（同日扩项）** 用户提出三方向补全：
  - **§3.5 五种初级毒丹 + 5 种毒粉规格表**：乌髓散心 / 赤陀蜘髓 / 青鳞曼陀 / 铁腹蛇胆 / 腐心玄龟，骨币阶梯 50→800，PoisonToxicity 累积 +5→+15，每丹独有 side_effect_tag。每丹 craft 研磨 → 对应粉 ×3，粉用于暗器涂抹瞬时附毒（区别于丹的长期累积）
  - **双层附毒设计**：长期 stat 修饰（吃丹 PoisonToxicity）+ 瞬时载体涂抹（毒粉 debuff），叠加规则"加性+乘性"防双重计算
  - **plan-craft-v1 ⏳ 集成**：毒丹研磨走 craft 通用手搓，需 plan-craft-v1 §5 #1 决策门补 `PoisonPowder` 第 7 类 category（或归 Misc）
  - **客户端资产扩**：10 个 item icon（5 丹+5 粉，gen-image skill `style=item` + 强制 transparent bg）+ **吃食物动画双视角**（第三人称 PlayerAnimator JSON `bong:eat_food` + 第一人称 mixin override）—— 当前 21 个 PlayerAnimator JSON 无 eat/drink 类，本 plan 借机立通用底盘
  - **网络参考代码**：① KosmX/fabricPlayerAnimatorExample（PlayerAnimator JSON schema 直接照搬）② Theoness1/EatingAnimation（ItemUse 事件捕获逻辑）③ 跳过 Fuzss/betteranimationscollection（1.20.1 仅维护）
  - **新增 §5 决策门 #8/#9/#10**：5 种丹设计合理性 / 双层附毒叠加规则 / 吃食物动画范围（通用 vs 私有 vs 复用 alchemy）
  - **P1 单测下限**：80 → 100（覆盖 5 丹独有 side_effect_tag + 5 粉 PoisonDebuff 差异化 + 双层叠加公式）
  - **反向被依赖扩**：plan-anqi-v2 双层接入（PoisonToxicity 修饰 + 涂粉接口）/ plan-craft-v1 5 种研磨配方注册 / plan-alchemy-v2 5 种初级毒丹 alchemy 配方注册

---

## Finish Evidence

- **落地清单**
  - server 底盘：`server/src/cultivation/poison_trait/{components,events,handlers,tick,attack_hook,recipes}.rs`，注册点 `server/src/cultivation/mod.rs`。
  - 毒丹/毒粉数据：5 个 alchemy recipe 位于 `server/assets/alchemy/recipes/poison_trait_*_v1.json`；5 丹 + 5 粉 item template 位于 `server/assets/items/pills.toml`；5 个研磨 recipe 由 `register_craft_recipes` 注入 `CraftCategory::PoisonPowder`。
  - consume / cost 链路：`ConsumePoisonPillIntent` 从 `server/src/network/client_request_handler.rs` 发出；`consume_poison_pill_system` 写 `PoisonToxicity` / `DigestionLoad`；`apply_poison_overdose_costs` 扣寿元并 emit `MeridianCrackEvent` MICRO_TEAR。
  - Attack hook：`apply_poison_attack_modifier` 覆盖长期 `PoisonToxicity` 修饰 + 瞬时毒粉 debuff，毒蛊招式路径排除长期毒性修饰。
  - IPC / agent：Rust schema `server/src/schema/poison_trait.rs`；server-data emit `server/src/network/poison_trait_emit.rs`；Redis channels `bong:poison/dose` / `bong:poison/overdose`；TypeBox schema `agent/packages/schema/src/poison-trait.ts` + generated JSON；叙事 runtime `agent/packages/tiandao/src/poison-trait-runtime.ts`。
  - client：HUD planner/store `PoisonTraitHudPlanner` / `PoisonTraitHudStateStore`；server-data handler `PoisonTraitServerDataHandler` + router registration；HUD layer `POISON_TRAIT`；2 粒子、1 音效 recipe、10 个透明 item icon；通用 `bong:eat_food` PlayerAnimator JSON 与 `client/tools/gen_eat_food.py`。

- **关键 commit**
  - `065c17042` · 2026-05-11 · `plan-poison-trait-v1: 落地毒性真元服务端底盘`
  - `ecdb67fb7` · 2026-05-11 · `plan-poison-trait-v1: 接入 agent 毒性叙事契约`
  - `ad0fdea34` · 2026-05-11 · `plan-poison-trait-v1: 补齐客户端毒性反馈资产`
  - `a13949aa2` · 2026-05-11 · `plan-poison-trait-v1: 修复毒性服务端 review 阻断`
  - `ae9dc914b` · 2026-05-11 · `plan-poison-trait-v1: 接入毒性 agent 叙事通道`
  - `a20ea66d1` · 2026-05-11 · `plan-poison-trait-v1: 接入客户端毒性 server-data`

- **测试结果**
  - `cargo fmt --check`（server）→ passed。
  - `cargo check --all-targets -j 1`（server）→ passed。
  - `CARGO_BUILD_JOBS=1 cargo clippy --all-targets -- -D warnings`（server）→ passed。
  - `CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 cargo test poison_trait -j 1`（server）→ 118 passed / 3836 filtered。
  - `npm run build`（agent workspace）→ passed。
  - `cd agent/packages/schema && npm test` → 17 files / 358 tests passed。
  - `cd agent/packages/tiandao && npm test` → 49 files / 339 tests passed。
  - `JAVA_HOME=$HOME/.sdkman/candidates/java/17.0.18-amzn ./gradlew test build`（client）→ BUILD SUCCESSFUL。
  - 资源检查：10 个 poison icon 均为 RGBA，且同时存在透明像素与不透明像素；`eat_food` / particle / audio recipe JSON 均通过 `python3 -m json.tool`；`python3 tools/render_animation.py ...eat_food.json --ticks 0,6,12,18,24` 生成 headless preview 成功。

- **跨仓库核验**
  - server 符号：`PoisonToxicity`、`DigestionLoad`、`PoisonDoseEvent`、`PoisonOverdoseEvent`、`PoisonPowderConsumedEvent`、`PoisonAttackKind`、`CraftCategory::PoisonPowder`、`RedisOutbound::PoisonDoseEvent`、`ServerDataPayloadV1::PoisonTraitState`。
  - agent 符号：`PoisonTraitStateV1`、`PoisonDoseEventV1`、`PoisonOverdoseEventV1`、`PoisonTraitNarrationRuntime`、`CHANNELS.POISON_DOSE_EVENT`。
  - client 符号：`PoisonTraitHudPlanner`、`PoisonTraitHudStateStore`、`PoisonTraitServerDataHandler`、`ServerDataRouter`、`HudRenderLayer.POISON_TRAIT`、`BongAnimations.EAT_FOOD`、`EatFoodAnimation.ID`。

- **遗留 / 后续**
  - WSLg `runClient` 双视角手动实跑未纳入本次自动验证；当前以 Gradle build、headless animation render、资源 JSON 和 icon alpha 检查覆盖。
  - `bong:eat_food` 已作为通用动画资源落地；后续可把既有 alchemy 服丹路径统一迁到该 id。
  - 毒性数值仍需 `plan-style-balance-v1` telemetry 校准；`plan-anqi-v2` / `plan-zhenfa-v2` / `plan-baomai-v3` 可直接调用本 plan 提供的 PoisonAttackHook。
  - `DigestionLoad` 作为通用底盘，留给 `plan-food-v1` / `plan-yangsheng-v1` 复用；毒丹 MICRO_TEAR 后续可与 `plan-yidao-v1` 接经术做 e2e 联调。
