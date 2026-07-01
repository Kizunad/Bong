# plan-module-wiring-gaps-v2

> 主题：module-map ⚑ 旗全量 triage（2026-07-01，53 sonnet agent 逐模块读真实代码）后，**涉 gameplay 设计抉择或 qi_physics 守恒**的 40 面旗归档成 **14 个决策主题**。本 v2 是 **report-only 决策菜单**——每个主题都需人工拍板触发语义/平衡数值/守恒口径后才能落地实施，禁止自动拍板（docs/CLAUDE.md §五 + qi 守恒红线）。
>
> **triage 分布 + 处置**：77 面 → **28 FIXABLE 已修**（机械接线，#803–808 已合并/PR，各旗已在 map 标 RESOLVED）/ **9 FALSE_POSITIVE 已清**（过时观察，#802 降 info）/ **40 NEEDS_DECISION（本文档）**。其中 5 面涉 qi 守恒（★标）。
>
> 注：40 = 原 39 NEEDS_DECISION + 1（T14 yidao，triage 初判 FIXABLE，实施时 Read server 代码发现是不可转换 id 空间的跨层项，reclassified 进本菜单）。

## 阶段总览（每个主题 = 一个候选未来 plan，均 ⬜ 待决策）

| # | 主题 | 涉旗数 | qi 守恒 | 待你拍板的核心抉择 |
|---|------|--------|---------|-------------------|
| T1 | 丹道走火入魔 gameplay 三件套 | 5 | ★×3 | 7 种变异效果各挂哪个子系统 + 3 变异招式战斗数值 + 内炼扣款守恒口径 |
| T2 | 应龙 BOSS 战斗跨层同步 | 1 | — | server boss action_state 字段 + 同步通道（DataTracker vs custom payload）|
| T3 | 社交 PvP 遭遇系统 | 2 | — | 遭遇状态机 + 六种肢体语言信号触发/呈现语义 |
| T4 | 身份声誉 → NPC 反应 | 1 | — | 哪些 NPC 对通缉玩家反应、复用/新建哪个 Action、Thinker 排序 |
| T5 | 世界事件广播（伪灵脉/宗核）| 2 | ★ | snapshot 发送频率 + 宗核激活触发源语义（PseudoVein bridge 部分机械可修）|
| T6 | ★剑道升阶流水线 | 1 | ★ | 升阶触发源 + `qi_consumed` 扣除后是否写回 zone（守恒记账口径）|
| T7 | forge/lingtian 世界站持久化 + 加工触发 | 3 | — | 定位型 block-entity 持久化表结构 + 加工启动 intent 承载 |
| T8 | botany 事件触发生态 | 3 | — | 残灰方块/死域边缘/伪灵脉消散的判定规则（依赖 plan-residue/plan-tribulation）|
| T9 | 采集/挖矿架构统一 | 2 | — | MiningSession 并入 GatheringSession? loot_table 通用化 or 废弃 |
| T10 | 天道 Agent 消费编排 | 3 | — | 30+ 跨系统频道由哪个 Agent 以何频率消费 |
| T11 | NPC 战力/装备/AI 平衡 | 2 | — | 装备倍率如何组合进伤害管线 + 拟态蛛 Ambush 战斗数值 |
| T12 | 世界后果/运行时地形驱动 | 3 | — | 焦土方块资产 + terrain_profile 是否驱动运行时地形 + 新环境效果 variant |
| T13 | client 渲染/UX 收尾（真机/资产/第三方）| 11 | — | OBJ 护甲/阵法核心/Iris uniform/畸变呼吸/音频渐出/多槽特效等各自技术方案 |
| T14 | yidao 健康师 AI HUD（跨层 id 桥接）| 1 | — | server 补 healer_id↔targetId 可换算字段（reclassified from FIXABLE）|

---

## T1 — 丹道走火入魔 gameplay 三件套 ★×3

**涉旗**：`server/dandao › 变异推进系统`、`server/dandao › 丹道招式`（×2，QI）、`server/dandao › 境界递进与化虚内炼`（QI）、`client/dandao › MutationInspectLabel`。

