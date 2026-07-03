# plan-ambient-threat-v1 — 环境威胁刷新：把 danger_level 接活

> **一句话主题**：新增按 zone `danger_level` 驱动的周期性环境威胁 spawner（每 zone 威胁预算 + 距玩家距离环 + 超距回收），物种池按危险度分层（低危鼠群袭扰 → 高危主动妖兽 → 噬灵域接活 tsy_hostile 现成敌对），并给 rat 加低威胁袭扰行为——让 spawn 附近有零星威胁、往 north_wastes 走压迫感陡增，填上"末法世界没有外部威胁"的体验空洞。

**状态**：Active。升 active 2026-07-03（§8 六条开放问题已收口，见 §8.1；R1/R2/R3 三处设计错误已在 Explore 核验后修正）。各阶段实施状态仍为 ⬜（收口只是解锁 P0，未开始实施）。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | ambient spawner 核心——danger 预算 / 距离环 / 密度上限 / 回收 | ⬜ |
| P1 | 物种池分层——danger 1~7 分层表 + tsy_hostile 自然涌现接活 | ⬜ |
| P2 | rat 袭扰行为——低威胁骚扰 AI + 咬击偷 qi（守恒）+ 视听 | ⬜ |
| P3 | 生态联动——兽潮门槛改造 / 负灵域威胁加成 / 与 horde 迁徙衔接 | ⬜ |

---

## 背景诊断（2026-07-02 起草，2026-07-03 Explore 核验修正）

玩家在主世界见不到任何**周期性 ambient**威胁刷新，根因是"有威胁触发链路但全挂特殊事件，唯独没有常驻调度器"：

1. **没有 ambient 敌对 spawner**：历史 zombie 定时刷新器已删未补（`server/src/npc/spawn/mod.rs:192` 注释 "PostStartup zombie spawn 已移除"）。现存威胁 spawn 全挂特殊触发：兽潮实为两条入口——**主入口** `maybe_queue_beast_tide`（`world/heartbeat.rs:661` 无条件调用 / `:1350-1439` 定义）按 zone `spirit_qi < 0.15` 持续累计满 `BEAST_TIDE_LOW_QI_REQUIRED_TICKS`（5 分钟，`low_qi_ticks_by_zone` 计时）**独立触发，不依赖任何灵脉/秘境塌缩事件先决**（`BEAST_TIDE_LOW_QI_THRESHOLD=0.15`，常数定义于 `heartbeat.rs:55-56`）；**次入口**挂 `PseudoVeinDissipated` 邻域扩散分支（`heartbeat.rs:716-750`，阈值判定在 `:733`），塌缩/秘境事件是这条次入口的触发源而非主入口的前置条件；植物招怪要玩家亲手采 `AttractsMobs` 植物（`botany/hazard.rs:228`）；tsy_hostile 全套敌对（道伥/执念/畸变体/skull_fiend/守灵）**只有 `/tsy_spawn` dev 命令入口**（`npc/tsy_hostile.rs:561`）；黑武士限 `giant_sword_sea`。
2. **`danger_level` 不是死字段，但无刷怪系统读它**——措辞修正（Explore 核验，R1）：zones.json 28 个 zone 全标了 danger 1~7（spawn=1，north_waste_east_scorch=7），**已有 4 处真实读取消费**：
   - `world/heartbeat.rs:938` 深区采集判定（`zone.danger_level >= DEEP_GATHERING_DANGER_LEVEL`(=3)）
   - `world/heartbeat.rs:944` 归途安全路线判定（`zone.danger_level <= RETURN_ROUTE_DANGER_LEVEL_MAX`(=1)）
   - `world/heartbeat.rs:1805`（`tide_sky_omen_anchor`）兽潮天象选最安全 zone 锚点（`min_by_key(danger_level, ...)`）
   - `server/src/movement/mod.rs:851` 死域口径：`zone.danger_level >= 5 && zone.spirit_qi <= 0.1` → `MovementZoneKind::Dead`

   写入侧：`command_executor.rs:1109`（era 政令改写）/ `world/events.rs:2810`（realm collapse 写 `COLLAPSED_ZONE_DANGER_LEVEL=5`）/ `world/events.rs:1306`（`.max(4)`）。本 plan 要接的缺口精确说是——**没有刷怪系统读它**，字段本身活跃于导航/安全判定语义。P3"负灵域威胁加成"必须直接对齐 `movement/mod.rs:851` 已有死域口径，不造第二套"死域"定义（§8.1 #4）。
3. **rat 的 `SeekQiSourceAction` 现已对玩家生效**：`rat_npc_thinker`（`npc/spawn_rat.rs:52-61`）无条件挂 `QiSourceProximityScorer → SeekQiSourceAction`（不受兽潮态门控），其目标查询 `QiSourceTargetQuery`（`npc/brain_rat.rs:37-48`）过滤器实为 `Without<NpcMarker>`（不是 `With`），命中所有持有 `Cultivation` 的非-NPC 实体（即玩家修士）——近距即 emit `RatBiteEvent`（`seek_qi_source_action_system`，`brain_rat.rs:212-255`），单测 `seek_qi_source_action_triggers_rat_bite_at_close_range`（`brain_rat.rs:432-474`）已实证会命中该类目标并触发咬击；此外兽潮态另有独立的打坐修士咬击（`LOCUST_CULTIVATOR_BITE_RADIUS=6.0`, `LOCUST_BITE_QI_STEAL=1`, `world/events.rs`）。P2 守恒路径已现成（`combat/rat_bite.rs:25-73` `RatBiteEvent` + `apply_rat_bite_qi_drain`），零风险，见 P2 段落。

