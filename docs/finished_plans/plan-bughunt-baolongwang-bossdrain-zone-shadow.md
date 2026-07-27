# plan-bughunt-baolongwang-bossdrain-zone-shadow

> BugHunt worker：server-qi r10
> 主题：暴龙王 BossDrain 只落 WorldQiAccount zone 镜像，不写回 ZoneRegistry.spirit_qi，导致玩家被抽走的真元在真实环境层不可见，并可能被后续 field-authority 重同步抹掉。

## 结论

`server/src/dandao/boss_spawn.rs:337-411` 的 `baolongwang_qi_drain_aura_system` 在暴怒/崩溃阶段会按距离持续扣玩家 `Cultivation.qi_current`，然后把同等 `actual_drain` 加到 `WorldQiAccount` 的 `zone:baolongwang_cavern_deep` 账户并 `push_transfer_audit(QiTransferReason::BossDrain)`。

问题是该系统没有 `ResMut<ZoneRegistry>`，也没有同步 `ZoneRegistry.spirit_qi`。而仓库当前的 zone qi 权威范式是 field-authority：`zone:<name>` ledger balance 是 `ZoneRegistry.spirit_qi * QI_ZONE_UNIT_CAPACITY` 的镜像，不是长期权威余额。`server/src/world/heartbeat.rs:2288-2292` 的 `zone_qi_inflow_tick` 会用 `zone.spirit_qi.max(0.0) * QI_ZONE_UNIT_CAPACITY` 覆写同名 zone ledger balance；`server/src/world/zone.rs:321` 也明确说明 BossDrain 这类 qi 入账场景需要直接修改 `zone.spirit_qi`。

因此当前实现只让测试看到“玩家减少量 == ledger zone 增加量”，但真实游玩层读到的环境灵压没有变化；一旦后续系统按字段权威重同步，BossDrain 刚加进 zone 镜像账的增量还会被陈旧 `zone.spirit_qi` 覆盖。

## 实际游玩体验影响

玩家在暴龙王光环范围内会稳定掉真元，战斗压力是真实的；但这些被抽走的真元不会稳定回到暴龙王巢穴环境。玩家看到的是“自己被吸干了”，可区域灵压、负灵域强度、后续依赖 `ZoneRegistry.spirit_qi` 的感知/风险/回血/生态判断不会按这笔吸取变化，长期 Boss 战等同于把玩家真元蒸发掉，破坏 `docs/worldview.md §二 L30-L46` 灵压环境与 `docs/worldview.md §十 L870-L879`“灵气零和、修炼消耗就是别人少掉”的守恒体验。

## 证据

- `server/src/dandao/boss_spawn.rs:382-408`：先 `cultivation.qi_current -= actual_drain`，再 `account.set_balance(zone_id, zone_balance + actual_drain)`；没有读取或写入 `ZoneRegistry`。
- `server/src/dandao/boss_spawn.rs:930-998`：现有回归只断言玩家减少量等于 `WorldQiAccount` zone 账户增加量，并检查 `BossDrain` 审计记录；没有断言 `ZoneRegistry.spirit_qi` 同步，也没有推进 heartbeat 重同步链。
- `server/src/qi_physics/ledger.rs:463-467`：`push_transfer_audit` 的语义是“余额已在外部正确更新，此处仅留轨迹”；BossDrain 不能把“只改 ledger 镜像”当成最终入账。
- `server/src/world/heartbeat.rs:2288-2292`：回流系统会把 zone ledger 镜像重设为 `zone.spirit_qi.max(0.0) * QI_ZONE_UNIT_CAPACITY`。
- `server/src/world/zone.rs:321`：注释点名 BossDrain 后续等 qi 入账场景需要直接修改 `zone.spirit_qi`。
- `docs/plans-skeleton/plan-bughunt-skull-fiend-drain-zone-shadow.md:44-53`：骨煞 plan 已把 BossDrain 列为同类旧范式审计对象，但明确“不在本 plan 范围内混修”。

## 去重说明

不是 #1050 craft qi_cost 固定落 `zone:spawn`；不是 #1056 NPC 日程/休息/QiSpring/Far LOD 凭空恢复真元；不是 #1076 灵田 `plot_qi`；不是 #1082 灵蝗潮推进扣 zone qi；不是 #1089 垂死大能 rift drain；不是 #1096 气针过期抹掉负灵域缺口。

