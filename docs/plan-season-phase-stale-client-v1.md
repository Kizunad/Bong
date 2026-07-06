# plan-season-phase-stale-client-v1

> 一句话主题：服务端的 `WorldSeasonState` 会按时推进并在跨相位时发出 `SeasonChangedEvent`，但客户端 `SeasonStateStore` 只依赖 `player_state` 增量包更新；生产环境里的 `emit_player_state_payloads` 又只在 `PlayerState/Cultivation/social` 这些组件发生变更时才发包，`SeasonChangedEvent` 本身只被转发到 Redis/world_state、没有任何面向客户端的同步路径。结果是：**玩家一旦处于“满 qi、满心境、原地等待跨季”这类静止状态，client 的 season/hud/atmosphere 会长期卡在旧相位，直到下一次无关的 player_state 变化才突然跳变。**

> 立项动机：这不是纯视觉小瑕疵，而是季节主路径上的状态断链。`plan-season-full-experience-v1` 把季节提示、粒子、突破叠层、音乐/atmosphere 联动都收敛到了 `SeasonStateStore` 上；一旦 store 卡旧值，玩家会直接被错误的季节反馈误导，尤其是在等汐转做突破、看灵田/灵草季节提示、靠 HUD 判断天地节律时最明显。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 季节跨相位时客户端 season state 陈旧 | fix_pr | ⬜ |

## P0 — 季节跨相位时客户端 season state 陈旧

- **现象**：`server/src/world/season/mod.rs:236-252` 的 `season_tick` 每 tick 推进 `WorldSeasonState`，跨相位时会真实 emit `SeasonChangedEvent`；但 `server/src/network/mod.rs:1297-1313` 的 `publish_season_changed_events` 只把这个事件送去 `RedisOutbound::SeasonChanged`。客户端这边，`client/src/main/java/com/bong/client/BongNetworkHandler.java:815-818` 明确只在收到 `ServerDataDispatch.seasonState()` 时才 `SeasonStateStore::replace`。也就是说，**服务端季节时钟和客户端季节 store 之间没有独立的“季节变了就同步”桥。**
- **断链点**：生产环境注册的 `emit_player_state_payloads` 虽然会把 season 一起塞进 `player_state`（`server/src/network/mod.rs:2054-2108`），但它的 Query filter 只订了 `Added/Changed<PlayerState>`、`Added/Changed<Cultivation>`、以及少数 social 组件（`2036-2052`）；并没有订 `Changed<WorldSeasonState>` / `SeasonChangedEvent`，也没有周期性 cadence。`server/src/network/mod.rs:381-383` 的系统注册同样只把这一个变更驱动版 emitter 挂进生产 Update 链。
- **为什么不是“反正会很快收到别的包”**：玩家只要处于“不触发这些 Changed 条件”的合法状态，这个陈旧就会无限延长。`server/src/cultivation/tick.rs:187-190,238-255,272-273` 说明 qi regen 只有 `qi_room > 0` 且 `gain > 0` 才会改 `cultivation.qi_current`；满 qi 玩家不会因此触发 `Changed<Cultivation>`。`server/src/cultivation/composure.rs:10-15` 说明 composure 只有 `< 1.0` 才回升；满心境玩家同样不会被动变更。于是“满 qi、满心境、站桩等跨季”就是一条正常可达、且足以让 client season 永久卡旧值的主路径。
- **仓库内自证这本来就该有周期/边界同步**：`server/src/network/mod.rs:5002-5063` 已经有一个只存在于测试模块里的 `emit_player_state_payloads_periodically_without_change` seam，而 `5320-5357` 还专门写了 `player_state_periodic_emission_happens_without_component_change` 测试去 pin“无变更时也应周期发 player_state”。这说明作者自己已经承认仅靠 Changed 触发不够；问题在于这条 periodic cadence 只活在 test seam，没接进生产系统。
- **对实际游玩体验的影响**：客户端所有季节体验主路径都会读旧值。`client/src/main/java/com/bong/client/season/SeasonVisualController.java:31-43,55-65` 会把 `SeasonStateStore.snapshot()` 驱动到 `ZoneAtmosphereRenderer`、`MusicStateMachine` 和季节粒子；`client/src/main/java/com/bong/client/hud/BongHudOrchestrator.java:180-194` 又把同一个 store 用在季节 screen tint、右上角季节 icon、突破叠层 HUD。结果就是：玩家明明已经跨入炎汐/凝汐/汐转，屏幕 tint、季节图标、粒子、音乐和相位切换事件都还停在上一季；等到稍后某次 qi/social 变化触发 `player_state`，这些反馈会突然整批跳变，体感像“季节晚到了一拍”。这会直接误导玩家对突破时机、灵田/灵草季节状态、天地节律变化的判断。
- **建议修复范围 / 模块**：优先收口 `server/src/network/mod.rs`、必要时少量补 `client/src/main/java/com/bong/client/state/SeasonStateStore.java` / `client/src/main/java/com/bong/client/season/SeasonVisualController.java` 的 reset 细节。服务端方向至少要补一条真实生产同步路径，二选一即可但要统一语义：1) `SeasonChangedEvent` 直接驱动面向在线客户端的 season-only payload；2) 把测试里的 periodic cadence 落成生产版，让 `player_state` 在无组件变更时也按固定周期补发。无论选哪条，**都不能继续让季节同步完全依赖无关组件的 Changed 偶然带出。**
- **验收抓手**：至少补 4 组 pin。1) 只推进 `WorldSeasonState`、不改 `PlayerState/Cultivation/social` 时，客户端仍能在跨相位后收到新 `season_state`。2) “满 qi、满心境、无 social 变化”的 idle 玩家跨季后，`SeasonStateStore` 会在可接受窗口内更新，而不是无限等到下一次 unrelated payload。3) `SeasonVisualController` 的 phase transition / atmosphere / particle 联动在 season-only 同步路径上照常生效。4) 回归现有 `player_state` 路径，确认没有把普通状态包频率意外放大到每 tick 刷屏。

