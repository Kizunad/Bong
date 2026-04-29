# Bong · plan-botany-agent-v1 · 骨架

**植物生态快照接入天道 agent**。plan-botany-v1（✅）已实装 server 侧每 600 tick（~30s）发布 `BotanyEcologySnapshotV1` 到 `bong:botany/ecology` channel，`@bong/schema` TypeBox 和 Rust schema 均已就绪；但天道 tiandao agent 尚未订阅该 channel，也无处理逻辑。本 plan 补全 tiandao 订阅、生态数据摄取、天道灵气重分配决策、稀有变种 narrative 埋点。

**世界观锚点**：`worldview.md §七 灵气是零和的`（天道缓慢重分配灵气，从死域向无人区转移——生态快照是天道感知的数据源）· `worldview.md §七 天道察觉`（植物密度过高/灵气透支 = 天道"注意到"的信号之一）· `worldview.md §七 天道叙事风格`（暗语式提示，不是直白通知）

**library 锚点**：`docs/library/ecology/末法药材十七种.json`（Thunder / Tainted 变种的物理解释 + 采集风险）· `docs/library/ecology/辛草试毒录.json`（稀有变种辛度暴增 → 丹毒加重，值得天道关注）

**交叉引用**：
- `plan-botany-v1`（✅ 前置；`server/src/botany/ecology.rs` 已实装 `emit_botany_ecology_snapshot`，每 600 tick 发布）
- `plan-narrative-v1`（天道叙事接口；生态异常触发 narration）
- `plan-lingtian-v1`（✅；`ZonePressureState` 天道密度阈值已实装；生态快照可与 lingtian 压力数据联合决策）
- `plan-tribulation-v1`（活跃渡劫区附近若植物密度异常，天道可能联动加重叙事）
- `plan-spirit-eye-v1`（灵眼附近植物变种密度是"地形特殊"的间接信号）

**阶段总览**：
- P0 ⬜ tiandao 订阅 `bong:botany/ecology` + 解析 + 写入 WorldModel
- P1 ⬜ 灵气分配决策（哪些 zone 透支 → 触发重分配叙事）
- P2 ⬜ Thunder / Tainted 变种 narrative 埋点
- P3 ⬜ 联合 lingtian 压力数据做综合天道决策

---

## §0 设计轴心

- [ ] **天道视角而非玩家视角**：生态快照是天道的"眼"——agent 看的是整体生态状态，不是单个玩家行为
- [ ] **低频感知**：600 tick / 30s 发一次，agent 不做实时反应，做趋势判断（连续 N 次快照数据才触发决策）
- [ ] **叙事节制**：稀有变种出现不必立刻 narrate——只有在多个 zone 同时出现 Tainted 变种（生态异常）才触发，防止 narration 过于频繁
- [ ] **不干预具体玩家**：天道不知道"谁在采药"，只知道"哪个 zone 植物被采了多少"

---

## §1 第一性原理（烬灰子四论挂点）

- **音论·生态音场**：每种植物的生长/枯萎产生微弱"音"，天道通过整体音场密度感知生态健康——密集采集 = 音场突然变稀 = 天道察觉缺口
- **噬论·灵气透支**：zone 内植物快速减少 → 地表灵气吸附减弱 → 噬散加速（灵气流向天地，不回 zone）——这是天道"重分配"的触发机制
- **缚论·Tainted 植物**：被污染真元浸润的植物，其生长模式被"缚"住（偏离正常形态），成为世界异常的指示器
- **影论·稀有变种**：Thunder / Tainted 变种是特殊压力环境下的"次级投影"——在正常生态下极稀，密集出现说明某 zone 压力异常

---

## §2 server 侧现状核查

server 侧已就绪：
- `server/src/botany/ecology.rs`：`emit_botany_ecology_snapshot` 每 600 tick 发布
- `server/src/schema/botany.rs`：`BotanyEcologySnapshotV1 { tick, zones: Vec<BotanyZoneSnapshotV1> }`
- `server/src/schema/channels.rs`：`CH_BOTANY_ECOLOGY = "bong:botany/ecology"` 常量
- `agent/packages/schema/src/botany.ts`：`BotanyEcologySnapshotV1` TypeBox 已定义
- `agent/packages/schema/src/channels.ts`：`CHANNELS.BOTANY_ECOLOGY = "bong:botany/ecology"` 常量

