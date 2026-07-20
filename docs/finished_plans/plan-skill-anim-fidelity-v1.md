# plan-skill-anim-fidelity-v1 —— 梯队二：施法动画精度重制 + 招式 A/V 去复用差异化

> 一句话主题：现有技能动画普遍是「4-16 tick 一次性快闪 + 3-4 个关键帧」的低精度产物，且大量招式借用别家动画/粒子——本 plan 建立动画精度标准（三段式结构 / 时长对齐 cast_ticks / 关键帧密度红线），分批重制主力招式动画，并给复用别家 A/V 的招式补专属资产，让每招从远处可辨。
>
> 用户拍板需求（2026-07-17）：「第三人称一些 technique 是很快的一次播放，我需要动作精度高」。
>
> 调研来源：2026-07-17 三路并行探查 + 全量动画 JSON 解析（基线 `origin/main` = `062cf636`）。

## 现状证据（量化实锤）

对 `client/src/main/resources/assets/bong/player_animation/*.json` 全量解析（tick 数 / 帧点数 / 轴关键帧数）：

**① 快闪短动画**（20 TPS 下 0.2-0.5 秒播完，肉眼就是「一闪而过」）：

| 动画 | endTick | 帧点 | 轴关键帧 |
|---|---|---|---|
| zhenmai_parry | 4 | 3 | 13 |
| zhenmai_neutralize | 6 | 3 | 12 |
| zhenmai_harden | 7 | 3 | 11 |
| beng_quan | 8 | 4 | 36 |
| dugu_needle_throw | 8 | 4 | 35 |
| woliu_vacuum_palm | 8 | 3 | 18 |
| release_burst（anqi 多招借用） | 4 | 3 | 81 |
| sword_thrust | 10 | 4 | 20 |

对照组（认真做过的）：`guangbo_ticao` 150 tick / 18 帧点 / 288 轴关键帧；`breakthrough_burst` 60/6/78。

**② cast_ticks ↔ 动画时长错配**（引导后半程角色站桩发呆）：

| 招式 | cast_ticks | 实际动画 | 差距 |
|---|---|---|---|
| anqi.echo_fractal | 60 | release_burst = 4 tick | 56 tick 静止 |
| anqi.armor_pierce | 40 | cast_invoke = 15 tick | 25 tick 静止 |
| anqi.multi_shot | 30 | release_burst = 4 tick | 26 tick 静止 |
| sword_path.resonance | 30 | 借 sword_cleave = 16 tick | 14 tick 静止 |
| morph.yixing | 60 | morph_cast = 30 tick | 30 tick 静止 |

正例：`woliu.vortex_resonance` cast=80 ↔ 动画 80 tick isLoop ✓（这是应有形态）。

**③ 模板批量产物**：一批动画呈「81 轴关键帧 = 27 轴 × 首/中/尾 3 帧」特征（`palm_strike`/`release_burst`/`parry_block`/`sword_slash_down`/`dodge_roll`/全部 `stance_*` 等），是生成器模板一把梭的产物，所有轴同节奏起落，无重心转移、无预备-发力-收势的时序错落。

**④ A/V 复用清单**（去复用 = 本 plan 差异化目标）。**以下为 P0 立项时的基线快照，保留以备追溯；括号内标注各条的清偿阶段**：

- 动画复用：sword_path `condense_edge`/`resonance` 借 `sword_cleave`、`qi_slash` 借 `sword_thrust`；anqi 6 招全借通用（`windup_charge`/`cast_invoke`/`release_burst`/`sword_stab`）；burst_meridian `tie_shan_kao`/`xue_beng_bu` 借 `beng_quan`、`ni_mai_hu_ti` 无动画（anim_id: None）。（**已清偿**：sword_path / anqi 归 P2 ✅、burst_meridian 3 招归 P3 ✅）
- 粒子复用：zhenmai 直接复用剑气 `SwordQiSlashPlayer`（`jiemai_*` 事件）；burst_meridian 全系共用 `bong:burst_meridian_beng_quan`；npc 3 招各借医道/真脉/崩拳粒子。（**已清偿**：全部 11 条归 P5 ✅ 2026-07-20——zhenmai 5 招转 `ZhenmaiPulsePlayer`、burst_meridian 3 招转 `BurstMeridianFamilyPlayer`、npc 3 招转 `NpcSkillAuraPlayer`，逐项接线见 P5.2 矩阵）
- yidao 5 招：plan-yidao-v1 §5 表格承诺的 5 个差异化动画（针灸/灸火/CPR/续命咏唱/环阵）完全未落地（Finish Evidence 只兑现 audio+VFX+HUD）。

## 与既有 plan 的关系（防重声明）

- **涡流虚蚀 5 招动画**归 active `plan-bughunt-woliu-voidpath-missing-animations-v1`，本 plan 范围显式排除；其验收的「五招可辨识姿态差异」标准与本 plan §精度标准一致，落地时应引用。
- **`plan-bughunt-woliu-resonance-loop-arm-decay-v1`（skeleton，#1038 循环动画单帧衰减）**：属 PlayerAnimator 库坑 #1 的存量 bug，归 bugfix 流程；本 plan 的精度标准把「循环动画每轴必须在 endTick 补同值关键帧」写成新作红线，防再犯。
- **`plan-bughunt-dugu-penetrate-av-mismatch` / `plan-bughunt-baomai-v3-av-double-source-v1` / `plan-bughunt-tuike-v2-duplicate-av-v1`（skeletons）**：A/V 错发/双源类 bug 归各自 bugfix，本 plan 只管资产精度与差异化，不修发射逻辑 bug。
- 孤儿动画接线、图标重链归 `plan-skill-av-relink-v1`（梯队一）；第一人称手臂动画归 `plan-fpv-cast-av-v1`（梯队三）。本 plan 产出的重制动画天然成为梯队三 FPV 变体的底稿，建议排期在梯队三之前。

## 接入面 checklist

- **进料**：`server/src/cultivation/known_techniques.rs` 的 `cast_ticks`/`cooldown_ticks`（动画时长对齐基准）；`server/src/network/vfx_animation_trigger.rs:42-137` const 映射表；既有动画 JSON 与 `client/tools/render_animation.py` / `gen_*.py` 工具链。
- **出料**：重制后的 `assets/bong/player_animation/*.json`（同名覆盖，server 映射表零改动或仅新增专属 id）；新增专属粒子 `VfxPlayer` 注册进 `VfxBootstrap`；`vfx_animation_trigger.rs` 里借用别家动画的 arm 改指新专属 id。
- **共享类型/event**：全部复用 `VfxEventPayloadV1::{PlayAnim,SpawnParticle}`，不新增 schema；新粒子只新增 event_id 字符串 + client 注册，走既有 `bong:vfx_event` 通道。
- **跨仓库契约**：server `vfx_animation_trigger.rs`/各 resolver ↔ client `BongAnimationRegistry`/`VfxBootstrap`；agent 不参与。
- **worldview 锚点**：worldview.md §四 招式物理可见性（读招/反应/复盘依赖姿态可辨）；招式 A/V 差异化红线（CLAUDE.md）。
- **qi_physics 锚点**：不涉及——纯表现层。gameplay 数值（cast_ticks/伤害/冷却）一律不动，见 §8 #1。

## 动画精度标准（本 plan 的验收红线，后续所有新招动画沿用）

1. **三段式结构**：anticipation（蓄势）→ strike/active（发力）→ recovery（收势），每段至少 2 个帧点；打击定格（hold 2-4 tick）算 strike 段内。
2. **时长对齐**：非循环招 `endTick = cast_ticks + recovery(4-8 tick)`——cast 完成瞬间是发力顶点，之后收势；`cast_ticks ≥ 40` 的长引导招必须拆「循环蓄力段（isLoop）+ release 段」两段动画（`sword_heaven_gate_charge/_release` 先例）；`cast_ticks ≤ 2` 的瞬发招（zhenmai.parry、baomai 双段、woliu 短招）做 6-12 tick「爆发帧 + 收势」，不因 cast 短而砍收势。
3. **关键帧密度**：主要运动轴每 ≤4 tick 一个帧点；easing 必须显式声明，主打击轴禁用 linear（蓄势用 easeOut 族、发力用 easeIn 族、收势 easeInOutSine）。
4. **重心与全身协调**：发力招必须有 torso 拧转 + body 位移（不许只挥手臂）；弯腰姿态走 torso+legs 同向 pitch + body.z 补偿（torso/legs 不共祖）。
5. **库坑红线**（违反即打回）：循环动画每个用到的轴在 endTick 补同值关键帧（防单帧衰减）；`leg.pitch ≤ 40°`，大幅度腿部动作由 `bend` 承担；整体位移/旋转用 `body.*`，上半身独立扭转用 `torso.yaw`。
6. **工具流**：动画一律脚本生成（`client/tools/gen_<anim>.py`，参照 `gen_fist_punch_right.py` 先例）便于参数化迭代；每批走 `render_animation.py` 三视图 headless 预览；按视觉资产纪律 3 轮打磨 + 终轮 `<PROMISE>` 担保。

**范例 spec（P1 首个交付物 sword.cleave 重制，展示本 plan 要求的书写精度；发力顶点 = cast_ticks=16，与标准 #2 一致）**：

- 总长：cast 16 + recovery 8 = endTick 24，stopTick 27，非循环。
- anticipation 0→10：右臂 pitch 0→-150°（举过头顶）、torso.yaw 0→-20°（拧腰）、torso.pitch 0→-5°、左臂 pitch 0→-30°（平衡），easeOutQuad，段内帧点 0/5/10。
- strike 10→16：右臂 pitch -150°→+55°（过身下劈）、torso.yaw -20°→+15°、torso.pitch -5°→+18°（前压）、body.z +0.3（前送）、前腿 pitch 30° + bend 12°（弓步），easeInCubic，顶点帧 = tick 16（cast 完成瞬间）。
- hold 16→19：全轴保持（打击定格）。
- recovery 19→24：全轴回 defaultValue，easeInOutSine。

## 阶段总览

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | 全量审计矩阵落档 + 精度标准定稿 + 时长对齐自动对拍测试 | ✅ 2026-07-18 |
| P1 | 批次一重制：sword 基础 3（infuse 移 P2，见附录 A）+ beng_quan + zhenmai 5（高频主力短招） | ✅ 2026-07-19 |
| P2 | 批次二：sword_path 5 专属化 + anqi 6 专属化 + sword_infuse 两段式（去复用 + 长引导循环段）。拆前半（2026-07-19：去复用 6 招专属化——sword_path condense_edge/qi_slash/resonance + anqi single_snipe/multi_shot/soul_inject，含 server 映射改指 + allowlist 删 5 条）+ 后半（2026-07-19：charge_carrier / sword.infuse 真两段式 + StopAnim 三类通道接线 §8.1 #3 + echo_fractal / armor_pierce / manifest 瞬发结算型分类契约（review r2 定形——strike 顶点=tick 0 与结算同帧，INSTANT_RESOLVER_SKILLS + instant manifest 机械锁，出 allowlist）+ heaven_gate 双段密度精修 + allowlist 净删 4 条） | ✅ 2026-07-19 |
| P3 | 批次三：burst_meridian 3 借用招专属化 + ni_mai_hu_ti 新增 + dugu 2 / tuike 3 / woliu 短招精修 | ✅ 2026-07-19 |
| P4 | yidao 5 招动画补齐（plan-yidao-v1 §5 欠账） | ✅ 2026-07-19 |
| P5 | 粒子去复用：zhenmai 专属 player + burst_meridian 家族分化 + npc 3 招分化 | ✅ 2026-07-20 |
| P6 | 回归收口：资源 pin 测试 + FPV/TPV 实机验收 + 两项裁决（heaven_gate 对齐口径 / 两段式相位承接契约）+ 架势亮相遗留清偿 | ✅ 2026-07-21 |

## P0 — 审计矩阵 + 标准定稿 + 对拍测试

- 49 招全量矩阵表落档到本 plan 附录：`skill_id / cast_ticks / anim_id / endTick / 帧点数 / 轴关键帧数 / 是否模板产物 / 是否借用 / 差距分级（A 达标 / B 精修 / C 重制 / D 缺失）`。
- **时长对齐自动对拍测试**（client 侧）：读 `player_animation/*.json` 元数据 + 一份 `cast_ticks` 快照表，按招式类型分三套断言（与精度标准 #2 严格同一时序模型，不用宽区间混过）：
  - 普通非循环招（2 < cast < 40）：`endTick ∈ [cast+4, cast+8]`（recovery 红线直接入断言）；
  - 瞬发招（cast ≤ 2）：总长 ∈ [6, 12]；
  - 长引导招（cast ≥ 40）：蓄力段动画 isLoop 且每个用到的轴在 endTick 有同值补帧（库坑 #1 边界）、release 段动画独立存在且两段 id 均被 server 映射表发射。
- **精度红线机械化断言**（同一测试套件）：每份动画随批提交一份结构化 spec manifest（anticipation/strike/recovery 的 tick 边界 + 每段帧点数），测试逐项断言：三段各 ≥2 帧点、主要运动轴相邻帧点间隔 ≤4 tick、所有关键帧 easing 显式且主打击轴非 linear、`leg.pitch ≤ 40°`、循环动画每轴 endTick 同值补帧。无法机械判定的重心转移/全身协调，列为逐招人工验收证据：批次 PR 附 `render_animation.py` 三视图 PNG + 对照 checklist。
- **快照单一真源**：快照由 server `TECHNIQUE_DEFINITIONS`（P4 起并入 `yidao_skill_spec`，具名双表有序并集，见 §8.1 #4a）单向生成（server 侧同步测试保证快照=定义，快照缺失/重复/漂移条目直接判红），client 测试只消费不维护——杜绝「错误时长靠同步改快照混过关」。现状不达标项进 allowlist，逐批清空——allowlist 清零 = P1-P4 完成的机械判据（**口径见 §8.1 #5**：只覆盖属于 P1-P4 重制清单的条目，明示归属外部 plan / 后续阶段的余项不计入但必须登记归属）；**allowlist 只允许缩小**，任何新增条目必须在 PR body 显式说明理由。
- 精度标准（上节）随 P0 一并进 `docs/player-animation-conventions.md`（该文档为动画约定正典，本 plan 允许追加不允许改写既有段落）。

## P1-P4 — 分批重制（每批同构）

每批交付物：`gen_<anim>.py` 生成脚本 + 重制 JSON + `render_animation.py` 三视图对照（round 1/2/3 commit）+ 终轮 `<PROMISE>` 块 + allowlist 对应条目删除。批内每招须给出 P1 范例 spec 同精度的骨骼数值表（写在各批 PR body，plan 只锁标准与清单）。

