# BugHunt: 渡劫「逃劫」（Fled）结局未收掉 tribulation_brace 全身循环动画，玩家永久卡抱臂姿势

## Bug 摘要

严重度：medium

`server/src/cultivation/tribulation.rs::settle_fled_tribulation`（渡劫「逃劫」结局，玩家跑出天劫锁定半径或切换维度触发）会发出 `TribulationSettled{outcome: Fled}` 和 `TribulationFled` 两个事件，但两个下游动画消费系统——`emit_tribulation_animation_triggers`（只订阅 `TribulationAnnounce`/`TribulationFailed`）和 `emit_tribulation_settled_vfx_triggers`（只匹配 `DuXuOutcomeV1::Ascended`/`HalfStep`，其余走 `_ => continue`）——都不消费 `TribulationFled`，也不为 Fled 结局发任何 `StopAnim`。

结果：由 `TribulationAnnounce` 起播的 `FULL_BODY`、`isLoop:true` 的 `bong:tribulation_brace`（抱臂承劫）循环动画，在玩家逃劫后永远收不到 `StopAnim`，玩家角色永久卡在抱臂姿势，直到被另一条同/更高优先级动画覆盖同通道或客户端重连。

这与 #1255（`628f644a8`，已合入 origin/main）刚修复的「`TribulationFailed` 存活结局不清 `FULL_BODY` brace 循环」是**完全同构**的 bug——`DuXuOutcomeV1` 的 5 个变体里，`Failed` 已被 #1255 修复，`Ascended`/`HalfStep` 靠同优先级 breakthrough 动画覆盖 brace（OK），`Killed` 走真实死亡生命周期（死亡动画顺带清通道，不在本 plan scope），只有 `Fled` 被漏掉——它跟 `Failed` 一样是「存活结局，不进入死亡生命周期，没有死亡动画顺带清通道」，但走的是完全独立的 `TribulationFled` 事件，#1255 的修复没有覆盖它。

## 实际游玩体验影响

任何试图六境突破（醒灵→引气→凝脉→固元→通灵→化虚）却选择/被迫逃跑的玩家都会踩中：渡劫开始后角色播放抱臂承劫的全身循环动画；玩家跑出天劫锁定半径（`state.lock_radius(clock.tick)`，随渡劫阶段变化，是设计好的「逃劫」机制）或切换维度逃劫，`tribulation_escape_boundary_system` 判定 Fled、扣真元惩罚、经脉走永久 SEVERED 处理——但角色从此永久保持抱臂姿势站立/行走/战斗，视觉上完全不像刚刚逃出生天的修士，直到某个更高优先级动画（例如下次攻击/受击/再次渡劫）偶然覆盖同一 `FULL_BODY` 通道，或者玩家重新连接客户端刷新动画状态。这是一个纯视觉但持续性极强的沉浸破坏（"卡姿"类 bug 对末法修士的具身感破坏尤其明显），且发生在渡劫这种高曝光、高情绪张力的场景。

## 证据定位

