# plan-ambient-ratbite-ledger-leak-v1（active）

> **Active（已从 skeleton 升级，待逐项消费）**。一句话主题：`plan-ambient-threat-v1` 接入的 ambient Rat 在正常玩家路径上会通过 `RatBite` 真实扣掉 `Cultivation.qi_current`，但只 emit `QiTransfer(RatBiteDrain)` 事件、没有任何账本落账；rat 死亡/超距回收又只把 `drained_qi` 的 1% 回写 zone，导致大部分被咬走的真元在守恒口径上长期蒸发。影响是：**新手/低 danger 区正常游玩就会被鼠患稳定偷真元，恢复压力被放大，且全服 qi 守恒审计会持续失真**。

> 立项动机：当前 `origin/main` 的 `plan-ambient-threat-v1` 文档把“emit `QiTransfer` event”误判成“已完成守恒落账”，但 `qi_physics::register` 并没有消费该事件写入 `WorldQiAccount` 的通用系统。该缺口位于最近合入、玩家高频可达的环境威胁主链，值得先立 skeleton plan 收口证据、修复面与验收抓手，再单独出 fix PR。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | ambient Rat 鼠咬真元漏账 / 99% 蒸发回归 | fix_pr | ⬜ |

## P0 — ambient Rat 鼠咬真元漏账 / 99% 蒸发回归

- **现象**：`server/src/combat/rat_bite.rs` 的 `apply_rat_bite_qi_drain` 会直接扣目标玩家 `cultivation.qi_current`，并把等量值累加到 `RatBlackboard.drained_qi`；随后只 `qi_transfers.send(QiTransfer::new(..., RatBiteDrain))`，没有 `WorldQiAccount::transfer`、没有 `push_transfer_audit`、也没有别的通用 `EventReader<QiTransfer>` system 负责把该 event 写回 ledger。结果是：**玩家真元真实减少了，但账本侧并没有出现等量去向**。
- **可达链路**：`server/src/npc/mod.rs` 正式注册 `spawn::ambient_scheduler::register(app)`；`server/src/npc/spawn/ambient_scheduler.rs` 把 danger 1~2 的 ambient threat pool 固定为 `ThreatSpecies::Rat`，`server/zones.json` 的 `spawn` 区就是 `danger_level: 1`；rat 由 `ambient_threat_pool_fn -> spawn_rat_npc_at` 正式落地，`brain_rat.rs` 中 `HarassPlayerQuery` 明确锁定 `With<ClientMarker>` 在线玩家，近身时 `harass_bite_action_system` / `seek_qi_source_action_system` 都会 emit `RatBiteEvent { qi_steal: 1 }`；`combat/mod.rs` 再把 `RatBiteEvent` 与 `apply_rat_bite_qi_drain` 接到正式 combat 主链。也就是说，**新手区/低 danger 区正常走图就能吃到这条扣真元路径**。
- **为什么这是 bug，不是设计**：`server/src/qi_physics/ledger.rs` 对 `RatBiteDrain` 的文档写的是“守恒转入 RatBlackboard.drained_qi”，而不是“允许 99% 永久蒸发”；仓库守恒红线也明确要求活体 qi 的减少量必须有等量去向。当前实现却只在 rat 死亡/超距回收时，通过 `return_rat_drained_qi_to_zone` 把 `drained_qi * 0.01` 回写 zone，意味着其余 99% 既不在玩家侧、也不在 zone 侧、也不在 ledger 可审计账户里，形成真实守恒缺口。
- **对实际游玩体验的影响**：低 danger 环境威胁原本应是“骚扰/消耗型压力”，现在却变成**可反复偷空玩家真元且几乎不归还环境**。对玩家侧，表现为新手区/过渡区被鼠患反复咬时恢复节奏异常变差、坐定/练功更容易被掐断；对服务器侧，表现为 `summarize_world_qi` 一类守恒审计口径持续低估世界中应存在的 qi，总量会随 rat bite 累积偏移。
- **建议修复范围 / 模块**：优先收口 `server/src/combat/rat_bite.rs`、`server/src/qi_physics/ledger.rs`、必要时补 `fauna/rat_phase.rs`。修复方向应明确选一条并统一语义：要么把 `RatBiteDrain` 做成标准 audit-only 守恒路径（像其他“活体 qi 在 ECS、去向在 ledger/zone”的场景那样显式 `push_transfer_audit` / 同步真实去向），要么把 rat 持有的 `drained_qi` 接成真实可审计账户并在死亡/回收时做完整结算；无论选哪条，**都不能继续停留在“只发 event、无人消费”的半接线路径**。
- **验收抓手**：至少补 4 组 pin。1) `RatBite` 命中后，玩家 `qi_current` 减少量必须与 rat/ledger/zone 去向量严格对齐。2) rat 死亡与超距回收都不能把未结算 `drained_qi` 吞掉。3) `summarize_world_qi` / 守恒断言在 rat bite 循环前后不应出现额外 drift。4) danger 1 的 `spawn` 区端到端模拟里，在线玩家被 ambient Rat 咬后，这条去向在审计轨迹中可观测。

## 反方裁决摘要

1. Round 1（本机本地模型，默认怀疑）没有提出任何能落到代码点位的实质反证，只给出“也许 `QiTransfer` 事件别处被消费”的弱怀疑。
2. Round 2 在补入 `qi_physics::register` 仅注册 event、无全局消费器，以及 `ambient_threat_pool_fn -> spawn_rat_npc_at` 与 `HarassPlayerQuery(With<ClientMarker>)` 这两条后，仍未给出新的代码级反证；可达性怀疑被排除，只剩模型输出质量不足。
3. 人工复核进一步确认：仓库内 audit-only 真实范式都是调用点自己 `push_transfer_audit` / `set_balance` / `transfer`，`RatBite` 三者全缺；因此该候选在两轮对抗后继续存活。

## 开放问题

1. `RatBiteDrain` 应该归类为 audit-only 留痕，还是应该给 rat 接真实 ledger 账户并在死亡/回收时完整清算？需要在修复 PR 中一次性定清语义，避免继续半接线。
2. `MimicSpider` 目前也走“`drained_qi` 持有 + 1% 死亡回灌”家族语义；修 rat 时是否顺手复核同类实现，防止只补一处、继续保留同型守恒缺口。

## 审计来源

bug-hunt 定点轮（仅收窄 `plan-ambient-threat-v1` / 当前 `HEAD` 附近 server-side gameplay 代码）。路线限定为环境威胁、spawn、AI、守恒、生命周期；候选经主代理人工复核 + 本机反方子代理两轮默认怀疑裁决后保留。当前结论是 **report-only**：先提交 skeleton plan，把玩家影响、可达链路、修复面与验收抓手讲清，再由后续 fix PR 单独落地守恒修复。
