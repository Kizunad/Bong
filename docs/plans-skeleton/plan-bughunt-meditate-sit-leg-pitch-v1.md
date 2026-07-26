# BugHunt: 打坐循环动画腿部 pitch 定死 -80°，撞穿项目自定 40° 断腿红线

## Bug 摘要

**严重度：medium（未调整）。**

`client/src/main/resources/assets/bong/player_animation/meditate_sit.json` 的 `rightLeg`/`leftLeg` 在全部 5 个关键帧（tick 0/10/20/30/40，一个 `isLoop:true` 的 40-tick 循环）里把 `pitch` 恒定写死为 `-1.3962634` rad（= **-80°**）。这正好是 `docs/player-animation-conventions.md §7.2` 明文记载的安全阈值（"pitch 控制在 ~40° 以内，θ=55° 时 ±1.64px 错位已'肉眼明显可见'"）的两倍，且是**持续渲染的静止姿势**而非一闪而过的过渡帧——髋部与大腿的接缝断裂会在整段循环里持续可见。

该动画不是冷门/边角内容：它是 `BreakthroughSpectacleRenderer.plan()` 里 `PRELUDE` 阶段硬编码的 `animationId`，而 `PRELUDE` 是**每一次自然境界突破**（醒灵→引气→凝脉→固元→通灵→化虚，cultivation 系统最核心的正反馈时刻）流程最前段必然播放的姿态，属于全服玩家观看频率最高的动画之一。

## 实际游玩体验影响

玩家每次修炼到临界点触发境界突破，屏幕特效（`fov_zoom_in`、灵气聚拢粒子、心跳音效）一起启动的同时，角色会摆出一个"盘腿打坐"姿势并保持整段 PRELUDE 阶段（最长可达 3 秒，`Math.min(durationMillis, 3_000L)`）。因为腿部 pitch 是文档阈值的两倍，第三人称观察这个角色（自己开镜头模式、或旁观其他玩家突破）时，大腿根部与髋部之间会出现明显的模型断裂缝隙——在修仙世界观里本应是"庄严盘坐、灵气环绕"的高光时刻，视觉上却像是角色的腿"脱臼摆错了位置"，破坏了这个本该是全场最值得展示的正反馈瞬间的沉浸感。且由于是持续 held 姿势（不是快速掠过的攻击帧），断裂缝隙有充足时间被玩家仔细看到。

## 证据定位

- `client/src/main/resources/assets/bong/player_animation/meditate_sit.json:5`：文件自带中文 `description` 字段声称 "腿 pitch=-90° ... bend=175°"，但实际写入的数值与描述本身不一致（进一步印证下面的 pitch/bend 是刻意设计的大幅度盘腿造型，而非笔误）。
- `client/src/main/resources/assets/bong/player_animation/meditate_sit.json`：`rightLeg.pitch` 在全部关键帧恒为 `-1.3962634` rad（≈ **-80.0000°**）：
  - tick 0：L106-111
  - tick 10：L267-273
  - tick 20：L428-434
  - tick 30：L589-595
  - tick 40：L750-756
- 同文件 `leftLeg.pitch` 同值同频次：
  - tick 0：L141-146
  - tick 10：L302-308
  - tick 20：L463-469
  - tick 30：L624-630
  - tick 40：L785-791
