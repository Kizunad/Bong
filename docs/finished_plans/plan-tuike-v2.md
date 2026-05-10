# Bong · plan-tuike-v2 · 骨架

替尸·蜕壳功法**三招完整包**：动画 / 特效 / 音效 / 装备 / 真元消耗 / 反噬 / 客户端 UI 全流程。承接 `plan-tuike-v1` ✅ finished（PR #124 commit 50cc0cc4 归档）—— v2 引入**影论假影承伤物理**（worldview §P 定律 6 + cultivation-0002 §影论）+ **物资派纯粹定调**（worldview §五:471 钱包代价不绑身体，永不 SEVERED）+ **三档伪皮 + 化虚上古级**（worldview §十六.三 一次性脆化）+ **专克毒蛊永久标记**（化虚级 hard counter）。三招（着壳 / 蜕一层 / 转移污染）覆盖物资派全部防御场景，**不做化虚专属新招式** ——化虚级仅是"钱包更深、能买到上古级伪皮"，无身体质变（区别于涡流化虚紊流死区 + 毒蛊化虚倒蚀）。**死蛹假死招式不立**（worldview §十二 死亡机制冲突 + 物资派应纯粹）。

**世界观锚点**：`worldview.md §五:438-441 替尸/蜕壳流核心定义`（伪灵皮 + 蜕落带走污染）· `§五:471 物资派定调`（代价在钱包不在真元）· `§五:567 凝实色`（与器修同源——伪皮亦载体）· `§六:603-617 染色谱凝实色`（真元有形质感易附着物体，御物/暗器/伪皮真元损耗 -）· `§十六.三 上古遗物脆化`（一次性使用，化虚伪皮的物理依据）· `§十六.六 道伥反向`（参考但本 plan 不实装）· `§K narration 沉默`

**library 锚点**：`cultivation-0002 烬灰子内观笔记 §影论`（身外身假影承实物理推导）· `peoples-0007 散修百态` 仓鼠玩家路（替尸者社会类型）

**前置依赖**：

- `plan-qi-physics-v1` P1 ship → 影论 K_shed / β=1.2 走 `qi_physics::collision`
- `plan-qi-physics-patch-v1` P0/P3 → 7 流派 ρ/W/β 矩阵实装（替尸 β=1.2）
- `plan-skill-v1` ✅ + `plan-hotbar-modify-v1` ✅ → SkillRegistry / Casting / cooldown 直接复用
- `plan-multi-style-v1` ✅ → PracticeLog vector 接入凝实色累积
- `plan-spiritwood-v1` ✅ + `plan-fauna-v1` ✅ → 伪皮材料（拟态灰烬蛛丝 / 死域朽木 / 异变兽骨皮 / 上古级灵木）
- `plan-craft-v1` 🆕 skeleton → **伪皮制作走通用手搓 tab，本 plan 不定义"制壳"招式**（避免重叠）
- `plan-inventory-v2` ✅ + `plan-input-binding-v1` ✅ + `plan-HUD-v1` ✅

**反向被依赖**：

- `plan-style-balance-v1` 🆕 → 三招的 W/β 数值进矩阵（替尸 β=1.2 / 专克毒蛊 W=0.7）
- `plan-tribulation-v2` 🆕 active → 化虚替尸者 hard counter 化虚毒蛊师场景（绝壁劫 PVP 威慑）
- `plan-narrative-political-v1` ✅ active → 化虚级一次性烧上古伪皮的江湖传闻（"昨日某战，烧 3 件上古皮，赢"）
- `plan-tsy-loot-v1` ✅ → 上古级伪皮材料源（坍缩渊深层 jackpot）

---

## 接入面 Checklist

- **进料**：`cultivation::Cultivation { qi_current, qi_max, realm, contamination, qi_color }` / `qi_physics::ledger::QiTransfer` / `qi_physics::collision::qi_collision`（β=1.2 蜕壳启动）/ `SkillRegistry` / `SkillSet` / `Casting` / `PracticeLog` / `Realm` / `inventory::EquipSlot` / `botany::PlantRegistry`（部分伪皮材料）/ `mineral` / `craft::CraftRegistry`（伪皮配方注册）
- **出料**：3 招 `TuikeSkillId` enum 注册到 SkillRegistry / `WornFalseSkin` component（装备槽 + 档位 + spirit_quality + 当前 contam 承载）/ `StackedFalseSkins` component（多层叠穿，境界递增 1-3 层）/ `FalseSkinSheddedEvent` 🆕（蜕落事件 + 伪皮残骸地面物）/ `ContamTransferredEvent` 🆕（污染推到伪皮）/ `FalseSkinDecayedToAshEvent` 🆕（10-30min 后蜕落物腐烂为残灰）/ `PermanentTaintAbsorbedEvent` 🆕（化虚级上古伪皮吸永久标记 hard counter dugu）
- **共享类型**：`StyleDefense` trait（qi_physics::traits）/ 伪皮 ItemId 三档 + 上古级 / `EnvField` 不需扩展（替尸不创造环境影响场，跟涡流紊流场 / 毒蛊残留区不同——这是物资派的纯粹）
- **跨仓库契约**：
  - server: `combat::tuike_v2::*` 主实装 / `schema::tuike_v2`
  - agent: `tiandao::tuike_v2_runtime`（3 招 narration + 蜕落叙事 + 化虚一战烧上古伪皮的江湖传闻 + 永久标记被吸的毒蛊师视角叙事）
  - client: 3 动画 + 3 粒子 + 3 音效 recipe + 2 HUD 组件
