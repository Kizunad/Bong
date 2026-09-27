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

### P0：脱靶完整结算

- 在 miss/OutOfRange/HitBlock/NaturalDecay 释放 `qi_evaporated + residual_qi` 的完整实际余额；对 carrier/zone/overflow 账户调用 `ledger.transfer(QiTransfer { from, to, amount, reason: QiTransferReason::ReleaseToZone })`，若 payload 已在命中效果中消费，则保持 HitTarget 的零 residual 语义并证明消费去向。
- 将实际 carrier account/source identity 传入释放 helper，落点优先使用投射物当前位置，zone 不可达时走 overflow；不可把“evaporated”当作系统外流。

### P1：回归契约

- 每种非命中 despawn reason 断言释放金额等于剩余 payload、zone/overflow 与 `QiTransfer` 完整；命中目标不重复释放。
- 保留 30/70 视觉效果计算断言，但把它与账本总量分开验证，防止修复反向改变 gameplay 伤害。

## §6 验证计划

实现后运行 server 栈 fmt、clippy、cargo test，覆盖 carrier/projectile 与 needle 对照测试。守恒断言使用 `QI_ZONE_UNIT_CAPACITY` 和 `assert_conservation`，本 skeleton 阶段不编译。

## §7 跨仓契约与可核验锚点

- **Server：** 脱靶链是 `projectile::residual_qi_after_miss` → `carrier::emit_projectile_despawn` → `carrier::projectile_miss_qi_release_system` → `release_residual_to_zone`/`release_account_to_zone`。测试按 `ProjectileDespawnedEvent.reason` 覆盖 OutOfRange、HitBlock、NaturalDecay 与 HitTarget，确认完整 payload 只结算一次。
- **Qi：** `qi_release_to_zone` 负责 `QI_ZONE_UNIT_CAPACITY` 下的 zone/overflow 拆分；当 carrier/container 与 zone/overflow 是真实 `WorldQiAccount` 账户时，必须提交 `ledger.transfer(QiTransfer { from, to, amount, reason: QiTransferReason::ReleaseToZone })`，而不是只 `EventWriter<QiTransfer>::send`。来源账户用 `carrier_qi_account`/`QiAccountId::container`，金额校验用 `QI_EPSILON`；守恒测试引用 `DEFAULT_SPIRIT_QI_TOTAL`（以及仍由 `schema::common` 提供的 `SPIRIT_QI_TOTAL` fixture）和 `assert_conservation`。
- **Agent：无变更。** 证据是 agent 继续读取既有 `ProjectileDespawnedEvent`/narration 语义，未改 Redis 或 schema。
- **Client：无变更。** 证据是投射物 despawn 的现有 VFX/事件 payload 不变，修复只补完整真元结算。
