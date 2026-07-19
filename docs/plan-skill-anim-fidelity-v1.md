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

**④ A/V 复用清单**（去复用 = 本 plan 差异化目标）：

- 动画复用：sword_path `condense_edge`/`resonance` 借 `sword_cleave`、`qi_slash` 借 `sword_thrust`；anqi 6 招全借通用（`windup_charge`/`cast_invoke`/`release_burst`/`sword_stab`）；burst_meridian `tie_shan_kao`/`xue_beng_bu` 借 `beng_quan`、`ni_mai_hu_ti` 无动画（anim_id: None）。
- 粒子复用：zhenmai 直接复用剑气 `SwordQiSlashPlayer`（`jiemai_*` 事件）；burst_meridian 全系共用 `bong:burst_meridian_beng_quan`；npc 3 招各借医道/真脉/崩拳粒子。
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
| P0 | 全量审计矩阵落档 + 精度标准定稿 + 时长对齐自动对拍测试 | ⬜ |
| P1 | 批次一重制：sword 基础 3（infuse 移 P2，见附录 A）+ beng_quan + zhenmai 5（高频主力短招） | ✅ 2026-07-19 |
| P2 | 批次二：sword_path 5 专属化 + anqi 6 专属化 + sword_infuse 两段式（去复用 + 长引导循环段）。拆前半（2026-07-19：去复用 6 招专属化——sword_path condense_edge/qi_slash/resonance + anqi single_snipe/multi_shot/soul_inject，含 server 映射改指 + allowlist 删 5 条）+ 后半（2026-07-19：charge_carrier / sword.infuse 真两段式 + StopAnim 三类通道接线 §8.1 #3 + echo_fractal / armor_pierce / manifest 瞬发结算型分类契约（review r2 定形——strike 顶点=tick 0 与结算同帧，INSTANT_RESOLVER_SKILLS + instant manifest 机械锁，出 allowlist）+ heaven_gate 双段密度精修 + allowlist 净删 4 条） | ✅ 2026-07-19 |
| P3 | 批次三：burst_meridian 3 借用招专属化 + ni_mai_hu_ti 新增 + dugu 2 / tuike 3 / woliu 短招精修 | ✅ 2026-07-19 |
| P4 | yidao 5 招动画补齐（plan-yidao-v1 §5 欠账） | ✅ 2026-07-19 |
| P5 | 粒子去复用：zhenmai 专属 player + burst_meridian 家族分化 + npc 3 招分化 | ⬜ |
| P6 | 回归收口：资源 pin 测试 + FPV/TPV 实机验收 | ⬜ |

## P0 — 审计矩阵 + 标准定稿 + 对拍测试

- 49 招全量矩阵表落档到本 plan 附录：`skill_id / cast_ticks / anim_id / endTick / 帧点数 / 轴关键帧数 / 是否模板产物 / 是否借用 / 差距分级（A 达标 / B 精修 / C 重制 / D 缺失）`。
- **时长对齐自动对拍测试**（client 侧）：读 `player_animation/*.json` 元数据 + 一份 `cast_ticks` 快照表，按招式类型分三套断言（与精度标准 #2 严格同一时序模型，不用宽区间混过）：
  - 普通非循环招（2 < cast < 40）：`endTick ∈ [cast+4, cast+8]`（recovery 红线直接入断言）；
  - 瞬发招（cast ≤ 2）：总长 ∈ [6, 12]；
  - 长引导招（cast ≥ 40）：蓄力段动画 isLoop 且每个用到的轴在 endTick 有同值补帧（库坑 #1 边界）、release 段动画独立存在且两段 id 均被 server 映射表发射。