- **worldview 锚点**：见头部
- **qi_physics 锚点**：蜕壳启动成本 K_shed 走 `qi_physics::constants::TUIKE_BETA`(=1.2，待 patch P3 加) + `qi_physics::collision::qi_collision`；伪皮承伤分配走 `qi_physics::field::shed_to_carrier(skin, damage, contam)` 🆕（patch P3 加新算子）；**禁止 plan 内自己写承伤分配公式**

---

## §0 设计轴心

- [ ] **物资派纯粹定调（worldview §五:471）**：替尸者损失的是**钱包**（材料 / 伪皮 / 上古遗物），**不是身体**（永不 SEVERED，永不损 qi_max，永不 contam 永久残留）。这是替尸跟其他 6 流派最本质的区别：
  - 体修：肉身代价（经脉龟裂）
  - 暗器：载体代价（一次性飞针）+ 真元封存代价
  - 阵法：环境代价（预埋失败 = 真元白白流失）
  - 毒蛊：身体永久代价（阴诡色形貌异化不可洗 + 自蕴慢性侵蚀）
  - 截脉：身体即时代价（皮下震爆自伤）
  - 涡流：身体可恢复代价（经脉过载 → contam → MICRO_TEAR/TORN/SEVERED）
  - **替尸：纯粹钱包代价，永不身体反噬**
  worldview §十二 死亡机制不会因替尸特殊化（重生时不必清空伪皮库存等）

- [ ] **影论假影承伤（worldview §P 定律 6 + cultivation-0002 §影论）**：
  ```
  E_shed = false_skin.spirit_quality × material_tier_factor   伪皮单层吸收上限
    material_tier_factor:
      凡级（凡铁/普通木）: 0.2  (上限 ~20 伤)
      轻档（拟态灰烬蛛丝）: 0.5 (上限 ~50 伤)
      中档（死域朽木）: 1.5    (上限 ~150 伤)
      重档（异变兽骨皮）: 4.0  (上限 ~400 伤)
      上古级（worldview §十六.三）: 10.0+ (上限 ~1000+ 伤，一次性脆化)

  K_shed = β × current_qi × 0.05 = 1.2 × qi_current × 0.05  蜕壳启动成本
    凝脉 qi 80: K_shed = 4.8
    通灵 qi 2100: K_shed = 126
    化虚 qi 10700: K_shed = 642

  替尸无反伤——蜕落是延迟+转移，不是反击
  ```

- [ ] **三档伪皮 + 化虚上古级 + 多层叠穿**：
  | 境界 | 伪皮档位上限 | 同时叠穿层数 |
  |---|---|---|
  | 醒灵-引气 | 凡级 | 1 层 |
  | 凝脉 | 轻档 | 1 层 |
  | 固元 | 中档 | 2 层 |
  | 通灵 | 重档 | 2 层 |
  | 半步化虚 | 重档+ | 3 层 |
  | **化虚** | **上古级**（worldview §十六.三 一次性脆化） | 3 层 |
  
  化虚替尸者 = 站着穿 3 层上古级伪皮的肉钱包。worldview §四:380「化虚老怪走过新人来不及看清袍角」物理化身——化虚替尸者一战可能烧 1-3 件上古级伪皮，是地球上最贵的人

- [ ] **专克毒蛊 + 化虚上古伪皮吸永久标记 hard counter（worldview §P W=0.7 + §十六.三）**：
  - 普通转移污染：当前 contam 推到当前穿着伪皮（qi 10/% 兑换）
  - **化虚级特殊**：上古级伪皮可**吸收永久 qi_max 衰减标记**（plan-dugu-v2 通灵+ 蚀针造成的 PermanentQiMaxDecay component）→ 蜕落该层 → 永久标记**跟伪皮一起销毁**
  - 化虚替尸者 vs 化虚毒蛊师场景：
    - 毒蛊蚀针注入永久 qi_max 衰减
    - 替尸 ③ 转移污染推到当前穿着的上古级伪皮
    - ② 蜕一层 → 永久标记跟伪皮一起销毁
    - 代价：一件上古级伪皮（worldview §十六.三 一次性脆化，不可逆）
  - 这是物资派最纯粹的极限化身——用钱包深度直接抵消最阴的身体永久代价

- [ ] **制壳走 craft-v1 通用手搓 tab（不在本 plan 内）**：所有伪皮通过 `plan-craft-v1` 注册的 `TuikeSkin` 类配方手搓制作（轻 / 中 / 重 / 上古级四档），本 plan 三招不重复定义"制壳"。worldview §五:471 物资派定调：替尸者**主要时间花在准备材料 / 制作伪皮**（craft-v1 慢任务），战场用现成的（tuike-v2 三招）

- [ ] **不做死蛹假死招式**：worldview §十二 死亡机制 / inspect 显死亡 / 重生流程冲突。物资派应纯粹"花钱买防御"，不需要装死类 trick。撤退靠跑路（worldview §四 "知道什么时候跑比知道怎么打更重要"），不靠装死