- `server/src/schema/tribulation.rs:25-31`：`DuXuOutcomeV1` 枚举 5 变体 `Ascended` / `HalfStep` / `Failed` / `Killed` / `Fled`。
- `server/src/cultivation/tribulation.rs:770-775`：`TribulationSettled` 事件定义（`entity`/`kind`/`source`/`result: DuXuResultV1`）。
- `server/src/cultivation/tribulation.rs:801-805`：`TribulationFled` 事件定义（`entity`/`tick`），独立于 `TribulationSettled`。
- `server/src/cultivation/tribulation.rs:3498-3585`：`tribulation_escape_boundary_system`——玩家跨维度或越出 `state.lock_radius(clock.tick)`（L3562）都会调 `settle_fled_tribulation`；已挂进生产 `Update` 调度（见下）。
- `server/src/cultivation/tribulation.rs:3589-3670`：`settle_fled_tribulation`——写传记 `BiographyEntry::TribulationFled`（L3612）、扣真元惩罚+经脉 SEVERED（L3618-3639，走 `release_qi_amount_to_zone` 守恒归还 zone，非本 bug 涉及范围）、发 `TribulationSettled{outcome: Fled}`（L3646-3659）+ `TribulationFled`（L3660-3663），**全程无任何 `StopAnim`/`PlayAnim` emit**。
- `server/src/network/vfx_animation_trigger.rs:458-496`：`emit_tribulation_animation_triggers` 的 `EventReader` 只有 `TribulationAnnounce`（L459）和 `TribulationFailed`（L460），无 `TribulationFled`；`TribulationFailed` 分支（L475-495）里对每个 failure 都做 `emit_stop_for_entity(ANIM_TRIBULATION_BRACE, TRIBULATION_BRACE_STOP_FADE_OUT_TICKS)`（L479-486，即 #1255 引入的修复）。
- `server/src/network/vfx_animation_trigger.rs:503-541`：`emit_tribulation_settled_vfx_triggers` 对 `TribulationSettled.result.outcome` 做 `match`，只处理 `Ascended`/`HalfStep`（L510-511），其余（含 `Failed`/`Killed`/`Fled`）落入 `_ => continue`（L512）——不发任何 VFX，也不发 `StopAnim`。
- `server/src/network/mod.rs:805-810`：`emit_tribulation_animation_triggers` 和 `emit_tribulation_settled_vfx_triggers` 均已挂进生产 `Update` 调度（分别 `.after(start_tribulation_system)`/`.after(tribulation_failure_system)` 和 `.after(juebi_settlement_system)`/`.after(tribulation_failure_system)`），确认非死代码。
- `server/src/network/vfx_animation_trigger.rs:66-71`：`ANIM_TRIBULATION_BRACE`（`"bong:tribulation_brace"`）与 `TRIBULATION_BRACE_STOP_FADE_OUT_TICKS`（`3`）常量定义，注释明确写「渡劫失败（`TribulationFailed`）时收掉…循环动画的淡出 tick」——只字未提 Fled。
- `server/src/network/vfx_animation_trigger.rs:3119-3139`：既有测试 `tribulation_killed_fled_settled_does_not_emit_success_vfx` 只断言 `Killed`/`Fled` 结局「不发 success VFX」，**没有断言 `StopAnim(brace)` 被发出**——测试当前证明的只是"没有误发"，不是"正确清了通道"。
- `server/src/network/vfx_animation_trigger.rs:3174-3222`：#1255 为 `TribulationFailed` 补的回归测试 `tribulation_failed_stops_stuck_brace_full_body_loop`——本 plan 的验收测试将对照这个既有写法为 `TribulationFled` 补同款测试。
- `server/src/test_coverage_guards.rs:126-131`：`INTENTIONAL_UNCONSUMED_EVENTS` 里已有 `TribulationFled` 的 triage 条目，`status: DeferredFollowUp`，`reason: "渡劫逃逸事件目前缺运行时 reader；渡劫状态机直接处理主效果"`，`follow_up: "tribulation narration/telemetry follow-up"`——即代码库自己承认 `TribulationFled` 目前没有真实 `EventReader` 消费者，但这条 triage 把缺口定性为"叙事/遥测待接"，完全没提及它同时也是一个动画状态机缺口（brace 卡姿）。**这条 triage 条目本身就是决定性证据**：确认了截至当前 HEAD，全仓没有任何地方读 `TribulationFled`。
- `server/src/test_coverage_guards.rs:187-190, 340-346, 375+`：`event_emitters_have_readers_or_triage_entries` 测试会跑 `find_stale_triage_entries`——一旦某个 triage 过的事件被真实 `EventReader<T>` 消费，这条测试会报「stale triage entry」并让 `cargo test` 失败。这意味着本 bug 的修复**必须同步删除** `test_coverage_guards.rs:126-131` 的 `TribulationFled` 条目，否则新增的 `EventReader<TribulationFled>` 会直接把这个门禁测试撞红。

## 触发路径