**缺失**：`agent/packages/tiandao/src/redis-ipc.ts` 仅订阅 `WORLD_STATE` 和 `TSY_EVENT`，未订阅 `BOTANY_ECOLOGY`；无任何处理函数。

---

## §3 P0 — tiandao 订阅 + WorldModel 摄取

- [ ] **`redis-ipc.ts` 扩展**：在 `connect()` 中追加 `await this.sub.subscribe(CHANNELS.BOTANY_ECOLOGY)`
- [ ] **消息路由**：`handleMessage` switch case 加 `BOTANY_ECOLOGY` → 调 `onBotanyEcology(payload: BotanyEcologySnapshotV1)`
- [ ] **WorldModel 字段**：`WorldModel.botany_ecology: BotanyEcologySnapshotV1 | null`（保留最新一次快照）
- [ ] **摄取函数** `onBotanyEcology`：
  - 更新 WorldModel.botany_ecology
  - 计算 `zone_qi_utilization`（plant_count 趋势 vs spirit_qi 趋势）
  - 如果某 zone spirit_qi < 0.2 + plant_count 仍高 → 标记 `ZoneStressFlag`
- [ ] **tests**：mock Redis publish BotanyEcologySnapshotV1 → WorldModel 更新；schema validate 通过；payload 解析失败不 crash（log + skip）

---

## §4 P1 — 灵气分配决策

> worldview §七：天道缓慢重分配灵气，从死域向无人区转移。

- [ ] **决策触发条件**（连续 5 次快照均满足）：
  - 某 zone `spirit_qi < 0.15`（近枯竭）+ `Σ plant_count > threshold`（植物仍茂盛，说明大量采集后灵气未补充）
  - 另一 zone `spirit_qi > 0.85`（灵气富余，无人使用）
- [ ] **天道动作**：触发 `IReallocationNarration`（天道叙事）：
  - 风格："某处灵脉已瘦，无人应。另一处灵气渐聚，犹无人知。"（暗语式，不点名地点）
  - 实际 server 动作归 plan-lingtian（通知 zone qi 系统做重分配）——本 plan 仅在 agent 层发出叙事信号，不直接改 server 状态
- [ ] **发布**：agent 通过 `AGENT_NARRATE` 通道推送叙事，由 server narration handler 送到聊天栏（按 plan-narrative 现有链路）
- [ ] **tests**：mock 连续 5 次 zone A spirit_qi=0.1 + plant_count 高 → 触发 narration publish；少于 5 次不触发；两个 zone 均枯 → 不触发（无"向无人区转移"可用）

---

## §5 P2 — Thunder / Tainted 变种 narrative 埋点

> `BotanyZoneSnapshotV1.variant_counts: [{ variant: "none"|"thunder"|"tainted", count }]`

- [ ] **单 zone 异常阈值**：`tainted_count > 3 or thunder_count > 5`（单次快照）→ 记录到 `ZoneAnomalyLog`（本次不 narrate）
- [ ] **多 zone 联合异常**：连续 3 次快照，2+ zone 同时超 tainted 阈值 → 触发天道 narration：
  - 例："天地真元中有某种杂质在蔓延。枯藤上有紫斑，但此并非普通枯腐。"
- [ ] **Thunder 变种高密度**：某 zone thunder_count 突增 3x 本 zone 历史均值 → narration 暗示（"那片区域最近雷声频繁，草木都学会了蓄势"）——与 plan-tribulation 天劫关联
- [ ] **WorldModel 存储**：`zone_anomaly_history: Map<ZoneId, AnomalyWindow>`（滑动窗口 5 次快照）
- [ ] **tests**：mock tainted_count 突增 → AnomalyWindow 记录；连续 3 次 2 zone 异常 → narration emit；thunder 突增 → narration 发出

---

## §6 P3 — 联合 lingtian 压力数据决策

> `server/src/lingtian/pressure.rs`：`ZonePressureCrossed` 事件（LOW/MID/HIGH 四档）已实装，发至 Redis（待接）。