- [ ] **不做化虚专属新招式**：worldview §五:471 物资派定调——化虚级仅是"钱包更深"，不需要"化虚质变招式"。这跟涡流（化虚涡心紊流死区）+ 毒蛊（化虚倒蚀）形成有意区别。化虚替尸者就是更贵更耐打，不是新形态

- [ ] **无境界 gate，只有威力门坎**（worldview §五:537）：3 招都允许任何境界 cast。低境受限于：
  - qi_current 不足（蜕一层 K_shed 4.8-642 qi）→ Reject
  - 伪皮档位上限（凡级 → 化虚才能买上古级）
  - 多层叠穿数（醒灵-凝脉 1 层 → 化虚 3 层）

---

## §1 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ⬜ | 决策门：3 招数值表锁定 + 4 档伪皮 spirit_quality 范围对齐 + 化虚级上古伪皮 hard counter dugu 永久标记机制设计 + §5 五决策门收口 + qi_physics 接入面定稿（K_shed / β=1.2 锚定到 patch P3 ρ/β 矩阵）+ 与 plan-craft-v1 伪皮配方分发协议（4 档配方注册到 craft-v1 TuikeSkin 类） | 数值矩阵 + 物理公式落 plan §2 / craft-v1 协议对齐 |
| **P1** ⬜ | server `combat::tuike_v2::*` 3 招 logic + 影论物理（K_shed / E_shed）+ 多层叠穿管理 + 永久标记吸收（化虚级）+ 蜕落物（FalseSkinShedded → 地面物 → 10-30min 腐烂为残灰）+ qi_physics 算子调用 + ≥80 单测（每招 ≥20 测覆盖 happy/边界/化虚 hard counter/守恒断言）| `cargo test combat::tuike_v2` 全过 / 守恒断言（K_shed 走 ledger）/ 化虚伪皮吸永久标记测试 / `grep -rcE '#\[test\]' server/src/combat/tuike_v2/` ≥ 80 |
| **P2** ⬜ | client 3 动画（着壳穿戴 / 蜕落瞬间衣物飘散 / 转移污染手指点经脉推到伪皮）+ 3 粒子（FALSE_SKIN_DON_DUST / FALSE_SKIN_SHED_BURST / 上古级蜕落特殊光效 ANCIENT_SKIN_GLOW）+ 2 HUD 组件（FalseSkinStackHud 多层显示 + ContamLoadHud 当前承载）| render_animation.py 验证 / WSLg 实跑 3 招视觉确认 / HUD 多层叠穿渲染清晰 |
| **P3** ⬜ | 3 音效 recipe（don_skin_low_thud / shed_skin_burst / contam_transfer_hum）+ agent 3 招 narration template + 化虚级一次烧上古伪皮的江湖传闻型叙事（plan-narrative-political-v1 联调）+ 永久标记被吸的毒蛊师视角叙事 | narration-eval ✅ 3 招 + 化虚一战烧 3 上古的政治叙事 全过古意检测 |
| **P4** ⬜ | PVP telemetry 校准 / 凝实色 hook（PracticeLog → QiColor 凝实色累积演化）/ 跟 dugu-v2 化虚 hard counter 实战测试 / 化虚级伪皮材料经济与 plan-tsy-loot-v1 + plan-economy-v1 联调（上古级伪皮材料源 + 价格区间）| 7 流派 4×3 攻防对位替尸通过 / 化虚 hard counter 实战测试通过 / 上古级伪皮经济曲线落 telemetry |

**P0 决策门**：完成前 §5 五决策必须有答案。

---

## §2 三招完整规格

### ① 着壳 — 装备伪灵皮（被动状态，非招式）

**用途**：inventory 装备槽插入 1-3 件伪皮（境界递增层数上限）。**不是战斗触发招式**，是装备状态。着壳后才能用 ②③。

| 境界 | 伪皮档位上限 | 叠穿层数 | 维持 qi/s（每层）|
|---|---|---|---|
| 醒灵 | 凡级（吸 ~20 伤）| 1 | 0.1 |
| 引气 | 凡级（吸 ~25 伤）| 1 | 0.1 |
| 凝脉 | 轻档（吸 ~50 伤）| 1 | 0.2 |
| 固元 | 中档（吸 ~150 伤）| 2 | 0.3 |
| 通灵 | 重档（吸 ~400 伤）| 2 | 0.5 |
| 半步化虚 | 重档+ | 3 | 0.6 |
| 化虚 | **上古级**（吸 1000+ 伤）| 3 | 1.0 |

**机制**：
- inventory 装备槽插入 → 装备 system 触发 `WornFalseSkin` component 写入 + `StackedFalseSkins` 多层管理
- 维持成本：每秒 0.1-1.0 qi/层（worldview §五:439 "注入少量真元模拟气息"），多层 qi cost 线性叠加
- 模拟气息效果：高境 NPC 神识感知到 caster 形象偏移（passive 伪装，类似 dugu 神识遮蔽但不主动）

**没有 cast 动作 / cooldown** —— 装备一次后常驻，直到主动卸下或全部蜕完

