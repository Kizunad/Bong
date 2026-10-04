# plan-bughunt-carrier-projectile-miss-qi-ledger-v1

## §0 摘要

**来源 Issue：#1670。** 暗器投射物脱靶时，发射阶段已从施术者扣出的 payload 在 `residual_qi_after_miss` 中被拆为 70% `qi_evaporated` 与 30% `residual_qi`；`projectile_miss_qi_release_system` 只把后者释放到 zone，前者在事件中没有任何消费端。脱靶/撞方块/超距/自然衰减因此凭空吞掉 70% 真元。本骨架只记录证据，不改生产代码。

接入面：进料是 `QiProjectile`、`AnqiProjectileFlight`、发射时 carrier account 和 `ProjectileDespawnedEvent`；出料应是实际落点 zone 或 overflow 的完整 payload 与 `QiTransfer`。复用 `carrier::release_residual_to_zone`/`needle.rs` 的全量释放模式、`QiAccountId` 和 `qi_release_to_zone`；agent narration 继续消费同一事件，wire schema 不变。依据 `docs/worldview.md` §二、§十，投射物衰减是效果变化，不是账本销毁。

## §1 游玩影响

- 玩家蓄力并掷出暗器后，只要没有命中目标，约 70% 的投入真元永久消失。
- 这是常规战斗路径，反复试射会持续减少玩家与世界总量；客户端只收到 despawn 叙事，无法看到缺失的回流。
- `needle` sibling 已全量释放，说明 carrier 路径是遗漏而不是有意的经济 sink。

## §2 复现路径

1. 完成 `CarrierCharging`，在发射阶段将 payload 转入 carrier/container account。
2. 令投射物撞方块、超出距离或自然衰减；`emit_projectile_despawn` 计算 `qi_at_despawn`。
3. `residual_qi_after_miss(qi_at_despawn)` 返回 `(0.7 * qi, 0.3 * qi)`，写入事件。
4. `projectile_miss_qi_release_system` 只读取 `event.residual_qi`，对 `event.qi_evaporated` 没有释放或 overflow 处理。

## §3 根因证据

- `server/src/combat/projectile.rs:95-98` 明确把 miss payload 分为 `(qi * 0.7, qi * 0.3)`。
- `server/src/combat/carrier.rs:1359-1374` 的 `emit_projectile_despawn` 将两部分分别写入 `ProjectileDespawnedEvent`；HitTarget 也单独将 residual 置零。
- `server/src/combat/carrier.rs:1382-1408` 的 `projectile_miss_qi_release_system` 读取并释放 `event.residual_qi`，没有读取 `qi_evaporated`；全仓 `qi_evaporated` 仅用于事件/桥接展示，没有真实 ledger consumer。
- 发射扣款在 `server/src/combat/carrier.rs:426-480` 的 `charge_carrier_tick`/`emit_carrier_channeling_transfer` 已把真元从玩家转入 carrier account，因此 despawn 必须结算完整 payload。
- 对照 `server/src/combat/needle.rs:204-220`：同样使用 `residual_qi_after_miss`，但在过期时把 `evaporated + residual` 全量送入 `release_needle_qi_to_zone`。

## §4 非重复比对

- `plan-bughunt-carrier-imprint-lifecycle-v1` 处理充能期间 instance 替换与 imprint 生命周期；本骨架只处理投射物 despawn 后 payload 的去向。
- `docs/finished_plans/plan-qi-conservation-leaks-v1.md` 的通用 ledger 原则和 needle 修复不自动覆盖 carrier system 的独立 EventReader。
- `plan-bughunt-dugu-v2-eclipse-qi-ledger-v1` 是战斗状态/脏真元问题，与暗器投射物容器无共享 producer。

## §5 修复计划骨架

### P0：脱靶完整结算 ✅ 2026-09-29

- 在 miss/OutOfRange/HitBlock/NaturalDecay 释放 `qi_evaporated + residual_qi` 的完整实际余额；carrier ledger 到外部 zone 用 `transfer_ledger_qi_to_zone`，外部 source 到 ledger 用 `transfer_external_qi_to_ledger`，只有纯 ledger 账户间才调用 `ledger.transfer(QiTransfer { from, to, amount, reason: QiTransferReason::ReleaseToZone })`，若 payload 已在命中效果中消费，则保持 HitTarget 的零 residual 语义并证明消费去向。
- 将实际 carrier account/source identity 传入释放 helper，落点优先使用投射物当前位置，zone 不可达时走 overflow；不可把“evaporated”当作系统外流。

