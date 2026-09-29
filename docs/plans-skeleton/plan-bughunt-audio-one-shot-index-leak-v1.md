# plan-bughunt-audio-one-shot-index-leak-v1

> 来源 Issue：#1808。MinecraftSoundSink 把一次性音效放进 activeByInstance，但 FadeableSoundInstance 在未 fade 时永远 isDone=false，索引会随播放次数增长。
>
> 阶段总览：P0 ⬜ 分离 one-shot 与 loop 的生命周期；P1 ⬜ 验证延迟播放、停止和断线清理。

## §0 摘要

MinecraftSoundSink.play 为每个 recipe 创建 repeat=false 的 FadeableSoundInstance，却无条件加入 activeByInstance。FadeableSoundInstance.isDone 在 beginFadeOut 前恒为 false；MinecraftSoundSink.pruneFinished 只移除 isDone 条目。一次性 recipe 没有 server stop 事件时，实例完成播放也不会从索引中消失，长会话下 map 以 instanceId 无界增长。

## §1 实际游玩体验影响

- 连续脚步、命中和环境 one-shot 会在 client 内留下大量已经听完的实例索引，内存和遍历成本随游戏时间增长。
- 断线清理虽然最终 clear map，但不能解决单次连接长期累积；loop 的 stop 需求也不应被 one-shot 的索引泄漏绑架。

## §2 复现路径

1. server/audio_event_emit.rs:89-133 发送没有 loop 配置的 PlaySoundRecipePayload。
2. client MinecraftSoundSink.play 创建 repeat=false 实例，在 :72-74 放入 activeByInstance。
3. 音效自然播放完毕，没有 bong:audio/stop；FadeableSoundInstance:101-119 不会把自身标记为 done。
4. 后续 play 只在新播放前惰性 prune，旧 one-shot 仍保留，观察 trackedInstanceCountForTests 持续上升。

## §3 根因证据

- client/src/main/java/com/bong/client/audio/MinecraftSoundSink.java:35-75 无条件把每个实例加入 activeByInstance；:166-171 只移除 isDone 的实例。
- client/src/main/java/com/bong/client/audio/FadeableSoundInstance.java:22-24 说明未 beginFadeOut 前 isDone 恒为 false；:32-56 构造 repeat=false；:101-119 只有 done 才结束。
- client/src/main/java/com/bong/client/audio/SoundRecipePlayer.java:71-99 只为 loop recipe 建立 loops，one-shot 仍经 enqueue 进入 sink，stop 事件不是 one-shot 的自然完成通知。
- server/src/network/audio_event_emit.rs:73-133 发送 PlaySoundRecipePayload，:137-161 的 StopSoundRecipePayload 是独立的可选 stop 通道。
- server/src/schema/audio.rs:171-225 的 play/stop payload 只有 instance_id、recipe 和 fade 参数，没有 one-shot 完成回调；client BongNetworkHandler:626-667 将两个 channel 路由到主线程。

## §4 非重复比对

- #1918 是 environment loop flag 在 world 切换时被清空；本 plan 只处理 one-shot 索引，不改 loop flag 或 zone 切换。
- MinecraftSoundSink 的断线 hard-stop 与本泄漏互为边界清理，不能以“断线会 clear”代替正常播放完成后的生命周期。

### 立项检查记录（2026-09-28）

- docs/worldview.md：检索“声音、环境、脚步、战斗反馈、感知”，核对玩家感知/环境章节；没有要求把已结束 one-shot 保存在会话索引。
- docs/finished_plans/：检索 MinecraftSoundSink、FadeableSoundInstance、SoundRecipePlayer、audio/play；命中 audio 基础计划，未发现 one-shot index 的完成修复。
- active plan（docs/plan-*.md）：检索 activeByInstance、one-shot、audio stop；未发现 active plan 负责该 map 生命周期。
- docs/plans-skeleton/：检索 activeByInstance、FadeableSoundInstance、一次性音效；未发现同主题骨架。
- docs/plans-skeleton/reminder.md：检索 one-shot、音效索引、pruneFinished；仓内无该主题延后事项。

## 接入面与跨仓契约

- **Inputs**：PlaySoundRecipePayload、SoundRecipe.loop 配置、instance_id、delay_ticks，以及可选 StopSoundRecipePayload。
- **Outputs**：MinecraftSoundSink 的 activeByInstance 只保留仍可被 stop 的 loop/显式可取消实例；one-shot 播放仍由 SoundManager 完成。
- **共享类型 / 事件**：PlaySoundRecipeRequest、PlaySoundRecipePayload、StopSoundRecipeRequest、StopSoundRecipePayload、SoundRecipe.loop、bong:audio/play、bong:audio/stop。
- **server 契约符号**：emit_audio_play_payloads、emit_audio_stop_payloads、PlaySoundRecipePayload::validate、StopSoundRecipePayload::validate。
- **agent**：无变更；当前音频 custom payload 由 server Rust schema 直接发送给 client Java，不经过 agent TypeBox 音频事件。
- **client**：有变更；MinecraftSoundSink.play/pruneFinished、FadeableSoundInstance 的完成语义和 SoundRecipePlayer 的 loop 分流。
- **worldview 锚点**：docs/worldview.md 的环境/战斗感知章节；只修表现层资源生命周期，不新增音效规则。
- **qi_physics**：无真元路径；音效 payload 的 volume/pitch 不得被解释为 qi 数值。

## §5 修复骨架

### P0 让 one-shot 不进入永久 stop 索引

- 对 SoundRecipe.loop 为空的播放，直接交给 SoundManager，不把实例加入 activeByInstance；只有需要后续 stop/fade 的 loop 或明确可取消实例才进入索引。
- 若业务确实需要停止延迟中的 one-shot，新增有界的 pending/expiry 或完成回调，并在完成、取消、断线三条路径移除；不得继续依赖 isDone 的旧语义。
- 保持 delay_ticks 的取消行为和 loop 的 fade-out 行为，避免为修泄漏而让已排队的 loop 补响。

### P1 回归与验收

- 播放大量 one-shot 后 tracked instance 数量保持有界；loop 仍可按 instance_id stop/fade。
- 延迟 one-shot 被 stop 时不应到点补响；断线后旧实例和索引都清空。
- server payload 的 recipe、instance_id、volume/pitch 校验保持不变，agent 继续无变更。
