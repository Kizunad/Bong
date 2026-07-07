# plan-bughunt-realm-taint-restart-amnesia-v1

> Skeleton Plan（BugHunt persistence r13）。仅记录真实 bug 与修复计划，不做实际修复。

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

- [ ] 在 `persist_player_cultivation_bundle` 参数与 JSON bundle 中加入可选 `RealmTaintState`。
- [ ] 断线、关服、周期 cultivation flush 的 player query 纳入 `Option<&RealmTaintState>`。
- [ ] `attach_cultivation_to_joined_clients` 从 persisted bundle 解码 `realm_taint`，只在未洗清且 severity > 0 时插回组件。
- [ ] 转世分支必须清空旧角色龛侵色，避免旧抄家染色附到新生命上。

### P1 - 时间语义与洗清边界

- [ ] 明确 `wash_available_at` 跨重启语义：至少保证重启不会提前清空；是否按停服墙钟流逝另行产品决策。
- [ ] 若继续使用 game tick，hydrate 时保留剩余 tick 而不是旧 uptime 绝对 tick，避免重启后窗口异常变长或变短。
- [ ] 若改为 wall-clock 到期，补迁移兼容旧 game tick 字段。

### P2 - 回归覆盖

- [ ] server 单测：构造玩家 `RealmTaintState`，保存 cultivation bundle，模拟新 App 重登，断言 `RealmTaintState` 恢复且 `spiritual_sense` 仍能生成 `NicheIntrusionTrace`。
- [ ] server 单测：转世后不恢复旧龛侵色。
- [ ] server 单测：已过期或 severity=0 的龛侵色 hydrate 时不插回。
- [ ] e2e：A 被抄家，B 重登或重启后，A/第三方固元+ 神识仍能在剩余窗口内追踪 B。

## Adversarial Review

- Round 1：反方尝试用“短期 session 状态 / 已有 StatusEffects plan / LifeRecord 可恢复”推翻，结论为 REAL。`plan-niche-defense-v1` 明确写 8 小时追凶窗口，`spiritual_sense` 实际只读 `RealmTaintState`。
- Round 2：强反方继续攻击“只在线有效”“已有 persistence/status/niche guardian 题覆盖”“LifeRecord/Redis/Tiandao 可恢复”，最终仍判 REAL、非 DUPLICATE。
- 最终 adversarial conclusion：`RealmTaintState` 是龛侵色/神识追凶的唯一运行时权威，但玩家断线、关服 flush 与重登 hydrate 都不保存/恢复它；现有 LifeRecord/Redis/Tiandao 只记录事件叙事，不能重建 8h 污染窗口，因此抄家者可通过重登或重启清空龛侵色。
