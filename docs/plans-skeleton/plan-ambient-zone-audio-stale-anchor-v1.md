# plan-ambient-zone-audio-stale-anchor-v1（骨架）

> **骨架（草案）**。一句话主题：`audio/ambient_zone` 主链把大多数区域环境音当 `zone_broadcast` 3D 声源来播，但服务端只在 zone/state 变化时推一次 `pos`，客户端 loop 之后一直复用首次 payload；同时 client 的 `TransitionKey` 也不含 `pos`，导致“只换坐标、不换 zone/state”的更新即使补发也会被判成 `noChange`。影响是：**玩家在同一区域内移动时，环境音会持续钉在旧坐标，出现声源方向错误、音量衰减异常，走远后甚至整段 ambience 直接听不到，直到切区/进战斗/入夜等状态变化才自愈**。

> 立项动机：这条缺口位于 `server/src/audio/ambient.rs` ↔ `client/.../MusicStateMachine.java` 的正式主链，不是 dev-only 边角料；而且影响对象正是玩家长时间暴露的环境氛围层。当前实现已经把 spawn/wilderness 等主环境音 recipe 配成 `zone_broadcast`，所以“坐标不刷新”会直接进入实际游玩体感。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | ambient_zone 环境音锚点陈旧 / 移动后跑偏或静音 | fix_pr | ⬜ |

## P0 — ambient_zone 环境音锚点陈旧 / 移动后跑偏或静音

- **现象**：`server/src/audio/ambient.rs:138-146` 的 `AudioWorldKey` 只包含 `zone_name / recipe_id / music_state / is_night / season / tsy_depth`，不包含玩家当前位置；`ambient_zone_change_system` 在 key 未变时直接 `continue`（`:253-264`），因此同一区域内纯移动不会重新发 `bong:audio/ambient_zone`。可真正下发给客户端的 payload 却把 `pos` 固定成“本次发包时的玩家坐标”（`:272-285`，尤其 `pos: Some(block_pos(position.get()))` 在 `:281`）。
- **客户端为何会把旧坐标永久复用**：`client/src/main/java/com/bong/client/audio/MusicStateMachine.java:30-50` 首次收到包后会把 `update.pos()` 塞进 `PlaySoundRecipe`；但它的 `TransitionKey` 只比较 `zoneName / recipeId / state / night / season / tsyDepth / volumeMul / pitchShift / recipe`，不含 `pos`（`:214-237`）。所以即便后续 server 想补发“同 zone、同 recipe、只是位置变了”的包，`apply()` 也会在 `:32-35` 直接判成 `noChange`。更进一步，`client/src/main/java/com/bong/client/audio/SoundRecipePlayer.java:72-125` 会把首个 loop payload 存进 `ActiveLoop.payload`，后续每轮重播都复用这个旧 payload（`:118-123`），不会用玩家当前坐标重算。
- **为什么这会真实影响玩家而不是只影响日志**：`server/assets/audio/recipes/ambient_spawn_plain.json:7-10` 与 `ambient_wilderness.json:6-9` 都把主环境音配成 `attenuation: "zone_broadcast"`；`client/src/main/java/com/bong/client/audio/MinecraftSoundSink.java:24-28` 会把这类非 `PLAYER_LOCAL/SELF` 声音走 `LINEAR` 衰减，且使用 payload 自带 `pos`（`:29-55`）。也就是说，这些 ambience 不是“贴耳本地音”，而是**真 3D 声源**。当玩家在同一大区内横向移动几十格后，环境音仍从旧位置发声，会出现方向感错误、音量越来越小；离旧锚点超过半径后甚至整段 ambience 消失。
- **可达链路**：该链路由 `server/src/audio/register` 正式挂入主应用，`ambient_zone_change_system` 面向所有 `With<Client>` 的在线玩家执行（`server/src/audio/ambient.rs:169-203`）。默认主环境 recipe 覆盖 `spawn`、`wilderness`、`qingyun_peaks`、`north_wastes` 等常走区域；这不是 debug 指令触发，也不是特殊 boss 单次场景，而是**玩家平时走图、采集、赶路就会连续暴露**的 ambience 主层。
- **对实际游玩体验的影响**：玩家从出生区往外跑、在大平原/荒野横向移动、或在同一 zone 内绕路探索时，环境音不会“跟着所处环境更新”，而是继续从旧坐标发出来。体感上会表现成：刚进区时 ambience 正常，走一段后声音像被甩在身后，方向感错乱；再走远一点，整个环境氛围层突然变得异常安静，直到切区、入夜、进战斗或入 TSY 才重新锚定。这会直接削弱环境压迫感和空间感，属于玩家能稳定感知的 AV 退化。
- **建议修复范围 / 模块**：至少联动 `server/src/audio/ambient.rs` 与 `client/src/main/java/com/bong/client/audio/MusicStateMachine.java` / `SoundRecipePlayer.java`。修复方向需要一次性定清：要么把位置变化纳入 server/client 的切换 key 并允许位置刷新触发重锚；要么在不重启 loop 的情况下给 active ambient loop 单独更新位置。无论选哪条，**都不能继续依赖“首次 payload 的静态 pos”**。
- **验收抓手**：至少补 4 组 pin。1) 同 zone 内移动但 zone/state 不变时，应存在可观察的位置刷新，不再只首包定锚。 2) client 收到“仅 pos 变化”的 ambient 更新时不能再返回 `noChange`。 3) loop 重播时应使用新锚点，而非历史 payload 的陈旧 `pos`。 4) 端到端模拟中，玩家在 `spawn` 或 `wilderness` 内移动超过 64 格时，ambience 不应因旧锚点衰减而异常静音。

## 反方裁决摘要

1. **Round 1**：反方怀疑“环境音也许本来就是本地 ambience，不需要随人移动”。复核后被代码反证：主环境 recipe 明确是 `zone_broadcast`，client 也把它走 `LINEAR` 3D 衰减而非 `PLAYER_LOCAL`，所以“位置无关”不成立。
2. **Round 2**：反方怀疑“即便 server 当前不重发，client 端也许会在 loop 时自动取玩家当前位置，或未来只补 server 就够”。复核后再次被反证：`SoundRecipePlayer` 会缓存首个 `PlaySoundRecipe payload` 做 loop 重播，`MinecraftSoundSink` 也优先使用 payload 自带 `pos`；同时 `MusicStateMachine.TransitionKey` 不含 `pos`，纯位置更新会被直接吞掉，因此不存在隐式自愈。
3. 两轮证伪后剩下的是同一条稳定结论：**这是 server 去重键、client 过渡键与 loop payload 复用三处共同造成的真 bug**，且玩家正常走图即可触发。

## 开放问题

1. 位置刷新应走“重新 apply ambient update 并平滑 crossfade”，还是给现有 active loop 增加原地更新锚点的能力？修复 PR 需要在 AV 平滑性与实现复杂度之间做一次明确取舍。
2. `ambient_tsy` 目前是 `player_local`，不受本缺口影响；修复时需要避免把 TSY 这种本地氛围音一并改成不必要的高频重锚。

## 审计来源

bug-hunt 定点轮（仅收窄 `server/src/audio/` 与 `client/src/main/java/com/bong/client/audio/` 为主，辅以 recipe 资源核对）。候选先经主代理链路复核，再做两轮默认怀疑式证伪；本题在“是否真 3D 声源”“是否已有位置刷新/自愈”两轮怀疑下都存活，结论为 **report-only**：先提交 skeleton plan 固化玩家影响、根因路径与修复面，再由后续 fix PR 单独落地。
