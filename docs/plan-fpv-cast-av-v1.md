# plan-fpv-cast-av-v1 —— 梯队三：第一人称手臂动画 + 施法瞬间 juice + 签名音效资产化

> 一句话主题：三项质感天花板投入——① **第一人称手臂动画**（用户重点）：当前 `BongAnimationPlayer` 写死 `FirstPersonMode.THIRD_PERSON_MODEL`，第一人称持武器施法基本看不到自己的动作，而这是玩家最常用视角；② 施法瞬间 juice：相机抖动/hit-stop 现只在命中结算触发，大招释放瞬间无屏幕反馈；③ 签名音效资产化：全仓零 `.ogg`，~250 个 audio recipe 全是原版音效拼合，给各流派 signature 招配真音源。
>
> 用户拍板（2026-07-17）：手臂动画特别重视，列为本 plan headline（P0-P2）。
>
> 调研来源：2026-07-17 三路并行探查（基线 `origin/main` = `062cf636`）。

## 现状证据

- **第一人称**：`client/src/main/java/com/bong/client/animation/BongAnimationPlayer.java:142` 固定 `dev.kosmx.playerAnim.api.firstPerson.FirstPersonMode.THIRD_PERSON_MODEL`——所有动画在第一人称渲染整个第三人称模型；代码注释实测记录：空手时勉强可见，持物时被 vanilla held item 渲染盖掉。结果：主视角施法零动作反馈。
- **施法 juice**：`client/.../combat/juice/CombatJuiceSystem.java:33` 已有完整 `CameraShakeController`（含 `MixinCamera`）/ `HitStopController` / `EntityTintController` / `KillJuiceController`，但全部由**命中**事件驱动；施法释放瞬间（heaven_gate 开天、full_power_release、turbulence_burst）无任何 shake/FOV 反馈。
- **音效**：全仓无 client 自有 `sounds.json`、无任何 `.ogg`；`SoundRecipePlayer` 机制完备（多层混音/ducking/loop/优先级）但素材 100% 是 `minecraft:` 原版事件叠 pitch 伪造，音色天花板锁死。

## 与既有 plan 的关系（防重声明）

- **`docs/plans-skeleton/plan-combat-event-juice-runtime-bridge-gap-v1`**：该 skeleton 证真了**命中侧** `combat_event` payload 只发六字段、`CombatJuiceSystem` 富字段（UUID/school/direction/kill）拿不到导致 hit-stop 线上失效——那是存量断桥 bug，归其自身修复。本 plan 的 P3 是**施法侧**新增反馈，与其互补不重叠；若其先落地，P3 复用其富化后的 school/tier 通道做流派参数化。
- **`plan-skill-anim-fidelity-v1`（梯队二）**：本 plan 的 FPV 手臂动画以梯队二重制后的高精度第三人称动画为底稿改编，**排期依赖梯队二对应批次完成**；先行的 P0 POC 用现状动画即可。
- **`docs/finished_plans/plan-audio-v1.md`**：audio recipe 体系的出处；本 plan P4 不推翻 recipe 机制，只给 signature 招把 recipe 的音源层从 `minecraft:` 换成 `bong:` 自有 `.ogg` 事件（recipe 混音/总线逻辑原样复用）。
- **`plan-module-wiring-gaps-v2` T13**（client 音频渐出 `MinecraftSoundSink.fadeOutTicks` 被吞）：独立技术债，不并入；若 P4 落地时该项已修则直接受益。

## 接入面 checklist

- **进料**：梯队二重制的 `player_animation/*.json`（FPV 变体底稿）；`CastStateStore`/`CastSyncHandler`（施法时序真相源，`client/.../network/CastSyncHandler.java`）；`server/assets/audio/recipes/*.json` 既有 recipe（P4 换音源层）。
- **出料**：FPV 变体动画 JSON + `BongAnimationRegistry` FPV 查找链；施法 juice 进 `CameraShakeController`/FOV 控制器；`.ogg` + client `assets/bong/sounds.json`（新建）+ recipe 引用 `bong:` 事件。
- **共享类型/event**：复用 `VfxEventPayloadV1::PlayAnim`（FPV 是 client 侧按本地玩家身份的渲染分支，wire 不变）；juice 触发源见 §8 #3（倾向纯 client，零新 payload）；音效仍走 `PlaySoundRecipeRequest`，不新增 schema。
- **跨仓库契约**：wire 层零变更是本 plan 的设计目标（例外见 §8 #3 若拍板 server hint 则加一个可选字段并同步 proto+samples）；agent 不参与。
- **worldview 锚点**：worldview.md §四 招式物理可见性——第一人称是「自己读自己招」的反馈面；末法基调下签名音效走衰败素朴音色（骨裂/锈响/闷雷，禁华丽仙音）。
- **qi_physics 锚点**：不涉及——纯表现层。

## 阶段总览

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | FPV 技术路线 POC（三选一拍板）+ 工具链增强 | ⏳ 路线 A 定形（§8.1 #1，2026-07-22 真机拍板）；PR-1 收口中 |
| P1 | FPV 基础设施：per-anim 第一人称配置 + `_fpv` 变体查找链 | ⏳ 本地玩家 `_fpv` 查找链 + 路线 A config 已落 `BongAnimationPlayer.playOnStack`（opt-in per 变体）；POC harness 已收敛移除 |
| P2 | 主力招 FPV 手臂动画批量产出（3 轮打磨 + PROMISE） | ⏳ `sword_cleave_fpv` round 3/3 定稿：双臂离线 IK 烘焙（右臂 yaw/roll 中线校正 + 左臂**逐 tick** IK 合握，加密防插值脱手），关键帧残差 ≤0.55、中间帧最差 2.43 模型单位，t0=t20 收势闭合。剩余招式 FPV 变体待续 |
| P3 | 施法瞬间 juice：重型招释放 shake/FOV 脉冲（按招参数化） | ⏳ 基础设施已落（纯 client，零 wire 变更；PR #1249）：`CastJuiceProfiles` 4 重型招注册表 + heaven_gate 两段走 `CastFovController.onAnimPlayed` 动画事件驱动（**授权仍是权威 CASTING 武装的 `AnimCastToken`**，动画只定触发时刻；令牌为按到达序的有界队列，连续同招施法不串用）+ `CastFovController` 生命周期状态机（**accepted 只认 `CastStateStore.Origin.SERVER_AUTHORITATIVE`**，本地预测降为候选；identity 门控 arming/按 identity 有界终态记录 `Terminal{FIRED,VOIDED}`/supersession·TTL·容量淘汰均落 VOIDED/IDLE 不清场/死亡·断线·**切世界** teardown）+ `JuiceConfig` 配置持有者 & `JuiceControls` keybind 可调入口（默认 1.0，0=关闭且进行中调 0 双通道立即取消；动作栏回显走 `Text.translatable`）+ `MixinGameRenderer` 加法 FOV 合成；`CastFovControllerTest` 76 + `JuiceConfigTest` 11 + `JuiceControlsTest` 12。**三条 amendment 见下**（参数保留真机值 / 配置持久化不在范围 / P2 动画变更已 revert）。**阶段未完成——阻塞项见「§P3 未完成欠账」**：参数表 5 个 release 目标里 baomai/woliu/anqi 三招在生产中拿不到权威 CASTING，注册项不可达，P3 的主交付（重型招真实释放时有 shake/FOV）只兑现了 2 招 |
| P4 | 签名音效资产化：每流派 1-2 条真 `.ogg` + 管线建立 | ✅ 2026-07-24（纯资产 + 管线，8 条 CC0 真 `.ogg` 落地；**9 条 server recipe 主层/前兆层换 `bong:` 事件**——8 招 signature + heaven_gate charge 前兆，heaven_gate release（`sword_manifest_strike`）+ charge（专属 `heaven_gate_charge`）均在 server 侧；resourcepack 纳入 `bong/sounds` + `bong/sounds.json` + sha1 同步；跨端契约测试 server 3 + client 12（heaven_gate signature pin 移到 server 侧后删了 client 对应用例），含**运行时消费 pin** 从生产映射取 recipe id + 经真实 registry 查找） |
| P5 | 回归收口：双视角验收 + 听觉差异化回归 | ⬜ |

## P0 — FPV 技术路线 POC（决策门）

用 `sword.cleave` 单招做三条路线的可行性对拍，产出对比录屏/截图 + 拍板记录（§8 #1 收口）：