- **P1**（高频短招，玩家看得最多）：`sword_{cleave,thrust,parry}`、`beng_quan`、`zhenmai_{parry,neutralize,multipoint,harden,sever_chain}`。瞬发招按标准 #2 做爆发帧+收势。（`sword_infuse` cast=40 属长引导域，移 P2，见附录 A。）
- **P2**（去复用 + 长引导）：sword_path 5 招各自专属动画（`condense_edge` 凝锋收剑入鞘式 / `qi_slash` 远程挥斩 / `resonance` 双手持剑共鸣颤 / `manifest` 已有 / `heaven_gate` 已有两段式，精修）；anqi 6 招专属（`charge_carrier` cast=400 → 循环封骨结印段 + 完成收势；`echo_fractal` cast=60 → 循环撒饵段 + 4 tick 爆发保留为 release）；`sword_infuse` cast=40 拆「循环蓄力段 + release 段」两段式（含 server 通道接线）。
- **P3**：`tie_shan_kao`（靠身撞击，与崩拳出拳区分）、`xue_beng_bu`（步法位移）、`ni_mai_hu_ti`（护体结印，当前 anim_id: None 补新）、dugu 2 / tuike 3 / woliu **基础与进阶**短招（`vacuum_palm`/`woliu.burst` 等 8-10 tick 快闪项）按标准精修。**明确排除涡流虚蚀 5 招**（`ambient_vortex`/`void_vortex`/`swallowing_vortex`/`vortex_echo`/`void_core`——其动画从无到有的补齐归 active `plan-bughunt-woliu-voidpath-missing-animations-v1`）；若将来需对其产物做二次精修，作为该 plan merge 后的后置依赖另列批次，且只改既有 JSON 精度、不新增动画、不动发射链。
- **P4**：yidao 5 招按 plan-yidao-v1 §5 表格逐条落地（针灸双手持针 30 穴位序 / 灸火对掌 / CPR 按压 / 续命喂丹+接天引 / 环阵持法器），server 侧 yidao emit 补 `PlayAnim`（当前 yidao 无动画常量）。（✅ 2026-07-19 交付：5 招通道核验全部为 `resolve_yidao_skill → insert_casting` 真实长引导窗（cast_ticks_base 100-1200t，`yidao_cast_ticks` 按 mastery/平和色缩放可变窗）→ 全部两段式——蓄力循环段 isLoop 覆盖任意窗长（逐招时长：接经术 **90t**（30 针×3t，r2 返工兑现 30 穴位序后的最终值）/ 排异 24t / 急救 20t / 续命 26t / 群体接经 32t）+ release 收势段 12-14t 三段式；server 10 动画 id 常量 + 起手 PlayAnim + `looping_cast_anim_id` yidao 分表登记（三打断/自然完成 StopAnim）+ `complete_yidao_casts` 有效结算分支 release 接力（无效完成不奖励收势，sword_infuse 先例同语义）；cast_ticks 快照真源扩展为 `TECHNIQUE_DEFINITIONS` + `yidao_skill_spec` 单向合并；client `SKILL_ANIM` +5、10 份 spec manifest，allowlist 零新增。）

## P5 — 粒子去复用

按 docs/CLAUDE.md §四 视听精度要求逐条写 spec 再实施（基类/数量/lifetime/速度方向/颜色 hex/spawn 模式/贴图复用或新增/VfxPlayer 类名/event_id）：

- zhenmai：弃借 `SwordQiSlashPlayer`，新建 `ZhenmaiPulsePlayer`（`BongLineParticle` 短脉冲 + `BongSpriteParticle` 穴位点，色系 #D4AF6A 金脉），5 招各自 event_id（`bong:zhenmai_{parry_flash,neutralize_dust,multipoint_ring,harden_shell,sever_snap}`），贴图复用既有 `qi_aura`/`lingqi_ripple` 不新增。
- burst_meridian：`tie_shan_kao` 撞击冲击环（GroundDecal）、`xue_beng_bu` 步法残影（Ribbon 短尾）、`ni_mai_hu_ti` 体表逆流纹（Sprite 环绕），共用色系 #C58B3F 但形态分化。
- npc 3 招：脱离借用，各给独立 event_id（形态可简，但 id 与颜色必须独立，保证旁观读招）。
- **双端闭环接线矩阵（P5 交付物）**：每个新 event_id 一行——`招式 id / server 发射点（resolver 或 emit system 文件）/ SpawnParticle event_id / client VfxPlayer 类名 / VfxBootstrap 注册行`。矩阵同源生成一份共享 event_id 清单驱动双端测试：server 侧逐项断言对应招式事件发出正确 `SpawnParticle`（旧借用 id 不再发出的负向断言一并锁）；client 侧逐项断言 `VfxRegistry` 已注册同一 id 并返回预期 player；再加集合一致性断言（发射集合 == 注册集合，防两端字符串各自漂移）+ 未注册 id 走 bridgeMiss 不崩溃的错误分支。

### P5.1 粒子 spec（按 docs/CLAUDE.md §四 视听精度要求，实施前定稿 2026-07-20）

**全域共同约束**：贴图**零新增**——全部复用既有 `BongParticles` sprite provider（`qi_aura` → `qiAuraSprites`、`lingqi_ripple` → `lingqiRippleSprites`）；不新增 schema，全部走既有 `VfxEventPayloadV1::SpawnParticle` → `bong:vfx_event` → `VfxRegistry` → `VfxPlayer` 通道；server 侧 `count` 参数驱动主粒子数，副粒子（穴位点等）数量由 player 按形态自持。

**① zhenmai 5 招 —— `ZhenmaiPulsePlayer`（金脉色系 anchor #D4AF6A）**

基类组合固定为「`BongLineParticle` 沿经脉走向的短脉冲 + `BongSpriteParticle` 驻留穴位点」，逐招靠**运动形态 + 金脉明度阶梯**分化（同族可认、逐招可辨）：

| event_id | 招式 | 运动形态 | 脉冲数 | 穴位点数 | lifetime | 主速度 格/t | 脉冲垂直 格/t | 穴位点漂移 格/t | 半径 格 | 颜色 hex | spawn 模式 | 贴图（复用） |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `bong:zhenmai_parry_flash` | `zhenmai.parry` | FORWARD | 8 | 3 | 20t | 0.35 | 0.00 | 0.00 | 0.55 | `#D4AF6A`（金脉本色） | burst 单帧齐发 | `qi_aura` + `lingqi_ripple` |
| `bong:zhenmai_neutralize_dust` | `zhenmai.neutralize` | RADIAL_OUT | 10 | 4 | 20t | 0.12 | -0.02 | -0.02 | 0.40 | `#C9A05C`（沉金，卸力散尘） | radial 水平环 | 同上 |
| `bong:zhenmai_multipoint_ring` | `zhenmai.multipoint` | ORBIT | 16 | 8 | 20t | 0.10 | 0.00 | 0.00 | 0.50 | `#E0C27E`（亮金，多点连环） | radial 腰高真环绕，穴位点等角分布 | 同上 |
| `bong:zhenmai_harden_shell` | `zhenmai.harden` | RADIAL_IN | 8 | 6 | 20t | 0.06 | 0.01 | 0.01 | 0.45 | `#B8944F`（暗金，硬化沉坠） | radial 双层护壳 | 同上 |
| `bong:zhenmai_sever_snap` | `zhenmai.sever_chain` | FORWARD | 18 | 2 | 20t | 0.55 | 0.00 | 0.00 | 0.70 | `#F2D68A`（最亮金，断脉爆闪） | burst 爆闪 | 同上 |

**列语义**：`运动形态` = `ZhenmaiPulsePlayer.Form.Motion` 枚举常量名。`主速度` 在 FORWARD 下是沿 `direction` 的前冲速率、RADIAL_* 下是径向速率（RADIAL_IN 为向心，符号在实现里取负）、ORBIT 下是**线**速度（角速度 = 线速度 ÷ 半径）。`脉冲垂直` 是脉冲的固定垂直分量（FORWARD 形态例外：垂直分量来自 `direction` 本身，可俯可仰，故列 0.00）。`穴位点漂移` 是穴位点的垂直速度——**穴位点 lifetime 与脉冲严格一致**，不额外延长。`半径` 是脉冲起始铺开半径（ORBIT 下即环绕半径；穴位点铺在其 0.6 倍处）。

明度阶梯 `harden(#B8944F) < neutralize(#C9A05C) < parry(#D4AF6A) < multipoint(#E0C27E) < sever(#F2D68A)` 与招式烈度同序。

> **review r1 修正（2026-07-20）**：本表此前把 `穴位点` 的行为混在散文里（parry 行写「穴位点静止」），而实现对五招统一给了 `lifetime + 6` 与 `vy = 0.008` 上浮——spec 与实现两处静默不一致。现拆出 `穴位点漂移` 独立数值列并逐招定值，lifetime 收敛为「与脉冲一致」。同时 `zhenmai.multipoint` 的「切向环绕」此前实现只设切向初速度（= 切向抛射，粒子沿切线直线飞离），现改真环绕（见 §P5.1 ⑥）。

**② burst_meridian 3 招 —— `BurstMeridianFamilyPlayer`（共用色系 `#C58B3F`，纯形态分化）**

三招颜色**一律 `#C58B3F`**（与既有 `beng_quan` 本尊同色，统一爆脉家族识别色），读招完全靠形态：

| event_id | 招式 | 基类 | 运动形态 | 数量 | count 下限 | lifetime | 主速度 格/t | 半径 格 | 垂直速度 格/t | 自旋 rad/t | 颜色 hex | spawn 模式 | 贴图（复用） |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `bong:burst_meridian_tie_shan_kao` | `burst_meridian.tie_shan_kao` | `BongGroundDecalParticle` | IMPACT | 10 | 4 | `cast_ticks`（缺省 16t） | 0.00 | 0.30 + 0.45×strength | 0.00 | 0.05 | `#C58B3F` | radial 地面撞击冲击环 | `lingqi_ripple` |
| `bong:burst_meridian_xue_beng_bu` | `burst_meridian.xue_beng_bu` | `BongRibbonParticle` | TRAIL | 12 | 2 | `cast_ticks`（缺省 14t） | 0.25 | 不适用 | 0.01 | 不适用 | `#C58B3F` | continuous 步法短尾（沿 `direction` **反向**，残影落身后） | `qi_aura` |
| `bong:burst_meridian_ni_mai_hu_ti` | `burst_meridian.ni_mai_hu_ti` | `BongOrbitSpriteParticle` | ORBIT | 14 | 4 | 12t（= 重发间隔） | 0.08 | 0.55 | 0.015 | 不适用 | `#C58B3F` | radial 双高度体表逆流纹（真环绕，随施法者重发） | `qi_aura` |

**列语义**：`运动形态` = `BurstMeridianFamilyPlayer.Form.Motion` 枚举常量名。`lifetime` 由 server `emit_burst_av` 下发（前两招 = `cast_ticks`，护体 = `NI_MAI_HU_TI_AURA_PARTICLE_LIFETIME_TICKS` = 12，即**单个环**的寿命而非整个 buff 窗口，见下），括号内是 payload 缺 `duration_ticks` 时的 client 回退值。`count 下限` 是 payload `count` 异常偏小时的钳制下界——环形招 4（低于此读不出「环」），线性残影 2 即可成束；上限统一 48。撞击环半径吃 `strength`（server 传 0.95）；`不适用` 的格子在实现里取 0。逆脉护体的双高度环：奇数颗在 `origin.y - 0.25`、偶数颗在 `origin.y + 0.45`。

> **体表锚定契约（review r2 #1 收口，2026-07-20）**：`ni_mai_hu_ti` 的逆流纹**跟随施法者移动**——plan P5 正文「**体表**逆流纹」是字面交付物，脱离身体即未兑现。
>
> `SpawnParticle` payload 只有世界坐标 `origin`、无实体标识，单个粒子环无从锚定移动中的身体；给 payload 加实体字段属跨端 schema 契约变更，越出 P5「不新增 schema」的共同约束。故走 **server 侧 buff 存续期周期重发**：
>
> - `NiMaiHuTiAura { started_at_tick, expires_at_tick }`（`cultivation/burst_meridian.rs`）由 `resolve_ni_mai_hu_ti` 在施放时挂上，窗口 = `NI_MAI_HU_TI_BUFF_DURATION_TICKS`（60t），与减伤 buff 严格同步。**不复用 `StatusEffects`**：`StatusEffectKind::DamageReduction` 是共享 kind（渡劫丹 / NPC `buff_defense` 同写，`upsert_status_effect` 按 kind 取 max 合并），读它会给嗑了渡劫丹的玩家凭空挂上爆脉护体环。
> - `ni_mai_hu_ti_aura_vfx_tick`（注册于 `network/mod.rs`，`.before(vfx_event_emit::emit_vfx_event_payloads)`）在窗口内每 `NI_MAI_HU_TI_AURA_REEMIT_INTERVAL_TICKS` = 12t 以施法者**当前 `Position`** 重发一个环；到期即摘锚点停发。
> - **环寿命 == 重发间隔**（12t）：老环恰在新环生成的同 tick 消失，既不叠环也不留空窗。`60 = 5 × 12` 整除，故 cast 首环 + 4 次重发正好铺满窗口，最后一环与 buff 同 tick 结束，不留「护体已过而纹还在转」的尾巴。
> - 首环（`emit_burst_av`）与重发环共用同一组形态常量，`p5_ni_mai_hu_ti_cast_ring_and_reemit_ring_share_one_form_spec` 断言两者除 `origin` 外逐字段相等。
>
> 半径恒 0.55 格由 `OrbitPath` 参数化保证（不是力平衡涌现），故不存在纹路逐渐外扩的退化。

**③ npc 3 招 —— `NpcSkillAuraPlayer`（形态从简，id + 颜色强制独立）**

| event_id | 招式 | 基类 | 运动形态 | 数量 | lifetime | 主速度 格/t | 半径 格 | 垂直速度 格/t | 颜色 hex | spawn 模式 | 贴图（复用） |
|---|---|---|---|---|---|---|---|---|---|---|---|
| `bong:npc_heal_basic` | `npc.heal_basic` | `BongSpriteParticle` | COLUMN | 12 | 40t | 0.00 | 0.4 | 0.03 | `#A8E6CF`（薄荷绿） | radial 上升柱 | `qi_aura` |
| `bong:npc_buff_speed` | `npc.buff_speed` | `BongLineParticle` | OUTRUSH | 12 | 40t | 0.22 | 0.3 | 0.01 | `#E3C766`（麦黄） | radial 疾行尘 | `qi_aura` |
| `bong:npc_buff_defense` | `npc.buff_defense` | `BongOrbitSpriteParticle` | ORBIT | 12 | 40t | 0.06 | 0.6 | 0.00 | `#5BA8C9`（青蓝） | radial 护体环（真环绕） | `lingqi_ripple` |