- 同文件 `rightLeg.bend`/`leftLeg.bend` 恒为 `1.5707963` rad（90°，非 description 声称的 175°），说明"髋膝踝同水平"的视觉目标几乎全靠 pitch 硬扛，`bend` 没有分担足够的旋转量。
- `docs/player-animation-conventions.md:225-234`（§7.2 "MC 模型 rigging 没有 IK / skinning —— 大 `leg.pitch` 必然断腿"）：明文给出物理公式（旋转 pivot 在 `(±1.9, 12, 0.1)`，错位量 ≈ `sin(θ)·2px`）、θ=55°→±1.64px "肉眼明显可见"、θ=40°→±1.29px "几乎看不出"、"pitch 控制在 ~40° 以内，优先用 bend 堆视觉强度"的硬性建议，以及 `sword_ride` 从 `pitch 55°+leg.z` 断连最严重迭代到 `pitch 40°+bend 105°` 接近无感的实证先例（L234）。meditate_sit 的 -80° 远超这条红线，`sin(80°)≈0.985`，错位量接近满幅 2px，比 sword_ride 最差版本的 55° 还严重。
- `client/src/main/java/com/bong/client/cultivation/BreakthroughSpectacleRenderer.java:26`：`PRELUDE` 阶段 `SpectaclePlan` 构造时把 `animationId` 硬编码为字符串 `"meditate_sit"`。
- `client/src/main/java/com/bong/client/network/BreakthroughCinematicHandler.java:184-198`：`triggerAnimation()` 取出 `plan.animationId()`，调用 `AnimationLayerManager.play(player, Identifier("bong", animId), Channel.FULL_BODY.priority(), DEFAULT_FADE_IN_TICKS)`，把该动画挂到 `FULL_BODY` 通道播放。
- `server/src/cultivation/breakthrough_cinematic.rs:165`：`BreakthroughCinematicState` 构造时起始阶段即为 `BreakthroughCinematicPhase::Prelude`（默认阶段，不是可跳过的可选分支）。
- `server/src/cultivation/mod.rs:379`：`start_breakthrough_cinematic_on_outcome.after(breakthrough_system)` —— 挂在生产 `Update` 调度里，跟随真实境界突破结算（`breakthrough_system`）之后触发，不是 dev-only 命令路径。

## 触发路径

1. 玩家自然修炼达到境界突破临界点（六境界醒灵→化虚任一次跃迁），`breakthrough_system` 结算成功。
2. `start_breakthrough_cinematic_on_outcome`（挂在 `breakthrough_system` 之后）创建 `BreakthroughCinematicState`，默认起始阶段为 `Prelude`（`breakthrough_cinematic.rs:165`）。
3. server 把 PRELUDE 阶段的 `BreakthroughCinematicPayload` 推给 client。
4. `BreakthroughSpectacleRenderer.plan()` 对 `PRELUDE` 分支构造 `SpectaclePlan`，`animationId` 硬编码为 `"meditate_sit"`（L26）。
5. `BreakthroughCinematicHandler.triggerAnimation()` 调 `AnimationLayerManager.play(...)`，把 `meditate_sit` 动画挂到 `FULL_BODY` 通道，播放时长最长 3 秒。
6. 该动画的 `rightLeg`/`leftLeg.pitch` 全程恒为 -80°（远超 40° 安全阈值），整段 PRELUDE 期间角色髋部-大腿接缝持续可见断裂。

## 反方审查记录

- 第一轮质疑：
  - 是否是笔误/未使用的废弃动画？——检查 `BreakthroughSpectacleRenderer.java:26` 确认 `"meditate_sit"` 是 PRELUDE 阶段生产路径实际使用的 animationId，非孤儿资产。
  - 是否只在 dev 命令/测试脚手架下触发？——检查 `cultivation/mod.rs:379` 确认 `start_breakthrough_cinematic_on_outcome` 挂在真实 `Update` 调度、跟随自然突破结算，六境界任一次突破都会经过，非 dev-only。
  - 是否文件本身数值有误读（比如单位不是弧度）？——`emote.degrees: false`（`meditate_sit.json:13`）确认全文件按弧度解析，`-1.3962634 rad × (180/π) = -80.0000...°`，换算无误。
  - 初裁：倾向通过，但需核对是否是"瞬时经过帧"从而豁免（转瞬即逝不算断腿风险）。
