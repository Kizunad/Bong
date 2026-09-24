# plan-bughunt-woliu-vortex-shield-loop-boundary-v1

## Bug

`woliu_vortex_shield.json` 是 `isLoop: true`、`endTick: 18`、`returnTick: 0` 的 PlayerAnimator 循环动画，但循环边界没有闭合：

- `body.y` 只在 tick 0 写入 `-0.02`，tick 18 缺失。
- `torso.yaw` 只在 tick 9 / tick 18 写入，tick 0 缺失。
- 双臂 `pitch/yaw/roll/bend` 在 tick 0 与 tick 18 取值明显不同。

项目动画约束说明 `isLooped=true` 时用到的 axis 必须在 `endTick` 补同值 keyframe，否则 PlayerAnimator 会在循环中插回 `defaultValue` 或在回环边界跳变（`docs/player-animation-conventions.md §7.1 L203-L205`）。

## Evidence

- `client/src/main/resources/assets/bong/player_animation/woliu_vortex_shield.json:7-L23`
  - `endTick=18`、`isLoop=true`、`returnTick=0`
  - tick 0 / tick 18 的 body、torso、左右臂边界不闭合。
- `docs/finished_plans/plan-woliu-v3.md:59-L65`
  - 涡流护体被定义为 `woliu_vortex_shield.json` 的 `FULL_BODY loop`。
- `server/src/combat/woliu_v2/skills.rs:1509-L1512`
  - `WoliuSkillId::VortexShield` 的 AV 三件套包含 `bong:woliu_vortex_shield`。
- `server/src/combat/woliu_v2/skills.rs:1952-L1956`
  - `VortexShield` 正式 `animation_id` 为 `bong:woliu_vortex_shield`。

## 实际游玩体验影响

玩家施放涡流护体时，持续护体姿态每约 0.9 秒循环一次。由于边界不闭合，身体、躯干和双臂会在循环边界出现回弹、衰减或跳变；远处观察时会削弱“涡流护体正在维持”的读招辨识度，让防御技看起来像卡顿的一次性摆臂，而不是稳定的持续护体姿态。

## 去重

- 非 #1038：#1038 是 `woliu_vortex_resonance` 循环姿态衰减，技能和资源不同。
- 非 #1074：#1074 是涡流虚蚀五招动画资源断链；本问题是已存在资源的 loop keyframe 边界错误。
- 非 #1085：#1085 是 `shield_raise` 举盾循环边界跳变；机制相邻但对象是盾牌格挡资源，不是涡流护体。

## Adversarial conclusion

两轮 adversarial 复核结论：KEEP。该问题不影响技能数值、粒子、音效或 HUD，但会实质影响 client combat 动画可读性；属于低到中等严重度的客户端战斗动画体验 bug。

## Skeleton TODO

- [ ] 修正 `woliu_vortex_shield.json` 的 loop 边界：tick 0 与 tick 18 对所有用到的 axis 闭合；若要保留摆动感，应在中间 tick 表达摆动，而不是依赖不闭合回环。
- [ ] 用 `client/tools/render_animation.py` 或等价 headless 预览验证 `woliu_vortex_shield` 连续循环无身体/躯干/双臂跳变。
- [ ] 增加或扩展 loop closure 校验，至少覆盖 `client/src/main/resources/assets/bong/player_animation/*.json` 中 `isLoop=true` 的战斗动画，防止同类资源回归。

## 验证结论（2026-07-26 整理审计追认）

commit 5d9bdd8fed（2026-07-20，PR #1241「skill-anim-fidelity PR-5」）修复了本 bug：`woliu_vortex_shield.json` 的 `endTick` 从 18 改为 20，重写了 loop 边界 keyframe，tick 0 与 tick 20 在 body/torso/双臂全轴上逐字段一致，循环闭合生效，消除了原描述的回弹/衰减/跳变问题。

## Finish Evidence

- **落地清单**：`client/src/main/resources/assets/bong/player_animation/woliu_vortex_shield.json`（loop 边界重写）
- **关键 commit**：5d9bdd8fed（2026-07-20，PR #1241「skill-anim-fidelity PR-5」）—— endTick 18→20，tick0/tick20 全轴逐字段一致
- **测试结果**：PR #1241 同批次的 loop 闭合断言覆盖该资源；2026-07-26 审计为只读核验（Read+grep+git log 对拍 origin/main），未重跑测试套件
- **跨仓库核验**：client `woliu_vortex_shield.json`；server `WoliuSkillId::VortexShield` 的 `animation_id=bong:woliu_vortex_shield` 未改动，AV 三件套契约不变
- **遗留 / 后续**：无