**交叉预警**：`plan-zone-qi-economy-v1`（active，落地序先于本 plan P3，见 §8.1 #5）P1 会把 spawn 稳在 equilibrium 0.35——qi 经济落地后，现存唯一自然威胁入口（兽潮 qi<0.15）被彻底焊死。威胁刷新必须改以 danger_level 为主驱动，这正是本 plan 的立项动因之一。

## 接入面（docs/CLAUDE.md §二 checklist）

- **进料**：`Zone.danger_level`（`world/zone.rs:25-36`，zones.json，已被 heartbeat/movement 4 处消费——见背景诊断 #2）；调度器模板 `heiwushi_natural_spawn_system`（`npc/heiwushi_spawn.rs:42`，state resource + 节流 + marker 活体计数 + 冷却 + 玩家在场半径门 + 维度过滤全套骨架，fork 而非从零写）；距离环采样器 `PoissonSpawnSampler::adaptive_for_zone`（`npc/spawn/mod.rs:73`）；节流 helper `should_run_interval`（`npc/dormant/mod.rs:712`）；现成 spawn 函数 `spawn_beast_npc_at`（`npc/spawn/beast.rs:51`）/ `spawn_rat_npc_at`（`spawn_rat.rs:63`）/ `spawn_natural_mob_at`（`world/mob_spawn.rs:54`）/ `spawn_tsy_hostiles_for_family`（`tsy_hostile.rs:561`，**直调，不经 `TsySpawnRequested` 事件**，见 §8.1 #3）；时代密度门 `era_beast_spawn_gate`（`mob_spawn.rs:117`）；`CultivationClock`；玩家 Position 查询。
- **出料**：真实 ECS beast/rat/tsy 实体（走现有 thinker/掉落/骨币链，fauna-v1 正典）；死亡→`plan-fauna-v1` 掉落表；agent `world_state` 的 npc_count 自然反映威胁密度；P2 咬击走 `QiTransfer`。
- **共享类型 / event**：复用 `NaturalMobKind`（`world/mob_spawn.rs:12-19`，6 变体已完备，P1 物种池分层按此枚举取子集）/ `BeastKind`（`fauna/components.rs:9-33`，17 变体，覆盖醒灵~化虚战力分层）/ `FaunaVisualKind`（`fauna/visual.rs:34-55`，20 变体，client 视觉 shell 映射）——三类型定义位置各自独立，非同一枚举的三个别名。**P0/P1 不新增任何实体种类**，只造调度器。**不**复用 `TsySpawnRequested` 事件链——该事件是 TSY 子域**创世**事件（注册 subzone + portal + LootContainer，带幂等锁），周期性 re-emit 会触发首次重建整座地牢/后续被幂等锁拒绝一只不刷（R2，见 §8.1 #3），TSY 接活改为直调 `spawn_tsy_hostiles_for_family`。新建调度核模块命名为 **`server/src/npc/spawn/ambient_scheduler.rs`**（非 `ambient_threat.rs`）——参数化为通用调度核（`pool_fn` / `budget_fn` / `marker_type` / `counts_against_threat_budget: bool`），供 `plan-mundane-fauna-v1` 复用同一套"距离环 24~64 + 密度上限 + 超 96 格 `insert(Despawned)` 回收"基建（该 plan 挂被动 fauna pool，不占威胁预算，见 §8.1 #1/#2 跨 plan 共享基建归属）。
- **跨仓库契约**：纯 server plan。无新 payload/schema/Redis key（threat 实体走既有 NPC 下发链路，client 零改动）。
- **worldview 锚点**：worldview §四 战力分层（妖兽/经脉损伤体系）+ `plan-fauna-v1` / `plan-tsy-hostile-v1`（finished，正典物种与掉落）+ worldview §一:22（"死域连野兽都活不了"）+ §七:759（负灵域野兽材质枯萎化飞灰）覆盖「灵气枯竭生异变」语义，**本 plan 不新开 worldview 条目**（§8.1 #4 已收口，倾向复用不触红线；若后续判定仍需补「危险度地理分布」明文，单独起 PR 人工 review）。
- **qi_physics 锚点**：spawner 本身不动灵气。P2 rat 咬击偷 qi **复用兽潮咬击同款守恒实现**——`combat/rat_bite.rs:25-73` 的 `RatBiteEvent` + `apply_rat_bite_qi_drain` 已是完整 `QiTransfer`（`QiTransferReason::RatBiteDrain`）路径，P2 只需 emit `RatBiteEvent{target=player}` 复用全链，不自拍新常数、不新写扣减公式。

---

## P0 ambient spawner 核心 ⬜

