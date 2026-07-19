# plan-bughunt-search-hud-stuck-v1

> **Finished plan（2026-07-15 review 返工验收归档）**。来源：`docs/plans-skeleton/plan-bughunt-search-hud-stuck-v1.md`。一句话主题：TSY 容器搜刮 HUD 的终态 flash（`COMPLETED_FLASH` / `ABORTED_FLASH`）承诺会自动回 `IDLE`，但 client runtime/store 里没有任何计时字段、tick 消费者或 disconnect 外的 reset 路径，导致提示会在**同一 session 内永久卡住**，直到下一次搜刮消息覆盖。
>
> 立项动机：本轮按「client runtime / store / session / consumer 漏 reset」角度复查 TSY 搜刮链路，避开既有 toast cross-session、identity panel stale session、zone_info 同区不刷新、灵龛 HUD 串局等已出题项后，确认这是一个**新的、同 session 即可稳定复现**的 HUD 状态机真 bug。

## 接入面

- **进料**：复用 `ContainerInteractionHandler` 已接收的 `search_started` / `search_progress` / `search_completed` / `search_aborted` payload，不新增协议。
- **出料**：`SearchHudStateStore.snapshot()` 继续供 `BongHudOrchestrator` 与 `SearchCancelInteractionBootstrap` 读取；过期后统一返回 `SearchHudState.idle()`，由既有 `SearchProgressHudPlanner` 自然停止渲染。
- **共享类型 / event**：只扩展既有 `SearchHudStateStore` 生命周期，不复制 `SearchHudState`、不新增 parallel store。
- **跨仓库契约**：纯 client 生命周期修复；server payload 名称和 agent/schema 均不变。
- **worldview 锚点**：**不适用**。审计对照 `worldview.md §二 L30-L56` 的灵压环境规则；本 plan 只收尾既有搜刮 HUD 快照，不改变灵压、境界、经济、叙事或任何正典玩法语义。
- **qi_physics 锚点**：**不适用**。审计对照 `docs/finished_plans/plan-qi-physics-v1.md §接入面 Checklist L28-L35` 与 `server/src/qi_physics/ledger.rs`；本 plan 不读取或写入真元/灵气数值，不产生 `QiTransfer`，也不新增物理公式或常数。

## §1 开放问题（P0 决策门前需收口）

- **开放问题：无。**
- 本 bugfix 的实现选择已全部在 §1.1 收口；实施与返工均以 §1.1 决议为准。

## §1.1 决议（pre-P0 收口，2026-07-15）

1. 计时所有权留在 `SearchHudStateStore`，不让纯渲染器 `SearchProgressHudPlanner` 推进状态。
   - **双锚**：`client/src/main/java/com/bong/client/hud/SearchHudStateStore.java:13-21`、`client/src/main/java/com/bong/client/hud/SearchProgressHudPlanner.java:17-18`；plan P0「实施范围」。
2. 生产时间基使用 `System.nanoTime()`；store 记录终态进入时刻，`snapshot()` 读取时惰性回收。生产 orchestrator 公开路径每帧只采样一次纳秒时间并转入 package-private `nowNanos` seam；该 seam 同时供确定性契约测试注入固定时刻。
   - **双锚**：`client/src/main/java/com/bong/client/hud/BongHudOrchestrator.java:99-135,494-498`、`client/src/main/java/com/bong/client/hud/SearchHudStateStore.java:13-21`；plan P0「实施范围」与「验收抓手」。
3. `COMPLETED_FLASH` TTL 固定为 3 秒，`ABORTED_FLASH` TTL 固定为 1 秒；精确边界采用“到期时刻即 `IDLE`”。新 `markStarted` / `markProgress` / 新终态会覆盖旧终态计时。
   - **双锚**：`client/src/main/java/com/bong/client/hud/SearchHudStateStore.java:4-5,24-54,69-85`；plan P0「实施范围」与「验收抓手」。
