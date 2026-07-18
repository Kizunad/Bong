# plan-unconsumed-event-feedback-v1 — 未消费领域事件反馈批量接线

> 主题：`server/src/test_coverage_guards.rs` 的 `INTENTIONAL_UNCONSUMED_EVENTS` 是一份**编译期强制登记**的「emit 了但没有任何 EventReader」triage 清单（24 条）。其中 19 条 `DeferredFollowUp` 的 follow_up 指向"feedback / narration / telemetry 接线"却**没有任何 plan 认领**——玩家侧表现为：炼器裂痕零回执、全力一击击杀无演出、渡劫逃逸无叙事、铸骨币无经济反馈等一大批"效果生效了但玩家/天道毫无感知"的半闭环。本 plan 按消费端类型分批把这些事件接上**现成的** narration / HUD / VFX / Redis 桥基建（`server/src/network/` 下已有约 90 个 `*_emit.rs` / `*_bridge.rs` 范本），并利用 guard 自带的 stale 检测形成防回归棘轮。
>
> 来源：2026-07-18 全仓扫描（server Explore agent + plan 覆盖面盘点交叉比对）。

## 阶段总览

| 阶段 | 内容 | 状态 |
|----|------|----|
| P0 | 叙事批（7 事件 → narration emit，纯 server） | ⬜ |
| P1 | 炼器反馈三连（bridge + client HUD toast） | ⬜ |
| P2 | 战斗/身体演出批（5 事件 → VFX + SFX + HUD） | ⬜ |
| P3 | 身份 → 生平记录（2 事件 → life_record + narration） | ⬜ |
| P4 | 跨端观察批（2 事件 → Redis 通道 + 天道 agent 观察） | ⬜ |
| P5 | 防回归收口（triage 清空核验 + e2e） | ⬜ |

## 接入面 Checklist

- **进料**：`INTENTIONAL_UNCONSUMED_EVENTS` 清单中 19 条 `DeferredFollowUp` 事件（生产者全部已存在且在跑，file:line 见各阶段块）；`cultivation::life_record` 生平记录模块；`server/src/network/` 既有 emit/bridge 基建。
- **出料**：narration（聊天栏/事件流）、client HUD toast/VFX/SFX、`bong.db` 生平记录、Redis 通道（P4 新增 2 条）→ 天道 agent 观察。
- **共享类型 / event**：**不新增任何 Bevy event**——本 plan 只给既有 event 加 EventReader 消费者。narration 走既有 `NarrationEvent` 管线；VFX 走既有 `bong:vfx_event` 管线。
- **跨仓库契约**：P1/P2 若新增 server_data payload 按 `reference_server_data_payload_field` 清单双端落地（proto + struct + From + convert + emit + schema regenerate）；P4 新增 Redis 通道需 `agent/packages/schema/src/channels.ts` + `server/src/schema/channels.rs` 双端登记 + sample 对拍。
- **worldview 锚点**：`worldview.md §四`（战斗物理可见性——招式/击杀效果必须可感知）、`§九`（经济——骨币铸造是经济事件）、`§十二`（天道观察世界——agent 需要感知面）。
- **qi_physics 锚点**：**本 plan 零真元流动**——所有接线均为只读观察者（EventReader 读事件发反馈），不触碰 `qi_current` / `zone.spirit_qi` / ledger。任何阶段若发现需要动 qi 记账，立即停下重新设计。

## 范围划定（查重结论，2026-07-18）

triage 清单 24 条中**排除**以下 5 条（已有归属，不重复认领）：

| 事件 | 排除理由 |
|------|----------|
| `TechniqueLearnedEvent` / `TechniqueMasteredEvent` | 已有 `plans-skeleton/plan-bughunt-technique-feedback-bridge-v1.md` 完整认领（schema 通道 + Redis bridge + 天道 narration runtime） |
| `CraftStartedEvent` | follow_up 明确归 `plan-craft-v1` P2/P3 |
| `FlowFieldPrototype` | follow_up 明确归 `plan-beast-horde-v1` cleanup（active plan） |
| `QiTransfer` | `DirectResourceConsumer`——守恒审计事件，设计上不要求 EventReader |
| `TsySpawnResult` | `AuditOnly`——dev-only 观测事件 |