- **精度红线机械化断言**（同一测试套件）：每份动画随批提交一份结构化 spec manifest（anticipation/strike/recovery 的 tick 边界 + 每段帧点数），测试逐项断言：三段各 ≥2 帧点、主要运动轴相邻帧点间隔 ≤4 tick、所有关键帧 easing 显式且主打击轴非 linear、`leg.pitch ≤ 40°`、循环动画每轴 endTick 同值补帧。无法机械判定的重心转移/全身协调，列为逐招人工验收证据：批次 PR 附 `render_animation.py` 三视图 PNG + 对照 checklist。
- **快照单一真源**：快照由 server `TECHNIQUE_DEFINITIONS` 单向生成（server 侧同步测试保证快照=定义，快照缺失/重复/漂移条目直接判红），client 测试只消费不维护——杜绝「错误时长靠同步改快照混过关」。现状不达标项进 allowlist，逐批清空——allowlist 清零 = P1-P4 完成的机械判据；**allowlist 只允许缩小**，任何新增条目必须在 PR body 显式说明理由。
- 精度标准（上节）随 P0 一并进 `docs/player-animation-conventions.md`（该文档为动画约定正典，本 plan 允许追加不允许改写既有段落）。

## P1-P4 — 分批重制（每批同构）

每批交付物：`gen_<anim>.py` 生成脚本 + 重制 JSON + `render_animation.py` 三视图对照（round 1/2/3 commit）+ 终轮 `<PROMISE>` 块 + allowlist 对应条目删除。批内每招须给出 P1 范例 spec 同精度的骨骼数值表（写在各批 PR body，plan 只锁标准与清单）。

- **P1**（高频短招，玩家看得最多）：`sword_{cleave,thrust,parry}`、`beng_quan`、`zhenmai_{parry,neutralize,multipoint,harden,sever_chain}`。瞬发招按标准 #2 做爆发帧+收势。（`sword_infuse` cast=40 属长引导域，移 P2，见附录 A。）
- **P2**（去复用 + 长引导）：sword_path 5 招各自专属动画（`condense_edge` 凝锋收剑入鞘式 / `qi_slash` 远程挥斩 / `resonance` 双手持剑共鸣颤 / `manifest` 已有 / `heaven_gate` 已有两段式，精修）；anqi 6 招专属（`charge_carrier` cast=400 → 循环封骨结印段 + 完成收势；`echo_fractal` cast=60 → 循环撒饵段 + 4 tick 爆发保留为 release）；`sword_infuse` cast=40 拆「循环蓄力段 + release 段」两段式（含 server 通道接线）。
- **P3**：`tie_shan_kao`（靠身撞击，与崩拳出拳区分）、`xue_beng_bu`（步法位移）、`ni_mai_hu_ti`（护体结印，当前 anim_id: None 补新）、dugu 2 / tuike 3 / woliu **基础与进阶**短招（`vacuum_palm`/`woliu.burst` 等 8-10 tick 快闪项）按标准精修。**明确排除涡流虚蚀 5 招**（`ambient_vortex`/`void_vortex`/`swallowing_vortex`/`vortex_echo`/`void_core`——其动画从无到有的补齐归 active `plan-bughunt-woliu-voidpath-missing-animations-v1`）；若将来需对其产物做二次精修，作为该 plan merge 后的后置依赖另列批次，且只改既有 JSON 精度、不新增动画、不动发射链。
- **P4**：yidao 5 招按 plan-yidao-v1 §5 表格逐条落地（针灸双手持针 30 穴位序 / 灸火对掌 / CPR 按压 / 续命喂丹+接天引 / 环阵持法器），server 侧 yidao emit 补 `PlayAnim`（当前 yidao 无动画常量）。（✅ 2026-07-19 交付：5 招通道核验全部为 `resolve_yidao_skill → insert_casting` 真实长引导窗（cast_ticks_base 100-1200t，`yidao_cast_ticks` 按 mastery/平和色缩放可变窗）→ 全部两段式——蓄力循环段 20-32t isLoop 覆盖任意窗长 + release 收势段 12-14t 三段式；server 10 动画 id 常量 + 起手 PlayAnim + `looping_cast_anim_id` yidao 分表登记（三打断/自然完成 StopAnim）+ `complete_yidao_casts` 有效结算分支 release 接力（无效完成不奖励收势，sword_infuse 先例同语义）；cast_ticks 快照真源扩展为 `TECHNIQUE_DEFINITIONS` + `yidao_skill_spec` 单向合并；client `SKILL_ANIM` +5、10 份 spec manifest，allowlist 零新增。）

