# plan-satiety-hydration-v1 — 玩家饱食度 + 水分双轴生存系统（塔科夫式）

> **状态：骨架（skeleton）**。转 active 前必须按 `docs/CLAUDE.md §五` 把 §8 开放问题收口成 §8.1 决议。
>
> 一句话主题：给玩家加「饱食度 + 水分」双生理轴（0–120，出生 80），五段生理带驱动体力恢复加速 / 虚弱 / 极度虚弱 / 缓慢掉血 / 过饱移动呕吐；参考塔科夫（Energy/Hydration）在 Inventory Tab 放双状态条；**存量食物全量迁移**为「使用后加减食/水数值」（可为负，如干粮扣水）。

## 阶段总览

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | server 底盘：`Nourishment` 组件 + 周期消耗 + 五段带判定 + 持久化 + dev 命令 | ⬜ |
| P1 | 生理效果接线：体力恢复乘数 / 虚弱减速 / 饥渴掉血（走 DeathEvent）/ 过饱移动呕吐（含 A/V） | ⬜ |
| P2 | 物品迁移：`NourishProfile` 字段 + food.toml 存量 5 食迁移 + 水囊实装 + 消费链接线 | ⬜ |
| P3 | schema + client：payload 扩展 + Inventory Tab 塔科夫双条 + 条件化 HUD 警示 + 屏幕效果 + 事件流 | ⬜ |
| P4 | A/V 收口回归 + bot e2e 场景 + 数值校准 | ⬜ |

## 接入面（docs/CLAUDE.md §二 checklist）

- **进料**：
  - `inventory` 物品消费链：`network/cast_emit.rs:679 apply_cast_item_effect`（现有吃食入口，`consume_food` freshness 判定保留）
  - `movement`：`Position`/`OldPosition` 位移 delta + `MovementState.action == Dashing`（`movement/mod.rs:106`）做移动/冲刺检测
  - `CombatClock` 节拍（stamina/bleed/status 同源时钟）；周期消耗仿 `shelflife/sweep.rs:30` 的 `is_multiple_of(N)` 门控
- **出料**：
  - 体力：给 `stamina_tick`（`combat/lifecycle.rs:272`）/ `sync_stamina_regen_from_realm`（`movement/mod.rs:179`）注入恢复乘数
  - 移速：`speed_multiplier_with_factors`（`movement/mod.rs:827`，已有低体力减速分支）加饥渴惩罚因子
  - 掉血：仿 `wound_bleed_tick`（`combat/lifecycle.rs:171`）范式改 `Wounds.health_current` 并 emit `DeathEvent { cause: "starvation" / "dehydration" }`——**不直写绕过死亡链**
  - 状态效果：emit `ApplyStatusEffectIntent`（`combat/events.rs:190`）复用既有 `StatusEffectKind::Slowed`（呕吐后短减速，见 §3）
  - client：扩 `CombatHudStateV1` 推双轴值 → `StatusBarsPanel` 双条 + planner 屏幕效果 + `event_stream_emit.rs:123` 事件流提示
- **共享类型 / event**：复用 `Wounds`/`DeathEvent`/`StatusEffects`/`ApplyStatusEffectIntent`/`StatusEffectKind::Slowed`/`CombatHudStateV1`/`EventStreamPushV1`；新增 `Nourishment` 组件 + `NourishBand` 枚举 + `NourishProfile` 物品字段。**近义重名声明**：`npc/hunger.rs::Hunger` 是 NPC AI need（`With<NpcMarker>` 门控），`fauna::ZoneBeastHungerTracker` 是 zone 级野兽融合计数——两者与玩家生理轴语义不同，**不复用不合并**，命名取 `Nourishment` 避混淆。
- **跨仓库契约**：server `schema/combat_hud.rs::CombatHudStateV1` 加 `satiety`/`hydration` 字段 → proto 同步 → `agent/packages/schema/samples/*.json` 双端 sample 对拍 → client `ProtoServerDataBridge`/`CombatHudStateHandler` 解析。agent 侧只透传不消费（世界状态摘要可后续接天道，非本 plan scope）。
- **上游 plan 血缘**：本 plan 叠加于 `plan-food-v1`（灵食与陈化，finished）之上——存量 5 食、`ItemEffect::FoodRegen`、`consume_food` freshness 链路均是其交付物，本 plan 只在旁边加食水数值结算，不改其行为；与 `plan-survival-gate-v1`（砍原版 HUD）不冲突——vanilla 饥饿条早已被 `MixinInGameHud` cancel，本 plan 双条在自研 Inventory Tab 面板内。
- **worldview 锚点**：`worldview.md §十 资源与匮乏`——末法凡躯离不开食水，匮乏是基调；本世界**无辟谷设定**（全文无命中），化虚也要吃饭，正合「末法去上古」。若需正典补一句「凡躯饮食」措辞，走人工单独 PR（见 §8 #6），本 plan 不动 worldview.md。
- **qi_physics 锚点**：**零真元流动**。饱食/水分是生理量不是灵气量，衰减常数是生理常数不是真元物理常数——统一 `NOURISH_*` 前缀命名（刻意避开 `*_DECAY*`/`*_DRAIN*` 红旗 grep 面），集中在 `server/src/nourishment/mod.rs` 顶部。现有 `FoodRegen → CultivationAcceleration`（守恒安全，只改 regen 乘数）链路**原样保留**，本 plan 只在其旁边加食水数值结算，不碰任何 `qi_current`/`zone.spirit_qi`。