**列语义**：`运动形态` = `NpcSkillAuraPlayer.Form.Motion` 枚举常量名。`数量` / `lifetime` 与 server `emit_npc_skill_av` 下发的 `count: Some(12)` / `duration_ticks: Some(40)` 一致，列出的是 payload 缺省时的 client 回退值（count 钳制区间 1–48）。`半径` 是绕身铺开半径（ORBIT 下即环绕半径）。回血柱另按 `i % 4` 分四段身高错落（起点 `origin.y - 0.5`，步进 0.22），疾行尘贴 `origin.y - 0.6` 脚边。

> **buff_speed 配色变更（本阶段有意决策，非漂移）**：旧值 `#9FD8C8` 与 heal 的 `#A8E6CF` 同属淡青绿、仅单通道差 ~10%，远距离不可辨——直接违背本条「颜色必须独立，保证旁观读招」的交付要求。改为麦黄 `#E3C766`，令三招构成 绿 / 黄 / 蓝 高分离色相三元组。heal 与 defense 颜色保持不变。

**④ 优先级档位（`vfx_event_emit::vfx_default_priority`）**

- zhenmai 5 个新 id 命中既有 `bong:zhenmai_` 前缀 → 自动 Important，**无需登记**（旧 `bong:jiemai_*` 同为 Important，档位无变化）。
- burst_meridian：新增 `"bong:burst_meridian_"` 到 `PLAYER_SKILL_VFX_PREFIXES`。顺带修掉既存漏网——`bong:burst_meridian_beng_quan` 此前既不在前缀表也不在散号表（散号表登记的是 `bong:beng_quan`），实际掉在 Normal 档，与「玩家主动施放技能粒子归 Important」的设计意图不符；本阶段随家族前缀一并归位。
- npc 3 个新 id **不登记**，落 Normal 档：NPC 施法属背景 cosmetic，不应与玩家技能反馈争抢拥挤 chunk 的粒子配额。注意这是相对现状的**有意降档**——旧借用 id 中 `bong:yidao_meridian_repair` / `bong:jiemai_neutralize_dust` 因借用而误吃 Important，去借用后回归正确档位。

**⑤ 旧借用 id 的去向**

| 旧 id | 去借用后是否仍有发射方 | 处置 |
|---|---|---|
| `bong:jiemai_burst_blood` | 否（原仅 zhenmai parry + multipoint） | 删 client 注册，成为死 id |
| `bong:jiemai_neutralize_dust` | 否（原 zhenmai neutralize + harden + npc buff_speed） | 删 client 注册，成为死 id |
| `bong:jiemai_sever_flash` | **是** —— `network/meridian_severed_emit.rs` 被动断脉仍发 | **保留** client 注册与 `bong:jiemai_` 前缀 |
| `bong:burst_meridian_beng_quan` | 是 —— `beng_quan` 本尊 | 保留（`BurstMeridianBengQuanPlayer` 不动） |
| `bong:yidao_meridian_repair` | 是 —— `combat/yidao.rs` 医道本尊 | 保留 |

**⑥ 环绕形态的实现契约（review r1 #2 收口，2026-07-20）**

P5 首版把三处「环绕」（`zhenmai.multipoint` / `burst_meridian.ni_mai_hu_ti` / `npc.buff_defense`）实现成**只设切向初速度**——粒子沿切线直线飞离，到中心距离随 tick 线性增长（护体 60t × 0.08 ≈ 4.8 格，早飞出体表几个身位），是切向抛射不是环绕。plan P5 正文「体表逆流纹（Sprite 环绕）」是字面交付物，故按真环绕重做而非改描述降级。

- **`OrbitPath`（新增，纯 Java 无 MC 依赖）**：位置参数化为 `圆心 + 半径 × (cos θ, sin θ)`，θ 每 tick 加 `角速度 = 线速度 ÷ 半径`；垂直分量按 `verticalSpeed × 已过 tick` 线性累积。**半径由构造恒定**，是可硬断言的不变量——区别于既有 `VortexSpiralParticle` 的加力模拟（半径由切向力/向心拉力/阻尼平衡涌现，不可控也不可断言）。
- **`BongOrbitSpriteParticle` / `BongOrbitLineParticle`（新增）**：把 `OrbitPath` 接到 MC 粒子生命周期。位置由轨道给出，速度字段每 tick 同步成当前切向量——`BongLineParticle` 的 quad 沿速度定向，故脉冲条始终切于圆周，朝向与运动自洽。
- **贴图仍零新增**：三处环绕复用既有 `qi_aura` / `lingqi_ripple`，未引入任何 png/贴图资产。
- **方向约定**：正线速度 = 俯视（自 +Y 向下看）自 +x 转向 +z。三处环绕招统一此约定（首版 `ni_mai_hu_ti` 注释写「逆时针」而 `multipoint` 写「顺时针」，两者数学其实相同，本次统一措辞）。
- **回归锁**：`OrbitPathTest` 断言半径 600 tick 恒定、速度恒切于圆周且速率恒定、角速度 = 线速度/半径、退化参数（半径 0/负/NaN/Inf）构造期拒绝；`SkillParticlePlanTest` 断言三招确实产出 `Orbit*` 描述符，并推进各自完整 lifetime 后半径不漂。
- **体表锚定的回归锁**（server，`cultivation::burst_meridian::tests`，见 ② 的锚定契约）：`p5_ni_mai_hu_ti_cast_installs_aura_anchor_matching_buff_window`（锚点窗口 == buff 时长）、`p5_ni_mai_hu_ti_cast_ring_lives_one_reemit_interval_not_whole_buff`（首环不再 60t）、`p5_ni_mai_hu_ti_aura_ring_follows_moving_caster`（**核心**：窗口内移动 4 段，逐环断言圆心 == 施法者当时位置）、`p5_ni_mai_hu_ti_aura_cadence_tiles_buff_window_exactly`（相位固定 + 首环与重发严丝合缝铺满 60t）、`p5_ni_mai_hu_ti_aura_stays_silent_on_cast_tick_and_off_phase_ticks`（cast 同帧不叠环）、`p5_ni_mai_hu_ti_aura_anchor_removed_and_silent_after_buff_expiry`（到期摘锚点 + 永久停发）、`p5_ni_mai_hu_ti_cast_ring_and_reemit_ring_share_one_form_spec`（两条发射路径除 origin 外逐字段相等）、`p5_ni_mai_hu_ti_recast_resets_aura_window_without_stacking`（重放覆盖窗口不叠环）。

### P5.2 双端闭环接线矩阵（交付物）

真源 = `server/src/network/skill_vfx_wiring.rs` 的 `P5_SKILL_VFX_WIRING` 表；单向导出 `client/src/test/resources/bong/skill_vfx_wiring_manifest.json` 供双端消费（重生成唯一入口 `cd server && BONG_REGEN_VFX_MANIFEST=1 cargo test skill_vfx_wiring`）。

| 招式 id | server 发射点 | SpawnParticle event_id | 旧借用 id（已解除） | client VfxPlayer | VfxBootstrap 注册 |
|---|---|---|---|---|---|
| `zhenmai.parry` | `combat/zhenmai_v2.rs::resolve_parry` → `emit_skill_feedback` | `bong:zhenmai_parry_flash` | `bong:jiemai_burst_blood` | `ZhenmaiPulsePlayer` | `ZhenmaiPulsePlayer.PARRY_FLASH` |
| `zhenmai.neutralize` | `combat/zhenmai_v2.rs::resolve_neutralize` → `emit_skill_feedback` | `bong:zhenmai_neutralize_dust` | `bong:jiemai_neutralize_dust` | `ZhenmaiPulsePlayer` | `ZhenmaiPulsePlayer.NEUTRALIZE_DUST` |
| `zhenmai.multipoint` | `combat/zhenmai_v2.rs::resolve_multipoint` → `emit_skill_feedback` | `bong:zhenmai_multipoint_ring` | `bong:jiemai_burst_blood`（借 parry） | `ZhenmaiPulsePlayer` | `ZhenmaiPulsePlayer.MULTIPOINT_RING` |
| `zhenmai.harden` | `combat/zhenmai_v2.rs::resolve_harden` → `emit_skill_feedback` | `bong:zhenmai_harden_shell` | `bong:jiemai_neutralize_dust`（借 neutralize） | `ZhenmaiPulsePlayer` | `ZhenmaiPulsePlayer.HARDEN_SHELL` |
| `zhenmai.sever_chain` | `combat/zhenmai_v2.rs::resolve_sever_chain` → `emit_skill_feedback` | `bong:zhenmai_sever_snap` | `bong:jiemai_sever_flash` | `ZhenmaiPulsePlayer` | `ZhenmaiPulsePlayer.SEVER_SNAP` |
| `burst_meridian.tie_shan_kao` | `cultivation/burst_meridian.rs::resolve_tie_shan_kao` → `emit_burst_av` | `bong:burst_meridian_tie_shan_kao` | `bong:burst_meridian_beng_quan` | `BurstMeridianFamilyPlayer` | `BurstMeridianFamilyPlayer.TIE_SHAN_KAO` |
| `burst_meridian.xue_beng_bu` | `cultivation/burst_meridian.rs::resolve_xue_beng_bu` → `emit_burst_av` | `bong:burst_meridian_xue_beng_bu` | `bong:burst_meridian_beng_quan` | `BurstMeridianFamilyPlayer` | `BurstMeridianFamilyPlayer.XUE_BENG_BU` |
| `burst_meridian.ni_mai_hu_ti` | `cultivation/burst_meridian.rs::resolve_ni_mai_hu_ti` → `emit_burst_av` | `bong:burst_meridian_ni_mai_hu_ti` | `bong:burst_meridian_beng_quan` | `BurstMeridianFamilyPlayer` | `BurstMeridianFamilyPlayer.NI_MAI_HU_TI` |
| `npc.heal_basic` | `npc/npc_skill.rs::npc_heal_basic` → `emit_npc_skill_av` | `bong:npc_heal_basic` | `bong:yidao_meridian_repair` | `NpcSkillAuraPlayer` | `NpcSkillAuraPlayer.HEAL_BASIC` |
| `npc.buff_speed` | `npc/npc_skill.rs::npc_buff_speed` → `emit_npc_skill_av` | `bong:npc_buff_speed` | `bong:jiemai_neutralize_dust` | `NpcSkillAuraPlayer` | `NpcSkillAuraPlayer.BUFF_SPEED` |
| `npc.buff_defense` | `npc/npc_skill.rs::npc_buff_defense` → `emit_npc_skill_av` | `bong:npc_buff_defense` | `bong:burst_meridian_beng_quan` | `NpcSkillAuraPlayer` | `NpcSkillAuraPlayer.BUFF_DEFENSE` |

**双端测试覆盖**（由上表同源驱动）：

- server `skill_vfx_wiring_test.rs`：清单 ↔ 常量表字节级对拍；11 招逐项断言 resolver 发出正确 event_id + color；**11 条旧借用 id 负向断言**（去复用不得回退）；发射集合 == 清单集合；id 形态（`bong` 命名空间 + Identifier 合法字符集）；优先级档位逐项 pin。
- client `SkillVfxWiringManifestTest.java`：逐行断言 `VfxBootstrap.registerDefaults()` 后 `VfxRegistry` 命中同一 id 且 `lookup()` 返回的 player 类名与清单声明一致；注册集合 ⊇ 清单集合；**负向**断言 11 个 id 均不再指向 `SwordQiSlashPlayer` / `BurstMeridianBengQuanPlayer` / `YidaoPeacePulsePlayer`；未注册 id 经 `BongVfxParticleBridge` 返回 `false`（bridgeMiss）不抛异常。

## P6 — 回归收口 ✅ 2026-07-21

> 交付摘要：8 项交付物全部兑现；两项裁决落 `docs/player-animation-conventions.md` §14（纯追加 55 行、零删除）；`CAST_ALIGNMENT_ALLOWLIST` 由 2 条降至 **1 条**（余项归外部未消费骨架，按 §8.1 #5 口径不计入本 plan 判据）。

- **动画资源 pin 测试** ✅：`AnimCastTicksAlignmentTest#everyPlanAnimIdResolvesThroughProductionRegistry` —— 经**生产** resource-reload 入口（`ProductionAnimationResources`，与 F3+T 同一实现）装载后，逐条断言 `BongAnimationRegistry.contains` / `get` 非空 / `sourceOf == JSON`（三者齐才算真解析；只查 `contains` 挡不住命中 inline 源的测试污染假阳性）。id 集合**从 `SKILL_ANIM` + `TWO_STAGE_PAIRS` 派生而非手写清单**（手写必随批次漂移），另加两条走习得通道的架势亮相，共 **56** 条；配 `size >= 50` 下界断言防派生失效退化成空测试。既有 `AnimWiringManifestTest` 同类用例只覆盖 7 条接线动画，本 pin 是其超集补位。
- **allowlist 按 §8.1 #5 收口** ✅ —— 2 条余项逐条落定，**最终余 1 条**：
  - `sword_path.heaven_gate`：裁决为**改判据不改动画**，**已出表**。新增第四类登记例外「定长相位充能型」（`FIXED_PHASE_CHARGE_SKILLS`），把豁免换成比循环档更严的正向机械锁。理由是结构性事实而非口径偏好：① 该段窗长是 `HEAVEN_GATE_CHARGE_END = 60`（`server/src/sword_path/heaven_gate.rs:15`）这一**具名确定性相位常量**，不随 mastery/平和色缩放，而「长引导必须 isLoop」的前提正是窗长可变；② 充能段是一条单调递进的抬剑坡道（`rightArm.pitch` -0.698 → -2.688 rad），强改 isLoop 就必须在 endTick 把所有轴补回起点，等于为满足判据而**引入**库坑 #1 类回绕跳变；③ 定长相位下交接点是确定单点，可要求**逐轴精确相等**（零容差）——现网资产已满足。裁决与入类门槛（5 条）落 conventions §14.1。
  - `woliu.vortex_resonance`：**维持在表**。复核结论：所属 `plan-bughunt-woliu-resonance-loop-arm-decay-v1` 截至 2026-07-21 仍在 `docs/plans-skeleton/` 下、**P0 ⬜ 未消费**、无同名远端分支与开放 PR（PR #1038 只是产出该骨架的 bughunt 轮次，不是修复）。**订正 §8.1 #5 原文的「归 active」表述**：它是 skeleton 而非 active。本 plan §P3 明确对涡流 5 招全程零触碰以防重复修改，故不越界代修。实测违规轴为 **11** 条（双臂 10 轴 endTick 无补帧 + `torso.pitch` t80=0.0 ≠ 回绕锚点 t40=-0.06 的值跳变），allowlist 注释原写「10 个手臂轴」已一并订正。
