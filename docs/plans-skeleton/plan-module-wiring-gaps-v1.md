# plan-module-wiring-gaps-v1（骨架 · report-only）

> 主题：模块图谱（`module-map/`）调查中发现的**孤岛/未完全链接**模块——定义齐全（含逻辑/测试/schema）但生产路径缺 producer 或 consumer，对应 gameplay loop 在正常游戏中**永不触发**。本 plan 汇总待修清单，多数涉接线语义抉择，先 report-only 立项，逐项确认后修。

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | server+client critical 孤岛（feature 永不触发） | ⬜ |
| P1 | server 层 warn 孤岛 / 数据丢失 / 文档漂移 | ⬜ |
| P2 | agent 层孤岛（待 server 对齐） | ⬜ |
| P3 | client 层 warn 孤岛 | ⬜ |

来源：`module-map/index.html`「⚑ 缺口」tab（sonnet 调查 → opus 抽查证实无 producer/consumer）。已覆盖 server(42)+client(57)+agent(2) 全 101 模块。**6 个模块含 critical 孤岛**：server/{shader,identity,social,mineral} + client/{iris,dandao}。

---

## P0 — server critical 孤岛（已 grep 证实可达） ⬜

每项均 opus 维护层 grep 全 `server/src` 确认无生产/消费方。**修复涉接线语义抉择，需逐项拍板触发条件后再动手。**

### P0-1 shader ↔ iris — Iris 视觉特效正常游戏永不触发（★跨层两端皆断）
**这是一条跨层 feature 的两端同时断裂**：
- **server 端（server/shader, critical）**：全仓 `ResMut<ShaderStatePayload>` 写入者仅 `server/src/cmd/dev/shader_push.rs` 一处；`bong:shader_state` 广播仅 `shader_push.rs:78`。渡劫/境界提升/灵气浓度变化等 gameplay 事件**均不驱动** shader 更新。
- **client 端（client/iris, critical）**：`BongShaderState.get()` 在 iris 包外**无任何消费者**，全仓无 `IrisApi`/`irisshaders`/`getShaderEnvironment`/uniform provider 引用。网络→State→插值算完后**从未写入实际 GLSL**——整个 iris 子系统是"数据水槽"。
- **影响**：即便接通 server 触发源，client 也不会把 uniform 注入 shader → 视觉效果实际为零。两端都得修。
- **待决策 + 接入面**：server 侧定 gameplay 触发源（cultivation/tribulation/qi_physics → ShaderStatePayload）；client 侧实现 Iris uniform provider（`net.irisshaders.iris.api.v0.IrisApi` + custom uniform）或 Mixin 劫持 uniform upload。

### P0-2 identity — 身份信誉对 NPC 行为零影响（两处孤岛）
- **DuguRevealedEvent 无 producer**：`cultivation/dugu.rs` 定义但全 src 仅 `identity/gossip.rs` 测试里 `send_event`。reveal → `consume_revealed_event`(写 RevealedTag) → reaction tier → gossip 扩散 → `wanted_player` Redis 下发 整条链缺触发源。毒蛊师身份暴露→声誉惩罚→通缉 loop 永不触发。
- **IdentityReactionScorer 从不挂载**：scorer system 注册了，但无 NPC spawn 路径 `insert` 该 Component、无 Thinker 编入决策树 → `Query<With<IdentityReactionScorer>>` 永远空集。身份信誉对 NPC 追杀/拒交易零影响。
- **待决策**：DuguRevealedEvent 何时 fire（被侦测/主动暴露/特定交互？）；IdentityReactionScorer 该挂哪些 NPC（全体 disciple？敌对派系？）。
- **接入面**：`server/src/identity/` + `server/src/npc/spawn/*`（insert Scorer + Thinker）。

### P0-3 social — PvP 社交后果永不触发
- **现状**：`PvpEncounterEvent` 无任何生产 `.send()`/EventWriter（grep 确认仅本模块定义 + consumer `handle_pvp_encounter_events`）。combat/pvp 击杀结算侧未接线发送。
- **影响**：PvP 社交后果（仇敌生成 / 背叛声誉惩罚 / 传记条目）整模块静默从不触发。
- **修复线索**：在 `combat` 击杀 / PvP 结算路径 `send(PvpEncounterEvent)`。比 P0-1/P0-2 清晰（consumer 已就绪，只缺 producer 一处接线），但需确认击杀路径上下文与 PvP 判定。
- **接入面**：`server/src/combat/*`（击杀结算）→ `server/src/social/pvp_encounter`。