### ② 蜕一层 — 弃壳承伤+清污染

**用途**：主动 / 受击双 trigger。承担伤害+污染，伪皮销毁。

**主动 trigger**（玩家按键）：
- 瞬间蜕一层
- 承担 0 当前伤害
- 把该层全部 contam 清空（连同伪皮一起销毁）
- qi cost：K_shed = 1.2 × qi_current × 0.05
- cooldown 8s

**受击 trigger**（伤害 ≥ 阈值自动）：
- 蜕落最外层
- 该层承担 100% 此次伤害（spirit_quality 归 0）
- 该层全部 contam 跟伪皮一起销毁
- 自动触发，无 qi cost（影论"无反伤" worldview §P 定律 6）

**蜕落物**：地上掉伪皮残骸 → emit `FalseSkinSheddedEvent { skin_tier, contam_load }` → 10-30 min 后腐烂为残灰方块（worldview §二 末法分解物理）→ emit `FalseSkinDecayedToAshEvent`
- 残骸期间其他玩家可拾取（材料回收 / 信号追踪——是身份暴露的物理风险源）
- 上古级伪皮残骸可被拾取作"上古遗物碎片"（worldview §十六.三 + plan-tsy-loot-v1）

**裸壳期**（蜕完无伪皮）：3-5s 内防御 -50%（破绽窗口，worldview §五:440 "几层壳打光后比纸还脆弱"）。多层叠穿时仅最外层蜕落，里层未受影响

### ③ 转移污染 — 推已侵入污染到伪皮（化虚级 hard counter dugu）

**用途**：把已侵入身体经脉的 contam 主动推到当前最外层伪皮，再用 ② 蜕一层带走。

| 境界 | qi 兑换率 | 单次推动上限 | 化虚专属 |
|---|---|---|---|
| 醒灵 | 15 qi / 1% contam | 1% | — |
| 引气 | 13 qi / 1% | 2% | — |
| 凝脉 | 11 qi / 1% | 3% | — |
| 固元 | 10 qi / 1% | 5% | — |
| 通灵 | 9 qi / 1% | 8% | — |
| 半步化虚 | 8 qi / 1% | 10% | — |
| **化虚** | 7 qi / 1% | 15% | **上古级伪皮可吸 PermanentQiMaxDecay component**（hard counter dugu 永久标记）|

**机制**：
- 普通转移：当前 contam 推到伪皮（伪皮承载上限 = spirit_quality 比例，满载 100%）
- 失败惩罚：超伪皮上限 → 反流回身体 + 额外 contam +5% / 5min（短期可恢复，物资派定调，无永久反噬）
- **化虚级 hard counter**：穿上古级伪皮时，③ 可吸收 dugu 蚀针造成的 PermanentQiMaxDecay component → 推到伪皮 → 蜕落 → 永久标记跟伪皮一起销毁
  - 物理依据：worldview §P W=0.7 + §十六.三 上古遗物级别一次性脆化
  - 经济代价：一件上古级伪皮（worldview §十六.三）—— 化虚替尸者 vs 化虚毒蛊师一战可能烧 1-3 件上古遗物
  - cooldown：吸永久标记 30s（防 spam）

**worldview 锚**：§P W=0.7（替尸 vs 毒蛊伤害最克）的物理化身 + §五:471 物资派最纯粹极限化（钱包抵消身体永久代价）

---

## §3 数据契约

```
server/src/combat/tuike_v2/
├── mod.rs              — Plugin + register_skills + register_recipes(craft-v1)
├── skills.rs           — TuikeSkillId enum (Don/Shed/TransferTaint)
│                        + 3 resolve_fn (cast_don / cast_shed / cast_transfer_taint)
├── state.rs            — WornFalseSkin component (skin_tier, spirit_quality,
│                                                   contam_load, perma_taint_load)
│                        + StackedFalseSkins component (Vec<Entity>，多层叠穿，
│                                                       境界递增 1-3 层)
│                        + FalseSkinResidue component (地面物，10-30min 腐烂计时)
├── tick.rs             — maintenance_tick (每秒扣维持 qi 0.1-1.0/层) +
│                        residue_decay_tick (蜕落物腐烂为残灰)
├── physics.rs          — 影论物理 (K_shed / E_shed) + 多层蜕落优先级 +
│                        永久标记吸收 (化虚级特殊判定)
└── events.rs           — DonFalseSkinEvent / FalseSkinSheddedEvent /
                          ContamTransferredEvent / FalseSkinDecayedToAshEvent /
                          PermanentTaintAbsorbedEvent (化虚级 hard counter)

server/src/schema/tuike_v2.rs  — IPC schema 3 招 + 多层叠穿状态 + 蜕落事件 payload

agent/packages/schema/src/tuike_v2.ts  — TypeBox 双端
agent/packages/tiandao/src/tuike_v2_runtime.ts  — 3 招 narration +
                                                  化虚级一战烧上古伪皮的江湖传闻 +
                                                  永久标记被吸的毒蛊师视角叙事

client/src/main/java/.../combat/tuike/v2/
├── TuikeV2AnimationPlayer.java        — 3 动画播放
├── FalseSkinDonDustParticle.java      — 着壳穿戴时尘埃飘起
├── FalseSkinShedBurstParticle.java    — 蜕落瞬间衣物碎片
├── AncientSkinGlowParticle.java       — 上古级蜕落特殊光效
├── FalseSkinStackHud.java             — 多层叠穿可视（小图标层叠 + spirit_quality bar）
└── ContamLoadHud.java                 — 当前最外层伪皮 contam 承载量

client/src/main/resources/assets/bong/
├── player_animation/tuike_don_skin.json
├── player_animation/tuike_shed_burst.json
├── player_animation/tuike_taint_transfer.json
└── audio_recipes/don_skin_low_thud.json + shed_skin_burst.json +
                  contam_transfer_hum.json
```

