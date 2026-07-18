# plan-ancient-relic-payoff-v1 — 上古遗物变现闭环：1% jackpot 从死物到可用可碎

> **一句话主题**：TSY 首入 1% 上古遗物（`ancient_relic_*`）是**死物**——模板绕过 `ItemRegistry` 手搓生成，遗物剑没有 `weapon_spec` 装不上、残卷没有对应 scroll spec 学不会；`ancient_relics.rs` 头注释承诺的「每次使用由对应系统 `-= 1`，归零时由消费系统从 inventory 移除」**全库零实现**（无任何 charges 递减代码）。开出 jackpot 只得到一件占格装饰。本 plan 把 Weapon/Scroll/BeastCore/Pendant 四类遗物接进既有使用系统，charges 归零碎裂兑现"捡到即用、易碎"的正典。
>
> 来源：2026-07-18 早期玩法诊断——「搜刮→变现」断链两环之一（最亮的奖励不兑现，搜打撤的"搜"失去顶层动机）。

**状态**：骨架（skeleton）。升 active 前按 docs/CLAUDE.md §五收口 §8。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | 注册归一：遗物模板进正规 item 管线 + 挂真实 spec | ⬜ |
| P1 | charges 消费系统（四类各自接线）+ 归零碎裂移除 | ⬜ |
| P2 | 遗物使用/碎裂 A/V + tooltip 剩余次数 | ⬜ |
| P3 | 饱和测试 + bot e2e + 1% roll 集成回归 | ⬜ |

## 现状证据（2026-07-18 Explore 实证）

- `inventory/ancient_relics.rs:1-16` 头注释：strength tier → charges（1/3/5 次），「归零时由消费系统从 inventory 移除」「每次使用由对应系统 `-= 1`」——**消费系统不存在**：全库 grep `charges -= 1` / `charges.saturating_sub` 生产代码零命中（唯一引用 `inventory/mod.rs:2235` 是相等比较）。
- `ancient_relics.rs:71-100` `to_item_instance` 直接手搓 `ItemInstance` 绕过 `assets/items/*.toml` 注册管线——遗物剑无 `weapon_spec`（combat 不识别、装备无收益）、遗物残卷无 `technique_scroll_spec`/配方 spec（学习链不识别）。
- 例外即范本：SectRuins 家族的 `yixing_scroll`（`inventory/tsy_loot_spawn.rs:218`）走真注册、真可学——证明接线模式现成。
- `AncientRelicKind::SpiritTreasure` 已由 `plan-spirit-treasure-v1` 接了激活链（T 键 `TreasureActivate`）——**本 plan 范围排除**，只处理 Weapon/Scroll/BeastCore/Pendant 四类。
- 已决事项（遵守不动）：`plan-economy-v1` §4——遗物 `spirit_quality = 0`（"无灵"，worldview §十六 锚定）、不参与衰变、不进价格指数 supply。

## 接入面（docs/CLAUDE.md §二 checklist）

- **进料**：`AncientRelicPool` 模板表 + `tsy_loot_spawn.rs:126` 1% roll（不动概率铁律）；`ItemRegistry` 注册管线；combat `weapon_spec` 链（`sync_weapon_component_from_equipped`）；`TechniqueScrollUse`/`ScrollReadRequest`/配方残片链；breakthrough 增益通道（`BreakthroughBonus` buff 先例）。
- **出料**：遗物剑进 combat 伤害结算；遗物残卷产出功法/丹方/阵图；兽核产出突破助力；charges 归零 → inventory 移除 + 碎裂 A/V + narration；可交易可掉落维持（worldview 红线：不做装备绑定，"脆化是脆化，不是绑定"）。
- **共享类型 / event**：不新造使用事件——每类复用其宿主系统的既有事件；新增仅限 `RelicChargeConsumed`（内部 event，驱动碎裂表现与移除，若宿主事件足够则连这个都不加，§8 #2）。
- **跨仓库契约**：client 侧 tooltip 剩余 charges 显示（`InventoryItemViewV1` 已带 `charges` 字段则零 wire 变更，需实地核对；不足则 proto/samples 同步）；agent 不参与。
- **worldview 锚点**：§十六 TSY 生命周期（遗物三来源类）；§K 红线「不做装备绑定」「上古遗物作为离群 jackpot 不进市场公式」；末法基调——上古之物强而将朽，charges 极少即是叙事。
- **qi_physics 锚点**：遗物 `spirit_quality=0` 无封存真元，使用/碎裂**零 qi 流动**（economy-v1 已决，本 plan 维持）；**兽核突破助力严禁做成 qi 注入**（无灵之物凭空产 qi = 守恒红旗）——助力语义定为成功率/门槛类修正（§8 #3 拍板细节），零守恒风险。

## P0 — 注册归一 + 真实 spec ⬜

- 决策门（§8 #1）后落地：遗物模板迁 `assets/items/` TOML（与全库单一真相源一致，推荐）或保留 Rust 表但补挂 spec 字段——两者取一，**消除"手搓 ItemInstance 绕过 registry"路径**，`to_item_instance` 改从注册模板实例化。
- Weapon：挂 `weapon_spec`（reach/wound_kind/damage_multiplier 语义与玩家武器一致；倍率显著高于同代凡兵，档位 §8 #4 数值表）；装备链零改动自动生效。
- Scroll：按模板细分挂 `technique_scroll_spec` / 丹方 / 阵图 spec（对齐 `yixing_scroll` 范本）；BeastCore/Pendant 挂各自 category 与 P1 使用入口所需字段。
- 测试：全遗物模板启动校验（registry 引用、spec 完整性）；`ItemRarity::Ancient` + `spirit_quality=0` + `durability=1.0` 不变量 pin；1% roll 出的实例与注册模板逐字段对拍。