1. 玩家触发六境突破（`DuXu`）渡劫，`TribulationAnnounce` 起播 `FULL_BODY`、`isLoop:true` 的 `bong:tribulation_brace`（抱臂承劫）循环动画，`STORY_PRIORITY`。
2. 玩家在天劫波次进行中主动逃离渡劫中心（epicenter），移动距离超出 `state.lock_radius(clock.tick)`（随波次收缩的锁定半径，是设计好的"逃劫"机制），或在渡劫过程中切换维度（`tribulation_dimension_for_participant(current_dimension) != tribulation_dimension`）。
3. `tribulation_escape_boundary_system`（挂在生产 `Update` 调度）检测到越界/离场，调用 `settle_fled_tribulation`。
4. `settle_fled_tribulation` 写传记、走真元惩罚（守恒归还 zone）、经脉走永久 SEVERED，发出 `TribulationSettled{outcome: Fled}` + `TribulationFled`；**全程不 emit 任何动画事件**。
5. `emit_tribulation_animation_triggers` 不订阅 `TribulationFled`，跳过；`emit_tribulation_settled_vfx_triggers` 对 `outcome: Fled` 走 `_ => continue`，同样跳过。
6. 客户端从未收到对 `bong:tribulation_brace` 的 `StopAnim`，玩家角色永久卡在抱臂 `FULL_BODY` 循环姿势——直到某次战斗/再次渡劫的更高/同优先级 `PlayAnim` 偶然覆盖同一通道，或客户端重连刷新动画状态。

## 反方审查记录

- 第一轮质疑：
  - 是否 `TribulationFled` 其实有别处消费？`grep` 全仓 `server/src/` 确认 `TribulationFled` 除 `life_record`/`persistence`（写传记文本，与动画无关）外无任何消费者；`test_coverage_guards.rs` 自己的 triage 条目也承认"缺运行时 reader"。
  - 是否 `emit_tribulation_settled_vfx_triggers` 隐式处理了 Fled？读代码确认 `match` 只列 `Ascended`/`HalfStep` 两个分支，`Fled` 落 `_ => continue`，无 side effect。
  - 是否已有 PR/plan 覆盖同一问题？搜索 `docs/plans-skeleton/` 未见 `tribulation-fled`/`brace` 相关 skeleton；唯一相关文档命中是 `docs/plans-skeleton/plan-unconsumed-event-feedback-v1.md`（骨架，非 active plan），该文档针对 `TribulationFled` 提议的是"缺叙事 narration reader"这一不同的缺口（玩家看不到逃劫叙事提示），跟本 finding 的"动画卡姿"是两个不同的失效模式，不构成重复。
  - 初裁：倾向真 bug，与 #1255 完全同构，可达性高（正常玩法分支，非 dev 命令）。
- 第二轮补证：
  - 核实 `tribulation_escape_boundary_system` 确实挂在生产 `Update` 调度（`server/src/network/mod.rs:805-810`），排除"死代码/未接线"的可能。
  - 核实 `DuXuOutcomeV1` 5 变体的其余 4 个都有对应处理路径（`Ascended`/`HalfStep` 靠 breakthrough 动画覆盖同通道，`Failed` 已被 #1255 修复，`Killed` 走死亡生命周期），确认 `Fled` 是唯一遗漏，不是设计上"部分结局不需要清理"。
  - 核实既有测试 `tribulation_killed_fled_settled_does_not_emit_success_vfx` 的断言范围——只保证"不误发 success VFX"，不构成对本 bug 的既有回归覆盖，即当前测试套件对这个卡姿场景是**盲区**。
  - 补证 `test_coverage_guards.rs` 的 `event_emitters_have_readers_or_triage_entries` 门禁机制：修复本身会让现有 `TribulationFled` 的 `DeferredFollowUp` triage 条目变 stale，必须同步移除，这是修复范围内的一个容易漏掉但会直接导致 `cargo test` 失败的步骤。
  - 终裁：通过，判定为真 bug，严重度 medium（视觉持续性卡姿，无数值/守恒/权限层面影响，不升级为 high/critical）。

主循环复核：已亲读关键行确认（`tribulation.rs:770-805/3498-3670`、`vfx_animation_trigger.rs:458-541/3119-3222`、`network/mod.rs:805-810`、`test_coverage_guards.rs:126-190,340-390`、`schema/tribulation.rs:25-31`，并核对 #1255 commit `628f644a8` 的既有修复范围与本 finding 的差异点）。

## Skeleton Fix Plan

