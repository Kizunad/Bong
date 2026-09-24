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
- `server/src/dandao/tests.rs`（`mod boss_spawn_integration`，PR #1296 回归修复）：`boss_spawn::register` 端到端集成测试早于 `ZoneRegistry` 接入就已存在，上一轮修复未同步给它插入 `ZoneRegistry` 资源，导致 `baolongwang_qi_drain_aura_system` 每 tick 在 `find_zone_mut` 处早退，`boss_spawn_register_drain_system_runs_end_to_end` 断言玩家真元减少撞红。补一份与 `boss_spawn.rs` 同款 `baolongwang_zone_fixture`，给两条集成测试都插入含 `BOSS_HOME_ZONE` 的 `ZoneRegistry`；顺带把 `boss_spawn_register_expel_phase_no_drain` 也接上真实 zone，让它锁住"Expel 阶段跳过吸取"分支本身，而非凑巧靠 zone 缺失早退得出同样结果。
- `server/src/dandao/boss_spawn.rs`（`mod boss_spawn_tests`，PR #1296 加固）：新增 `boss_home_zone_exists_in_production_zones_json` pin 测试，直接用 `ZoneRegistry::load()` 读生产 `server/zones.json` 断言 `BOSS_HOME_ZONE` 存在——`find_zone_mut` 未命中时是静默早退（无报错），一旦有人改名/删掉这个 zone，BossDrain 会在无任何测试撞红的情况下失效，这条测试把该配置漂移钉在测试期。
- `server/src/dandao/boss_spawn.rs`（PR #1296 `/review` blocker 修复，2026-07-27）：`/review` 复投 4 位 reviewer 全票 blocker，指出生产 `baolongwang_cavern_deep` 当前 `spirit_qi=-0.729232`（负灵域，约 -36.46 绝对真元点债务）时，实现先把该负值 `.max(0.0)` 归零参与 `qi_release_to_zone` 计算，再用非负 `outcome.zone_after` 直接覆盖 `zone.spirit_qi` 字段权威——单次约 0.35 的小额吸取即可把字段从 -0.729232 跳变为约 +0.007，凭空抹除整笔负灵域债务。修复：保留原始有符号字段值到局部变量 `zone_spirit_qi_before`，`.max(0.0)` 仅继续用作 `WorldQiAccount::set_balance` 非负校验所需的 ledger 输入基线，字段写回改为 `zone_spirit_qi_before + outcome.accepted / QI_ZONE_UNIT_CAPACITY`（在原始有符号值上累加本次实际吸取量，而不是用 floor 过的镜像值覆盖）——债务未被填平前继续为负、只减少 accepted 那部分，填平后自然转正，全程不凭空增减；`zone_spirit_qi_before >= 0` 时与旧写法代数完全等价（`outcome.zone_after / CAP == zone_spirit_qi_before + outcome.accepted / CAP`），非负路径无行为变化。复用既有 `qi_physics::release::qi_release_to_zone` 的 `accepted` 计算，未新增衰减/入账公式，未改动 `qi_physics` 目录任何文件。新增两条回归：`qi_drain_aura_preserves_negative_zone_debt_when_debt_not_fully_repaid`（生产真实值 -0.729232 起步，单 tick 吸取量远小于债务，断言字段仍为负且债务减少量精确等于玩家减少量）、`qi_drain_aura_crosses_zero_exactly_when_drain_exceeds_remaining_debt`（极小负债 -0.001，单 tick 吸取量足以填平并跨零转正，断言跨零后增量同样精确对齐玩家减少量）。

### 关键 commit

- `c79bc03eb` — 2026-07-27 — 修复暴龙王 BossDrain 只落 ledger 镜像不写 `ZoneRegistry.spirit_qi` 的吞真元漏洞（含全部代码修复 + 5 个新增/改造回归测试）。
- `869ade73a` — 2026-07-27 — PR #1296：修复 `dandao::tests::boss_spawn_integration` 两条 App 级集成测试未插入 `ZoneRegistry` 导致的回归（`c79bc03eb` 引入）。
- `e3bfd907e` — 2026-07-27 — PR #1296：加固，新增 `boss_home_zone_exists_in_production_zones_json` pin 测试锁死生产 `zones.json` 必须注册 `BOSS_HOME_ZONE`。
- `064c1ca04` — 2026-07-27 — PR #1296：修复 `/review` blocker——负灵域字段权威被 floor 过的 ledger 镜像凭空覆盖抹除债务；新增 2 条负灵域回归（债务未填平 / 跨零转正）。