- **路线 A**：`FirstPersonMode.ENABLED` + `FirstPersonConfiguration`（showRightArm/showLeftArm/showItem 逐动画配置）——PlayerAnimator 原生 FPV 支持，重点验证：持物时 vanilla `HeldItemRenderer` 与动画手臂的叠加/遮挡关系、视角跟随（body 位移是否晃动相机）。
- **路线 B**：保持 `THIRD_PERSON_MODEL` + 自绘第一人称手臂渲染层（完全自控但工作量最大）。
- **路线 C**：mixin `HeldItemRenderer` 把动画骨骼变换注入 vanilla 第一人称手臂（改动最小但表达力受 vanilla 手臂模型限制）。

同步交付：`client/tools/render_animation.py` 增加 FPV 视角渲染模式（相机置于头部骨骼、只渲染手臂+持物），headless 迭代 FPV 姿态不进游戏。

## P1 — FPV 基础设施

- `BongAnimationRegistry` 查找链（inline→JSON→Java）扩展 FPV 命名空间：本地玩家播放 `<anim_id>` 时优先查 `<anim_id>_fpv.json`，缺省回落到路线 A/C 的自动配置（第三人称动画 + FPV 手臂可见性配置）。
- `BongAnimationPlayer.java:142` 的固定 `THIRD_PERSON_MODEL` 改为 per-animation 配置驱动（数据形状随 P0 拍板：JSON 顶层扩展字段 or Java 侧注册表）。
- 远端玩家路径零变化（FPV 只影响本地玩家渲染分支）；`AnimationLayerManager` 层优先级/打断语义对 FPV 变体与 TPV 保持一致。
- 测试：FPV 查找链单测（有 `_fpv` 取变体 / 无则回落 / 远端玩家永不取 FPV）+ 打断时 FPV/TPV 同步停止。

## P2 — 主力招 FPV 手臂动画批量

- 首批清单（高频 + 大招）：`sword_{cleave,thrust,parry,infuse}`、sword_path 5 招（heaven_gate 两段）、`beng_quan`、zhenmai 5 招、woliu `vacuum_palm`/`vortex_shield`/`turbulence_burst`、`movement_dash`、`shield_raise`。
- FPV 变体书写标准：只做腰以上（相机外肢体不做无效关键帧）；手臂动作幅度按 FPV 放大 1.2-1.5×（贴脸视角下第三人称幅度显平）；`body.*` 位移在 FPV 变体中禁用或减半（防相机晃晕）；每招给出与梯队二同精度的骨骼数值 spec（写各批 PR body）。
- 走视觉资产纪律：`gen_<anim>_fpv.py` 脚本生成 + FPV 渲染工具预览 + 3 轮 `(round N/3)` + 终轮 `<PROMISE>`。

## P3 — 施法瞬间 juice

- 触发：client 本地 `CastStateStore` 已知 cast 起止（`push_skill_cast_started_sync` 下发 duration），release 时刻本地推断（§8 #3 拍板数据源）。
- **门控硬约束**（无论 §8 #3 选哪条数据源）：release juice 必须绑定**已确认 accepted 的 cast identity**——`CastStateStore` 中该次施法处于 accepted 且未被拒绝/打断才触发；server 拒绝（`reject_*` 回执）、打断、取消回执到达时作废对应 pending juice（取消令牌语义）。本地计时器到点但确认缺失/已作废 → 不触发。同一 cast identity 的 release juice 幂等（重复回执/乱序不二次触发）。
  - **落地口径（PR #1249 二轮返工收口）**：「accepted」= `CastStateStore.Origin.SERVER_AUTHORITATIVE` 的 CASTING，即 `CastSyncHandler` → `CastStateStore.replace` 落地的服务端回执。`SkillBarKeyRouter` 按键那一刻写的 CASTING 是**本地乐观预测**，只作候选（它的作用是让 `CastSyncHandler.sourceFor` 把随后的权威回执认成 SKILL_BAR），**不授予任何触发权**。COMPLETE / 动画事件都只是「触发时刻」信号，权限一律来自已被权威武装的 pending / 令牌。
  - 幂等与防复活按 identity 存**有界**终态记录（`CastFovController.Terminal{FIRED, VOIDED}`，LRU 上限 `TERMINAL_MEMORY=16`），先到者胜；teardown 把在飞身份整批记 VOIDED。`IDLE` **不清场**——`CastState.idle()` 是 slot=-1/startedAtMs=0 的无身份单例，无条件清场会让迟到的 IDLE 误杀在飞施法；生产上服务端的 idle 一律带 `Reject*` outcome、被 `CastSyncHandler` 合成成 INTERRUPT 走取消令牌，不以 IDLE 形态到达。

### §P3 参数 amendment（2026-07-26，真机调参，用户拍板保留）

> **决策主体**：plan owner（用户），2026-07-26 二轮 review 返工时拍板。
> **决策**：**保留** 2026-07-25 真机调校后的数值，不回退立项草稿。
> **理由**：草稿的「抖一下」短时长（3-8 tick）实机手感太轻，重型招释放没有存在感；改
> SUSTAIN 持续震动 + 放大 FOV punch。heaven_gate 两段另有结构性原因（见下方驱动契约条）。
> **原始立项草稿数值原样保留在本块内供追溯**——不许直接把原表改掉当没发生过。

**现行定稿**（代码单一真源：`CastJuiceProfiles` + `CastFovController.HEAVEN_GATE_CHARGE_JUICE` /
`HEAVEN_GATE_RELEASE_JUICE`；测试侧字面量镜像 `CastFovControllerTest.EXPECTED_PROFILES` 逐字段 pin）：

| 招式 | shake 幅度/时长 | FOV 脉冲 | 备注 |
|---|---|---|---|
| sword_path.heaven_gate（charge） | 0.8 / 60 tick CRESCENDO | — | **动画事件驱动**（非 CastState）：跟随 `sword_heaven_gate_charge` 动画自身 endTick=60 渐强，动画淡出即止 |
| sword_path.heaven_gate（release） | 1.5 / 24 tick SUSTAIN | +12° 收缩回弹 8 tick | **动画事件驱动**：cast 条 4s 与真实引导窗 7s 错开，只有动画事件能对准劈下那一刻 |
| baomai.full_power_release | 强 / 20 tick SUSTAIN | +9° / 7 tick | 与力竭灰雾同步 |
| woliu.turbulence_burst | 中 / 18 tick SUSTAIN | +6° / 6 tick | |
| zhenmai.sever_chain | 中 / 14 tick SUSTAIN | — | 断链顿挫感 |
| anqi.echo_fractal（release） | 弱 / 12 tick SUSTAIN | — | |
| 其余招式 | 无（默认零） | — | 沉浸式极简：juice 只给重型招 |

强/中/弱 = `CastJuiceProfiles.STRONG/MEDIUM/WEAK` = 1.2 / 0.85 / 0.5。heaven_gate 两段不走三档，
数值直接写在 `CastFovController` 的 `ChargeShake` / `ReleaseBurst` 里（含包络，测试逐字段 pin）。

**原始立项草稿**（2026-07-17，**已被上表取代**，仅供追溯）：

| 招式 | shake 幅度/时长 | FOV 脉冲 | 备注 |
|---|---|---|---|
| sword_path.heaven_gate（release） | 强 / 8 tick | +6° 收缩回弹 4 tick | 蓄力段镜头微震随充能爬升 |
| baomai.full_power_release | 强 / 6 tick | +4° | 与力竭灰雾同步 |
| woliu.turbulence_burst | 中 / 6 tick | +3° | |
| zhenmai.sever_chain | 中 / 4 tick | — | 断链顿挫感 |
| anqi.echo_fractal（release） | 弱 / 3 tick | — | |
| 其余招式 | 无（默认零） | — | 沉浸式极简：juice 只给重型招 |

**同一 amendment 内的驱动契约变更**：草稿写「驱动路径唯一（`CastSyncHandler` cast 状态转换）」，
现改为**两条路径**——heaven_gate 走第二条动画事件路径。理由是结构性的：它的 cast 条
（cast_ticks=80=4s）与真实引导窗（`HeavenGateChanneling` 到 `HEAVEN_GATE_AOE_END=140`=7s 才 emit
release）错开 3s，走 CastState 会在举剑蓄力中途触发而非劈下那一刻。**门控硬约束不打折**：第二条
路径的授权仍是权威 CASTING 武装的 `AnimCastToken`（绑 cast identity，charge/release 各自一次性
消费，reject/INTERRUPT/teardown/TTL 15s 过期/容量淘汰均作废并落 `VOIDED`），动画事件只决定触发
时刻。**令牌是按到达序的有界队列**（`CastFovController.animTokens`，上限 `ANIM_TOKEN_CAPACITY=4`）
而非单枚——单枚会让「新施法顶掉旧令牌」后，前一次施法的迟到 release 动画消费掉后一次的令牌
（后一次真正劈下时静默）；归属判据与残余局限见上方「📌 P3 已登记但不阻塞的边界」首条。

### §P3 交付物 amendment（2026-07-26，用户拍板）