- **两段式「相位承接契约」统一裁决** ✅ —— **采纳方案①（fade 混合即为契约）**。依据是读 PlayerAnimator 源码得到的三段语义链（同 channel 换 animId 时旧层带 `fadeOut` 留栈；同优先级下后进的 release 层位于其**下方**；`AbstractFadeModifier` 的混合源是 `beginAnimation` 即蓄力段**当前相位**姿态）——故任意相位结束都由真实交叉淡入承接，**相位无关性是结构性保证而非资产巧合**，无需像 ② 那样牺牲基位连续性、也无需像 ③ 那样为每招增列 release 变体。据此确立 5 条硬约束（`fadeOut > 0` 为必要条件、两段同 channel 同 priority、release `fade_in` 宜短、相位姿态预算 60°、仅单侧声明的轴必须中立）。裁决落 conventions §14.2。
  - **相位覆盖测试** ✅ `AnimCastTicksAlignmentTest#twoStageHandoffHoldsAcrossEveryReachableLoopPhase`：对 4 招（`sword.infuse` / `anqi.charge_carrier` / `sword_path.heaven_gate` / yidao 5 招，共 **8 对**）枚举 `[0, 周期)` **全部整数相位**——这是 plan 原文「可达 `cast_ticks` 对 loop 周期取余」的**严格超集**（无论 cast_ticks 怎样浮动其余数必落在本域内），且对未来调参免疫；plan 点名的基位 / 中间相位 / 周期末相位另行单独断言留痕。`heaven_gate` 作为定长相位的确定性退化档，改由零容差接缝断言覆盖（比预算更严）。预算 60° 依据现网实测全相位最大差 46°（`yidao.contam_purge` 中段 `rightArm.bend`）留余量，已用突变验证（降到 40° 即撞红）。
  - **客户端桥接 pin** ✅ `TwoStageHandoffBlendTest` 4 例（合成动画锁桥接语义，与资产预算 pin 互补不重叠）：任意相位交接瞬间姿态连续、全程不穿 vanilla、淡出后收敛到 release 且层不泄漏，外加 `fadeOut = 0` 退化对照——**实测该退化下姿态塌回 vanilla 中立而非 release 首帧**（比预想更糟：玩家会看到手臂先弹回下垂再抬起），故 `fadeOut > 0` 入硬约束。
- **`stance_woliu` + `stance_zhenmai`「一次性亮相」遗留清偿** ✅（§8.1 #2 第 4 条）：两张资产原为 `isLoop:true` 站桩，但 `emit_technique_learned_stance_triggers` 只单发一次 `PlayAnim`、全仓无持续架势状态可驱动循环、也无任何 `StopAnim` —— 即 conventions §13 #6 红线违例。改为一次性亮相（`isLoop:false` + 收势回中立）：`stance_woliu` 32t「吸→吐」沉身收拢后双掌外旋托举撑开涡场；`stance_zhenmai` 28t「提指取穴→下针」以指代针前点、发力由 `torso.yaw` 送肩承担。顺带清偿两笔精度欠账（woliu 原 3 帧点间隔 20t、且除双臂外全身不动；zhenmai 原 3 帧点**逐字节完全相同**即静止图空转）。走视觉资产纪律 3 轮打磨：round 2 出三视图自评抓到并修复两处**远距离读招**缺陷（针脉正前方直点透视缩短成一个点 → 外分成斜线剪影；涡流双臂完全镜像只读作「举起双手」→ 螺旋错高），终轮 commit 带 `<PROMISE>`。补三段式 spec manifest 机械锁。
- **TPV 实机验收（完成判据）** ✅：终轮全帧三视图存档 + 逐招人工验收 checklist 落 `client/tools/renders/stance_p6/`（沿用 PR-6 `yidao_p4/` 形态：grid PNG + `README.md`）。checklist 五栏（双手职责 / 姿态母题兑现 / 重心与全身协调 / 收势归中立 / 远距离可辨性），并明写哪些项已由机器断言因而不依赖肉眼，把人工验收收敛到真正只能用眼睛判的部分。**互不混淆判定**：涡流峰值 =「双臂上举、左右不等高」，针脉峰值 =「单臂斜伸 + 另一臂胸前虚扶 + 躯干拧转」。
- **FPV 兼容性冒烟（非阻塞，不作为完成判据）**：现状第一人称渲染路径（`THIRD_PERSON_MODEL`）未改动，无**路径**回归。真正的第一人称手臂验收仍归梯队三 `plan-fpv-cast-av-v1` P5。
  - **判定从断言升级为机械证据**（review r1 #2 返工）：FPV 渲染路径的全部生产代码面 = ① `client/src/main/java/com/bong/client/animation/BongAnimationPlayer.java`（全仓**唯一**设置 `FirstPersonMode.THIRD_PERSON_MODEL` 的地方，同时掌管 fade-in/out 与 `ModifierLayer` 层管理，即任何动画进入 FPV 管线的必经口）、② `client/src/main/java/com/bong/client/mixin/MixinHeldItemRenderer.java`（该模式下 FPV 持握物注入）。判定命令与结果：
    ```bash
    git diff --stat origin/main...HEAD -- \
      client/src/main/java/com/bong/client/animation/BongAnimationPlayer.java \
      client/src/main/java/com/bong/client/mixin/MixinHeldItemRenderer.java
    # → 空输出（零改动）
    git diff --name-only origin/main...HEAD -- 'client/src/main/java/**'
    # → 空输出（本 plan 全程未改动任何 client 生产 java）
    ```
  - **为什么「零改动即无路径回归」成立**：一份动画能否在 FPV 下被看到、以什么姿态被看到，只由三件事决定——(a) 该层被设成哪种 `FirstPersonMode`、(b) FPV 持握物渲染是否被接管、(c) 动画数据本身。(a)(b) 两项的代码逐字节未变（上方命令为证），(c) 在本 plan 内变化的只是 `player_animation/*.json` 数据，而 `THIRD_PERSON_MODEL` 对同一份数据在 FPV 与 TPV 走的是**同一套上半身模型渲染**，不存在只在 FPV 生效的分支逻辑。故本 plan 不可能引入「TPV 正常、FPV 坏掉」的路径级回归。
  - **本条明确不主张的内容**（避免与梯队三职责混淆）：零改动**不等于**逐招 FPV 取景已验收——新资产的手臂是否落在第一人称视野内（conventions §3 的可见性判据）属于**资产取景**问题而非路径问题，必须实机逐招看，这正是梯队三 `plan-fpv-cast-av-v1` P5 的交付物。本 plan 从 P0 起即把 FPV 列为非阻塞冒烟、完成判据取 TPV 实机验收（见上一条），两处口径一致。
- **server 侧映射表单测** ✅：核验结论 —— 本 plan 触碰过 `vfx_animation_trigger.rs` 的 PR 仅 PR-3/4/5，涉及 `emit_sword_path_visual_triggers` / `emit_anqi_visual_triggers` / `emit_dugu_needle_visual_triggers` / `emit_woliu_v1_vortex_visual_triggers`，**每个都已配 pin 测试**（随各自批次交付）。P6 在此基础上补 3 条契约级 pin：`anqi_two_stage_handoff_satisfies_phase_handoff_contract`（§14.2 的 fade 形状：`fade_out ≥ 2`、release `fade_in ≤ fade_out`、两段同 priority，且两个 fade 值都要求显式 `Some(..)` 不许回落客户端默认）、`heaven_gate_two_stage_uses_same_priority_for_both_phases`、`stance_reveal_play_anim_carries_explicit_cold_start_fade_in`（架势亮相是**冷起手**故要求 `fade_in ≥ 3`，方向与两段式 release 的「热交接宜短」相反，此前 `assert_play_anim` 用 `..` 丢弃该值、全无覆盖）。
  - 顺带登记（**不在本 plan 范围**）：`emit_defense_animation_triggers` / `emit_baomai_v3_visual_triggers` / `emit_scroll_read_stop_for_entity` 三个 arm 至今零 pin 测试，`emit_tribulation_animation_triggers` 仅有间接覆盖——均为本 plan 未触碰的**存量**缺口，见下「遗留与后续」。

## §8 开放问题（P0 决策门前需收口）

1. **gameplay 数值是否连动**：推荐 cast_ticks/冷却一律不动（本 plan 纯表现层），瞬发招用「爆发帧+收势」表达而不是拉长 cast——若用户想借机调战斗节奏（如 sword.parry cast=4 太短），另立 combat 数值 plan，不混入本 plan。
2. **批次范围裁剪**：49 招中 npc 3 招是 mob 实体（PlayerAnimator 不适用，动画列 N/A 只做粒子分化）；确认最终重制清单 = 46 玩家招中 C/D 级全量还是先 P1-P2 主力 20 招验证标准再扩。`morph_cast`（§现状证据 ② 的错配例证，cast 60↔动画 30）与梯队一接线后的 `stance_*` 循环站桩是否纳入重制批次，也在此一并拍板归属。
3. **`echo_fractal` 等「循环引导段」的移动打断表现**：引导中移动/受击打断时动画 fade_out 参数（当前 PlayAnim 只有 fade_in_ticks），是否需要 server 发 `StopAnim` 补齐打断链路——需读 cast 打断处理代码后收口。
4. **对拍测试的 cast_ticks 快照机制**：client 测试无法直接读 server Rust 源——用 checked-in JSON 快照（server 侧测试保证快照与 `TECHNIQUE_DEFINITIONS` 同步）还是构建期导出，二选一；无论选哪种，方向必须是 `TECHNIQUE_DEFINITIONS` → 快照的单向生成（见 P0），快照不可手改。

### §8.1 决议（pre-P0 收口，2026-07-18）

> 决议依据：两路并行只读调研（npc/morph/stance 现状 + cast 打断/StopAnim 链路），全部结论带文件:行号证据。

#### #1 gameplay 数值不连动 —— 已收口：纯表现层，cast_ticks/冷却/伤害一律不动

**决议**：
1. 采纳推荐路线：本 plan 纯表现层，`cast_ticks`/`cooldown_ticks`/伤害数值零改动；`known_techniques.rs` 只作时长对齐的只读基准。
2. 瞬发招（cast ≤ 2）按精度标准 #2 用「爆发帧 + 收势」表达质感，不拉长 cast；节奏调整需求（如 sword.parry cast=4 过短）另立 combat 数值 plan。
3. 表现层伴随参数（如 `emit_yixing_av` 的粒子 `duration_ticks`，`server/src/body_plan/morph.rs:224`）随动画时长对齐允许同步改——它们是 AV 元数据非 gameplay 数值。

**落点**：`server/src/cultivation/known_techniques.rs`（只读基准）；plan §动画精度标准 #2。

#### #2 批次范围裁剪 —— 已收口：46 玩家招 C/D 级全量分批；npc N/A；morph_cast/stance 归 P3

**决议**：
1. **npc 3 招动画列 N/A 成立**：调研证实三招 caster 是 NPC mob 实体（`server/src/npc/npc_skill.rs:284-287` 注册，big-brain `NpcTechniqueAction` 以 NPC Entity 施放），统一走 `emit_npc_skill_av`（`npc_skill.rs:239-266`）——头注释明写 NPC 非玩家实体、PlayAnim 不适用，只发 SpawnParticle + 音效。npc 3 招仅入 P5 粒子分化（且粒子 id/颜色已各自独立，P5 复核差异化充分性即可）。

   > **P5 复核结论（2026-07-20，修正本条决议的乐观前提）**：本决议写「粒子 id/颜色已各自独立」不成立——三招 id 确实两两不同，但**全都是借来的**（heal 借 `bong:yidao_meridian_repair` / speed 借 `bong:jiemai_neutralize_dust` / defense 借 `bong:burst_meridian_beng_quan`），旁观者分不清是 NPC 施法还是玩家在放同名招；颜色上 speed `#9FD8C8` 与 heal `#A8E6CF` 同属淡青绿、仅单通道差 ~10%，远距离不可辨。故 P5 按本 plan P5 段正文「脱离借用，各给独立 event_id」执行（而非仅复核）：三招改 `bong:npc_*` 专属 id + `NpcSkillAuraPlayer`，speed 配色改麦黄 `#E3C766` 构成绿/黄/蓝三元组。附带修正：借用 id 命中玩家技能家族前缀曾让 NPC 背景 cosmetic 误吃 Important 优先级，专属 id 归 Normal 档后不再与玩家技能反馈争拥挤 chunk 的粒子配额。
2. **重制清单 = 46 玩家招中 C/D 级全量，P1-P4 分批推进**（不做「先 20 招验证再扩」的折半）：标准已随 P0 定稿 + P1 首批交付本身就是标准验证批，后批照走同构流程，无需额外验证阶段。
3. **morph_cast 归 P3 批次**：发射链活着（`server/src/body_plan/morph.rs:229-237` `emit_yixing_av` 发 `bong:morph_cast`，`cast_morph_yixing` 两分支触发），60↔30 错配证实（`YIXING_CAST_TICKS=60` @ morph.rs:96 vs `morph_cast.json` endTick=30）——P3 按标准 #2 重制对齐（cast 完成 = 发力顶点 + recovery），粒子 duration 同步对齐（见 #1 第 3 条）。
4. **stance_woliu / stance_zhenmai 归 P3，按「一次性亮相」精修**：两 JSON 均为 `isLoop:true` 循环站桩形态（stance_woliu endTick=40 循环开合、stance_zhenmai endTick=20 三帧全同的静态持守），与梯队一接线的「习得时刻单发」语义错配（`vfx_animation_trigger.rs` `emit_technique_learned_stance_triggers` 单发、全仓无持续架势状态可驱动循环）。决议：改 `isLoop:false` + 补收势回中立，做成习得亮相动画；「循环站桩」形态等持续架势 gameplay 状态落地后另议（不在本 plan 造事件）。

**落点**：`server/src/npc/npc_skill.rs:239-266`；`server/src/body_plan/morph.rs:96,224,229-237`；`client/src/main/resources/assets/bong/player_animation/{morph_cast,stance_woliu,stance_zhenmai}.json`；plan §P3 批次清单。

#### #3 循环引导段的打断表现 —— 已收口：循环动画必须有停止路径（红线），按通道逐招接线