### 测试结果

- 判据自检：把 `zone.spirit_qi = (outcome.zone_after / QI_ZONE_UNIT_CAPACITY).clamp(-1.0, 1.0);` 临时注释掉后，`cargo test --lib dandao::boss_spawn::` 撞红 2 个（`qi_drain_aura_is_conserved_using_spirit_qi_total_const`、`qi_drain_survives_heartbeat_zone_inflow_resync`），恢复后全绿——确认测试真正锁住了行为，不是空转。
- `cargo test --lib dandao::boss_spawn::` → `22 passed; 0 failed`（定向；含 PR #1296 新增的 pin 测试，较归档时的 21 passed 多 1）。
- `cargo test --lib dandao::tests::boss_spawn_integration` → `2 passed; 0 failed`（PR #1296 修复的回归用例，此前撞红的 `boss_spawn_register_drain_system_runs_end_to_end` 现已转绿）。
- `cargo test --lib qi_physics::release::` → `7 passed; 0 failed`；`cargo test --lib qi_physics::zone_inflow::` → `16 passed; 0 failed`；`cargo test --lib world::heartbeat` → `46 passed; 0 failed`（交叉验证复用的既有路径与覆盖链系统未受影响）。
- `cargo fmt --check` → 干净（0 diff）；`cargo clippy --all-targets -- -D warnings` → 干净（`Finished`，无 warning）。
- pin 测试实测：临时把 `server/zones.json` 里 `baolongwang_cavern_deep` 改名后 `boss_home_zone_exists_in_production_zones_json` 立刻撞红，改回后转绿，确认测试真能钉住配置漂移（非空转）。
- `agent/` 侧 `network::tests::narration_tests::realm_gate_producer_consumer_selector_routes_only_target_player`：新建 worktree 环境缺口（缺 `agent/node_modules/.bin/tsx` 与 `@bong/schema` 构建产物），`npm ci` + `npm run build -w @bong/schema` 后转绿，与本轮 server 代码改动无关。
- 未跑全量 `cargo test`（按任务约束，交后续全量门禁验证）。
- 负灵域修复（`064c1ca04`）判据自检：把字段写回临时退回旧写法 `zone.spirit_qi = (outcome.zone_after / QI_ZONE_UNIT_CAPACITY).clamp(-1.0, 1.0);` 后，`qi_drain_aura_preserves_negative_zone_debt_when_debt_not_fully_repaid` 与 `qi_drain_aura_crosses_zero_exactly_when_drain_exceeds_remaining_debt` 均撞红（字段从 -0.729232 跳变为 +0.007031250 / 跨零增量差值 0.05），恢复后 `cargo test --lib dandao::boss_spawn::` → `24 passed; 0 failed`，`cargo test --lib dandao::tests::boss_spawn_integration::` → `2 passed; 0 failed`；`cargo fmt --check` 干净、`cargo clippy --all-targets -- -D warnings` 干净。

### 对抗验证

