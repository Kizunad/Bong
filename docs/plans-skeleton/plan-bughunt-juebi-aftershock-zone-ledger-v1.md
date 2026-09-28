# plan-bughunt-juebi-aftershock-zone-ledger-v1

> 来源 Issue：#1407。绝壁劫余震直接清空 zone，并在恢复时只写回原值 50%，没有 qi ledger 记录。

## §0 摘要

`juebi_zone_aftershock_system` 首次命中 zone 时把 `zone.spirit_qi` 直接写成 0.0，余震结束或每 tick 恢复时又按 `original_qi * 0.5` 写回。`jue_bi_scar` 只是 active-event 标记，没有承接被清掉的真元。正 zone、负 zone 和恢复期间的其他流动都会被覆盖，导致 zone 字段与 `SPIRIT_QI_TOTAL` 漂移。

## §1 实际游玩体验影响

- 绝壁劫触发后，区域灵气最多只恢复一半；任何玩家都能观察到持续性的环境资源缩水。
- 负灵域的债务也会被直接归零，余震结束时再写入一个不代表真实流量的数值。
- 余震重复触发时，当前实现按字段覆盖而非按转账结算，无法审计损失来源。

## §2 复现路径

1. 在含有非零（或负值）`spirit_qi` 的 zone 触发 `JueBiTriggeredEvent`。
2. 运行 `juebi_zone_aftershock_system`，记录 `original_qi` 与 zone 当前值；当前首帧直接变成 0.0。
3. 在恢复窗口内让 heartbeat 或玩家改变该 zone，再跑到 `restore_until_tick`。
4. 系统将 zone 覆盖为 `original_qi * 0.5`，而 `WorldQiAccount` 没有相应的 transfer/sink。

## §3 根因证据

- `server/src/cultivation/tribulation.rs:1797-1827` 保存 `original_qi` 后立即执行 `zone.spirit_qi = 0.0`。
- `server/src/cultivation/tribulation.rs:1835-1855` 在到期和中间 tick 都按原值的 50% 直接覆盖 zone，忽略窗口内合法流动。
- `QiTransferReason::EraDecay` 是 qi_physics 中唯一可表达时代衰减的现有理由；若余震确实要损耗真元，必须把损耗作为明确的 era-decay 交易落账，不能用字段赋值伪造。
- `qi_physics::ledger::assert_conservation` 的守恒基线应引用 `SPIRIT_QI_TOTAL`，第三个 `era_decay` 参数只传真实记录的时代衰减。

## §4 非重复比对

- `plan-bughunt-explode-zone-return-ledger-v1.md` 处理化虚障爆发的 schedule 借还；这里是 JueBi 事件的 zone 余震，生命周期与 owner 不同。
- `tribulation_aoe_system` 的 #1410 是重复伤害/抽灵门禁，不能替代本 issue 的 zone 物理结算。
- 没有发现已归档 plan 为余震的 50% 恢复建立过 ledger sink；当前仍是生产路径可达的字段覆盖。

### 立项检查记录（2026-09-28）

- `docs/worldview.md`：检索“绝壁、渡虚劫、时代衰减、负灵域”，核对 §二 L18-L22、§三 L127-L130 与 §十六 L1566-L1569。
- `docs/finished_plans/`：检索 `juebi_zone_aftershock_system`、`JueBiZoneAftershock`、`original_qi`；命中 cultivation/tribulation 背景，但没有余震 zone sink。
- active plan：检索 `juebi_zone_aftershock_system`、`TribulationZoneTransfer`、`EraDecay`；`plan-container-filter-and-completion-v1.md` 仅列迁移符号，不覆盖 50% 覆盖写入。
- `docs/plans-skeleton/`：检索 `juebi_zone_aftershock`、`original_qi`、`jue_bi_scar`；已有 quota marker lifecycle skeleton 但不处理 zone ledger，未发现同主题 skeleton。

## 接入面与跨仓契约

- **Inputs**：`JueBiTriggeredEvent`、`JueBiZoneAftershocks`、`ZoneRegistry`、`CombatClock`、期间的 zone 流动和 `WorldQiAccount`。
- **Outputs**：余震的消耗、恢复和当前 zone 值有明确 `QiTransfer`，负 zone signed 值保留，失败不部分覆盖。
- **共享类型 / 事件**：`JueBiTriggeredEvent`、`JueBiZoneAftershocks`、`QiTransfer`、`QiTransferReason::EraDecay`（若确认是合法时代衰减）、`QiTransferReason::ReleaseToZone`、`WorldQiAccount`；不新增 agent/client payload。
- **server 契约符号**：`juebi_zone_aftershock_system`、`JueBiZoneAftershock`、`WorldQiAccount::transfer`、`qi_physics::ledger::transfer_ledger_qi_to_zone`、`qi_physics::ledger::assert_conservation`。
- **agent**：无变更。绝壁劫事件及 zone 物理目前由 server 结算，agent 不持有 zone ledger。
- **client**：无变更。现有 zone/劫事件快照足以呈现结算结果，不改变 payload。
- **worldview 锚点**：`docs/worldview.md §二 L18-L22`（真元总量与时代衰减边界）；`§三 L127-L130`（通灵至化虚的渡虚劫）；`§十六 L1566-L1569`（负灵域环境）。
- **qi_physics**：不把 `Zone.spirit_qi` 清零当作物理操作。若余震损耗不是 era decay，就必须保留原量并把影响建模为 signed zone 流动；若是 era decay，则显式构造 `QiTransfer { from, to, amount, reason: EraDecay }`，调用 `WorldQiAccount::transfer`，并让 `assert_conservation(before, after, era_decay)` 使用真实衰减量。所有 zone 回流仍通过 `transfer_ledger_qi_to_zone`，释放理由使用 `ReleaseToZone`。

## §5 修复骨架

### P0 余震流量化

- 先决定“减少 50%”是否是世界观允许的时代衰减；不成立则恢复完整原值，成立则为减少量建立稳定 sink 和 `QiTransfer`。
- 余震窗口内不覆盖其他系统的合法 zone 改动；按当前 signed 值计算恢复/衰减，不使用 `= 0.0` 或 `original_qi * 0.5` 的快照回写。

### P1 回归与守恒

- 覆盖正/负 zone、窗口内 heartbeat/玩家流动、重复事件和 ledger 失败。
- 断言每个减少量都有对应 transfer 或真实 era decay，并引用 `SPIRIT_QI_TOTAL`。