- **变异效果（D1）**：`mutation.rs:169-210` 定义 7 种 `MutationEffect`（VisionBoost/UnarmedDamageBonus/PurgeBoost/NaturalArmor/DamageReduction/ExtraHandSlots/ConstitutionBoost/IntimidateAura），`mutation_advance_system` 只推进 stage 从不调 `kind.effect()`。仅 IntimidateAura 有半径光环先例（`boss_spawn.rs:336`），其余 6 种各需决定挂载子系统 + 数值语义（ExtraHandSlots↔库存 schema、NaturalArmor 伤害类型降级算法）。
- **变异招式（QI）**：`mutation.rs:128-146` BoneRidge→`bone_slam`（无数值）/Horns→`horn_charge`(qi_cost:5)/Tail→`tail_strike`(减伤%)，`register_skills` 未注册；三招缺 damage/cooldown/判定范围；`horn_charge` 扣 qi 需走 `QiTransfer` 归还 zone。
- **内炼/化虚（QI）**：`progression.rs` `abilities_unlocked_at` + `internal_brew.rs` `can_internal_brew`/`internal_brew_qi_cost` 逻辑完整但零非测试调用方；`pill_resonance`(被动效率)/`pill_to_blood`(永久提 qi_max)/`great_transmutation`(体内炼丹)缺触发源；注释写"qi 走 QiTransfer"但当前直接扣 `player.qi_current` 无归还路径。
- **MutationInspectLabel（D6，跨层）**：`buildLabels()` 无生产调用；`MutationVisualState` 是纯静态本地字段（只存"自己"），要 inspect 他人异化需先设计 server→client 同步他人异化状态的跨层协议。

**决策**：这是丹道流派完整的"走火入魔"玩法落地——变异效果如何影响战斗/视觉/库存、招式平衡、内炼扣款守恒。建议单独立 `plan-dandao-mutation-gameplay-v1`。

## T2 — 应龙 BOSS 战斗跨层同步

**涉旗**：`client/dandao › BaolongwangEntity`。`actionState` 注释标 Phase B-2 未做；server `boss.rs`/`boss_ai.rs` 连 action_state 字段都未定义。**决策**：先在 server 设计 BOSS 动作状态字段 + 触发语义（何时 attack/skill1/skill2、tick 节奏），再定同步通道。

## T3 — 社交 PvP 遭遇系统

**涉旗**：`server/social › PvP Encounter`、`client/social › SilentSignalSystem`。`PvpEncounterEvent` 无生产者；需新设计跨 tick 遭遇状态机（FarAssessment→MidProbe→CloseContact、outcome 六分类、betrayer 判定、zone→EncounterContext 映射）。`SilentSignalSystem` 六种肢体语言信号需全新 per-remote-player 状态追踪 + 呈现方式。**决策**：`plan-pvp-encounter-v1`。不涉 qi 守恒（qi_color_hint 仅视觉字符串）。

## T4 — 身份声誉 → NPC 反应

**涉旗**：`server/identity › IdentityReactionScorer`（critical）。scorer 注册但 `Query<With<IdentityReactionScorer>>` 恒空，无 spawn 路径插入。README 明示"留给消费方 opt-in"。**决策**：哪些 NPC（rogue/disciple/commoner）反应、复用 ChaseAction 还是新建 RefuseTradeAction、Thinker FirstToScore 排序（会改现有优先级语义）。

## T5 — 世界事件广播（伪灵脉/宗核）★

**涉旗**：`server/network › RedisBridge`（QI）、`server/schema › 世界环境地貌 schema`。三 telemetry channel（PseudoVeinSnapshot/Dissipate/ZongCoreActivated）序列化臂齐全但无 `tx_outbound.send` 生产者，agent 端永不收到。
- **PseudoVein 部分**：`pseudo_vein_runtime_tick_system` 已算好 settlement/snapshot，可按 `tsy_event_bridge.rs` 先例加 `pseudo_vein_event_bridge.rs`，**只读已算值不改账、不触碰守恒**——这部分接近机械，但 snapshot 发送频率是设计抉择。
- **ZongCore 部分**：`ZongFormationCore`/`ZongFormationCharge` 只在定义文件出现，无 ECS component/spawn/玩家入口——"宗核何时被何行为触发激活"整个玩法未落地。