> line「juice 强度全局倍率进 client 配置」的交付形态定为**进程内配置持有者（`JuiceConfig`）+
> keybind 运行时入口（`JuiceControls`）+ 默认 1.0**。**文件级持久化明确不在本 plan 范围**——
> 全仓 `client/` 当前无任何配置文件持久化层（无 `FabricLoader.getConfigDir()` 读写、无 owo config
> screen、无 ModMenu；`HudConfig` 同样是 `static volatile` + 注释自陈「future wiring into the
> external config file」），为这一个倍率单独引入文件持久化会成为全仓**第一个** client 配置持久化
> 先例，属架构决策，用户决定不在本 PR 顺手定。**已知且被接受的行为：每次启动倍率回到 1.0。**
>
> **决策主体**：plan owner（用户），2026-07-26 二轮 review 返工时拍板——这是 owner 修订交付物，
> 不是实施方自我降标。后续若要建 client 配置持久化，应作为独立 plan / PR 统一给 `HudConfig` 等
> 一起接，不为单个 tunable 现造一套。

### §P3 夹带变更 revert（2026-07-26，用户拍板）

> 二轮 review 指出本 PR 夹带了 **P2 阶段**的动画资产行为变更（`sword_heaven_gate_charge` 的
> endTick 60→200 顶点冻结 hold 过 release，连同生成脚本、`AnimCastTicksAlignmentTest` 的定长充能段
> 契约、segment manifest 的 `motion_end_tick`）。用户拍板：**从本 PR revert 出去**（四文件整体回
> `origin/main`），另开小 PR 收。
>
> 连带调整：`CHARGE_ANIM_JUICE` 的 buildDurationTicks 160→**60**。160t 是为「hold 到 release(140t)」
> 配的；动画回到 endTick=60 后震感必须跟着**在播的蓄力动画**收尾，否则画面上蓄力动作已淡出却还在
> 震，违背本条 juice「与画面严格对齐」的前提。
>
> **后续 PR 建议范围**（P2 阶段，独立验收）：天门蓄力动画顶点冻结 hold 过 release 交接点 +
> `AnimCastTicksAlignmentTest` 定长充能段契约改判（endTick ≥ release tick、hold 段逐主轴恒定、
> 密度红线只查运动段）+ segment manifest 加 `motion_end_tick`；落地后 charge juice 时长可随之回到
> 覆盖 release 的量级。

- 复用 `CameraShakeController` 既有 mixin，不新建相机通道；FOV 走独立控制器（新建 `CastFovController`，与 shake 同帧调度）。
- **`CastFovController` 生命周期契约（交付物，不许只交一个孤立类）**：
  - 状态机：`idle → pulse → decay → idle`，所有路径终点必须回到进入施法前的**单一基准 FOV**，复位幂等（重复复位无副作用）；
  - 驱动路径：主路径由 `CastSyncHandler` 的 cast 状态转换回调驱动（started/accepted/release/reject/cancel），client tick 循环负责 decay 推进；bootstrap 注册位置跟随 `CombatJuiceSystem.bootstrap()` 先例（`BongClient.java:127`），cast 时序数据源为 `CastStateStore`（施法预测先例 `CombatHudBootstrap.java:48-49`）。**例外：heaven_gate 走第二条动画事件驱动路径**（`CastFovController.onAnimPlayed`，由 `VfxEventRouter` 在 play_anim **真正播出后**调）；两条路径共享同一 pulse / shake 单通道（last-write-wins），且**共用同一套 accepted identity 门控与终态记录**（见上方参数 amendment 的驱动契约条）；
  - **cast identity = `(slot, startedAtMs)`**，刻意不含 `source`：`source` 不是 wire 字段，`CastSyncHandler.sourceFor` 靠「当前快照是否正 CASTING 在同一 slot」推断，任何非 CASTING 快照落地后即退化成 QUICK_SLOT；写进身份会让「A 被 B 取代 → 迟到 INTERRUPT(A) → COMPLETE(B)」的 B 认不出自己的 pending。`source` 的门控作用保留在 arming 时刻（`resolveSkillId` 只给 SKILL_BAR 解析 profile）；
  - **取消令牌按身份作废**：INTERRUPT 无条件把该 identity 记成 `VOIDED`（防乱序打断被后到的同 identity CASTING 复活），但只清**同 identity** 的 pending / 令牌——否则迟到/重传的旧打断会误杀在飞的新施法；
  - 与其他 FOV 修改源（原版疾跑/药水、shader）的合成规则显式声明（加法偏移量，不直写绝对 FOV）；
  - teardown：断线、切世界、玩家死亡时立即复位基准并清 pending 状态（并把在飞身份整批记 VOIDED，防迟到的旧 CASTING 重新武装）。**切世界**走 `ClientEntityEvents.ENTITY_UNLOAD` 上的本地玩家实体卸载——跨维度/换服时 vanilla 不重建 `ClientPlayNetworkHandler`，`ClientPlayConnectionEvents.DISCONNECT` 不触发，而 Fabric 在 `onPlayerRespawn`/`onGameJoin`/`clearWorld` 三处对旧世界全量 emit ENTITY_UNLOAD（1.20.1 无 `ClientWorldEvents`，这是唯一现成钩子）。
- 可及性：juice 强度全局倍率进 client 配置（0 = 关闭），默认 1.0；**进行中把倍率调 0 立即复位**，不是只影响后续脉冲。
  - **落地**（PR #1249）：`JuiceConfig`（配置持有者，默认 1.0、钳位 NaN/负→0、上限 2.0、档位表 `{0, 0.5, 1.0, 1.5}`）+ `JuiceControls`（keybind `key.bong-client.juice_multiplier_cycle`，默认不绑键随玩家自绑，动作栏回显新档位走 `Text.translatable` + en_us/zh_cn 双份 lang；GUI 打开/无玩家时**排空并丢弃**按键队列，不攒着关界面后补触发；`BongClient` 接线）。写配置**直调** `CastFovController.onJuiceMultiplierChanged`（编译期保证不脱钩，不用可漏注册的 listener）。
  - 「立即复位」= **两个通道都真停**：清 FOV 脉冲对象 + `CameraShakeController.clearIfOwnedBy(Source.CAST)`，不是把 FOV 读数乘 0 遮起来（那样 shake 停不下来、脉冲状态还在、倍率调回来会诈尸）。抖动是与命中 juice 共享的单通道，故取消是**定向**的（只停施法自己造的那条，作用域理由见「📌 已登记但不阻塞的边界」第三条）；死亡/断线/切世界的 `teardown` 仍是无差别 `clear()`。倍率在 `fire`/`onAnimPlayed` 触发时刻烘焙进脉冲峰值与 shake 强度，故恢复倍率只影响**后续** release。
  - **关闭档不放过一次性消费**：倍率 0 时动画事件仍先落 `chargeFired` / 出队 + `FIRED` 终态，再跳过视觉输出——与 CastState 路径 `release()` 的「先消费 pending，再由 `fire` 抑制输出」同序。否则关闭期间到达的 release 事件不落终态，恢复倍率后同一条事件重传即可放出一次早已过去的 juice。
  - **⚠️ 遗留：跨会话文件持久化未交付**——已由上方「§P3 交付物 amendment」正式收窄出本 plan 范围（owner 决策，非遗漏）。
- 测试（饱和覆盖状态机，**从真实入口驱动，不直接调 controller 方法**）：参数表**测试侧字面量**逐招逐字段 pin（含注册集合精确相等、三档常量、动画段两组参数与包络）+ 状态转换全路径——正常 release 完整脉冲后自然回基准、**仅本地预测（缺权威 CASTING）零 juice**、server 拒绝不触发、晚到打断作废 pending juice、回执乱序（打断先于 started 到达 / `INTERRUPT(A)→INTERRUPT(B)→CASTING(A)` 不复活 / 迟到 `IDLE(A)` 不杀 B / 同身份跨 IDLE 重放只触发一次）、同 cast identity 重复 release 幂等、**supersession / TTL 过期 / 容量淘汰后旧身份落 VOIDED 且迟到 CASTING 不得复活**、连续施法（前一脉冲 decay 中开新 cast）、重叠触发合成、倍率 0 进行中立即复位、**倍率 0 期间的动画事件仍被一次性消费（恢复后重放不诈尸，charge/release 各一条）**、死亡/切世界/断线复位 + teardown 后旧身份不可重新武装、动画事件路径的令牌门（无 accepted 零 juice / 重复 charge·release 各一次 / 取消·teardown 后迟到动画不触发 / TTL 两侧边界 / **连续同招施法各拿各的令牌不串用（release 与 charge 两段各一条）** / **release 动画条数不得超过 accepted 施法数** / **队列有界、消费即出队、容量淘汰的身份不得被迟到 CASTING 重新武装**）、**共享 shake 通道所有权**（关施法震感不误清命中抖动 / teardown 无差别清场 / 命中 juice 不受本倍率门控）、真实 `VfxEventRouter` 路由接线——每条路径末尾断言 FOV == 基准值。

