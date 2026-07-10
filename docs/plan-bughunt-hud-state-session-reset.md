# plan-bughunt-hud-state-session-reset

> Skeleton Plan（BugHunt C4 client-ui 第四轮）。仅记录高置信 bug 与修复骨架；本轮不消费、不归档、不修改实际代码。

## Bug 摘要

`BongHudStateStore` 是生产 HUD 管线使用的 static snapshot，但断线/切服清理清单没有把它 reset 到 `BongHudStateSnapshot.empty()`。上一服写入的 `zoneState` 与 `visualEffectState` 会跨 session 残留：新服首个 `zone_info` 到达前，HUD/atmosphere 仍按旧区域渲染；旧 visual effect 在剩余 TTL 内继续影响 HUD tint、相机偏移或 FOV。

本 plan 不纳入 `BongToast.activeToast`、legacy `EventAlertState`、legacy `com.bong.client.ZoneState`。这些是独立状态或已有 PR 范围；本 bug 只针对生产 `com.bong.client.hud.BongHudStateStore`。

## 实际游玩体验影响

玩家从旧服/旧存档断线后马上进新服时，屏幕左上区域 HUD 可能短暂显示上一服区域（例如血谷、负灵域、死域），并用旧区域驱动环境氛围、雾/粒子变体。若断线前触发过 `blood_moon`、`meditation_ink_wash`、`near_death_vignette`、FOV/相机类 visual effect，新服开局仍可能带着旧 tint、边缘暗角、镜头晃动或 FOV 偏移，直到旧 effect 自然过期或新服发出新的 effect。

这不是永久 zone 串服：正常 server 首个 zone snapshot 会覆盖旧 `zoneState`。但首包前的错误 HUD/环境反馈已经足够误导玩家，尤其在进入危险区、死亡恢复、切服测试时会把上一局状态带进下一局第一眼体验。

## 证据定位

- `client/src/main/java/com/bong/client/hud/BongHudStateStore.java:4`：`snapshot` 是 static volatile，默认 `BongHudStateSnapshot.empty()`。
- `client/src/main/java/com/bong/client/BongHud.java:90`：生产 HUD 每帧直接读取 `BongHudStateStore.snapshot()` 并传给 `BongHudOrchestrator.buildCommands`。
- `client/src/main/java/com/bong/client/hud/BongHudOrchestrator.java:127`：`safeSnapshot.zoneState()` 进入 `ZoneHudRenderer.append`。
- `client/src/main/java/com/bong/client/hud/BongZoneHud.java:63`：区域持久 overlay 文案由 `ZoneState` 生成。
- `client/src/main/java/com/bong/client/atmosphere/ZoneAtmosphereRenderer.java:49`：atmosphere 也从 `BongHudStateStore.snapshot().zoneState()` 规划环境效果。
- `client/src/main/java/com/bong/client/hud/BongHudOrchestrator.java:171`：`safeSnapshot.visualEffectState()` 进入 `VisualHudRenderer`。
- `client/src/main/java/com/bong/client/mixin/MixinCamera.java:55`、`client/src/main/java/com/bong/client/mixin/MixinGameRenderer.java:40`：相机旋转/位移与 FOV mixin 读取同一个 visual effect snapshot。
- `client/src/main/java/com/bong/client/BongNetworkHandler.java:131`-`173`：disconnect callback 清理多种 client store，但没有清 `BongHudStateStore`。
- `client/src/main/java/com/bong/client/BongNetworkHandler.java:873`-`895`：zone/visual effect 写入时复用当前 snapshot 的其它字段，说明旧字段会继续参与组合。
- `client/src/main/java/com/bong/client/network/ServerDataDispatch.java:281`：empty visual effect 会被 sanitize 成 null；`none` payload 不能作为清空旧 effect 的可靠通道。

## 触发路径