与 `plans-skeleton/plan-module-wiring-gaps-v2.md` 的边界：v2 是**涉 gameplay 设计抉择**的 report-only 决策菜单（17 主题），本 plan 只收**不需要设计拍板的机械反馈接线**；`InfluenceChangedEvent` 的 **NPC 态度反应**属设计抉择（与 v2 T4 同类），本 plan 只接其**叙事切片**，NPC 反应显式留给未来 territory 设计 plan（见 §8 #3）。

---

## P0 — 叙事批（7 事件 → narration，纯 server） ⬜

每条 = 一个小 EventReader system（照抄 `server/src/network/` 同域既有 `*_emit.rs` 模式），产出 narration。narration 文案示例为骨架草稿，升 active 时按 docs/CLAUDE.md 视听规格补齐至 2-3 条/事件并标 scope/style：

| 事件 | 生产者 | 落点模块 | narration 草稿（scope / style） |
|------|--------|----------|--------------------------------|
| `TribulationFled` | `cultivation/tribulation.rs` | `network/tribulation_*_emit.rs` 同域扩展 | 「有人临劫而逃，天雷余怒未消。」（zone / perception） |
| `TsyZoneInitialized` | tsy zone 初始化路径 | `network/tsy_event_bridge.rs` 扩展 | 「坍缩渊又一处裂面睁开了眼。」（broadcast / narration） |
| `NpcScheduleChangedEvent` | `npc/schedule.rs:361` | npc 叙事 emit 新增 | 仅对玩家可见 NPC 播报作息变化（zone / perception，需节流，见 §8 #1） |
| `BoneCoinCrafted` | `fauna/bone_coin.rs:132` | 经济叙事 emit 新增 | 「兽骨入阵，又一枚骨币成了。」（player / narration）+ 经济 telemetry 计数 |
| `SupplyCoffinOpened` | 物资棺交互路径 | coffin emit 同域扩展 | 开棺统计 + narration（player / narration）；与 `plan-lootcrate-v1` 协调（§8 #4） |
| `ZoneEnvironmentLifecycleEvent` | zone environment 资源 | zone 叙事 emit 扩展 | 环境相变播报（zone / perception）——仅叙事切片，可视化归 P2 之后评估 |
| `InfluenceChangedEvent` | `world/territory.rs:424/481/653/672` | territory 叙事 emit 新增 | 「此地的气息渐渐换了主人。」（zone / narration，跨阈值才播，见 §8 #1/#3） |

- 测试：每事件一条「event 发出 → narration 队列出现对应条目」专项测试 + 节流边界测试。

## P1 — 炼器反馈三连（bridge + client HUD） ⬜

炼器是玩家长交互流程，三个关键节点当前零回执：

| 事件 | 生产者 | 消费 |
|------|--------|------|
| `ArtifactMeridianDepthChanged` | `forge/artifact_meridian.rs:601` | forge HUD 深度进度更新 + narration |
| `ArtifactMeridianCracked` | `forge/artifact_meridian.rs:613` | HUD 裂痕警告 toast（红色）+ SFX + narration「器脉一声闷响，裂了。」 |
| `ForgeProcessingAccepted` | `forge/processing_mode.rs:84` | HUD 受理确认（对齐 `feedback_bot_client_e2e` 的"请求必有回执"约定） |

- 落点：扩展 `network/forge_bridge.rs` / `forge_snapshot_emit.rs`（一个 bridge 收三事件）；client 侧走既有 forge HUD 面板。
- SFX：`audio_recipe` JSON，裂痕用 `minecraft:block.anvil.place`(pitch 0.6, volume 0.5) 打底，升 active 时定稿。
- 测试：三事件各自的 bridge 转换专项测试 + client HUD 状态测试；bot 场景组补 forge 反馈断言（`scripts/bot/scenarios/`）。

## P2 — 战斗/身体演出批（5 事件 → VFX/SFX/HUD） ⬜

「爽点时刻」效果已生效但无演出。VFX 基建现成：`vfx_event_emit.rs` / `gameplay_vfx.rs` / `vfx_animation_trigger.rs`。视听规格骨架如下，升 active 时按 docs/CLAUDE.md §四精度（基类/数量/lifetime/hex/spawn 模式/贴图 ID）逐条定稿：

