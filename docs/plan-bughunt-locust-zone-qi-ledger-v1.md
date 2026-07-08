# plan-bughunt-locust-zone-qi-ledger-v1

> **BugHunt / server-qi r07**。一句话主题：灵蝗潮推进到新 chunk 时直接扣 `Zone.spirit_qi`，但没有 `WorldQiAccount` / `QiTransfer` 落账，导致蝗潮路径把区域灵气从守恒账里凭空抹掉。

## 阶段总览

| 阶段 | 主题 | 状态 |
|---|---|---|
| P0 | 灵蝗潮 zone drain 接入 qi ledger 守恒 | ⬜ |

## 背景与证据

守恒锚点：

- `docs/worldview.md §十 L872-L880`：全服灵气总量固定，区域灵气是零和资源。
- `docs/CLAUDE.md §四 L58-L60`：所有真元/灵气流动必须走 `qi_physics::ledger::QiTransfer`；`zone.spirit_qi -= Y` 无对应去向是守恒红旗；只 emit 无真实消费也不能当余额落账。

代码链路：

- `server/src/world/events.rs:1668-1685` 的 `tick_active_events` 参数只包含 `ZoneRegistry`、`QiDensityHeatmap`、掉落、rat bite / death event 等，没有 `WorldQiAccount`，也没有 `EventWriter<QiTransfer>`。
- 同文件 `server/src/world/events.rs:1813-1823` 的 `flush_collapse_qi_transfers` 只 flush `ActiveEventsResource.pending_qi_transfers`，注释和调用点都服务于坍缩重分配 overflow，不覆盖灵蝗潮。
- `server/src/world/events.rs:2620-2670` 的 `advance_locust_swarm` 在 `state.drained_chunks.insert(current_chunk)` 首次命中时，先 `heatmap.drain_heat(...)`，随后真实执行 `zone.spirit_qi = (zone.spirit_qi - LOCUST_ZONE_QI_DRAIN_PER_CHUNK).clamp(-1.0, 1.0)`。
- `server/src/world/events.rs:3756` 的现有测试名为 `locust_swarm_advance_drains_qi_loot_and_cultivator_qi_pressure`，覆盖 heatmap、掉落、RatBiteEvent 压力，但没有断言 `WorldQiAccount` 余额，也没有断言任何 `QiTransfer`。

账本缺口：

`LOCUST_ZONE_QI_DRAIN_PER_CHUNK = 0.05` 是 `Zone.spirit_qi` 的真实字段扣减。按现有 `QI_ZONE_UNIT_CAPACITY = 50` 口径，单个 chunk 约等于 2.5 绝对 qi 单位。灵蝗潮沿途每进一个新 chunk 都会让可修炼区域变贫瘠，但这部分灵气没有进入鼠群账户、天道账户、rift、overflow、era_decay_accum 或待分配池；因此 `summarize_world_qi` 能看到 zone 侧减少，却找不到对应增加项。

这不是纯表现层：`QiDensityHeatmap` 的降低可以解释为热图/权重变化，但 `Zone.spirit_qi` 直接影响修炼、生态、zone info 和后续 world qi 汇总，属于真实世界状态。

这也不是允许的天道外流：允许的系统外流只应走时代衰减并累计到 `era_decay_accum`；灵蝗潮路径没有调用 `era_decay_step` / `WorldQiBudget::apply_era_decay`，也没有任何追踪槽。

## 实际游玩体验影响

玩家会看到灵蝗潮沿路线把灵气区压成贫瘠区：打坐回复变慢或中断，区域生态和危险判定改变，掉落被吞、鼠群咬击继续发生。问题在于这不是“鼠群把灵气带走并可追踪”，而是灵气从世界账本里无声消失。

长距离蝗潮或无人区推进会让多个 zone/chunk 的灵气永久少一截，后续平衡回流、zone HUD、天道生态判断可能与 ledger 视角分裂。玩家体感会是“蝗潮过境后地脉突然枯了”，但管理员/测试从守恒账看不到这批灵气去了哪里，难以判断是正常生态迁移还是账本蒸发。

## 去重结论