**SkillRegistry 注册**：

```rust
pub fn register_skills(registry: &mut SkillRegistry) {
    registry.register("tuike.don",             cast_don);             // 着壳
    registry.register("tuike.shed",            cast_shed);            // 蜕一层
    registry.register("tuike.transfer_taint",  cast_transfer_taint);  // 转移污染
}
```

**craft-v1 配方注册**（伪皮制作走通用手搓）：

```rust
pub fn register_recipes(registry: &mut CraftRegistry) {
    registry.register(CraftRecipe {
        id: RecipeId::new("tuike.false_skin.light"),
        category: CraftCategory::TuikeSkin,
        materials: vec![(ItemId::AshSpiderSilk, 5), (ItemId::SpiritWoodScrap, 2)],
        qi_cost: 15.0,
        time_ticks: 10 * 60 * 20,  // 10 min in-game
        output: (ItemId::FalseSkinLight, 1),
        requirements: CraftRequirements::default(),  // 无境界 gate
    });
    // ... 中档 / 重档 / 上古级（require 上古遗物级材料）
}
```

**PracticeLog 接入**：

```rust
emit SkillXpGain {
    char: caster,
    skill: SkillId::Tuike,
    amount: per_skill_amount(skill_kind),  // don 1 / shed 2 / transfer 1
    source: XpGainSource::Action {
        plan: "tuike_v2",
        action: skill_kind.as_str(),
    }
}
```

PracticeLog 累积驱动 QiColor **凝实色**（worldview §六:614）演化，由 plan-multi-style-v1 ✅ 已通的机制接管。凝实色加成：worldview §六:614 "御物 / 暗器流真元损耗 -" → 替尸维持 qi/s 减半（凝实色 ≥ 30% 后）

---

## §4 客户端新建资产

| 类别 | ID | 来源 | 优先级 | 备注 |
|---|---|---|---|---|
| 动画 | `bong:tuike_don_skin` | 新建 JSON | P2 | 着壳穿戴姿态（双手抚背 + 衣物虚化 → 实化），priority 300（姿态层）|
| 动画 | `bong:tuike_shed_burst` | 新建 JSON | P2 | 蜕落瞬间衣物飘散 + 身体短暂缩屈，priority 1500（高阶战斗）|
| 动画 | `bong:tuike_taint_transfer` | 新建 JSON | P2 | 手指点经脉 → 推真元到伪皮，priority 800（中阶战斗）|
| 粒子 | `FALSE_SKIN_DON_DUST` ParticleType + Player | 新建 | P2 | 着壳尘埃飘起 |
| 粒子 | `FALSE_SKIN_SHED_BURST` ParticleType + Player | 新建 | P2 | 蜕落瞬间衣物碎片爆开 |
| 粒子 | `ANCIENT_SKIN_GLOW` ParticleType + Player | 新建 | P2 | 上古级蜕落特殊光效（金色 + 古纹理碎裂感）|
| 音效 | `don_skin_low_thud` | recipe 新建 | P3 | layers: `[{ sound: "item.armor.equip_leather", pitch: 0.7, volume: 0.6 }]`（着装低沉感）|
| 音效 | `shed_skin_burst` | recipe 新建 | P3 | layers: `[{ sound: "block.wool.break", pitch: 1.2, volume: 0.7 }, { sound: "item.armor.equip_leather", pitch: 1.5, volume: 0.4, delay_ticks: 1 }]`（衣物撕裂 + 飘散）|
| 音效 | `contam_transfer_hum` | recipe 新建 | P3 | layers: `[{ sound: "block.beacon.activate", pitch: 1.0, volume: 0.4 }]`（嗡鸣低音）|
| HUD | `FalseSkinStackHud` | 新建 | P2 | 多层叠穿可视（角色侧栏小图标层叠 + 每层 spirit_quality bar）|
| HUD | `ContamLoadHud` | 新建 | P2 | 当前最外层伪皮 contam 承载量（用于决策何时主动 ② 蜕掉）|

**无独立蓄力 ChargeRing UI** ——3 招都是瞬时 / 持续，无蓄力期。复用 SkillBar cooldown 灰显即可。

---

## §4.5 P1 测试矩阵（饱和化测试）

下限 **80 单测**：

