# plan-unconsumed-event-feedback-v1 — 未消费领域事件反馈批量接线

> 主题：`server/src/test_coverage_guards.rs` 的 `INTENTIONAL_UNCONSUMED_EVENTS` 是一份**编译期强制登记**的「emit 了但没有任何 EventReader」triage 清单（**25 条**，逐条盘点见下）。其中 **19 条** `DeferredFollowUp` 的 follow_up 指向"feedback / narration / telemetry 接线"却没有任何 plan 认领——玩家侧表现为：炼器裂痕零回执、全力一击击杀无演出、渡劫逃逸无叙事、铸骨币无经济反馈等一大批"效果生效了但玩家/天道毫无感知"的半闭环。本 plan 按消费端类型分批把这 19 条接上**现成的** narration / HUD / VFX / Redis 桥基建（`server/src/network/` 下已有约 90 个 `*_emit.rs` / `*_bridge.rs` 范本），并利用 guard 自带的 stale 检测形成防回归棘轮。
>
> 来源：2026-07-18 全仓扫描（server Explore agent + plan 覆盖面盘点交叉比对）；生产链 file:line 均于同日对 origin/main（`7ad2be2d`）grep 核验。

## 阶段总览

| 阶段 | 内容 | 状态 |
|----|------|----|
| P0 | 叙事批（7 事件 → narration emit，纯 server） | ⬜ |
| P1 | 炼器反馈三连（bridge + client HUD toast） | ⬜ |
| P2 | 战斗/身体演出批（5 事件 → VFX + SFX + HUD） | ⬜ |
| P3 | 身份 → 生平记录（2 事件 → life_record + narration） | ⬜ |
| P4 | 跨端观察批（2 事件 → Redis 通道 + 天道 agent 观察） | ⬜ |
| P5 | 防回归收口（triage 集合 pin + 代表性 e2e） | ⬜ |

## 接入面 Checklist

- **进料**：`INTENTIONAL_UNCONSUMED_EVENTS` 中 19 条 `DeferredFollowUp` 事件。**每条的 emit 调用点 + 生产 system + 生产端注册点已逐条核验**（见各阶段接线表；唯一例外 `TsyZoneInitialized` 的生产者挂在 dev 命令模块，已单列处理，见 §8 #7）。`cultivation::life_record` 生平记录模块；`server/src/network/` 既有 emit/bridge 基建。
- **出料**：narration（聊天栏/事件流）、client HUD toast/VFX/SFX、`bong.db` 生平记录、Redis 通道（P4 新增 2 条）→ 天道 agent 观察。
- **共享类型 / event**：**不新增任何 Bevy event**——本 plan 只给既有 event 加 EventReader 消费者。narration 走既有 `NarrationEvent` 管线；VFX 走既有 `bong:vfx_event` 管线。
- **跨仓库契约**：P1/P2 若新增 server_data payload 按 `reference_server_data_payload_field` 清单双端落地（proto + struct + From + convert + emit + schema regenerate）；P4 新增 Redis 通道需 `agent/packages/schema/src/channels.ts` + `server/src/schema/channels.rs` 双端登记 + sample 对拍。
- **worldview 锚点**：`worldview.md §四`（战斗物理可见性——招式/击杀效果必须可感知）、`§九`（经济——骨币铸造是经济事件）、`§十二`（天道观察世界——agent 需要感知面）。
- **qi_physics 锚点**：**本 plan 零真元流动**——所有接线均为只读观察者（EventReader 读事件发反馈），不触碰 `qi_current` / `zone.spirit_qi` / ledger。任何阶段若发现需要动 qi 记账，立即停下重新设计。

## 范围划定：25 条 triage 逐项盘点（2026-07-18 对 origin/main 核验）

**数量等式**：25（triage 全量）= **19**（本 plan 认领：P0×7 + P1×3 + P2×5 + P3×2 + P4×2）+ **4**（他 plan 已认领）+ **1**（DirectResourceConsumer，设计上不接）+ **1**（AuditOnly，设计上不接）。

