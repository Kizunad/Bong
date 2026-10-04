# plan-bughunt-zone-environment-audio-world-switch-v1

> 来源 Issue：#1918。普通世界切换错误复用断线清理，清空进程级环境音频标记后同一音乐键不会重新播放。
>
> 阶段总览：P0 ⬜ 修正 world/session 清理边界；P1 ⬜ 补跨维度回归与音频恢复验证。

## §0 摘要

EnvironmentEffectController 在发现 ClientWorld 对象变化时调用普通 clear。EnvironmentAudioController.clearLoops(false) 却无条件调用 EnvironmentAudioLoopState.clearOnDisconnect，清空进程级 ACTIVE_FLAGS。随后同一 TransitionKey 的 AmbientZoneUpdate 会被 MusicStateMachine.apply 视为重复而直接返回，音乐循环的 while flag 已不存在，导致切维度后环境音乐硬停且不恢复。

## §1 实际游玩体验影响

- 玩家从一个维度或世界进入另一个维度时，环境音循环可能停止，直到音乐键发生变化才再次出现。
- 同一个 zone、同一个配方在新世界仍是合法状态；玩家只能通过离开并重新触发另一首音乐来偶然恢复，表现为音频状态与环境状态脱节。

## §2 复现路径

1. 收到带 loop 配方的 zone_environment 状态，确认 MusicStateMachine 激活对应 while flag。
2. 让 client.world 替换为新的 ClientWorld，但保持下一次环境更新的 TransitionKey 不变。
3. tick 路径调用 EnvironmentEffectController.clear，观察 EnvironmentAudioLoopState 的 flag 被清空。
4. 再次调用 MusicStateMachine.apply；因为 key 未变而返回 false，SoundRecipePlayer 不会重新建立该 loop。

## §3 根因证据

- client/src/main/java/com/bong/client/environment/EnvironmentEffectController.java:63-66 在 ClientWorld 对象变化时调用 clear；:112-117 明确普通 world 清理与断线清理是两个入口。
- client/src/main/java/com/bong/client/environment/EnvironmentAudioController.java:66-79 的 clearLoops(false) 与 clearLoops(true) 共用实现，并在两条路径都调用 EnvironmentAudioLoopState.clearOnDisconnect。
- client/src/main/java/com/bong/client/environment/EnvironmentAudioLoopState.java:6-30 的 ACTIVE_FLAGS 是进程级集合，clearOnDisconnect 会直接清空它。
- client/src/main/java/com/bong/client/audio/MusicStateMachine.java:30-50 在 active.key 等于新键时直接返回 false，而 :40 的 loop flag 正是后续 SoundRecipePlayer 的播放门禁。
- server/src/network/zone_environment_bridge.rs:18-68 仅向同维度在线 client 发送 ZoneEnvironmentUpdate；server/src/schema/zone_environment.rs:20-27 定义 dimension、zone_id、effects、generation。当前 server 发送链没有断线语义。

## §4 非重复比对

- #1808 的 audio-one-shot-index-leak 处理一次性音效索引生命周期；本 issue 是 world/session 清理时误删 loop flag，两个根因和修复入口不同。
- 既有环境音频计划只覆盖 zone effect/recipe 接入，不覆盖 EnvironmentAudioLoopState 的 world 变更边界；若实施前发现新的 active plan 已声明同一清理入口，应并入而不是另起实现。

### 立项检查记录（2026-09-28）

- docs/worldview.md：检索“环境、区域、维度、声音、灵气、死域”，核对 §一 L18-L22 与 §二环境形态；没有要求普通换世界执行断线语义。
- docs/finished_plans/：检索 EnvironmentAudioController、EnvironmentAudioLoopState、zone_environment、MusicStateMachine；命中既有音频/环境接入文档，未发现本清理边界的已落地修复。
- active plan（docs/plan-*.md）：检索同上符号及 world switch；未发现声明 ACTIVE_FLAGS 所有权的 active plan。
- docs/plans-skeleton/：检索 zone_environment、ACTIVE_FLAGS、clearOnDisconnect、TransitionKey；未发现同主题待办骨架。
- docs/plans-skeleton/reminder.md：检索 EnvironmentAudioLoopState、world switch、音频 loop；仓内无该主题延后事项。

## 接入面与跨仓契约

- **Inputs**：server 的 ZoneEnvironmentRegistry dirty 状态、ZoneEnvironmentStateV1、bong:zone_environment payload、client 当前 ClientWorld 与 MusicStateMachine 状态。
- **Outputs**：client 的 EnvironmentEffectController、EnvironmentAudioController、EnvironmentAudioLoopState、MusicStateMachine 和 SoundRecipePlayer 重新建立正确的 session loop；server 发送格式不变。
- **共享类型 / 事件**：ZoneEnvironmentUpdate、ZoneEnvironmentStateV1、EnvironmentEffectV1、bong:zone_environment、AmbientZoneUpdate、AudioLoopConfig。
- **server 契约符号**：zone_environment_broadcast_system、ZoneEnvironmentStateV1::new_with_dimension、RedisOutbound::ZoneEnvironmentUpdate、bong:zone_environment。
- **agent**：无变更；agent 侧只接收 zone_environment_update 的既有 schema，清理边界发生在 client 音频运行态。
- **client**：有变更；EnvironmentEffectController.clear/clearOnDisconnect、EnvironmentAudioController.clearLoops、EnvironmentAudioLoopState.clearOnDisconnect、MusicStateMachine.apply。
- **worldview 锚点**：docs/worldview.md §一 L18-L22 的区域灵气和死域规则、§二环境形态；本 plan 只修表现层生命周期，不新增世界规则。
- **qi_physics**：无真元流动；不得在音频修复中修改 zone 灵气或引入任何 ledger 路径。

## §5 修复骨架

### P0 分离 world 清理与 session 清理

- 普通 clear 只能停止当前 world 的 emitter、音频实例、天空和雾状态；只有 clearOnDisconnect 才清空 EnvironmentAudioLoopState 的旧 session flag。
- world 替换后让下一条同键 AmbientZoneUpdate 具有重新应用的可观察路径（例如使 active key 失效或由 world generation 参与键），不得依赖“先来一首不同音乐”触发恢复。
- 保持 server 的同维过滤和 generation 语义，不把 client 的生命周期问题转嫁为广播所有维度。

### P1 回归与契约验收

- 同一 TransitionKey 在切换 ClientWorld 后仍能重新激活 loop flag 并播放一次。
- 断线后旧 flag、旧 pending layer 和 MusicStateMachine active 状态全部清空，新连接不会继承旧 session。
- zone_environment payload 的 dimension、generation、effects 仍按现有 Rust schema 和 agent schema 校验。