**决策**：snapshot 广播频率口径 + 宗核激活触发源。可拆分为"先接 PseudoVein bridge（低风险）+ 后设计 ZongCore 触发"两切片。

## T6 — ★剑道升阶流水线（qi 守恒）

**涉旗**：`server/sword_path › 升阶流水线`（QI★）。`check_upgrade`/`resolve_upgrade` 仅测试调用，`add_systems` 无 upgrade system，`UpgradeIntent` 全仓零命中——升阶逻辑无运行时入口。`resolve_upgrade` 直接操作 `qi_current`/`qi_consumed`/`stored_qi_lost`。**决策**：①玩家触发源（工作台/命令/intent，先例 heaven_gate 走 cast+phase 双阶段）；②**`qi_consumed` 扣除后是否按 [[project_bughunt_qi_conservation]] 先例写回 `zone.spirit_qi` 而非蒸发**——这正是历史 qi 守恒漏洞类，必须专项守恒设计。

> 注：这与 module-map 之前记忆里"★sword `stored_qi` 脱 `summarize_world_qi` 守恒账"疑点同源。triage 确认升阶流水线整体是**孤岛（无运行时入口）**，故当前不构成"运行中的守恒漏洞"，但一旦接线必须走守恒记账。

## T7 — forge/lingtian 世界站持久化 + 加工触发

**涉旗**：`server/forge › WeaponForgeStation`、`server/forge › 灵田丹炉炮制桥接`、`client/processing › ProcessingActionScreen`。WeaponForgeStation（+ alchemy furnace + lingtian plot 同 TODO）缺 block-entity 持久化，现无"世界坐标定位、多实例、entity↔pos 映射"的持久化表先例（player_shrine 是单实例按 username）。`lingtian::processing` 4 种 kind 生产端从未被玩家交互触发。**决策**：定位型 block-entity 持久化表结构（三处统一设计）+ 加工启动 intent 承载方式（新 proto / 复用物品交互 / 丹炉 UI 按钮）。

## T8 — botany 事件触发生态

**涉旗**：`server/botany › EventTriggeredSpawn`、`server/mob › AshSpider 刷新权重`、`server/botany › Integration`。多个 EventTriggered 物种（YangJingTai/HuiJinTai/HeiGuJun/ZhongYanTeng/TianNuJiao）依赖未实装的残灰方块/死域/伪灵脉消散事件；AshSpider 死域边缘加权公式未接入 spawn 调度；integration.rs 别名归一化无消费方。**决策**：残灰/死域/伪灵脉的判定规则（依赖 plan-residue / plan-tribulation 伪灵脉机制）。

## T9 — 采集/挖矿架构统一

**涉旗**：`server/mineral › MiningSession`、`server/gathering › 会话调度器`。MiningSession（多 tick 进度模型）与生产在用的 GatheringSession 字段重叠但设计不同，未拍板是否并入；`Gatherable.loot_table` 是死字段（三类采集各自旁路发放）。**决策**：MiningSession 取代/合并 GatheringSession？loot_table 通用化还是废弃？（plan-mineral-v2）

## T10 — 天道 Agent 消费编排

**涉旗**：`agent/tiandao › Redis IPC 通信层`、`agent/tiandao › 生态/经济/派系分析器`、`agent/tiandao › Skill 提示词库`。30+ `getLatest*` 跨系统事件 getter 整批"先订阅广谱、暂不接消费"；`FactionCensusStore` 无实例化；`calamity-selector.md` 无 switch 分支加载。**决策**：各频道由哪个 Agent（灾劫/变化/演绎/Arbiter）以何频率/触发条件消费；selector 提示词该删还是接。属天道三 Agent 推演编排设计。

## T11 — NPC 战力/装备/AI 平衡

**涉旗**：`server/npc › Infra (Combat Power · Equipment)`、`server/fauna › mimic-spider-ai`。`assign_npc_equipment` 从不被 spawn 调用，`NpcEquipment` 永远 None，装备倍率无处组合进 `AttackIntent`（qi_invest/wound_kind/reach 驱动的 resolver）；拟态蛛 Ambush 阶段缺攻击/追击 Action（daozhan.rs register_p2 有完整先例但战斗参数需设计）。**决策**：装备如何 scale NPC 伤害（战斗平衡）+ Ambush 战斗数值。

