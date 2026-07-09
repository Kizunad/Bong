# plan-bughunt-realm-taint-restart-amnesia-v1

> Active Plan（BugHunt persistence r13）。本文件位于 `docs/plan-*.md` 活跃消费路径，已定稿为可由 plan 流水线零交互执行的修复计划。

## Bug 摘要

龛侵色 `RealmTaintState` 是灵龛抄家后的追凶状态：入侵者拿走物品后会累积 `NicheIntrusion` 染色，8 小时窗口内可被固元+ 玩家用神识追踪。但该状态只存在 ECS 组件里，玩家断线、关服 flush、重登 hydrate 都没有保存或恢复它。结果是抄家者只要断线重连或等服主重启，就能清空龛侵色追踪窗口。

## 实际游玩体验影响

守家玩家本应在 8 小时内用神识追踪抄家者，或至少在抄家者身上看到龛侵色代价。现在抄家者成功偷走灵龛物品后，只要重登或服务器重启，服务端就不再给神识扫描生成 `NicheIntrusionTrace`。玩家视角是“刚被抄家，追凶线索被重启洗白”，守家/复仇玩法的核心惩罚窗口被持久化缺口绕过。

## 复现路径

1. 玩家 A 放置灵龛并有可被抄家的物品。
2. 玩家 B 成功突破守护并拿走至少一件物品，`niche_defense` 给 B 发出 `ApplyRealmTaint { kind: NicheIntrusion }`。
3. B 身上出现 `RealmTaintState`，`wash_available_at = tick + 8h`。
4. 在 8 小时窗口内让 B 断线重连，或服主正常关服后重启。
5. B 重登后 `RealmTaintState` 不再存在；固元+ 玩家神识扫描不到 B 的龛侵色痕迹。

## 根因证据

- `docs/finished_plans/plan-niche-defense-v1.md:152-157`、`:164-171`：设计明确要求抄家者染上龛侵色，8 小时内可被神识追凶。
- `docs/finished_plans/plan-niche-defense-v1.md:381-382`：P4 明确把 `NicheIntrusionTrace` 接到 `spiritual_sense/push.rs`。
- `server/src/social/niche_defense.rs:337-343`：抄家结果 `taint_delta > 0.0` 时向入侵者发送 `ApplyRealmTaint`。
- `server/src/cultivation/realm_taint.rs:20-26`：`RealmTaintState` 是独立玩家组件，包含 `qi_taint_severity` / `last_tainted_at` / `wash_available_at`。
- `server/src/cultivation/realm_taint.rs:40-44`：每次龛侵色累积会刷新 8 小时洗清窗口。
- `server/src/cultivation/spiritual_sense/push.rs:214-225`、`:400-411`：神识追踪只读取当前 ECS 上的 `Option<&RealmTaintState>`；组件缺失即没有 `NicheIntrusionTrace`。
- `server/src/persistence/mod.rs:5484-5520`：`persist_player_cultivation_bundle` 保存 cultivation、meridians、qi_color、karma、contamination、life_record、insight、poison/digestion 等字段，但没有 `realm_taint`。
- `server/src/cultivation/mod.rs:589-664`、`:883-900`：重登 hydrate 只恢复上述 cultivation bundle 字段，最终插回的组件列表没有 `RealmTaintState`。

## 去重边界

- 不重复 #1064 / `plan-bughunt-status-effects-consumable-persistence-v1`：那条是 `StatusEffects` 消耗品长效 buff 重启清空；本题是 `RealmTaintState` 龛侵色追凶状态，组件、写入入口、读取入口均不同。
- 不重复 niche guardian client/store 题：本题不是 HUD 跨 session 残留，也不是 guardian proto kind，而是服务端权威追凶状态在 player persistence 中缺失。
- `LifeRecord::NicheIntrusion`、Redis / Tiandao 事件只能记录叙事或广播；它们不含可恢复的 severity / wash 窗口，也不被 `spiritual_sense` 读取，不能替代 `RealmTaintState` hydrate。

## 修复计划

### P0 - 玩家 cultivation bundle 持久化龛侵色

