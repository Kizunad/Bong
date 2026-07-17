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
| P0 | FPV 技术路线 POC（三选一拍板）+ 工具链增强 | ⬜ |
| P1 | FPV 基础设施：per-anim 第一人称配置 + `_fpv` 变体查找链 | ⬜ |
| P2 | 主力招 FPV 手臂动画批量产出（3 轮打磨 + PROMISE） | ⬜ |
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
- 可及性：juice 强度全局倍率进 client 配置（0 = 关闭），默认 1.0。
- 测试（饱和覆盖状态机）：profile 注册表单测（每招参数 pin）+ release 触发时序单测——正常 release 触发、server 拒绝不触发、晚到打断作废 pending juice、回执乱序（打断先于 started 到达）、同 cast identity 重复 release 幂等、倍率 0 完全关闭。

## P4 — 签名音效资产化

- 范围（音源渠道 §8 #2 拍板后执行）：每流派 signature 招 1-2 条，首批 8 条——`sword_path.heaven_gate`（charge 尾程 + release）、`woliu.void_core` 或 `heart`、`zhenmai.sever_chain`、`baomai.full_power_release`、`dugu.infuse_poison`、`tuike.shed`、`morph.yixing`、`anqi.echo_fractal`。音色基调：末法衰败（骨裂声/锈金属/闷雷/砂砾摩擦），禁华丽仙侠音。
- 管线建立（一次性基建，后续扩曲目录复用）：音源 → `.ogg`（mono, 44.1kHz，短样本 ≤3s）→ `client/.../assets/bong/sounds.json`（新建，注册 `bong:skill.<school>.<move>` 事件）→ 对应 `server/assets/audio/recipes/*.json` 把主层 sound id 从 `minecraft:` 换成 `bong:` 事件（recipe 其余混音层保留做空间感铺底）。
- **资产变更硬约束**：同步 `resourcepack.rs` + committed manifest 的 sha1/size（否则 Build resource pack CI 红）。
- 测试：sounds.json 注册与 .ogg 文件一一对应扫描测试；recipe 引用的 `bong:` 事件全部可解析（防 recipe 指向不存在事件静默无声）。

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

## 测试声明

- client：FPV 查找链/回落/远端隔离单测、juice 状态机饱和单测（正常/拒绝/晚到打断/乱序/重复 release 幂等/倍率 0）、sounds.json↔ogg 双向对应扫描（注册无文件、文件无注册均判红）（gradlew test）；
- server：recipe 引用 `bong:` 事件解析测试（cargo test，指向不存在事件判红）；
- e2e：`bash scripts/smoke-test-e2e.sh` + bot cast 场景绿；资源包 CI（sha1 同步）绿。

## §10 实施工作流

- 单 plan 多 PR 序列化：PR-1 = P0 POC + 工具（含拍板记录回写 §8.1）；PR-2 = P1 基础设施；PR-3/4 = P2 分批 FPV 动画；PR-5 = P3 juice；PR-6 = P4 音效管线 + 首批；PR-7 = P5 收口。
- P2 批次依赖梯队二（`plan-skill-anim-fidelity-v1`）对应批次先 merge（FPV 以重制稿为底）；P3/P4 与 P2 无依赖可并行排期。
- 每 PR 独立实施 subagent；视觉/听觉资产 PR 走 3 轮 + `<PROMISE>`；CodeRabbit / `/review` 等待走 ScheduleWakeup 1200s 协议。
- **单次 consume-plan 全自动到 merge**：用户提交 `/consume-plan` 后全自动走完实施→review→merge→归档至 `docs/finished_plans/`，无需人工值守（例外：§8 #2 音源渠道属用户拍板项，须在 pre-P0 收口时定案，不阻塞其余 PR）。
