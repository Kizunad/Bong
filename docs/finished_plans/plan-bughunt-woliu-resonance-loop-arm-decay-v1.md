# plan-bughunt-woliu-resonance-loop-arm-decay-v1

> 一句话主题：`bong:woliu_vortex_resonance` 是 80 tick 持续型 loop 玩家动画，但双臂 `rightArm/leftArm` 的关键轴只写到 tick 40，`endTick=80` 没有补同轴 keyframe；PlayerAnimator 会把后半段插回默认值，导致涡流共振施法窗口后半段角色双臂逐渐垂回默认姿态。**收口日期 2026-07-23，骨架 → active → finished 同 PR 内一次性收口。**

> 立项动机：本轮 BugHunt D9 聚焦 client combat/skills/cast animation/VFX/SFX/HUD/icon registry/packet bridge。该问题落在实际涡流共振施法的玩家动画反馈链路上，不重复 #987 技能配置拒绝缺少施法同步、#997/#1002 毒蛊视听/HUD、#1012 vfx_event slash 契约、#1018 蜕壳视听双源、#1027 技能栏施法源、#1033 爆脉视听双源。

## Bug 摘要

`client/src/main/resources/assets/bong/player_animation/woliu_vortex_resonance.json` 声明 `emote.isLoop=true`、`endTick=80`。双臂 `rightArm/leftArm` 的 `pitch/yaw/roll/bend/axis` 只在 tick 0 和 tick 40 定义，tick 80 只补了 `body.y` 与 `torso.pitch`。

`docs/player-animation-conventions.md §7.1` 记录了 PlayerAnimator 的循环动画坑：loop 动画里用到的每个 axis 都必须在 `endTick` 补同值 keyframe，否则库会在最后一个 keyframe 后向 `endTick+1` 的 `defaultValue` 插值。对本动画来说，双臂最后一个 keyframe 是 tick 40，后续到 tick 80/81 会逐渐回到默认手臂姿态。

## 对实际游玩体验的影响

影响是战斗可读性和姿态反馈退化，不是技能机制失效，也不是 A/V 全链路丢失。技能效果、粒子、音效、HUD hint 和 StopAnim 清理仍然正常，玩家仍能施放并看到涡旋粒子。

实际割裂点在 4 秒涡流共振窗口内：前半段角色能呈现“托起共振场”的双臂剪影，约第 2 秒后双臂会逐渐垂回接近默认姿态。远处玩家会更难通过骨骼姿态区分“对方正在维持涡流共振”与普通站立/其他涡流招式，削弱招式 A/V 差异化红线要求的可辨识度。

## 证据定位

- `client/src/main/resources/assets/bong/player_animation/woliu_vortex_resonance.json:6-23`：`isLoop=true`、`endTick=80`；双臂轴只在 tick 0/tick 40 出现，tick 80 只剩 `body.y`、`torso.pitch`。
- `docs/player-animation-conventions.md:203-214`：项目约束明确 PlayerAnimator loop 动画中每个用到的 axis 必须在 `endTick` 补同值 keyframe。
- `client/src/main/java/com/bong/client/animation/BongAnimationRegistry.java:22-28`、`119-128`：客户端播放时从 `assets/{namespace}/player_animation/*.json` 资源包读取 PlayerAnimator JSON，`woliu_vortex_resonance.json` 是实际播放源。
- `server/src/combat/woliu_v2/skills.rs:238-245`：`cast_vortex_resonance` 是正式施放入口。
- `server/src/combat/woliu_v2/skills.rs:1679-1695`：涡流共振 spec 是 4 秒/80 tick 持续窗口，并使用 `visual_for(WoliuSkillId::VortexResonance)`。
- `server/src/combat/woliu_v2/skills.rs:1966-1971`：`visual_for(VortexResonance)` 映射 `animation_id: "bong:woliu_vortex_resonance"`。
- `server/src/combat/woliu_v2/skills.rs:734-763`：施法成功后 `emit_cast_events` 发出 `VortexCastEvent`，并携带 `spec.visual`。
- `server/src/network/vfx_animation_trigger.rs:312-338`：`emit_woliu_v2_visual_triggers` 读取 `VortexCastEvent` 并发 `PlayAnim`。
- `server/src/network/vfx_animation_trigger.rs:1798-1830`：现有测试断言涡流共振会发出 `bong:woliu_vortex_resonance`。
- `server/src/network/vfx_animation_trigger.rs:1848-1879`：现有测试把涡流共振当作 active window 内持续动画管理，到期才发同一动画 ID 的 `StopAnim`。

## 触发路径