| 模块 | 测试组 | 下限 |
|---|---|---|
| `cast_don` | 7 境界档位上限校验 + 多层叠穿数边界 + qi 不足 reject + 装备槽空 reject + 维持 qi/s 计算 | 12 |
| `cast_shed` | 主动 trigger 7 境界 K_shed + 受击 trigger 自动判定 + 该层 contam 清空 + 蜕落物地面落 + 裸壳期 -50% 防御 + 多层情况下仅蜕最外层 | 18 |
| `cast_transfer_taint` | 7 境界 qi/% 兑换 + 伪皮承载上限 + 超上限反流 + 化虚级吸 PermanentQiMaxDecay + 通灵以下吸永久标记拒绝 + cooldown 30s | 18 |
| `false_skin_residue_decay` | 蜕落物 10-30min 腐烂为残灰 + 残骸期被拾取 + 上古级残骸算上古遗物碎片 | 8 |
| `worn_skin_maintenance` | 维持 qi/s 多层叠加 + qi 不足时哪层先剥落 + 凝实色加成 ≥ 30% 半价 | 8 |
| `permanent_taint_absorb` | 化虚专属判定 + dugu 注入永久标记 → tuike 推到上古伪皮 → 蜕落 → 标记销毁 + 物理依据守恒 | 10 |
| `craft_recipe_register` | 4 档伪皮配方注册到 craft-v1 + 材料/qi cost/时间正确 + 上古级要求上古遗物级材料 | 6 |

**P1 验收**：`grep -rcE '#\[test\]' server/src/combat/tuike_v2/` ≥ 80。守恒断言：所有 K_shed 必须走 `qi_physics::ledger::QiTransfer`。

---

## §5 开放问题 / 决策门（P0 启动前必须收口）

### #1 化虚级上古伪皮吸永久标记是 100% 还是部分？

- **A**：100% 吸收（化虚 hard counter 化虚毒蛊师，简洁有力）
- **B**：80% 吸收 + 20% 残留身体（避免完全废毒蛊师）
- **C**：随永久标记累积量分级（≤30% 100% 吸 / 30-60% 80% 吸 / >60% 50% 吸）

**默认推 A** —— worldview §五:471 物资派最纯粹极限化身。化虚替尸 vs 化虚毒蛊就是钱包深 vs 阴险，物资派应允许"花钱完全清账"。但代价 = 一件上古级伪皮（worldview §十六.三 一次性脆化），经济代价已极高

### #2 蜕落物伪皮残骸的腐烂时间

- **A**：10 min（快速回收，不污染地图）
- **B**：30 min（中等，给追兵留追踪窗口）
- **C**：60 min（长期，强化"残骸即信号"）

**默认推 B** —— worldview §五 PVP 信息差强调，蜕落物作为身份暴露物理依据需要足够窗口。但 30 min 后腐烂为残灰，不永久污染地图

### #3 多层叠穿同时维持 qi cost 是否线性叠加

- **A**：线性叠加（3 层 = 3 × 0.5 = 1.5 qi/s）
- **B**：累进打折（第 1 层 1.0，第 2 层 0.7，第 3 层 0.4 = 总 2.1 但每层不同）
- **C**：按最外层（仅维持最外层 qi cost，里层免费）

**默认推 A** —— 简洁 + worldview §五:471 物资派代价直接，不需要叠穿优化机制。3 层化虚伪皮维持 3 qi/s 对化虚 qi_max 10700 微不足道

### #4 上古级伪皮材料 cost

worldview §十六.三 一次性脆化级。材料源应该是：

- **A**：仅坍缩渊深层 jackpot 掉落（worldview §十 资源匮乏 + plan-tsy-loot-v1）
- **B**：坍缩渊 jackpot + 巨型异变兽 boss 掉落（多源）
- **C**：上述 + 玩家间交易（高价骨币 / 信息）

**默认推 C** —— 多源 + 自由经济，符合 worldview §九 信息比装备值钱（化虚替尸者付高骨币换上古材料是合理路径）

### #5 凝实色加成 hook 实装位置

毒蛊 v2 有自身 permanent_lock_mask 字段扩展（plan-multi-style-v1 模块）。替尸需要的凝实色加成（维持 qi/s 减半）实装位置：

- **A**：tuike-v2 内自行查询 PracticeLog 凝实色比例（推荐，归属清晰）
- **B**：扩展 cultivation::QiColor 加 style_passive_buff fn（其他流派可复用）
- **C**：等 plan-style-balance-v1 实装时统一处理

**默认推 A** —— 跟 dugu-v2 一致，每流派自行查询 QiColor 计算自身加成。其他流派 vN+1 时再考虑提取通用模块

---

## §6 进度日志