## §1 核心模型

新模块 `server/src/nourishment/`（`mod.rs` 组件+常数、`tick.rs` 周期消耗、`effects.rs` 生理效果聚合、`vomit.rs` 过饱呕吐）。

```rust
/// 双生理轴，范围 [0.0, 120.0]，出生/复活默认 NOURISH_SPAWN_VALUE = 80.0
#[derive(Component, Serialize, Deserialize)]
pub struct Nourishment { pub satiety: f32, pub hydration: f32 }

pub enum NourishBand { Overfull, Comfort, Weak, Sapped, Critical }
pub fn band_of(value: f32) -> NourishBand   // 阈值见下表，边界闭开约定在测试里逐点 pin
```

**五段生理带**（两轴共用阈值；用户拍板的 120/100/80/60/40/20 数值线）：

| 带 | 区间 | 体力恢复乘数 | 附加效果 |
|------|------|------|------|
| 过饱 Overfull | (100, 120] | ×1.25 | 移动累积反胃值 → 呕吐（§3） |
| 舒适 Comfort | (60, 100] | ×1.0 | 80 为最舒适锚点 = 出生/复活默认值 |
| 虚弱 Weak | (40, 60] | ×0.6 | 入带事件流提示 |
| 重虚 Sapped | (20, 40] | ×0.3 | 移速 ×0.92 + 屏幕去色（§6） |
| 濒竭 Critical | [0, 20] | ×0.15 | 移速 ×0.85 + 持续掉血（§2），入带强提示 + 暗色 vignette |

**双轴合成规则**：体力恢复乘数 = 两轴乘数之积 clamp `[0.10, 1.5]`；移速惩罚取两轴较小值；掉血两轴叠加（又饿又渴死得更快）。

**周期消耗**（`tick.rs`，仿 `shelflife/sweep.rs` 每 200 tick = 10s 扫一次）：

- `NOURISH_SATIETY_LOSS_PER_MIN = 0.8`、`NOURISH_HYDRATION_LOSS_PER_MIN = 1.2`（静止基准；80→20 分别约 75 / 50 分钟，塔科夫式「水比饭掉得快」）
- 活动乘数：该 sweep 窗口内有水平位移 ×1.5，`MovementState.action == Dashing` ×3.0
- clamp 到 0 不为负；数值全部标「§8 #5 待校准」，P4 按 playtest 调

## §2 饥渴掉血（P1）

仿 `wound_bleed_tick`（`combat/lifecycle.rs:171`）新写 `nourishment_starve_tick`（每 20 tick），**每轴独立结算**：饱食轴处于 Critical 扣 `NOURISH_STARVE_HP_PER_SEC = 0.04`，水分轴处于 Critical 扣 `NOURISH_PARCH_HP_PER_SEC = 0.08`，仅当**两轴同时 Critical** 才叠加为 0.12/s（对应 §1「掉血两轴叠加」——单轴时只扣该轴自己的速率），扣 `Wounds.health_current`，跨死亡线时 `deaths.send(DeathEvent { cause: "starvation" | "dehydration", attacker: None })` 进 `death_arbiter_tick` → NearDeath 正常链路。**禁止**绕过 `DeathEvent` 直接判死。掉血速率相对 `health_max` 的实际标定（预期：纯饿死 Critical 满血起 ≥8 分钟、纯渴死 ≥4 分钟）在 P1 落地时按真实 `health_max` 换算并 pin 测试。

