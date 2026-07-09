# BugHunt: TSY 撤离 HUD 断线残留

> Active plan。BugHunt worker：client-ui r08。范围限定 Fabric client 非战斗 UI / HUD / keybind / local session。本文只记录候选，不实际修代码。

## P0 — TSY 撤离状态断线不清，重连后沿用旧裂口与撤离忙态

- **问题定义（fix_pr）**：`client/src/main/java/com/bong/client/tsy/ExtractStateStore.java:17-19` 用静态 `portals`、`snapshot`、`collapseFlashTriggered` 保存 TSY 裂口、撤离进度、坍缩倒计时和屏幕闪烁；`ExtractStateStore.resetForTests()` 虽能清空它们，但生产断线清理 `client/src/main/java/com/bong/client/BongNetworkHandler.java:857-900` 没有调用。
- **触发路径**：
  1. 旧连接收到 `rift_portal_state` / `extract_started` / `extract_progress` / `tsy_collapse_started_ipc`，`client/src/main/java/com/bong/client/network/ExtractServerDataHandler.java:21-83` 写入 `ExtractStateStore`。
  2. 玩家断线、切服或回主菜单后，`clearClientStateOnDisconnect()` 清了 TSY boss / death vfx、craft、agent UI 等多类 store，但没有清 `ExtractStateStore`。
  3. 新连接首帧前，`BongHud` 继续 tick 旧 `ExtractStateStore`，`BongHudOrchestrator` / `ExtractProgressHudPlanner` 继续读取旧 snapshot。
  4. 玩家按 `Y` / `U` 时，`client/src/main/java/com/bong/client/tsy/ExtractInteractionBootstrap.java:37-45` 仍以旧 `extracting()` 和旧 `nearestPortal()` 判断是否发送 `start_extract` / `cancel_extract`。
- **实际游玩体验影响**：玩家重连或切到另一服后，可能继续看到上一局 TSY 的撤离进度、坍缩倒计时、裂口列表、屏幕闪烁和“撤离中断/完成”提示。更严重的是旧 `extracting=true` 会让 `Y` 启动撤离被本地吞掉，只允许 `U` 发取消；旧 portal 列表若与新世界玩家坐标接近，则 `Y` 可能把上一会话的旧 `entityId` 发给当前服务器，表现为无效撤离、错误拒绝提示或调试上难追的幽灵请求。

## 非重复性

- 不是 #1049 `mineral_probe_result` 网络线程直触 HUD/SFX。
- 不是 #1066 Forge、#1077 灵田、#1086 炼丹这三类静态 UI store 断线残留。
- 不是 #1032 / `plan-bughunt-tsy-search-extract-concurrent-busy-v1`：该题是“搜刮中可启动撤离”的忙态互斥缺口；本题是“断线/切服后撤离 store 未清”的跨 session 残留。
- 不是 #947 / `plan-bughunt-search-hud-stuck-v1` / TSY 容器搜刮 HUD：本题证据集中在 `ExtractStateStore`、撤离裂口、坍缩倒计时和 Y/U 撤离键，不消费 `TsyContainerStateStore` 或 `SearchHudStateStore`。
- 不是 #914 `TsyPresence` 重登丢失：那是 server/world transport presence；本题是 client 本地 UI store 与 keybind gate。
- 不是 #969 灵宝面板跨 session 残留；灵宝候选已排除。

## 建议修复范围

1. 在 `BongNetworkHandler.clearClientStateOnDisconnect()` 中调用 `ExtractStateStore.clearOnDisconnect()` 或生产语义命名的清理方法，避免复用 `resetForTests()` 的测试命名。
2. 清理内容必须包含 `portals`、`snapshot`、`collapseFlashTriggered`，并让首帧 HUD 回到 `ExtractState.empty()`。
3. 加 client 单测或断线清理 pin：模拟写入 portal + extracting + collapse 后调用断线清理，断言 `snapshot().portals()` 为空、`extracting()==false`、`collapseActive(now)==false`，并覆盖 `nearestPortal(player)==null`。
4. 回归 Y/U 键：断线清理后 `Y` 不应引用旧 portal entityId，`U` 不应因旧 `extracting=true` 发送无上下文 `cancel_extract`。

## 对抗审计结论

- Round 1 subagent：确认 `ExtractStateStore` 是高置信断线残留候选；同时提出 TSY 搜刮容器和灵宝面板候选。灵宝已被 #969 覆盖，搜刮容器与 #947/#951/#1032 重复风险高，均不采用。
- Round 2 subagent：专门攻击 TSY 撤离候选后裁决为成立。它确认 `BongHud.java` 每帧 tick 只会收尾 timed message / collapse / flash，不会清 portal 列表或 active extracting；`ExtractProgressHudPlanner` 仍会用旧 snapshot 渲染撤离进度、坍缩倒计时、红屏和裂口列表；`ExtractInteractionBootstrap` 的 Y/U 键仍会按旧 `nearestPortal()` / `extracting()` 派发请求。重复性裁决：不撞 #1032（搜刮与撤离并发）、#951（搜刮 HUD 终态）、#947（server 容器锁）、#914（server `TsyPresence` 重登丢失）；仅与历史“client session store 漏 reset”模式相同，题目和玩家体验面不同。
