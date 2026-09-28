# plan-bughunt-negative-zone-release-v1

> 来源 Issue：#1920。爆脉 spend_qi 释放时把负灵域当成零，抹平 signed debt 并可能凭空铸造真元。

## §0 摘要

`emit_spent_qi_release` 使用 `zone.spirit_qi.max(0.0) * QI_ZONE_UNIT_CAPACITY` 计算 zone 当前值。负灵域应保留 signed 缺口，但这里把负值改成零，再把 `qi_release_to_zone` 的结果写回 zone，导致负债被覆盖成正向流入。爆脉四招都可走这条 spend 路径，玩家随后可以吸收被错误创造的灵气。

## §1 实际游玩体验影响

- 在坍缩渊负灵域施法会把环境债务直接抹平，玩家得到超出守恒的可吸收真元。
- 同一技能在正 zone 与负 zone 的物理规则不同，负值越大漏洞越明显。
- 当前单元测试虽有其他模块锁住“不得 `.max(0.0)`”，爆脉入口仍未遵守。

## §2 复现路径

1. 准备 `zone.spirit_qi < 0.0` 的 zone 和带 `Cultivation` 的爆脉施法者。
2. 触发任一 `burst_meridian` spend path，使 `emit_spent_qi_release` 释放消耗量。
3. 观察 `zone_current` 被按 0 计算，`zone.spirit_qi` 随结果写回，负缺口被覆盖。
4. 对照 `full_power_strike.rs:360`、`death_hooks.rs:333` 等使用 signed 值的正确路径，并用 ledger 守恒对拍。

## §3 根因证据

- `server/src/cultivation/burst_meridian.rs:1037-1073`（当前函数行）把 `zone.spirit_qi` 先 `.max(0.0)`，再用 `qi_release_to_zone` 计算并写回。
- `server/src/cultivation/burst_meridian.rs` 的 spend 入口在 `spend_qi`，四个爆脉技能共享同一释放 helper。
- `server/src/cultivation/components/qi_flow.rs:641-653` 和 `qi_physics::ledger::transfer_ledger_qi_to_zone` 都支持 signed zone；把负值钳为 0 是调用方破坏守恒的额外步骤。

## §4 非重复比对

- `docs/finished_plans/plan-bughunt-full-power-strike-interrupt-refund-v1.md` 处理蓄力打断退款，不覆盖 burst_meridian 的负 zone release。
- `plan-bughunt-explode-zone-return-ledger-v1.md` 处理借还时的 zone 覆盖；本 issue 是每次 spend 的 signed room 计算。
- `npc_skill.rs`、`woliu.rs` 的负灵域锁测试是证据和回归先例，不是对 `burst_meridian` 的修复。

### 立项检查记录（2026-09-28）

- `docs/worldview.md`：检索“负灵域、反吸、灵压、真元”，核对 §二 L46-L54 与 §十六 L1566-L1569。
- `docs/finished_plans/`：检索 `negative zone`、`.max(0.0)`、`qi_release_to_zone`；命中暗器/负压相关修复，但没有 burst_meridian spend 入口。
- active plan：检索 `emit_spent_qi_release`、`burst_meridian`、`signed zone`；`plan-container-filter-and-completion-v1.md` 只列 transfer gateway，不覆盖此 signed baseline 错误。
- `docs/plans-skeleton/`：检索 `burst_meridian`、`zone.spirit_qi.max`、`negative zone`；已有暗器负灵域 skeleton 不涉及该入口，未发现同主题 skeleton。
- `docs/plans-skeleton/reminder.md`：已查阅并检索 `emit_spent_qi_release`、`burst_meridian`、`negative zone`，未发现同主题延后事项。

## 接入面与跨仓契约

- **Inputs**：施法者 `Cultivation.qi_current`、`ZoneRegistry`、`Position`/`CurrentDimension`、`qi_release_to_zone`、overflow ledger、`Events<QiTransfer>`。
- **Outputs**：current 减少量等于 zone accepted + overflow；signed `Zone.spirit_qi` 仍表示真实负债，所有 transfer 可审计。
- **共享类型 / 事件**：`qi_release_to_zone`、`QiTransfer`、`QiTransferReason::ReleaseToZone`、`transfer_ledger_qi_to_zone`、`transfer_external_qi_to_ledger`；不改 agent/client payload。
- **server 契约符号**：`spend_qi`、`emit_spent_qi_release`、`push_spent_qi_overflow`、`qi_release_to_zone`、`qi_physics::ledger::assert_conservation`。
- **agent**：无变更。爆脉资源结算在 server，agent 不持有 zone signed 值。
- **client**：无变更。现有技能结果和 cultivation/zone 快照字段足以呈现修复后的数值。
- **worldview 锚点**：`docs/worldview.md §二 L46-L54`（负灵域反吸真元）；`§十六 L1566-L1569`（坍缩渊负压环境）。
- **qi_physics**：zone 当前必须使用 `zone.spirit_qi * QI_ZONE_UNIT_CAPACITY`，不使用 `.max(0.0)`。玩家 current 是外部权威，扣除成功后沿 `ReleaseToZone` 生成审计；若从稳定 ledger 回写 zone，则调用 `transfer_ledger_qi_to_zone(&mut WorldQiAccount, from, zone_name, &mut zone.spirit_qi, requested, zone_ceiling, reason)`。测试引用 `SPIRIT_QI_TOTAL` 和 `assert_conservation`。

## §5 修复骨架

### P0 signed release

- 删除 `.max(0.0)`，沿用 signed zone 当前值；当 zone 为负且请求不足以填平债务时，结果仍可为负。
- 释放事务失败时不改 current、zone 或 overflow；不要用后置 clamp 修复浮点边界。
- 复核所有 burst_meridian spend 调用点，确保没有另一处把 signed zone 转成非负。

### P1 回归与守恒

- 覆盖 `-0.73` 一类深负 zone、接近零、正 zone、满 zone、无 zone overflow。
- 用 `assert_conservation(before, after, era_decay)` 对拍 `SPIRIT_QI_TOTAL`，断言负债未被铸平。