- 第二轮补证：
  - 逐 tick 核对：0/10/20/30/40 全部 5 个关键帧 pitch 恒定同值 -80°，`isLoop:true`，是**持续 held 姿势**（不是快速掠过的中间帧），断连缝隙有充分曝光时间，不满足"瞬时经过可豁免"的条件。
  - 全库横向扫描：`meditate_sit` 是全部含 `rightLeg`/`leftLeg` 数据的动画文件中 |pitch| 最大值（80°），高于 `death_collapse`(75°，倒地瞬时非循环)、`dodge_roll`(55°，翻滚瞬时非循环)、`sword_ride`(40°，恰好卡在阈值上限)、`npc_flee_run`(35°)，且是唯一"循环 + 持续渲染 + 超过阈值两倍"的组合。
  - 让步：这是纯视觉瑕疵（无功能性/守恒律/数据影响），不阻塞游戏进程；且文件本身 `description` 字段自称的角度（-90°/175°）与实际写入值（-80°/90°）本身就对不上，说明这条动画从未被真机严格核验过就已投入使用。
  - 查重：`docs/plans-skeleton/` 下未见同名或同主题骨架（`meditate`/`pitch`/`断腿` 相关关键词均无命中）；已知动画类 in-flight 条目（`nimai-huti-missing-animation`、`woliu-voidpath-missing-animations`、`playeranim-reconnect-stale-layer` 等）均是"缺动画"或"跨会话动画层残留"问题，与本 finding（"动画存在但腿部姿态数值违反自定安全阈值"）不重叠。
  - 终裁：通过。属于视觉资产纪律范围内的真实回归缺陷，且触发在游戏最核心的正反馈时刻（境界突破），持续可见性使其不该被当作"边角瑕疵"忽略。
- 主循环复核：已亲读关键行确认（`meditate_sit.json` 全文 822 行、`BreakthroughSpectacleRenderer.java:1-40`、`BreakthroughCinematicHandler.java:178-207`、`breakthrough_cinematic.rs` 相关行、`cultivation/mod.rs:354-380`、`docs/player-animation-conventions.md:199-234`）。

## Skeleton Fix Plan

> 本 bug 纯属客户端视觉资产（PlayerAnimator JSON 姿态数值），不涉及真元/灵气流动，不适用 qi_physics 守恒模式；也不涉及 C2S 请求，不适用 server gate 权威模式。修复遵循**视觉资产纪律**（3 轮打磨 + `<PROMISE>` 担保 + 渲染证据验收）。

- [ ] Round 1（first cut）：把 `meditate_sit.json` 的 `rightLeg`/`leftLeg.pitch` 从 `-1.3962634` rad（-80°）收进 `±40°`（`~0.6981317` rad）以内的安全值，全部 5 个关键帧（tick 0/10/20/30/40）同步改，保持数值一致（避免又踩"循环单帧衰减"的坑——所有用到的 axis 每个 keyframe 都要显式写同值）。
- [ ] 把 pitch 收窄后损失的"腿部平放"视觉目标，转移给 `bend`（当前恒 90°，可以视觉需要再加大，如收到 100-115° 区间，参照 `docs/player-animation-conventions.md:234` 的 `sword_ride v5`（`pitch 40°+bend 105°`）先例）和 `roll`/`axis`（当前 `roll=∓0.2617994` rad ≈ ∓15°，`axis=0`）重新分配，目标仍是文件 description 描述的"髋膝踝同水平、脚踝回到髋正下方"的盘坐造型。
- [ ] 顺手核对/修正文件自身 description 字段与实际写入数值不一致的问题（现状声称 -90°/175°，实际是 -80°/90°），改完后 description 应如实反映新数值，避免继续误导后续维护者。
- [ ] Round 2（自评）：用 `client/tools/render_animation.py client/src/main/resources/assets/bong/player_animation/meditate_sit.json` 渲染三视图 PNG，核对髋部-大腿接缝是否仍有断裂缝隙可见；对照 `docs/player-animation-conventions.md §7.2` 的错位公式（`sin(θ)·2px`）估算新角度下的理论错位量，确认落在"几乎看不出"区间。
- [ ] Round 3（终轮）：与 spec（"庄严盘坐、髋膝踝同水平"）核对视觉叙事一致性，必要时微调 `yaw`/`axis` 保证左右腿对称；确认循环首尾帧（tick 0 与 tick 40）数值一致以避免 loop 跳变（当前设计已经是 tick0=tick40，保持）。
- [ ] 终轮 commit message 末尾附 `<PROMISE>` 担保块：写明"已 3 轮打磨"、"已检查[髋部接缝无断裂 / 循环首尾帧一致 / 盘坐视觉叙事符合 description / pitch 落在 ±40° 内]"、"仍存局限[...]"（如实填写，不夸大）。
- [ ] 顺手扫描 `player_animation/` 目录下是否还有其他**循环且持续渲染**的动画同样违反 40° 阈值（本 finding 的横向扫描已确认 `death_collapse`/`dodge_roll`/`npc_flee_run` 是瞬时非循环、`sword_ride` 恰好卡线未超标，暂无发现其他同类问题，但修复时应重新跑一遍确认，因为改动 `meditate_sit` 本身可能促使连带核查同源姿态模板）。

