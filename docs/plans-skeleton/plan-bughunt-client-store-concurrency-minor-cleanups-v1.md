# plan-bughunt-client-store-concurrency-minor-cleanups-v1（骨架）

> **来源 issue**：#1391、#1482、#1768。三处都是客户端 server snapshot 与本地更新并发时的 volatile 读改写覆盖。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | toast、skill bar、harvest session 的更新采用原子快照语义 | ⬜ |
| P1 | 网络替换与主线程更新交错回归 | ⬜ |

## §0 摘要

`BongToast.current`、`SkillBarStore.updateSlot` 和 `HarvestSessionStore.requestMode/interruptLocally` 都先读取旧快照，再把派生值写回。网络回调可以在这段间隙写入新的 server 状态，旧派生值随后覆盖新状态；toast 还会把刚显示的新提示清成 empty。

## §1 游玩影响

天道提示可能在显示前丢失，技能栏的绑定或冷却会回退，采集浮窗会把 server 的进度/完成态显示成旧值。问题只影响客户端镜像，不应改变 server 权威状态。

## §2 复现路径

1. 在 `current` 判定旧 toast 过期、`updateSlot` 或 `requestMode` 读出快照后，插入一条网络线程的 `show/replace`。
2. 让旧线程继续写回，观察新 toast 消失、skillbar 的 cooldown/slot 回退或 harvest 进度倒退。

## §3 今天 `origin/main` 的证据

- `client/src/main/java/com/bong/client/hud/BongToast.java:79-87` 读取 `activeToast` 后把过期结果无条件写成 `empty()`；`BongNetworkHandler.applyDispatch:1063-1070` 的 server-data 分发路径可直接调用 `BongToast.show`。
- `client/src/main/java/com/bong/client/combat/SkillBarStore.java:20-30` 的 `replace` 与 `updateSlot` 都写同一 volatile `snapshot`；`SkillBarConfigHandler:55` 可由网络回调替换完整 server 配置。
- `client/src/main/java/com/bong/client/botany/HarvestSessionStore.java:17-39` 的 `requestMode/interruptLocally` 基于局部 `current` 调 `replace`；`BotanyHarvestProgressHandler:42` 同时接收 server 进度并替换快照。

## §4 非重复比对

已查 `docs/finished_plans/`、active plan 与 skeleton 中的 `BongToast`、`SkillBarStore`、`HarvestSessionStore`、`replace`、`updateSlot`；生命周期清理已有计划，但没有覆盖这三类跨线程读改写。三条来源 issue 共享同一原子性缺口，保留各自输入输出验收。

## §5 立项检查记录

- `docs/worldview.md §四 L213-L382`：查战斗反馈、技能栏和真元表现；不改 server 资源规则。
- `docs/finished_plans/`：查 client store lifecycle、combat HUD、botany harvest；确认已有断线清理不等于并发合并。
- active plan：查 `BongToast.current/show`、`SkillBarStore.replace/updateSlot`、`HarvestSessionStore.replace/requestMode`，未见同一并发修法。
- skeleton：查 `volatile snapshot`、`compareAndSet`、`CopyOnWriteArrayList` 和三份来源 issue；无同主题骨架。
- `docs/plans-skeleton/reminder.md`：仓内无该文件。

## §6 接入面与跨仓契约

- **Inputs**：server-data 的 narration/alert toast、`skillbar_config`、`botany_harvest_progress`，以及主线程本地 slot/mode 操作。
- **Outputs**：单一客户端快照和 HUD；并发冲突时保留较新的 server 字段，不伪造回执。
- **共享类型/事件**：复用 `ServerDataEnvelope`、`SkillBarConfigV1`、`BotanyHarvestProgress`、各 store 的 immutable view model；不新增 wire 字段。
- **三端契约符号**：server `skillbar_config_emit::emit_skillbar_config_payloads`、`botany::emit_botany_harvest_progress` 和既有 server-data；agent **无变更**，这些状态不依赖 agent 推演；client `BongToast`、`SkillBarStore`、`HarvestSessionStore` 负责原子收敛。
- **worldview/qi_physics**：客户端仅镜像 `docs/worldview.md` §四的战斗反馈与 §七的采集表现；真元仍由 server 的 `qi_physics` ledger 权威维护，本骨架不新增 transfer。

## P0/P1 验收

- 旧线程不得覆盖在其读取之后到达的新 server snapshot。
- toast 过期清理只清理仍是同一快照的状态；skillbar 与 harvest 的 server 字段在并发交错后仍可见。
- 断线清理和 listener 通知语义保持不变。
