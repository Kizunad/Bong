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
| P1 | 批次一重制：sword 基础 3（infuse 移 P2，见附录 A）+ beng_quan + zhenmai 5（高频主力短招） | ⬜ |
| P2 | 批次二：sword_path 5 专属化 + anqi 6 专属化 + sword_infuse 两段式（去复用 + 长引导循环段） | ⬜ |
| P3 | 批次三：burst_meridian 3 借用招专属化 + ni_mai_hu_ti 新增 + dugu 2 / tuike 3 / woliu 短招精修 | ⬜ |
| P4 | yidao 5 招动画补齐（plan-yidao-v1 §5 欠账） | ⬜ |
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
- **P4**：yidao 5 招按 plan-yidao-v1 §5 表格逐条落地（针灸双手持针 30 穴位序 / 灸火对掌 / CPR 按压 / 续命喂丹+接天引 / 环阵持法器），server 侧 yidao emit 补 `PlayAnim`（当前 yidao 无动画常量）。

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

- 单 plan 多 PR 序列化：PR-1 = P0（审计+标准+对拍测试）；PR-2/3/4 = P1/P2/P3 批次；PR-5 = P4 yidao；PR-6 = P5 粒子；PR-7 = P6 收口。前一 PR merge 后开下一个。
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
| `sword.thrust` | 10 | `sword_thrust` | 16 | — | 8 | 170 | — | **A** | P1 | 专属；P1 批次一重制（2026-07-19）：收剑腰侧直刺+侧身送肩 |
| `sword.parry` | 4 | `sword_parry` | 10 | — | 6 | 126 | — | **A** | P1 | 专属；P1 批次一重制（2026-07-19）：斜举格挡弹开，密度补齐 |
| `sword.infuse` | 40 | `sword_infuse` | 40 | — | 29 | 29 | — | **B** | P2 | 专属；无 recovery、密度低；cast=40 属长引导域，按标准 #2 须两段式，移 P2 与 anqi 长引导批次同做（含 server 通道接线） |
| `movement.dash` | 0 | `dash_forward` | 4 | — | 13 | 21 | — | **C** | P3 | 瞬发 4t 快闪；密度低 |
| `shield_block` | 0 | `shield_raise` | 6 | ✓ | 35 | 35 | — | **B** | P3 | 循环举盾+StopAnim 闭环已有；密度低 |
| `burst_meridian.beng_quan` | 8 | `beng_quan` | 14 | — | 9 | 192 | — | **A** | P1 | 专属；P1 批次一重制（2026-07-19）：沉马蓄劲→拳炸出→震颤收；endTick=14 为三借用方 cast 区间交集 |
| `burst_meridian.tie_shan_kao` | 10 | `beng_quan` | 14 | — | 9 | 192 | ✓ | **D** | P3 | 借 beng_quan（靠撞≠出拳）；时长随重制达标已出 allowlist，专属化仍归 P3 |
| `burst_meridian.xue_beng_bu` | 6 | `beng_quan` | 14 | — | 9 | 192 | ✓ | **D** | P3 | 借 beng_quan（步法≠出拳）；时长随重制达标已出 allowlist，专属化仍归 P3 |
| `burst_meridian.ni_mai_hu_ti` | 12 | —— | — | — | — | — | — | **D** | P3 | anim_id: None（burst_meridian.rs:637） |
| `baomai.full_power_charge` | 1 | `windup_charge` | 16 | ✓ | 81 | 81 | ✓ | **D** | P3 | 借通用蓄力（loop+StopAnim 闭环在） |
| `baomai.full_power_release` | 1 | `release_burst` | 4 | — | 81 | 81 | ✓ | **D** | P3 | 借通用爆发（4t 模板） |
| `zhenmai.parry` | 1 | `zhenmai_parry` | 8 | — | 6 | 126 | — | **A** | P1 | 专属；P1 批次一重制（2026-07-19）：瞬发单手拍挡爆发帧+收势 |
| `zhenmai.neutralize` | 4 | `zhenmai_neutralize` | 10 | — | 7 | 144 | — | **A** | P1 | 专属；P1 批次一重制（2026-07-19）：双掌下按化劲+沉桩 |
| `zhenmai.multipoint` | 6 | `zhenmai_multipoint` | 12 | — | 10 | 211 | — | **A** | P1 | 专属；P1 批次一重制（2026-07-19）：连环三点指（右高/左中/右深） |
| `zhenmai.harden` | 5 | `zhenmai_harden` | 11 | — | 8 | 163 | — | **A** | P1 | 专属；P1 批次一重制（2026-07-19）：抱臂沉桩硬化+紧咬 clench |
| `zhenmai.sever_chain` | 8 | `zhenmai_sever_chain` | 14 | — | 9 | 194 | — | **A** | P1 | 专属；P1 批次一重制（2026-07-19）：手刀横斩断链+overshoot |
| `woliu.vortex` | 1 | —— | — | — | — | — | — | **D** | P3 | v1 涡流术无动画发射（combat/woliu.rs 零 PlayAnim） |
| `woliu.hold` | 1 | `vortex_palm_open` | 12 | — | 14 | 40 | — | **B** | P3 | 12t/40KF；瞬发标准内但密度可精修 |
| `woliu.burst` | 1 | `palm_strike` | 6 | — | 81 | 81 | ✓ | **D** | P3 | 借通用 palm_strike（6t 模板 81KF） |
| `woliu.mouth` | 6 | `palm_thrust` | 12 | — | 105 | 105 | ✓ | **D** | P3 | 借通用 palm_thrust |
| `woliu.pull` | 5 | `woliu_vacuum_lock` | 10 | — | 8 | 32 | ✓ | **D** | P3 | 与进阶 vacuum_lock 共用 |
| `woliu.heart` | 10 | `vortex_spiral_stance` | 20 | — | 15 | 53 | — | **B** | P3 | 20t/53KF；可精修 |
| `woliu.vacuum_palm` | 6 | `woliu_vacuum_palm` | 8 | — | 6 | 18 | — | **C** | P3 | 8t/18KF 快闪 |
| `woliu.vortex_shield` | 10 | `woliu_vortex_shield` | 18 | ✓ | 9 | 33 | — | **B** | P3 | 18t loop/33KF；停止路径待核 |
| `woliu.vacuum_lock` | 8 | `woliu_vacuum_lock` | 10 | — | 8 | 32 | — | **C** | P3 | 10t/32KF 快闪 |
| `woliu.vortex_resonance` | 80 | `woliu_vortex_resonance` | 80 | ✓ | 9 | 25 | — | **A** | — | 80t loop 对齐 cast=80 ✓（时长正例）；对拍实测 10 手臂轴 endTick 无补帧（库坑 #1）——存量 bug 归 `plan-bughunt-woliu-resonance-loop-arm-decay-v1`（防重排除），修复前暂驻 allowlist |
| `woliu.turbulence_burst` | 40 | `woliu_turbulence_burst` | 40 | — | 11 | 35 | — | **B** | P3 | 40t/35KF；cast≥40 应两段式 |
| `dugu.shoot_needle` | 1 | `dugu_needle_throw` | 8 | — | 13 | 35 | — | **C** | P3 | 8t/35KF 快闪 |
| `dugu.infuse_poison` | 1 | `dugu_needle_throw` | 8 | — | 13 | 35 | ✓ | **D** | P3 | 与凝针共用一条动画 |
| `tuike.don` | 12 | `tuike_don_skin` | 16 | — | 16 | 48 | — | **B** | P3 | 16t/48KF；补 recovery 即达标 |
| `tuike.shed` | 8 | `tuike_shed_burst` | 12 | — | 20 | 56 | — | **B** | P3 | 12t/56KF；可精修 |
| `tuike.transfer_taint` | 10 | `tuike_taint_transfer` | 14 | — | 19 | 55 | — | **B** | P3 | 14t/55KF；可精修 |
| `anqi.charge_carrier` | 400 | `windup_charge` | 16 | ✓ | 81 | 81 | ✓ | **D** | P2 | 借通用蓄力；cast=400 需专属循环结印段+release |
| `anqi.single_snipe` | 6 | `sword_stab` | 8 | — | 124 | 124 | ✓ | **D** | P2 | 借剑刺（暗器≠剑） |
| `anqi.multi_shot` | 30 | `release_burst` | 4 | — | 81 | 81 | ✓ | **D** | P2 | 借通用爆发；4t vs cast=30 错配 26t 静止 |
| `anqi.soul_inject` | 20 | `cast_invoke` | 15 | — | 75 | 75 | ✓ | **D** | P2 | 借通用施法 |
| `anqi.armor_pierce` | 40 | `cast_invoke` | 15 | — | 75 | 75 | ✓ | **D** | P2 | 借通用施法；15t vs cast=40 错配 |
| `anqi.echo_fractal` | 60 | `release_burst` | 4 | — | 81 | 81 | ✓ | **D** | P2 | 借通用爆发；4t vs cast=60 错配 56t 静止 |
| `body.guangbo_ticao` | 60 | `guangbo_ticao` | 150 | — | 288 | 288 | — | **A** | — | 150t/288KF 高完成度 |
| `sword_path.condense_edge` | 12 | `sword_cleave` | 16 | — | 19 | 19 | ✓ | **D** | P2 | 借基础横劈 |
| `sword_path.qi_slash` | 20 | `sword_thrust` | 10 | — | 20 | 20 | ✓ | **D** | P2 | 借基础刺击 |
| `sword_path.resonance` | 30 | `sword_cleave` | 16 | — | 19 | 19 | ✓ | **D** | P2 | 借基础横劈；16t vs cast=30 错配 |
| `sword_path.manifest` | 40 | `sword_manifest_cast` | 40 | — | 32 | 32 | — | **B** | P2 | 40t/32KF 对齐 cast；补 recovery+密度 |
| `sword_path.heaven_gate` | 80 | `sword_heaven_gate_charge(+release)` | 60+20 | — | 32+24 | 32+24 | — | **B** | P2 | 两段式先例；charge 60t 非循环 hold 末帧，精修密度 |
| `npc.heal_basic` | 20 | —— | — | — | — | — | — | **N/A** | P5 粒子 | NPC mob 无 PlayAnim 通道（§8.1 #2） |
| `npc.buff_speed` | 10 | —— | — | — | — | — | — | **N/A** | P5 粒子 | 同上 |
| `npc.buff_defense` | 10 | —— | — | — | — | — | — | **N/A** | P5 粒子 | 同上 |
| `morph.yixing` | 60 | `morph_cast` | 30 | — | 42 | 42 | — | **C** | P3 | 30t vs cast=60 错配 30t 静止（§8.1 #2） |

**分级统计**（P0 初判）：A×2 / B×13 / C×12 / D×19 / N-A×3（46 玩家招中 B+C+D = 44 条入 P1-P4 重制/精修清单，与 §8.1 #2 决议一致）。

**P1 批次一后（2026-07-19）**：A×11 / B×11 / C×5 / D×19 / N-A×3——9 条重制达标转 A（sword 基础 3 + beng_quan + zhenmai 5），sword.infuse 移 P2 长引导批次；剩余 B+C+D = 35 条随 P2-P4 清空。