新模块 **`server/src/npc/spawn/ambient_scheduler.rs`**（通用调度核，非威胁特化命名——R3/§8.1 收口），`register` 挂进 `npc/mod.rs:64` 序列（紧邻 `heiwushi_spawn::register(app)` 之后插入，对齐现有 plugin 注册顺序防孤岛）：

- **实现模板**：fork `heiwushi_natural_spawn_system`（`npc/heiwushi_spawn.rs:42-115`）的结构，**不从零写**——state resource（`last_check_tick` 节流字段）+ marker 活体计数 query + 冷却判定 + 玩家在场半径门 + 维度过滤（`heiwushi_spawn.rs:100-110` 的 `DimensionKind::Overworld` 过滤模式）全套骨架直接复用，替换掉"单一 boss 判活"为"按 zone/danger 分组计数"。
- **泛型可注入**（防孤岛核心，与 `plan-mundane-fauna-v1` 共同地基）：调度核参数化为 `(pool_fn, budget_fn, marker_type, counts_against_threat_budget: bool)`，把"距离环采样 + 密度上限 + 超距 `Despawned` 回收"抽成与"物种池 / 威胁预算"解耦的通用核心，使 mundane-fauna 的被动 pool 能挂同一套调度核而不重复实现基建、且 `counts_against_threat_budget=false` 时不占本 plan 的威胁预算表。
- **威胁预算**：每 zone 按 `danger_level` 查预算表（`fn threat_budget(danger: u8) -> ThreatBudget { max_alive, spawn_interval_ticks, pack_size_range }`，数值见 §8.1 #1：danger1→max_alive 1-2 / danger4→3-5 / danger7→8-10，spawn_interval 用 `should_run_interval`（`dormant/mod.rs:712`）驱动，danger1→~600 tick / danger7→~150 tick）；zone 内存活 ambient 威胁计数 ≥ `max_alive` 则跳过。
- **距离环**：只在"有玩家在 zone 内"时刷；spawn 点取距最近玩家 **24~64 格**环带内 Poisson 备选点，直接调用 `PoissonSpawnSampler::adaptive_for_zone`（`npc/spawn/mod.rs:73-97`，按 zone 面积自适应 `min_same_archetype_dist`/`min_cross_archetype_dist`，不改此逻辑只 import）。
- **回收**：ambient 威胁带 `AmbientThreatMarker { spawned_at, home_zone }`；距所有玩家 > 96 格持续 N tick 或存活超上限时长 → despawn（**必须 `commands.entity(e).insert(Despawned)`**，Valence 层实体裸 despawn 会崩服——[[feedback_valence_despawn_layer_entity]]，对齐 `heiwushi_spawn.rs` 现有回收模式）。
- **门控**：`era_beast_spawn_gate` 时代倍率照吃；`REALM_COLLAPSE` 事件 zone 跳过（塌缩另有兽潮）；spawn 保护：danger=1 的 zone 预算给到"零星、非包围"档（1-2 而非 0，新手也要见到世界有牙）；预留 `weight_hook: Option<f32>`（默认 `None`）供未来昼夜/天气权重回填，本阶段不接实际昼夜逻辑（§8.1 #6）。
- **峰值闸门**：真实闸门是 `max_hydrated_count=200`（`dormant/mod.rs:141`，非 `max_dormant_count=5000`——后者是休眠快照上限，不占 ECS 实体）；per-zone 预算须保证跨所有同时段活跃 zone 求和 << 200（通常同时段活跃 zone 1-3 个，留 dormant hydration 余量，§8.1 #1）。
- **测试**：预算表边界（danger 0/1/7/未知 zone）；计数≥上限不刷；无玩家不刷；距离环 off-by-one；despawn 走 `Despawned` 断言（非裸 `.despawn()`）；era 倍率 clamp；泛型调度核参数注入（不同 `pool_fn`/`budget_fn`/`marker_type` 组合独立生效，`counts_against_threat_budget=false` 时不计入威胁预算计数）——为 mundane-fauna 复用铺路。

## P1 物种池分层 ⬜

`fn threat_pool(danger: u8, dimension: DimensionKind) -> &[ThreatEntry]`（entry = 物种 + 权重 + 群体大小），全部复用现有物种，**不新增变体、不加 buff**（§8.1 #2）：

| danger | 池（终表，§8.1 #2 收口，已按 `spawn_natural_mob_at` 实际调度路径订正） |
|---|---|
| 1~2 | rat 小群（2~4 只，中立袭扰档，见 P2） |
| 3~4 | 通用 beast（`NaturalMobKind::{Zombie,Skeleton,Creeper,Rogue,Daoxiang}` 走 `spawn_natural_mob_at`，`mob_spawn.rs:54,77-91`；主动） |
| 5~6 | 通用 beast（同上）+ AshSpider（死域白名单物种，`DEAD_ZONE_MOB_WHITELIST`，`mob_spawn.rs:30`；死域过滤走既有 `MobSpawnFilter::ban_in_dead_zone`，不新写判定） |
| 7 | 同 5~6 档物种池——**只调 pack_size 与 spawn_interval，不造新怪、不加 buff 修饰**（拒绝"精英" modifier 组件路线，见 §8.1 #2 边界） |