## P5 — 粒子去复用

按 docs/CLAUDE.md §四 视听精度要求逐条写 spec 再实施（基类/数量/lifetime/速度方向/颜色 hex/spawn 模式/贴图复用或新增/VfxPlayer 类名/event_id）：

- zhenmai：弃借 `SwordQiSlashPlayer`，新建 `ZhenmaiPulsePlayer`（`BongLineParticle` 短脉冲 + `BongSpriteParticle` 穴位点，色系 #D4AF6A 金脉），5 招各自 event_id（`bong:zhenmai_{parry_flash,neutralize_dust,multipoint_ring,harden_shell,sever_snap}`），贴图复用既有 `qi_aura`/`lingqi_ripple` 不新增。
- burst_meridian：`tie_shan_kao` 撞击冲击环（GroundDecal）、`xue_beng_bu` 步法残影（Ribbon 短尾）、`ni_mai_hu_ti` 体表逆流纹（Sprite 环绕），共用色系 #C58B3F 但形态分化。
- npc 3 招：脱离借用，各给独立 event_id（形态可简，但 id 与颜色必须独立，保证旁观读招）。
- **双端闭环接线矩阵（P5 交付物）**：每个新 event_id 一行——`招式 id / server 发射点（resolver 或 emit system 文件）/ SpawnParticle event_id / client VfxPlayer 类名 / VfxBootstrap 注册行`。矩阵同源生成一份共享 event_id 清单驱动双端测试：server 侧逐项断言对应招式事件发出正确 `SpawnParticle`（旧借用 id 不再发出的负向断言一并锁）；client 侧逐项断言 `VfxRegistry` 已注册同一 id 并返回预期 player；再加集合一致性断言（发射集合 == 注册集合，防两端字符串各自漂移）+ 未注册 id 走 bridgeMiss 不崩溃的错误分支。

## P6 — 回归收口

- 动画资源 pin 测试：`BongAnimationRegistry.contains` 断言本 plan 全部新增/重制 anim_id 可解析。
- P0 对拍测试 allowlist 清零。
- **TPV 实机验收（完成判据）**：`render_animation.py` 三视图存档 + 远距离旁观者读招录屏对照——「能从姿态分辨对面在用 X 不是 Y」。
- **FPV 兼容性冒烟（非阻塞，不作为完成判据）**：现状第一人称渲染路径（`THIRD_PERSON_MODEL`）不回归即可。真正的第一人称手臂验收归梯队三 `plan-fpv-cast-av-v1` P5——避免用尚未落地的下游能力当本 plan 完成条件（梯队三反过来以通过本 plan TPV 验收的动画为输入）。
- server 侧映射表单测：`vfx_animation_trigger.rs` 新增/改动的 arm 各配 pin 测试。

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

> §8 原表保留作历史回溯。全部已在 §8.1 收口，**实施时以 §8.1 决议为准**。

## 测试声明

- client：动画 JSON 元数据对拍测试（分类型时长断言 / 三段 manifest 帧点下限 / 主轴帧间隔 ≤4 / easing 显式且打击轴禁 linear / leg.pitch 上限 / 循环每轴 endTick 补帧 / 快照缺失/重复/漂移判红）+ 资源 pin + 粒子 registry 集合一致性（gradlew test）；
- server：`vfx_animation_trigger` 映射 arm 单测（含借用改专属后旧 id 不再发出的负向断言）+ P5 粒子发射 pin + cast_ticks 快照单向同步测试（cargo test）；
- 实机：每批 `render_animation.py` 三视图存档 + 人工验收 checklist（重心/协调等非机械项）+ P6 TPV 读招验收（FPV 仅非阻塞冒烟）；
- e2e：`bash scripts/smoke-test-e2e.sh` 绿。

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