4. 增加生产态 `clearOnDisconnect()`，并接入 `BongNetworkHandler.clearClientStateOnDisconnect()`，避免终态或进行中搜刮状态跨 session 残留。
   - **双锚**：`client/src/main/java/com/bong/client/hud/SearchHudStateStore.java:56-67`、`client/src/main/java/com/bong/client/BongNetworkHandler.java:859-895`；plan P0「实施范围」。
5. 测试通过 package-private 显式纳秒时间入口驱动，不 sleep、不依赖真实墙钟；覆盖最终 `SEARCH_PROGRESS` 命令流的 TTL-1ns、精确边界与 TTL+1ns，以及时钟回拨、新搜索覆盖、终态替换、断线清理和既有 reason/safe-kind 分支。
   - **双锚**：`client/src/test/java/com/bong/client/hud/BongHudOrchestratorTest.java:334-402`、`client/src/test/java/com/bong/client/hud/SearchHudStateStoreTest.java:31-266`；plan P0「验收抓手」。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | TSY 搜刮 HUD 终态不自动回 `IDLE` | client | ✅ 2026-07-15 |

## P0 — TSY 搜刮 HUD 终态不自动回 `IDLE`（✅ 2026-07-15）

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

### 实施范围

- 给 `SearchHudStateStore` 增加基于单调纳秒时钟的终态时间基，并在 `snapshot()` 读取前做超时回收：
  - `COMPLETED_FLASH` 超过 3000ms 自动回 `IDLE`
  - `ABORTED_FLASH` 超过 1000ms 自动回 `IDLE`
- `SearchProgressHudPlanner` 保持纯函数，不承担状态推进。
- `BongNetworkHandler.clearClientStateOnDisconnect()` 调用 `SearchHudStateStore.clearOnDisconnect()`，进行中与终态状态都立即清空。

#### 玩家可感知行为规格（本阶段内联）

- **粒子**：不适用；既有搜刮 HUD 不发 `bong:vfx_event`，本修复不新增、替换或延长任何粒子。
- **音效**：不适用；既有终态 flash 没有 audio recipe，本修复不新增 SFX，也不改变服务端 payload 的声音语义。
- **HUD**：保持既有 `SearchProgressHudPlanner` 外观与位置；`COMPLETED_FLASH` 继续使用底部中央 140×4 满格条、`0xFFFFD060` 文字/填充并显示 3 秒，`ABORTED_FLASH` 继续使用 `0xFFE05030` 中断文字并显示 1 秒。仅修正到期收尾；`FULL` 保留未过期命令，其它可见模式继续按既有 layer 策略过滤。
- **环境**：不适用；不改变天空色温、雾、方块、terrain profile 或任何世界状态。
- **动画**：不适用；不触发或修改 PlayerAnimator 动画，容器与玩家姿态保持既有行为。
- **旁白**：不适用；不新增 narration 模板、scope 或 style，现有 `search_*` payload 与中文 HUD 标签保持不变。

### 验收抓手

- **store 状态机契约**：
  - `markCompleted()` 后 2999ms 仍显示，3000ms 精确边界自动 `IDLE`。
  - `markAborted()` 后 999ms 仍显示，1000ms 精确边界自动 `IDLE`。
  - 新一轮 `search_started` / `search_progress`、后到终态、断线、负向时间差与 `nanoTime` 回绕均有确定性测试。
- **生产 HUD 帧消费集成门禁**：
  - `BongHudOrchestratorTest` 必须经 package-private `nowNanos` seam 注入固定 `startedAtNanos`，驱动真实 `SearchHudStateStore.snapshotAtNanos(nowNanos)` 构建最终命令流；completed/aborted 覆盖 TTL-1ns 仍可见、TTL 精确边界消失、TTL+1ns 保持消失，不 sleep、不依赖真实墙钟。
  - 必须以 active combat snapshot 与变化后的 `HudRuntimeContext` 坐标/朝向复验，证明进入战斗和移动后，过期终态不会重新进入最终命令流。
