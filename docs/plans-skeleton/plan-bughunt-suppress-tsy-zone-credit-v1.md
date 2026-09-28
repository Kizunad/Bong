# plan-bughunt-suppress-tsy-zone-credit-v1

> 来源 Issue：#1492。SuppressTsy 从在线玩家扣真元写入 zone 镜像账户，却没有增加 `Zone.spirit_qi`。

## §0 摘要

`cast_suppress_tsy` 调用 `debit_caster_qi_to_account`，把玩家的 200 真元转给 `QiAccountId::zone(zone_id)`，但随后只更新 Tsy 生命周期，不更新对应 `Zone.spirit_qi`。zone 账户是由字段派生的镜像，后续 dormant regen 会用 zone 字段覆盖它，因此玩家支付的真元最终被吞掉，区域也没有得到应有的灵气。

## §1 实际游玩体验影响

- 化虚者支付镇压代价后，坍缩渊没有获得任何灵气改善。
- 后续 zone 同步会抹掉临时账户余额，造成一次真实的总量损失。
- 同类化虚动作可能形成“扣玩家但不改环境”的隐形经济漏洞。

## §2 复现路径

1. 准备在线玩家 `Cultivation.qi_current >= 200`、目标 Tsy zone、`WorldQiAccount` 和 zone registry。
2. 调用 `cast_suppress_tsy`，记录调用前后 `Cultivation.qi_current`、`Zone.spirit_qi` 与 zone ledger balance。
3. 当前玩家 current 减少、zone 账户增加，但 zone 字段不变；运行 `apply_dormant_regen_with_multiplier` 后账户又被字段镜像覆盖。
4. 观察 `QiTransfer` 审计与 `SPIRIT_QI_TOTAL`，确认没有稳定的 zone 物理落点。

## §3 根因证据

- `server/src/cultivation/void/actions.rs:340-380` 的 SuppressTsy 分支只调用 `debit_caster_qi_to_account`，没有传入可写的 `Zone`。
- `server/src/cultivation/void/ledger_hooks.rs:60-87` 的 helper 先把玩家 current 镜像到 player 账户，再执行 `WorldQiAccount::transfer`；它不能代替 zone 字段更新。
- `server/src/cultivation/void/ledger_hooks.rs:1824-1831`（当前 grep 位置）会以 `Zone.spirit_qi` 重建 zone 账户，证明该账户不是独立的环境权威。
- 在线玩家真元权威始终是 `Cultivation.qi_current`；若需要从外部源入账，应核对 `transfer_external_qi_to_ledger` 的真实签名，而不是把不存在的 player 余额当作来源。

## §4 非重复比对

- `plan-bughunt-void-barrier-dispel-qi-ledger-v1.md` 处理障折半；SuppressTsy 是玩家付费进入 zone 的正向流量。
- `plan-bughunt-explode-zone-return-ledger-v1.md` 处理 ExplodeZone 借还生命周期；本 issue 的缺口发生在一次性动作提交。
- `docs/finished_plans/plan-bughunt-void-actions-v1.md` 记录动作门禁和成本，但未覆盖 zone 字段与账户镜像的一致性。

### 立项检查记录（2026-09-28）

- `docs/worldview.md`：检索“化虚、坍缩渊、镇压、灵气”，核对 §二 L18-L22 与 §十六 L1566-L1569。
- `docs/finished_plans/`：检索 `cast_suppress_tsy`、`debit_caster_qi_to_account`、`zone.spirit_qi`；命中 void actions 总 plan，但没有 SuppressTsy 的字段 credit。
- active plan：检索 `SuppressTsy`、`apply_dormant_regen_with_multiplier`、`ZoneQiTransfer`；`plan-container-filter-and-completion-v1.md` 只有迁移清单，不覆盖该一次性动作根因。
- `docs/plans-skeleton/`：检索 `cast_suppress_tsy`、`SuppressTsy`、`zone credit`；已有 target-zone lock skeleton 但未覆盖 qi credit，未发现同主题 skeleton。

## 接入面与跨仓契约

- **Inputs**：`VoidActionIntent::SuppressTsy`、施法者 `Cultivation.qi_current`/`ActorQiIdentity`、目标 `Zone`、`TsyZoneStateRegistry`、`WorldQiAccount`、`Events<QiTransfer>`。
- **Outputs**：玩家 current 减少的金额等量增加目标 zone 的 signed `spirit_qi`，并留下一个可核验的 `QiTransfer`；生命周期状态照旧更新。
- **共享类型 / 事件**：`Cultivation::release_to_zone`、`QiTransfer`、`QiTransferReason::VoidAction`（动作成本）与 `ReleaseToZone`（回流理由）、`transfer_external_qi_to_ledger`、`transfer_ledger_qi_to_zone`；不改 agent/client payload。
- **server 契约符号**：`cast_suppress_tsy`、`debit_caster_qi_to_account`、`Cultivation::release_to_zone`、`release_external_qi_to_zone`、`apply_dormant_regen_with_multiplier`、`qi_physics::ledger::assert_conservation`。
- **agent**：无变更。SuppressTsy 由 server 处理，agent 不保存 zone ledger 余额。
- **client**：无变更。已有 zone/cultivation 快照可以显示新的值，不增加字段。
- **worldview 锚点**：`docs/worldview.md §二 L18-L22`（真元流动与总量）；`§十六 L1566-L1569`（坍缩渊环境）。
- **qi_physics**：从 `Cultivation.qi_current` 扣除后，优先在同一事务调用 `Cultivation::release_to_zone(Some(zone), ledger, actor, amount, QiTransferReason::ReleaseToZone)`；该路径会更新 signed zone 字段并在 zone 满/缺失时使用真实 overflow。若必须进入稳定账户，使用 `transfer_external_qi_to_ledger(&mut WorldQiAccount, from, to, amount, reason)` 的真实签名。不要先 transfer 到 zone 镜像再单独改字段造成双计。

## §5 修复骨架

### P0 玩家→zone 原子流

- 在状态变更前取得目标 zone 的可变引用，把动作成本与 zone credit 放进失败原子事务；zone 缺失或 ledger/身份校验失败时不扣玩家、不推进 lifecycle。
- 统一区分动作审计理由和真元回流理由，最终 zone credit 必须是 `ReleaseToZone` 或经核实的等价 balance-mutating reason。
- 删除只写 zone 账户、不写字段的路径，避免 dormant regen 覆写已付真元。

### P1 回归与守恒

- 覆盖正/负/满 zone、zone 缺失、ledger 失败和 lifecycle 失败；断言 current、zone 字段、overflow、audit 一致。
- 用 `assert_conservation(before, after, era_decay)` 对拍 `SPIRIT_QI_TOTAL`，引用常量而不是字面量。