**决议**：
1. **机制齐备，无需新 schema**：`StopAnim { anim_id, fade_out_ticks }` 已有 fade_out 参数（`server/src/schema/vfx_event.rs:130-134`），client 消费链完整实装（`VfxEventRouter.java:86-92` → `ClientAnimationBridge` → `AnimationLayerManager.stopAnimationOnStack`，PR #1221 已加真实 bridge stop 闭环测试）；server 通用 helper `send_stop_anim`（`vfx_animation_trigger.rs:1496-1505`）与「打断 → StopAnim」既有先例（`network/full_power_emit.rs:66-105`：`ChargeInterruptedEvent` → StopAnim(windup_charge)）均可直接复用。
2. **新作红线（随精度标准入档）**：任何 `isLoop:true` 引导段动画，落地时必须同批交付停止路径——正常完成时刻由 release 段 PlayAnim 同优先级覆盖或显式 StopAnim；打断时刻必须显式 StopAnim。无停止路径的循环动画不予合入。
3. **按通道逐招接线**（调研证实三类通道打断行为各异）：
   - `charge_carrier`（独立 `CarrierCharging` 通道）：移动打断分支 `finish_charge(full_charge=false)`（`carrier.rs:485-506`）当前不发 StopAnim——P2 落地循环结印段动画时在该分支补 StopAnim。
   - `echo_fractal`：现状**瞬发结算**（有 resolver，`anqi_v2.rs:530-533` 直接 `CastResult::Started`，不插 `Casting`、不可打断）——其「循环撒饵段」若做 isLoop，release 时刻在结算点显式 StopAnim + release PlayAnim；无打断分支需接（瞬发无打断窗口）。
   - 走通用 `Casting` 状态机的招（无 resolver、`start_generic_skillbar_cast` 路径）：`tick_casts_or_interrupt` 三打断分支（`cast_emit.rs:130-219`）当前只发 cast_sync+audio 不发 StopAnim——P2 为此类招落地循环段时在三分支补 StopAnim（`Casting` 组件需记录当前循环 anim_id，实施设计随 P2 收口）。
   - 非循环两段式（heaven_gate 先例，`skill_register.rs:557` phase system + 两条 PlayAnim 覆盖）：不需 StopAnim，维持现状范式。

**落点**：`server/src/network/cast_emit.rs:130-219`；`server/src/cultivation/anqi/carrier.rs:485-506`（按调研实名路径为准）；`server/src/network/full_power_emit.rs:66-105`（先例）；`vfx_animation_trigger.rs:1496-1505`（helper）；plan §动画精度标准（红线追加）+ §P2。

#### #4 cast_ticks 快照机制 —— 已收口：checked-in JSON 单向生成（梯队一双先例复用）

**决议**：
1. 选 checked-in JSON 快照：`client/src/test/resources/bong/technique_cast_ticks_snapshot.json`，由 server `TECHNIQUE_DEFINITIONS` 单向生成。
2. 机制逐条复用梯队一已验收先例（`technique_icon_snapshot_test.rs` + `anim_wiring_manifest_test.rs`）：server 侧同步测试断言快照与定义表完全一致（缺失/多余/漂移/字节级格式手改分别点名判红），重生成唯一入口 `BONG_REGEN_CAST_TICKS_SNAPSHOT=1 cargo test`，快照不可手改；client 侧经 classloader 只消费不维护。
3. 拒绝构建期导出：checked-in 快照有稳定 diff、review 可见、CI 无额外构建步骤，梯队一两份快照已证明该机制可靠。

**落点**：新增 `server/src/cultivation/technique_cast_ticks_snapshot_test.rs` + `client/src/test/resources/bong/technique_cast_ticks_snapshot.json`；client 对拍测试消费点归 P0。

**修订 #4a（2026-07-20，P4 review r3 提出）——权威来源由单表改为「具名双表有序并集」**：

- **背景**：P4 落地 yidao 5 招时发现它们不在 `TECHNIQUE_DEFINITIONS`（该表是**功法**注册表），其 cast_ticks 由 `combat::yidao::yidao_skill_spec()` 独立定义。实现遂改为双表合并生成快照（49→54 键），但 P0 与本决议原文仍写「由 `TECHNIQUE_DEFINITIONS` 单向生成」——形成同一 plan 内的互斥契约。
- **决议**：正式采纳**双源**契约，不把 yidao 塞进 `TECHNIQUE_DEFINITIONS`——后者是功法表，yidao 是 combat 侧技能 spec，强行合表属 gameplay 结构改动，越出本 plan「纯表现层」边界（§8.1 #1）。
- **契约细则**（同步测试逐条锁定）：权威输入 = `cultivation::known_techniques::TECHNIQUE_DEFINITIONS` ∪ `combat::yidao::yidao_skill_spec()`（两个**具名**来源，无第三方）；skill_id 全局唯一，**跨源撞名即判红**（不做静默覆盖）；`BTreeMap` 保证确定性排序；重生成唯一入口不变 `BONG_REGEN_CAST_TICKS_SNAPSHOT=1 cargo test technique_cast_ticks_snapshot`；快照仍不可手改，缺失/多余/漂移/撞名分别点名判红。
- **落点**：P0「快照单一真源」条目按本修订读作「具名双表并集单向生成」；`server/src/cultivation/technique_cast_ticks_snapshot_test.rs` 的 `REGEN_HINT` 与模块注释同步表述。

#### #5 allowlist 清零判据的口径 —— 已收口（2026-07-20，P4 review 提出）：判据只覆盖**本 plan 责任内**条目

**背景（矛盾点）**：P0 声明「allowlist 清零 = P1-P4 完成的机械判据」，但 P3/P4 交付完成时 `CAST_ALIGNMENT_ALLOWLIST` 仍余 2 条；同时 P6 段又把「P0 对拍测试 allowlist 清零」列为 P6 自己的交付物——同一文档两处对「何时应清零」口径打架，P3/P4 标 ✅ 缺少正式豁免依据。

**决议**：
1. P0 判据的原意是「P1-P4 的**重制批次**不得留下未达标的动画」，其成立前提是 allowlist 全部条目都落在 P1-P4 重制清单内。该前提在实施中被两类合法例外打破，故判据口径正式修订为：**P1-P4 完成判据 = allowlist 中属于 P1-P4 重制清单的条目清零**；明示归属外部 plan 或后续阶段的条目不计入，但必须在余项登记里写明归属，不得无主悬挂。
2. 现存 2 条余项的归属逐条确认：`woliu.vortex_resonance` 归 active `plan-bughunt-woliu-resonance-loop-arm-decay-v1`（P3 段既定排除项，本 plan 全程零触碰以防重复修改）；`sword_path.heaven_gate` 归 **P6**（残留问题是「动画对齐 60t 充能相位而非 cast=80 总窗」的口径之争 + hold 末帧与 isLoop 正典统一，属跨招约定裁决而非单招重制欠账，P2 后半已把资产密度精修到位）。
3. P6 段「allowlist 清零」交付物随之明确为**收口这 2 条余项**（裁决 heaven_gate 口径 → 改动画或改判据二选一并落文档；vortex_resonance 待其 bugfix plan merge 后复核可否出表），而非重复 P1-P4 的批量重制工作——两处不再冲突。
4. 棘轮硬约束不变：allowlist **只允许缩小**，冻结基线 `P0_BASELINE` 永不追加；本决议只调整「完成判据如何读」，不放宽任何一条现存条目的达标要求。

**落点**：P0「快照单一真源」条目与 P6「allowlist 清零」条目按本决议口径解读；余项归属登记见附录 A 后各批次统计段。

**P6 收口结果（2026-07-21）**：2 条余项已逐条落定，`CAST_ALIGNMENT_ALLOWLIST` **最终余 1 条**。① `sword_path.heaven_gate` 裁决为**改判据不改动画**并**已出表**，转入新设的第四类登记例外「定长相位充能型」（`FIXED_PHASE_CHARGE_SKILLS`，正向机械锁取代豁免），裁决理由与入类门槛见 conventions §14.1 与 P6 段。② `woliu.vortex_resonance` **维持在表**——**订正本决议第 2 点的「归 active」表述**：`plan-bughunt-woliu-resonance-loop-arm-decay-v1` 截至 2026-07-21 仍在 `docs/plans-skeleton/` 下、P0 ⬜ **未消费**、无同名远端分支与开放 PR（PR #1038 只是产出该骨架的 bughunt 轮次而非修复），故不是 active plan；按 §P3 既定排除本 plan 不越界代修。另订正实测违规轴数为 **11** 条（双臂 10 轴 endTick 无补帧 + `torso.pitch` t80 值跳变），allowlist 注释原写「10 个手臂轴」已同步更正。

> §8 原表保留作历史回溯。全部已在 §8.1 收口，**实施时以 §8.1 决议为准**。

## 测试声明

- client：动画 JSON 元数据对拍测试（分类型时长断言 / 三段 manifest 帧点下限 / 主轴帧间隔 ≤4 / easing 显式且打击轴禁 linear / leg.pitch 上限 / 循环每轴 endTick 补帧 / 快照缺失/重复/漂移判红）+ 资源 pin + 粒子 registry 集合一致性（gradlew test）；
- server：`vfx_animation_trigger` 映射 arm 单测（含借用改专属后旧 id 不再发出的负向断言）+ P5 粒子发射 pin + cast_ticks 快照单向同步测试（cargo test）；
- 实机：每批 `render_animation.py` 三视图存档 + 人工验收 checklist（重心/协调等非机械项）+ P6 TPV 读招验收（FPV 仅非阻塞冒烟）。**存档位置**：`client/tools/renders/<批次>/`（随 PR 提交，与仓库既有 renders 惯例一致），checklist 同目录 `README.md`——P4 批次见 `client/tools/renders/yidao_p4/`；
- e2e：`bash scripts/smoke-test-e2e.sh` 绿。**证据口径**：该脚本由 CI `e2e` job 在每个 PR HEAD 上执行（`.github/workflows/e2e.yml:144`，同 job 另跑 `bot-e2e.sh`），验收记录引用对应 HEAD SHA 的绿色 job 即可，不要求本地重跑（本地无 headless MC 依赖链）。

## §10 实施工作流

- 单 plan 多 PR 序列化（2026-07-19 更新：P2 按批量拆两 PR，后续顺延）：PR-1 = P0（审计+标准+对拍测试，#1234）；PR-2 = P1 批次一（#1235）；PR-3 = P2 前半去复用 6 招（#1239）；PR-4 = P2 后半长引导两段式+StopAnim 接线+heaven_gate 精修；PR-5/6/7/8 = P3/P4/P5/P6。前一 PR merge 后开下一个。
- 每 PR 独立实施 subagent（context 隔离），动画批次 PR 强制 3 轮打磨 commit `(round N/3)` + 终轮 `<PROMISE>`。
- CodeRabbit / `/review` 等待走 ScheduleWakeup 1200s 协议，修完意见重等 re-review。
- **单次 consume-plan 全自动到 merge**：用户提交 `/consume-plan` 后全自动走完实施→review→merge→归档至 `docs/finished_plans/`，无需人工值守；动画属视觉资产，每批终轮三视图 PNG 附 PR body 供人工抽查。

## 附录 A —— 49 招全量审计矩阵（P0 落档，2026-07-18）

> 元数据由 `player_animation/*.json` 全量解析 + `TECHNIQUE_DEFINITIONS` cast_ticks + 各发射映射逐条核对产出。
> 分级：A 达标 / B 精修 / C 重制（有专属但快闪/模板/错配）/ D 缺失（无专属动画：借用别家或 None）/ N/A（NPC mob 无 PlayAnim 通道）。
> 分级由 P0 机械规则 + 调研初判；各批实施时逐招按精度标准复核，可调级但只许更严不许放宽。「模板产物」判据 = 81 轴 KF（27 轴 × 首/中/尾 3 帧）特征。