| # | 事件 | triage 状态 | 归属 |
|---|------|-------------|------|
| 1 | ArtifactMeridianCracked | DeferredFollowUp | 本 plan **P1** |
| 2 | ArtifactMeridianDepthChanged | DeferredFollowUp | 本 plan **P1** |
| 3 | BoneCoinCrafted | DeferredFollowUp | 本 plan **P0** |
| 4 | CraftStartedEvent | DeferredFollowUp | 他 plan：`plan-craft-v1` P2/P3（follow_up 明示） |
| 5 | DigestionOverloadEvent | DeferredFollowUp | 本 plan **P2** |
| 6 | FlowFieldPrototype | DeferredFollowUp | 他 plan：`plan-beast-horde-v1` cleanup（active，待删原型） |
| 7 | ForgeProcessingAccepted | DeferredFollowUp | 本 plan **P1** |
| 8 | FullPowerStrikeKilledEvent | DeferredFollowUp | 本 plan **P2** |
| 9 | IdentityCreatedEvent | DeferredFollowUp | 本 plan **P3** |
| 10 | IdentitySwitchedEvent | DeferredFollowUp | 本 plan **P3** |
| 11 | InfluenceChangedEvent | DeferredFollowUp | 本 plan **P0**（仅叙事切片，NPC 反应出 scope，见 §8 #3） |
| 12 | MigrationEvent | DeferredFollowUp | 本 plan **P4** |
| 13 | NpcScheduleChangedEvent | DeferredFollowUp | 本 plan **P0** |
| 14 | QiTransfer | DirectResourceConsumer | 不接——守恒审计事件，余额由 `WorldQiAccount` 直接 apply |
| 15 | SupplyCoffinOpened | DeferredFollowUp | 本 plan **P0**（与 `plan-lootcrate-v1` 协调，见 §8 #4） |
| 16 | SwordBondFormedEvent | DeferredFollowUp | 本 plan **P2** |
| 17 | TechniqueLearnedEvent | DeferredFollowUp | 他 plan：`plans-skeleton/plan-bughunt-technique-feedback-bridge-v1` |
| 18 | TechniqueMasteredEvent | DeferredFollowUp | 他 plan：同上 |
| 19 | TribulationFled | DeferredFollowUp | 本 plan **P0** |
| 20 | TsySpawnResult | AuditOnly | 不接——dev-only 观测事件 |
| 21 | TsyZoneInitialized | DeferredFollowUp | 本 plan **P0**（生产链特例，见 §8 #7） |
| 22 | TurbulenceFieldDecayed | DeferredFollowUp | 本 plan **P2** |
| 23 | VoidActionBroadcast | DeferredFollowUp | 本 plan **P4** |
| 24 | YidaoCastCompleteEvent | DeferredFollowUp | 本 plan **P2** |
| 25 | ZoneEnvironmentLifecycleEvent | DeferredFollowUp | 本 plan **P0** |

与 `plans-skeleton/plan-module-wiring-gaps-v2.md` 的边界：v2 是**涉 gameplay 设计抉择**的 report-only 决策菜单（17 主题），本 plan 只收**不需要设计拍板的机械反馈接线**。

---

## P0 — 叙事批（7 事件 → narration，纯 server） ⬜

每条 = 一个小 EventReader system（照抄 `server/src/network/` 同域既有 `*_emit.rs` 模式；范本 `forge/mod.rs:144` 的 `artifact_tier_evolved_narration` 即同文件事件接 narration 的先例）。**接线表**（emit 点与生产注册点均已核验为生产代码，非测试 app）：