- **2026-05-06** 骨架立项，承接 plan-tuike-v1 ✅ finished（PR #124 commit 50cc0cc4 归档）。
  - 设计轴心：物资派纯粹定调（钱包代价不绑身体，永不 SEVERED）+ 影论假影承伤 + 三档伪皮 + 化虚上古级 + 多层叠穿 + 专克毒蛊 + 化虚级 hard counter 永久标记
  - 三招完整规格锁定（着壳 / 蜕一层 / 转移污染）—— 用户拍 B 选项 4 招后再删 ④ 死蛹假死，最终 3 招
  - **不做化虚专属新招式**（区别于涡流化虚紊流死区 + 毒蛊化虚倒蚀）—— 化虚级仅是钱包更深、能买上古级伪皮，无身体质变
  - **不做死蛹假死**（用户拒绝，跟 worldview §十二 死亡机制冲突 + 物资派应纯粹）
  - 化虚 hard counter dugu 永久标记：worldview §五:471 物资派最纯粹极限化身（钱包抵消身体永久代价）
  - 制壳走 plan-craft-v1 通用手搓 tab（伪皮 4 档配方注册，本 plan 不重复定义）
  - worldview 锚点对齐：§五:438-441 + §五:471 + §五:567 + §六:603-617 + §十六.三 + §十六.六 + §K + §P 定律 6
  - qi_physics 锚点：等 patch P0/P3 完成后接入；K_shed / β=1.2 / 化虚级 hard counter 走 qi_physics 算子
  - SkillRegistry / PracticeLog / HUD / 音效 / 动画 / craft-v1 配方注册 全部底盘复用
  - **当前游戏伤害基线确认**（grep server/src/combat/resolve.rs:85）：`ATTACK_QI_DAMAGE_FACTOR = 1.0` → damage ≈ qi_invest × 部位/武器/防御倍率，worldview §五:336-340 已对齐实装。伪皮档位 50/150/400/1000 锚定到"挡一次该境界全力一击"基准
  - 待补：与 plan-style-balance-v1 W/β 矩阵对齐 / 凝实色 hook（tuike-v2 P1 自行查询 PracticeLog）/ plan-tribulation-v1 化虚 hard counter 实战测试 / plan-tsy-loot-v1 + plan-economy-v1 上古级伪皮材料源与价格区间 / plan-craft-v1 4 档伪皮配方注册

---

## Finish Evidence

### 落地清单

- **P0 / 设计门收口**：`server/src/combat/tuike_v2/state.rs` / `physics.rs` 固化三招数值、`FalseSkinTier` 五档、境界叠穿层数、10-30min 残骸腐烂窗口、化虚上古级永久标记吸收规则；`TUIKE_BETA = 1.2` 归入 `server/src/qi_physics/constants.rs`。
- **P1 / server 三招与物理**：`server/src/combat/tuike_v2/{mod,state,physics,events,skills,tick,tests}.rs` 实装 `tuike.don` / `tuike.shed` / `tuike.transfer_taint`，接入 `SkillRegistry`、`KnownTechnique`、`DerivedAttrs.tuike_layers`、`PracticeLog` 凝实色维持折扣、`PermanentQiMaxDecay` hard counter、独立 `FalseSkinResidue` 地面残骸。
- **P1 / qi_physics 与 craft**：`server/src/qi_physics/field.rs` 增加 `shed_to_carrier` 算子，`server/src/craft/mod.rs` 注册 `tuike.false_skin.light` / `mid` / `heavy` / `ancient` 四档伪皮配方。
- **P2 / client UI 与视觉**：`client/src/main/java/com/bong/client/combat/store/FalseSkinHudStateStore.java`、`client/src/main/java/com/bong/client/hud/FalseSkinStackHud.java`、`ContamLoadHud.java`、`BongHudOrchestrator.java`、`TuikeFalseSkinParticlePlayer.java`、`VfxBootstrap.java`、`BongAnimations.java`，以及三份 `client/src/main/resources/assets/bong/player_animation/tuike_*.json`。
- **P3 / 音效与 agent 叙事**：`server/assets/audio/recipes/{don_skin_low_thud,shed_skin_burst,contam_transfer_hum}.json`，`server/src/network/{tuike_event_bridge,vfx_animation_trigger,audio_trigger,false_skin_state_emit,redis_bridge}.rs`，`agent/packages/schema/src/tuike-v2.ts`，`agent/packages/tiandao/src/tuike_v2_runtime.ts`。
- **P4 / 联调钩子**：`PracticeLog` 凝实色折扣、化虚上古伪皮吸永久标记、`FalseSkinStateV1.layers` 多层 HUD payload、`TuikeSkillEventV1` 叙事/视觉/音频事件、上古级配方材料与境界门槛均已落入可测 contract。

### 关键 commit

- `fdfdc418d` / 2026-05-10 / `feat(tuike): 实现蜕壳三招完整链路`：server + agent + client 跨栈实现、100 个 server 单测、schema/generated、HUD/VFX/动画/音频/叙事事件接入；rebase 到 `origin/main` 后保留 botany/fauna/lingtian/Dugu v2/zone-atmosphere/NPC engagement 上游注册并合入替尸 v2 注册。
- `d595b9e2f` / 2026-05-10 / `fix(tuike): 对齐音效 recipe schema`：替尸 v2 三份音效 recipe 改用当前合法 `HOSTILE` category，并把污染转移 hum pitch 收进 schema 下限。
- `1d2136d0c` / 2026-05-10 / `修复替尸 review 阻断项`：补 server `false_skin_state` 真实 `layers` 数组、裸壳期承伤放大、普通/永久污染转移 cooldown 分流、qi 不足自动蜕最外层、替尸空经脉依赖声明、`TUIKE_BETA` 归位 qi_physics、K_shed 真元释放入 zone/overflow 账本。
- `241728dab` / 2026-05-10 / `修复替尸 v2 CodeRabbit 反馈`：补 client HUD payload 非负清洗与同帧 snapshot、替尸粒子 origin/duration 防御、spent qi overflow 构造失败日志、Redis/VFX 映射回归测试、`TuikeSkillEventV1` 非空视觉 contract。
- `f41b9aad1` / 2026-05-10 / `修复替尸 v2 运行时边界反馈`：按 CodeRabbit 反馈收紧 server 运行时边界：`shed_to_carrier` 返回实际污染写回/溢出量、重复外层着壳拒绝且不产 cooldown/event/XP、裸壳窗口未过期时保留空 `StackedFalseSkins`、腐烂残渣发事件后 despawn 实体。

