# plan-bughunt-surface-stash-global-reset-v1

> Skeleton plan。只读审计产物，来源 issue #1364。

## §0 摘要

散修遗缴搜索限额的计数键虽然包含 `(poi_id, player_id)`，但重置时使用一个全局 `last_reset_wall_clock`，并在任意玩家触发 24 小时边界时执行 `limits.clear()`。玩家 A 的搜索会因此清掉玩家 B 在所有遗缴点的剩余配额；同一进程内的配额也会随谁先触发检查而改变。

## §1 游玩影响

玩家可以通过等待或让另一名玩家触发一次全局重置，提前恢复自己的散修遗缴搜索次数。多人同时搜刮时，玩家看到的“每个 POI、每个玩家每日三次”不再成立，搜刮节奏和容器经济被不透明地改变。

## §2 复现路径

1. 在同一个服务端资源中，让玩家 A 在 POI `a` 搜索并记录次数，玩家 B 在 POI `b` 也记录次数。
2. 将 `last_reset_wall_clock` 设为距 `now_secs` 超过 24 小时，先调用 B 的 `can_search`。
3. `maybe_reset` 清空整张 `limits`，随后 A 的旧计数也变为零，A 可以立即重新搜索。
4. 重复交替调用不同玩家的检查，观察重置由调用顺序而非玩家自己的周期决定。

## §3 今天 `origin/main` 证据

- `server/src/world/tsy_container_search.rs:170-175`：`SurfaceStashPlayerLimit` 只有一张 `(poi_id, canonical_player_id) -> count` 表和一个全局 `last_reset_wall_clock`。
- `server/src/world/tsy_container_search.rs:177-194`：`can_search` 与 `record_search` 每次都先调用同一个 `maybe_reset`。
- `server/src/world/tsy_container_search.rs:196-201`：达到 24 小时后直接 `self.limits.clear()`，没有按玩家或 POI 保留未到期计数。
- `server/src/world/tsy_container_search.rs:238-242`：该资源由 TSY 搜索完成后的磨损链路使用，配额是正式搜刮入口的门禁，不是测试辅助状态。

## §4 非重复比对

- `plan-bughunt-surface-stash-lifecycle-volatile-v1.md` 处理 POI 生命周期/易失状态，不覆盖 `SurfaceStashPlayerLimit::maybe_reset` 的跨玩家清表。
- `plan-surface-stash-search-hud-label-gap-v1.md` 只处理客户端标签显示；本 issue 的服务端计数在 HUD 之外仍可独立复现。
- 现有 `finished_plans` 中未找到以 `last_reset_wall_clock` 或全局 24 小时重置为根因的 plan，因此保留为新骨架。

## §5 立项检查记录

- **worldview**：查 `散修`、`搜刮`、`容器与搜刮`、`24h`；命中 `docs/worldview.md §十六.三（容器与搜刮）` 的遗物容器风险循环，未发现允许跨玩家共享冷却的设定。
- **finished_plans**：查 `surface stash`、`SurfaceStashPlayerLimit`、`last_reset`、`poI`；已有生命周期/展示 plan，无本根因。
- **active plan**：查 `surface stash`、`search limit`、`last_reset_wall_clock`；未见 active plan 修改该资源。
- **skeleton**：查 `SurfaceStashPlayerLimit`、`limits.clear`、`24h`；仅命中本文件列出的生命周期与 HUD 骨架。
- **reminder.md**：查 `surface stash`、`遗缴`、`配额`、`24h`；仓内无相关条目。

## §6 接入面与跨仓契约

- **Inputs**：`SearchCompleted` 触发的 POI/玩家标识，`SurfaceStashPlayerLimit::can_search` / `record_search` 的 `now_secs`。
- **Outputs**：每个 `(poi_id, player_id)` 的每日计数与搜索准入结果，供 `tick_search_progress`/搜索处理链路读取。
- **共享类型或事件**：复用 `SurfaceStashPlayerLimit`、`SearchCompleted`、`TsyZoneStateRegistry`；不新造跨仓 schema。
- **server 符号**：`server/src/world/tsy_container_search.rs::SurfaceStashPlayerLimit::{can_search,record_search,maybe_reset}`、`tick_search_progress`、`apply_search_attrition`。
- **agent**：无变更；该配额只存在服务端 ECS/TSY 搜索门禁，不发布 Redis/agent 事件。
- **client**：无变更；客户端只发起既有搜刮交互并显示既有结果，不拥有或计算配额。
- **worldview 锚点**：`docs/worldview.md §十六.三` 的容器搜刮风险/节奏。

## 阶段总览

| 阶段 | 状态 | 交付物 |
| --- | --- | --- |
| P0 | ⬜ | 为每个玩家/POI 记录独立可比较的重置时间，去掉跨玩家清表副作用 |
| P1 | ⬜ | 迁移旧资源状态并补边界测试，证明并发调用不互相恢复配额 |

## P0：按玩家或按计数桶重置

- 选择与现有 wall-clock 契约一致、可持久化的 per-player/per-POI 时间桶；未到期条目不得被其他玩家清除。
- 保持 `SURFACE_STASH_DAILY_LIMIT`、`can_search`、`record_search` 的外部语义，重置只改变内部计数归属。

## P1：回归与迁移

- 覆盖同一 POI 两名玩家、同一玩家两个 POI、跨 24 小时边界和重复调用。
- 若资源未来持久化，定义旧全局时间戳的 fail-safe 迁移，不能因迁移把所有玩家无限放行。

## 验收测试计划

- A 触发重置后，B 的未到期计数保持不变；B 触发重置不清 A。
- 单玩家单 POI 达到 `SURFACE_STASH_DAILY_LIMIT` 时仍拒绝，跨一个完整周期后只恢复该键。
- 不同 POI 的计数互不影响，时钟回退和 `u64` 饱和计算不导致绕过。

## 来源 issue

- #1364 `[flash-review][major] 散修遗缴 24h 限额全局清空，可被他人搜索重置绕过`
