# BugHunt: /season 人工相位重启回滚

> Skeleton Plan（report-only）。一句话主题：op/admin 通过 `/season set|advance` 建立的世界季节 override 只存在 `WorldSeasonState.tick_offset` 内存里；服务器重启后 `WorldSeasonState::default()` 重新从默认 tick 派生季节，人工切到冬季/汐转的事件相位会回滚。

## Bug 摘要

`/season set` 与 `/season advance` 会调用 `WorldSeasonState::set_phase` / `advance_by_ticks`，把目标相位编码进私有 `tick_offset`。该 offset 决定 `effective_tick()`，进而决定 `WorldSeasonState.current`。但 persistence bootstrap 只 hydrate void cooldown、heartbeat pseudo-veins、zone runtime / overlays、zone influence；shutdown 也只强制 flush zone runtime。当前没有任何 world season snapshot/hydrate，也没有 schema 保存 `tick_offset` 或等价的 effective phase。

因此，服主或活动管理员把世界切到冬季、汐转等人工事件相位后，只要服务器正常重启，季节资源会回到默认时钟派生值。这个 plan 不主张“自然季节必须随停服墙钟流逝”，也不要求持久化全局 `CultivationClock`；问题只限定为 `/season` 建立的人工 override 没有恢复。

## 实际游玩体验影响

玩家正在参与由管理员切出的冬季/汐转活动时，重启后会看到世界规则突然跳回默认夏季节律：灵田 pressure 与自然供给、天气概率、NPC 季节行为、world heartbeat 季节倍率、client/agent 下发的 `season_state` 都会跟着改变。体感上不是“停服期间季节没流逝”，而是服主刚设定的事件季节被服务器忘掉，活动环境和数值条件在重登后不一致。

## 证据定位

- `server/src/main.rs:90`：生产入口调用 `cmd::register(&mut app)`，命令树不是测试专用。
- `server/src/cmd/mod.rs:10-12`：`cmd::register` 注册 `dev::register(app)`。
- `server/src/cmd/dev/mod.rs:44-56`：`dev::register` 注册 `season::register(app)`，`/season` 属于生产命令树里的 op-only 工具。
- `server/src/cmd/dev/season.rs:135-140`：`season::register` 初始化 `WorldSeasonState`，注册 `SeasonCmd` 和 `handle_season`。
- `server/src/cmd/dev/season.rs:167-172`：`/season set|advance` 分别调用 `set_phase` / `advance_by_ticks`。
- `server/src/world/season/mod.rs:149-179`：`WorldSeasonState` 持有私有 `tick_offset`，`effective_tick()`、`set_phase()`、`advance_by_ticks()` 都依赖它；该 struct 未接持久化 schema。
- `server/src/world/season/mod.rs:189-192`：world season 注册只插入 `WorldSeasonState::default()` 并跑 `season_tick`。
- `server/src/cultivation/mod.rs:236-238`：`CultivationClock` 默认插入；本 plan 不要求改成墙钟或全局持久化时钟。
- `server/src/persistence/mod.rs:688-790`：persistence bootstrap hydrate 的资源不包括 `WorldSeasonState` / `tick_offset`。
- `server/src/persistence/mod.rs:661-685`、`server/src/persistence/mod.rs:854-877`：shutdown 强制 flush 只覆盖 zone runtime + heartbeat pseudo-veins。
- `server/src/lingtian/systems.rs:1576-1600`：灵田 pressure 从 `WorldSeasonState.current.season` 取季节修饰。
- `server/src/lingtian/weather.rs:445-529`：天气生成读取 `WorldSeasonState.current.season`。
- `server/src/npc/seasonal_behavior.rs:37-50`：NPC 季节行为读取 `WorldSeasonState.current.season`。
- `server/src/world/heartbeat.rs:731-767`：world heartbeat 使用 `WorldSeasonState` 的季节和相位切换 tick。

## 触发路径

1. op/admin 在正式服务器执行 `/season set winter` 或 `/season advance ...`，用于活动、联调或灾异节律切换。
2. `handle_season` 调用 `WorldSeasonState::set_phase` / `advance_by_ticks`，更新内存里的 `tick_offset` 与 `current`。
3. 灵田、天气、NPC、heartbeat、client/agent season state 都开始按人工相位运行。
4. 服务器正常停服或重启。
5. 启动时 `world::season::register` 重新插入默认 `WorldSeasonState`；`persistence::bootstrap_persistence_system` 没有 hydrate season override。
6. 世界回到默认 tick 派生季节，人工设置的活动相位消失。

## Skeleton Fix Plan

- [ ] 为世界季节 override 增加最小 snapshot：保存能恢复 `/season` 人工相位的 `tick_offset` 或等价 `effective_tick` / phase anchor；不要持久化全局墙钟时间。
- [ ] 在 persistence migration 中新增 world season runtime 表或复用合适的 singleton runtime 表，明确 schema version。
- [ ] 在 startup bootstrap hydrate `WorldSeasonState`，只恢复显式 override；缺行时保持当前默认行为。
- [ ] 在 `/season set|advance` 后将 override 标脏，按节流或立即写入；shutdown `Last` 收到 `AppExit` 时强制 flush。
- [ ] 提供清除/回归自然节律的策略，避免一次 admin override 永久卡死为不可取消状态。

## 验收测试计划

- [ ] `season_override_round_trips_through_sqlite`：设置 winter / 汐转后持久化，再 fresh app hydrate，断言 `effective_tick()` 和 `current.season` 与重启前一致。
- [ ] `missing_season_override_keeps_default_startup_behavior`：空 DB 启动仍是 `WorldSeasonState::default()`，不引入自然季节墙钟流逝语义。
- [ ] `season_advance_override_survives_shutdown_flush`：`/season advance` 后不等节流，发送 `AppExit`，fresh app hydrate 后相位不回滚。
- [ ] `clear_override_returns_to_clock_derived_season`：若实现清除命令/状态，验证清除后重启不再恢复旧 override。
- [ ] server 验证走 `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`。

## 反方审查记录

### Round 1

反方尝试打回为 dev-only / 已有重复 / 隐藏恢复路径。结论保留：`cmd::register` 在生产 `main.rs` 注册，`/season` 实际进入命令树；`WorldSeasonState.tick_offset` 没有 Serialize/Deserialize 或 persistence hydrate；现有 season proto enum、client stale、zone environment plans 都不是 server restart + tick_offset 恢复问题。

### Round 2

反方继续攻击严重度和表述边界。结论降级保留：`plan-jiezeq-v1` 允许自然季节从 server start tick 派生，不能把“停服期间自然季节没流逝”写成 bug，也不能要求持久化 `CultivationClock`。但 `/season set|advance` 是 admin 操作建立的显式 override，丢失后会改变所有读取 `WorldSeasonState.current` 的生产系统；应按中等 severity 的 admin-event persistence gap 处理。

## 风险与边界

- 不要把本 plan 扩大成全局时间系统迁移。
- 不要声称普通玩家可直接触发 `/season`；触发者是 op/admin，但影响会传导给在线玩家。
- 不要与 `docs/plan-season-phase-stale-client-v1.md` 混淆：那是服务端已跨季但 client 未及时同步，本 plan 是服务端重启后丢失人工 override。
- 修复时要保持缺 snapshot 时的默认启动语义，避免把历史存档突然迁移到墙钟季节。