- **界面切换集成门禁**：
  - `BongHudTest` 必须走生产 `ScreenHudVisibility` 过滤策略；`FULL` 直接复用原命令列表并保留尚未过期的搜刮 flash，`INVENTORY_DIMMED`、`CAST_BAR_ONLY`、`AGENT_UI_ONLY`、`HIDDEN` 均不得泄漏 `SEARCH_PROGRESS`。
  - main 侧 `AgentUiScreen` 生产渲染链测试与上述 search flash 不泄漏契约并存；从其它界面返回 `FULL` 后，再由上一条生产帧消费测试证明过期 flash 不会复现。
- **当前环境的验收约定**：
  - 当前 SSH 环境无 `DISPLAY`、Wayland socket 或 Xvfb，不能诚实执行玩家可见 `./gradlew runClient`，也不得把未执行的手工观察写成证据。
  - 本 plan 将上述“store 契约 + 生产 orchestrator 最终命令流 + 生产 screen visibility 过滤”确定性自动化测试定义为归档门禁；本地 WSLg 的 `runClient` 仅作为可选视觉 smoke，不再是本次 headless 流水线的阻塞条件。

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

bughunt 线程 CM（2026-07-05），限定 worktree `.worktree/bughunt-loop-20260705-cm`，角度：disconnect/reconnect、world 切换、增量状态不清、consumer 漏 reset。结论：TSY 搜刮 HUD 终态未自动收尾是高置信新真 bug；原 skeleton 当时仅立项，不含代码修复。

## Finish Evidence

### 落地清单

- `client/src/main/java/com/bong/client/hud/SearchHudStateStore.java`
  - `COMPLETED_FLASH` 使用 3 秒单调时间 TTL，`ABORTED_FLASH` 使用 1 秒 TTL；`snapshotAtNanos(nowNanos)` 在精确边界惰性回收为 `IDLE`。
  - `markStarted` / `markProgress` / 新终态覆盖旧计时；状态和 `System.nanoTime()` 取时受同一 class monitor 保护。
  - `clearOnDisconnect()` 同时清理进行中与终态搜刮状态。
- `client/src/main/java/com/bong/client/BongNetworkHandler.java`
  - `clearClientStateOnDisconnect()` 已接入 `SearchHudStateStore.clearOnDisconnect()`，不再让旧 session HUD 串入重连。
- `client/src/main/java/com/bong/client/hud/BongHudOrchestrator.java`
  - 保留公开默认 `buildCommands(...)`（内部 `System.nanoTime()`）与 package-private `nowNanos` overload；Search flash 经 `snapshotAtNanos(nowNanos)` 进入最终命令流。
- `client/src/main/java/com/bong/client/BongHud.java`
  - 生产 `render` 走 orchestrator 公开默认路径；该路径每帧只采样一次 `System.nanoTime()` 并传入 package-private `nowNanos` seam。
  - `filterCommandsForVisibility`：`FULL` 直接返回原 commands（不做 `List.copyOf`）；完整保留 main 的 `AGENT_UI_ONLY` / inventory dim / cast-only / hidden 策略。
- `client/src/test/java/com/bong/client/hud/SearchHudStateStoreTest.java`
  - 覆盖 3 秒/1 秒前一纳秒与精确边界、新搜索/进度覆盖、后到终态替换、负向时间差、`nanoTime` long 回绕及断线清理。
- `client/src/test/java/com/bong/client/hud/BongHudOrchestratorTest.java`
  - completed/aborted 最终 HUD command flow 使用固定 `startedAtNanos`，覆盖 TTL-1ns / TTL / TTL+1ns；不 sleep、不依赖真实墙钟。
- `client/src/test/java/com/bong/client/BongHudTest.java`
  - 保留 main 新增 `AgentUiScreen` 生产渲染链测试。
  - 补回 search flash 不泄漏到 inventory / cast / hidden / agent-ui 界面，且 `FULL` 直接复用原命令列表。