| 事件 | emit 点 | 生产 system（生产端注册点） | 新消费落点 → 输出 |
|------|---------|------------------------------|--------------------|
| TribulationFled | `cultivation/tribulation.rs:3660`（共享 helper） | `abort_du_xu_on_client_removed`(:3449) + `tribulation_escape_boundary_system`(:3515)，注册 `cultivation/mod.rs:449/:452` | 新 reader → narration「有人临劫而逃，天雷余怒未消。」（zone / perception） |
| TsyZoneInitialized | `world/tsy_dev_command.rs:319` | `apply_tsy_spawn_requests`(:172)，注册 `world/tsy_dev_command.rs:461`——**生产触发源仅 dev 命令，先按 §8 #7 补生产端 emit** | 新 reader → narration「坍缩渊又一处裂面睁开了眼。」（broadcast / narration） |
| NpcScheduleChangedEvent | `npc/schedule.rs:361` | `schedule_phase_event_system`(:335)，注册 `npc/schedule.rs:81` | 新 reader → 仅玩家可见 NPC 播报（zone / perception，节流见 §8 #1） |
| BoneCoinCrafted | `fauna/bone_coin.rs:160` | `handle_bone_coin_craft_requests`(:130)，注册 `fauna/mod.rs:63` | 新 reader → narration「兽骨入阵，又一枚骨币成了。」（player / narration）+ 经济 telemetry 计数 |
| SupplyCoffinOpened | `supply_coffin/interact.rs:294` | `handle_supply_coffin_interact`(:71)，注册 `supply_coffin/mod.rs:285-288` | 新 reader → 开棺统计 + narration（player / narration） |
| ZoneEnvironmentLifecycleEvent | `world/environment.rs:274` | `publish_zone_environment_lifecycle_events`(:269)，注册 `world/environment.rs:255-263` | 新 reader → 环境相变播报（zone / perception）——仅叙事切片 |
| InfluenceChangedEvent | `world/territory.rs:424/481`（territory_tick 路径）+ `:653/:672`（PvP 路径） | `territory_tick`(:339)，注册 `world/territory.rs:783`；`territory_pvp_influence_system`(:599)，注册 `combat/mod.rs:427` | 新 reader → 跨阈值 narration「此地的气息渐渐换了主人。」（zone / narration） |

- narration 文案升 active 时补齐至 2-3 条/事件并标 scope/style（docs/CLAUDE.md 视听规格）。
- **测试契约（每事件全链）**：真实 Bevy App + 生产注册路径（非直调转换函数）触发 → narration 队列出现**恰好一条**对应条目；重复事件在节流窗口内不重发；缺 zone 上下文走 fallback 路由不静默丢弃（§8 #2）；InfluenceChanged 未跨阈值不播。

## P1 — 炼器反馈三连（bridge + client HUD） ⬜

炼器是玩家长交互流程，三个关键节点当前零回执。**接线表**：

| 事件 | emit 点 | 生产 system（生产端注册点） | 新消费落点 → 输出 |
|------|---------|------------------------------|--------------------|
| ArtifactMeridianDepthChanged | `forge/artifact_meridian.rs:601` | `artifact_meridian_deepen_on_use`(:558)，注册 `forge/mod.rs:141` | 扩展 `network/forge_bridge.rs`（一个 bridge 收三事件）→ forge HUD 深度进度 + narration |
| ArtifactMeridianCracked | `forge/artifact_meridian.rs:613` | 同上 | 同 bridge → HUD 裂痕警告 toast（红）+ SFX + narration「器脉一声闷响，裂了。」 |
| ForgeProcessingAccepted | `forge/processing_mode.rs:84` | `forge_processing_mode_handler`(:35)，注册 `forge/mod.rs:130` | 同 bridge → HUD 受理确认（对齐"请求必有回执"约定） |

- SFX：`audio_recipe` JSON，裂痕用 `minecraft:block.anvil.place`(pitch 0.6, volume 0.5) 打底，升 active 时定稿。
- **测试契约（每事件全链）**：server 侧 event → bridge 转换 → payload 结构 pin（正反 sample）；client 侧 payload → HUD 状态转换断言（深度值更新 / 裂痕 toast 置位 / 受理确认置位）+ malformed payload 拒绝分支 + 无 forge 会话时收到 payload 的降级分支；bot 场景组补 forge 回执断言（`scripts/bot/scenarios/`）。

