# plan-bughunt-forge-lingtian-processing-deadpath-v1（骨架）

> 一句话主题：`plan-lingtian-process-v1` 已归档宣称 P2/P3 完成，但 `forge/lingtian/processing` 运行时主链仍停在**测试/死代码**：玩家侧没有启动入口，server 只会挂 `ProcessingSession` 并 tick 进度、不会结算产物/清 session/下发 `processing_session`，client `ProcessingActionScreen` 也无任何创建路径，导致晾晒/碾粉/炮制/萃取整条加工玩法在实际游玩中不可用。

> 立项动机：这不是“未来 enhancement”，而是**已归档功能半接线**。`docs/finished_plans/plan-lingtian-process-v1:388` 明写 P3 已接入 `ProcessingActionScreen` / `ProcessingServerDataHandler` / `ProcessingSessionDataV1.active` 清 session；实际代码里只有 schema、handler 和测试，运行时主链断在多处。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|---|---|---|---|
| P0 | 加工主链停在测试/死代码：无玩家启动、无完成结算、无 client 同步/UI 打开 | fix_pr | ⬜ |

## P0 — processing 主链停在测试/死代码

- **#1 major（fix_pr）**：`forge/lingtian/processing` 整条玩家链路当前不可用。
  - **启动端只存在测试调用**：`server/src/forge/processing_mode.rs:17-91` 定义并消费 `StartForgeProcessingRequest`，真正 runtime 只会在校验通过后 `insert(ProcessingSession::new(...))`。但全仓生产代码里只有 `server/src/forge/mod.rs:98-114` 注册 event/system，**唯一 `send_event(StartForgeProcessingRequest { ... })` 出现在测试** `server/src/forge/processing_mode.rs:137-156`。也就是说玩家没有任何 C2S / UI / world interaction 能把加工 session 启起来。
  - **session 只会 tick，不会结算**：`server/src/lingtian/mod.rs:176-203` 调度里只挂了 `processing::processing_session_tick_system`；`server/src/lingtian/processing.rs:440-503` 只提供 `tick_processing_session` 与纯函数 `complete_processing_session`。全仓对 `complete_processing_session(...)` 的引用只有 `server/src/lingtian/processing.rs` 测试（`771/795/819`），**没有任何生产系统在 `is_complete()` 后发放产物、移除/重置 session 或写回 inventory**。即便未来有人把 session 启起来，进度走满后也只会卡在 100%。
  - **client 同步/UI 也是死半边**：client 的消费面齐了，`client/src/main/java/com/bong/client/network/ServerDataRouter.java:191-193` 注册了 `processing_session` / `freshness_update`，`client/src/main/java/com/bong/client/network/processing/ProcessingServerDataHandler.java:11-39` 会写 `ProcessingSessionStore`，`client/src/main/java/com/bong/client/processing/ProcessingActionScreen.java:27-93` 会读 store 画“当前无加工/进度”。但 `client/src/main/java/com/bong/client/ui/UiOpenScreens.java:21-50` 只识别 `cultivation_panel + player_overview` 模板，**全仓没有任何 `new ProcessingActionScreen(...)` 或等价创建路径**；server 侧也没有 `processing_session` payload 生产者（`rg "processing_session"` 只命中 schema/handler/测试，不命中任何 emit）。结果是 client 端这套 UI/handler 目前完全吃不到真实运行时数据。
  - **与归档文档直接矛盾**：`docs/finished_plans/plan-lingtian-process-v1:31` 把 “client `ProcessingActionScreen` + HUD 进度 + schema `ProcessingSessionDataV1` 双端镜像” 列为 P3；`docs/finished_plans/plan-lingtian-process-v1:388` 的 Finish Evidence 更明确写“`ProcessingSessionDataV1.active` 可清空客户端 session；`ProcessingActionScreen`、`FreshnessTooltipHook`、`ProcessingServerDataHandler` 接入进度和 freshness UI”。现状说明 P3 只落了**类型和空壳消费端**，未落运行时生产链。

## 这个 bug 对实际游玩体验的影响

- 玩家目前**无法真正使用**晾晒 / 碾粉 / 炮制 / 萃取这条二级加工玩法；`plan-lingtian-process-v1` 名义上 finished，但实机里没有可触发的玩家链路。
- 即便后续别的切片补了一个最小启动入口，当前代码也会让加工 **卡死在“开始了但永远不结算”**：不吐产物、不清状态、不推 client 进度。
- client 已有的 `ProcessingActionScreen` / `processing_session` handler 看起来像“功能已做完”，实际上是**玩家永远看不到的死 UI**，这会误导后续计划和验收。

## 与现有 skeleton 去重边界

- **不等同于** `docs/plans-skeleton/plan-module-wiring-gaps-v2.md:T7`。T7 关注的是“世界站持久化 + 加工启动 intent 承载方式”的**设计决策**；本 bug 关注的是 `plan-lingtian-process-v1` 已归档后，**现有运行时主链仍停在测试/死代码**，即：
  - 没有任何生产调用方发 `StartForgeProcessingRequest`
  - 没有完成结算 system 消费 `ProcessingSession`
  - 没有 `processing_session` emit，也没有 `ProcessingActionScreen` 创建路径
- 即使 T7 未来拍板“用哪种交互方式启动加工”，上面这三段断链依然会让功能继续不可用，因此值得单独立 bug。

## 两轮反方裁决摘要

### Round 1：这是不是“故意留给后续切片”的未实现，而不是 bug？

- **反方观点**：`docs/finished_plans/plan-lingtian-process-v1:406` 明写“启动加工的 client intent 与 inventory/forge 交互动作留给后续切片”，所以没入口不算 bug。
- **裁决**：**不能据此免责**。文档只把“启动 intent”留后续；但同一份 finished plan 又在 `:31`、`:388` 明确声称 P3 的 `ProcessingActionScreen` / `ProcessingSessionDataV1.active` / `ProcessingServerDataHandler` 已接入“进度和 freshness UI”。现代码并非“只差按钮”，而是**连 session 完成结算、server emit、client screen 创建路径都没有**，超出了“留待后续切片”的范围。

### Round 2：这会不会只是 `plan-module-wiring-gaps-v2/T7` 已经覆盖的老题，不能再立？

- **反方观点**：T7 已经说了 “`lingtian::processing` 4 种 kind 生产端从未被玩家交互触发”，本题重复。
- **裁决**：**不重复**。T7 的核心是“触发方式/持久化表结构尚未决策”；本题额外证明了**即便绕过触发问题**，processing 运行时也没有完成结算和 client 同步，属于另一层更具体的 implementation gap。换言之，T7 解决后，本题仍会残留为真实 bug。

## 审计来源

- 代码路径：`server/src/forge/processing_mode.rs`、`server/src/lingtian/processing.rs`、`server/src/lingtian/mod.rs`、`client/src/main/java/com/bong/client/processing/ProcessingActionScreen.java`、`client/src/main/java/com/bong/client/network/processing/ProcessingServerDataHandler.java`、`client/src/main/java/com/bong/client/ui/UiOpenScreens.java`
- 文档对照：`docs/finished_plans/plan-lingtian-process-v1.md`、`docs/plans-skeleton/plan-module-wiring-gaps-v2.md`
