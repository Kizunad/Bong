# Plan: BugHunt SurfaceStash lifecycle 易失状态

> Skeleton（BugHunt H3 persistence 第三轮）。一句话：`SurfaceStash` 的生命周期只停在易失 ECS 状态：同进程内搜空后 3 分钟 respawn 只进入 ready 日志、不恢复可搜；进程重启后 Startup 又重建未搜空容器并清空 24h/player 限频，导致入门资源 uptime 内断供、重启后可重复刷。

## Bug 摘要

`SurfaceStash` 的设计目标是“每个遗缴对每个玩家每 real-time 24h 只产出 3 次”，并通过 3600 tick respawn 避免同一批地表遗缴永久消失。但当前实现把关键状态拆在两个纯内存 `Resource` / ECS component 上：

- `SurfaceStashPlayerLimit` 只在 `server/src/world/tsy_container_search.rs` 内存计数，没有 SQLite/JSON/Redis load/save。
- 搜刮完成只把当前 `LootContainer.depleted` 置 `true`。
- `PoiRespawnStore` 只判定站点 ready 并 `mark_refreshed`，没有重置已有 `LootContainer.depleted`，也没有重新 spawn 可搜容器。
- server 重启后 `scatter_and_spawn_surface_stashes` 又用 `LootContainer::new(... SurfaceStash ...)` 生成同一批 `depleted=false` 容器，同时 24h/player 限频从空 `HashMap` 开始。

## 对实际游玩体验的影响

玩家正常在线游玩时，搜完 spawn 附近 12 个散修遗缴后，文档承诺的 3 分钟补刷不会真的恢复可搜状态，入门资源链会在当前进程内断供。

如果服务器重启，同一批固定 seed 的散修遗缴又会以未搜空状态出现，且玩家 24h 限频也被清空。玩家可以通过重启周期重复刷 `surface_stash_basic` / `surface_stash_scroll` / `surface_stash_craft`，包括 `ling_shui`、配方残卷和蓝图。`ling_shui` 在 onboarding 文档里是入门阶段唯一来源，因此这不是单纯数值小偏差，而是“正常 uptime 断供 / 重启后破限刷”的双向 lifecycle 断裂。

## 证据定位

- `docs/finished_plans/plan-onboarding-loop-v1.md:101-114`：设计写明搜索完成后 3600 tick respawn，且每遗缴/每玩家/real-time 24h 只产出 3 次。
- `docs/finished_plans/plan-onboarding-loop-v1.md:620`：`ling_shui` 当前无其他获取路径，依赖 `surface_stash_craft`。
- `server/src/world/tsy_container_search.rs:168-209`：`SurfaceStashPlayerLimit` 是 `Resource`，内部只有 `HashMap` 与 `last_reset_wall_clock`。
- `server/src/world/tsy_container_search.rs:211-220`：注册只 `init_resource::<SurfaceStashPlayerLimit>()`，没有持久化 hydrate。
- `server/src/world/tsy_container_search.rs:377-388`：开搜只查内存 `stash_limit.can_search(...)`。
- `server/src/world/tsy_container_search.rs:589-599`：完成后 `container.depleted = true`，并只调用内存 `record_search(...)`。
- `server/src/world/poi_respawn_tick.rs:45-87`：`PoiRespawnStore` 同样是内存 `Resource`。
- `server/src/world/poi_respawn_tick.rs:107-115`：ready 后只日志 + `mark_refreshed`，没有恢复容器可搜状态。
- `server/src/world/tsy_container.rs:141-158`：`LootContainer::new` 固定 `depleted: false`。
- `server/src/world/poi_novice.rs:614-696`：Startup scatter 每次用固定 seed 重新生成 `LootContainer::new(ContainerKind::SurfaceStash, ...)`。
- `server/src/world/poi_novice_scatter_integration_test.rs:91-130`：现有回归只验证 `PoiRespawnStore` 能看到站点且 ready，未验证 ready 后实体真的重置/重建。

