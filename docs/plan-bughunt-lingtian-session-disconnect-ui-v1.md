# BugHunt: 灵田会话 HUD 断线串场

## 摘要

`LingtianSessionStore` 是 client 进程级 `static volatile` snapshot，但断线清理路径没有把它重置。玩家在开垦、种植、收获、补灵、吸灵等灵田动作进行中断线、切服或返回标题后，新连接收到新的 `lingtian_session` 快照前，旧的 active snapshot 仍会被 HUD 与音频条件读取。

实际游玩体验影响：玩家会在新世界或重连后的短窗口继续看到上一局的灵田进度条、地块名和进度；如果上一局处于 `drain_qi`，已经启动的 `lingtian_drain_active` 循环音效也可能因为旧 flag 仍为 true 而不停止，造成“旧服吸灵声/进度条串到新服”的错觉。

## 证据

- `client/src/main/java/com/bong/client/lingtian/state/LingtianSessionStore.java:71`：`snapshot` 是 `static volatile`；`replace()` 只覆盖传入值，没有生产用 `clearOnDisconnect()`。
- `client/src/main/java/com/bong/client/BongNetworkHandler.java:132`：disconnect 统一调用 `clearClientStateOnDisconnect()`。
- `client/src/main/java/com/bong/client/BongNetworkHandler.java:857`：该清理函数清了 realm collapse、NPC、TSY、gathering、craft 等会话态 store，但没有清 `LingtianSessionStore`。
- `client/src/main/java/com/bong/client/BongNetworkHandler.java:135`：JOIN 只标记连接与设置本地 player id，没有主动清空或请求覆盖灵田 snapshot。
- `client/src/main/java/com/bong/client/BongHud.java:374` 与 `client/src/main/java/com/bong/client/hud/BongHudOrchestrator.java:453`：两条 HUD 渲染路径每帧直接读取 `LingtianSessionStore.snapshot()`。
- `client/src/main/java/com/bong/client/lingtian/LingtianSessionHud.java:35` 与 `client/src/main/java/com/bong/client/hud/LingtianOverlayHudPlanner.java:32`：只要 snapshot `active()` 为 true 就渲染。
- `client/src/main/java/com/bong/client/audio/SoundRecipePlayer.java:232`：`lingtian_drain_active` flag 直接读取同一个 snapshot，`active && kind == DRAIN_QI` 时返回 true。
- `docs/finished_plans/plan-lingtian-v1.md:34`：设计上 server 每帧推 active session，active=false 时隐藏 HUD；这说明断线/切服时 client 本地也应主动落到 empty 状态，而不能依赖旧连接后续包。
- `docs/finished_plans/plan-botany-visual-v1.md:44` 与 `:126`：`lingtian_drain` recipe 带 `lingtian_drain_active` loop，音频影响不是纯理论。

## 复现路径

1. 进入世界并开始一个灵田动作，例如开垦、收获或吸灵，确认屏幕中下方灵田进度 HUD 出现。
2. 在 session 仍 active 时断开连接，或直接切到另一个 server / 单机世界。
3. 在新连接首个 `lingtian_session` 覆盖包到达前观察 HUD；若旧 session 是 `drain_qi`，同时观察吸灵循环音效是否延续。

预期：断线时灵田 HUD 与相关 loop 条件立即变为 inactive。

实际：本地 `LingtianSessionStore` 没有被 disconnect 清理，旧 active snapshot 可继续被 HUD/音频读取。

## 修复建议

- 给 `LingtianSessionStore` 增加生产用 `clearOnDisconnect()` 或 `clear()`，内部写入 `Snapshot.empty()`。
- 在 `BongNetworkHandler.clearClientStateOnDisconnect()` 中调用该清理函数。
- 可选：在 JOIN 时也做一次幂等清理，防止某些连接生命周期边界没有触发 disconnect。

## 验收

- client 单测：先写入 active `HARVEST` / `DRAIN_QI` snapshot，调用断线清理后断言 `LingtianSessionStore.snapshot().active() == false`。
- HUD planner 单测：断线清理后 `LingtianOverlayHudPlanner.buildCommands(...)` 返回空列表，legacy `LingtianSessionHud` 不渲染 label。
- 音频条件单测：active `DRAIN_QI` 时 `lingtian_drain_active` 为 true；断线清理后为 false。
- 手工回归：灵田动作中断线/切服后，新连接首屏不显示旧地块进度，不继续旧吸灵 loop。

## 去重与对抗结论

- 已避开 #1049 `mineral_probe_result` 网络线程直触 HUD/SFX。
- 已避开 #1066 Forge 静态 store 断线 stale UI；本案对象是 `LingtianSessionStore`，玩家可见面是灵田 HUD 与吸灵 loop。
- 不重复 #1022 灵田 C2S 距离/维度门禁，也不重复 `docs/plans-skeleton/plan-bughunt-lingtian-c2s-range-gate-v1.md`。
- 第 1 轮对抗否定了较弱的 `ProcessingSessionStore` 候选，理由是加工浮窗生产入口不可达。
- 第 2 轮对抗支持本候选：HUD 证据硬，音频影响成立但依赖旧 `lingtian_drain` loop 已启动；分类为 Fabric client 非战斗 UI / local session hygiene，不是 combat A/V，也不是 server-only。