## 验收测试计划

本 bug 是纯视觉资产回归，没有对应的 server/client 自动化断言可以直接锁住"像素级接缝是否可见"，因此验收以**渲染证据 + 数值断言**双轨走：

- **渲染证据（强制，视觉资产纪律要求）**：
  - 用 `python3 client/tools/render_animation.py client/src/main/resources/assets/bong/player_animation/meditate_sit.json -o /tmp/anim_render/meditate_sit_fixed` 对修复后的 JSON 出三视图 PNG（正面/侧面/背面），人工核对髋部-大腿接缝无可见断裂缝隙。
  - 若脚本支持逐 tick 抽样（0/10/20/30/40），逐帧核对循环内无姿态跳变。
- **数值断言（client JUnit 或轻量脚本测试，可选新增）**：
  - happy path：`meditate_sit.json` 中 `rightLeg.pitch`/`leftLeg.pitch` 的绝对值在全部关键帧 ≤ `0.6981317` rad（40°）。
  - 边界：若引入通用"动画姿态 lint"校验（可选延伸），对 `player_animation/*.json` 中标记为 `isLoop:true` 的文件，遍历全部 `rightLeg`/`leftLeg.pitch` 关键帧，断言 `|pitch| ≤ 0.6981317` rad；非循环（单次播放）动画允许放宽阈值但仍建议校验不超过既有 `death_collapse`(75°)/`dodge_roll`(55°) 的历史上限，作为回归防线。
  - 状态转换：确认 tick 0（循环起点）与 tick 40（循环终点，`endTick`）的 `rightLeg`/`leftLeg` 全部轴值（pitch/yaw/roll/bend/axis）严格相等，防止改动引入首尾不一致导致的循环跳变（这是 CLAUDE.md 记载的"循环动画单帧衰减"坑的姊妹检查，虽然本文件已用显式关键帧规避了该坑，但改动时必须保持这一性质）。
  - 错误分支：若新增 lint 脚本，需覆盖"关键帧数量不完整"（如某个 tick 缺失某个 axis 的显式 keyframe，可能触发 isLoop 衰减回 defaultValue 的坑）的检测用例。
- **回归确认**：`BreakthroughSpectacleRenderer.java:26` 的 `animationId` 字符串不变（仍是 `"meditate_sit"`），只改 JSON 内部姿态数值，因此不需要改 server/schema/网络契约，也不需要新增跨端集成测试；`cultivation::` 相关 server 单测（境界突破流程本身）不受本次纯客户端资产改动影响，无需重跑 server 门禁，但按仓库约定改动范围内仍建议跑一次 `cd client && ./gradlew test build` 确认资源加载无 JSON 语法错误。

## 风险

- 纯视觉资产改动，不涉及真元/灵气/守恒律，不涉及 C2S 契约，风险面很窄——主要风险是"改完 pitch 之后 bend/roll/axis 分配不当，出现新的姿态怪异"（3 轮打磨 + 渲染证据要求正是为了兜住这条）。
- 如果只调小 pitch 而不相应加大 `bend` 承担视觉强度，"盘腿打坐"的既有视觉叙事（髋膝踝同水平）可能被削弱变成"半蹲"，需要 Round 2/3 渲染核实。
- 本 plan 不解决"其他动画文件是否也有类似问题"——已在 Fix Plan 末尾列为顺带核查项，但不在本次验收范围内，避免范围蔓延；若发现新的同类实例应另开 skeleton。