| 事件 | 生产者 | 演出草稿 |
|------|--------|----------|
| `FullPowerStrikeKilledEvent` | `cultivation/full_power_strike.rs:555` | 击杀特写：受击点 burst 粒子 + 短促重音 SFX + 事件流条目「全力一击」 |
| `YidaoCastCompleteEvent` | yidao cast 路径 | 收刀顿帧反馈：HUD 刀势条归位动画 + SFX |
| `SwordBondFormedEvent` | sword_path 结契路径 | 结契仪式演出：剑体环绕粒子（continuous, ~40 tick）+ HUD toast「剑契已成」+ narration |
| `TurbulenceFieldDecayed` | `combat/woliu_v2/` | 涡流场消散粒子（radial 扩散淡出）——补上"场没了"的视觉信号 |
| `DigestionOverloadEvent` | `cultivation/poison_trait/tick.rs:105` | HUD 反胃 vignette（绿色调 tint, ~30 tick fade）+ SFX + 状态条提示 |

- 每条走既有 `bong:vfx_event` ID 注册 + client `VfxPlayer`；测试锁 emit → vfx_event payload 结构。

## P3 — 身份 → 生平记录（2 事件） ⬜

| 事件 | 生产者 | 消费 |
|------|--------|------|
| `IdentityCreatedEvent` | `identity/` 命令路径 | `cultivation::life_record` 写「立化名」生平条目 + narration（player） |
| `IdentitySwitchedEvent` | `identity/` 切换路径 | life_record 写「换面示人」条目；与 `plan-life-record-epitaph-v1`（active，~30%）协调落点（§8 #5） |

- 化名/换身份是修仙叙事强钩子，先落生平记录闭环；社交认知更新（NPC 认不认得出）属设计抉择，显式出 scope。

## P4 — 跨端观察批（2 事件 → Redis + 天道） ⬜

参照 `plan-bughunt-technique-feedback-bridge-v1` 的三段式（schema 通道 → server bridge → 天道 runtime）：

| 事件 | 生产者 | 消费 |
|------|--------|------|
| `VoidActionBroadcast` | 化虚行为路径 | 新增 Redis 通道（`channels.ts` + `channels.rs` 双端登记）→ 天道 agent 观察化虚者动向；化虚是最高境界，天道理应看见 |
| `MigrationEvent` | `fauna/migration.rs` | 迁徙 telemetry → agent 生态观察（兽潮已有独立 `BeastHordeEvent` reader，本条只补普通迁徙的观察面） |

- schema 改动连同 `agent/packages/schema/samples/*.json` 正反 sample + `REDIS_V1_CHANNELS` pin 测试一起落；schema src 改后必须 `npm run build -w @bong/schema`。

## P5 — 防回归收口 ⬜

- **棘轮机制（guard 自带）**：`test_coverage_guards.rs` 的 `find_stale_triage_entries` 会在事件获得 EventReader 后**强制要求**从 `INTENTIONAL_UNCONSUMED_EVENTS` 删除对应条目——每个阶段的 PR 必须同步删除本批条目，否则 `cargo test` 红。P5 终态断言：清单中 `DeferredFollowUp` 且 follow_up 含 "feedback/narration" 的条目归零（仅剩 technique 双事件等已划归他 plan 的）。
- e2e：`bash scripts/smoke-test-e2e.sh` 全绿；bot 场景组对 P1 forge 回执、P2 击杀演出补断言。

## §8 开放问题（P0 决策门前需收口）

1. **高频事件节流口径**：`InfluenceChangedEvent`（territory.rs 四处密集 emit）与 `NpcScheduleChangedEvent` 需 dedupe/throttle 策略（跨阈值播报 vs 冷却窗口），数值待实地统计 emit 频率后定。
2. **zone/player 路由 fallback**：部分事件（如 `BoneCoinCrafted`）可能缺 zone 上下文，narration scope 的 fallback 规则需统一（参 technique-feedback-bridge 同题）。
3. **InfluenceChanged 的 NPC 态度反应划界**：本 plan 只接叙事切片；「影响力 → NPC 行为改变」是独立设计题（Scorer/Action/Thinker 排序），须另立 plan，不得在本 plan 内顺手实装。
4. **SupplyCoffinOpened 与 plan-lootcrate-v1 协调**：lootcrate 骨架出料清单已提「`SupplyCoffinOpened` 类事件供天道 narration（可选）」——两 plan 谁先升 active 谁落地，后者复用不重复接。
5. **IdentitySwitched 与 plan-life-record-epitaph-v1 协调**：生平条目 schema 以该 active plan 为准，本 plan 只新增条目类型不改结构。
6. **P2/P4 是否需要新 server_data payload**：优先复用既有 payload 通道（vfx_event / event_stream），实在装不下再按字段清单新增——升 active 前逐条核对。