- [ ] 在 `server/src/network/vfx_animation_trigger.rs::emit_tribulation_animation_triggers` 的函数签名新增 `mut fled: EventReader<TribulationFled>` 参数（对齐已有 `announces`/`failures` 命名风格）。
- [ ] 在函数体内新增 `for event in fled.read() { ... }` 分支：对每个 fled 事件调用 `emit_stop_for_entity(position, unique_id, ANIM_TRIBULATION_BRACE, TRIBULATION_BRACE_STOP_FADE_OUT_TICKS, &mut vfx_events)`（`players.get(event.entity)` 查询失败时与 `TribulationFailed` 分支同款静默跳过，不 panic）。
- [ ] 评估是否把 `TribulationFailed` 分支和新 `TribulationFled` 分支的"StopAnim(brace)"逻辑抽成共享私有 helper（例如 `fn stop_tribulation_brace_for(entity, players, vfx_events)`），减少重复；两者结构完全一致，抽取可读性更好，但不是强制项——保留独立分支也可接受，只要行为正确且有测试锁住。
- [ ] **不要**给 `Fled` 分支额外复用 `ANIM_HURT_STAGGER`/`HIT_RECOIL_PRIORITY`（那是"被劫火命中"的受击反馈，逃劫成功不等于挨打）；本次修复范围只保证收掉 `bong:tribulation_brace` 循环，不引入新的"逃劫"专属姿态动画（如需要更丰富的逃劫视觉表现，另开 plan，避免范围蔓延）。
- [ ] 同步删除 `server/src/test_coverage_guards.rs:126-131` 里 `TribulationFled` 的 `UnconsumedEventTriage` 条目——新增 `EventReader<TribulationFled>` 后该条目会被 `find_stale_triage_entries` 判定为 stale，`event_emitters_have_readers_or_triage_entries` 测试会panic 报错，必须在同一 PR 里删掉这条 triage entry（对齐 `TechniqueLearnedEvent` 在同文件里被移除 triage 条目的先例，见该文件 L117-119 的注释）。
- [ ] 补充与 #1255 同款饱和测试（对照 `tribulation_failed_stops_stuck_brace_full_body_loop`，`vfx_animation_trigger.rs:3174-3222` 的写法）：`TribulationAnnounce` 起播 brace → `TribulationFled` → 断言恰好 emit `StopAnim(ANIM_TRIBULATION_BRACE, TRIBULATION_BRACE_STOP_FADE_OUT_TICKS)`，事件计数精确断言（而非只判非空）。
- [ ] 核实/扩展既有 `tribulation_killed_fled_settled_does_not_emit_success_vfx`（`vfx_animation_trigger.rs:3119-3139`）：该测试跑在 `emit_tribulation_settled_vfx_triggers` 上，与新增的 `emit_tribulation_animation_triggers` 分支是两个独立系统，职责不应混淆——`settled` reader 继续保证 Fled 不发 success VFX（PlayAnim breakthrough + SpawnParticle pillar），新的 `animation` reader 分支单独保证 Fled 发 StopAnim；两套测试都要绿且互不干扰。
- [ ] 补一条显式回归：`Ascended`/`HalfStep`（`tribulation_settled_success_outcomes_do_not_regress_with_explicit_brace_stop`）与 `Failed`（`tribulation_failed_stops_stuck_brace_full_body_loop`、`existing_tribulation_failure_animation_unaffected_by_settled_system`）路径必须保持既有断言不变——尤其确认成功路径**没有**因为本次改动被误加 `StopAnim`。
- [ ] 本 bug 不涉及真元/灵气流动新增或改动（`settle_fled_tribulation` 内既有的 `release_qi_amount_to_zone` 真元惩罚归还逻辑保持不变，不在本 plan scope），也不涉及 C2S 请求（`TribulationFled` 完全是服务端渡劫状态机内部产生的 S2C 动画事件，玩家客户端没有任何输入触发或绕过它的手段）——因此无需守恒模式改造，也无需 server gate 权威性设计；本修复纯粹是"服务端已有权威状态转换补一条缺失的下游动画消费"。

## 验收测试计划

全部落在 `server/` 栈，`cd server && cargo test`（附带 `cargo fmt --check && cargo clippy --all-targets -- -D warnings`）：