### P1：回归契约 ✅ 2026-09-29

- 每种非命中 despawn reason 断言释放金额等于剩余 payload、zone/overflow 与 `QiTransfer` 完整；命中目标不重复释放。
- 保留 30/70 视觉效果计算断言，但把它与账本总量分开验证，防止修复反向改变 gameplay 伤害。

## §6 验证计划

实现后运行 server 栈 fmt、clippy、cargo test，覆盖 carrier/projectile 与 needle 对照测试。守恒断言使用 `QI_ZONE_UNIT_CAPACITY` 和 `assert_conservation`，本 skeleton 阶段不编译。

## §7 跨仓契约与可核验锚点

- **Inputs：** `residual_qi_after_miss`、`emit_projectile_despawn` 和 `projectile_miss_qi_release_system` 输入 `QiProjectile`、`AnqiProjectileFlight`、despawn reason、位置及 carrier source account。
- **Outputs：** OutOfRange/HitBlock/NaturalDecay 的完整剩余 payload 进入同维 zone 或 overflow；HitTarget 保持零 residual 且有已消费证据。
- **共享类型/事件：** `ProjectileDespawnedEvent`、`QiAccountId`、`QiTransfer`、`QiTransferReason::ReleaseToZone`、`ZoneRegistry`；server 符号为 `emit_projectile_despawn`、`projectile_miss_qi_release_system`、`release_residual_to_zone`、`release_account_to_zone`。
- **三端契约符号：** Server 负责 payload 结算和账本；Agent：无变更，理由是继续读取既有 `ProjectileDespawnedEvent`/narration；Client：无变更，理由是投射物 despawn VFX/事件 payload 不增字段。
- **Qi：** `qi_release_to_zone` 用 `QI_ZONE_UNIT_CAPACITY` 拆 zone/overflow；ECS/物品外部 source 先用 `transfer_external_qi_to_ledger`，真实 ledger source 对外部 zone 用 `transfer_ledger_qi_to_zone`，只有纯 ledger→ledger 才直接 `ledger.transfer(QiTransfer { from, to, amount, reason: QiTransferReason::ReleaseToZone })`。断言调用 `qi_physics::ledger::assert_conservation`、`QI_EPSILON` 与 `crate::schema::common::SPIRIT_QI_TOTAL`。
- **worldview 锚点：** `docs/worldview.md` §二、§十的投射物衰减不销毁真元和总量守恒。

## Finish Evidence

- **验真结论：** 真 bug。复现测试先证明旧实现只为 `residual_qi=5.0` 发出转移，遗漏同一事件中的 `qi_evaporated=11.666...`；修复后 `projectile_miss_qi_release_system` 对 `HitBlock`、`OutOfRange`、`NaturalDecay` 将两部分合计，从 `carrier_qi_account(owner, instance_id)` 经 `qi_release_to_zone` 回流，zone 无空间时进入 overflow；`HitTarget` 仍不重复释放。
- **落地清单：** P0 落在 `server/src/combat/carrier.rs:1407-1420` 的 `projectile_miss_qi_release_system` 与 `carrier_qi_account_for_projectile`；P1 落在 `server/src/combat/carrier_tests.rs` 的 carrier 账户归零、完整 payload、满 zone overflow、命中不回流与端到端守恒契约测试。
- **关键 commit：** `b5e8ed9f9`（2026-09-29，提升本 plan 为 Active）；`51d5d1fc2`（2026-09-29，脱靶完整真元回流及回归测试）。
- **测试结果：** 修改前复现 `cargo test -p bong-server combat::carrier::tests::conservation_invariant_releases_full_miss_payload --lib -- --exact --nocapture` 失败（实际 5、期望 16.666...）；修复后 carrier 回归组 36 passed，`cargo fmt --check` 通过；端到端测试用 `SPIRIT_QI_TOTAL` 建预算并以 `qi_physics::ledger::assert_conservation`（server 符号为 `crate::qi_physics::ledger::assert_conservation`）断言。
- **跨仓核验：** server 命中 `ProjectileDespawnedEvent`、`carrier_qi_account_for_projectile`、`projectile_miss_qi_release_system`、`qi_release_to_zone`、`QiTransfer`、`SPIRIT_QI_TOTAL` 与 `assert_conservation`；agent 继续消费既有 despawn/narration 事件，client 继续消费既有投射物消失表现，wire schema 无变更。
- **遗留 / 后续：** 本 plan 未改 agent、client、灵田、经脉、功法或身体部位；投射物到达脱靶点前的既有距离衰减语义不在本次范围。
