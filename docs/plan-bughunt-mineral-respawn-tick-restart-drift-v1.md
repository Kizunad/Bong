# plan-bughunt-mineral-respawn-tick-restart-drift-v1

> Skeleton Plan。主题：矿脉耗尽日志把再生时间保存为进程内绝对 tick，但 `MineralTickClock` 重启归零，导致已落盘的再生倒计时在重启后整体漂移。

## 一句话 bug

可再生矿脉的 `respawn_at_tick` 使用旧进程 `MineralTickClock.tick + respawn_ticks` 落盘；服务器重启后 `MineralTickClock::default()` 从 0 开始，而 hydrate 只恢复 entries、不恢复或换算 tick 基准，导致矿脉再生被上一轮 uptime 整体推迟，已到期矿脉也不会在重启后及时恢复。

## 实际游玩体验影响

玩家挖空凡铁、粗铁、杂钢、灵铁、髓铁、丹砂或灵石后，预期 1-8 小时内自然再生。若服务器在倒计时期间重启，重进后这些矿点会比配置时间更久才回来；旧进程运行越久，额外等待越长。

典型体感是“昨天挖过的低阶矿今天开服还没刷”，玩家会误判矿点是永久耗尽或世界资源被重启卡住。对炼器、炼丹、灵石燃料早期循环影响尤其明显，因为凡铁/粗铁 1 小时、丹砂 1.5 小时这类短周期资源最容易被日常重启打断。

## 证据定位

- `server/src/mineral/persistence.rs:57-70`：`ExhaustedEntry::from_event_with_respawn` 把 `respawn_at_tick` 写成 `tick.saturating_add(respawn_ticks)`。
- `server/src/mineral/persistence.rs:177-186`：`MineralTickClock` 是本模块轻量计数器，`Default` 后从 0 开始，每 tick 只做 `saturating_add(1)`。
- `server/src/mineral/persistence.rs:191-206`：耗尽事件记录时使用当前 `MineralTickClock.tick` 作为 `tick` 参数。
- `server/src/mineral/persistence.rs:234-253`：hydrate 只把磁盘 entries 放回 `log.entries`，没有恢复旧 tick 基准，也没有把 `respawn_at_tick` 换算为新进程时间。
- `server/src/mineral/mod.rs:71-78`：启动注册先 `ExhaustedMineralsLog::hydrated()`，随后直接 `insert_resource(MineralTickClock::default())`。
- `server/src/mineral/respawn.rs:29-37`：再生系统用新进程 `clock.tick` 调 `exhausted.remove_respawned(clock.tick)` 判断到期。
- `server/src/mineral/registry.rs:83-88`、`97-105`：多类资源配置了 72,000 到 576,000 tick 的再生周期。

## 触发路径

1. 服务器已运行一段时间，例如 `MineralTickClock.tick = 100000`。
2. 玩家挖空凡铁，`record_exhausted_minerals` 写入 `respawn_at_tick = 100000 + 72000 = 172000`。
3. `ExhaustedMineralsLog` 正常 flush 到 `data/minerals/exhausted.json`。
4. 服主重启服务器，`ExhaustedMineralsLog::hydrated()` 读回 `respawn_at_tick = 172000`。
5. 同一次启动中 `MineralTickClock::default()` 归零。
6. `respawn_exhausted_minerals` 需要新进程再跑到 tick 172000 才再生，而不是按剩余 72000 tick 或已离线时间判断。

## 反方审查记录

### Round 1

反方尝试反驳：

- 已有 `respawn_exhausted_minerals` 兜底，每 tick 会调用 `remove_respawned(clock.tick)` 并重建 `MineralOreNode`。
- 启动路径确实 hydrate 了 `ExhaustedMineralsLog`，但未看到恢复全局 tick；随后直接插入 `MineralTickClock::default()`。
- 持久化不是相对剩余时间，而是 `tick + respawn_ticks` 的绝对 tick；到期判断是 `current_tick >= respawn_at_tick`。
- 现有矿脉相关 PR/plan 覆盖的是耗尽日志半写、节流 flush 或矿点复活，不是“已落盘绝对 tick 跨重启基准丢失”。

### Round 2

反方再审结论：候选成立。未发现全局 tick hydrate、启动补偿、导入清理或 active plan 覆盖该具体缺陷。关键链路是旧进程绝对 tick 被序列化，新进程 tick 从 0 重算，导致额外延迟等于旧进程 uptime；若矿脉本应在关服期间或启动瞬间到期，也不会立即再生。

## 修复计划骨架

- [ ] 明确矿脉再生时间口径：使用持久化墙钟 `respawn_at_wall`，或持久化剩余 tick 并在 shutdown/startup 做有界换算；避免保存不可跨进程解释的本地绝对 tick。
- [ ] 为旧格式 `respawn_at_tick` 提供迁移策略，不能把已有可再生矿全部永久化或立即全量刷出。
- [ ] hydrate 时处理“已经到期”的条目，确保重启后首轮 Update 可以恢复矿点。
- [ ] 保持 `respawn_at_tick: None` 的永久耗尽旧语义不变。
- [ ] 不改矿物掉落、真元/灵气账本或 worldgen 锚点语义；本 plan 只处理再生时间持久化口径。

## 验收测试计划

- [ ] server 单测：构造磁盘 exhausted entry，模拟旧进程 tick=100000 写入、重启后 tick=0，断言不会要求新进程再跑完整 172000 tick。
- [ ] server 单测：离线/重启期间已到期的可再生矿，hydrate 后首轮再生系统能物化回 `MineralOreNode` 并更新 `MineralOreIndex`。
- [ ] server 单测：未到期矿保留正确剩余时间，不因重启立即刷出。
- [ ] 兼容性测试：旧 JSON 缺少新字段时按迁移策略处理；`respawn_at_tick=None` 永不再生矿仍不再生。
- [ ] 跑 `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`。

## 风险

- 直接按墙钟补偿会让长期停服期间矿物大量到期，可能改变资源节奏；需要用 plan 明确是否允许离线再生。
- 旧 `respawn_at_tick` 无法精确知道旧进程当前 tick 与关服时间，只能选择保守迁移策略。
- 修复不应复发 #985 一类耗尽日志损坏导致矿点复活问题，也不应混入关服强制 flush 范围。