## P1 — charges 消费四类接线 + 归零碎裂 ⬜

- **Weapon**：命中结算后 charges `-= 1`（挂 combat resolve 出口，只对 `rarity=Ancient` 且带 charges 的主手生效）；tier 2/3 = 3/5 次全力一击的定位。
- **Scroll**：使用（学功法/读丹方/阵图）即 `-= 1`——tier 1 一次性，成功习得才消耗（学习被门槛拒绝不扣）。
- **BeastCore**：主动使用（复用 ApplyPill 式使用入口）挂"突破助力"buff——语义为下次突破成功率/环境门槛修正（非 qi 注入，见 qi_physics 锚点），用后 `-= 1`。
- **Pendant**：v1 定位为高价值信物/交易品 + lore tooltip（不接佩戴数值，防 scope 蔓延；佩戴语义留 §8 #5 决策是否进 v2）。
- 归零统一路径：inventory 移除 + 碎裂事件 → P2 表现；**charges 恒不为负**不变量。
- narration（player/perception）：使用「上古的东西醒了一瞬，又沉下去。」；碎裂「它终于碎了。碎得像早就该碎。」
- 测试：四类各自消费专属 case（含"学习失败不扣""非 Ancient 武器不扣"反例）、归零移除 + 背包格释放、charges 边界（1→0、连续使用）、交易/掉落后 charges 随实例保留。

## P2 — A/V + tooltip ⬜

- **使用反馈**：charges 递减瞬间——粒子 `BongSpriteParticle` ×8 burst，lifetime 10 tick，金褐 `#B49A5A`，速度 0.08 radial，复用 ancient 微光贴图（无则 `/gen-image particle` 补，跑不了则 `[BLOCKED: 需 /gen-image]` 占位）；SFX recipe `relic_charge_use.json`：layer1 `block.respawn_anchor.charge` pitch 0.5 vol 0.4，layer2 `block.ancient_debris.hit` pitch 1.3 vol 0.3 delay 2 tick。
- **碎裂**：粒子同色 ×20 burst + `BongGroundDecalParticle` 残屑贴地 40 tick；SFX `relic_shatter.json`：`item.shield.break` pitch 0.7 vol 0.6 + `block.amethyst_block.break` pitch 0.6 vol 0.5 delay 1 tick；HUD 事件流一条「遗物碎裂」。
- **tooltip**：剩余 charges 语义化「余 N 次」+ Ancient 专属描述行（client `ItemTooltipPanel` 既有 rarity 分支扩展）；`bong:vfx_event` 新 ID 注册进 `VfxBootstrap`（防孤岛）。
- 测试：VFX/SFX 注册断言、tooltip 各 tier 渲染分支、碎裂表现与移除同 tick 原子性。

## P3 — 饱和测试 + bot e2e ⬜

- bot 场景 `tsy_relic_use.py`：dev 给遗物剑 → 攻击 NPC → 断言伤害倍率生效 + charges 递减 payload → 耗尽 → 断言 inventory 移除 + 碎裂事件；Scroll/BeastCore 各一 leg。
- 1% roll 集成回归：种子固定跑 N 次 spawn，命中分布 pin（不动概率，只锁不回归）。
- 与 [[plan-lootcrate-v1]] 联动登记：talisman 变种若挂遗物钩子（其 §8 追加项），复用本 plan 的注册与消费管线，零重复实现。

## §8 开放问题（升 active / P0 决策门前收口）

1. **注册形态**：模板迁 `assets/items/*.toml`（单一真相源，推荐）vs Rust 表补 spec——TOML 化需核对 Ancient 专属字段（source_class/strength_tier）在 TOML schema 的表达；`AncientRelicPool::sample` 加权逻辑保留 Rust。
2. **消费事件形态**：新增 `RelicChargeConsumed` 内部 event（统一碎裂驱动）vs 各宿主系统直调共用 helper——取决于四类接线点分散度，P0 spike 定。
3. **BeastCore 助力的具体修正**：突破成功率 +X% vs 环境灵气门槛 -Y（如固元 0.80 门槛减免——等效"随身半个灵眼"，正典张力更强）；数值与 `plan-tribulation-balance-v1`（active）协调，不各拍各的。
4. **Weapon 倍率档位**：tier 2/3 相对同代凡兵/灵兵的倍率表；「3/5 次全力一击」的定位要不要限定只对 NPC/妖兽生效（防 PvP 一击必杀滥用）。
5. **Pendant v2 佩戴语义**：是否接 accessory 槽位数值（依赖装备槽体系现状）——v1 明确不做，此处只留决策记录。
6. **charges 显示的 wire 现状核验**：`InventoryItemViewV1.charges` 是否已随既有 payload 下发（`inventory/mod.rs:2235` 有比较引用，疑似已带）——P0 实地核对，缺则补 proto/samples。