## P2 — 战斗/身体演出批（5 事件 → VFX/SFX/HUD） ⬜

「爽点时刻」效果已生效但无演出。VFX 基建现成：`vfx_event_emit.rs` / `gameplay_vfx.rs` / `vfx_animation_trigger.rs`。**接线表**：

| 事件 | emit 点 | 生产 system（生产端注册点） | 新消费落点 → 输出 |
|------|---------|------------------------------|--------------------|
| FullPowerStrikeKilledEvent | `cultivation/full_power_strike.rs:555` | `full_power_kill_detection_system`(:529)，注册 `full_power_strike.rs::register`（`cultivation/mod.rs:272` 调用） | vfx_event → 受击点 burst 粒子 + 重音 SFX + 事件流条目 |
| YidaoCastCompleteEvent | `network/cast_emit.rs:228`（**唯一生产 emit**；`combat/yidao.rs:1682` 是测试 helper，不算生产点） | `tick_casts_or_interrupt`(:97 上下文)，注册 `network/mod.rs:988` | HUD 刀势条归位动画 + SFX |
| SwordBondFormedEvent | `sword_path/systems.rs:99` | `sword_bond_tracking_system`(:32)，注册 `sword_path/mod.rs:30` | 结契演出：剑体环绕粒子（continuous, ~40 tick）+ HUD toast「剑契已成」+ narration |
| TurbulenceFieldDecayed | `combat/woliu_v2/tick.rs:58` | `turbulence_decay_tick`(:25)，注册 `combat/woliu_v2/mod.rs:33` | 涡流场消散粒子（radial 扩散淡出） |
| DigestionOverloadEvent | `cultivation/poison_trait/tick.rs:105` | `consume_poison_pill_system`(:76 上下文)，注册 `cultivation/mod.rs:488` | HUD 反胃 vignette（绿色 tint, ~30 tick fade）+ SFX + 状态条提示 |

- 粒子规格升 active 时按 docs/CLAUDE.md §四精度定稿（基类/数量/lifetime/hex/spawn 模式/贴图 ID/VfxPlayer 类名/`bong:vfx_event` ID）。
- **测试契约（每事件全链）**：server event → vfx/HUD payload 结构 pin；client 侧 vfx_event ID → VfxPlayer 派发断言 + **未知 vfx ID 静默降级分支**；HUD 状态转换（置位→fade→复位）跨 tick 断言；无目标玩家在线时不 panic；同 tick 多事件（连杀）不互吞。

## P3 — 身份 → 生平记录（2 事件） ⬜

| 事件 | emit 点 | 生产 system（生产端注册点） | 新消费落点 → 输出 |
|------|---------|------------------------------|--------------------|
| IdentityCreatedEvent | `identity/command.rs:351` | `handle_identity_command`(:311)，注册 `identity/command.rs:106` | `cultivation::life_record` 写「立化名」条目 + narration（player） |
| IdentitySwitchedEvent | `identity/command.rs:380` | 同上 | life_record 写「换面示人」条目 + narration（player） |

- **测试契约（每事件全链）**：event → life_record 持久化**恰好一次**（重发事件不重复写条目）→ narration 路由 player scope；life_record 写失败分支不吞 narration（或反之，二者解耦断言）；条目 schema 与 `plan-life-record-epitaph-v1`（active）对齐（§8 #5）。
- 社交认知更新（NPC 认不认得出）属设计抉择，显式出 scope。

## P4 — 跨端观察批（2 事件 → Redis + 天道） ⬜

参照 `plan-bughunt-technique-feedback-bridge-v1` 的三段式（schema 通道 → server bridge → 天道 runtime）：

| 事件 | emit 点 | 生产 system（生产端注册点） | 新消费落点 → 输出 |
|------|---------|------------------------------|--------------------|
| VoidActionBroadcast | `cultivation/void/actions.rs:321` | `resolve_void_action_intents`(:156)，注册 `cultivation/void/mod.rs:26-29` | 新 Redis 通道 → 天道 agent 观察化虚者动向 |
| MigrationEvent | `fauna/migration.rs:385` | `fauna_migration_system`(:299)，注册 `fauna/mod.rs` register 块 | 迁徙 telemetry → agent 生态观察（兽潮已有独立 `BeastHordeEvent` reader，本条只补普通迁徙） |

