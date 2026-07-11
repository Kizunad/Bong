# BugHunt: PlayerAnimator 重连旧层缓存导致同招静默无动画

> Skeleton Plan / report-only。client-combat 20260708 r01 发现：`BongAnimationPlayer` 把玩家动画层按 `UUID + animId` 缓存在静态 map；断线 / 切服 / 重连时没有清理。重连后的同一玩家 UUID 再次播放断线前登记过的同一 `animId` 时，客户端会命中旧 `ModifierLayer`，只在旧 layer 上 `replaceAnimationWithFade` 并返回成功，不会把动画层挂到新 `PlayerEntity` 的 `AnimationStack`，表现为同招骨骼动画静默缺失。

## Bug 摘要

`client/src/main/java/com/bong/client/animation/BongAnimationPlayer.java:56-58` 使用静态 `ACTIVE_LAYERS` 记录 `UUID -> animId -> ModifierLayer`。`playOnStack` 收到当前 player 的 `AnimationStack` 后，只有首次分支会执行 `stack.addAnimLayer(...)`（同文件 `:159-166`）；若 `ACTIVE_LAYERS` 已有同 UUID + 同 animId，`:146-156` 直接对旧 `ModifierLayer` 调 `replaceAnimationWithFade(...)` 并返回 `true`。

重连会创建新的 client player entity / PlayerAnimator stack，但断线清理没有清空这份静态表。`AnimationLayerManager` 也有静态 `ACTIVE_BY_CHANNEL`（`client/.../AnimationLayerManager.java:20-21`），其生产 `play` 路径每次从当前 player 取新 stack（`:64-81`），再委托到 `BongAnimationPlayer.playOnStack(...)`。因此旧 map key 会让新 stack 的同招重播误走“同层重触发”分支。

`client/src/main/java/com/bong/client/BongNetworkHandler.java:132-133` 的 disconnect 只调用 `clearClientStateOnDisconnect()`；该函数 `:857-901` 清了 realm collapse、NPC、TSY、音频、虚蚀、时代、Agent UI、遗骸、craft 等状态，但没有触碰 `BongAnimationPlayer` 或 `AnimationLayerManager`。`client/src/main/java/com/bong/client/combat/CombatHudBootstrap.java:96-121` 的 combat reset 也只清 HUD / store / panel 状态，不清动画层缓存。

## 实际游玩体验影响

玩家不断客户端、只断线 / 切服 / 重连后，如果重新按同一个热键技能、再次举同一种盾、重试同一个蓄力 / 护体 / stance 动作，粒子、音效、HUD 可能都正常，`bong:vfx_event` 路由也会返回成功，但角色骨骼动画不出现在新会话的 player model 上。对其他玩家和第三人称视角来说，就是“招式有特效没动作”：技能辨识度下降，格挡 / 蓄力 / 持续护体的读招反馈丢失。

影响需要收窄：这不是全局动画永久失效。若重连后先收到对应 `stop_anim`，或先播放同 channel 的另一个 `animId`，`AnimationLayerManager` 会 stop 旧记录并让新动画进入新 stack；完全重启客户端也会自愈。缺口集中在“同 UUID + 同 animId 重播”路径，而这条路径在热键技能、持续防御、重复调试和同招连用中很常见。

## 证据定位

- `client/src/main/java/com/bong/client/animation/BongAnimationPlayer.java:56-58`：静态 `ACTIVE_LAYERS` 以 UUID 和 animId 保存 `ModifierLayer`。
- `client/src/main/java/com/bong/client/animation/BongAnimationPlayer.java:146-156`：同 UUID + 同 animId 已存在时，只替换旧 layer，不调用当前 `stack.addAnimLayer(...)`。
- `client/src/main/java/com/bong/client/animation/BongAnimationPlayer.java:159-166`：只有首次分支会把新 layer 加入传入的 `AnimationStack`。
- `client/src/main/java/com/bong/client/animation/BongAnimationPlayer.java:64-68`：注释只讨论 `PENDING_REMOVALS` 持有旧 stack 后的 try/catch 回收，不处理 `ACTIVE_LAYERS` 的旧绑定。
- `client/src/main/java/com/bong/client/animation/AnimationLayerManager.java:20-21`：同样维护静态 `ACTIVE_BY_CHANNEL`。
- `client/src/main/java/com/bong/client/animation/ClientAnimationBridge.java:34-39`：`play_anim` 每次解析当前 world 的 player，再调用 `AnimationLayerManager.play(...)`。
- `client/src/main/java/com/bong/client/BongNetworkHandler.java:857-901`、`client/src/main/java/com/bong/client/combat/CombatHudBootstrap.java:96-121`：断线清理路径没有动画层 reset。

