# plan-neardeath-ux-v1 — 濒死窗口玩家侧 UX：权威契约 + 倒计时/自救他救条 + 挣扎/救援交互 + 倒地姿态广播

> **一句话**：服务端濒死状态机（NearDeath 30s 窗口 → AwaitingRevival 裁决）已完整，但玩家侧是黑箱——本 plan 把濒死期做成**可见、可操作、可救援**的玩法段：权威 S2C 契约、`[====]` 自救他救进度条、挣扎自救、队友渡真元救援、倒地姿态全员可见。
>
> 来源：2026-07-06 bot playtest 协议层实证（kill/revive 链路通但濒死玩家侧零可观察事件）+ 用户拍板要 `[====]` 自救他救条（memory `project_neardeath_ux_gap`）。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | 权威 NearDeathState S2C 契约 + client 切权威驱动 + bot 可观察性场景 | ⬜ |
| P1 | `[====]` 双条 HUD（窗口倒计时 + 自救他救进度） | ⬜ |
| P2 | 自救：挣扎 intent（消耗真元，走 qi_physics 守恒） | ⬜ |
| P3 | 他救：渡真元救援 intent（QiTransfer 救者→倒地者） | ⬜ |
| P4 | 倒地姿态广播（他人视角可见倒地/起身） | ⬜ |
| P5 | 数值收口 + 双 bot 救援 e2e + 首次濒死生还成就对齐 | ⬜ |

---

## §1 接入面（docs/CLAUDE.md §二 checklist）

- **进料**：
  - `combat::lifecycle` 状态机全套（**只消费不重造**）：`LifecycleState`（components.rs:175）、`Lifecycle::near_death_deadline_tick`、`enter_near_death`（lifecycle.rs:2106）、稳定逃生分支（`near_death_tick` lifecycle.rs:754-761，HP 回升过 `NEAR_DEATH_HEALTH_FRACTION=0.05` → Alive）
  - `cultivation` 真元（挣扎消耗、他救渡入）
  - `client_request_handler` 现有 `RevivalActionIntent` 事件管线（events.rs:356-367）
- **出料**：
  - `ServerDataPayloadV1::NearDeathState` → client HUD/动画/他人视角
  - 救援成功/挣扎逃生 → 复用稳定逃生分支（不新开复活路径）；`BiographyEntry` 追加救援记录（life_record.rs:108 旁）
  - 首次濒死生还 → worldview :671 成就钩子
- **共享类型 / event**：复用 `LifecycleState`/`Lifecycle`/`DeathEvent`/`RevivalActionIntent`；**不新造任何死亡/复活 event**。新增仅限：`NearDeathState` payload、`NearDeathStruggle`/`NearDeathRescue` 两个 C2S variant、`RescueProgress` 组件。
- **跨仓库契约**：
  - server：`schema/server_data.rs::NearDeathState` + `schema/client_request.rs::{NearDeathStruggle,NearDeathRescue}` + `proto_convert.rs` 桥（对齐 #662 死亡屏桥模式 :953-985）
  - client：`NearDeathStateHandler`（ServerDataHandler）、`NearDeathBarsHudPlanner`、`NearDeathRescueIntentHandler`
  - agent：不参与（纯 server↔client；narration 走 server 本地 Narration payload）
  - schema samples：`agent/packages/schema/samples/` 正反对拍各 1 份
- **worldview 锚点**：§十二 死亡、重生与一生记录（worldview.md:1010）；濒死生还语义 :671/:695「首次濒死生还（气血 <5% 后逃出生天）」——挣扎/救援正是「逃出生天」的玩法实现，不改正典只落地它。
- **qi_physics 锚点**：
  - 挣扎消耗：`qi_release_to_zone(amount, region, env)`（消耗的真元守恒释放回 zone，不蒸发）
  - 他救渡真元：`qi_physics::ledger::QiTransfer { from: 救者, to: 倒地者, amount }`——玩家↔玩家流动必走 ledger
  - **不新增任何衰减/物理常数**；救援速率/挣扎消耗是玩法参数（本 plan §7 数值表声明），公式归 qi_physics