最接近的是 #1046“骨煞抽真元落入 zone 镜像账”。本发现与 #1046 是同型 field-authority bug，但对象不同：#1046 修 `SkullFiendDrain`，这里是丹道暴龙王 `BossDrain`。骨煞 skeleton 还把 BossDrain 列为后续拆分审计对象，因此本 plan 是未覆盖的独立候选。

## 修复要求

- [x] `baolongwang_qi_drain_aura_system` 接入 `ResMut<ZoneRegistry>`，按 `BOSS_HOME_ZONE` 找到真实 zone，并将 `actual_drain / QI_ZONE_UNIT_CAPACITY` 写回 `zone.spirit_qi`，与 ledger zone 镜像保持一致。
- [x] 保留玩家真元在 ECS `Cultivation.qi_current`、审计用 `push_transfer_audit` 的既有语义；不要把玩家余额镜像进 `WorldQiAccount` 后调用 `transfer()`。
- [x] 处理 zone 缺失或 ledger 缺失时的原子性：不能先扣玩家再入账失败；失败时应跳过本 tick 吸取或保证玩家扣减与环境入账同成同败。
- [x] 补回归测试：暴龙王光环 tick 后断言玩家减少量、zone ledger 增量、`ZoneRegistry.spirit_qi` 增量三者按 `QI_ZONE_UNIT_CAPACITY` 对齐。
- [x] 补 heartbeat 覆盖链回归：BossDrain 后推进一次会触发 zone 镜像重同步的系统，断言刚入账的真元不会被陈旧 `zone.spirit_qi` 抹掉。

## 对抗复核

- 第 1 轮 Socrates：判定成立；指出 BossDrain 只写 `WorldQiAccount zone:<BOSS_HOME_ZONE>`，没有写字段，和 #1043/#1046 骨煞是同型但不同对象。
- 第 2 轮 Beauvoir：判定不允许；强调 `zone:<name>` 是 `ZoneRegistry.spirit_qi` 镜像，不是长期权威余额，现有测试只放过了 ledger 增量，未覆盖字段权威链。

## Finish Evidence

### 落地清单

- `server/src/dandao/boss_spawn.rs`：`baolongwang_qi_drain_aura_system` 新增 `Option<ResMut<ZoneRegistry>>` 参数；zone/账户缺失时整 tick 早退（原子性）；复用既有 `qi_physics::release::qi_release_to_zone`（不新写公式）算出本次可入账量，先提交唯一可能失败的一步（`WorldQiAccount::set_balance`），成功后才一次性提交玩家扣减 `Cultivation.qi_current` 与 `ZoneRegistry.zones[].spirit_qi` 字段写入；审计记录仍手工构造 `QiTransfer{reason: QiTransferReason::BossDrain}` 并走 `push_transfer_audit`（未改用 `qi_release_to_zone` 内部产出的 `ReleaseToZone` transfer，未调用 `WorldQiAccount::transfer()`）。
- `server/src/dandao/boss_spawn.rs`（`mod boss_spawn_tests`）：
  - 新增 fixture `baolongwang_zone_fixture(spirit_qi)` 构造 `BOSS_HOME_ZONE` 的 `Zone`。
  - 改造 `qi_drain_aura_is_conserved_using_spirit_qi_total_const`：插入 `ZoneRegistry`，新增「玩家减少量 == zone ledger 增量 == `ZoneRegistry.spirit_qi` 增量换算绝对值」三方对齐断言（修复要求 #4）。
  - 改造 `qi_drain_zero_for_expel_phase`：插入 `ZoneRegistry`（避免被新增的资源早退分支掩盖 phase_gate 断言），新增 `spirit_qi` 不变断言。
  - 新增 `qi_drain_skips_tick_atomically_when_zone_registry_resource_missing`：`ZoneRegistry` 资源整体缺失时玩家 qi / zone ledger / 审计记录均不变（修复要求 #3）。
  - 新增 `qi_drain_skips_tick_atomically_when_boss_home_zone_missing_from_registry`：`ZoneRegistry` 存在但未注册 `BOSS_HOME_ZONE` 时同样整 tick 跳过（修复要求 #3）。
  - 新增 `qi_drain_survives_heartbeat_zone_inflow_resync`：真实注册 `baolongwang_qi_drain_aura_system` + `heartbeat::zone_qi_inflow_tick` 两个 system 进同一 `App`，推进 `CultivationClock` 触发字段权威重同步，断言 BossDrain 入账不被陈旧 `zone.spirit_qi` 抹掉（修复要求 #5）。