### 测试结果

- `git diff --check`：通过。
- `cd server && cargo fmt --check`：通过。
- `cd server && CARGO_BUILD_JOBS=1 CARGO_PROFILE_TEST_DEBUG=0 cargo test combat::tuike_v2`：通过，`109 passed`。
- `cd server && CARGO_BUILD_JOBS=1 CARGO_PROFILE_TEST_DEBUG=0 cargo test network::false_skin_state_emit::tests::emits_tuike_v2_layer_details_on_stack_change`：通过，锁定多层 HUD payload。
- `cd server && CARGO_BUILD_JOBS=1 CARGO_PROFILE_TEST_DEBUG=0 cargo test combat::resolve::tests::resolver_applies_tuike_naked_window_damage_penalty`：通过，锁定裸壳期承伤放大。
- `cd server && CARGO_BUILD_JOBS=1 CARGO_PROFILE_TEST_DEBUG=0 cargo test network::redis_bridge::redis_bridge_tests::publishes_tuike_v2_skill_event_on_correct_channel`：通过，锁定 `RedisOutbound::TuikeV2SkillEvent` → `CH_TUIKE_V2_SKILL_EVENT`。
- `cd server && CARGO_BUILD_JOBS=1 CARGO_PROFILE_TEST_DEBUG=0 cargo test network::vfx_animation_trigger::tests::tuike_v2`：通过，3 passed；锁定三类替尸 v2 视觉触发与永久污染颜色分支。
- `grep -rcE '#\[test\]' server/src/combat/tuike_v2/`：`tests.rs:109`，满足 P1 `>= 80`。
- `cd server && CARGO_BUILD_JOBS=1 CARGO_PROFILE_TEST_DEBUG=0 cargo clippy --all-targets -- -D warnings`：通过。
- `cd server && CARGO_BUILD_JOBS=1 CARGO_PROFILE_TEST_DEBUG=0 cargo test audio`：通过，22 passed；验证替尸 v2 三份音效 recipe 符合当前音频 schema。
- `cd server && CARGO_BUILD_JOBS=1 CARGO_PROFILE_TEST_DEBUG=0 cargo test`：通过，3937 passed。
- `cd agent && npm run generate -w @bong/schema`：通过。
- `cd agent && npm run generate:check -w @bong/schema`：通过，335 个 generated schema 文件保持 fresh。
- `cd agent && npm run build`：通过。
- `cd agent && npm test -w @bong/schema -- --maxWorkers=1`：通过，15 files / 353 tests。
- `cd agent && npm test -w @bong/tiandao`：通过，48 files / 333 tests。
- `cd client && JAVA_HOME=/home/kiz/.sdkman/candidates/java/17.0.18-amzn PATH=/home/kiz/.sdkman/candidates/java/17.0.18-amzn/bin:$PATH ./gradlew --no-daemon test --tests "com.bong.client.combat.handler.FalseSkinStateHandlerTest"`：通过。
- `cd client && JAVA_HOME=/home/kiz/.sdkman/candidates/java/17.0.18-amzn PATH=/home/kiz/.sdkman/candidates/java/17.0.18-amzn/bin:$PATH ./gradlew --no-daemon test build`：通过。

### 跨仓库核验

- **server**：`combat::tuike_v2::*`、`TuikeSkillId`、`FalseSkinTier`、`StackedFalseSkins`、`FalseSkinResidue`、`PermanentQiMaxDecay`、`shed_to_carrier`、`register_tuike_v2_recipes`、`TuikeSkillEventV1`、`FalseSkinStateV1.layers`、`FalseSkinStackStateV1`。
- **agent/schema**：`CH_TUIKE_V2_SKILL_EVENT`、`CH_FALSE_SKIN_STACK_STATE`、`tuikeV2SkillEventV1`、`falseSkinStackStateV1`、`createTuikeV2Runtime`。
- **client**：`FalseSkinStateHandler`、`FalseSkinHudStateStore`、`FalseSkinStackHud`、`ContamLoadHud`、`TuikeFalseSkinParticlePlayer`、`FALSE_SKIN_DON_DUST` / `FALSE_SKIN_SHED_BURST` / `ANCIENT_SKIN_GLOW`。

### 遗留 / 后续

- `plan-style-balance-v1` 继续消费真实 PVP telemetry 后调 W/β 数值；本 plan 已提供替尸侧事件与 hard-counter contract。
- `plan-economy-v1` / `plan-tsy-loot-v1` 后续可基于 `tuike.false_skin.ancient` 配方材料与上古残骸产物校准价格曲线。
- 若后续多个流派都需要凝实色维持折扣，可把当前 `PracticeLog` 查询从 `tuike_v2` 抽到统一 style passive helper。