**P4 后（2026-07-19）**：矩阵外欠账清偿——yidao 5 招（plan-yidao-v1 §5 ①-⑤）全部两段式补齐入对拍契约：`SKILL_ANIM` +5（映射蓄力段）、cast_ticks 快照真源扩展为 `TECHNIQUE_DEFINITIONS` + `yidao_skill_spec` 单向合并（49→54 键，server 同步测试锁双源不撞名、重生成入口不变）、10 资产（5 loop + 5 release）+ 10 spec manifest（loop 段 segment 型 / release 三段式型），**allowlist 零新增**（5 招直接达标长引导判据：isLoop + 全轴 endTick 同值闭环；棘轮基线不动）。姿态语言逐招独立可辨：①俯身右针左探四落点施针（bow 补偿）②直立双掌对拢灸火推送 ③深俯直臂 CPR 双压深浅起伏 ④左手喂丹右臂对天接引定格 ⑤双手捧法器高举环阵横扫；release 收势互异：提针直身拂袖 / 双臂横向开扇散烟 / 侧耳俯听直起 / 单臂自天纵贯合封 / 对称沉落抱器。停止路径全链闭环（§13 #6）：起手 loop PlayAnim（`resolve_yidao_skill`）→ 三打断分支 + 自然完成分支表驱动 StopAnim（`looping_cast_anim_id` → `yidao_loop_anim_for_skill_id` 分表）→ 有效结算 release 接力 / 无效完成不奖励收势（`complete_yidao_casts`）；事件路径 pin：cast 起播 loop、成功完成恰发 release、无效完成零 release、移动打断只 StopAnim 且无完成事件。

**P4 打磨记录（2026-07-19/20，3 轮）**：10 资产（5 loop BASE-inherit 机械闭环 + 5 release 三段式）——(round 1/3) 逐招第一性原理通道核验（结论以函数锚写入各 gen 脚本 docstring：5 招同走 `resolve_yidao_skill` 真实引导窗、完成链 cast_emit → `complete_yidao_casts`）后参数化 first cut → (round 2/3) 机械四查复跑（循环缝合按 returnTick 回绕锚点同值 / leg.pitch≤40°，实测最大帧距 4t、无 linear easing）**ALL CLEAN**；`render_animation.py` 三视图 grid 目检 5 loop 姿态语言互异可辨（俯身左右分点施针 vs 直立对掌推送起伏 vs 深俯中线合掌按压 vs 右臂朝天左臂喂丹不对称 vs 双臂举过头顶环视）+ 5 release 收势轨迹互异且终帧回中立；新增机械项「loop 基位 → release 承接帧」对拍：4/5 完全一致（0 轴偏差），`mass_meridian_repair` 7 轴微差系 `torso.yaw` 在循环内 ±16° 环视摆动、release 取中位锚点所致（该轴不存在可精确承接的单一相位），仍显著优于已 merge 两段式先例（sword.infuse 9 轴 / charge_carrier 12 轴 / heaven_gate 16 轴偏差）——判定零缺陷无资产增量改动（记录并入终轮，PR-4 先例）→ (round 3/3) 决定性再生成 10/10 字节一致 + 双栈门禁于最终资产之后真跑全绿（server FMT:0 / CLIPPY:0 / TEST:0，11821 passed 0 failed；client `gradlew test build` BUILD SUCCESSFUL，AnimCastTicksAlignmentTest 8/8 含新 10 份 manifest 机械断言），终轮 commit 附 `<PROMISE>` 担保。**执行说明**：round 1 由实施 subagent 交付后撞 session 限额终止（transcript 丢失不可续跑），round 2/3 核验与收口由主干接手完成。