## §3 过饱呕吐（P1，机制 + A/V 同交付物）

`vomit.rs`：任一轴 Overfull 时，每 tick 比较 `Position` vs `OldPosition` 水平位移 > 0.05 格判「在移动」，反胃值 `nausea` +1.2（Dashing +3.0）；静止时 nausea 每 tick −0.5 回落。`nausea ≥ 100` → 呕吐结算：

- `satiety −15`、`hydration −10`（吐了白吃），nausea 清零，`NOURISH_VOMIT_COOLDOWN_TICKS = 200` 冷却
- 挂 `StatusEffectKind::Slowed`（**复用既有通用减速变体**，magnitude 0.5 = 移速减半，duration 60 tick）走 `ApplyStatusEffectIntent` 标准链——不新增 `Nausea` 变体：`Slowed` 已是 magnitude 参数化通用 debuff（sword_path/alchemy/npc/zhenfa 等 15+ 处复用），另造近义变体是命名红旗；若后续呕吐需要独立 HUD 图标语义再评估拆分
- 事件流：「胃里翻江倒海，你把刚吃下的东西吐了个干净。」（player scope，World 频道，Normal 优先级，perception）

**呕吐 A/V 规格**（视听与机制同阶段交付，不后置）：

- **粒子**：`BongSpriteParticle`，burst 模式一次 14 枚，lifetime 12–18 tick，初速沿玩家视线前下方 0.15–0.3 格/tick 锥形散布，颜色 `#7A8F3C` / `#5C6E2E` 交替，贴图新增 `vomit_chunk`（4×4 斑点两帧），`bong:vfx_event` ID `vomit_burst`，client 侧 `VomitBurstVfxPlayer`
- **音效**：audio_recipe `vomit.json` 三层——L1 `entity.player.burp` pitch 0.6 vol 0.8 delay 0；L2 `entity.slime.squish` pitch 0.65 vol 0.9 delay 2；L3 `block.pointed_dripstone.drip_water` pitch 0.7 vol 0.5 delay 5
- **动画**：`gen_vomit.py` 产 PlayerAnimator JSON——torso.pitch 0→0.55rad（6 tick，easeOutQuad）→ 保持 8 tick → 回正 6 tick，head.pitch +0.3rad 同步，body.z 前移 0.05；endTick 20 处**所有用到的 axis 补同值关键帧**（PlayerAnimator 循环衰减库坑 #1）
- **HUD**：`screenTint` `#4A5D2A` opacity 0.22，fade-in 10 tick / fade-out 15 tick，VISUAL 层（仿 `TiandaoPresenceHudPlanner`）

## §4 P0 — server 底盘

- `server/src/nourishment/{mod,tick,effects,vomit}.rs`：`Nourishment` / `NourishBand` / `band_of` / 常数表；sweep system 注册进主 schedule
- **持久化**：仿 `DigestionLoad` 模板（`cultivation/poison_trait/components.rs`）——`persist_player_cultivation_bundle`（`persistence/mod.rs:6153`）bundle 加 `"nourishment"` 键；`cultivation/mod.rs:910` hydration 段加解码块；autosave query 带上组件；转世/复活重置 80/80（§8 #3）
- **dev 命令**：`/nourish set satiety|hydration <value>`、`/nourish show`（brigadier，dev-only 直写绕过消耗，对齐 `/qi set` 模式；CLAUDE.md dev 命令表更新交人工，本 plan 不改 CLAUDE.md）
- **饱和测试**：五带边界逐点 pin（120.0/100.0/100.01/60.0/40.0/20.0/0.0，含 off-by-one）、消耗 clamp 0、活动乘数、持久化 roundtrip、断线重连保留、转世重置、`band_of` 全带专属 case

## §5 P2 — 物品迁移（塔科夫式食水数值，存量全迁）

`ItemTemplate` 加正交字段（**不占用** `[item.effect]` 槽，与既有 buff 共存）：

```toml
[item.nourish]
satiety = 12.0      # 可为负（如烈酒/干粮脱水）
hydration = 15.0
```