- **物种差异化边界订正（第二轮 Explore 核验，撤销原"逐种可辨"假设）**：`spawn_natural_mob_at`（`mob_spawn.rs:54`，match 分支 `:77-91`）对 `Zombie|Skeleton|Creeper|Rogue|Daoxiang` 五个变体统一走 `spawn_beast_npc_at`（`npc/spawn/beast.rs:51`），该函数签名不接收 `kind` 参数；落地实体的 `BeastKind`（进而其 `EntityKind`/视觉表现）由 `fauna_tag_for_beast_spawn(home_zone, fauna_seed)`（`fauna/components.rs:125` → `beast_kind_for_spawn_context`，`components.rs:301-340`）按 `home_zone` 字符串匹配 / zone qi 分档 + seed 内部派生，与调用方传入的 `NaturalMobKind` 完全无关。因此除 `AshSpider`（唯一走独立 `spawn_ash_spider_npc_at` 分支的例外）外，上表所有档位产出的实体**视觉/种类不可逐种可辨**——物种池分层实际只体现为 rat（danger 1~2）vs 通用 beast（danger 3~7，靠 pack_size/spawn_interval/danger 数值梯度而非逐种表达强度差异）vs AshSpider（死域白名单）三档。**不承诺** `Skeleton`/`Creeper`/`Rogue` 在游戏内呈现为可区分的骷髅/苦力怕/游荡者外观；若后续判定确需视觉逐种分层，须先把"让 `spawn_beast_npc_at`/`spawn_natural_mob_at` 接收并落实 `kind` 参数（取代当前纯 zone-name 派生的 `fauna_tag_for_beast_spawn`）"列为 P1 前置改造工作项，本 plan 现状不做该项、不复用现成物种池即得分层的措辞。

- **TSY 接活**（P1 内独立小节，设计已按 §8.1 #3 修正——建议实施时拆为 P1.5 独立小 PR，见 §10）：TSY dimension 内检测到已初始化 subzone（`TsyZoneInitialized` 已消费过）后，**直调 `spawn_tsy_hostiles_for_family`**（`npc/tsy_hostile.rs:561`），**禁止** 周期 re-emit `TsySpawnRequested`（该事件是创世事件带幂等锁，见 §8.1 #3）；限定只在 TSY 维度生效；持有独立 respawn 预算，与主世界 `threat_budget` 表分离（`counts_against_threat_budget=false` 挂在 P0 通用调度核上）。tsy_hostile dev-only 是**刻意**的（`plan-tsy-hostile-v1.md:789` Finish Evidence 遗留节明文"道伥喷出主世界行为归独立 plan"），ambient 直调是合法填缺，不改变该 plan 边界。
- **测试**：每 danger 档池命中专属 case；TSY 维度隔离（主世界不出道伥，直调路径只在 `DimensionKind::Tsy` 触发）；权重抽样分布 pin；TSY 路径**不 emit** `TsySpawnRequested` 的负向断言（防回归成 re-emit）。

## P2 rat 袭扰行为 + 视听 ⬜

rat 现有的 `SeekQiSourceAction`（`brain_rat.rs:37-48` `QiSourceTargetQuery` 的 `Without<NpcMarker>` 过滤器）已会主动索敌玩家并咬击——P2 的目标不是"填补对玩家的中立空白"，而是**复用/收编既有 `SeekQiSourceAction` 分支或与之互斥编排**表达"低威胁骚扰"：新增 `PlayerHarassScorer`（玩家 ≤ 8 格且冷却就绪时起评）→ `HarassBiteAction`（冲近咬一口 → emit `RatBiteEvent{rat, target: player, qi_steal: 1}` → 立即进逃逸/游荡冷却 20s）。`rat_npc_thinker` 用 `FirstToScore` picker（`spawn_rat.rs:53-56`），`PlayerHarassScorer` 与既有 `QiSourceProximityScorer` **必须互斥编排**（明确谁在 picker 序列中排前、避免同一目标被两条分支各自 emit `RatBiteEvent` 造成双倍咬击/冲突），不能简单并列两个都可能命中玩家的 Scorer。不伤血、不锁定追杀——是"烦人的末法鼠患"不是战斗怪；打坐时被咬会打断凝神（对齐兽潮咬击既有语义）。

**守恒路径零风险**（Explore 核验确认现成）：`combat/rat_bite.rs:25-73` 的 `apply_rat_bite_qi_drain` 已是完整 `QiTransfer`（`from: player`, `to: npc:rat`, `reason: RatBiteDrain`）实现；`brain_rat.rs` 的 `QiSourceTargetQuery` 过滤器实为 `Without<NpcMarker>`（`brain_rat.rs:37-48`），rat 现在已会主动索敌玩家并 emit `RatBiteEvent`（`seek_qi_source_action_system`，`brain_rat.rs:212-255`）——`PlayerHarassScorer`/`HarassBiteAction` 不是从零建攻击链，而是复用已有 `RatBiteEvent{target: player}` 发射路径，**不新写扣减公式**；实施时需按上段与既有 `SeekQiSourceAction` 分支互斥编排，避免同 `FirstToScore` picker 下双 Scorer 冲突同时命中同一玩家目标。

