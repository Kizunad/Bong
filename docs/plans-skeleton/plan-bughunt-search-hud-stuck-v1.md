# plan-bughunt-search-hud-stuck-v1（骨架）

> **骨架（草案）**。一句话主题：TSY 容器搜刮 HUD 的终态 flash（`COMPLETED_FLASH` / `ABORTED_FLASH`）承诺会自动回 `IDLE`，但 client runtime/store 里没有任何计时字段、tick 消费者或 disconnect 外的 reset 路径，导致提示会在**同一 session 内永久卡住**，直到下一次搜刮消息覆盖。

> 立项动机：本轮按「client runtime / store / session / consumer 漏 reset」角度复查 TSY 搜刮链路，避开既有 toast cross-session、identity panel stale session、zone_info 同区不刷新、灵龛 HUD 串局等已出题项后，确认这是一个**新的、同 session 即可稳定复现**的 HUD 状态机真 bug。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | TSY 搜刮 HUD 终态不自动回 `IDLE` | fix_pr | ⬜ |

## P0 — TSY 搜刮 HUD 终态不自动回 `IDLE`

- **类型**：client runtime / store / consumer 漏 reset
- **优先级**：major
- **复现路径**：
  1. 进入 TSY，对任一可搜刮容器触发 `search_started` / `search_progress`。
  2. 让搜刮完成，或主动取消/移动/受击，使 client 收到 `search_completed` 或 `search_aborted`。
  3. 不再进行第二次搜刮，继续在同一 session 内移动、战斗、开别的 UI。
  4. 观察屏幕底部中央 HUD：会一直停留在“搜刮完成：<容器>”或“搜刮中断：<原因>”，不会按注释所说在 3 秒/1 秒后消失。

### 根因链路

1. `client/src/main/java/com/bong/client/hud/SearchHudState.java` 的状态机注释明确写了：
   - `SEARCHING → COMPLETED_FLASH（收 SearchCompletedV1，3 秒后回 IDLE）`
   - `SEARCHING → ABORTED_FLASH（收 SearchAbortedV1，1 秒后回 IDLE）`
2. 但 `SearchHudState` record 本体只存 `phase/containerKindZh/requiredTicks/elapsedTicks/abortReason`，**没有任何** `updatedAtMs` / `phaseSinceMs` / `ttlMs` 字段，天然无法判断 flash 何时过期。
3. `client/src/main/java/com/bong/client/hud/SearchHudStateStore.java` 在 `markCompleted()` / `markAborted()` 里只把快照改成终态 flash；生产代码里唯一能把它改回 `IDLE` 的赋值只出现在 `resetForTests()`，没有运行时调用点。
4. 全仓没有任何 `ClientTickEvents`、HUD tick、planner 副作用或别的消费者去驱动 `COMPLETED_FLASH/ABORTED_FLASH -> IDLE` 转换。
5. `client/src/main/java/com/bong/client/hud/BongHudOrchestrator.java` 每帧都直接把 `SearchHudStateStore.snapshot()` 交给 `SearchProgressHudPlanner.buildCommands(...)`；planner 是纯渲染器，只会把当前 phase 画出来，不会回写 store。
6. 结果就是：只要 terminal payload 抵达一次，HUD 就会卡在终态，直到下一次搜刮消息覆盖，或整个 session 断线销毁 client 进程态。

### 影响面

- **直接体感**：玩家第一次搜刮完成/中断后，底部中央会长期残留错误提示，误导玩家以为自己仍处在某种搜刮结算态。
- **UI 污染**：该位置与其它环境/HUD 提示复用，长期残留会干扰后续观察。
- **链路语义错误**：server 已把搜刮流程结束了，client 却把终态 flash 错当成持久态，属于典型 consumer 漏收尾。
- **跨场景扩散**：哪怕玩家离开容器、转去战斗或撤离，只要没有下一次搜刮消息覆盖，旧 flash 仍继续显示。

### 修复建议