| skill_id | cast | anim_id | endTick | loop | 帧点 | 轴KF | 借用 | 分级 | 批次 | 备注 |
|---|--:|---|--:|---|--:|--:|---|---|---|---|
| `sword.cleave` | 16 | `sword_cleave` | 20 | — | 8 | 168 | — | **A** | P1 | 专属；P1 批次一重制（2026-07-19）：举剑过头竖劈+弓步前压三段式；endTick=20 为与借用方 condense_edge（cast=12）区间交集 |
| `sword.thrust` | 10 | `sword_thrust` | 16 | — | 8 | 172 | — | **A** | P1 | 专属；P1 批次一重制（2026-07-19）：收剑腰侧直刺+侧身送肩 |
| `sword.parry` | 4 | `sword_parry` | 10 | — | 6 | 126 | — | **A** | P1 | 专属；P1 批次一重制（2026-07-19）：斜举格挡弹开，密度补齐 |
| `sword.infuse` | 40 | `sword_infuse`(loop)+`sword_infuse_release` | 28 loop+14 | ✓ | 9+7 | 207+154 | — | **A** | P2 | P2 后半两段式（2026-07-19）：真实引导窗（`cast_sword_infuse` 插 `Casting`+`PendingSwordInfuse`，sword_basics.rs:723-746）→ 蓄力段重制 isLoop 28t 横剑抚刃（id 沿用 v1 资产清单 pin）+ release 14t 剑身一振；打断 = cast_emit 三分支表驱动 StopAnim，完成 = completion_tick StopAnim+release（失败分支亦 StopAnim） |
| `movement.dash` | 0 | `dash_forward` | 8 | — | 6 | 126 | — | **A** | P3 | P3 重制（2026-07-19）：压身摆臂→蹬地前窜→刹步，8t 瞬发域；出 allowlist |
| `shield_block` | 0 | `shield_raise` | 18 | ✓ | 7 | 161 | — | **A** | P3 | P3 重制（2026-07-19）：raise 0→6 + hold 呼吸微晃 6→18（returnTick=6，t18≡t6 闭合）；三路 StopAnim 既有，持续维持型例外保持 |
| `burst_meridian.beng_quan` | 8 | `beng_quan` | 14 | — | 9 | 194 | — | **A** | P1 | 专属；P1 批次一重制（2026-07-19）：沉马蓄劲→拳炸出→震颤收；endTick=14 为三借用方 cast 区间交集 |
| `burst_meridian.tie_shan_kao` | 10 | `tie_shan_kao` | 16 | — | 7 | 161 | — | **A** | P3 | P3 专属化（2026-07-19）：拧腰蓄靠→肩胯撞出（手臂折叠贴身，顶点=t10）；混合通道（resolver+真实 Casting）三段式，解除 beng_quan 借用+负向 pin |
| `burst_meridian.xue_beng_bu` | 6 | `xue_beng_bu` | 12 | — | 8 | 168 | — | **A** | P3 | P3 专属化（2026-07-19）：起跑压桩→双臂拖尾疾步窜出（顶点=t6 位移落定）；解除 beng_quan 借用+负向 pin |
| `burst_meridian.ni_mai_hu_ti` | 12 | `ni_mai_hu_ti` | 16 | — | 8 | 176 | — | **A** | P3 | P3 缺失补齐（2026-07-19）：交臂引气→沉桩压封护体结印（anim_id None→Some 专属常量，事件路径 pin）；删 MISSING 条目 |
| `baomai.full_power_charge` | 1 | `baomai_full_power_charge` | 24 | ✓ | 9 | 216 | — | **A** | P3 | P3 专属化（2026-07-19）：抱脉沉桩呼吸循环 24t 闭环（ChargingState 持续维持型，入 SUSTAINED_LOOP_EXCEPTIONS）；释放/打断双路 StopAnim 与 PlayAnim 共享 full_power_strike 常量+负向 pin |
| `baomai.full_power_release` | 1 | `baomai_full_power_release` | 12 | — | 7 | 168 | — | **A** | P3 | P3 专属化（2026-07-19）：蓄力位无缝接力→双拳崩出→泄力虚脱意，12t 瞬发域；解除 release_burst 借用 |
| `zhenmai.parry` | 1 | `zhenmai_parry` | 8 | — | 6 | 130 | — | **A** | P1 | 专属；P1 批次一重制（2026-07-19）：瞬发单手拍挡爆发帧+收势 |
| `zhenmai.neutralize` | 4 | `zhenmai_neutralize` | 10 | — | 7 | 146 | — | **A** | P1 | 专属；P1 批次一重制（2026-07-19）：双掌下按化劲+沉桩 |
| `zhenmai.multipoint` | 6 | `zhenmai_multipoint` | 12 | — | 10 | 215 | — | **A** | P1 | 专属；P1 批次一重制（2026-07-19）：连环三点指（右高/左中/右深） |
| `zhenmai.harden` | 5 | `zhenmai_harden` | 11 | — | 8 | 165 | — | **A** | P1 | 专属；P1 批次一重制（2026-07-19）：抱臂沉桩硬化+紧咬 clench |
| `zhenmai.sever_chain` | 8 | `zhenmai_sever_chain` | 14 | — | 9 | 198 | — | **A** | P1 | 专属；P1 批次一重制（2026-07-19）：手刀横斩断链+overshoot |
| `woliu.vortex` | 1 | `woliu_vortex_cast` | 10 | — | 6 | 126 | — | **A** | P3 | P3 缺口修正（2026-07-19）：核验证实并非零发射——field lifecycle 借播 v2 站桩 vortex_spiral_stance（vfx `emit_woliu_v1_vortex_visual_triggers`）；改指专属双臂开涡 10t 瞬发+拼写/负向 pin；删 MISSING 条目 |
| `woliu.hold` | 1 | `vortex_palm_open` | 10 | — | 6 | 132 | — | **A** | P3 | P3 重制（2026-07-19）：绕臂托举→头顶撑伞开掌+微颤，10t 瞬发域 |
| `woliu.burst` | 1 | `woliu_burst` | 8 | — | 6 | 126 | — | **A** | P3 | P3 专属化（2026-07-19）：交臂紧压→双掌对称外弹+退步卸力；解除 palm_strike 借用，基础 5 招动画跨招唯一断言 |
| `woliu.mouth` | 6 | `woliu_mouth` | 12 | — | 7 | 154 | — | **A** | P3 | P3 专属化（2026-07-19）：拧身列位→探爪开口+左手撕回对拉（顶点=t6 定格微颤）；解除 palm_thrust 借用 |
| `woliu.pull` | 5 | `woliu_pull` | 11 | — | 8 | 176 | — | **A** | P3 | P3 去共用（2026-07-19）：前探扣抓→撕拽回身后坐拧腰；与 vacuum_lock 解除共用+互异断言 |
| `woliu.heart` | 10 | `woliu_heart` | 16 | — | 8 | 184 | — | **A** | P3 | P3 去共用（2026-07-19）：举天聚涡→千钧压落深沉马步（顶点=t10）；与 v1 站桩解除共用；出 allowlist |
| `woliu.vacuum_palm` | 6 | `woliu_vacuum_palm` | 12 | — | 8 | 176 | — | **A** | P3 | P3 重制（2026-07-19）：拧腰收掌→平刺→抽真空回拖收爪；出 allowlist |
| `woliu.vortex_shield` | 10 | `woliu_vortex_shield` | 20 | ✓ | 6 | 144 | — | **A** | P3 | P3 重制（2026-07-19）：环抱屏障公转 20t 全轴闭环；停止路径已核验（唯一退出=VortexV2State 窗到期 StopAnim，无提前破盾/取消机制），入 SUSTAINED_LOOP_EXCEPTIONS；出 allowlist |
| `woliu.vacuum_lock` | 8 | `woliu_vacuum_lock` | 13 | — | 7 | 154 | — | **A** | P3 | P3 重制（2026-07-19）：开臂张笼→合拢下压锁困（顶点=t8）；pull 拿到专属后本动画为 vacuum_lock 独占；出 allowlist |
| `woliu.vortex_resonance` | 80 | `woliu_vortex_resonance` | 80 | ✓ | 9 | 25 | — | **A** | — | 80t loop 对齐 cast=80 ✓（时长正例）；对拍实测 10 手臂轴 endTick 无补帧（库坑 #1）——存量 bug 归 `plan-bughunt-woliu-resonance-loop-arm-decay-v1`（防重排除），修复前暂驻 allowlist |
| `woliu.turbulence_burst` | 40 | `woliu_turbulence_burst` | 20 | — | 8 | 192 | — | **A** | P3 | P3（2026-07-19）：通道核验=resolver 同步一次性结算零 Casting（cast=40 纯透传，无窗可挂循环段）→ **瞬发结算型分类契约**（顶点=t0 爆开与结算同帧，入 INSTANT_RESOLVER_SKILLS + instant manifest）；出 allowlist |
| `dugu.shoot_needle` | 1 | `dugu_needle_throw` | 10 | — | 7 | 154 | — | **A** | P3 | P3 重制（2026-07-19）：耳侧引针→鞭甩掷出→随针目送；infuse_poison 拿到专属后本动画为凝针独占 |
| `dugu.infuse_poison` | 1 | `dugu_infuse_poison` | 10 | — | 7 | 154 | — | **A** | P3 | P3 去共用（2026-07-19）：举针凝视→覆手淬毒→腕封（无掷出）；vfx 灌毒分支改指专属常量+负向 pin |
| `tuike.don` | 12 | `tuike_don_skin` | 18 | — | 9 | 207 | — | **A** | P3 | P3 重制（2026-07-19）：俯身探底（bow 补偿）→沿身披壳上提→抖身定壳（顶点=t12）；同 id 双源不动（归 bugfix plan） |
| `tuike.shed` | 8 | `tuike_shed_burst` | 13 | — | 8 | 184 | — | **A** | P3 | P3 重制（2026-07-19）：裹身紧缩→炸开甩壳+挺胸微跳→左右抖落（顶点=t8 壳离体） |
| `tuike.transfer_taint` | 10 | `tuike_taint_transfer` | 15 | — | 8 | 176 | — | **A** | P3 | P3 重制（2026-07-19）：按胸引秽→抽出带颤→前推按入壳层→下抚（顶点=t10） |
| `anqi.charge_carrier` | 400 | `anqi_charge_carrier_loop`(+`_release`) | 32 loop+14 | ✓ | 9+7 | 207+154 | — | **A** | P2 | P2 后半两段式（2026-07-19）：真实 400t 通道（`CHARGE_DURATION_TICKS` carrier.rs:47）→ 专属封骨结印循环 32t + release 14t；`CarrierChargeBegan/Ended` 事件接线（begin 起播 / finish_charge 全退出路径 StopAnim / full_charge 才播 release，早退分支覆盖有专属 pin） |
| `anqi.single_snipe` | 6 | `anqi_single_snipe` | 12 | — | 7 | 161 | — | **A** | P2 | 专属；P2 前半重制（2026-07-19）：侧身瞄准线→骨镖弹射出手→随镖目送 |
| `anqi.multi_shot` | 30 | `anqi_multi_shot` | 36 | — | 11 | 231 | — | **A** | P2 | 专属；P2 前半重制（2026-07-19）：胸前拢镖蓄势（load-snap 呼吸）→双臂开扇撒出 |
| `anqi.soul_inject` | 20 | `anqi_soul_inject` | 26 | — | 9 | 189 | — | **A** | P2 | 专属；P2 前半重制（2026-07-19）：单手举镖凝神灌注→刺送注入 |
| `anqi.armor_pierce` | 40 | `anqi_armor_pierce` | 12 | — | 6 | 138 | — | **A** | P2 | P2 后半（2026-07-19，review r2 定形）：**瞬发结算型分类契约**——`resolve_anqi_skill`（anqi_v2.rs:420-534）在 cast 起始 tick 立即结算，cast_ticks=40 为元数据 → 12t 非循环、**strike 顶点=tick 0**（开帧即贯刺命中，t2/t4 钻拧余震 roll 极值帧→撤臂收势），解除 cast_invoke 借用（负向 pin）；出 allowlist，改由 `INSTANT_RESOLVER_SKILLS` 分类 + instant manifest（strike_peak_tick=0）机械锁定（conventions §13 #2 例外 ③）；通道真实化则退类改两段式 |
| `anqi.echo_fractal` | 60 | `anqi_echo_fractal` | 20 | — | 8 | 184 | — | **A** | P2 | P2 后半（2026-07-19，review r2 定形）：同 armor_pierce 瞬发结算型分类（同一 resolver 通道）→ 20t 非循环、**strike 顶点=tick 0**（开帧即爆撒仰开→织网反相波动余韵渐衰），解除 release_burst 借用（负向 pin）；出 allowlist（同 armor_pierce 分类契约） |
| `body.guangbo_ticao` | 60 | `guangbo_ticao` | 150 | — | 288 | 288 | — | **A** | — | 150t/288KF 高完成度 |
| `sword_path.condense_edge` | 12 | `sword_path_condense_edge` | 18 | — | 7 | 147 | — | **A** | P2 | 专属；P2 前半重制（2026-07-19）：收剑入鞘式蓄意→拔剑亮刃定势；endTick=18 ∈ [16,20]（去借用后仍达标，未入过 allowlist） |
| `sword_path.qi_slash` | 20 | `sword_path_qi_slash` | 26 | — | 9 | 198 | — | **A** | P2 | 专属；P2 前半重制（2026-07-19）：高位回环蓄势→大斩挥出剑随气送远 |
| `sword_path.resonance` | 30 | `sword_path_resonance` | 36 | — | 13 | 273 | — | **A** | P2 | 专属；P2 前半重制（2026-07-19）：双手持剑颤鸣蓄振（往复微颤帧）→振荡外放 |
| `sword_path.manifest` | 40 | `sword_manifest_cast` | 14 | — | 6 | 138 | — | **A** | P2 | P2 后半（2026-07-19，review r2 定形）：**瞬发结算型分类契约**——`cast_manifest`（skill_register.rs:288-345）tick 0 即 spawn SwordIntentEntity → 14t 非循环、**strike 顶点=tick 0**（开帧即翻腕送出→目送余韵 head.pitch -12），cast_ticks=40 为元数据；出 allowlist（同 armor_pierce 分类契约） |
| `sword_path.heaven_gate` | 80 | `sword_heaven_gate_charge(+release)` | 60+20 | — | 16+8 | 368+184 | — | **A** | P2 | P2 后半精修（2026-07-19，review 返工补欠账）：charge 旧 4 帧（最大帧距 30t）重制为 4t 步进 16 帧参数化生成（提举/渐升脉动/蓄满微颤极值帧/拉满定格；60t=`HEAVEN_GATE_CHARGE_END` 充能相位全对齐、末帧=release 交接帧，charge_hold segment manifest 锁密度）+ release 旧 3 帧重制三段式 8 帧（巨斩顶点 t7 鞠躬补偿）；驻 allowlist（动画对齐 60t 充能相位而非 cast=80 总窗）；hold-末帧与 isLoop 正典统一归 P6 注记 |
| `npc.heal_basic` | 20 | —— | — | — | — | — | — | **N/A** | P5 粒子 | NPC mob 无 PlayAnim 通道（§8.1 #2） |
| `npc.buff_speed` | 10 | —— | — | — | — | — | — | **N/A** | P5 粒子 | 同上 |
| `npc.buff_defense` | 10 | —— | — | — | — | — | — | **N/A** | P5 粒子 | 同上 |
| `morph.yixing` | 60 | `morph_cast` | 20 | — | 8 | 192 | — | **A** | P3 | P3（2026-07-19）：通道核验=cast_morph_yixing 双分支立即变形零 Casting（YIXING_CAST_TICKS 纯元数据）→ **瞬发结算型分类契约**（顶点=t0 塌形与结算同帧，入 INSTANT_RESOLVER_SKILLS）；粒子 lifetime 30→20 随动画对齐（§8.1 #1）；出 allowlist |

**分级统计**（P0 初判）：A×2 / B×13 / C×12 / D×19 / N-A×3（46 玩家招中 B+C+D = 44 条入 P1-P4 重制/精修清单，与 §8.1 #2 决议一致）。

**P1 批次一后（2026-07-19）**：A×11 / B×11 / C×5 / D×19 / N-A×3——9 条重制达标转 A（sword 基础 3 + beng_quan + zhenmai 5），sword.infuse 移 P2 长引导批次；剩余 B+C+D = 35 条随 P2-P4 清空。

**P2 前半后（2026-07-19）**：A×17 / B×11 / C×5 / D×13 / N-A×3——6 条去复用重制达标转 A（sword_path condense_edge/qi_slash/resonance + anqi single_snipe/multi_shot/soul_inject），allowlist 删 5 条（condense_edge 原不在表）；剩余 B+C+D = 29 条随 P2 后半-P4 清空。

**P2 后半后（2026-07-19，review r2 定形）**：A×23 / B×8 / C×5 / D×10 / N-A×3——6 条转 A（sword.infuse / charge_carrier 真两段式落地 + armor_pierce / echo_fractal / manifest 瞬发结算型分类契约交付 + heaven_gate 双段密度精修）。allowlist 净删 4 条（sword.infuse 两段式达标 + 三招瞬发分类出表；charge_carrier 原本不在表）。**瞬发结算型分类契约**（review r2 裁定，conventions §13 #2 例外 ③）：三招 resolver 在 cast 起始 tick 立即结算、cast_ticks 为元数据，动画 **strike 顶点=tick 0**（开帧即命中姿态，余韵/收势后置），由 `INSTANT_RESOLVER_SKILLS` 分类 pin + instant spec manifest（strike_peak_tick=0、主打击轴 tick 0 落帧）机械锁定——分类契约取代 allowlist 豁免；通道日后真实化（引入 Casting 引导窗，gameplay 变更需独立决议）则退类改两段式。**遗留登记**：仅 heaven_gate 一条驻 allowlist（动画对齐 60t 充能相位而非 cast=80 总窗）+ hold-末帧与 isLoop 正典统一注记，归 P6 裁决。剩余 B+C+D = 23 条随 P3-P4 清空。