- 无上下文 read-only validator（`general-purpose` agent）对 HEAD `c79bc03eb187fe4a255950b2f25543b0070e6022` 独立复核：**PASS**。覆盖：① 判据自检复现（撞红 2 个测试，恢复后全绿）② 原子性逐行走查（`set_balance` 失败路径先于任何 mutation）③ 确认 `qi_drain_survives_heartbeat_zone_inflow_resync` 真实通过 `app.add_systems` 注册 `zone_qi_inflow_tick` 并靠 `app.update()` 触发，非手塞状态 ④ `git diff origin/main HEAD -- server/src/qi_physics/` 为空，未新增衰减/漂移类常数，写回口径与 `combat/carrier.rs:1382` / `npc/npc_skill.rs:173` / `npc/dormant/mod.rs:2045` 既有范式字节级一致 ⑤ 确认 `ZoneRegistry` 在生产 `Startup` 真实注册（非仅测试可达）。
- PR #1296（`/review` 复投 blocker 修复）：无上下文 read-only validator（`general-purpose` agent）对 HEAD `064c1ca044547b60e726896813080cd5b37a87fa` 独立复核：**PASS**。第一步核对 `git rev-parse HEAD` 与目标 SHA 一致后再判定。覆盖：① 独立把字段写回临时改回旧写法并实测两条新回归均 FAILED（`zone_spirit_qi_after=0.007031250` / 差值 0.05），验证后 `git checkout --` 恢复并核对 `git diff`/`git status` 为空、HEAD SHA 未变 ② `git show <SHA> -- server/src/qi_physics/` 为空，独立代数验算 `zone_spirit_qi_before>=0` 时新旧写法完全等价，确认未自造公式 ③ `git show <SHA> --stat` 仅 1 个文件、`npc/dormant/mod.rs` 等其余同款 `.max(0.0)` 站点未被触碰 ④ 独立重跑 `dandao::boss_spawn::`（24 passed）与 `dandao::tests::boss_spawn_integration::`（2 passed）确认既有路径无回归 ⑤ 指出归档 Finish Evidence 在验证时仍是修复前的旧版本（本次已更新，不影响其 PASS 判定）。

### 跨仓库核验

- 仅 `server` 栈改动（Rust ECS system + 单元测试）；`agent` / `client` 无接线面（BossDrain 是纯 server 侧真元流转 bug，无 IPC schema / CustomPayload 变更），不适用跨仓库 symbol 核验。PR #1296 的两条 tsx/schema 环境修复动作（`npm ci` / `npm run build -w @bong/schema`）不产生任何 tracked diff，纯本地环境补全。

### 遗留 / 后续

- ~~`zone.spirit_qi` 为负值时字段权威被 floor 覆盖抹除债务~~ —— **已于 PR #1296 `064c1ca04` 修复**（见上方「落地清单」「关键 commit」）：`.max(0.0)` 现在仅作为 `qi_release_to_zone` 调用与 `WorldQiAccount::set_balance` 的非负输入基线，字段权威写回改为在原始有符号值上累加本次实际吸取量，不再用 floor 过的镜像值覆盖。
- **`.max(0.0)` 同款镜像口径的其余站点未逐一审计**：本轮 grep 确认仓库里至少还有 `npc/dormant/mod.rs`、`combat/carrier.rs`、`combat/needle.rs`、`combat/woliu_v2/skills.rs`、`combat/tuike_v2/skills.rs`、`cultivation/burst_meridian.rs`、`cultivation/void/actions.rs` / `ledger_hooks.rs`、`zhenfa/mod.rs`、`world/pseudo_vein_runtime.rs`、`world/heartbeat.rs`、`npc/lod.rs`、`fauna/rat_phase.rs` / `mimic_spider.rs`、`persistence/mod.rs` 等站点使用同一 `zone.spirit_qi.max(0.0) * QI_ZONE_UNIT_CAPACITY` 模式；哪些站点之后还会把 floor 过的结果**反写**回 `zone.spirit_qi`（即具备本次同型"凭空抹除负债"风险）未逐一核实——按"一个 PR 只动一个 plan"原则本轮不顺手改，留待独立 plan/PR 统一核查处理。
- 本 plan 只修暴龙王 `BossDrain`；`docs/plans-skeleton/plan-bughunt-skull-fiend-drain-zone-shadow.md` 中同型 `SkullFiendDrain`（骨煞）仍待独立 plan/PR 处理，未在本次改动内。
- PR #1296 未给 `baolongwang_qi_drain_aura_system` 的 zone 缺失早退分支加 `tracing::warn!`：该 system 挂在 `Update`，每 tick 都跑，若真触发早退会变成无限刷日志（与仓库里 `forge/mod.rs:596-599` / `client_request_handler.rs:18504` 那种事件驱动、单次触发的 `warn!` 场景不同）；且新增的 pin 测试已经把这条早退路径的现实触发条件（zone 改名/删除）钉在测试期，运行时再加日志对已经在生产失效的场景边际诊断价值有限。判断为不加。
