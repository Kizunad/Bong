# plan-bughunt-combat-qi-max-shrink-ledger-v1

## §0 摘要

**来源 Issue：#1921、#1388、#1378。** 战斗路径收缩 `Cultivation.qi_max` 后，直接用 `min`/`clamp` 覆盖超出新上限的 `qi_current`，没有把差额通过 `qi_physics::ledger::QiTransfer` 释放到 zone 或 overflow。三处分别位于撼山散功、涡流超时反噬和道蛊永久蚀印维护 tick；它们是同一守恒根因的三个 producer。本 skeleton 不改生产代码。

接入面：进料是 `Cultivation`、经脉状态、`TaintMark`/`VortexField` 以及当前 zone；出料必须是新的 `qi_current`、zone/overflow 余额和可审计 `QiTransfer`，不能静默减少全服总量。共享 `QiMaxShrinkReleaseContext`（若实现阶段复用已有 helper）、`ZoneRegistry`、`CurrentDimension` 和 ledger reason；没有 agent/client wire 变更。依据 `docs/worldview.md` §二、§十的零和真元约束，所有缩容差额都必须有归宿。

## §1 游玩影响

- 玩家在使用散功/体魄透支、让涡流维护超时或被永久蚀印持续侵蚀时，接近满真元的那部分会突然消失。
- UI 只显示上限降低或招式结束，玩家看不到释放、zone 回灌或 overflow 审计；重复 tick 会长期抽干世界灵气。
- 三条路径挂在正常战斗 Update 流程，普通玩家即可触发，不依赖 dev-only 命令。

## §2 复现路径

1. 令玩家 `qi_current` 高于即将得到的新 `qi_max`。
2. 触发 `BaomaiSkillId::Disperse`，`WoliuField` 超过 `maintain_max_ticks`，或给目标施加 permanent `TaintMark` 后推进维护 tick。
3. 代码先计算新的 `qi_max`，再 `qi_current.min(new_max)`/`clamp`。
4. 检查 zone 与 `QiTransfer` 事件：当前无对应的差额释放。

## §3 根因证据

- `server/src/combat/baomai_v3/skills.rs:776-785` 的 `apply_qi_max_loss` 直接写 `qi_max`，随后 `qi_current = qi_current.clamp(0.0, qi_max)`；`cast_disperse` 在约 `server/src/combat/baomai_v3/skills.rs:569` 调用它。没有位置、维度、zone 或 ledger 参数。
- `server/src/combat/woliu.rs:658-667` 的 `vortex_maintain_tick` 在超时且未抵抗时 `sever_meridian`、重算 `qi_max`，随后 clamp `qi_current`；同一函数的维护消耗路径虽有 `ReleaseToZone`，反噬造成的缩容差额却没有释放。
- `server/src/combat/dugu_v2/tick.rs:63-79` 的 `permanent_qi_max_decay_tick` 每 tick 降低 `qi_max`，再用 `min` 截断 `qi_current`，只发送 `PermanentQiMaxDecayApplied` 观察事件；该事件不是 ledger consumer。
- 现有 `docs/finished_plans/plan-bughunt-qimax-shrink-clamp-leak-v1.md` 只覆盖延寿丹与断续散文件，不能证明这三条 combat producer 已修复；其正确先例是 `race_change`/`death_hooks` 先释放差额再提交缩容。

## §4 非重复比对

- 延寿丹/断续散的缩容缺口已在 `docs/finished_plans/plan-bughunt-qimax-shrink-clamp-leak-v1.md` 归档，但文件、调用方和触发状态均不同，本骨架不重复修改它们。
- `dugu_v2/skills.rs` 的 Eclipse/穿透命中扣减是一次性 `qi_loss` 与 taint intensity 的记账问题，归 `plan-bughunt-dugu-v2-eclipse-qi-ledger-v1`；本骨架只负责 `qi_max` 收缩。
- `docs/finished_plans/plan-qi-physics-v1.md` 提供 ledger 底盘，不拥有这些 combat 接线；不能以已有 helper 存在当作 producer 已守恒。

## §5 修复计划骨架

### P0：统一缩容释放事务

- 把三处缩容改为先计算 `excess = (qi_current - new_qi_max).max(0.0)`，用 `qi_release_to_zone` 拆分 zone/overflow；对 ledger 账户提交 `ledger.transfer(QiTransfer { from, to, amount, reason: QiTransferReason::ReleaseToZone })`，只有完整提交后才写入新 `qi_max`/`qi_current`。
- 区分真实余额变更与 `PermanentQiMaxDecayApplied` 等审计事件，禁止新增全局 event-only consumer。

### P1：回归契约

- 每个 producer 覆盖“当前值低于新上限”和“当前值超过新上限”两类；后者断言 zone/overflow 增量恰等于 excess、`QiTransfer` reason 可追踪且总量守恒。
- 缺位置/zone/ledger 等资源时 fail closed 或进入已有 overflow，不得静默截断；抵抗反噬和非永久 mark 的分支保持原行为。

## §6 验证计划

实现后运行 server 栈完整 fmt、clippy、cargo test，并覆盖 `baomai_v3`、`woliu`、`dugu_v2::tick` 相关测试。守恒断言引用 qi_physics 常量与 `assert_conservation`，不写 `100.0` 等总量字面量。本 skeleton 阶段不编译。

## §7 跨仓契约与可核验锚点

- **Inputs：** `cast_disperse`/`apply_qi_max_loss`、`vortex_maintain_tick`、`permanent_qi_max_decay_tick` 输入 `Cultivation.qi_current/qi_max`、经脉/mark/field、位置维度和 zone。
- **Outputs：** `excess` 只进入同维 zone 或 overflow，随后提交新的 `qi_max/qi_current`；`PermanentQiMaxDecayApplied` 仅作观察事件。
- **共享类型/事件：** `Cultivation`、`TaintMark`、`VortexField`、`ZoneRegistry`、`WorldQiAccount`、`QiTransfer`、`QiTransferReason::ReleaseToZone`；server 符号为上述三个 producer。
- **三端契约符号：** Server 负责 `qi_release_to_zone` 与余额提交；Agent：无变更，理由是组件/zone/ledger 均为 server 内部；Client：无变更，理由是现有战斗事件/HUD wire 不增字段。
- **Qi：** ECS 玩家来源不得直接 `ledger.transfer`；用 `qi_release_to_zone` 更新 ZoneRegistry 并留下 `ReleaseToZone` 审计，真实 ledger 来源/去向才调用 `ledger.transfer(QiTransfer { from, to, amount, reason: QiTransferReason::ReleaseToZone })`。守恒断言用 `qi_physics::ledger::assert_conservation`、`QI_ZONE_UNIT_CAPACITY`、`QI_EPSILON` 与 `crate::schema::common::SPIRIT_QI_TOTAL`。
- **worldview 锚点：** `docs/worldview.md` §二、§十的真元零和；缩容差额不能因 clamp 消失。
