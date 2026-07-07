# plan-bughunt-perception-edge-session-leak-v1

> BugHunt client-ui r09。仅新增 plan 文档，不修代码。主题：`PerceptionEdgeStateStore` 是 `spiritual_sense_targets` 的 client 本地静态快照，但断线 / 切服清理路径没有 reset；同一 Minecraft 进程进入新 session 后，旧神识 / 灵觉边缘 marker 与通灵+ 威胁边框可能继续渲染，直到新 session 发出下一份有效 `spiritual_sense_targets` 覆盖。

## Bug 摘要

`client/src/main/java/com/bong/client/visual/realm_vision/PerceptionEdgeStateStore.java:5-18` 用 static `AtomicReference<PerceptionEdgeState>` 保存上一份神识目标快照，生产写入口是 `SpiritualSenseTargetsHandler` 收到 `spiritual_sense_targets` 后整包 `replace`。但 `BongNetworkHandler.clearClientStateOnDisconnect()` 已清理大量跨 session store，却没有清 `PerceptionEdgeStateStore`。

这不是服务端状态串服，也不是权威数据污染；问题只在 client 本地 HUD snapshot 未随 session 生命周期 reset。若新服很快推送空或非空 `spiritual_sense_targets`，旧状态会自愈；若新服没有立即推空目标，旧感知残影会持续到下一次覆盖。

## 实际游玩体验影响

玩家从 A 服 / 旧存档断线后进入 B 服，屏幕边缘可能继续显示 A 服的神识 / 灵觉目标方向 marker。通灵及以上玩家的 `ThreatIndicator` 也可能继续按旧 `CRISIS_PREMONITION`、`HEAVENLY_GAZE`、`CULTIVATOR_REALM`、`ZHENFA_WARD_ALERT`、`NICHE_INTRUSION_TRACE` 条目闪边。

体感上，玩家会以为新服附近仍有敌意修士、阵法告警、灵龛入侵痕迹、天道凝视或其它可追踪目标，从而错误移动、规避或回头检查。若 B 服很快发新 `spiritual_sense_targets`，问题只是首包前窗口；若 B 服没有推空目标，则不是只闪一帧，而会保留到下一次感知目标同步。

## 证据定位

- `client/src/main/java/com/bong/client/visual/realm_vision/PerceptionEdgeStateStore.java:5-18`：静态 `STATE` 只有 `snapshot()` / `replace(...)`，没有生产 clear/reset API。
- `client/src/main/java/com/bong/client/network/SpiritualSenseTargetsHandler.java:21-31`：收到 `spiritual_sense_targets` 后写 `PerceptionEdgeStateStore.replace(next)`，并从同一快照派生垂死大能 `spiritEyeActive`。
- `client/src/main/java/com/bong/client/BongHud.java:228-254`：每帧从 `PerceptionEdgeStateStore.snapshot()` 计算边缘指示器。
- `client/src/main/java/com/bong/client/hud/BongHudOrchestrator.java:118-119`：HUD 编排直接读取 `PerceptionEdgeStateStore.snapshot()`。
- `client/src/main/java/com/bong/client/hud/BongHudOrchestrator.java:245-253`：同一快照进入 `ThreatIndicatorHudPlanner.buildCommands(...)`。
- `client/src/main/java/com/bong/client/hud/ThreatIndicatorHudPlanner.java:63-75`：威胁边框从 `PerceptionEdgeState.entries()` 聚合，命中高强度近距离条目会全屏闪边。
- `client/src/main/java/com/bong/client/BongNetworkHandler.java:857-900`：断线清理清单覆盖 NPC、TSY、coffin、gathering、visual、spider、daozhan、hallucination、dying elder、tiandao、agent ui、craft 等 store，但没有 `PerceptionEdgeStateStore`。
- `server/src/cultivation/spiritual_sense/push.rs:125-134`：存在 `push_empty_spiritual_sense_targets` helper。
- `server/src/cultivation/mod.rs:488-494`：生产调度挂的是 `push_spiritual_sense_targets`，不是空包 helper；因此不能假设新 session 一定立刻主动推空来清 client。

## 触发路径

1. 在 session A 中，服务端向 client 推送非空 `spiritual_sense_targets`，例如危机预感、天道凝视、修士境界、阵法告警、灵龛入侵痕迹、伪装蛛 / 道伥或垂死大能真元波动。
2. client 的 `SpiritualSenseTargetsHandler` 把这些目标写入 `PerceptionEdgeStateStore`。
3. HUD 每帧读取该 store，渲染屏幕边缘 marker；通灵+ 玩家还会看到 `ThreatIndicator` 边缘告警。
4. 玩家不断开 Minecraft 进程，直接断线、返回服务器列表或切到 session B。
5. disconnect cleanup 没有 reset `PerceptionEdgeStateStore`。
6. session B 首个有效 `spiritual_sense_targets` 覆盖前，或 B 服不推空目标时，HUD 仍按 session A 的感知条目渲染。

