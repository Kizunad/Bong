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
| P2 | 主力招 FPV 手臂动画批量产出（3 轮打磨 + PROMISE） | ⏳ `sword_cleave_fpv` 双手合拢 round 1（gen 脚本 + 变体 json），待真机复测迭代 |
| P3 | 施法瞬间 juice：重型招释放 shake/FOV 脉冲（按招参数化） | ⬜ |
| P4 | 签名音效资产化：每流派 1-2 条真 `.ogg` + 管线建立 | ⬜ |
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
- 参数表（新建 `CastJuiceProfile`，按招注册，表格化写进实施 PR）：

| 招式 | shake 幅度/时长 | FOV 脉冲 | 备注 |
|---|---|---|---|
| sword_path.heaven_gate（release） | 强 / 8 tick | +6° 收缩回弹 4 tick | 蓄力段镜头微震随充能爬升 |
| baomai.full_power_release | 强 / 6 tick | +4° | 与力竭灰雾同步 |
| woliu.turbulence_burst | 中 / 6 tick | +3° | |
| zhenmai.sever_chain | 中 / 4 tick | — | 断链顿挫感 |
| anqi.echo_fractal（release） | 弱 / 3 tick | — | |
| 其余招式 | 无（默认零） | — | 沉浸式极简：juice 只给重型招 |

- 复用 `CameraShakeController` 既有 mixin，不新建相机通道；FOV 走独立控制器（新建 `CastFovController`，与 shake 同帧调度）。
- **`CastFovController` 生命周期契约（交付物，不许只交一个孤立类）**：
  - 状态机：`idle → pulse → decay → idle`，所有路径终点必须回到进入施法前的**单一基准 FOV**，复位幂等（重复复位无副作用）；
  - 驱动路径唯一：由 `CastSyncHandler` 的 cast 状态转换回调驱动（started/accepted/release/reject/cancel），client tick 循环负责 decay 推进；bootstrap 注册位置跟随 `CombatJuiceSystem.bootstrap()` 先例（`BongClient.java:127`），cast 时序数据源为 `CastStateStore`（施法预测先例 `CombatHudBootstrap.java:48-49`）；
  - 与其他 FOV 修改源（原版疾跑/药水、shader）的合成规则显式声明（加法偏移量，不直写绝对 FOV）；
  - teardown：断线、切世界、玩家死亡时立即复位基准并清 pending 状态。
- 可及性：juice 强度全局倍率进 client 配置（0 = 关闭），默认 1.0；**进行中把倍率调 0 立即复位**，不是只影响后续脉冲。
- 测试（饱和覆盖状态机，**从真实 `CastSyncHandler` 入口驱动，不直接调 controller 方法**）：profile 注册表单测（每招参数 pin）+ 状态转换全路径——正常 release 完整脉冲后自然回基准、server 拒绝不触发、晚到打断作废 pending juice、回执乱序（打断先于 started 到达）、同 cast identity 重复 release 幂等、连续施法（前一脉冲 decay 中开新 cast）、重叠触发合成、倍率 0 进行中立即复位、死亡/切世界/断线复位——每条路径末尾断言 FOV == 基准值。

## P4 — 签名音效资产化

- 范围（音源渠道 §8 #2 拍板后执行）：每流派 signature 招 1-2 条，首批 8 条——`sword_path.heaven_gate`（charge 尾程 + release）、`woliu.void_core` 或 `heart`、`zhenmai.sever_chain`、`baomai.full_power_release`、`dugu.infuse_poison`、`tuike.shed`、`morph.yixing`、`anqi.echo_fractal`。音色基调：末法衰败（骨裂声/锈金属/闷雷/砂砾摩擦），禁华丽仙侠音。
- 管线建立（一次性基建，后续扩曲目录复用）：音源 → `.ogg`（mono, 44.1kHz，短样本 ≤3s）→ `client/.../assets/bong/sounds.json`（新建，注册 `bong:skill.<school>.<move>` 事件）→ 对应 `server/assets/audio/recipes/*.json` 把主层 sound id 从 `minecraft:` 换成 `bong:` 事件（recipe 其余混音层保留做空间感铺底）。
- **资产变更硬约束**：同步 `resourcepack.rs` + committed manifest 的 sha1/size（否则 Build resource pack CI 红）。
- **跨端音效契约测试（同一测试联结三级 ID，不许 server/client 各自维护互不校验的清单）**：提取全部 server audio recipe 引用的 `bong:` sound ID 集合，断言 ⊆ client `sounds.json` 事件键集合；`sounds.json` 每个事件引用的 `.ogg` 文件真实存在且已进 committed manifest（双向：注册无文件、文件无注册均判红）；错误分支覆盖重复键、非法命名空间前缀、缺文件。首批 8 条 signature 招逐项 pin：各自 recipe **真实引用**了为它新建的 `bong:` 事件（防资产只注册不消费）。

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

> 本节按 `docs/CLAUDE.md §5.1` 追加。#3/#4 由两个并行 Explore agent 核查 origin/main（`9a2cff02c`）代码现状产出（不拍脑袋）；#1/#2 为用户决策门，仅记录收口路径与卡点，待用户拍板后补决议。

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