**视听（全复用既有原语，无新增资产）**：
- 粒子：咬中瞬间复用既有 qi 抽取类 VfxPlayer 事件（兽潮咬击现有表现；若无则 `BongSpriteParticle` 复用既有灰白 sprite，burst 4 粒、lifetime 8 tick、色 `#9B9B8C`、自咬点向上飘 0.03 b/t），经 `VfxBootstrap.registerDefaults()` 注册核对，漏注册=静默孤岛。
- SFX：`entity.silverfish.ambient` pitch 0.7 vol 0.5（咬击）+ `entity.rat` 无原版音，逃逸复用现有 rat 移动音（无则不加）。
- HUD：不加常驻元素；被咬走既有事件流（通用非战斗专用）出一条条目。
- narration：无（低威胁事件不值得天道开口）。
- **测试**：Scorer 距离/冷却边界；偷 qi 守恒对拍（玩家 -1 → 咬击者 +1 或按既有路径归宿）；打坐打断分支。

## P3 生态联动 ⬜

- **兽潮门槛改造**：`BEAST_TIDE_LOW_QI_THRESHOLD`（`heartbeat.rs:55-56`，值 0.15）驱动两条既有入口——主入口 `maybe_queue_beast_tide`（`heartbeat.rs:661` 调用 / `:1350-1439` 定义，按 `low_qi_ticks_by_zone` 累计满 `BEAST_TIDE_LOW_QI_REQUIRED_TICKS` 独立触发，不依赖塌缩事件）、次入口挂 `PseudoVeinDissipated` 邻域扩散分支（`heartbeat.rs:716-750`，阈值判定在 `:733`）。改双因子（qi 持续低位时长或塌缩/扩散事件）× danger 加权——阈值常数保留在 `heartbeat.rs` 不动，新增的 danger 权重因子放在 ambient 模块内独立计算后分别与两条入口的既有判定组合。**落地顺序**：`plan-zone-qi-economy-v1` P1（equilibrium=0.35）先行落地，再启动本 P3——equilibrium(0.35) > 阈值(0.15) 意味着经济落地后单靠 qi 阈值兽潮入口被焊死，需先确认 zone-qi 落地后的真实 equilibrium 数值再收口双因子加权系数（跨 plan 依赖，非"带着开放问题进 P0"——公式结构已锁定，仅加权系数留给依赖 plan 落地后填，且 P3 非 P0 范围，§8.1 #5）。
- **负灵域/死域加成**：直接对齐 `movement/mod.rs:851` 已编码的死域判定口径——`zone.danger_level >= 5 && zone.spirit_qi <= 0.1`（或 `REALM_COLLAPSE` 活跃事件）→ 判定为死域，威胁预算在此类 zone 乘区放大。**不新造第二套"死域"定义**；正典依据 worldview §一:22（"死域连野兽都活不了"）+ §七:759（负灵域野兽材质枯萎化飞灰），不新开 worldview 条目（§8.1 #4）。
- **horde 衔接**：ambient 刷出的常驻 beast 使 `beast_horde_detect_system`（`fauna/migration.rs:388`）的 `beast_count==0` 短路自然消失，迁徙系统开始有料可迁——**只声明衔接，不吞 `plan-beast-horde-v1` P2 领地争夺 scope**。
- **测试**：双因子门槛矩阵（qi 骤降速率单独触发 / collapse 事件单独触发 / 两者都不满足三态）；负灵域乘区对齐 `movement_zone_kind` 断言；ambient 存量触发迁徙的集成 case。

---

## §8 开放问题（升 active / P0 决策门前收口）

1. **预算表数值**：danger 1~7 各档 `max_alive / spawn_interval / pack_size` 具体值；服务器性能预算（与 dormant `max_dormant_count=5000` 及 hydrate 半径 64 共存时的实体峰值上限）。
2. **物种池终表**：P1 草案表逐档确认；danger 7 "精英"是否需要新 buff 修饰（倾向：只调 pack/间隔，不造新怪）。
3. **tsy_hostile dev-only 是否有意**：查 `plan-tsy-hostile-v1` Finish Evidence / 遗留节是否预留了自然涌现 Phase2 口子；若原设计刻意 dev-only（等某上游 plan），改为在此接活须写明理由。
4. **worldview 正典补充**：「危险度地理分布」「灵气枯竭生异变」是否需要 worldview 明文条目（若需，单独 PR 人工 review，本 plan 归档前 land）。
5. **兽潮双因子公式**：与 `plan-zone-qi-economy-v1` 的 equilibrium/inflow 数值联动收口（两 plan 若同期 active，先后顺序与共享常数归属）。
6. **昼夜/天气权重**：夜间加成是否引入（现无昼夜 spawn 概念；引入则 P0 加权重钩子，数值后调）。

**全部已在 §8.1 收口，原表留追溯，实施以 §8.1 为准。**

## §8.1 决议（pre-P0 收口，2026-07-03）

