# plan-bughunt-z-fauna-audio-fade-stop-v1（骨架）

> **骨架（草案）**。一句话主题：`mob/fauna/audio-client` 主路径确认 1 个高置信真 bug：**client `MinecraftSoundSink.stop()` 直接硬停 `SoundInstance`，把上层精心传下来的 `fadeOutTicks` 全部吞掉**。结果不是“音频按设定淡出”，而是 **Fuya 压迫 hum、环境 loop、音乐切换统一硬切**。

> 立项动机：这不是“体验可以更好”的 polish，而是**契约已设计、server/client 中层已完整传值、最终实现层失配**的真功能缺失。对实际游玩最直观的影响是：Fuya 死亡时压迫 hum 不会按 20 tick 收尾，而是瞬间断掉；离开环境特效半径、区域音乐切换也不会 crossfade，而是像开关一样被掐断。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | `fadeOutTicks` 在 `client/audio` 末端被吞，导致 fauna/环境/音乐 stop 全部硬切 | plan_skeleton | ⬜ |

## P0 — `fadeOutTicks` 在 `client/audio` 末端被吞

- **#1 major（plan_skeleton）**：`client/src/main/java/com/bong/client/audio/MinecraftSoundSink.java:59-69` 的 `stop(long instanceId, int fadeOutTicks)` 虽然接收了 `fadeOutTicks`，但实现只做 `client.getSoundManager().stop(instance)`，**没有任何 TickableSound / 音量包络 / 延迟摘除逻辑**，等价于“无条件立即停音”。这不是调用方没传，而是**最终执行层完全忽略该参数**。
- 上层契约明确存在，且已经被活跃路径使用：
  - `client/src/main/java/com/bong/client/audio/MusicStateMachine.java:37-49,83-89` 在环境音乐切换时先 `stopActive(update.fadeTicks())`，再启动新 loop，设计意图就是 **crossfade/至少 fade-out**，不是 snap cut。
  - `client/src/test/java/com/bong/client/audio/MusicStateMachineTest.java:23-45` 测试名就叫 `musicStateTransitionsCrossfadePreviousLoop`，并显式断言旧 loop 会收到 `stoppedFadeOutTicks == 60`。说明**产品语义和测试语义都认定这里应当淡出**，不是“立刻停也算对”。
  - `client/src/main/java/com/bong/client/environment/EnvironmentAudioController.java:65-78,85-94` 给本地环境 loop 记录 `effect.fadeOutTicks()`，停音时原样透传给 `SoundRecipePlayer.stop(...)`；这条链路同样期待“离开范围时逐步退场”。
  - `server/src/npc/tsy_hostile.rs:1123-1132` 在 Fuya 死亡时明确发送 `StopSoundRecipeRequest { fade_out_ticks: 20 }`。server 不是没给值，而是**client 到最后一跳把 20 tick 淡出硬切成 0 tick**。

## 这个 bug 对实际游玩体验的影响

- **Fuya / TSY 遭遇感受断裂**：Fuya 压迫 hum 本来是 loop 音墙，死亡时 server 明确要求 20 tick 淡出；现在客户端瞬停，玩家会听到“上一帧还在压迫、下一帧突然静音”的硬断。
- **区域音乐切换像切电源**：`MusicStateMachine` 设计的是先停旧 loop 再起新 loop，并带 `fadeTicks`；但 sink 末端不执行 fade，导致 combat / ambient / tribulation 等切换没有预期中的平滑收尾。
- **环境特效离场没有退场感**：本地 `EnvironmentAudioController` 给 emitter 维护了 fade-out tick，离开效果半径时仍被立即 stop，风声/静电脉冲/冷风 loop 都会“啪”一下断掉。

## 两轮反方裁决

### Round 1

- **反方质疑**：也许 `fadeOutTicks` 只是协议预留字段，当前版本就是允许 client 立即停音。
- **裁决**：否。`MusicStateMachineTest.musicStateTransitionsCrossfadePreviousLoop` 已把“crossfade previous loop”写进测试语义，并断言 stop 收到 `60`；`EnvironmentAudioController` 也持续保存并透传 `fadeOutTicks`。这不是死字段，而是**上层真实依赖的行为契约**。

### Round 2

- **反方质疑**：就算忽略 fade，也只是 ambience polish，不构成明确影响游玩体验的 bug。
- **裁决**：否。`server/src/npc/tsy_hostile.rs:1123-1132` 的 Fuya 死亡 stop 是**敌对 fauna 遭遇主路径**，不是边角美化；同一根因还会波及 combat/tribulation/环境 loop 切换。影响是**可直接听见的错误行为**，而不是只有代码洁癖才在乎的实现细节。

## 建议修复范围

- 优先收口 `client/src/main/java/com/bong/client/audio/MinecraftSoundSink.java`，把 `fadeOutTicks` 落成真实可执行的淡出 stop（例如自管 `TickableSoundInstance` / 包装层音量包络 / 延迟摘除）。
- 回归面至少覆盖：
  - `MusicStateMachine` loop 切换确实不是 snap cut。
  - `EnvironmentAudioController` 离场 stop 尊重 `fadeOutTicks`。
  - `Fuya` 压迫 hum 死亡 stop 尊重 server 下发的 `fade_out_ticks=20`。

## 审计来源

bughunt 线程 Z，范围优先扫 `server/src/fauna/`、`server/src/audio/`、`client/src/main/java/com/bong/client/audio/`、`client/src/main/java/com/bong/client/fauna/` 及直接相邻 network/state。候选先后排除了“枚举 wire 大小写映射错误”等伪阳性后，最终收敛到这条**server/client 中层都传值、末端实现单点吞参**的高置信主链问题。