- **测试契约（每通道全链）**：schema 正反 sample 对拍 + `REDIS_V1_CHANNELS` pin；server 侧 event → `RedisOutbound` 队列出现对应 payload；agent 侧新 runtime 订阅 → 合法 payload 产出观察/narration、**非法 payload reject 不发布、未知通道消息不 crash**；schema src 改后 `npm run build -w @bong/schema`。

## P5 — 防回归收口 ⬜

- **棘轮机制（guard 自带）**：`find_stale_triage_entries` 会在事件获得 EventReader 后**强制要求**从 `INTENTIONAL_UNCONSUMED_EVENTS` 删除对应条目——每阶段 PR 必须同步删除本批条目，否则 `cargo test` 红。
- **终态 pin 断言（按显式事件名集合，不做 follow_up 文本匹配）**：本 plan 五阶段全部落地后，上表 19 个「本 plan」事件名**全部不在** `INTENTIONAL_UNCONSUMED_EVENTS` 中；清单剩余条目 ⊆ {`CraftStartedEvent`, `FlowFieldPrototype`, `QiTransfer`, `TechniqueLearnedEvent`, `TechniqueMasteredEvent`, `TsySpawnResult`}（各自归属见盘点表；他 plan 落地后自行削减，不由本 plan 断言）。
- **代表性 e2e**：每类消费链选一个代表事件走完整链路——P0 选 TribulationFled（event→narration 聊天栏可见）、P1 选 ArtifactMeridianCracked（event→bridge→client HUD toast）、P2 选 FullPowerStrikeKilledEvent（event→vfx_event→client 粒子派发）、P3 选 IdentityCreatedEvent（event→bong.db life_record 行 + narration）、P4 选 VoidActionBroadcast（event→Redis→agent runtime 消费）；`bash scripts/smoke-test-e2e.sh` 全绿。

## §8 开放问题（P0 决策门前需收口）

1. **高频事件节流口径**：`InfluenceChangedEvent`（territory.rs 四处 emit，含 60s 批次与 PvP 即时路径）与 `NpcScheduleChangedEvent` 需 dedupe/throttle 策略（跨阈值播报 vs 冷却窗口），数值待实地统计 emit 频率后定。
2. **zone/player 路由 fallback**：部分事件（如 `BoneCoinCrafted`）可能缺 zone 上下文，narration scope 的 fallback 规则需统一（参 technique-feedback-bridge 同题）。
3. **InfluenceChanged 的 NPC 态度反应划界**：本 plan 只接叙事切片；「影响力 → NPC 行为改变」是独立设计题（Scorer/Action/Thinker 排序），须另立 plan，不得在本 plan 内顺手实装。
4. **SupplyCoffinOpened 与 plan-lootcrate-v1 协调**：lootcrate 骨架出料清单已提「`SupplyCoffinOpened` 类事件供天道 narration（可选）」——两 plan 谁先升 active 谁落地，后者复用不重复接。
5. **IdentitySwitched 与 plan-life-record-epitaph-v1 协调**：生平条目 schema 以该 active plan 为准，本 plan 只新增条目类型不改结构。
6. **P2/P4 是否需要新 server_data payload**：优先复用既有 payload 通道（vfx_event / event_stream），实在装不下再按字段清单新增——升 active 前逐条核对。
7. **TsyZoneInitialized 生产链特例**：当前唯一 emit 在 `apply_tsy_spawn_requests`（`world/tsy_dev_command.rs`，dev 命令触发的 spawn 请求消费 system）。升 active 时核验生产 TSY zone 初始化路径（非 dev 命令）是否也发该事件；若否，本条先做**生产链修复**（在真实初始化路径补 emit）再接叙事，工作量归 P0 但单独 commit。