## T12 — 世界后果/运行时地形驱动

**涉旗**：`server/tribulation › ScorchRecord`、`server/worldgen › TransientZone 蓝图模板工厂`、`server/world › WorldFeatures`。ScorchRecord push 进 resource 无消费者，`glass_fulgurite` 无对应 BlockState；TransientWorldgenTemplate 的 terrain_profile/landmarks/boundary 无处落地（Zone 结构体无对应字段）；wangyintai 的 particle/echo/sky-shift 效果无 EnvironmentEffect variant。**决策**：焦土方块资产 + terrain_profile 是否驱动运行时地形重塑 + 是否新增环境效果 variant（跨 server proto→client 契约）。

## T13 — client 渲染/UX 收尾（真机/资产/第三方集成）

**涉旗**（11）：`client/armor › ArmorFeatureRenderer`（OBJ_RENDER_READY=false，需真机 F5 调 pivot/scale）、`client/entity › FormationCoreRenderer`（DISABLE=true 临时隐藏，待模型就绪）、`client/iris › BongShaderState`（current[] 从未喂进 Iris uniform，需新增 Iris compileOnly 依赖 + custom-uniform API）、`client/atmosphere › Command`（distortion/breathing 无 HudRenderCommand 畸变原语）、`client/audio › MinecraftSoundSink`（fadeOutTicks 被吞，需自建 TickableSoundInstance）、`client/state › VisualEffectState`（单槽硬覆盖，多槽/优先级需设计）、`client/state › UiOpenState`（无就绪门槛可能被原版切屏覆盖）、`client/spirittreasure`（为未来灵宝类型预留 dispatch 骨架？）、`client/whale`（entity raw_id 靠人工同步，握手机制未定）、`client/insight › Dispatcher`（idx 二次解析降级契约）、`client/insight › OfferHandler`（offer 字段用硬编码 fallback，需扩 InsightOfferV1 schema）。

**决策**：多为"选技术方案/等资产/第三方集成取舍/架构契约"，各自独立。建议按优先级挑选（如 iris uniform、armor OBJ 属视觉资产落地走 [[feedback_part_based_modeling]] 真机流程；insight offer 字段扩展有 heart_demon_offer 先例但字段语义需裁定）。

---

## T14 — yidao 健康师 AI HUD（跨层 id 桥接，triage 误判 FIXABLE 后 reclassified）

**涉旗**：`client/yidao › YidaoNpcAiStateStore`。triage 初判 FIXABLE，实施(Sonnet 5)时 Read server 代码发现前提**错误**：server `healer_id='entity_bits:<Bevy Entity::to_bits() u64>'`（`combat/yidao.rs:1618`）与 client `targetId='entity:<Valence EntityId i32>'`（`network/npc_metadata.rs`）是**两套不可转换的 id 空间，服务端无任何桥接字段**。若照 triage 方案把 TargetInfoHudPlanner 接上 YidaoNpcAiStateStore，生产环境永不命中（fails-closed=新增孤岛，违反反孤岛铁律），故未 ship。**决策**：server 侧在 healer AI payload 或 npc_metadata 补一个可与 targetId 换算的 id 字段（如同时携带 Valence EntityId），client 再按原接线方式消费。属跨层契约扩展，非纯 client 接线。

## §决策指引

- 想推进某主题：告诉我主题号（如"做 T6 剑道升阶"），我会先按正典 + qi 守恒律出实施方案交你确认，再开 active plan 实施。
- T1/T5/T6 涉 qi 守恒，实施时强制走 `QiTransfer`/`zone.spirit_qi` 记账，不蒸发（[[project_bughunt_qi_conservation]]）。
- T13 里 armor OBJ / formation core 属视觉资产，走真机 3 轮打磨 + PROMISE 惯例。
- 全部 triage 证据（每面旗 file:line + reason + risk）在 `module-map/index.html` ⚑ tab 及本会话 triage 输出。