### 🚧 P3 未完成欠账（阻塞项）：三招无生产可达的权威 CASTING

> **这是阻塞 P3 完成的欠账，不是「已知限制」。** P3 的主交付是「参数表里的重型招在**真实
> 释放那一刻**拿到 shake/FOV」；参数表 5 个 release 目标中当前只有 2 个在生产里能拿到，故
> 阶段状态保持 ⏳。三轮 review 四个 reviewer 一致以 blocker 提出，判断成立。

**现状（代码实地核查，非推断）**：二轮返工把 accepted 门控收紧到「只认服务端权威 CASTING」
后，实地核查服务端发现 `push_skill_cast_started_sync`（`server/src/network/client_request_handler.rs`）
在实体上没有 `Casting` 组件时直接 early-return，而下列 resolver **全程不插 `Casting`**（属瞬发招，
resolver 内一次结算完，没有引导窗），故服务端**从不**为它们下发权威 `cast_sync{phase:casting}`：

| 招式 | resolver | 证据 |
|---|---|---|
| `baomai.full_power_release` | `combat::baomai_v3::skills::cast_full_power_release` | `grep -rn Casting server/src/combat/baomai_v3/` = 0 命中 |
| `woliu.turbulence_burst` | `combat::woliu_v2::skills::resolve_woliu_v2_skill` | `grep -rn Casting server/src/combat/woliu_v2/` = 0 命中 |
| `anqi.echo_fractal` | `combat::anqi_v2::resolve_anqi_skill` | `grep -n Casting server/src/combat/anqi_v2.rs` = 0 命中 |

只有 `zhenmai.sever_chain`（`combat::zhenmai_v2::insert_casting_snapshot`）与走动画事件路径的
`sword_path.heaven_gate`（`sword_path::skill_register::insert_casting`，经 `apply_cast_costs`）会下发。
即：`CastJuiceProfiles` 的 4 条注册项里 3 条是**生产不可达的死注册项**，`CastFovControllerTest`
里 baomai 那组用例锁的是「客户端能正确消费一条服务端当前不会生产的报文」，锁不住跨端真实契约。

**还要落什么（P3 完成的必要条件）**：为这三招接通生产可达的权威触发链，并补**从真实生产
映射/emit 路径到客户端消费**的跨端契约测试（不是手工构造 casting JSON）。两条候选路线互斥，
**需 owner 拍板，实施方不自选**：

1. **服务端为瞬发招补发生命周期回执**：`cast_sync{phase:casting}` + `{phase:complete}`。改动最小，
   但要先确认「瞬发招要不要 cast 条」这一 gameplay 语义——补发会让这三招在 HUD 上出现施法条。
2. **迁到动画事件驱动路径**（heaven_gate 现走的那条）：三招各自都有服务端 emit 的签名动画，
   `emit_skill_av` 是同样权威的「服务端已执行」信号。代价是手感时序要重新真机调；另需注意
   本路径的令牌归属靠**到达序**（见下「动画事件与 cast identity」条），瞬发招连发时在飞令牌
   会明显多于 heaven_gate，迁移时要一并复核该假设。

**验收怎么算（三条全绿才算 P3 完成）**：
- 五招（含 heaven_gate 两段）逐招有一条**生产链路**测试：从服务端真实 resolver / 映射函数
  产出的信号出发（同 P4 的「运行时消费 pin」做法：recipe id 一律从生产映射取，不在测试里另抄
  一份），到客户端 `CastFovController` 的 juice 产出，中途不手工构造 wire 报文；
- `CastJuiceProfiles.skillIds()` 的注册集合与「生产可达集合」精确相等（有死注册项即撞红）；
- 真机回归：五招各自释放瞬间可感（P5 的 juice 回归条目）。

**本 PR 的处置**：不放宽 accepted 门控去凑「看起来能用」——那是回退掉二轮修掉的 bug。
`CastJuiceProfiles` 条目保留（参数是 plan 定稿，链路补齐后即刻生效）并在类文档诚实标注，
本 PR 交付的是**正确性修复 + 缺口如实登记**，P3 保持 ⏳。补链属**服务端/跨端改动**，不在本
纯 client PR 范围。

### 📌 P3 已登记但**不阻塞**的边界（三轮 review 逐条回应）

以下三条经复核判定为「当前不可达 / 超出本阶段交付面」，已在代码注释与测试里锁住现状语义，
登记在此以免下一轮 review 当成未回应：

- **动画事件与 cast identity**：`VfxEventPayloadV1::PlayAnim` 只有 `target_player` + `anim_id`，
  wire 上**没有** cast id/nonce。本 PR 不造假身份，改为按**到达序**归属——`cast_sync` 与
  `vfx_event` 同走一条 TCP 连接、客户端侧又都经 `BongNetworkHandler` 的 `client.execute` marshal
  到同一线程队列，故「第 N 条 release 动画」对应「第 N 枚在飞令牌」（`CastFovController.animTokens`
  有界队列）。**授权强度不打折**：N 条 release 动画最多消费 N 枚已 accepted 的令牌。残余局限：
  某次施法既不发 release 动画也不发 INTERRUPT/teardown 而静默消失时，队首滞留到 TTL，期间下一
  次的 release 会记到滞留那枚身份上——触发时刻与画面仍对齐、次数仍不超发，只是终态归属错位。
  要彻底闭合需在 `PlayAnim` 上加 cast identity 字段（server + schema + client 三端同步），属跨端
  改动，与上方阻塞项的路线 2 一起拍板更划算。
- **teardown 后首次迟到的旧 CASTING**：当前不可达——服务端在 cast 起手那一刻就发
  `cast_sync{phase:casting}`，而死亡 / 换维度 / 断线的信号必然晚于它产生，同一条有序连接上先发
  先到；死亡路径另有 `CastFovController.tick()` 每 tick 重复 teardown 兜底。**不加**生命周期
  generation 门：`CastStateStore.beginCast` 在旧 CASTING 快照未自然回 IDLE 前是 no-op，加了会换来
  一个**可达**的假阴性（死亡复活后短窗内的真实施法被静默吞掉），净负。论证写在 `teardown()` 注释。
- **倍率的作用域 = 施法 juice**：玩家看到的名字就是「施法震感 / Cast Shake」（`en_us`/`zh_cn`
  两份 lang 原文），「全局」指**一个开关管所有招**，不是「管全部 juice」。命中 / 格挡 / 击杀
  juice（`CombatJuiceSystem`）走自己的 profile 表，不受本倍率门控；对应地倍率调 0 只**定向**取消
  施法自己造的抖动（`CameraShakeController.clearIfOwnedBy(Source.CAST)`），不掐在播的命中抖动。
  要升级成真·全局 juice 开关须同步改命中手感 + UI 命名，属另一份 plan 的交付面，**需 owner 拍板**；
  现状已由 `castMultiplierDoesNotGateHitJuiceWhichKeepsItsOwnStrength` 锁成机器判据，改语义时该
  用例必须被有意识地改掉。

## P4 — 签名音效资产化

- 范围（音源渠道 §8 #2 拍板后执行）：每流派 signature 招 1-2 条，首批 8 条——`sword_path.heaven_gate`（charge 尾程 + release）、`woliu.void_core` 或 `heart`、`zhenmai.sever_chain`、`baomai.full_power_release`、`dugu.infuse_poison`、`tuike.shed`、`morph.yixing`、`anqi.echo_fractal`。音色基调：末法衰败（骨裂声/锈金属/闷雷/砂砾摩擦），禁华丽仙侠音。
- 管线建立（一次性基建，后续扩曲目录复用）：音源 → `.ogg`（mono, 44.1kHz，短样本 ≤3s）→ `client/.../assets/bong/sounds.json`（新建，注册 `bong:skill.<school>.<move>` 事件）→ 对应 `server/assets/audio/recipes/*.json` 把主层 sound id 从 `minecraft:` 换成 `bong:` 事件（recipe 其余混音层保留做空间感铺底）。
- **资产变更硬约束**：同步 `resourcepack.rs` + committed manifest 的 sha1/size（否则 Build resource pack CI 红）。
- **跨端音效契约测试（同一测试联结三级 ID，不许 server/client 各自维护互不校验的清单）**：提取全部 server audio recipe 引用的 `bong:` sound ID 集合，断言 ⊆ client `sounds.json` 事件键集合；`sounds.json` 每个事件引用的 `.ogg` 文件真实存在且已进 committed manifest（双向：注册无文件、文件无注册均判红）；错误分支覆盖重复键、非法命名空间前缀、缺文件。首批 8 条 signature 招逐项 pin：各自 recipe **真实引用**了为它新建的 `bong:` 事件（防资产只注册不消费）。