## §2 已有地基（2026-07-07 Explore 实测，写实施代码前以此为准）

- **状态机全套已实装**（`plan-death-lifecycle-v1` 遗产）：四态 + `NEAR_DEATH_WINDOW_TICKS=600`(30s) + `REVIVAL_CONFIRM_WINDOW_TICKS=1200`(60s) + `REVIVE_WEAKENED_TICKS=3600`(180s) + Fortune/Tribulation 裁决（`determine_revival_decision` lifecycle.rs:1316，运数期 ≤3 次 100%，劫数期 `max(5%, 80%-15%×(n-3))`）
- **AwaitingRevival 段可观察性已完整**：`DeathScreen`/`TerminateScreen` payload（server_data.rs:463-482）+ client `DeathScreen` UI（倒计时/luck bar/重生/终结按钮）+ 死亡电影分镜——**本 plan 不动这段**
- **client 已有濒死表现资产**（`plan-player-animation-v1` 遗产）：`death_collapse.json`/`rebirth_wake.json` 等姿态、`NearDeathCollapsePlanner`（倒地动画）、`NearDeathOverlayPlanner`（红屏 vignette + hold-on 文本，THRESHOLD=0.12）
- **关键缺陷：以上 client 表现全靠本地 HP<0.12 推断触发**，非权威 `LifecycleState`——拿不到 deadline_tick（倒计时不准）、拿不到死因、他人视角完全无感知
- **交互先例**：dying_elder G 键给丹（`DyingElderInteractionKeybindings` + C2S `give_dan_to_elder`）= 最近的"对濒死者施救"模式；`IntentHandler` 接口（input/IntentHandler.java:7）+ 十余个实体交互实现类；**无玩家↔玩家救援先例，无 struggle/rescue intent**（client_request.rs 只有 CombatReincarnate/CombatTerminate）
- **条渲染先例**：`MiniBodyHudPlanner`（8×65 竖条）、`ChargingProgressBarHud`（横向进度条）

## §3 P0 — 权威 NearDeathState 契约 + client 切权威驱动 ⬜

- `schema/server_data.rs`：`ServerDataPayloadV1::NearDeathState { v, active: bool, cause: String, deadline_tick: u64, now_tick: u64, struggle_progress: f32, rescue_progress: f32, rescuer_name: Option<String> }`；enter/every-second/exit 三类时机 emit（进度变更即时推）
- `proto_convert.rs` 桥 + proto 定义 + `samples/near_death_state_{active,cleared}.json` 双端对拍（proto3 铁律：u64→字符串，见 [[project_wire_format_bridge_audit]]）
- emit 点挂 `near_death_tick`（lifecycle.rs:713）：进入/每 20 tick/逃生/转 AwaitingRevival 各一发
- client：`NearDeathStateHandler` 写 store；`NearDeathOverlayPlanner`/`NearDeathCollapsePlanner` 触发源从 HP 阈值**切换为权威 store**（HP 推断分支删除，不留双源）
- bot 场景 `lifecycle_neardeath_observability.py`：`/kill self` → 必收 `NearDeathState{active:true}` 且 deadline_tick > now_tick；`/revive self` → 必收 `active:false`。锁「倒下必有权威可观察事件」（AGENTS.md §15.2）
- 测试：schema 正反 sample 对拍、payload 字段 pin、emit 时机单测（enter/periodic/exit 三态转换各一）

## §4 P1 — `[====]` 双条 HUD ⬜

- 新 `NearDeathBarsHudPlanner`（复用 `ChargingProgressBarHud` 横条渲染模式，挂 `BongHudOrchestrator.buildCommands`）
- **视听规格**：
  - 窗口倒计时条：屏幕中下（death screen 同水位线上方 24px），宽 180px 高 6px；底色 `#3A0A0A` 80%，填充 `#C22B2B`，随 `(deadline_tick-now)/600` 线性衰减；剩 <10s 填充切 `#FF5533` 并 2Hz 闪烁
  - 自救他救进度条：紧贴上方 4px，同尺寸；底色 `#1A1A0A`，自救填充 `#D9A441`（金），他救期间填充切 `#41C9D9`（青）且左端显示 `rescuer_name`
  - 两条仅 `active:true` 渲染，Alive 零渲染（[[feedback_hud_conditional]]）；fade in 6 tick / fade out 10 tick，ease-out
  - 文案行：条下 10px 居中，「挣扎求生 [连按空格]」/ 他救时「XX 正在渡真元」