> Explore 核验（aa48）同时发现本 plan 骨架有 3 处需修正的设计错误（非单纯数值待定）：**R1** danger_level 措辞（已在头部/背景诊断修正，非死字段有 4 处消费）、**R2** P1 TSY 接活原设计会撞 `TsySpawnRequested` 幂等锁（已在 P1 块改为直调 `spawn_tsy_hostiles_for_family`）、**R3** P0 应 fork `heiwushi_natural_spawn_system` 而非从零写（已在 P0 块改写 + 泛型化为 `spawn/ambient_scheduler.rs` 供 `plan-mundane-fauna-v1` 复用）。以下 6 条决议是在此基础上对 §8 原表逐条收口。

### #1 预算表数值 + 服务器性能峰值闸门

**决议**：
1. 真实峰值闸门是 `max_hydrated_count = 200`（`dormant/mod.rs:141`），**不是** `max_dormant_count = 5000`——后者是休眠快照上限，快照不占 ECS 实体位，用它做闸门会把预算算宽 25 倍。
2. per-zone 预算表：danger 1 → `max_alive` 1-2、danger 4 → 3-5、danger 7 → 8-10；`spawn_interval_ticks` 用 `should_run_interval`（`dormant/mod.rs:712`）驱动，danger 1 → ~600 tick、danger 7 → ~150 tick；`pack_size_range` danger 1-2 → 2-4、danger 7 → pack 增大 + 更短间隔（非新增怪种，见 #2）。跨所有同时段有玩家在场的活跃 zone 求和须 `<< 200`——同时段活跃 zone 通常 1-3 个，即使各按上限取值求和也远低于 200，为 dormant hydration（半径 64）留出余量。
3. 拒绝直接照抄 `max_dormant_count=5000` 作为容量基准——那是不同语义的上限（休眠快照，非实时 ECS 实体），照抄会让 P0 预算表在多 zone 同时活跃时把服务器实体数顶穿 `max_hydrated_count`。

**落点**：`server/src/npc/dormant/mod.rs:135-145`（`max_hydrated_count=200` / `HYDRATE_RADIUS_BLOCKS` / `DEHYDRATE_RADIUS_BLOCKS`）+ `dormant/mod.rs:712`（`should_run_interval`）+ plan §P0「威胁预算」「峰值闸门」两段 / §8.1 #1

### #2 物种池终表

**决议**：
1. 全部复用现有 `NaturalMobKind` 6 变体（`Zombie/Skeleton/Creeper/Rogue/AshSpider/Daoxiang`），不新增任何变体、不加任何 buff 组件。
2. **物种差异化边界（第二轮 Explore 核验订正，撤销原「逐种可辨」假设）**：`spawn_natural_mob_at`（`mob_spawn.rs:54`，match 分支 `:77-91`）对 `Zombie|Skeleton|Creeper|Rogue|Daoxiang` 统一走 `spawn_beast_npc_at`（`npc/spawn/beast.rs:51`）——该函数不接收 `kind` 参数，落地实体的 `BeastKind` 由 `fauna_tag_for_beast_spawn(home_zone, fauna_seed)`（`fauna/components.rs:125` → `beast_kind_for_spawn_context` `:301-340`）按 zone 名称/qi 内部派生，与调用方传入的 `NaturalMobKind` 无关。因此实际物种池只有三档：1~2 档 rat；3~7 档「通用 beast」（`spawn_natural_mob_at` 五个非-AshSpider 变体产出实体不可逐种可辨，靠 pack_size/spawn_interval/danger 数值梯度表达强度差异）；5~7 档另加 `AshSpider`（唯一走独立分支的例外，死域白名单 `DEAD_ZONE_MOB_WHITELIST`，死域过滤直接调用既有 `MobSpawnFilter::ban_in_dead_zone`，不新写判定）。**不承诺** Skeleton/Creeper/Rogue 呈现为可区分的实体外观——撤销"复用现成物种池即得分层"措辞。与 `plan-mundane-fauna-v1` 的 `MundaneFaunaKind` 是独立 enum，零重叠；唯一交叉点是 T2+ 掠食者（狼/狐）是否计入 ambient 威胁密度——两 plan 需共用同一张 `max_hydrated_count` 峰值预算表（引用 #1），交叉点在两 plan 各自 P0 落地时对账。
3. 拒绝"danger 7 精英需要新 buff 修饰"路线——违反 §8/#2 已定的"不造新怪"红线；"精英感"完全通过 pack_size + spawn_interval 参数化表达，不引入新组件/modifier。
4. 若后续判定确需视觉逐种分层（如 Skeleton 呈现为骷髅、Creeper 呈现为苦力怕），须先把"让 `spawn_beast_npc_at`/`spawn_natural_mob_at` 接收并落实 `kind` 参数（取代当前纯 zone-name 派生的 `fauna_tag_for_beast_spawn`）"列为 P1 前置改造工作项——本 plan 现状不做该项，只如实收口为 rat（danger1-2）vs 通用 beast（danger3-7）vs AshSpider（死域白名单）三档。