### P4 落地（2026-07-24）

- **音源**：8 条 CC0 signature `.ogg`，全部源自 [BigSoundBank](https://bigsoundbank.com)（作者 Joseph SARDIN，**CC0 / public-domain-equivalent，无署名义务**）——WebSearch 检索 → WebFetch 核许可 → `curl` 下载 → `ffmpeg`（`silenceremove`+`loudnorm -16 LUFS`+`afade`，输出 mono / 44.1kHz / ≤3s vorbis）。出处清单 `client/src/main/resources/assets/bong/sounds/ATTRIBUTION.md`。
- **事件↔资产**：`client/src/main/resources/assets/bong/sounds.json`（新建，8 事件）→ `assets/bong/sounds/skill/<school>/<move>.ogg`。事件键用 `.`（`skill.zhenmai.sever_chain`），文件名用 `/`（`bong:skill/zhenmai/sever_chain`）。
- **recipe 主层 swap（保留 vanilla 铺底层）——必须 swap「招式实际 emit 的 recipe」**：8 招各自 signature recipe 的 L0 `sound` → `bong:skill.<school>.<move>`、pitch 复位 1.0：`sword_manifest_strike`（heaven_gate release，`network::audio_trigger::sword_path_recipe_for_skill`）、`woliu_void_core`、`zhenmai_sever_crack`、`baomai_signature`、`dugu_poison_signature`、`shed_skin_burst`（tuike shed，`combat::tuike_v2::events`）、`anqi_echo_fractal`、`yixing_cast`（除 heaven_gate 的 sword_manifest_strike 也在 server registry 外，全部 server `assets/audio/recipes/*.json`）。`schema::audio::validate_identifier` 接受任意 `namespace:path`，`bong:` 与 `minecraft:` 层共存。
  - **⚠️ 修正（2026-07-24～25，fix/p4-signature-recipe-runtime-consumption）**：P4 首版误把签名换进**同名但招式不消费的死 recipe**——heaven_gate release 换进 `heaven_gate_release.json`（招式实际 emit `sword_manifest_strike`）、heaven_gate **charge 换进 `heaven_gate_charge_2s.json`**（招式实际 emit 走 `sword_path_recipe_for_skill(HeavenGateCharge)`）、tuike 换进 `tuike_signature.json`（招式实际 emit `shed_skin_burst`），静态契约测试假绿、实机零签名音（真机试听发现）。修复：① release/tuike swap 真实 recipe、死 client recipe 回退 vanilla；② **charge 建专属 server recipe `heaven_gate_charge`**（不复用共享的 `sword_infuse`——那被 `sword_basics` 基础剑招也消费，塞签名会泄漏），`sword_path_recipe_for_skill(HeavenGateCharge)` 改指向它，前兆 L0 = release 的 `bong:skill.sword_path.heaven_gate` 签名 ogg @ pitch 0.72/vol 0.4 + amethyst 铺底。**根因**：契约测试只验「recipe 文件引用 bong:」，没验「招式真消费该 recipe」——reviewer 的运行时消费告警成真。修法见下「运行时消费 pin」。
- **resourcepack**：`scripts/build-resourcepack.sh` INCLUDE_PREFIXES + audio 子包均纳入 `bong/sounds` + `bong/sounds.json`（`count_files` 支持文件前缀计数）；`scripts/test_build_resourcepack.py` 加 fixture + audio 子包路径/计数断言；`server/src/network/resourcepack.rs` sha1/size 同步为新构建值（sha1 `3d0866dd…`，size 72_501_865；`publish-release` job 在 merge 到 main 时自动重建 + 上传 release 资产）。
- **跨端契约测试**：server `audio::signature_recipes_reference_registered_bong_events`（recipe `bong:` 引用 ⊆ client sounds.json 键）+ **`each_signature_skill_actually_emitted_recipe_swaps_l0_to_its_bong_event`**（**运行时消费 pin**：9 招逐项锁「招式实际 emit 的 recipe」L0=bong: + pitch（release/常规招 1.0、天门蓄力前兆 0.72）——**recipe id 一律从生产映射取**：调真实映射函数 `sword_path_recipe_for_skill`/`baomai_recipe_for_skill`/`ZhenmaiSkillId::audio_recipe` 或引用生产 `pub(crate) const` 单一真源 `WOLIU_VOID_CORE_RECIPE`/`SHED_SKIN_BURST_RECIPE`/`DUGU_POISON_SIGNATURE_RECIPE`/`YIXING_CAST_RECIPE`/`ANQI_ECHO_FRACTAL_RECIPE`，测试内不另抄 recipe id 表；经真实 `SoundRecipeRegistry` 加载+查找、锁 L0 sound/pitch/volume≥可听下限/delay 有界窗口；招式改播别的 recipe 或目标 recipe 退回 vanilla 都撞红——防「换错死 recipe」bug 回归）+ **emit-path 集成测试**（跑真实 emit 系统、断言实发的 `PlaySoundRecipeRequest.recipe_id`，防「删掉发声调用」emit 断链）：`network::audio_trigger` 的 `sword_path_skills_emit_dedicated_recipes`（heaven_gate charge/release）/ `anqi_skills_emit_dedicated_recipes`（echo_fractal）/ `baomai_full_power_release_emits_signature_recipe` / `woliu_void_core_emits_signature_recipe` / `tuike_shed_passive_emits_signature_recipe` / `zhenmai_skills_emit_their_mapped_recipes` / `dugu_reverse_emits_signature_recipe` + `body_plan::morph` yixing emit 断言（zhenmai/dugu/tuike-主动 三处内联 emit 已于 P5 重构为 Pattern A 并补端到端 emit-path 门，见下「P5 emit 架构统一」）；client `SignatureAudioContractTest`（从单一 canonical `SIGNATURE_SPEC` 表派生：事件→sound name→ogg 路径逐项映射 + sounds.json↔ogg 双向 + **错误分支**：重复事件键 token 级检测 + 非法命名空间前缀拒绝 + **committed manifest 覆盖** + **音频格式 ffmpeg 完整解码门禁**：统一 `validateSignatureAudio` 谓词（`ffmpeg -f null` 完整解码每 packet + `ffprobe duration_ts` 精确采样数严格 ≤132300=3s@44.1k，无浮点容差），正向真资产与负向 fixture（ffmpeg lavfi 现场合成的 stereo/48k/超长/Opus + garbage）**共用同一谓词**防漂移；工具/编码器缺失时 Assumptions 跳过而非破红 CI。校验只作用于 signature 集合。sounds.json 移除未接线的 `subtitle` key + Minecraft 不消费的 `category` field（字幕/分类：字幕留 P5 补 lang；播放分类由 recipe 的 `category`/`SoundSource` 在播放入口设置）。
- **听审记录（听觉资产 3 轮 + `<PROMISE>`，plan §10 硬约束）**：8 条经用户逐条真机试听 + 多轮换源定稿——Round 1 首版 8 条 → Round 2 用户听审反馈换源（涡心：无人机声 → 呼啸风 #0155；回响分形：尖叫鸡 → 藏铜钵 #1110；脱壳：平淡撕→激烈撕毁 #0013 长撕段）+ 修静音 bug（`atrim` 未重置 PTS 致淡出吞声）→ Round 3 用户复听全部拍板「ok了」。终审 `<PROMISE>` 见本轮音效 commit message。
- **已交付（charge 尾程）**：heaven_gate charge 尾程签名 = 复用 release 签名 ogg 的压调层（pitch 0.72/vol 0.4），接进 charge 招式实际消费的专属 server recipe `heaven_gate_charge`（`sword_path_recipe_for_skill(HeavenGateCharge)` 指向它）。天门是 committed 单向门（蓄力 elapsed 0→60 临界→120 冲击波→140 释放，cast 即结算不可中断），recipe 于蓄力起始 emit、L0 `delay_ticks=100` 把压调签名推到蓄力**尾段**（临界后、释放前 ~2s）才响——兑现「charge 尾程」预示（非蓄力起始就播）。运行时消费 pin 锁 `delay_ticks>=60` + `volume>0` + L0=bong: 事件 + pitch 0.72。未单独取音（复用 release ogg 压调）。
- **遗留**：P5 盲听时若 charge 前兆感不足，再单独取一条 charge 专属音（当前压调复用已可辨）；其余 7 招（heaven_gate 之外的 woliu/zhenmai/baomai/dugu/tuike/morph/anqi）P5 走差异化盲听回归（能从流派分辨）。
- **P5 emit 架构统一（zhenmai/dugu/tuike-active 签名 emit-path 补测）✅ 2026-07-25**：三处内联 emit（Pattern B）已重构为 Pattern A 独立系统 + 补齐 emit-path 集成测试，**9 招签名 emit-firing 全覆盖**：
  - **zhenmai**：`emit_skill_feedback` 不再内联发 `PlaySoundRecipeRequest`，改发新的纯 cosmetic 事件 `combat::zhenmai_v2::ZhenmaiSkillCastEvent{caster, skill, center}`；新系统 `network::audio_trigger::emit_zhenmai_v2_audio_triggers` 读它、经生产映射 `ZhenmaiSkillId::audio_recipe` 发声（注册在 `network::mod` 的 audio 调度块，`.after(tick_audio_dedup_clock).before(emit_audio_play_payloads)`）。
  - **dugu**：倒蚀签名从 `skills::apply_reverse` 内联 emit 移出；`emit_dugu_needle_audio_triggers` 更名 `emit_dugu_v2_audio_triggers` 并加读 `ReverseTriggeredEvent` → `DUGU_POISON_SIGNATURE_RECIPE`（音源仍锚爆发中心 `event.center`）。
  - **tuike 主动**：`cast_shed` 内联 emit 直接删除——主动/被动都走 `shed_outer_layer` 发的 `FalseSkinSheddedEvent`，由既有 `emit_tuike_v2_audio_triggers` 统一发 `SHED_SKIN_BURST_RECIPE`（变异验证：加回内联即出两条 `shed_skin_burst`）。删掉的那条与幸存那条**recipe 相同但路由不同**：内联是 `pos: None` 听者锚点 + 64 格广播 + 不过 dedup，Pattern A 是世界锚点 + 距离衰减（L0 volume 0.9 ⇒ 实际可听约 16 格）。**代价已知并接受**：删掉的那条是听者锚点、对 64 格内所有人恒近满音量（≈0.79），删后只剩世界锚点那条 `0.9 × (1 − d/16)` ⇒ 16~64 格外由有声变无声，**近场也随距离变轻**（4 格 ≈0.68、8 格 ≈0.45、12 格 ≈0.23），只有贴身几乎不变。这是有意接受：蜕壳是发生在施法者身上的爆发，空间化才是正确表现（被动掉壳一直如此），非空间化的远距离满音量才是异常。与 dugu 倒蚀保留原路由的差别在于那边丢的是**收包半径**（64→8 格，连近场之外都收不到包），不是衰减曲线。
  - **接线门禁**（PR #1262 review 两轮补强）：audio-trigger 调度提取为**唯一生产注册入口** `network::audio_trigger::register`；`network::register` 进一步拆成 `bootstrap_redis_bridge`（唯一有外部副作用的一段：起 Redis 线程）+ **纯 App 装配** `register_app_wiring`（只 `insert_resource`/`add_systems`/`add_event`），于是门禁测试能跑**真正的顶层生产装配路径**而不是自欺欺人地只调子函数。三道门：
    - `production_wiring_registers_audio_trigger_systems_exactly_once_in_order`——调生产 `network::register_app_wiring`，对 19 个 emit 系统 + dedup 时钟断言**恰好注册一次**（重复注册会让逻辑时钟一帧推进多次、悄悄改短 dedup 窗口），并从 `ScheduleGraph` 依赖图断言每个 emit 系统的 `.after(tick_audio_dedup_clock)` 与 `.before(emit_audio_play_payloads)` 两条边都在（顺序边连的是函数的 `SystemTypeSet` 节点）。**三重变异验证**：删掉 `network/mod.rs` 顶层那行 `audio_trigger::register(app)` → 撞红；删掉 `.after` 约束 → 撞红；重复注册 dedup 时钟 → 撞红。
    - `production_module_registers_install_signature_cast_events`——三条链的事件由各自模块生产 `register` 装好（缺 `add_event` 则 `send_event` 静默丢弃 → 实机零签名音），已变异验证。
    - 路由锁：`zhenmai_audio_uses_cast_time_center_not_live_position` 除 cast-time 音源外，另断言 recipient = 以 cast-time center 为圆心的 `world_3d` 64 格广播（测试 app 装**真实** `SoundRecipeRegistry`，否则 recipient 会退化成 `Single` 让断言空转）——把「32→64」这条已披露的路由变化钉住，改成听者锚点或 MELEE 8 格都撞红（已变异验证）；dugu 侧对应的路由锁见 `dugu_reverse_emits_signature_recipe`。
    - dedup 状态转换（本 PR 新引入、plan 明确接受，故必须有门）：`shed_signature_dedup_collides_within_window_and_recovers_after`（同 owner 主动+被动同帧只响一次 / 窗口内仍抑制 / 跨过 2 tick 边界恢复发声 / 不同 owner 互不抑制）与 `zhenmai_shared_recipe_skills_dedup_within_window`（共用 `zhenmai_shield_hum` 的 multipoint+harden 同帧只响一次，parry 另发一条）。
  - **emit-path 测试**：`network::audio_trigger` 的 `zhenmai_skills_emit_their_mapped_recipes`（五招逐招；注：multipoint 与 harden 在既有映射里共用 `zhenmai_shield_hum`，本测试锁的是「每招发它映射到的那条」，不宣称五条两两不同）/ `zhenmai_audio_uses_cast_time_center_not_live_position` / `dugu_reverse_emits_signature_recipe` / `dugu_three_readers_do_not_cross_talk`；**端到端**（真跑一次施法 + 真跑音效系统）`combat::zhenmai_v2::tests::sever_chain_cast_emits_signature_recipe_end_to_end`、`combat::dugu_v2::tests::reverse_cast_emits_signature_recipe_end_to_end`、`combat::tuike_v2::tests::active_cast_shed_emits_signature_recipe_exactly_once_end_to_end`；**拒绝路径无声门**（起手失败不许响招式音）`rejected_cast_emits_no_audio_and_no_cast_event` / `rejected_reverse_emits_no_audio_and_no_domain_event` / `rejected_cast_shed_emits_no_audio_and_no_shed_event`。
  - **音源 / 路由契约（逐字段对齐重构前，PR #1262 review 两轮抓出）**：
    - zhenmai 音源**无条件取事件 `cast_center`（cast-time 位置，字段名即契约）**，刻意不查消费时实时 `Position`——重构前内联 emit 锁的就是施法当时位置，读实时位置会随跨帧消费 / 玩家移动漂移且依赖未声明的系统顺序。
    - dugu 倒蚀签名走 `emit_play_listener_anchored_broadcast`（`pos: None` 听者位置发声 + 以爆发中心为圆心 64 格广播），**可听字段**（pos / recipient / volume / pitch）与重构前 `dugu_v2::skills::emit_audio` 一致（差异见上 ③④，均不改可听结果）；**不能**走通用 `emit_play`——`dugu_poison_signature` 声明的是 `MELEE`，收包半径会从 64 格塌到 **8 格**（比该招自己 10 格的 `ReverseAftermathCloud` 还小，站在毒雾里都可能收不到包），再叠上世界锚点的 LINEAR 衰减（L0 volume 0.24 ⇒ 8 格处已衰到约一半），远距离直接静默。近场增益两条路线量级相当（≈0.21~0.24），**问题出在收包半径塌缩、不是「0.24 一定听不见」**。测试断言 `pos == None` 且 recipient 为 64 格广播，已变异验证（换回 `emit_play` 即撞红）。要不要把倒蚀改成空间化签名（须同步调 recipe attenuation/volume）留 P5 盲听回归再定。
    - 已知且可接受的差异（穷举，PR #1262 第四轮审计补全）：① zhenmai 收包半径由内联硬编码 32 格改为 recipe 声明的 `world_3d` 64 格——只放宽「谁收到包」，pos 与 LINEAR 衰减不变，实际可听距离由 volume 决定（vanilla LINEAR 在 volume ≤ 1 时约 16 格截止），故 32~64 格的新增收包者本就听不到；② 音源取整由 `as i32` 截断改为 `floor()`（负坐标差 1 格，新写法才是正确 block 坐标语义）；③ **三处站点首次套上 `AudioImplementationDedup`**（`emit_play` 的 `should_emit`，key = (entity, recipe)，窗口 2 tick ≈ 0.1s），内联那三条都不过 dedup。可达影响仅两处，均接受：zhenmai 的 multipoint 与 harden 共用 `zhenmai_shield_hum`（既有映射），同一施法者 0.1s 内先后放这两招则第二声被吞（两招音色本就相同，且 0.1s 双击同音会打颤）；tuike 主动蜕壳与维护/被动掉壳共用 `shed_skin_burst`，0.1s 内相撞同样只响一次。**sever_chain 与倒蚀两条签名自撞不可达**——`zhenmai_sever_crack` / `dugu_poison_signature` 各自 recipe 唯一，冷却分别 `SEVER_CHAIN_COOLDOWN_TICKS` = 20 min（24000 t）与 `skill_spec(Reverse).cooldown_ticks` = 60 s（1200 t），远大于 2 tick 窗口；**蜕壳签名是例外**：`shed_skin_burst` 被主动施法（`ACTIVE_SHED_COOLDOWN_TICKS` = 8 s）与维护/被动掉壳共用，两者 0.1 s 内相撞时只响一次（主动那次可能被吞）——旧内联无 dedup 必响，这一条属**接受的行为变化**，不在「不受影响」名单里。④ dugu 站点 `flag` 由 `None` 变 `Some("dugu_reverse")`（调试标记；client `SoundRecipePlayer` 只在带 `loop` 的 recipe 上消费 flag，`dugu_poison_signature` 无 loop ⇒ no-op）；⑤ tuike 幸存那条在 `permanent_taint_load > 0` 时带 `pitch_shift = +0.08`（client 按 `pitch × 2^shift` 算 ⇒ 约 +5.7%、近一个半音），删掉的内联恒 0.0——蜕带永久污的壳正是主动蜕壳的主用法，故这一支音高确有变化（且 `flag` 由 `None` 变 `Some("tuike_shed")`，同为 no-op）；⑥ zhenmai 新路径多一条旧路径没有的退化分支：`SoundRecipeRegistry` 缺失或 recipe 未注册时 recipient 塌成 `Single(caster)`（旧内联恒 32 格广播）——生产必载 registry 且有运行时消费 pin 兜底，不可达，此处只作穷举记录。
  - **零战斗/qi 语义改动**：三处只挪发声位置，伤害 / 创口 / `QiTransfer` / 冷却 / 经脉门全未触碰。
  - **后续（不在本条范围）**：tuike `cast_don` / `cast_transfer_taint` 的非签名内联 `emit_audio` 与 Pattern A 同样重复发声（`don_skin_low_thud` / `contam_transfer_hum` 各两条）、dugu `cast_shroud` 等基础招内联 emit 亦未走 Pattern A——同类清理留后续。

## P5 — 回归收口

- FPV/TPV 双视角实机验收：首批每招录屏（第一人称能看到手臂动作全程、持物不遮挡关键帧）；
- 听觉差异化回归：8 条签名音效盲听可分辨流派；
- juice 回归：重型招释放有反馈、普通招零 juice、全局倍率 0 时完全关闭；
- e2e + bot 场景：施法 payload 链路回归（`scripts/bot/scenarios/` 补 cast 触发场景断言 vfx/audio payload 发出）。

## §8 开放问题（P0 决策门前需收口）

1. **FPV 技术路线三选一**：P0 POC 用实测对比拍板（持物遮挡是决定性判据）；预判推荐路线 A（库原生，改动集中），但 playerAnim 1.20.1 分支的 FPV 成熟度必须实测。
2. **签名音效音源渠道**：AI 音效生成（类比 `/gen-image` 建 `/gen-audio` 管线）/ CC0 素材库（freesound 等，须核许可证）/ 用户自供——**需用户拍板**，涉及外部资源与授权，agent 不自决。
3. **施法 juice 触发数据源**：纯 client（`CastStateStore` 本地推断 release，零 wire 变更，但 server 拒绝/打断的边缘一致性靠 cast_sync 已有回执）vs server 显式 juice hint（精确但加 payload 字段）。推荐纯 client 起步，误差可接受再不加字段。**两条路线都必须满足 P3 门控硬约束**（accepted 确认 + 取消令牌 + 幂等），纯 client 路线的收口前提是核实 cast_sync 回执在拒绝/打断路径的到达保证。
4. **FPV 变体的维护成本边界**：每招两份动画（TPV+FPV）长期同步维护——是否约定「FPV 变体只在 TPV 定稿后产出、TPV 改动必须连带复核 FPV」写入 `docs/player-animation-conventions.md`。

> **收口状态（2026-07-22）**：全部 4 项已在 §8.1 收口——#1 FPV 技术路线用户真机拍板**路线 A**（库原生 `THIRD_PERSON_MODEL`+config，非 plan 预判的不存在的 `ENABLED`）、#2 音源渠道 CC0 素材库、#3 juice 数据源纯 client、#4 FPV 维护约定入档 §16。原表保留以备追溯，**实施时以 §8.1 决议为准**。

## §8.1 决议（pre-P0 收口，2026-07-22）

> 本节按 `docs/CLAUDE.md §5.1` 追加。#3/#4 由两个并行 Explore agent 核查 origin/main（`9a2cff02c`）代码现状产出（不拍脑袋）；#1/#2 为用户决策门，**已于 2026-07-22 用户拍板收口**（#1 路线 A、#2 CC0 素材库，决议见下文对应小节）。

### #3 施法 juice 触发数据源 —— 采纳纯 client，零 wire 变更

**决议**：

1. **采纳纯 client 路线**，juice 触发数据源 = `CastStateStore` 单快照（`phase==CASTING` 即已 accepted；`COMPLETE`/`INTERRUPT` 为终态）；**拒绝 server 显式 juice hint 路线**——不加 payload 字段，wire 层零变更的设计目标守住。
2. **门控硬约束（accepted 确认 + 取消令牌作废 pending + 幂等）在纯 client 下已满足**，逐路径核验：
   - **施放前拒绝**（真元不足 / 冷却 / 未习得 / 经脉门 / race gate 等）：部分分支 server 静默 `return`（`client_request_handler.rs:13320/13431/13437/12892/12898/12905`），部分发 Idle+reject outcome（`:13358/13404/13465`、`push_skill_cast_rejected_sync:13944`）。**但施放前 client 从未收到 Casting/accepted sync → 无 pending juice 可作废 → 对门控无害**（juice 只绑已 accepted 的 cast，reject 静默不构成缺口）。
   - **施法中打断**（控制 / 受击 / 移动）：三分支全发 `cast_sync` Interrupt（`cast_emit.rs:215/258/293`，对应 Casting 移除 `:207/250/285`）→ client `transitionToInterrupt` → **到达保证 ✓**，作废 pending。
   - **切槽主动取消**：发 UserCancel（`client_request_handler.rs:14034`，对应 Casting 移除 `:14014`）→ **到达保证 ✓**。
   - **施法中非受击死亡**（修炼死 / 真元枯竭 / 坠落溺水，无同 tick wound）：server 在 `combat/lifecycle.rs:1857` 玩家状态重置路径静默 `remove::<Casting>`，而 `CastOutcomeV1::Death` 定义了却全仓从不 emit（`schema/combat_hud.rs:69` 定义、仅 `schema/proto_convert.rs:2239` 映射引用）——**这是唯一 server 静默缺口**。**由 P3 已明令的 client 侧 death teardown 关闭**（P3 契约原文：「teardown：断线、切世界、玩家死亡时立即复位基准并清 pending 状态」）：client 观测本地玩家死亡即作废 pending juice + 复位 FOV，不依赖 server 回执。**故纯 client 路线的收口前提（拒绝/打断路径到达保证）成立，死亡缺口不构成破功点**。
3. **幂等 + 取消令牌语义的实现依据**：`CastStateStore` 是单个 volatile 快照、`replace(next)` 整体替换（`CastStateStore.java:18/60`），无唯一 cast id，身份 = `(source, slot, startedAtMs)` 三元组。P3 的 pending juice **必须绑定当前快照身份并每 tick 从 store 重解算**：快照身份变化（新 `startedAtMs`）即视前一 pending 作废（supersession），同一身份重复 release 只认第一次（幂等）——快速同槽连发不会误触前一次 pending。

**落点**：`client/src/main/java/com/bong/client/network/CastSyncHandler.java:37-53/105-127`（phase/outcome 分派）、`client/src/main/java/com/bong/client/combat/CastStateStore.java:18/60/76-91`（单快照 replace + tick 300ms 自回 idle）、`server/src/network/cast_emit.rs:215/258/293/469`（interrupt/complete emit）、`server/src/network/client_request_handler.rs:14034`（UserCancel）、`server/src/combat/lifecycle.rs:1857`（死亡静默移除 Casting，缺口）；plan §P3（`CastFovController` 生命周期契约 + death teardown）。

**可选后续（非本 plan 范围）**：server 在 `combat/lifecycle.rs:1857` 施法中死亡路径补发 `cast_sync`(Interrupt, `CastOutcomeV1::Death`) 可让死亡缺口获得权威回执、并退掉 `CastOutcomeV1::Death` 的死代码状态——但纯 client death teardown 已完全满足 juice 门控，此项为健壮性增益，不阻塞本 plan，宜另立 skeleton 处理。

### #4 FPV 变体维护成本边界 —— 约定入档 player-animation-conventions.md §16

**决议**：

1. **采纳约定并写入文档**：`docs/player-animation-conventions.md` 新增 **§16**（授权追加节，格式对齐既有 §13/§14/§15 的引言块 + `---` 分隔模式），明文规定「FPV 变体只在对应 TPV 动画定稿后产出；TPV 改动必须连带复核 / 同步 FPV 变体」。
2. **不与既有 §3 重叠**：§3「FPV 可见性要求」管的是**单份动画**在第一人称视野内的骨骼可见性（guard 位置等）；§16 管的是 **TPV↔FPV 双份资产的同步维护约定**，是新维度，独立成节。
3. **机器把关留到 FPV 资产落地时**：pre-P0 全仓无任何 `*_fpv.json`（`player_animation/` 140 份动画均单份），暂无可锁对象；§16 先立文字约定，并指明将来 P1/P2 FPV 变体落地后参照 §14.1「机械锁登记表」模式（`AnimCastTicksAlignmentTest` 的 `FIXED_PHASE_CHARGE_SKILLS` 先例）加一条 TPV↔FPV 对拍锁，把「连带复核」从文字升级为机器判据。

**落点**：`docs/player-animation-conventions.md`（新增 §16，line 681 后追加）；`client/src/main/java/com/bong/client/animation/BongAnimationRegistry.java:120-129/170`（FPV 查找链落点，P1）；plan §P1/§P2。

### #1 FPV 技术路线 —— 用户真机拍板路线 A（2026-07-22）

**决议**：

1. **锁定路线 A（库原生）**：`FirstPersonMode.THIRD_PERSON_MODEL` + 一个 `showRightArm/showLeftArm/showRightItem/showLeftItem=true` 的 `FirstPersonConfiguration`。**拒绝 B（自绘 FPV 手臂层）与 C（mixin 注入 vanilla 手臂）**——二者是 A 遮挡出问题时的后备，A 遮挡实测干净故无需做。
2. **plan 预判的 `FirstPersonMode.ENABLED` 在 player-anim `1.0.2-rc1` 不存在**（该版本枚举仅 `NONE/VANILLA/THIRD_PERSON_MODEL/DISABLED`）——「库原生显示动画手臂」正解是 `THIRD_PERSON_MODEL` + config 开手臂。出厂 `BongAnimationPlayer.playOnStack` 只设 mode 没设 config，默认 config 的 `showArm=false` 正是第一人称无手臂的根因。库 `ItemInHandRendererMixin` 在 `THIRD_PERSON_MODEL` 下整段 cancel vanilla FP 手/物渲染 → plan 担心的「vanilla 盖掉持物」在 A 下**不发生**，真机印证。
3. **真机 POC 证据（决定性判据=持物遮挡）**：POC harness（`FpvPocState`/`FpvPocControls` 运行时切 OFF/A/B/C + 键位 + `scripts/fpv-poc.sh` 快捷命令）在 `sword.cleave` 上真机对比——用户判定**A 下第一人称出现动画手臂 + 剑、遮挡正确**（无剑穿手/无 vanilla 双重渲染/无 z-fighting）。唯一观察：双手持剑时两手在贴脸视角**未视觉合拢**——经确认这是**姿态问题非路线问题**（剑=单手主手物只在右手渲染，左手空手摆双手持姿态，贴脸视差下需专门 FPV 变体把左手并到剑柄），归 **P2** 的 `sword_cleave_fpv.json` 处理，A/B/C 任选此姿态都要调。
4. **P1 数据形状（据本决议定形，已落地）**：本地玩家 **`<anim_id>_fpv` 查找链** + 路线 A 的 `FirstPersonConfiguration` 落于 `BongAnimationPlayer.playOnStack`——本地玩家播 `bong:sword_cleave` 时优先取 `bong:sword_cleave_fpv`，命中则播变体 + `applyFirstPersonRendering(true)`（`THIRD_PERSON_MODEL` + 开双臂/持物）。**回落语义（对 plan 原文的实现细化）**：无 `_fpv` 变体或远端玩家 → 出厂行为（`applyFirstPersonRendering(false)`，第一人称隐藏手臂）——即 **FPV 手臂按 `_fpv` 变体存在与否 opt-in**，而非 plan 原文的「缺省 blanket 开手臂」；理由是未调姿态的 TPV 动作直接进第一人称贴脸会显糙（用户实测），逐招授权变体更稳。远端玩家渲染分支零变化。POC harness（`FpvPocState`/`FpvPocControls`/键位/`fpv-poc.sh`）已在本 PR 收敛移除。

**落点**：`client/src/main/java/com/bong/client/animation/BongAnimationPlayer.java`（`playOnStack` 内 `_fpv` 查找 + `isLocalPlayer`/`fpvVariantId`/`applyFirstPersonRendering` 三 helper）、`client/src/test/java/com/bong/client/animation/BongAnimationPlayerFpvTest.java`（变体 id + config pin）、`client/src/main/java/com/bong/client/animation/BongAnimationRegistry.java:122-135`（`contains`/`get` 查找源）；`client/tools/gen_sword_cleave_fpv.py` + `assets/bong/player_animation/sword_cleave_fpv.json`（P2 双手合拢变体）；render 工具 `client/tools/render_animation.py --fpv`（P2 迭代用）。

### #2 签名音效音源渠道 —— 用户拍板 CC0 素材库（2026-07-22）

**决议**：

1. **采纳 CC0 素材库**（freesound 等）作首批 8 条 signature 音效音源；拒绝 AI 生成与用户自供。
2. **许可证硬约束**：每条采用的 `.ogg` 必须逐一核验为 **CC0 / 等价公有领域 / 无署名要求**许可，并在 P4 PR 内留出处清单（源 URL + 许可证 + 检索日期），落一份 `client/src/main/resources/assets/bong/sounds/ATTRIBUTION.md`（或等价出处文件）。**非 CC0（含要求署名的 CC-BY）不采用**，避免署名义务渗入资源包。
3. **音色基调**（plan §P4 已定，此处重申）：末法衰败——骨裂 / 锈金属 / 闷雷 / 砂砾摩擦，禁华丽仙侠音（`worldview.md §四`）。
4. **管线复用**（plan §P4 原样）：CC0 源 → 切样本（mono / 44.1kHz / ≤3s）→ `.ogg` → client `sounds.json` 注册 `bong:skill.<school>.<move>` → recipe 主层换 `bong:` 事件；同步 `resourcepack.rs` + committed manifest 的 sha1/size。

**落点**：plan §P4；`client/src/main/resources/assets/bong/sounds.json`（P4 新建）+ `server/assets/audio/recipes/*.json`（P4 换音源层）+ 出处文件 `assets/bong/sounds/ATTRIBUTION.md`。仅卡 P4/PR-6，不阻塞 P0–P3、P5。

## 测试声明

- client：FPV 查找链/回落/远端隔离单测、juice 状态机饱和单测（从真实 CastSyncHandler 入口驱动：正常/拒绝/晚到打断/乱序/重复 release 幂等/连续/重叠/倍率 0 立即复位/死亡切世界断线复位，全路径终点断言基准 FOV）、sounds.json↔ogg 双向对应扫描（gradlew test）；
- server：recipe 引用 `bong:` 事件解析测试（cargo test，指向不存在事件判红）；
- 跨端：recipe ID 集合 ⊆ sounds.json 键集合 + signature 招 recipe 真实引用自有事件的逐项 pin；
- e2e：`bash scripts/smoke-test-e2e.sh` + bot cast 场景绿；资源包 CI（sha1 同步）绿。

## §10 实施工作流

- 单 plan 多 PR 序列化：PR-1 = P0 POC + 工具（含拍板记录回写 §8.1）；PR-2 = P1 基础设施；PR-3/4 = P2 分批 FPV 动画；PR-5 = P3 juice；PR-6 = P4 音效管线 + 首批；PR-7 = P5 收口。
- P2 批次依赖梯队二（`plan-skill-anim-fidelity-v1`）对应批次先 merge（FPV 以重制稿为底）；P3/P4 与 P2 无依赖可并行排期。
- 每 PR 独立实施 subagent；视觉/听觉资产 PR 走 3 轮 + `<PROMISE>`；CodeRabbit / `/review` 等待走 ScheduleWakeup 1200s 协议。
- **单次 consume-plan 全自动到 merge**：用户提交 `/consume-plan` 后全自动走完实施→review→merge→归档至 `docs/finished_plans/`，无需人工值守（例外：§8 #2 音源渠道属用户拍板项，须在 pre-P0 收口时定案，不阻塞其余 PR）。