- 测试：planner 快照单测（active/inactive/双源切换/rescuer_name 有无 4 分支）

## §5 P2 — 自救挣扎 ⬜

- C2S `ClientRequestV1::NearDeathStruggle { v }`（连按空格映射，client 侧 4 tick 冷却防 spam；server 侧同样限速——宽容不踢，超速静默丢弃）
- server `handle_neardeath_struggle`（新 system，挂 `handle_revival_action_intents` 旁）：仅 `LifecycleState::NearDeath` 受理；每次 `struggle_progress += STRUGGLE_GAIN`，同时消耗 `STRUGGLE_QI_COST` 真元经 `qi_release_to_zone` 守恒释放（真元不足则本次无效并 emit 拒因 narration）
- `struggle_progress >= 1.0` → 把 HP 拉到 `NEAR_DEATH_HEALTH_FRACTION + 0.01`，让现有稳定逃生分支（lifecycle.rs:754-761）自然接管——**不新开复活路径**
- **视听规格**：每次有效挣扎——粒子 `BongSpriteParticle`×6，burst，lifetime 8 tick，速度 0.05 向上，颜色 `#D9A441`，贴图复用 `qi_wisp`；音效 recipe `neardeath_struggle.json`：layer1 `entity.player.breath` pitch 0.7 vol 0.5，layer2 `block.gravel.step` pitch 0.5 vol 0.3 delay 2 tick；attenuation MELEE（=AUDIO_MELEE_RADIUS，[[reference_server_data_payload_field]] 半径一致铁律）
- narration（player scope，perception style）：「你抓住一缕将散的真元，指节抠进土里。」/「气血逆冲——还不能死在这里。」/ 逃生成功（zone scope）：「XX 从鬼门关爬了回来。」
- 测试：状态前置（Alive/AwaitingRevival 发挣扎→拒）、真元不足分支、进度累积→逃生转换、qi 守恒断言（zone 增量=玩家消耗）、限速丢弃

## §6 P3 — 他救渡真元 ⬜

- C2S `ClientRequestV1::NearDeathRescue { v, target_entity_id: i32, action: Start|Stop }`；client 侧 `NearDeathRescueIntentHandler`（IntentHandler 模式，准星对准倒地玩家 + 潜行长按右键 channel，松开发 Stop）
- server `RescueProgress` 组件挂倒地者：`rescuer: Entity, progress: f32`；channel 期间每 20 tick `QiTransfer{from:救者, to:倒地者, amount:RESCUE_QI_PER_SEC}` 走 ledger；救者真元不足/移出 3 格/自身进战斗 → 自动 Stop，progress 每秒衰减 `RESCUE_DECAY_PER_SEC`
- `rescue_progress >= 1.0` → 同 P2 汇入稳定逃生分支；`struggle_progress` 与 `rescue_progress` 取 max 显示在同一条（自救他救共享 `[====]`，见 §8#6）
- **视听规格**：channel 期间——救者与倒地者之间 `BongRibbonParticle` 连线，continuous，每 tick 1 条，lifetime 10 tick，颜色 `#41C9D9` → `#D9A441` 渐变；音效 `neardeath_rescue_channel.json`：layer1 `block.beacon.ambient` pitch 1.4 vol 0.35 loop；成功时 `neardeath_rescue_done.json`：`block.beacon.power_select` pitch 1.6 vol 0.6；attenuation MELEE
- narration：救者（player/perception）「你把真元渡过去，像往漏壶里添水。」；倒地者「有人的真元顺着经脉爬进来，烫的。」；成功（zone/narrative）「XX 把 YY 从土里拖了回来。」
- 测试：双实体状态机（Start/Stop/打断/距离/进度衰减/共享条 max 语义）、QiTransfer 守恒对拍、倒地者中途被终结时 RescueProgress 清理