- [ ] **新增 IPC channel**：`bong:lingtian/zone_pressure`（server 在 `ZonePressureCrossed` 时发布 `LingtianZonePressureV1 { zone, level, raw_pressure, tick }`）— **注意：server 侧需补发布这一 channel**（当前仅内部 ECS 事件，未 emit Redis）
- [ ] **tiandao 订阅**：同 P0 模式，订阅 `CHANNELS.LINGTIAN_ZONE_PRESSURE`
- [ ] **联合决策**：当某 zone 同时出现 `HIGH lingtian_pressure` + `spirit_qi < 0.2`（生态快照）+ `tainted_count > 2` → 高置信度"这个 zone 被滥用了" → 天道发更强烈叙事（或触发 plan-lingtian §5.1 HIGH 路径：清 zone 所有 plot_qi）
- [ ] **tests**：mock lingtian HIGH + botany tainted → 联合触发强叙事；单独 HIGH 不触发联合；单独 tainted 不触发联合

---

## §7 数据契约（下游 grep 抓手）

| 契约 | 位置 |
|---|------|
| `subscribe(CHANNELS.BOTANY_ECOLOGY)` | `agent/packages/tiandao/src/redis-ipc.ts` |
| `WorldModel.botany_ecology: BotanyEcologySnapshotV1` | `agent/packages/tiandao/src/world-model.ts` |
| `ZoneAnomalyLog / ZoneStressFlag` | `agent/packages/tiandao/src/ecology-analyzer.ts`（新文件）|
| `CHANNELS.LINGTIAN_ZONE_PRESSURE` | `agent/packages/schema/src/channels.ts` |
| `LingtianZonePressureV1` TypeBox | `agent/packages/schema/src/lingtian.ts` |
| `bong:lingtian/zone_pressure` publish（server 补发）| `server/src/lingtian/pressure.rs`（补 Redis emit）|

---

## §8 实施节点

- [ ] **P0**：`redis-ipc.ts` 订阅 `BOTANY_ECOLOGY` + 路由 + `WorldModel.botany_ecology` 摄取 + `ZoneStressFlag` 计算 + schema 测试
- [ ] **P1**：灵气重分配决策（连续 5 次 + 双 zone 条件）+ `IReallocationNarration` emit + `AGENT_NARRATE` 链路
- [ ] **P2**：tainted / thunder 变种异常窗口 + 多 zone 联合判断 + narration 两类触发
- [ ] **P3**：server `bong:lingtian/zone_pressure` 补发 + tiandao 订阅 + 联合决策 HIGH×tainted 触发强叙事

---

## §9 开放问题

- [ ] `ZoneStressFlag` 是否需要持久化（跨 agent 重启保留状态）？当前 WorldModel 持久化用 Redis，可以存，但窗口数据量小，是否值得？
- [ ] 天道 narration 频率控制：生态异常可能持续很长时间，防止每 30s 一条叙事刷屏——需要 narration 冷却（同类型 narration 最少间隔 10 分钟）
- [ ] P3 中 server 补发 `bong:lingtian/zone_pressure`：是否等 plan-lingtian 后续 plan 统一处理，还是本 plan 内顺手补发（修改 server/src/lingtian/pressure.rs）？后者更快但跨 plan 边界
- [ ] 采药工具系统（botany §1.3）：右键即开 session 的现状后续加"采药刀 / 刨锄"影响品质 / 安全度——已由 `plan-tools-v1`（骨架，2026-04-29 立）覆盖；本 plan P3+ 接入时直接读 `ToolKind` enum 即可

---

## §10 进度日志

- **2026-04-27**：骨架立项。来源：`docs/plans-skeleton/reminder.md` "plan-botany-v1 → 天道 agent 钩子（待 agent 侧接入）"节。核查确认：server `emit_botany_ecology_snapshot` 已每 600 tick 发布（`ecology.rs:27`）；schema 双端就绪；tiandao `redis-ipc.ts:181-182` 仅订阅 `WORLD_STATE` 和 `TSY_EVENT`，`BOTANY_ECOLOGY` 未接。P0 是纯 TS 改动，无需 server 侧配合，可优先启动。