- 给 `SearchHudStateStore` 增加终态时间基（如 `phaseSinceMs` / `expiresAtMs`），并在 client tick 或 HUD 构建前做超时回收：
  - `COMPLETED_FLASH` 超过 3000ms 自动回 `IDLE`
  - `ABORTED_FLASH` 超过 1000ms 自动回 `IDLE`
- 或把 flash 改成和 `ExtractStateStore` 同类的带时间戳 snapshot，由 store/tick 层统一收口；不要让 `SearchProgressHudPlanner` 承担状态推进。
- 顺手补一个 disconnect clear 兜底，避免 terminal flash 在断线前最后一帧继续悬挂到下个连接周期。

### 验收抓手

- **单测**：
  - `SearchHudStateStore`/对应新 tick 逻辑：`markCompleted()` 后 2999ms 仍显示，3000ms 后自动 `IDLE`
  - `markAborted()` 后 999ms 仍显示，1000ms 后自动 `IDLE`
  - 新一轮 `search_started` 到来时可正常覆盖旧 flash，不受过期回收干扰
- **现有缺口补测**：
  - `client/src/test/java/com/bong/client/network/ContainerInteractionHandlerTest.java` 目前只断言能进入 `ABORTED_FLASH`，没断言它会回 `IDLE`
  - `client/src/test/java/com/bong/client/hud/SearchProgressHudPlannerTest.java` 只测渲染形状，没测生命周期收尾
- **手动**：
  1. 完成一次搜刮，停在原地 4 秒，HUD 应自动消失
  2. 触发一次取消/受击中断，停在原地 2 秒，HUD 应自动消失
  3. 完成/中断后立刻切战斗、开其它界面、原地移动，旧搜刮提示不应继续常驻

## 反方裁决（退化处理）

> 按要求应做两轮反方裁决；当前会话未启用 subagent，故退化为主线程显式记录两轮反方论点与驳回理由，并在后续 PR 原文如实披露。

### Round 1

- **反方论点**：也许仓库别处已经有隐藏 tick/timeout 会把 `SearchHudStateStore` 自动清空，只是 grep 没第一时间看出来。
- **驳回理由**：
  - 终态回 `IDLE` 需要运行时写回 `SearchHudState.idle()` 或等价 store 覆盖。
  - 生产代码里这类写回只出现在 `resetForTests()`。
  - `SearchProgressHudPlanner` 是纯函数，只读 state 出命令；`BongHudOrchestrator` 也是只读 store 后渲染。
  - 因此不存在“隐藏消费者已经收尾”的空间，这是真正的漏 reset，而不是代码搜索遗漏。

### Round 2

- **反方论点**：这也许不算 bug，因为下次搜刮 `search_started` 会覆盖旧状态；可以视作“直到下一次搜刮前保留最后结果”。
- **驳回理由**：
  - `SearchHudState` 注释自己定义的是 flash 语义，不是 sticky last-result 语义。
  - `COMPLETED_FLASH/ABORTED_FLASH` 的命名本身也表明这是短暂提示，不是常驻面板。
  - 玩家完全可能在很长时间内不再发起第二次搜刮；把“依赖下次交互覆盖”当成清理机制，会让单次搜刮永久污染本 session HUD。
  - 所以这不是产品选择，而是状态机终态缺收尾的实现 bug。

## 去重结论

- 已避开已知题：toast cross-session、false_skin_state 残留、identity panel stale session、zone_info 同区不刷新、灵龛守护 HUD/龛侵记录跨 session 串局、顿悟弹窗被本地切屏吞掉。
- 本题不是跨 session toast，也不是断线后旧 store 串局；它在**单次搜刮结束后的同一 session**就会稳定出现，属于另一类 consumer 漏 reset。

## 审计来源

bughunt 线程 CM（2026-07-05），限定 worktree `.worktree/bughunt-loop-20260705-cm`，角度：disconnect/reconnect、world 切换、增量状态不清、consumer 漏 reset。结论：TSY 搜刮 HUD 终态未自动收尾是高置信新真 bug；本 skeleton 仅立项，不含代码修复。