### P0-4 mineral — 矿脉再生功能不可达 ★最清晰，候选优先修
- **现状**：`ExhaustedMineralsLog::remove_respawned`（`mineral/persistence.rs`）有完整逻辑 + 单测，但 `mineral/mod.rs` Update 调度只注册 `tick_mineral_clock`/`record_exhausted_minerals`，**无系统在运行时调用 `remove_respawned` 重建 OreNode + 更新 `MineralOreIndex`**。带 `respawn_at_tick` 的矿脉到期后永不真正 respawn。
- **修复线索**：加一个 Update 系统：到期时调 `remove_respawned` → 重建 `OreNode` + 更新 `MineralOreIndex`，注册进 `mod.rs`。函数与测试已存在，**接线最清晰、风险最低**，是本 plan 首个动手候选。
- **待确认**：respawn 时是否需重新生成矿脉品质/储量（看 log 里存了什么）。

---

### P0-5 client/dandao — 丹道 HUD 与异化贴图永不显示（两处孤岛）
- **MutationHudPlanner.buildCommands() 孤岛**：主代码无 HUD 编排器调用（仅测试引用），`DANDAO_MUTATION` 层在 HudRenderLayer/HudLayoutPreset 已声明但 `buildCommands` 从未触发 → 丹道 HUD 面板从不渲染。mutation_visual→state→HUD 链断在最后一跳。
- **MutationFeatureRenderer 从未注册**：主代码仅 WornPack 三处 javadoc 把它当"从未注册的孤岛反面教材"引用 → 玩家身体异化叠加贴图永远不显示。
- **修复线索**：把 `MutationHudPlanner` 接入 HUD 编排器主循环；把 `MutationFeatureRenderer` 注册到 PlayerEntityRenderer 的 FeatureRenderer（注意 [[feedback_mixin_package_helper]] / GeckoLib 不能在 player FeatureRenderer 驱动 GeoModel 的坑）。

---

## P1 — server warn 孤岛 / 数据丢失 / 文档漂移 ⬜

- **tribulation scorch 孤岛**：`record_tribulation_scorch_system` 持续生产 records，但 persistence/world 无消费者，`glass_fulgurite` 永不写块（焦土玻璃化视觉缺失）。
- **economy BoneCoinTickV1 遥测丢弃**：server 发布到 `CH_BONE_COIN_TICK`，但 agent `redis-ipc.ts` 根本没 subscribe → 经济遥测被丢，天道无法感知货币流动。
- **craft RecipeUnlockState 无持久化**：纯内存，玩家重连解锁配方状态全丢（对照 [[player_inventory_persist_migration_gap]] 同类持久化缺口）。
- **qi_physics QiTransferReason audit-only footgun**：仅 `HalfStepBuff` 在 transfer 入口强拒，其他 reason 变体误传会静默改 balance 而无审计拦截（守恒 footgun，非现存 bug）。
- **fauna 妖兽龙簇 dead content**：VoidDistorted/PoisonDragon/BoneDragon components+drop+visual 齐全但五档 spawn 权重池均无、无专属 spawn → 永不被生成。
- **sword_path upgrade.rs / dandao 变异技能 & P5 技能 / skill mod.rs doc-code 矛盾**：多处 skeleton/孤岛（详见 webui 各模块 gap）。
- **forge / botany / spiritwood / gathering 等**：详见 `module-map` 缺口 tab。

## P2 — agent 孤岛（待 server 对齐） ⬜

- **FactionCensusStore 孤岛**：完整实现 + 测试但 main/runtime 零实例化。
- **CROSS_SYSTEM_EVENT_CHANNELS 30+ 频道**：订阅 + 缓存但无任何消费方，静默丢弃。
- **3 payload 无 pin 测试**：baomai_v4 / woliu_erosion / halfstep_rechallenge 已定义+导出+激活+有 Rust 对齐结构体，但未进 SCHEMA_REGISTRY → TS↔Rust 漂移无自动捕获。
- **era Agent intervalMs=36,000,000ms(10h)**：疑似配置笔误（对比 calamity 180s / mutation 600s），第三个"演绎时代"Agent 几乎从不主动触发；时代切换全靠 `Arbiter.detectEraFromNarrations` 反推。**这条相对清晰，可单独快修。**
- **文档漂移**：`CLAUDE.md` 写的 channel `bong:agent_cmd` 与代码实际 `bong:agent_command`（`CHANNELS.AGENT_COMMAND`）不一致——交人工改 CLAUDE.md。

