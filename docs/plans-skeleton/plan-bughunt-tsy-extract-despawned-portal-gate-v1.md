# plan-bughunt-tsy-extract-despawned-portal-gate-v1

> Skeleton plan。只读审计产物，来源 issue #1384。

## §0 摘要

TSY 撤离进度系统用 `Query<&RiftPortal>` 查询 portal，并以 `portals.get(progress.portal).is_err()` 作为存在性检查；但 `RiftPortal` 被标记 `Despawned` 后、Valence 真正移除实体前仍然匹配该 query。塌缩完成或 portal 过期的窗口内，玩家仍可能通过检查并完成撤离。

## §1 游玩影响

玩家在裂口已经被判定失效、秘境已经塌缩的时刻仍可完成撤离，绕过 race-out 的风险和“死坍缩渊无安全点”规则。该路径会让同一个 portal 的撤离结果取决于 deferred removal 的时序，重现时表现为有时能逃、有时被中断。

## §2 复现路径

1. 创建 `RiftPortal` 与玩家 `ExtractProgress`，让玩家处于撤离倒计时。
2. 运行 `on_tsy_collapse_completed` 或 `despawn_expired_portals`，portal 被 `commands.entity(...).insert(Despawned)`。
3. 在组件实际移除前运行 `tick_extract_progress`。
4. `Query<&RiftPortal>` 仍能 `get(progress.portal)`，倒计时继续；若达到 required ticks，发送 `ExtractCompleted`。

## §3 今天 `origin/main` 证据

- `server/src/world/extract_system.rs:181-186`：`start_extract_request` 的 portal query 没有 `Without<Despawned>`。
- `server/src/world/extract_system.rs:312-324`：`tick_extract_progress` 同样使用不带生命周期过滤的 `Query<&RiftPortal>`。
- `server/src/world/extract_system.rs:343-350`：只检查 `portals.get(progress.portal).is_err()`，没有判断 `Despawned`。
- `server/src/world/extract_system.rs:748-761`：塌缩完成会对匹配的 portal 插入 `Despawned`；`:784-795` 的过期清理也如此。
- 同文件 `despawn_expired_portals` 的 `Without<Despawned>` query 证明 `Despawned` 是正式的软删除门，而不是仅测试标记。

## §4 非重复比对

- `docs/finished_plans/plan-tsy-extract-v1.md` 已覆盖撤离组件、计时、race-out 和塌缩清理，但没有把实际运行 query 的 `Without<Despawned>` 门列为验收条件。
- `docs/plan-bughunt-tsy-extract-disconnect-stale-v1.md` 处理断线后的玩家 `ExtractProgress`，不处理 portal 生命周期。
- `plan-bughunt-tsy-search-extract-concurrent-busy-v1.md` 处理并发占用，不处理已软删除 portal 仍可 `get`。

## §5 立项检查记录

- **worldview**：查 `撤离点`、`塌缩`、`死坍缩渊`、`race-out`；命中 `docs/worldview.md §十六.一/§十六.内部法则`，结论是塌缩后必须关闭出口。
- **finished_plans**：查 `RiftPortal`、`tick_extract_progress`、`Despawned`；完成 plan 只描述功能链路，未覆盖软删除过滤。
- **active plan**：查 `extract progress`、`portal expired`、`Despawned`；未发现同一 query gate 的 active plan。
- **skeleton**：查 `Without<Despawned>`、`portals.get(progress.portal)`、`collapse completed`；未找到针对该门的骨架。
- **reminder.md**：查 `extract`、`portal`、`Despawned`、`撤离`；仓内无对应条目。

## §6 接入面与跨仓契约

- **Inputs**：`StartExtractRequest`、`ExtractProgress.portal`、`RiftPortal.activation_window`、`TsyCollapseCompleted`、`CombatClock`。
- **Outputs**：`ExtractAborted` / `ExtractFailed` / `ExtractCompleted`、玩家 `ExtractProgress` 的移除，以及既有 `extract-v1` 进度 payload。
- **共享类型或事件**：复用 `RiftPortal`、`ExtractProgress`、`Despawned`、`ExtractAbortReason::PortalExpired`、`ExtractCompleted`；不新增 portal 状态类型。
- **server 符号**：`start_extract_request`、`tick_extract_progress`、`on_tsy_collapse_completed`、`despawn_expired_portals`。
- **agent**：无变更；撤离状态不进入 Redis agent 命令或世界推演 schema。
- **client**：无变更；client 继续消费既有 `extract-v1`/HUD，portal 是否有效由 server authoritative gate 决定，wire 结构不变。
- **worldview 锚点**：`docs/worldview.md §十六.一` 的活/死坍缩渊生命周期与 §十六.四 race-out。

## 阶段总览

| 阶段 | 状态 | 交付物 |
| --- | --- | --- |
| P0 | ⬜ | 所有会开始或推进撤离的 portal query 统一排除 `Despawned` |
| P1 | ⬜ | 塌缩/过期/实际移除前后的回归测试与事件幂等检查 |

## P0：生命周期门

- `start_extract_request`、`tick_extract_progress` 以及任何读取 `progress.portal` 的路径统一使用 `Without<Despawned>` 或显式组件检查。
- portal 被标记 `Despawned` 时，已有进度发送一次 `ExtractAborted { reason: PortalExpired }`，不得继续完成；不改变 Valence 的 deferred removal 语义。

## P1：验收与幂等

- 覆盖塌缩完成、activation window 到期、同 tick 标记与推进、portal 已真实移除四种顺序。
- 断言同一进度不会同时发 `ExtractCompleted` 与 `ExtractAborted`，并保持既有 `extract-v1` payload。

## 验收测试计划

- `Despawned` portal 在 `tick_extract_progress` 中立即触发 `PortalExpired` abort。
- 未标记的 portal 仍按原计时完成，过期清理不会误伤其他 family。
- 同 tick 的 collapse event 与 extract request 组合不产生可逃逸的完成事件。

## 来源 issue

- #1384 `[flash-review][major] 已 Despawned 的 RiftPortal 仍可通过 portals.get 校验，撤离在坍缩后继续`