## 反方裁决摘要

1. **Round 1 反方主张**：`Changed<Cultivation>` 可能几乎每 tick 都会触发，季节最多只会短暂陈旧。  
   **裁决**：不成立。`server/src/cultivation/tick.rs:187-190,254-255,272-273` 明确要求玩家存在 `qi_room` 且 `gain > 0` 才会改 `qi_current`；`server/src/cultivation/composure.rs:10-15` 也只在 `composure < 1.0` 时写回。满 qi + 满心境玩家不会被这两条被动路径刷新，因此“站桩等跨季”是稳定可达的陈旧场景。
2. **Round 2 反方主张**：也许已经有生产级的周期补发或 season 专属同步通道，只是没在第一眼代码附近。  
   **裁决**：不成立。全仓检索后，`SeasonChangedEvent` 在网络层唯一消费点就是 `server/src/network/mod.rs:1297-1313` 的 Redis 发布；面向客户端的 season 写入唯一入口是 `client/src/main/java/com/bong/client/BongNetworkHandler.java:817`。而所谓 periodic emission 只存在于测试 seam `5002-5063` 与对应测试 `5320-5357`，生产 `add_systems(Update, ...)` 里并没有挂进去。
3. **人工复核补充**：这个缺口不是“纯 atmosphere 漂一点”的轻微问题。`SeasonVisualController`、`BongHudOrchestrator`、突破季节叠层和季节图标都挂在同一份 store 上；一旦 store 旧，玩家看到的季节反馈就是系统性错误，而不是某一个特效晚刷新。

## 开放问题

1. 修复 PR 应优先选“season-only payload”还是“生产 periodic player_state cadence”？前者最小化带宽外溢，后者能顺手补齐更多依赖 `player_state` 的陈旧风险，但也更容易影响全局发包节奏。
2. 如果走 periodic `player_state`，cadence 应复用 `WORLD_STATE_PUBLISH_INTERVAL_TICKS` 还是单独给客户端状态包更短/更长的周期，需要在修复 PR 中用现有 payload 体积和 HUD 敏感度做一次权衡。

## 审计来源

bug-hunt 定点轮（只收窄 `season / era / hud / state` 主路径）。本轮人工沿 `WorldSeasonState -> SeasonChangedEvent -> network emit -> client SeasonStateStore -> SeasonVisualController/HUD` 全链路核对，候选经过两轮默认怀疑式证伪后保留。当前结论是 **report-only**：先提交 skeleton plan，把玩家影响、断链点、修复面与验收抓手讲清，再由后续 fix PR 单独落地。