**落点**：`server/src/world/mob_spawn.rs:12-19`（`NaturalMobKind` 枚举）+ `mob_spawn.rs:30`（`DEAD_ZONE_MOB_WHITELIST`）+ `mob_spawn.rs:54,77-91`（`spawn_natural_mob_at` 及其无 `kind` 透传的 match 分支）+ `npc/spawn/beast.rs:51`（`spawn_beast_npc_at` 签名不含 `kind`）+ `fauna/components.rs:125,301-340`（`fauna_tag_for_beast_spawn` → `beast_kind_for_spawn_context` 派生逻辑）+ plan §P1「物种池分层」表格 / §8.1 #2

### #3 tsy_hostile dev-only 是否有意 + P1 TSY 接活设计

**决议**：
1. dev-only 是**刻意**的——`plan-tsy-hostile-v1.md:789` Finish Evidence「遗留 / 后续」明文"道伥喷出主世界行为均归独立 plan"。ambient 接活是合法填缺，但骨架原设计（周期 emit `TsySpawnRequested` 仿 dev 命令）是设计错误：该事件是 TSY 子域**创世**事件（`apply_tsy_spawn_requests`，`world/tsy_dev_command.rs:167`，消费后注册 subzone + portal + LootContainer + 一次性 spawn），带幂等锁（同 `family_id` portal 已存在 → `AlreadySpawned` 拒绝，`tsy_dev_command.rs:190-205`）——周期 re-emit 首次会重建整座地牢，后续被幂等锁拒绝，一只不刷。
2. 正解：向已存在 TSY 域持续刷敌对，**直调 `spawn_tsy_hostiles_for_family`**（`npc/tsy_hostile.rs:561`），限定只在 TSY 维度生效，持有独立 respawn 预算（与主世界 `threat_budget` 表分离，`counts_against_threat_budget=false`）；禁止周期 re-emit `TsySpawnRequested`。因该路径依赖 TSY 专属状态（`TsyContainerSpawnRef`/`TsySpawnPoolRegistry`/`ZoneRegistry` 的 TSY 分支），耦合度显著高于主世界物种池，**建议实施时把 TSY 接活从 P1 拆出，后置为独立小 PR**（P1 先落地主世界 1~7 档物种池，TSY 部分单独排期，见 §10）。
3. 拒绝"仿 dev 命令周期 emit `TsySpawnRequested`"路线——该路线在真实运行中等价于 no-op（幂等锁挡光）且有零星重建地牢风险；不修改 `tsy_dev_command.rs` 既有幂等语义（那是另一 plan 的既定契约，本 plan 只读不改）。

**落点**：`server/src/world/tsy_dev_command.rs:167-205`（`apply_tsy_spawn_requests` 幂等锁）+ `server/src/npc/tsy_hostile.rs:561`（`spawn_tsy_hostiles_for_family` 签名）+ `docs/finished_plans/plan-tsy-hostile-v1.md:789`（遗留节佐证 dev-only 刻意）+ plan §P1「TSY 接活」小节 / §8.1 #3

### #4 worldview 正典补充

**决议**：
1. 「灵气枯竭生异变」已有正典锚点覆盖，**不新开 worldview 条目**；「危险度地理分布」非本 plan 强制项，倾向判定现有锚点已足够支撑本 plan 语义。
2. P3「负灵域/死域加成」直接对齐 `movement/mod.rs:851` 已编码的死域判定口径（`zone.danger_level >= 5 && zone.spirit_qi <= 0.1` 或 `REALM_COLLAPSE` 活跃事件 → `MovementZoneKind::Dead`），不造第二套"死域"定义；正典依据 worldview §一:22（"死域连野兽都活不了"）+ §七:759（负灵域野兽材质枯萎化飞灰）。
3. 拒绝在本 plan 内新写 worldview 条目——本 plan 判定现有正典锚点已覆盖语义，不触碰 `docs/CLAUDE.md` §六.3 唯一例外条款（worldview 修改必须单独 PR 人工 review）；若归档前实际实施中发现锚点不够用，停下交人工另起 worldview PR，不在本 plan 自动改。

**落点**：`server/src/movement/mod.rs:845-857`（`movement_zone_kind` 死域判定）+ `docs/worldview.md` §一:22 / §七:759（正典锚点，只读引用不改）+ plan §P3「负灵域/死域加成」段 / §8.1 #4

### #5 兽潮双因子公式（与 zone-qi-economy 联动，需同期收口）