`inventory/mod.rs` 加 `NourishProfile { satiety: f32, hydration: f32 }` 解析（`parse_item_effect` 旁新 `parse_item_nourish`）；`apply_cast_item_effect`（`cast_emit.rs:679`）在既有 effect 结算旁加食水入账：`value = clamp(value + delta, 0, 120)`——**允许吃过 100 顶到 120**（过饱是玩家自己的选择，超 120 部分浪费；§8 #4）。

**存量 food.toml 5 食迁移基线**（数值 §8 #5 校准；干湿分明，参考塔科夫）：

| 物品 | satiety | hydration | 既有 effect |
|------|------|------|------|
| 熟肉 | +35 | −3 | 保留 |
| 陈饼 | +25 | −8（干噎） | 保留 |
| 灵果 | +12 | +15 | 保留 `food_regen 0.20` |
| 陈酒 | +3 | +18 | 保留 |
| 陈醋 | +2 | +10 | 保留 |

**水囊实装**：`water_skin`（现为 `workbench_materials.toml:260` 材料）加「满水囊」`water_skin_filled` 真实物品（`ItemCategory::Liquid`）：`hydration +55`，用后退回空 `water_skin`。注意：该 id 目前**仅**出现在过滤器单测的内存 fixture（`inventory/mod.rs:15134` `test_template(...)`，非生产注册）——P2 需**从零接线**（TOML 注册 + 消费链 + icon），不存在"落地即接活"的捷径，命名沿用该 id 只为与测试语汇一致。灌装交互（对水方块右键 C2S）见 §8 #2。新物品 icon 走 `/gen-image item`（跑不了则 `[BLOCKED: 需 /gen-image 生成 water_skin_filled.png]` + 占位接线）。

**测试**：nourish 字段解析正反 sample、负值扣减、120 截断、空/满水囊转换、5 存量食物逐项 pin、无 nourish 字段物品行为不变（向后兼容）。

## §6 P3 — schema + client（塔科夫双条进 Inventory Tab）

- **payload**：`CombatHudStateV1`（`schema/combat_hud.rs`）加 `satiety_percent: f32` / `hydration_percent: f32`（**沿用该 struct 既有 `_percent` 归一惯例**（`hp_percent`/`qi_percent`/`stamina_percent` ∈ [0,1]），= 值/120；client ×120 还原显示 `82/120`，band 阈值对应 100/120≈0.833、60/120=0.5、40/120≈0.333、20/120≈0.167，band 由 client 从值推导）；emit 侧 `combat_hud_state_emit.rs:41` filter 加 `Changed<Nourishment>`；proto + `agent/packages/schema/samples/*.json` 同 PR 改（wire 契约三件套不拆分）
- **client 桥接**：复用 `combat_hud_state` payloadCase（无新 case），`CombatHudStateHandler` 解析新字段进 `CombatHudStateStore`
- **Inventory Tab 双条**（塔科夫 Energy/Hydration 式）：`StatusBarsPanel.java`（`InspectScreen` 内既有境界/真元/体魄条面板）复用 `drawBar()` 加两行——食条 `#C9822E`（>100 段渐亮 `#E0B040` 警示）、水条 `#2E86C9`（>100 段 `#7FD0E8`），条旁数值 `82/120`，条头 16×16 icon（食=粗陶碗、水=水滴，`/gen-image` 产出）
- **条件化 HUD 警示**（对齐「HUD 沉浸式极简 + Conditional Display」）：常驻 HUD **不加**新条；仅当任一轴 ≤40 才在左下状态小控件旁浮现对应警示小图标，≤20 图标闪烁（20 tick 周期）。恢复 >40 即消失
- **屏幕效果**（仿 `TiandaoPresenceHudPlanner.edgeVignette`）：Sapped 带起 desaturation 0.35 渐入；Critical 带 `edgeVignette` `#2E1A10`，opacity 随值 20→0 线性 0.15→0.35，fade 曲线 easeInOut 20 tick；新 `NourishmentHudPlanner` 读 store 产出命令
- **事件流入带提示**（`push_to_client_priority`，player scope，World 频道；带间迁移各触发一次，同带不重复刷）：
  - 饱食入 Weak：「腹中空空，手脚渐渐使不上力。」（perception）
  - 水分入 Sapped：「喉咙干裂，眼前阵阵发黑。」（perception）
  - 任一入 Critical：「再不吃点喝点，命就要交代在这了。」（perception，Priority High）