1. 在旧 session A 中进入特征明显的区域，例如血谷、负灵域、死域，客户端收到 `zone_info`，`BongHudStateStore.zoneState` 被写入。
2. 同一 session 触发一个持续数秒到数十秒的 visual effect，例如冥想水墨、血月、濒死暗角、FOV/相机类效果。
3. 不退出 Minecraft 进程，直接断线、返回服务器列表或切到新 server/session B。
4. disconnect 清理没有 reset `BongHudStateStore`。
5. session B 首帧到首个 `zone_info` 之前，HUD 和 atmosphere 仍读取旧 zone；若旧 visual effect TTL 未过，HUD tint、相机、FOV 继续按旧 effect 生效。
6. 新 `zone_info` 到达后 zone 多数自愈；旧 visual effect 若新服不发新 effect，只能等 TTL 自然结束。

## 反方审查记录

第一轮反方结论：候选成立，但必须收窄。`zoneState` 与 `visualEffectState` 的跨 session 残留证据强；`narrationState` 不应作为主张，因为可见 toast 走 `BongToast.activeToast`，且已有 #922 覆盖。

第一轮反方提出的限制：
- 全量重启客户端进程不复现，因为 static 会重置。
- 正常 server 首次 track 玩家会发 `zone_info`，所以 zone 残留应写成“新 `zone_info` 首包前短窗口”，不是永久错误。
- visual effect 有自然过期，但剩余 TTL 内仍会污染新 session。

第二轮反方结论：通过，按中低严重度 session hygiene bug 写。反方确认 `ServerDataDispatch.sanitizeVisualEffectState` 会丢弃 empty effect，`applyDispatch` 只在 `visualEffectState().ifPresent` 时替换，因此 server 不发 effect 或发 `none` 都不会自然清旧 visual effect。

排重结论：
- 不重复 #969：该 PR 是灵宝面板 state/dialogue 跨 session。
- 不重复 #976：该 PR 是暗器 HUD `lastTick` 跨 session。
- 不重复 #984：该 PR 是 dropped loot store 断线残留。
- 不重复 #917：#917 是 `zone_info` 同区运行态变化不刷新；本 bug 是 client disconnect 不清生产 HUD snapshot。
- 不重复 #922：#922 是 `BongToast.activeToast` 跨 session；本 bug 不纳入 toast。

## Skeleton Fix Plan

1. 在 `BongHudStateStore` 增加显式 reset/clear API，或在 disconnect callback 直接调用 `BongHudStateStore.replace(BongHudStateSnapshot.empty())`。
2. 在 `BongNetworkHandler` 的 `ClientPlayConnectionEvents.DISCONNECT` 清理清单中补上生产 HUD snapshot reset。
3. 验证 reset 后 `zoneState == ZoneState.empty()`、`visualEffectState == VisualEffectState.none()`，且不影响正常 `replaceZoneState` / `replaceVisualEffectState` 写入。
4. 不在本修复中处理 `BongToast.activeToast`、legacy `EventAlertState`、legacy `com.bong.client.ZoneState`，避免和已有/独立问题混杂。

## 验收测试计划

- 单测：构造非空 `BongHudStateSnapshot`，模拟 disconnect reset 后断言 `BongHudStateStore.snapshot().isEmpty()`。
- 单测：reset 前给 `zoneState` 写入血谷/负灵域，reset 后 `BongZoneHud.buildCommands` 对空 zone 返回空命令，不再显示旧区域 overlay。
- 单测：reset 前写入 active `VisualEffectState`，reset 后 `VisualEffectPlanner.buildCommands`、`CameraShakeOffsets`、`CameraFovOffset` 不再产生旧 effect 输出。
- 集成/手测：旧服触发明显 zone + visual effect，断线切新服，首帧不显示旧区域，不出现旧 tint/暗角/相机/FOV 残留；新服首个 `zone_info` 到达后正常显示新区域。
- 回归：正常在线收到 `zone_info`、visual effect payload 后 HUD/atmosphere/visual 仍能按新 payload 更新。

## 风险

- reset 放在 disconnect 时机过早/过晚都应只影响离开 session 后的本地 UI，不应吞掉当前 session 正常 payload。
- `BongHudStateSnapshot.empty()` 会同时清 zone/narration/visualEffect；当前主修复只依赖 zone/visualEffect，但需确认 narration 字段清空不会影响非 toast 的后续逻辑。
- 如果未来有非网络本地 visual effect 依赖跨世界保留，应明确改为独立 store；生产 HUD session snapshot 不应跨 server 保留。