**P2 后半打磨记录（2026-07-19，两批各 3 轮）**：首批 7 资产（2 loop + 2 release + 3 单段）——(round 1/3) gen 脚本参数化 first cut → (round 2/3) `render_animation.py` 三视图 grid 目检（loop 首尾同帧 / release 与 loop 稳定帧衔接 / 轨迹互异）+ 机械四查（循环每轴 endTick 同值 / leg.pitch≤40° / 打击轴无 linear / 主轴密度 ≤4t）→ (round 3/3) 决定性再生成 7/7 字节一致 + 双栈门禁绿、终轮 commit 附 `<PROMISE>` 担保。**review 返工批**（PR #1240 blocker：三招瞬发结算与 40/60t 发力顶点脱节 + heaven_gate 欠账）5 资产——(round 1/3) strike 对齐重做 armor_pierce 18t / echo_fractal 24t / manifest_cast 20t + heaven_gate charge 16 帧 / release 8 帧精修 first cut → (round 2/3) 三视图 grid 目检 + 机械查全过 + segment manifest（loop / charge_hold 两型）扩展入对拍测试、7 份 manifest 锁密度 → (round 3/3) 动画测试组全绿 + client 门禁复验、终轮 commit 附 `<PROMISE>` 担保。**review r2 定形批**（r2 裁定 2t anticipation 仍违反「顶点贴 tick 0」契约）3 资产——(round 1/3) 顶点前置到开帧重做：armor_pierce 12t / echo_fractal 20t / manifest_cast 14t（t0 即命中姿态，余韵/收势后置）→ (round 2/3) instant 分类契约机械化：`INSTANT_RESOLVER_SKILLS` 分类 pin + instant spec manifest（strike_peak_tick=0、strike 从 0 起、主打击轴 tick 0 落帧）、三招出 CAST_ALIGNMENT_ALLOWLIST、conventions §13 #2 增例外 ③ → (round 3/3) 三视图 grid 目检（t0 开帧即全伸命中）+ 动画测试组全绿 + client 门禁复验、终轮 commit 附 `<PROMISE>` 担保。

**P3 批次三后（2026-07-19）**：A×46 / B×0 / C×0 / D×0 / N-A×3——矩阵内 46 玩家招全部达标转 A（本批 23 条：借用/共用解除专属化 10 + 缺失补齐 2 + 精修重制 11）。**allowlist 净删 9 条**（movement.dash / baomai.full_power_charge / baomai.full_power_release / woliu.heart / woliu.vacuum_palm / woliu.vortex_shield / woliu.vacuum_lock / woliu.turbulence_burst / morph.yixing），余 2 条：woliu.vortex_resonance（bughunt plan 防重排除项）+ sword_path.heaven_gate（P6 注记项）；**MISSING_ANIM_ALLOWLIST 清零**（ni_mai_hu_ti 补齐 + woliu.vortex 缺口修正）。分类登记：SUSTAINED_LOOP_EXCEPTIONS += baomai.full_power_charge / woliu.vortex_shield（两者停止路径均核验完整：前者释放/打断双路 StopAnim 共享常量，后者唯一退出=VortexV2State 窗到期 StopAnim）；INSTANT_RESOLVER_SKILLS += woliu.turbulence_burst / morph.yixing（均核验为 resolver 立即结算零 Casting 无窗，conventions §13 #2 例外 ③）。**事实修正**：woliu.vortex 原判「combat/woliu.rs 零 PlayAnim」仅对文件成立——系统级动画走 field lifecycle 借播 v2 站桩（`emit_woliu_v1_vortex_visual_triggers`），本批以改指专属完成缺口闭合（矩阵行备注已更正）。矩阵外欠账：P4 yidao 5 招（plan-yidao-v1 §5）。**遗留登记**：§8.1 #2 第 4 条的 stance_woliu / stance_zhenmai「一次性亮相」精修未随本批交付（本批范围锁定附录 A P3 矩阵行 + P3 段清单，两 stance 均非矩阵行），移交 P6 收口或独立小批次。

**P3 批次三打磨记录（2026-07-19，3 轮）**：23 资产（11 新增 + 12 原地重制）——(round 1/3) 逐招第一性原理通道核验（结论以文件:行号写入各 gen 脚本 docstring）后参数化 first cut：三段式 18 / segment loop 2（闭环 BASE-inherit 机械保证）/ instant 2（顶点 t0）/ raise+hold 循环 1（shield_raise returnTick=6、t18≡t6 闭合，绕过全程闭合断言的显式 build 路径 + 脚本内 hold 段闭合自断言）；本地机械预检（时长窗三套 / 循环缝合同值 / leg.pitch≤40° / 主轴密度≤4t / 打击轴无 linear / instant t0 落帧 / 三段各≥2 帧点）在 commit 前抓出 woliu_vortex_shield 5t 帧距违反密度红线 → 改 4t 步进后 ALL CLEAN → (round 2/3) `render_animation.py` 23/23 三视图 grid 逐个目检：借用解除组姿态语言互异可辨（靠撞折臂 vs 疾步拖尾 vs 出拳；淬毒覆手 vs 鞭甩掷针；双掌外弹 / 探爪对拉 / 扣抓撕拽 / 举天压落 / 撑伞 / 开涡横撒各不相同），instant 组 t0 开帧即命中/塌形顶点，loop 组首尾同帧——零缺陷无资产增量改动 → (round 3/3) 决定性再生成 23/23 字节一致；门禁抓出两笔并修正：cargo fmt 格式化 3 处 + shield_raise.json 需内嵌 `<PROMISE>` 块（shield_block.rs 既有 §10.1 pin，补块后重生成）；终验双栈全绿（server CLIPPY:0 + TEST:0，11797 passed 0 failed；client gradle test build SUCCESSFUL 于最终资产之后复跑）。

**P4 后（2026-07-19）**：矩阵外欠账清偿——yidao 5 招（plan-yidao-v1 §5 ①-⑤）全部两段式补齐入对拍契约：`SKILL_ANIM` +5（映射蓄力段）、cast_ticks 快照真源扩展为 `TECHNIQUE_DEFINITIONS` + `yidao_skill_spec` 单向合并（49→54 键，server 同步测试锁双源不撞名、重生成入口不变）、10 资产（5 loop + 5 release）+ 10 spec manifest（loop 段 segment 型 / release 三段式型），**allowlist 零新增**（5 招直接达标长引导判据：isLoop + 全轴 endTick 同值闭环；棘轮基线不动）。姿态语言逐招独立可辨：①俯身**双手持针**沿经脉交替落针、一循环走完整 30 穴位序（90t=30 针×3t，左右各 15 针，bow 补偿）②直立双掌对拢灸火推送 ③深俯直臂 CPR 双压深浅起伏 ④左手喂丹右臂对天接引定格 ⑤双手捧法器高举环阵横扫；release 收势互异：双手同拔提针直身拂袖 / 双臂横向开扇散烟 / 侧耳俯听直起 / 单臂自天纵贯合封 / 对称沉落抱器。停止路径全链闭环（§13 #6）：起手 loop PlayAnim（`resolve_yidao_skill`）→ 三打断分支 + 自然完成分支表驱动 StopAnim（`looping_cast_anim_id` → `yidao_loop_anim_for_skill_id` 分表）→ 有效结算 release 接力 / 无效完成不奖励收势（`complete_yidao_casts`）；事件路径 pin：cast 起播 loop、成功完成恰发 release、无效完成零 release、移动打断只 StopAnim 且无完成事件。

**P4 打磨记录（2026-07-19/20，3 轮）**：10 资产（5 loop BASE-inherit 机械闭环 + 5 release 三段式）——(round 1/3) 逐招第一性原理通道核验（结论以函数锚写入各 gen 脚本 docstring：5 招同走 `resolve_yidao_skill` 真实引导窗、完成链 cast_emit → `complete_yidao_casts`）后参数化 first cut → (round 2/3) 机械四查复跑（循环缝合按 returnTick 回绕锚点同值 / leg.pitch≤40°，实测最大帧距 4t、无 linear easing）**ALL CLEAN**；`render_animation.py` 三视图 grid 目检 5 loop 姿态语言互异可辨（俯身左右分点施针 vs 直立对掌推送起伏 vs 深俯中线合掌按压 vs 右臂朝天左臂喂丹不对称 vs 双臂举过头顶环视）+ 5 release 收势轨迹互异且终帧回中立；新增机械项「loop 基位 → release 承接帧」对拍：4/5 完全一致（0 轴偏差），`mass_meridian_repair` 7 轴微差系 `torso.yaw` 在循环内 ±16° 环视摆动、release 取中位锚点所致（该轴不存在可精确承接的单一相位），仍显著优于已 merge 两段式先例（sword.infuse 9 轴 / charge_carrier 12 轴 / heaven_gate 16 轴偏差）——判定零缺陷无资产增量改动（记录并入终轮，PR-4 先例）→ (round 3/3) 决定性再生成 10/10 字节一致 + 双栈门禁于最终资产之后真跑全绿（server FMT:0 / CLIPPY:0 / TEST:0，11821 passed 0 failed；client `gradlew test build` BUILD SUCCESSFUL，AnimCastTicksAlignmentTest 8/8 含新 10 份 manifest 机械断言），终轮 commit 附 `<PROMISE>` 担保。**执行说明**：round 1 由实施 subagent 交付后撞 session 限额终止（transcript 丢失不可续跑），round 2/3 核验与收口由主干接手完成。

**P4 review r2 返工（2026-07-20）**：`/review` 引擎首次跑通（前三轮连续 infra 降级），4 reviewer 一致 REQUEST_CHANGES，实质 finding 一条——**接经术未兑现 plan 锁定的「双手持针 30 穴位序」**：初版交付为「右手持针 + 左手探穴、一循环四落点」，在双手职责与穴位序规模两项上均不等价。按原交付物重做（未走裁剪决议路线）：`yidao_meridian_repair_loop` 由 28t/4 落点重制为 **90t/30 针**（每 3t 一针 = 密度红线内最密，左右手各 15 针交替落针，落点沿经脉三角 sweep 外推再回程，i=30 与 i=0 同相位机械闭环）；下顿/提针幅度按「远距离读招」验收口径由 13°/6° 拉开至 pitch 20°/bend 14°/roll 18°；`yidao_meridian_repair_release` 同步改为**双手同拔**并逐轴对齐新 BASE（t0 承接零偏差，否则接力瞬间左手会从持针跳成空手）；manifest `expected_end_tick` 28→90、主轴增列 `leftArm.roll`。另 3 条 finding 不采纳：2 条为 Codex reviewer 524 超时的 infra 噪音；1 条 blocker「contam_purge body.x 缺 endTick 补帧」经提出者 D 复投时自行撤销、A/B/C 均核实 tick 24 确有 `body.x=0.0`（本地机械缝合检查同样 ALL CLEAN），属误报。

**P6 收口后（2026-07-21）**：矩阵 A×46 / N-A×3 不变（P6 不含矩阵行重制）。`CAST_ALIGNMENT_ALLOWLIST` **由 2 条降至 1 条**——`sword_path.heaven_gate` 出表转「定长相位充能型」分类契约（conventions §14.1），`woliu.vortex_resonance` 维持在表并订正归属为**未消费骨架**（非 active）。矩阵外交付：`stance_woliu` / `stance_zhenmai` 两张架势亮相由 `isLoop:true` 站桩重制为一次性亮相（§8.1 #2 第 4 条遗留清偿，两者均非矩阵行、走 `TechniqueLearnedEvent` 通道而非 skill 通道）。新增分类登记：`FIXED_PHASE_CHARGE_SKILLS += sword_path.heaven_gate`。`TWO_STAGE_PAIRS` 登记 8 对两段式配对，全部纳入全相位承接预算断言。

**P6 打磨记录（2026-07-21，3 轮）**：2 资产（stance_woliu 32t / stance_zhenmai 28t，均由 `isLoop:true` 改一次性亮相）——(round 1/3) 按精度标准建三段结构 + 4t 步进帧点参数化 first cut → (round 2/3) `render_animation.py` 三视图 grid 自评，抓到两处**远距离读招**缺陷并修复：针脉 round 1 为正前方直点（`rightArm.yaw` 仅 -14 / `torso.yaw` +34），正面视图几乎完全透视缩短成一个点——正前方直刺是最差剪影，改为手臂外分 `yaw -28` + `torso.yaw` 收到 +28（世界方向仍近乎正前，即「拧身而直点」，但手臂相对躯干张开成清晰斜线）+ `head.pitch +8` 低头看穴；涡流 round 1 双臂完全镜像只读作「举起双手」，涡流旋转意象不可见且「对称双臂上举」是全仓最拥挤剪影区间，改为托举段右臂比左臂高约 22°、外分更大 + `torso.yaw +7` 轻拧成螺旋（收势段回对称并归零）。同轮补三段式 spec manifest 机械锁 → (round 3/3) 终轮全帧三视图存档 + 逐招五栏 checklist 落 `client/tools/renders/stance_p6/`；末帧全轴归零核验（一次性亮相播完不僵停，从 isLoop 改单发后最易漏的一条）；终轮 commit 附 `<PROMISE>` 担保。

---

## Finish Evidence

### 落地清单（每阶段 → 真实模块/文件路径）

| 阶段 | 落地路径 |
|---|---|
| **P0** 审计矩阵 + 标准定稿 + 对拍测试 | `client/src/test/java/com/bong/client/animation/AnimCastTicksAlignmentTest.java`（三套时长断言 + 双 allowlist 棘轮 + `P0_BASELINE` 冻结基线 + spec manifest 框架）；`server/src/cultivation/technique_cast_ticks_snapshot_test.rs` + `client/src/test/resources/bong/technique_cast_ticks_snapshot.json`（单向生成快照）；精度标准入档 `docs/player-animation-conventions.md` §13；本 plan 附录 A 49 招矩阵 |
| **P1** 批次一 高频短招 | `client/src/main/resources/assets/bong/player_animation/{sword_cleave,sword_thrust,sword_parry,beng_quan,zhenmai_parry,zhenmai_neutralize,zhenmai_multipoint,zhenmai_harden,zhenmai_sever_chain}.json` + 对应 `client/tools/gen_*.py` |
| **P2** 批次二 去复用 + 长引导两段式 | 前半：`sword_path_{condense_edge,qi_slash,resonance}.json` / `anqi_{single_snipe,multi_shot,soul_inject}.json` + `server/src/network/vfx_animation_trigger.rs`（`sword_path_anim_for_skill` / anqi 常量改指）。后半：`sword_infuse{,_release}.json` / `anqi_charge_carrier_{loop,release}.json` / `anqi_{armor_pierce,echo_fractal}.json` / `sword_manifest_cast.json` / `sword_heaven_gate_{charge,release}.json`；StopAnim 三通道接线 `server/src/network/cast_emit.rs`（`looping_cast_anim_id` / 三打断分支）+ `server/src/combat/sword_basics.rs` + `server/src/combat/carrier.rs` |
| **P3** 批次三 借用招专属化 + 缺失补齐 + 短招精修 | `tie_shan_kao.json` / `xue_beng_bu.json` / `ni_mai_hu_ti.json`（新增）/ `dugu_infuse_poison.json` / `woliu_vortex_cast.json` / `woliu_{burst,mouth,pull,heart,vacuum_palm,vacuum_lock,vortex_shield,turbulence_burst}.json` / `dash_forward.json` / `baomai_full_power_{charge,release}.json` / `morph_cast.json` + `server/src/combat/burst_meridian.rs`（anim_id `None → Some`） |
| **P4** yidao 5 招补齐 | `yidao_{meridian_repair,contam_purge,emergency_resuscitate,life_extension,mass_meridian_repair}_{loop,release}.json`（10 份）+ `server/src/combat/yidao.rs`（10 anim id 常量 / `loop_anim_id` / `release_anim_id` / `complete_yidao_casts` release 接力）；快照真源扩展为双表并集 |
| **P5** 粒子去复用 | `server/src/network/skill_vfx_wiring.rs` + `server/src/network/skill_vfx_wiring_test.rs`；`server/src/combat/zhenmai_v2.rs` / `burst_meridian.rs` / `server/src/npc/npc_skill.rs`（NPC 三招改 `bong:npc_*` 专属 id）；client 粒子 player 与注册 |
| **P6** 回归收口 | `AnimCastTicksAlignmentTest`（`FIXED_PHASE_CHARGE_SKILLS` / `TWO_STAGE_PAIRS` / `PHASE_HANDOFF_BUDGET_RAD` + 4 条新用例）；`client/src/test/java/com/bong/client/animation/TwoStageHandoffBlendTest.java`（新增，4 例）；`server/src/network/vfx_animation_trigger.rs`（3 条契约 pin + 2 个取值助手）；`client/tools/gen_stance_{woliu,zhenmai}.py` + 两份重制 JSON + `client/src/test/resources/bong/anim_spec_manifests/stance_{woliu,zhenmai}.json`；`client/tools/renders/stance_p6/`（三视图存档 + checklist）；两项裁决入档 `docs/player-animation-conventions.md` §14 |