- 不重复 #1050：那是 craft `qi_cost` 固定落 `zone:spawn` 账户。
- 不重复 #1056：那是 NPC 日程/休息/QiSpring/Far LOD 凭空恢复真元。
- 不重复 #1076：那是灵田 `plot_qi` 未进 `WorldQiAccount`。
- 不重复 #969-#1077 区间其它主题：近期 server-qi 多数覆盖技能消耗归还、overflow、死亡释放、骨币、heartbeat、灵物磨损；没有覆盖灵蝗潮行进时的 zone drain。
- `docs/plan-bughunt-locust-warning-duration-contract-drift-v1.md` 是客户端预警时长 contract drift，只涉及 `duration_ticks` 消费，不涉及 `Zone.spirit_qi` / `WorldQiAccount`。
- `docs/finished_plans/plan-ambient-threat-v1.md` 明确说 spawner 本身不动灵气，P2 rat 咬击复用 `RatBiteDrain` 守恒路径；它没有给灵蝗潮路径扣 zone 灵气落账。

## P0 — 灵蝗潮 zone drain 接入 qi ledger 守恒

- [ ] 明确灵蝗潮路径扣减的账本语义：推荐新增 `QiTransferReason::LocustSwarmDrain`，不要复用 `EraDecay`，因为这是局部生态灾害，不是全服时代衰减。
- [ ] 给 `tick_active_events` / `advance_locust_swarm` 接入真实 `WorldQiAccount` 或等价可追踪余额写入路径。若 Bevy 参数已满，参考同文件 `flush_collapse_qi_transfers` 拆分 system，但不能只发无消费者的 `QiTransfer` event。
- [ ] 计算实际扣减量必须基于 `before - after`，低于 `-1.0` clamp 边界时只转移真实减少量，不能按常量多记账。
- [ ] 目标账户需按最终设计收口：若语义是“鼠群吸走”，进入稳定 swarm/npc 账户，并在鼠群解散/死亡时释放或结算；若语义是“生态灾害沉降”，进入专用 overflow/tiandao-like tracked sink。无论选哪种，必须在 `WorldQiAccount` 中可追踪，不能只改字段。
- [ ] 保持 heatmap / 掉落 / RatBiteEvent 行为不退化；rat 咬玩家真元仍走既有 `RatBiteDrain`，不要把玩家真元咬击和 zone drain 混成一笔。
- [ ] 日志或测试事件应暴露 `zone_name`、`chunk`、`actual_drained_qi`，方便定位蝗潮造成的区域枯竭。

## 验收抓手

- [ ] 新增单测：灵蝗潮首次进入新 chunk 后，`zone.spirit_qi` 降低的绝对量等于目标账户或 tracked sink 的 `WorldQiAccount` 增量。
- [ ] 新增单测：重复进入同一个 `drained_chunks` chunk 不重复扣 zone，也不重复写账。
- [ ] 新增单测：`zone.spirit_qi` 接近 `-1.0` 时只按 `before - after` 落账，防止 clamp 边界多记。
- [ ] 新增单测：缺 `QiDensityHeatmap` 时仍执行真实守恒落账；heatmap 是表现/权重，不是账本前置条件。
- [ ] 新增回归：现有 `locust_swarm_advance_drains_qi_loot_and_cultivator_qi_pressure` 的掉落吞噬与 RatBiteEvent 仍通过。
- [ ] 守恒快照测试取 `qi_physics::constants`，不要写死全服总量字面值。

## 对抗结论

### Round 1（Hypatia）

结论：候选成立，置信度高。理由是 `advance_locust_swarm` 在新 chunk 上直接扣 `zone.spirit_qi`，而 `tick_active_events` / `advance_locust_swarm` 都没有 ledger 参数。按 `QI_ZONE_UNIT_CAPACITY=50` 估算，每 chunk 蒸发约 2.5 绝对 qi。近期 PR 未覆盖该路径。

### Round 2（Euclid）

结论：反方未能推翻，继续成立。反驳检查确认：

- 没有 `WorldQiAccount` 同步，`pending_qi_transfers` 仅服务坍缩路径。
- 不是 heatmap 表现层，因为真实 `Zone.spirit_qi` 被扣。
- 不是允许的天道外流，因为没有进入 `era_decay_accum` 或任何 tracked sink。
- 现有 locust warning plan 只覆盖客户端预警时长；历史 rat bite 守恒只覆盖咬玩家真元，不覆盖路径 zone drain。
- 同文件域崩路径已有 `QiTransfer` overflow 先例，反证灵蝗潮直接扣字段缺少守恒兜底。

## 审计来源

BugHunt worker：server-qi 分区 r07。主代理静态复核 + 两轮对抗 subagent（Hypatia / Euclid）。本 PR 仅报告 plan，不修改代码、配置、资源或依赖。