1. 玩家通过技能栏或 server cast 路径触发 `cast_vortex_resonance`。
2. `vortex_resonance_spec()` 设置 `duration_ticks=4 * TICKS_PER_SECOND`、`cast_ticks=80`，并选择 `visual_for(VortexResonance)`。
3. 施法进入 `emit_cast_events`，发出携带 `bong:woliu_vortex_resonance` 的 `VortexCastEvent`。
4. `emit_woliu_v2_visual_triggers` 把该事件桥接成 `bong:vfx_event` 的 `PlayAnim`。
5. 客户端 `BongAnimationRegistry` 从 `assets/bong/player_animation/woliu_vortex_resonance.json` 读取 loop 动画。
6. 动画播放到 tick 40 后，双臂没有 tick 80 同轴 keyframe；PlayerAnimator 向默认值插值，手臂姿态在窗口后半段退化。

## 反方审查记录

### Round 1

**反方结论**：成立，但严重度应限定为“实际可见的战斗动画退化”，不是玩法机制失效。

**主要反驳点**：

- 文档例子说的是 tick 0 单 keyframe，而本 JSON 双臂在 tick 0/tick 40 都有 keyframe；plan 必须说明真正命中点是“最后一个双臂 keyframe 早于 endTick，tick 40 之后仍会向 defaultValue 插值”。
- 技能持续时间与动画 `endTick` 都是 80 tick，StopAnim 会在 active window 到期清理；不应写成无限循环边界反复跳变，而应写成“4 秒窗口后半段姿态回落”。
- 粒子、音效、HUD hint、技能效果仍在，不能夸大成涡流共振不可用或 A/V 全链路丢失。

**采纳处理**：收窄 bug 定义为“施法 4 秒窗口后半段双臂姿态退化，降低远距离辨识度”。

### Round 2

**反方结论**：通过。

**通过理由**：修订表述已经避开机制失效和全链路丢失的夸大，只保留 `bong:woliu_vortex_resonance` 这个 80 tick loop 动画后半段双臂姿态被插回默认值的事实。开放 PR 中没有同一涡流共振 loop 姿态/PlayerAnimator 末帧补值问题。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 修复涡流共振 loop 动画 endTick 双臂轴补帧 | fix_pr | ✅ 2026-07-23 |

### P0 — 修复涡流共振 loop 动画 endTick 双臂轴补帧

- 在 `woliu_vortex_resonance.json` 的 tick 80 显式补齐 `rightArm/leftArm` 的 `pitch/yaw/roll/bend/axis`。
- 补值应按设计意图选择：
  - 若要 80 tick 后无缝回到初始托掌姿态，则 tick 80 双臂应等于 tick 0。
  - 若要维持 tick 40 的峰值托举到结束，则 tick 80 双臂应等于 tick 40，并重新评估 loop 边界是否自然。
- 不改服务端技能数值、不改粒子/音效/HUD/icon 注册；本 bug 的 fix 面应限定在客户端动画资源和对应资源测试。
- 若引入动画资源 lint，规则应只针对 `isLoop=true` 且非零轴在 `endTick` 缺同轴 keyframe 的玩家动画，避免把有意往复或纯 0 值轴误报。

## 验收测试计划

1. 增加资源级回归：扫描 `client/src/main/resources/assets/bong/player_animation/*.json`，对 `isLoop=true` 的玩家动画校验所有用到的非零 axis 在 `endTick` 有同轴 keyframe；至少覆盖 `woliu_vortex_resonance` 负例修复后的 pin。
2. 用 `client/tools/render_animation.py` 或等价 headless 渲染查看 `bong:woliu_vortex_resonance` 的 tick 0、40、60、79/80 三视图，确认后半段双臂没有垂回默认。
3. 在 `client/` 使用 JDK 17 跑 `./gradlew test build`。
4. 若能做联调，触发涡流共振实际施法，确认 4 秒窗口内粒子、音效、HUD hint 仍正常，双臂剪影在后半段保持可辨识，active window 到期后 StopAnim 正常淡出。

## 风险

- 直接把 tick 80 设回 tick 0 可修复默认值衰减，但可能让 tick 40 峰值回落节奏显得太线性；需要渲染检查 tick 60/79。
- 直接把 tick 80 设成 tick 40 可维持托举峰值，但 loop 回 tick 0 时可能出现边界跳变；需要决定涡流共振是“托举呼吸循环”还是“托举保持”。
- 资源 lint 若过严，可能误伤有意在 endTick 回到默认的 loop 动画；规则需要区分“缺 keyframe”与“显式写默认值”。

## 审计来源

BugHunt D9 client-combat 定点轮。方法：开放 PR 去重、server 施法路径与 client PlayerAnimator JSON 播放路径对照、loop 玩家动画批量扫描、两轮反方 subagent 对抗审查。当前 PR 仅新增 report-only skeleton plan，不修改代码/配置/依赖/资源，不消费/归档 plan。