**决议**：
1. 双因子 = (zone 持续低灵气时长 **或** 邻域塌缩扩散事件) × danger 加权。`BEAST_TIDE_LOW_QI_THRESHOLD = 0.15`（定义于 `heartbeat.rs:55-56`）驱动两条既有入口——**主入口** `maybe_queue_beast_tide`（`heartbeat.rs:661` 无条件调用 / `:1350-1439` 定义）按 `low_qi_ticks_by_zone` 累计满 `BEAST_TIDE_LOW_QI_REQUIRED_TICKS`（5 分钟）独立触发，**不依赖任何塌缩事件先决**；**次入口**挂 `PseudoVeinDissipated` 邻域扩散分支（`heartbeat.rs:716-750`，阈值判定在 `:733`）。两条入口的阈值常数**保留原位不动**，新增的 danger 权重因子放在 ambient 模块内独立计算，通过读取 `Zone.danger_level` 分别与两条入口的既有判定组合，不直接改写 heartbeat 内的常量定义或两条入口各自的触发条件。
2. 落地顺序：`plan-zone-qi-economy-v1` P1（把 spawn 稳在 equilibrium=0.35）**先行落地**，再启动本 plan P3——equilibrium(0.35) > 阈值(0.15) 意味着经济落地后单靠 qi 阈值兽潮入口被焊死（这正是本 plan 立项动因之一，见背景诊断），需先确认 zone-qi 落地后的真实 equilibrium 数值，再收口双因子加权系数的具体数值。P3 测试用"双因子门槛矩阵"覆盖 qi 骤降速率单独触发 / collapse 事件单独触发 / 两者都不满足三态。
3. 这不是"带着开放问题进 P0"——公式**结构**已在此锁定（双因子 OR 逻辑 + danger 加权、常数归属 heartbeat 不动），只有加权**系数数值**留给 zone-qi-economy 落地后按真实 equilibrium 回填，且 P3 属生态联动阶段非 P0 范围，不阻塞本 plan P0 决策门。

**落点**：`server/src/world/heartbeat.rs:661`（`maybe_queue_beast_tide` 主入口调用点）+ `:1350-1439`（`maybe_queue_beast_tide` 定义 + `low_qi_ticks_by_zone` 累计逻辑，主入口不依赖塌缩事件）+ `:716-750`（`PseudoVeinDissipated` 邻域扩散次入口，阈值判定在 `:733`）+ `:55-56`（`BEAST_TIDE_LOW_QI_THRESHOLD`/`BEAST_TIDE_LOW_QI_REQUIRED_TICKS` 常数定义）+ `docs/plan-zone-qi-economy-v1.md` §P1（equilibrium 落地依据，跨 plan 依赖锚点）+ plan §P3「兽潮门槛改造」段 / §8.1 #5

### #6 昼夜/天气权重

**决议**：
1. **不引入**昼夜/天气刷怪权重——全仓无昼夜 spawn 概念先例，本 plan 不首创。
2. P0 `threat_budget`/`threat_pool` 签名预留 `weight_hook: Option<f32>` 参数（默认 `None`，效果等价 1.0 倍率），不接入任何实际昼夜/天气读取逻辑，仅占位；后续若立"昼夜系统"独立 plan 再回填实现。
3. 拒绝本 plan 自行引入昼夜系统——超出"环境威胁刷新"scope，属另立项范畴；hook 是可选参数默认无操作，不阻塞 P0 交付。

**落点**：新模块 `server/src/npc/spawn/ambient_scheduler.rs`（`threat_budget`/`threat_pool` 签名的 `weight_hook` 参数，P0 实施时新建）+ plan §P0「门控」段 / §8.1 #6

---

**跨 plan 排序总结**：与 `plan-mundane-fauna-v1` 共享 `spawn/ambient_scheduler.rs` 通用调度核归属——本 plan（ambient-threat）先建通用核，mundane-fauna 复用（#1/#2）；与 `plan-zone-qi-economy-v1` 的 P3 兽潮双因子常数归属 + 落地序——zone-qi-economy PR 先行配好 equilibrium，本 plan P3 后行（#5）。

## §10 实施工作流

scope 预估 5 PR（P0 / P1a 主世界物种池 / P1b TSY 接活独立小 PR，见 §8.1 #3 / P2 / P3），按 docs/CLAUDE.md §六：

1. **PR-1（P0）**：`spawn/ambient_scheduler.rs` 通用调度核 + 主世界威胁预算表 + 距离环 + 回收。落地后 `plan-mundane-fauna-v1` 可复用（不阻塞本 plan 后续 PR）。
2. **PR-2（P1a）**：`threat_pool` danger 1~7 主世界物种池分层（§8.1 #2 终表）。
3. **PR-3（P1b，可延后）**：TSY 接活——直调 `spawn_tsy_hostiles_for_family`，TSY 维度隔离 + 独立 respawn 预算（§8.1 #3）。耦合度高于 PR-2，可视排期延后于 PR-4/5。
4. **PR-4（P2）**：rat `PlayerHarassScorer`/`HarassBiteAction` + 复用 `RatBiteEvent` 守恒路径（全复用既有原语，无建筑/资产多轮要求）。
5. **PR-5（P3）**：兽潮双因子改造（需 `plan-zone-qi-economy-v1` P1 先落地，§8.1 #5）+ 负灵域加成对齐 `movement_zone_kind`（§8.1 #4）+ horde 衔接声明。

各 PR 走 docs/CLAUDE.md §六.4 独立 subagent 实施 + §六.5 CodeRabbit ScheduleWakeup 等待协议；P2 含视听但全复用既有原语（`combat/rat_bite.rs` 现成 VfxPlayer/SFX 表现），无 §六.1 建筑三轮 PROMISE 要求。全部 PR merge 且 Finish Evidence 补齐后按根 CLAUDE.md 流转规则归档。
