# plan-bughunt-shield-raise-loop-boundary-v1

> Skeleton Plan。主题：`bong:shield_raise` 是盾牌格挡的持续举盾 PlayerAnimator 循环动画，但 JSON 的保持段没有闭合；玩家按住盾牌格挡时，第三人称/旁观视角可能看到盾牌姿态在 loop 边界轻微抽动或回弹。

## 一句话 bug

`shield_raise.json` 声明 `isLoop=true`、`returnTick=3`，但 tick 3 的真正举盾保持姿态与 tick 6 的呼吸末帧不闭合，且 tick 0 轴与 endTick 轴也不一致；PlayerAnimator 循环播放时存在边界跳变/姿态回落风险。

## 实际游玩体验影响

- 玩家按住盾牌格挡时，角色不一定稳定保持“盾牌抬起护住身体”的剪影，loop 边界可能出现盾牌轻微回弹、身体 bob 抽动或手臂姿态跳变。
- 旁观者从远处读招时，“正在持续举盾防御”的姿态可辨识度下降，容易误判为短促抬盾或动作抖动。
- 不影响服务端格挡数值、体力 drain、盾破、停止动画清理；这是客户端战斗动画可读性 bug，不是盾牌格挡机制失效。

## 复现路径

1. 准备一名装备盾牌的玩家。
2. 按住盾牌格挡输入，触发服务端 `RaiseShieldIntent`。
3. 服务端 `raise_shield_handler` 发送 `bong:shield_raise` 持续举盾动画。
4. 在第三人称或旁观者视角持续观察 1-2 秒。
5. 预期：盾牌保持稳定举盾姿态，只做平滑呼吸循环。实际风险：动画在 `3 -> 6 -> 3` 循环边界出现小幅跳变；若 PlayerAnimator 按 tick 0/endTick 边界处理，则还可能出现更明显的入场姿态回弹。

## 根因证据

- `client/src/main/resources/assets/bong/player_animation/shield_raise.json:7` 到 `:11`：`endTick=6`、`isLoop=true`、`returnTick=3`，说明这是持续循环动画。
- `client/src/main/resources/assets/bong/player_animation/shield_raise.json:15` 到 `:90`：tick 0 是入场初姿态，例如 `leftArm.pitch=-0.2617994`、`leftArm.bend=0.2617994`。
- `client/src/main/resources/assets/bong/player_animation/shield_raise.json:93` 到 `:175`：tick 3 才是真正举盾保持姿态，例如 `leftArm.pitch=-1.3962634`、`leftArm.bend=1.9198622`、`body.y=-0.05`。
- `client/src/main/resources/assets/bong/player_animation/shield_raise.json:177` 到 `:258`：tick 6 是呼吸末帧，但与 tick 3 不闭合，例如 `body.y=-0.04`、`leftArm.pitch=-1.3788088`、`leftArm.bend=1.9024076`；循环回 tick 3 时会产生边界跳变。
- `docs/player-animation-conventions.md:203` 到 `:214`：项目已固化 PlayerAnimator loop 坑，循环动画用到的 axis 必须在 endTick 补同值 keyframe，避免衰减到 defaultValue 或出现回环跳变。
- `client/src/test/java/com/bong/client/animation/SwordPathV2AnimationAssetTest.java:58` 到 `:73`：仓内已有测试口径要求 loop 动画 tick0/endTick 同轴同值，并把不一致定义为 loop boundary mismatch。
- `client/src/main/java/com/bong/client/animation/BongAnimations.java:36` 到 `:37`：`SHIELD_RAISE` 明确是持续举盾动画，且与 `guard_raise` 独立。
- `server/src/combat/shield_block.rs:346` 到 `:351`：成功举盾路径会触发 `emit_shield_raise_for_entity`。
- `server/src/network/vfx_animation_trigger.rs:1477` 到 `:1495`：服务端向客户端发送 `PlayAnim{anim_id=ANIM_SHIELD_RAISE, priority=COMBAT_PRIORITY, fade_in_ticks=Some(2)}`。
- `client/src/main/java/com/bong/client/animation/BongAnimationRegistry.java:22` 到 `:28`：客户端从 `assets/{namespace}/player_animation/*.json` 加载 PlayerAnimator JSON，`shield_raise.json` 是实播资源。

## 修复计划骨架

- [ ] 明确 `shield_raise` 的循环语义：若保留 `returnTick=3`，则让 tick 6 与 tick 3 在所有保持姿态轴上闭合；若改为常规 tick 0/endTick 循环，则拆出入场动画或让 tick 0 直接成为 guard pose。
- [ ] 补齐 `shield_raise.json` 的 loop 末帧 keyframe，避免 PlayerAnimator 对单 key axis 做 defaultValue 衰减。
- [ ] 将 `SwordPathV2AnimationAssetTest` 中的 loop 轴闭合断言抽成全资源 manifest 测试，覆盖 `shield_raise`、涡流、盾牌等所有 `isLoop=true` JSON。
- [ ] 保持 `emit_shield_raise_for_entity`、`emit_shield_stop_for_entity`、格挡数值和体力消耗逻辑不变。

## 验证计划

- [ ] client 资源测试：遍历 `client/src/main/resources/assets/bong/player_animation/*.json`，所有 `isLoop=true` 动画的 loop 边界轴必须闭合；`shield_raise` 必须被覆盖。
- [ ] 回归测试：`bong:shield_raise` 仍由举盾成功路径发出，放下盾牌/死亡仍发停止动画，不改变格挡状态机。
- [ ] 视觉验收：第一人称、第三人称、旁观者视角按住盾牌格挡至少 2 秒，盾牌保持稳定护身剪影，无明显手臂回弹或身体边界跳动。

## 去重说明

- 非 #1038：#1038 是 `woliu_vortex_resonance` 涡流共振循环姿态衰减；本 plan 对象是 `shield_raise` 盾牌举盾持续姿态。机制相邻，但触发链、资源和实际战斗反馈不同。
- 非 #1074：#1074 是涡流虚蚀五招 PlayerAnimator JSON/fallback 资源断链；本 plan 的资源存在并会播放，问题是 loop 边界不闭合。
- 非 #1063：#1063 是逆脉护体缺失独立动画；本 plan 的 `PlayAnim` 和 JSON 均存在，问题是持续姿态质量。
- 非 #1051/#1057：不涉及绝脉断链 HUD false positive，也不涉及 VFX/SFX 跨维广播。

## 对抗复核结论

两轮对抗后结论：接受为高置信、低到中等严重度的客户端战斗动画 bug。第一轮指出同类 PlayerAnimator loop 盲区，但涡流共振部分与 #1038 重合；第二轮收窄到 `shield_raise` 后确认不重复，且强调 `returnTick=3` 是合理反驳点，所以表述必须限定为“盾牌举盾 loop 保持姿态不闭合/边界跳变风险”，不得夸大为盾牌格挡功能不可用。