## 不重复说明

- 不重复 #1038 / #1085 / #1121：那些是 PlayerAnimator JSON loop 边界或姿态衰减，属于资源内容问题。
- 不重复 #1063 / #1074：那些是特定招式缺少动画接线或资源断链，本 bug 是中央动画播放缓存跨重连命中旧 stack。
- 不重复 #1094 / #1100 / #1105 / #1110：那些是 combat HUD store 断线残留，本 bug 不影响 HUD store 本身，而是 `play_anim` 成功后玩家骨骼动画层没有进入新 `AnimationStack`。
- 不重复 #1125：那是战斗浮字方向标识与误判根因修正，不涉及 client PlayerAnimator 生命周期。

## Skeleton Fix Plan

- [ ] 给 `BongAnimationPlayer` 增加生产可调用的断线清理入口，清空 `ACTIVE_LAYERS` 与 `PENDING_REMOVALS`；测试钩子可复用但不要只暴露 test-only 方法。
- [ ] 给 `AnimationLayerManager` 增加生产可调用的断线清理入口，清空 `ACTIVE_BY_CHANNEL`。
- [ ] 将两者接入 client disconnect 清理路径，优先放在 `BongNetworkHandler.clearClientStateOnDisconnect()` 或统一 combat/animation bootstrap 的 disconnect hook 中，保证切服 / 断线 / 重连前旧 stack binding 不跨 session。
- [ ] 更稳的实现可让 `ACTIVE_LAYERS` 记录当前 `AnimationStack` identity；若当前播放传入的 stack 与缓存 stack 不同，则丢弃旧 binding 并重新 `addAnimLayer`，防止未来非 disconnect 的 player entity 替换也踩中。
- [ ] 不改变同会话同 animId 重触发复用 layer 的行为；那是现有测试覆盖的平滑重播语义。

## 验证计划

- [ ] 新增 client 单测：同一 UUID 在旧 `AnimationStack` 播放某 `animId` 后，模拟断线清理，再用新 `AnimationStack` 播同一 `animId`，断言新 stack 获得新 layer 且 `playOnStack` 返回 true。
- [ ] 新增负例回归：未断线、同一 stack 上同 animId 重触发仍不新增重复 layer，只替换现有 layer。
- [ ] 新增 `AnimationLayerManager` 测试：断线清理后同 channel 同 animId 可重新进入新 stack；同 channel 不同 animId 的 stop / play 语义不回归。
- [ ] 跑 `cd client && ./gradlew test`（JDK 17）。
- [ ] 可选实测：本地进入服务器，触发 `bong:shield_raise` 或任一持续玩家动画，断线重连后先触发同一动画，第三人称确认骨骼动作仍出现。

## 反方审查记录

第一轮反方结论：可提交。反方确认这不是已有 client-combat PR 的重复；现有断线残留 PR 多集中在 HUD store，资源类 PR 集中在 JSON / icon / 特定招式接线，均不触碰 `client/src/main/java/com/bong/client/animation/*` 的静态 layer 生命周期。

第二轮反方结论：提交但收窄。反方指出同 channel 不同 `animId`、收到 `stop_anim`、或重启客户端都能自愈，所以不能写成“大面积动画永久失效”。最终裁决是：问题真实存在，标题和影响应限定为“同 UUID + 同 animId 重播命中旧 `AnimationStack` binding，路由成功但新会话静默无骨骼动画”。

## 风险

- 直接清空所有动画层状态会截断断线瞬间的淡出队列；但断线时旧 world / player entity 本就不再可见，清理比保留旧 stack binding 更合理。
- 若未来支持同客户端多 world preview 或 fake player animation stack，需要确认清理只在真实网络 session 生命周期触发。
- 若选择 stack identity 自愈而非只做 disconnect 清理，需要避免引入强引用泄漏；测试应覆盖 pending removal 和 active map 都能释放旧 binding。