### 关键 commit

- `c79bc03eb` — 2026-07-27 — 修复暴龙王 BossDrain 只落 ledger 镜像不写 `ZoneRegistry.spirit_qi` 的吞真元漏洞（含全部代码修复 + 5 个新增/改造回归测试）。

### 测试结果

- 判据自检：把 `zone.spirit_qi = (outcome.zone_after / QI_ZONE_UNIT_CAPACITY).clamp(-1.0, 1.0);` 临时注释掉后，`cargo test --lib dandao::boss_spawn::` 撞红 2 个（`qi_drain_aura_is_conserved_using_spirit_qi_total_const`、`qi_drain_survives_heartbeat_zone_inflow_resync`），恢复后 21 passed / 0 failed——确认测试真正锁住了行为，不是空转。
- `cargo test --lib dandao::boss_spawn::` → `21 passed; 0 failed`（定向）。
- `cargo test --lib qi_physics::release::` → `7 passed; 0 failed`；`cargo test --lib qi_physics::zone_inflow::` → `16 passed; 0 failed`；`cargo test --lib world::heartbeat` → `46 passed; 0 failed`（交叉验证复用的既有路径与覆盖链系统未受影响）。
- `cargo fmt --check` → 干净（0 diff）。
- `cargo clippy --all-targets -- -D warnings` → 干净（`Finished` + `CLIPPY_RC=0`，无 warning）。
- 未跑全量 `cargo test`（按任务约束，交后续全量门禁验证）。

### 对抗验证

- 无上下文 read-only validator（`general-purpose` agent）对 HEAD `c79bc03eb187fe4a255950b2f25543b0070e6022` 独立复核：**PASS**。覆盖：① 判据自检复现（撞红 2 个测试，恢复后全绿）② 原子性逐行走查（`set_balance` 失败路径先于任何 mutation）③ 确认 `qi_drain_survives_heartbeat_zone_inflow_resync` 真实通过 `app.add_systems` 注册 `zone_qi_inflow_tick` 并靠 `app.update()` 触发，非手塞状态 ④ `git diff origin/main HEAD -- server/src/qi_physics/` 为空，未新增衰减/漂移类常数，写回口径与 `combat/carrier.rs:1382` / `npc/npc_skill.rs:173` / `npc/dormant/mod.rs:2045` 既有范式字节级一致 ⑤ 确认 `ZoneRegistry` 在生产 `Startup` 真实注册（非仅测试可达）。

### 跨仓库核验

- 仅 `server` 栈改动（Rust ECS system + 单元测试）；`agent` / `client` 无接线面（BossDrain 是纯 server 侧真元流转 bug，无 IPC schema / CustomPayload 变更），不适用跨仓库 symbol 核验。

### 遗留 / 后续

- `zone.spirit_qi` 为负值（当前 `baolongwang_cavern_deep` 实际配置为 `-0.729232`，负灵域）时，`qi_release_to_zone` 的 `zone_current` 入参沿用仓库既有 `.max(0.0)` 镜像口径（与 `release_dormant_qi_to_zone` / `zone_qi_inflow_tick` 同款），即从 0 开始重新累积，不会精确保留负值债务的连续性——这是仓库里"负灵域 ↔ ledger 非负余额"这条更深的既有架构特性（`WorldQiAccount::set_balance` 硬性拒绝负数），不属于本 plan 范围，未在此修复。
- 本 plan 只修暴龙王 `BossDrain`；`docs/plans-skeleton/plan-bughunt-skull-fiend-drain-zone-shadow.md` 中同型 `SkullFiendDrain`（骨煞）仍待独立 plan/PR 处理，未在本次改动内。