- `client/src/test/java/com/bong/client/BongNetworkHandlerTest.java`
  - 同时保留 main 的 PlayerState/Season reset 与本 PR 的 SearchHud disconnect reset。

### 关键 commit

- `651d3cbc`（2026-07-15）：升格 plan 并收口单调时间、惰性过期与断线清理方案。
- `182ebd41`（2026-07-15）：加入修复前红灯契约；32 个定向测试中 5 个按预期失败。
- `b72a702f`（2026-07-15）：实现终态 TTL 与统一断线清理。
- `330e2afe`（2026-07-15）：把生产取时纳入状态锁，消除锁竞争导致的 TTL 起点偏移。
- `1694589f`（2026-07-15）：运行 `scripts/plan-finish.sh`，把 active plan 迁入 `docs/finished_plans/`。
- `15eef3b8`（2026-07-15）：响应首轮 review，将 plan 恢复为 active，撤回未完成的运行时验收声明。
- `b6e28ffe`（2026-07-15）：补齐 orchestrator 最终命令流与生产 screen visibility 集成回归。
- `e99cba9e`（2026-07-15）：将 headless 环境下的生产链确定性集成测试收口为等价验收门禁。
- `112c753c`（2026-07-16）：返工锁定终态边界契约（固定 `startedAtNanos` + `nowNanos` seam，不再依赖真实墙钟）。
- merge `origin/main`（2026-07-19）：重放确定性终态门禁到 main 最新 `BongHud` / `BongHudTest` / `BongNetworkHandlerTest` 形状，保留 Agent UI screen gate 与 Season state。

### 测试结果

- 修复前证真：JDK 17 定向执行 `SearchHudStateStoreTest` + `BongNetworkHandlerTest`，`32 tests completed, 5 failed`；失败精确命中 completed/aborted TTL、后到终态 deadline、`nanoTime` 回绕和断线清理。
- 修复后定向：同两组测试 `BUILD SUCCESSFUL`，上述失败全部转绿。
- review 返工定向门禁（JDK 17）：`SearchHudStateStoreTest` + `BongHudOrchestratorTest` + `BongHudTest` → `33 tests, 0 skipped, 0 failures, 0 errors`；覆盖 store → orchestrator 固定 `nowNanos` 最终命令流与 screen visibility 过滤。
- 主线 merge 后客户端完整门禁（JDK 17）：`cd client && ./gradlew test build`；以本次 merge commit 门禁结果为准（不宣称真实墙钟或 runClient 观察）。
- 主 agent 对抗复审：首次复审发现“锁外取 `nanoTime`”竞态并以 `330e2afe` 修正；`112c753c` 将最终命令流 TTL 边界改为固定纳秒注入。按用户明确要求，本次未启动独立 validator subagent。

### 跨仓库核验

- **client**：`ContainerInteractionHandler` 四类 search payload → `SearchHudStateStore` → `BongHudOrchestrator` / `SearchProgressHudPlanner` → `BongHud.filterCommandsForVisibility` 链路保持原协议并补齐生命周期收尾、确定性 TTL 边界与界面切换回归；与 main Agent UI screen gate / Season state 共存。
- **server**：未改；`search_started` / `search_progress` / `search_completed` / `search_aborted` payload 契约不变。
- **agent/schema**：未改；本修复不新增或变更跨仓库 schema。

### 遗留 / 后续

- 无阻塞项，无协议迁移、真元守恒或视觉资产遗留。
- 当前 SSH 环境无图形显示能力，未执行也未宣称执行 `runClient`；归档依据由 store 精确边界、固定 `nowNanos` 的生产 orchestrator 最终命令流、以及 production screen visibility（含 `AGENT_UI_ONLY`）三层确定性自动化证据共同提供。本地 WSLg 视觉 smoke 为可选后续，不构成本次归档条件。