- **测试**：proto 新字段正反 sample 对拍、band 推导边界、planner 命令快照（各带 vignette/图标状态）、双条渲染 smoke

## §7 P4 — 收口回归

- bot e2e：`scripts/bot/scenarios/survival_nourishment.py`（仿 `cultivation_pill_consume.py` 结构）——`give` 熟肉 → intent 吃 → `wait_for` `combat_hud_state` satiety 上升；`/nourish set satiety 15` → 断言掉血 + 事件流；`/nourish set satiety 115` + 移动 → 断言呕吐（satiety 回落 + `vomit_burst` vfx event）；水囊满→空转换
- A/V 差异化回归：呕吐动画/粒子/音效三件在真机可辨；双条/警示图标/vignette 各带截图核对
- 数值校准：一轮真机 playtest 核 §1/§2 表（消耗节奏、掉血致死时长、呕吐触发手感），校准结果回写本 plan 数值表
- 全栈门禁：server `cargo fmt/clippy/test`、client `./gradlew test build`、schema `npm test`、`bash scripts/smoke-test-e2e.sh`

## §8 开放问题（P0 决策门前需收口）

1. **境界是否降低消耗**：worldview 无辟谷，但高境界体魄强、耗得慢是否成立？**建议**：每境界 −5% 消耗、封顶 −25%（化虚也得吃饭，符合末法基调），实施为 `sync` 系 system 里按 realm 查表。
2. **水源灌装交互**：对水方块右键灌水囊需要新 C2S intent + 准星检测（仿 `NpcEngagementIntentHandler → C2S` 模式，不走 vanilla）。**建议**：本 plan 只做「物品消费」闭环，灌装挂 P4 末尾可选或滚 v2——但必须在 §8.1 明确拍板，不留悬空。血谷等污染水域喝了应有代价（接 `DigestionLoad` 毒性？）一并拍。
3. **死亡/转世重置值**：**建议**：复活/转世统一重置 80/80（最舒适锚点），濒死自救成功不重置。
4. **过饱进食策略**：>100 后还能不能继续吃？**建议**：允许吃到 120 截断（塔科夫式，过饱是玩家自担风险），不做 100 禁食门。
5. **数值校准归属**：§1 消耗速率、§2 掉血速率、§5 物品数值全部标定为初值，P4 playtest 后统一校准；校准只改常数不改结构。
6. **worldview 是否补锚**：§十 资源表无「食物」条目。**建议**：补一句「凡躯饮食」入 §十（人工单独 PR，本 plan 不动 worldview.md）；不补也不阻塞——匮乏基调已覆盖。
7. **NPC 是否共用**：`npc/hunger.rs` 是否迁移到 `Nourishment`？**建议**：不迁，NPC AI need 与玩家生理轴目标不同（NPC 要的是 utility scorer 输入），另立 plan 再议。
8. **呕吐动画与现有动作互斥**：呕吐动画播放期间 cast/格挡是否打断？**建议**：呕吐不打断硬动作，只叠加 `Slowed` 减速——保持简单，§8.1 时核 `PlayerAnimator` 通道占用现状再定。

## §10 实施工作流（scope = 5 PR，按 docs/CLAUDE.md §六）

- **PR 拆分**（依赖序，前一 merge 后开下一）：PR-1 = P0 底盘；PR-2 = P1 生理效果 + 呕吐（含 server 侧 A/V emit）；PR-3 = P2 物品迁移；PR-4 = P3 schema+client（wire 三件套同 PR）；PR-5 = P4 回归 + bot 场景 + 校准 + 归档
- **每 PR 独立 subagent 实施**（context 隔离按 §6.4，强制配置显式落定）：`Agent(subagent_type: "claude", model: "opus", prompt: "...本 PR 范围 + 测试要求...\n\nultrathink")`——主线只调度不亲跑，subagent 只实施 + 提 PR 不等 review
- **CR 等待协议**：按 §6.5 `ScheduleWakeup` 节奏，修完意见重等 re-review
- **纯逻辑 commit 常规 atomic；无 NBT/layout 资产**，`<PROMISE>` 三轮打磨条款不适用（icon 走 `/gen-image` 批量豁免）
- 用户 `/consume-plan` 后全自动到 merge；醒来看 `finished_plans/` 是否有本 plan