## 触发路径

1. 新号进入 spawn 附近，搜完一个或多个 `SurfaceStash`。
2. 搜索完成路径把对应 `LootContainer.depleted` 设为 `true`，玩家计数写入 `SurfaceStashPlayerLimit` 内存。
3. 等待 3600 tick，`PoiRespawnStore` 可判定 ready，但没有任何系统把该容器恢复为 `depleted=false` 或重建实体。
4. 重启 server。
5. Startup scatter 按固定 seed 再次生成同位置 `SurfaceStash`，`LootContainer::new` 令其未搜空；`SurfaceStashPlayerLimit` 也回到空计数。
6. 同一玩家再次搜同一批遗缴，绕过原本 24h/player 资源约束。

## 反方审查记录

Round 1：反方检查全仓 `SurfaceStashPlayerLimit` / `PoiRespawnStore` / `LootContainer`，未找到 SQLite/JSON/Redis 持久化或启动恢复；确认 `plan-surface-stash-runtime-scatter-gap-v1` 只覆盖零生成，不覆盖限频重启。

Round 2：反方建议主 bug 不写成单点“重启绕过限频”，而写成“SurfaceStash lifecycle 状态未落地”。理由是同进程内 respawn no-op 与重启后重建未搜空容器是同一状态边界缺口的两面，证据更强，也不与已有 PR 重复。

## Skeleton Fix Plan

- [ ] P0：为 `SurfaceStash` 建立权威 lifecycle 状态，至少包含 `poi_id`、`depleted`、`last_searched_tick` / `last_searched_wall`、per-player 24h 计数窗口。
- [ ] P1：把 `SurfaceStashPlayerLimit` 从纯内存 `Resource` 改为可启动 hydrate、完成搜索后落盘、失败不清 dirty 的持久状态。
- [ ] P2：让 3600 tick respawn 真正恢复可搜容器：选择重置现有实体 `depleted=false` 或按 `poi_id` 重建实体，但必须保持 registry/site/entity 一致。
- [ ] P3：Startup scatter 先读取已持久化 lifecycle；同一 `poi_id` 在 24h/player 窗口内不得因为重启变成可重复产出。
- [ ] P4：保留 `surface_stash_basic/scroll/craft` 的确定性分布，但把 loot roll 与 per-player 限频绑定到稳定 `poi_id`，避免 `family_id=spawn` 造成语义漂移。

## 验收测试计划

- [ ] server 单测：同一玩家对同一 `SurfaceStash` 搜 3 次后，第 4 次在 24h-1s 内拒绝；模拟重启 hydrate 后仍拒绝。
- [ ] server 单测：24h 到期后 hydrate 的持久化限频允许再次搜索。
- [ ] server 集成：搜空 `SurfaceStash` 后推进 `SURFACE_STASH_RESPAWN_TICKS`，断言对应容器实体重新可搜，而不是只在 `PoiRespawnStore` ready。
- [ ] server 集成：Startup scatter 遇到已持久化 depleted 状态时不生成未搜空副本；同一 `poi_id` 不出现双实体。
- [ ] e2e smoke：新号能搜到入门遗缴；用完当日次数后重启 server，不能立刻重复刷 `ling_shui` / 蓝图 / 残卷。

## 风险

- 需要决定权威 key：当前限频用 `container.family_id`，而 SurfaceStash 生成时传的是 `spawn`，这会把“每遗缴”语义压成“每 spawn 区”。修复时应改用稳定 `PoiNoviceSite.id` 或显式 `poi_id`。
- 持久化失败不能静默放开限频，否则会把数据库短暂错误变成资源复制入口；应采用保守拒绝或 dirty retry。
- 修复 respawn 时要避免生成重复 `LootContainer` 实体，必须维持 `PoiNoviceRegistry`、`PoiRespawnStore`、世界实体三者一致。