## 排重结论

- 不重复 `docs/plans-skeleton/plan-bughunt-hud-state-session-reset.md` / #993：该题限定 `BongHudStateStore` 的 zone / visualEffect snapshot，本题是独立 `PerceptionEdgeStateStore`。
- 不重复 #1049：#1049 是 `mineral_probe_result` 网络线程直触 HUD/SFX，本题是断线生命周期清理。
- 不重复 #1066、#1077、#1086、#1092：这些分别是锻造、灵田、炼丹、TSY store 断线残留，本题是神识感知 HUD store。
- 不重复 niche 相关 skeleton：`NICHE_INTRUSION_TRACE` 只是 `SenseKind` 的一个使用者；本题修通用 `spiritual_sense_targets` 快照，不处理 `NicheGuardianStore` 或 proto kind。
- 不重复 target-info 匿名名泄漏：target-info 关注准星目标信息与身份显示，本题关注神识边缘 marker / threat indicator 的本地快照。
- 不重复 qi radar mainpath / tide-sky omen：那些分别是雷达主路径或 omen 白名单问题；本题不恢复雷达、不新增 VFX、不改 omen。
- 不属于 combat A/V：不涉及技能动画、粒子、SFX、HUD 图标或招式差异化；落点是 Fabric client 非战斗 HUD / session hygiene。

## 对抗审查记录

第一轮 subagent 结论：通过候选。反方指出“server 可能很快推空全量包覆盖”，因此文案必须写成首个有效包前窗口，或新 session 未推空目标时持续；不能宣称必然永久。反方同时确认这不是 niche、target-info、tide-sky、qi radar、voidaction、灵宝、identity、toast、search 等已有主题。

第二轮 subagent 结论：通过，但按中等偏低严重度的 client session hygiene bug 写。反方确认 `push_empty_spiritual_sense_targets` helper 未挂入生产 schedule，`push_spiritual_sense_targets` 之外不能假设断线 / join 自动推空包；同时要求 PR body 明写实际游玩体验影响，并收窄为“同一 Minecraft 进程内断线 / 切服后，client 本地 perception HUD snapshot 未 reset”。

## Skeleton Fix Plan

1. 给 `PerceptionEdgeStateStore` 增加生产可用的 `clear()` / `resetOnDisconnect()`，内部写回 `PerceptionEdgeState.empty()`。
2. 在 `BongNetworkHandler.clearClientStateOnDisconnect()` 的统一清理清单中调用该 reset。
3. 同步清理由 `SpiritualSenseTargetsHandler` 派生出的 `DyingElderEncounterStore.setSpiritEyeActive(false)` 是否需要放进同一入口；若 `DyingElderEncounterStore.clearOnDisconnect()` 已覆盖，避免重复或反序。
4. 不在本修复中恢复 / 改造 `QiDensityRadarHudPlanner` 主路径，不改 `ThreatIndicatorHudPlanner` 视觉策略，不混入 niche / target-info / combat A/V。

## 验收测试计划

- client 单测：写入非空 `PerceptionEdgeStateStore` 后触发 disconnect reset，断言 `snapshot().isEmpty()`。
- HUD 单测：reset 前构造 `CRISIS_PREMONITION` / `NICHE_INTRUSION_TRACE` 条目，`ThreatIndicatorHudPlanner` 有输出；reset 后同一路径无旧威胁命令。
- 投影单测或轻量集成：reset 前 `BongHud.computeSpiritualSenseIndicators` 对非空条目产出 marker；reset 后返回空。
- 回归：正常收到新的非空 `spiritual_sense_targets` 后，边缘 marker 与 `ThreatIndicator` 仍按新 payload 渲染。
- 手测：A 服制造明显神识目标后断线切 B 服；B 服首屏不再显示 A 服 marker / 威胁闪边；B 服后续收到自身感知目标时正常显示。

## 风险

- 清理时机应只在 disconnect 触发，不要在 join 后异步清空，避免吞掉新 session 已先到达的 `spiritual_sense_targets`。
- 该 store 同时服务探索感知、灵龛痕迹、垂死大能 spirit-eye 派生和通灵+ 威胁边框；修复应保证只清跨 session 旧快照，不改变在线感知聚合语义。
- 如果后续决定让 server 在 join 时显式推空包，也仍建议保留 client disconnect reset；session 生命周期不应依赖服务端用空包兜底。
