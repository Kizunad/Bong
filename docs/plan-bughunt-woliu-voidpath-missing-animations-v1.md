# BugHunt: 涡流虚蚀五招 PlayerAnimator 动画资源断链

## 结论

涡流虚蚀路径五招服务端会通过 `bong:vfx_event` 下发 `PlayAnim`，但客户端没有对应的 `assets/bong/player_animation/*.json`，也没有 Java fallback/inline 预注册。结果是 `BongAnimationRegistry.get(animId)` 返回 `null`，`BongAnimationPlayer.playOnStack` 直接 `false`，客户端记录 bridge miss 后不播放施法者骨骼动画。

## 实际游玩体验影响

玩家施放 `AmbientVortex` / `VoidVortex` / `SwallowingVortex` / `VortexEcho` / `VoidCore` 时，粒子、SFX、HUD/icon 仍可能出现，但角色身体不做对应动作。近端玩家和远端旁观者只能看见环境特效，无法从施法者姿态判断“对面正在起哪一招”，虚蚀路径五招的 A/V 差异化少掉关键一层；在混战或远距离观察时，这会直接削弱读招、反应和复盘体验。

## 证据

- `server/src/combat/woliu_v2/skills.rs:1479` 的 `emit_anim` 构造 `VfxEventPayloadV1::PlayAnim`，把 `anim_id` 原样发给客户端。
- `server/src/combat/woliu_v2/skills.rs:1981` 起，虚蚀路径五招分别配置：
  - `bong:woliu_ambient_vortex`
  - `bong:woliu_void_vortex`
  - `bong:woliu_swallowing_vortex`
  - `bong:woliu_vortex_echo`
  - `bong:woliu_void_core`
- 当前客户端 `client/src/main/resources/assets/bong/player_animation/` 中只有：
  - `stance_woliu.json`
  - `vortex_palm_open.json`
  - `vortex_spiral_stance.json`
  - `woliu_turbulence_burst.json`
  - `woliu_vacuum_lock.json`
  - `woliu_vacuum_palm.json`
  - `woliu_vortex_resonance.json`
  - `woliu_vortex_shield.json`
- `client/src/main/java/com/bong/client/animation/ClientAnimationBridge.java:38` 会把 `play_anim` 转入 `AnimationLayerManager.play`。
- `client/src/main/java/com/bong/client/animation/BongAnimationRegistry.java:120` 明确查找顺序是 `inline → JSON → Java fallback`；全仓搜索上述五个动画 ID 只命中服务端配置，没有命中客户端资源或 Java fallback。
- `client/src/main/java/com/bong/client/animation/BongAnimationPlayer.java:132` 在 `BongAnimationRegistry.get(animId)` 为 `null` 时直接返回 `false`，因此这些动画会被路由层视为 bridge miss。

## 非重复说明

- 不是 #1038 “涡流共振循环姿态衰减”：#1038 是已存在循环动画的 keyframe/endTick 问题；本 bug 是五个虚蚀路径动画资源完全不存在。
- 不是 #1063 “逆脉护体缺失独立动画”：本 bug 只覆盖涡流虚蚀路径五招。
- 不是 #1057 “VFX/SFX 跨维广播”：本 bug 与广播维度无关，目标客户端即使收到 payload 也无法解析到动画资源。
- `docs/finished_plans/plan-bughunt-r3-findings-v1.md` 记录过虚蚀路径五招粒子 ID 缺注册；当前问题是同一组技能的 `animation_id` 缺客户端 PlayerAnimator 资源，属于不同链路。

## 对抗结论

- Round 1 Finder：支持，高置信。独立指出五个 `animation_id` 只在服务端映射出现，客户端 `player_animation` 目录缺资源，`BongAnimationPlayer` 查不到动画会返回 `false`。
- Round 1 Skeptic：未反驳该候选，主要排除了 cast source、技能 icon 兜底、动画层清理等误报方向。
- Round 2 本地反证：再次核对 `emit_anim`、`visual_for`、客户端资源目录、`BongAnimationRegistry` 查找顺序和 `BongAnimationPlayer` null 分支，未发现别名、inline 或 Java fallback 可兜底。尝试再 spawn 外部 Round 2 skeptic 时工具达到 agent thread limit；未创建实现改动。

## 修复 TODO

- [ ] 为五个虚蚀路径 `animation_id` 补齐 PlayerAnimator JSON，或将服务端映射改到已存在且语义合理的动画 ID；若复用，必须保证五招仍有可辨识姿态差异。
- [ ] 补客户端资源回归：`BongAnimationRegistry.contains` 或资源扫描测试必须 pin 住 `bong:woliu_ambient_vortex`、`bong:woliu_void_vortex`、`bong:woliu_swallowing_vortex`、`bong:woliu_vortex_echo`、`bong:woliu_void_core`。
- [ ] 补端到端/路由回归：构造五个 `play_anim` payload，确认 `VfxEventRouter` 不再 bridge miss。
- [ ] 按 `docs/player-animation-conventions.md` 验收 FPV/TPV，确认五招 animation、particle、SFX、HUD/icon 在实机读招上互相区分。
