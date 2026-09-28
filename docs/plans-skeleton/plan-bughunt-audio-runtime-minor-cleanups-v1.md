# plan-bughunt-audio-runtime-minor-cleanups-v1（骨架）

> **来源 issue**：#1744、#1758。音频运行时分别存在循环表重入修改和 1.20.1 不存在的原版 sound id。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 循环音频 tick 不发生迭代器失效，散灵珠使用 1.20.1 已注册音效 | ⬜ |

## §0 摘要

`SoundRecipePlayer.tick` 迭代 `loops` 时到期分支调用 `enqueue`，而高优先级 `enqueue` 对同一 map 执行 `removeIf`，下一次 iterator 访问可能抛 `ConcurrentModificationException`。`ScatterBurstPlayer.audioRecipe` 硬编码 `minecraft:entity.breeze.idle_air`，但 client 锁定 MC 1.20.1，该音效属于更高版本，生产路径会静默无声。

## §1 游玩影响

天劫/时代环境音在特定优先级交错时可能让客户端崩溃；散灵珠特效缺失应有音效，降低技能反馈但不影响 server 结算。

## §2 复现路径

1. 在 `loops` 中同时放入高优循环和低优同类别循环，令高优循环在 `tick()` 到期并触发 `enqueue`，观察 `removeIf` 修改外层迭代。
2. 触发 `bong:scatter_burst`，观察 `minecraft:entity.breeze.idle_air` 在 1.20.1 registry 中找不到而只打印警告。

## §3 今天 `origin/main` 的证据

- `client/src/main/java/com/bong/client/audio/SoundRecipePlayer.java:107-125` 的 iterator 循环在到期时调用 `enqueue`；`:194-207` 的 `enqueue` 对 `loops.entrySet()` 执行 `removeIf`。
- `client/src/main/java/com/bong/client/visual/particle/ScatterBurstPlayer.java:76-80` 构造 `minecraft:entity.breeze.idle_air`；`client/gradle.properties` 锁定 `minecraft_version=1.20.1`。
- server `server/src/network/audio_event_emit.rs` 与 client `AudioEventEnvelope`/`SoundRecipePlayer` 是既有音频 payload 链，错误音效不需要 agent 参与。

## §4 非重复比对

已查 audio recipe、environment audio、scatter burst 的 finished/active plans 和 skeleton；没有覆盖循环表重入和版本化 sound registry 的小型运行时清理。任务卡允许同模块零碎 minor 合并，但保留两个独立验收。

## §5 立项检查记录

- `docs/worldview.md`：查天劫氛围、时代环境和散灵珠表现关键词；音频修复不新增世界规则。
- `docs/finished_plans/`：查 audio recipe、era ambiance、scatter burst 计划。
- active plan：查 `SoundRecipePlayer.tick/enqueue`、`AudioEventEnvelope`、`ScatterBurstPlayer.audioRecipe`，未见同一修复。
- skeleton：查 `loops`、`removeIf`、`breeze.idle_air`、`minecraft_version`；无同主题骨架。
- `docs/plans-skeleton/reminder.md`：仓内无该文件。

## §6 接入面与跨仓契约

- **Inputs**：server `audio_event_emit` 的 recipe/priority/loop，client MC sound registry 与 tick。
- **Outputs**：安全的循环播放/停止，散灵珠选择一个 MC 1.20.1 已注册 sound id；不改变 payload 的 qi 或战斗结果。
- **共享类型/事件**：复用 `AudioEventPayload.PlaySoundRecipe`、`AudioRecipe`、`AudioLayer`、`SoundRecipePlayer`；sound id 必须符合 1.20.1 客户端 registry。
- **三端契约符号**：server `audio_event_emit` 和 `bong:audio/play` 保持字段语义；agent **无变更**，该音频通道不经 agent IPC；client `AudioEventEnvelope`、`SoundRecipePlayer`、`ScatterBurstPlayer`。
- **worldview/qi_physics**：只修视听表现；天劫/散灵珠的真元结算继续沿 server `qi_physics::ledger`，无新增 transfer。

## P0 验收

- `tick` 在同一 map 迭代期间不结构性修改外层 iterator，优先级淘汰在安全边界执行。
- 散灵珠在 1.20.1 registry 中播放可验证音效，未知 sound id 仍 fail closed，不使客户端崩溃。