## Finish Evidence

### 落地清单（P0）

- `client/tools/gen_woliu_vortex_resonance.py`（新增）：原资产手写无生成器，改用 `anim_common.emit_json`（自带 `_check_loop_closure` 强校验，`is_loop=True` 时若 tick0/endTick 逐轴不等会直接 `AssertionError` 拒绝生成）。POSE 表在 tick 40 保留原始峰值托举姿态不变，只在 endTick(80) 补 `rightArm`/`leftArm` 全部轴（pitch/yaw/roll/bend/axis）== tick0 的收尾帧，并在 tick0 补 `torso.pitch` == endTick(80) 值(0.0)的起手帧。原始弧度值经 `math.degrees()` 换算，往返 `round(...,7)` 精度下与原文件逐位相等（已用 Python 验证零漂移，见关键 commit）。
- `client/src/main/resources/assets/bong/player_animation/woliu_vortex_resonance.json`：经上述生成器重新生成，`emote.isLoop=true`/`endTick=80`/`returnTick=0` 不变，`moves` 从 9 组扩到 36 组（一 move 一 axis，与仓库内其余 `anim_common` 生成资产格式一致，如 `woliu_vortex_cast.json`）。
- `client/src/test/java/com/bong/client/animation/AnimCastTicksAlignmentTest.java`：`CAST_ALIGNMENT_ALLOWLIST` 从含 `woliu.vortex_resonance` 单条改为 `Set.of()`（清零）；类头 Javadoc 同步更新「本表余 1 条」为「本表清零」。`woliu.vortex_resonance` 现直接经主契约 `castAlignmentContractHoldsForEverySkillOutsideAllowlist` 的 `loopSeamViolations` 校验（cast_ticks=80 走长引导 loop 分支），不再依赖 allowlist 豁免。
- headless 渲染核验（`client/tools/render_animation.py --ticks 0,40,60,79,80`）：tick0 与 tick80 姿态逐值相同（`rArm p=-29 y=-23 bend=+54@ax+180`），tick60/79 在 tick40 峰值（`p=-77`）与 tick0/80 基位之间平滑过渡，无跳变/回默认值现象。

### 关键 commit

- `ed2bbc350`（2026-07-23）docs(promote): 骨架 → active。
- `ff545b20a`（2026-07-23）fix(client): woliu_vortex_resonance 补齐循环闭合关键帧，修 PlayerAnimator 库坑 #1 单帧衰减。

### 测试结果

- `cd client && JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64 ./gradlew test build` → `BUILD SUCCESSFUL`；全量 4274 个测试用例 0 failures / 0 errors / 0 skipped（`build/test-results/test/TEST-*.xml` 统计）。`AnimCastTicksAlignmentTest` 14/14 通过（含 `castAlignmentContractHoldsForEverySkillOutsideAllowlist`、`allowlistEntriesActuallyFailAlignment`、`allowlistsOnlyShrinkAgainstFrozenBaseline`）。
- 独立无上下文 read-only validator agent（对拍 HEAD `ff545b20aa6f6e83e9794d0baa8e55e5f3ac297d`）逐轴核验 tick0/tick80 闭合、生成器确定性重跑无 diff、tick40 峰值姿态未被破坏、改动范围仅限 3 个文件——结论 **PASS**。

### 跨仓库核验

- **client**：`BongAnimationRegistry`（消费 `assets/bong/player_animation/woliu_vortex_resonance.json`，未改，仅资产内容更新）、`AnimCastTicksAlignmentTest`（改）。
- **server**：`server/src/combat/woliu_v2/skills.rs`（`cast_vortex_resonance`/`vortex_resonance_spec`）、`server/src/network/vfx_animation_trigger.rs`（`emit_woliu_v2_visual_triggers`）——均未改动，核验为可达但无辜（cast_ticks=80 与动画 endTick=80 对齐正确，PlayAnim/StopAnim 接线正常）。
- **无** schema / IPC / Redis key 改动；**无** agent 改动；不涉及粒子/音效/HUD/icon（bug 本身与这些无关）。

### 遗留 / 后续

- 骨架 §验收测试计划 #1 提到的「资源级批量扫描所有 `isLoop=true` json」未新增独立扫描器——`AnimCastTicksAlignmentTest` 的 allowlist 棘轮机制已对 `woliu.vortex_resonance` 提供等价的机械 pin（清零后任何回归会立刻在主契约撞红），且该测试已覆盖仓库内全部走 `SKILL_ANIM` 映射的 loop 动画，判定无需额外重复扫描器；若未来出现不经 `SKILL_ANIM` 映射的 loop 动画（如纯装饰性亮相动作），扫描器仍是有效补强，留给后续 bughunt 轮次视需要立项。