## P3 — client 孤岛 ⬜

**critical 已上提到 P0**（client/iris → P0-1 与 shader 同源；client/dandao → P0-5）。以下为 warn 级：

- **armor / weapon OBJ 渲染未实装**：`OBJ_RENDER_READY=false` early-return；bone/lingmu_sword `bongObjModelPath=null` → 自定义护甲/武器模型只显原版占位（视觉占位，对照 [[project_model_linkage_audit]] B/C 组）。
- **entity FormationCore 硬关渲染**：`DISABLE_FORMATION_CORE_RENDER=true`（render() 开头 return）→ 阵核(raw_id=154)服务端 spawn marker 但客户端完全不渲染。
- **npc 断线日志不清**：`NpcInteractionLogStore` 只有 `resetForTests`、无 `clearOnDisconnect` → 重连后旧交互日志条目残留。
- **whale**：raw_id 注册顺序敏感（静默偏移）；Phase B-2 server 同步未接。
- **spirittreasure 面板分发孤岛** + **social SilentSignalSystem / NicheGuardianPanel 孤岛**（detect/snapshot 双向无接线）。
- **era ambient_sound 孤岛**：解析存储但无播放消费方。
- **yidao YidaoNpcAiStateStore 只写不读孤岛**。
- **forge ForgeOutcomeStore.markDisplayed 无调用方**（hasNewOutcome 结算后恒 true）。
- **alchemy 投料链路孤岛**：`AlchemyScreen` 用 `MockInventoryData.create()` 投料不发 C2S；`InventoryMetaStore` 无 S2C handler（硬编码默认值）——plan MVP 占位。
- **client-infra ProcessingActionScreen**：输入/输出硬编码 0 + C2S intent 缺失。
- 多处 dead-code（visual NicheDefense、hud VortexCharge/VortexCooldown、daozhan P3 计划内未注册 mixin）—— info 级，详见 webui 缺口 tab。

---

## P4 — 跨层 feature 发现（dossier 维度） ⬜

来自 6 张跨层 feature dossier（webui「跨层 Features」tab），跨模块串接处的问题：

- **★剑道 stored_qi 脱守恒账（需核验，可能 critical→单独开 plan）**：`summarize_world_qi` 统计 player qi_current + zone.spirit_qi + inventory + npc/lod WorldQiAccount，**唯独不数 `SwordBondComponent.stored_qi`**（虚剑 cap 3000 量级）。`inject_bond_qi` 把 qi 从 qi_current 移进 bond 后即从守恒快照消失；化虚 aftermath `bond.stored_qi=0` 只走 audit-only `QiTransfer` 不回灌 zone。与 MEMORY 记的 qi 守恒漏洞类同源（[[project_bughunt_qi_conservation]]）——需核验是真守恒还是大额脱账。
- **剑道天门双结算管线并存**：legacy `heaven_gate_cast_system` 仍注册但 dormant，两套逐字复制 aftermath 守恒代码并行，误发事件会重复结算。
- **黑武士命中反馈断层**：boss 是无 UUID 的 Marker 实体，client `CombatEventHandler` 伤害浮字按 `target_uuid` 键控 → 攻击 boss 大概率不出伤害数字/受击反馈；且无 boss 血条/相位 UI（已有 `TsyBossHealthBar` 先例未复用）。需真机验证。
- **alchemy tick 从不调度**：`AlchemySession::tick()` 无 per-tick 系统调度，`elapsed_ticks` 恒 0、温度不采样，仅取丹瞬间 for 循环快进补齐且记同一温度 → 调温玩法被当全程恒温评分。`InterventionRequest` 全库零 EventReader（孤儿）。
- **经济指数未闭环**：`npc::social` 估价用 `neutral_price_index()` 常量，实时 `EconomyPriceIndex` 只进遥测/叙事，供需波动不影响 NPC 报价。

## 备注

- 本 plan 由 `/runwebui` 模块图谱审计自动汇总，**report-only**：多数项涉接线语义抉择，逐项确认触发条件后再开 worktree 修。
- 首个动手候选：**P0-4 mineral**（函数+测试已就绪，仅缺调度注册）；次选 **P2 era interval**（疑似笔误，单值修改）。
- 关联记忆：[[project_module_map_webui]]、[[project_bughunt_findings]]、[[feedback_spawn_chain_wiring]]（emit 无 consumer 孤岛同源问题）。