### 关键 commit

| hash | 日期 | 一句话 |
|---|---|---|
| `ab15e6474` | 2026-07-18 | PR-1：P0 审计矩阵 + 精度标准 + cast_ticks 对拍防回归（#1234） |
| `79db7d898` | 2026-07-19 | PR-2：P1 批次一 9 招高频短招重制（#1235） |
| `a2a162ce3` | 2026-07-19 | PR-3：P2 前半 6 招去复用专属化（#1239） |
| `2bef48b40` | 2026-07-19 | PR-4：P2 后半 长引导两段式 + StopAnim 停止路径接线（#1240） |
| `5d9bdd8fe` | 2026-07-19 | PR-5：P3 批次三 借用招专属化 + 缺失补齐 + 短招精修群（#1241） |
| `610ed31f3` | 2026-07-19 | PR-6：P4 yidao 5 招两段式动画补齐（矩阵外欠账清偿）（#1243） |
| `14f8f5e1f` | 2026-07-20 | PR-7：P5 粒子去复用（11 招脱离借用 + 双端闭环接线矩阵）（#1244） |
| `bd0b4d975` | 2026-07-21 | P6 裁决：heaven_gate 改判据 + 两段式相位承接契约采纳 fade 混合 |
| `5f95a97c3` | 2026-07-21 | P6：server 侧补相位承接契约与架势亮相的 payload pin |
| `8bac838b0` / `b1b67d15a` / `cfedb6656` | 2026-07-21 | P6 架势亮相重制 round 1/2/3（终轮附 `<PROMISE>` + 三视图存档） |
| `c72aa4228` | 2026-07-21 | P6：补全 plan 全量动画资源 pin（`BongAnimationRegistry.contains`，56 条） |
| `2ee733501` | 2026-07-21 | P6：补通用停止路径的相位承接契约守卫（主干核验补漏） |
| `c75c5c63e` | 2026-07-21 | review r1 #1/#3 资产侧：strike 发力 easing 修正（含 `anqi_single_snipe` off-by-one）+ 亮相 description 收敛 |
| `529f59bfb` | 2026-07-21 | review r1 #1 锁侧：`strikePhaseCarriesEaseInDrive` 机械锁 + instant 豁免反向核验（三发变异验证） |

### 测试结果

- **server**：`cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` —— FMT:0 / CLIPPY:0 / TEST:0，**11871 passed / 0 failed / 6 ignored**（5 个 test binary 汇总，无非 ok）。P6 的净增量为 **+4**，由 diff 机械证明：`git diff origin/main...HEAD -- server/` 只触及 `vfx_animation_trigger.rs` 与 `cast_emit.rs` 两个文件，新增 `#[test]` 4 条、删除 0 条。（本行数字为 **review r1 返工后**的实测值；r1 未改动任何 server 代码，4 条净增全部来自 P6 本体的 3 条 payload pin + `2ee733501` 补的通用停止路径相位承接守卫 1 条。）
  > 口径说明：PR-7 记录的 11856 是**该 PR 本地门禁**的数字，与本分支 base（`origin/main` = PR-7 的 merge commit `14f8f5e1f`）实测不一致——按上述 diff，差额不由 P6 引入（本 PR 只增不减测试）。以本节实测值为准。
- **client**：`cd client && ./gradlew test build` —— BUILD SUCCESSFUL，**4222 tests / 0 failures / 0 skipped**（PR-7 基线 4213 之上净增 **+9**，与 diff 一致：`git diff origin/main...HEAD -- 'client/src/test/**'` 新增 `@Test` 9 条、删除 0 条；`AnimCastTicksAlignmentTest` +5、`TwoStageHandoffBlendTest` +4）。`AnimCastTicksAlignmentTest` 单类 **13 passed**。第 9 条即 review r1 新增的 `strikePhaseCarriesEaseInDrive`（12 → 13）。
- **归档后复跑**：`git mv` 入 `docs/finished_plans/` 后 client 门禁**再跑一次**并绿（上列 4222/0 即归档后的数字）。`SkillParticleSpecDocTest` 经 `client/build.gradle:215-229` 的双路径 task input 与测试内 `PLAN_CANDIDATES` 双候选，路径变更后仍能定位 plan §P5.1 表格；并已核对其 JUnit XML 时间戳确认该类**确实重新执行**（4 例全绿）而非被判 UP-TO-DATE 跳过——「只改真源不改测试源就静默跳过」正是该 task input 要防的漂移。
- **突变验证（防空测试）**：相位预算降到 40°（低于实测最大 46°）→ `twoStageHandoffHoldsAcrossEveryReachableLoopPhase` 撞红；`FIXED_PHASE_CHARGE_SKILLS` 期望 endTick 改 61 → `fixedPhaseChargeSeamIsExactAndNonLooping` 撞红；资源 pin 塞入不存在的 id → `everyPlanAnimIdResolvesThroughProductionRegistry` 撞红。
  - **review r1 新锁的三发变异**（`strikePhaseCarriesEaseInDrive`，全部撞红、恢复后复绿）：① `stance_woliu` strike 三帧改回 `INOUTSINE` → FAILED，报文列出实际 easing `{8=INOUTSINE, 12=INOUTSINE, 16=INOUTSINE}` 并指明发力帧须落在段起始侧；② `anqi_single_snipe` 还原 `t4=OUTSINE / t6=INQUAD` 的 off-by-one 形态 → FAILED；③ 给**未登记**的 `beng_quan` manifest 强加 `instant: true` 试图绕过豁免 → FAILED。第 ③ 发专测「豁免通道本身不可被滥用」，是本锁与 PR-7 r1「只测枚举映射」式失效的关键区别。
- **e2e**：`bash scripts/smoke-test-e2e.sh` 由 CI `e2e` job 在 PR HEAD 执行（`.github/workflows/e2e.yml:144`），按 §测试声明 的证据口径引用对应 HEAD 的绿色 job。

### 跨仓库核验（命中 symbol）

- **server**：`vfx_animation_trigger::{emit_sword_path_visual_triggers, emit_anqi_visual_triggers, emit_technique_learned_stance_triggers, emit_dugu_needle_visual_triggers, emit_woliu_v1_vortex_visual_triggers}`；`ANIM_SWORD_HEAVEN_GATE_{CHARGE,RELEASE}` / `ANIM_ANQI_CHARGE{,_RELEASE}` / `ANIM_STANCE_{WOLIU,ZHENMAI}`；`cast_emit::{looping_cast_anim_id, cast_loop_stop_anim_request, CAST_LOOP_ANIM_*_FADE_OUT_TICKS}`；`combat::yidao::{YidaoSkillId::loop_anim_id, release_anim_id, yidao_loop_anim_for_skill_id}`；`combat::sword_basics::{ANIM_SWORD_INFUSE_CHARGE, ANIM_SWORD_INFUSE_RELEASE}`；`sword_path::heaven_gate::HEAVEN_GATE_CHARGE_END`；`cultivation::technique_cast_ticks_snapshot_test`；`network::skill_vfx_wiring`。
- **client**：`BongAnimationRegistry.{contains, get, sourceOf}`；`BongAnimationPlayer.{playOnStack, stopOnStack}`；`AnimationLayerManager.{playOnStack, Channel.UPPER_BODY, channelForPriority}`；`ClientAnimationBridge.{playAnim, stopAnim}`；`VfxEventRouter`；测试侧 `AnimCastTicksAlignmentTest.{SKILL_ANIM, TWO_STAGE_PAIRS, FIXED_PHASE_CHARGE_SKILLS, INSTANT_RESOLVER_SKILLS, CAST_ALIGNMENT_ALLOWLIST, P0_BASELINE, strikePhaseCarriesEaseInDrive}` / `TwoStageHandoffBlendTest` / `ProductionAnimationResources.loadViaProductionReloadCallback`。
- **agent**：本 plan 纯表现层，**不涉及** agent 侧 schema 与 IPC（§8.1 #1 决议：cast_ticks / 冷却 / 伤害零改动）。
- **共享契约**：`bong:vfx_event` 的 `PlayAnim { anim_id, priority, fade_in_ticks }` / `StopAnim { anim_id, fade_out_ticks }`（`server/src/schema/vfx_event.rs`）；checked-in 快照 `technique_cast_ticks_snapshot.json`（server 单向生成 → client 只消费）。

### 遗留 / 后续

**本 plan 范围内已知余项**

1. `CAST_ALIGNMENT_ALLOWLIST` 余 **1 条** `woliu.vortex_resonance`：归 `docs/plans-skeleton/plan-bughunt-woliu-resonance-loop-arm-decay-v1.md`（**未消费骨架**，P0 ⬜）。实测 11 轴违反 endTick 同值补帧。该骨架被消费并 merge 后，`allowlistEntriesActuallyFailAlignment` 会因「条目已达标」立刻撞红，**强制**删除该条目——棘轮自带回收机制，无需人工记得。
2. `woliu_vortex_resonance` 至今**无 spec manifest**（故不受 `specManifestsEnforcePrecisionStandardMechanically` 覆盖）；上述 bugfix 落地时应同批补一份 `{"segment":"loop"}` manifest。
3. 架势亮相目前只在「习得时刻」单发。若日后落地**持续架势 gameplay 状态**需要循环形态，按 §8.1 #2 第 4 条原话另议新增循环资产，**不要**把这两张亮相图改回 `isLoop`（会重新引入无停止路径的红线违例）。
4. **easing 管辖方向的存量未审**（review r1 副产物，建议单独 bugfix plan）：r1 返工期间读 PlayerAnimator 源码确认了本仓 easing 的真实语义是 `before.ease`——**某帧的 easing 管辖「本帧 → 下一帧」**（`isEasingBefore` 默认 false + 全仓 140 份动画无一声明 `easeBeforeKeyframe`；完整推导与正例落 conventions §15）。据此新设的 `strikePhaseCarriesEaseInDrive` 当场揪出两处写反：`stance_woliu`（strike 全段 INOUTSINE，无发力）与 `anqi_single_snipe`（生成器 docstring 写明要 easeIn，但 INQUAD 放在**顶点帧**上，实际管的是 recovery，strike 反被 OUTSINE 管成减速），两处均已修。**但本锁只覆盖 strike 段「是否存在发力帧」这一条**；anticipation 段的 easeOut、recovery 段的 easeInOutSine 是否也存在同类 off-by-one，**尚未全量审计**——`anqi_single_snipe` 的形态说明这类错误能长期潜伏且不被任何测试察觉（它甚至通过了自己批次的三视图人工验收，因为 `render_animation.py` 出图用线性插值、**对 easing 完全不敏感**，肉眼验收天然看不见 easing 错误）。建议立 bugfix plan 全量扫 140 份资产的三段 easing 方向，并把 anticipation/recovery 两段的族别一并机械化。

**PR-7 移交的五条遗留（本 plan 范围外，登记不实施）**

1. `ArmorProfileStoreCrossCheckTest` 的 gradle task input **未声明**：该测试跨目录读 `server/assets/combat/armor_profiles/*.json`，但 `client/build.gradle` 只为 `SkillParticleSpecDocTest` 声明了 input（`:215-229`）。只改 server 侧 profile JSON、不动 client 测试源时 `:test` 会判 UP-TO-DATE 直接跳过，漂移静默溜过——与 PR-7 为 spec doc 修的是同一类窗口。修法：同一 `tasks.named('test')` 块内加 `inputs.dir('../server/assets/combat/armor_profiles').withPropertyName('serverArmorProfiles')`。PR-7 r2 按范围纪律移出。
2. client 侧字面量 `12` 与 server `NI_MAI_HU_TI_AURA_PARTICLE_LIFETIME_TICKS` **无机器对拍**。要闭死需给 `SkillVfxWiring` 表加 `duration` 字段并重新生成 manifest。
3. 环绕粒子的 `tick()` / `createParticle()` **无测试覆盖**（需真 `ClientWorld`）。暴露面已缩到 6 行委托，风险有限。
4. `SpawnParticle.strength` **无区间校验**（schema 层）。
5. `server/src/network/gameplay_vfx.rs:90` 的 `f32::clamp` 对 `NaN` 返回 `NaN`，导致整包被静默丢弃——真缺陷，建议单独 bugfix plan（`clamp` 前先判 `is_finite`）。

**存量缺口（本 plan 未触碰，顺带登记）**

6. `vfx_animation_trigger.rs` 三个 arm 至今零 pin 测试：`emit_defense_animation_triggers`（格挡 / parry）、`emit_baomai_v3_visual_triggers`（baomai v3 全招）、`emit_scroll_read_stop_for_entity`（读卷停止）；`emit_tribulation_animation_triggers` 仅有间接覆盖（一条「不受结算系统干扰」的非干涉断言）。本 plan 触碰过的 arm 均已配 pin，这四条属既有欠账。

**下游依赖**

7. 梯队三 `plan-fpv-cast-av-v1` P5 以本 plan 通过 TPV 验收的动画为输入，负责真正的第一人称手臂验收（本 plan 的 FPV 冒烟为非阻塞项）。