- **happy path**：`vfx_animation_trigger.rs` 新测试（对照 `tribulation_failed_stops_stuck_brace_full_body_loop` 写法）——`TribulationAnnounce` 起播 → `drain_vfx` 断言恰好 1 条 `PlayAnim(ANIM_TRIBULATION_BRACE, STORY_PRIORITY)`；随后发 `TribulationFled{entity, tick}` → `drain_vfx` 断言恰好 1 条 `StopAnim(ANIM_TRIBULATION_BRACE, TRIBULATION_BRACE_STOP_FADE_OUT_TICKS)`（**不是** `>=1` 或非空判断，精确断言事件数量和字段，失败信息注明"没有 StopAnim 则玩家永久卡抱臂姿势"）。
- **边界（多 entity）**：同一 tick 内两个不同玩家实体各自触发 `TribulationFled`，断言每个 entity 都各自收到自己的 `StopAnim`（用各自的 `position`/`unique_id`），不串扰、不漏发、不重复。
- **错误分支（entity 查询失败）**：`fled.entity` 对应的 `players.get()` 查询失败（模拟玩家已 despawn/断线场景）时，与 `TribulationFailed` 分支同款静默跳过、不 panic——补一条显式测试覆盖这个边界，不能只靠 happy path 隐式带过。
- **状态转换（不回归其余 4 个 `DuXuOutcomeV1` 分支）**：
  - `Ascended`/`HalfStep`：`tribulation_settled_success_outcomes_do_not_regress_with_explicit_brace_stop` 保持绿，显式断言 emitted 事件中**没有** `StopAnim`（success 路径靠 breakthrough PlayAnim 覆盖同通道，不该新增 StopAnim）。
  - `Failed`：`tribulation_failed_stops_stuck_brace_full_body_loop`、`existing_tribulation_failure_animation_unaffected_by_settled_system` 保持绿，不受本次改动影响。
  - `Killed`：新增一条显式回归，确认 `emit_tribulation_animation_triggers`（新 Fled 分支引入后）不会误吞或误触发 Killed 路径的行为——Killed 继续被死亡生命周期单独处理，本 plan 不改动。
  - `Fled` 经 `emit_tribulation_settled_vfx_triggers`：`tribulation_killed_fled_settled_does_not_emit_success_vfx` 保持绿（settled reader 继续不发 success VFX），与新增的 animation reader 分支（发 StopAnim）互不冲突——补充断言说明两套系统各管一半、合起来才是完整的 Fled AV 收尾。
- **门禁回归（test_coverage_guards）**：`event_emitters_have_readers_or_triage_entries`、`triage_entries_are_unique_and_documented` 两条测试必须绿——验证 `TribulationFled` 的 `UnconsumedEventTriage` 条目已从 `INTENTIONAL_UNCONSUMED_EVENTS` 删除，且删除后不产生"untriaged unconsumed writer"（因为现在有真实 `EventReader<TribulationFled>` 消费了）。
- **完整命令**：`cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`，确认新增测试 + 全部既有 `tribulation`/`vfx_animation_trigger`/`test_coverage_guards` 测试同时绿。

## 风险

- **最容易漏掉的一步**：忘记同步删除 `test_coverage_guards.rs:126-131` 的 `TribulationFled` triage 条目，会让 `event_emitters_have_readers_or_triage_entries` 因 `find_stale_triage_entries` 直接把 `cargo test` 撞红——这不是"锦上添花"，是本修复能否通过门禁的必要步骤。
- **范围蔓延风险**：产品/视觉上可能想给"逃劫成功"设计一个专属姿态（不只是简单收掉 brace），但那属于新增招式视听差异化范畴（需要遵守 A/V 差异化硬约束：独立 animation+粒子+音效+HUD），超出本 bug 修复范围——本 plan 明确只做"StopAnim 收尾"，不新增姿态资产。
- **Killed 分支未探查**：本 finding 假设 `Killed` 结局由死亡生命周期的死亡动画顺带清理 `FULL_BODY` 通道，但本 plan 未深入验证死亡动画具体清理路径的代码；若后续发现 `Killed` 也有类似卡姿（本 plan 未覆盖），应另开独立 bughunt，不要顺手在本 PR 里扩大修复面。
- **跨栈影响面小**：本修复只新增 server 端 `EventReader` 消费和一次 `emit_stop_for_entity` 调用，复用 #1255 已验证过的 `StopAnim` payload 类型和客户端消费路径，预期不需要改动 client/schema/agent；若 review 中发现客户端对 `StopAnim` 的处理存在 Fled 场景特有的边界问题，需要单独升级为跨栈修复而不是默认"肯定没问题"。
