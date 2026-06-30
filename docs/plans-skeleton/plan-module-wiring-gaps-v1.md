# plan-module-wiring-gaps-v1（骨架 · report-only）

> 主题：模块图谱（`module-map/`）调查中发现的**孤岛/未完全链接**模块——定义齐全（含逻辑/测试/schema）但生产路径缺 producer 或 consumer，对应 gameplay loop 在正常游戏中**永不触发**。本 plan 汇总待修清单，多数涉接线语义抉择，先 report-only 立项，逐项确认后修。

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | server 层 critical 孤岛（feature 永不触发） | ⬜ |
| P1 | server 层 warn 孤岛 / 数据丢失 / 文档漂移 | ⬜ |
| P2 | agent 层孤岛（待 server 对齐） | ⬜ |
| P3 | client 层孤岛（client map 跑完后补） | ⬜ 待补 |

来源：`module-map/index.html`「⚑ 缺口」tab（sonnet 调查 → opus 抽查证实无 producer/consumer）。截至 v1 仅含 server+agent 层；client 层映射完成后扩 P3。

---

## P0 — server critical 孤岛（已 grep 证实可达） ⬜

每项均 opus 维护层 grep 全 `server/src` 确认无生产/消费方。**修复涉接线语义抉择，需逐项拍板触发条件后再动手。**

### P0-1 shader — Iris 视觉特效正常游戏永不触发
- **现状**：全仓 `ResMut<ShaderStatePayload>` 写入者仅 `server/src/cmd/dev/shader_push.rs` 一处；`bong:shader_state` 广播仅 `shader_push.rs:78`。渡劫/境界提升/灵气浓度变化等 gameplay 事件**均不驱动** shader 更新。
- **影响**：所有 Iris shader 视觉特效只能 dev 命令手动推，玩家正常游戏看不到。
- **待决策**：哪些 gameplay 事件该驱动 shader（境界突破？渡劫阶段？区域灵气浓度？），各自映射到哪个 ShaderState。
- **接入面**：`server/src/shader/` + 触发源模块（cultivation/tribulation/qi_physics）emit → ShaderStatePayload。

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

## P3 — client 孤岛 ⬜ 待补

client map（57 模块）跑完后扩写。

---

## 备注

- 本 plan 由 `/runwebui` 模块图谱审计自动汇总，**report-only**：多数项涉接线语义抉择，逐项确认触发条件后再开 worktree 修。
- 首个动手候选：**P0-4 mineral**（函数+测试已就绪，仅缺调度注册）；次选 **P2 era interval**（疑似笔误，单值修改）。
- 关联记忆：[[project_module_map_webui]]、[[project_bughunt_findings]]、[[feedback_spawn_chain_wiring]]（emit 无 consumer 孤岛同源问题）。
