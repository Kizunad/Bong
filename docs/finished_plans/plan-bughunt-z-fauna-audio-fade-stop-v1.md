# plan-bughunt-z-fauna-audio-fade-stop-v1

> 一句话主题：`mob/fauna/audio-client` 主路径确认 1 个高置信真 bug：**client `MinecraftSoundSink.stop()` 直接硬停 `SoundInstance`，把上层精心传下来的 `fadeOutTicks` 全部吞掉**。结果不是“音频按设定淡出”，而是 **Fuya 压迫 hum、环境 loop、音乐切换统一硬切**。
>
> 立项动机：这不是“体验可以更好”的 polish，而是**契约已设计、server/client 中层已完整传值、最终实现层失配**的真功能缺失。对实际游玩最直观的影响是：Fuya 死亡时压迫 hum 不会按 20 tick 收尾，而是瞬间断掉；离开环境特效半径、区域音乐切换也不会 crossfade，而是像开关一样被掐断。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | `fadeOutTicks` 在 `client/audio` 末端被吞，导致 fauna/环境/音乐 stop 全部硬切 | plan_skeleton | ✅ 2026-07-11 |

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

## Finish Evidence

### 第一性原理验真结论

**真 bug，确认成立。** 在 origin/main 上重新读代码复核 skeleton 的结论：`client/src/main/java/com/bong/client/audio/MinecraftSoundSink.java` 的 `stop(long instanceId, int fadeOutTicks)` 收到 `fadeOutTicks` 参数后确实完全丢弃，只做 `client.getSoundManager().stop(instance)`（`PositionedSoundInstance` 非 `TickableSoundInstance`，没有任何逐 tick 音量包络机制）。上游三条活跃路径确认依赖真实淡出语义：
- `MusicStateMachine.stopActive()`（区域音乐/combat/tribulation 切换）
- `EnvironmentAudioController.stopLoop()`（环境 loop 离场）
- `server/src/npc/tsy_hostile.rs`（Fuya 死亡显式发 `fade_out_ticks: 20`）

且反编译 Minecraft 1.20.1（`yarn 1.20.1+build.10`）`net/minecraft/client/sound/SoundSystem.class` 字节码独立确认：`SoundSystem.play(SoundInstance)` 对 `instanceof TickableSoundInstance` 的实例会注册进 `tickingSounds`；每个游戏 tick 的私有 `tick()` 都会对其调用 `tick()` → 检查 `isDone()` → 若未完成则重新读取 `getVolume()` 并把新音量推给底层 OpenAL source。这证明了修复路径（把淡出实现为 `TickableSoundInstance`）是 Minecraft 引擎原生支持、经过验证可行的机制，而非臆测。

### 落地清单

- `client/src/main/java/com/bong/client/audio/FadeableSoundInstance.java`（新增）—— `AbstractSoundInstance implements TickableSoundInstance`，`beginFadeOut(int ticks)` 状态机：`ticks<=0` 立即静音+`done`；`ticks>0` 逐 tick 线性衰减 `volume = baseVolume * (剩余/总数)`，衰减完毕 `volume=0` 且 `isDone()=true`，交给引擎自行摘除 channel。附 `volumeForTests()`/`pitchForTests()` 测试钩子（继承的 `getVolume()`/`getPitch()` 会乘上未解析的 `Sound` FloatSupplier，headless 单测下为 null 会 NPE）。
- `client/src/main/java/com/bong/client/audio/MinecraftSoundSink.java`（修改）—— `play()` 改为始终构造 `FadeableSoundInstance`（替代 `PositionedSoundInstance`），字段透传（位置/朝向/分类/pitch/relative/attenuationType）与旧实现完全等价；`stop()` 按 `fadeOutTicks` 分流：`<=0` 保留旧硬停语义（`soundManager.stop(instance)`），`>0` 改为 `instance.beginFadeOut(fadeOutTicks)`，不再立即调用 `soundManager.stop()`（避免打断刚起步的淡出）。
- `client/src/test/java/com/bong/client/audio/FadeableSoundInstanceTest.java`（新增）—— 9 个测试用例，饱和覆盖：未触发淡出的满音量常驻态、`0`/负数 ticks 的立即停音边界、4-tick 精确分数线性衰减（0.75/0.5/0.25/0.0）、非单位 baseVolume 折算、单 tick 边界、done 后 `tick()` 幂等、淡出中途重新触发的重新起算、构造参数透传契约（id/category/repeat/repeatDelay/attenuationType/x/y/z/relative）。

### 关键 commit

- `e386209b` (2026-07-11)：`docs(promote): plan-bughunt-z-fauna-audio-fade-stop-v1 骨架→active`
- `3d02604b` (2026-07-11)：`fix(audio): MinecraftSoundSink.stop() 落实 fadeOutTicks 淡出` —— 核心修复 + 新增测试

### 测试结果

- `cd client && ./gradlew test --tests "com.bong.client.audio.*"` → `BUILD SUCCESSFUL`，`FadeableSoundInstanceTest` 9 用例全绿（`tests=9 skipped=0 failures=0 errors=0`），`MusicStateMachineTest` 等既有 audio 包测试同步全绿。
- `cd client && ./gradlew test build`（全量客户端门禁）→ `BUILD SUCCESSFUL`。

### 对抗验证闭环

- 无上下文 read-only validator（Explore agent）针对 HEAD `3d02604b1dc662fa0959f853cbe0d7c0bee04e00` 独立复核：`git rev-parse HEAD` 对拍一致；独立反编译 `SoundSystem.class` 确认 `TickableSoundInstance` 机制真实存在（非臆测）；确认修复无 off-by-one/除零/音量越界/channel 泄漏；确认 `play()` 定位字段与旧 `PositionedSoundInstance` 语义完全等价；独立重跑测试套件确认真实通过（非陈旧缓存）；确认测试覆盖饱和无缺口；确认无第二处遗漏的 `PositionedSoundInstance` 构造点需要同步改造。结论：`PASS 3d02604b1dc662fa0959f853cbe0d7c0bee04e00`。

### 跨仓库核验

- **client**：`FadeableSoundInstance`（新增）、`MinecraftSoundSink.play/stop`（改造）——纯客户端音频渲染层修复，无 server/agent/schema 契约变更（`fadeOutTicks`/`fade_out_ticks` wire 字段本就已存在且双端已对齐，本次只修复 client 末端执行）。
- **server**：无改动，`server/src/npc/tsy_hostile.rs` 的 `StopSoundRecipeRequest { fade_out_ticks: 20 }` 发送逻辑本就正确，本次验证只确认了它现在能被 client 正确消费。
- **agent**：无涉及。

### 遗留 / 后续

- `client/src/main/java/com/bong/client/fauna/HallucinationTickController.java` 仍有一处直接 `client.getSoundManager().play(new PositionedSoundInstance(...))`（一次性播放，从不调用带 fadeOutTicks 的 stop），不在本 plan 修复范围内——不是同一 bug（该处从未接收/丢弃 fadeOutTicks，只是单纯没有淡出需求的一次性音效），如后续该处新增需要淡出的一次性音效需求，可复用本 plan 新增的 `FadeableSoundInstance`。