- [ ] 不直接把运行时 `RealmTaintState` 原样序列化为持久格式；新增持久化 DTO，例如 `PersistedRealmTaintState { v, kind, qi_taint_severity, wash_remaining_ticks, tainted_elapsed_ticks }`。原因：`wash_available_at` / `last_tainted_at` 是本进程 uptime tick，重启后绝对 tick 失效。
- [ ] `persist_player_cultivation_bundle` 增加 `now_tick: u64` 与 `realm_taint: Option<&RealmTaintState>` 参数；写 JSON bundle 的可选 `realm_taint` 字段时，`wash_remaining_ticks = state.wash_available_at.saturating_sub(now_tick)`，只保存 `kind == NicheIntrusion`、`qi_taint_severity > 0`、`wash_remaining_ticks > 0` 的状态。
- [ ] 断线 cleanup、关服 flush、周期 cultivation flush 的 query 纳入 `Option<&RealmTaintState>`，并读取 `Res<CombatClock>` 传入当前 tick；测试 helper 调用显式传入固定 tick，避免隐藏依赖。
- [ ] `attach_cultivation_to_joined_clients` 解码 persisted `realm_taint`，只在字段合法、未洗清且 severity > 0 时插回 `RealmTaintState`：`wash_available_at = now_tick + wash_remaining_ticks`，`last_tainted_at = now_tick.saturating_sub(tainted_elapsed_ticks)`。
- [ ] 旧 bundle 缺少 `realm_taint` 时按 `None` 处理；`realm_taint` 字段类型错误、未知 `kind`、非数字/负数 severity、`wash_remaining_ticks == 0`、过大 tick 溢出等坏数据只 warn 并忽略该字段，不阻断玩家登录，也不影响 cultivation / meridian / life_record 等其它字段 hydrate。
- [ ] 转世分支必须清空旧角色龛侵色：新生命上线不插回旧 `realm_taint`，并持久化 `realm_taint: null` 或省略该字段，避免旧抄家染色附到新生命上。

### P1 - 时间语义与洗清边界

- [ ] 定稿语义：龛侵色洗清窗口按游戏内 server tick 流逝，服务器停机期间不推进洗清；重启/重登不能提前清空，最多只让墙钟时间看起来变长。这与当前 `RealmTaintState::wash_if_ready(now_tick)` 的 tick 语义一致。
- [ ] 持久化只保存剩余 tick，不保存旧 `wash_available_at` 绝对 uptime tick；hydrate 时以当前 `CombatClock.tick` 重新计算 `wash_available_at`。这是本 plan 的唯一 P0 口径，不再保留 wall-clock 二选一产品决策。
- [ ] 如果后续产品要改成墙钟到期，必须另立 plan 做 schema v2 迁移；本 plan 不引入 wall-clock deadline，避免在修复重启遗忘 bug 时顺手改变 8h 游戏内窗口平衡。

### P2 - 回归覆盖

- [ ] server 单测：构造玩家 `RealmTaintState`，保存 cultivation bundle，模拟新 App 重登，断言 `RealmTaintState` 恢复为当前 tick + 剩余 tick，且 `spiritual_sense` 仍能生成 `NicheIntrusionTrace`。
- [ ] server 单测：旧 bundle 缺少 `realm_taint` 字段时登录不崩，恢复结果为无龛侵色；坏 `realm_taint` 字段（类型错误、未知 kind、负 severity、零/溢出 remaining）只忽略该字段，不阻断其它 cultivation bundle 字段恢复。
- [ ] server 单测：断线 cleanup、关服 flush、周期 cultivation flush 三条调用都把 `Option<&RealmTaintState>` 和当前 `CombatClock.tick` 传入 persistence，避免只修一条保存路径。
- [ ] server 单测：转世后不恢复旧龛侵色。
- [ ] server 单测：已过期或 severity=0 的龛侵色 hydrate 时不插回。
- [ ] Bot e2e：新增 `scripts/bot/scenarios/realm_taint_persistence.py`。场景用两名 bot 或 dev-only 测试入口给 B 施加 `NicheIntrusion` 龛侵色，触发 B 重登或测试服重启，再由 A/第三方固元+ 发起神识扫描，黑盒断言仍能看到 B 的 `NicheIntrusionTrace` / 对应可观察反馈。
- [ ] 根目录 e2e：`bash scripts/bot-e2e.sh realm_taint_persistence` 或等价 bot scenario runner 纳入验证说明；若 CI 不适合重启真服，至少覆盖 B 重登路径，并用 server 集成测试覆盖新 App 重启 hydrate 路径。

## Adversarial Review

- Round 1：反方尝试用“短期 session 状态 / 已有 StatusEffects plan / LifeRecord 可恢复”推翻，结论为 REAL。`plan-niche-defense-v1` 明确写 8 小时追凶窗口，`spiritual_sense` 实际只读 `RealmTaintState`。
- Round 2：强反方继续攻击“只在线有效”“已有 persistence/status/niche guardian 题覆盖”“LifeRecord/Redis/Tiandao 可恢复”，最终仍判 REAL、非 DUPLICATE。
- 最终 adversarial conclusion：`RealmTaintState` 是龛侵色/神识追凶的唯一运行时权威，但玩家断线、关服 flush 与重登 hydrate 都不保存/恢复它；现有 LifeRecord/Redis/Tiandao 只记录事件叙事，不能重建 8h 污染窗口，因此抄家者可通过重登或重启清空龛侵色。
