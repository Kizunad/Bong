# plan-bughunt-whale-debug-world-lifecycle-v1（骨架）

> **来源 issue**：#1802。client-only `/whale-debug` 的静态实体列表必须随世界/会话结束清理。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 世界卸载/断线时 discard 并清空 `SPAWNED` | ⬜ |

## §0 摘要

`WhaleDebugCommand.SPAWNED` 是 static `ArrayList`，只有 `/whale-debug clear` 会 discard 并清空。玩家换世界或断线后，旧 `ClientWorld` 的 `WhaleEntity` 仍被列表和 GeckoLib cache 强引用，下一次 spawn 继续累积。

## §1 游玩影响

这是 dev-only 客户端调试命令，但长时间调试或反复重连会保留旧世界实体引用，造成客户端内存占用和调试实体数量错误；不产生 server、agent 或 gameplay 状态。

## §2 复现路径

执行 `/whale-debug spawn`，断线或切换世界，再 spawn 多次；观察旧 `WhaleEntity` 仍在 static `SPAWNED`，直到手工执行 clear 或进程退出。

## §3 今天 `origin/main` 的证据

- `client/src/main/java/com/bong/client/whale/WhaleDebugCommand.java:35,55-89` static 列表只在 spawn 追加。
- `WhaleDebugCommand.java:92-104` 的清理只存在于 `executeClear`；没有 world unload/disconnect listener。
- `client/src/main/java/com/bong/client/BongClient.java:136` 注册该命令，说明它在客户端生产启动路径可触达，但命令本身不联 server。

## §4 非重复比对

已查 client lifecycle、MorphRenderProxy stale prune、whale debug 与 finished/active plans；现有断线 store 清理不覆盖该 static entity list。#1802 独立保留。

## §5 立项检查记录

- `docs/worldview.md`：查鲸类调试实体和世界切换关键词；该命令是调试工具，不扩展正典。
- `docs/finished_plans/`：查 client session lifecycle、world unload、GeckoLib entity cleanup。
- active plan：查 `WhaleDebugCommand`、`BongClient`、disconnect/world change hooks，未见列表清理。
- skeleton：查 `SPAWNED`、`executeClear`、`ClientWorld`、`clearOnDisconnect`；无同主题骨架。
- `docs/plans-skeleton/reminder.md`：仓内无该文件。

## §6 接入面与跨仓契约

- **Inputs**：client command spawn、当前 `ClientWorld` 生命周期和 disconnect/world change callback。
- **Outputs**：对旧 whale 调 `discard` 后清空列表；新世界只保留当前会话实体。
- **共享类型/事件**：复用 Fabric client lifecycle callback、`WhaleEntity`、`SPAWNED`/`executeClear`；不新增 server payload。
- **三端契约符号**：server **无变更**，命令不联 server；agent **无变更**，不经 Redis；client `WhaleDebugCommand` 与 `BongClient` lifecycle hook。
- **worldview/qi_physics**：调试实体不属于玩法规则；不涉及 `qi_physics` 或真元转移。

## P0 验收

- 断线、切世界和正常 clear 都 discard 一次并清空列表，不影响新世界 spawn。
- 重复 lifecycle 回调幂等，不对已 discard 实体重复加入或抛异常。