## §7 P4 — 倒地姿态广播 + P5 数值收口/e2e ⬜

- **P4**：server `enter_near_death` 时经现有 `VfxEntityAnimationBridge` 通道向**视野内**玩家广播实体动画事件（半径过滤复用 #1069 `disguise_sync::ids_visible_to_client` 模式）——他人客户端对该实体播 `death_collapse.json`（已有资产，endTick 保持），逃生播 `rebirth_wake.json`，AwaitingRevival/Terminated 维持倒地帧；断线重连玩家由 P0 payload 的周期 emit 补状态
- **P5 数值表**（初值，平衡期可调）：`STRUGGLE_GAIN=0.12`（约 9 次满）、`STRUGGLE_QI_COST=2.0`、`RESCUE_QI_PER_SEC=6.0`（约 8s 满）、`RESCUE_DECAY_PER_SEC=0.08`、救援距离 3.0 格——全部 const 声明在 `combat/components.rs` 常量区，测试引用 const 不写字面量
- **P5 e2e**：双 bot 场景 `lifecycle_rescue_e2e.py`（A `/kill self` 倒地 → B 发 Rescue Start → 断言 A 收 rescue_progress 上升 + 最终 `active:false` + A HP>5%）+ 挣扎自救单 bot 场景；首次濒死生还成就钩子对齐 worldview :671/:695

## §8 开放问题（转 active 前需按 docs/CLAUDE.md §五 收口成 §8.1 决议）

1. **挣扎输入形态**：连按空格 vs 长按 vs QTE。**推荐连按空格**——最接近"挣扎"体感、client 改动最小、bot 可直接测（连发 intent）。
2. **自救成功语义**：进度满=稳定逃生（推荐，复用现有分支零新路径） vs 只延长 deadline。延长窗口会让 30s 常数失去意义，不推荐。
3. **他救介质**：v1 徒手渡真元（推荐，QiTransfer 干净守恒） vs 必须喂丹。喂丹复用 dying_elder 给丹模式留 v2 增强（丹药加速渡真元倍率）。
4. **PvP 处决**：现状 player_attack 禁止攻击倒地者（player_attack.rs:68）。**推荐维持**；处决玩法留未来 PvP plan，本 plan 不碰。
5. **NPC 参与**：NPC 现在跳过濒死窗口直接终结（lifecycle.rs:873）。**推荐 v1 玩家↔玩家 only**；NPC 救人/救 NPC 都不做。
6. **双条 vs 单条**：倒计时+进度两条（推荐，语义清晰） vs 单条双色叠加。自救他救共享一条进度（取 max）已定，争议只在倒计时是否独立条。
7. **挣扎与他救叠加**：progress 取 max（推荐，防双人刷满过快） vs 相加。相加需要重调数值表且鼓励脚本连点，不推荐。

## §9 实施备注

- scope 预估 4 PR（P0 / P1+P2 / P3 / P4+P5），转 active 时按 docs/CLAUDE.md §六 补 §10 实施工作流章节
- 每 PR 必配 bot 场景（AGENTS.md §15 硬约定）；P0 的 observability 场景是后续所有 PR 的回归底座；P5 双 bot 救援场景与 [[plan-bot-e2e-coverage-v1]] P5 多 bot 族共享框架先例
- 全程不改 `docs/worldview.md`（濒死生还语义已有正典锚点，纯落地）
- **2026-07-18 诊断补充**：早期玩法诊断把本 plan 列为「反馈层黑箱」三件之首（另两件：[[plan-combat-event-juice-runtime-bridge-gap-v1]] 命中手感断桥、`plan-beast-horde-v1`（active）P2/P3 兽潮玩家侧 VFX/叙事）——服务端状态机完整而玩家零感知的形态高度一致，实施排期建议三件同批收口，"世界在动但看不见"的空洞感才能整体消除
