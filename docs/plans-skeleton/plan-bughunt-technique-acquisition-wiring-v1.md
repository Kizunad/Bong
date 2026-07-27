# plan-bughunt-technique-acquisition-wiring-v1（骨架）

> **一句话主题**：功法「获得途径」设计了四条（残卷研读 / 观摩偷师 / 师承传功 / 首击领悟），实际活着的只有残卷与首击领悟——观摩与师承两条链的判定 helper、冷却结构、学习源枚举全部齐备，却**零生产调用点**；且学习成功时刻**不存在任何 server→client 通知桥**（client `TechniqueObserveHud.showObservedLearned` 全 client 零调用方）。本 plan 把两条死路接进运行时，并把学习时刻的跨端反馈链一并闭合。

> 来源：technique 流派系统专项审查（2026-07-26，条目 M1 / M2 / S1）。骨架（草案），只记录缺陷与修复骨架，不含实施。

## 阶段总览

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | 学习时刻跨端桥（`TechniqueLearnedV1` S2C + LearnSource 路由） | ⬜ |
| P1 | 观摩偷师接线（M1，含运行时注册验收） | ⬜ |
| P2 | 师承传功接线（M2，含生产对话链验收） | ⬜ |
| P3 | 防回归：注册表级 wiring guard + 双链 e2e | ⬜ |

> 阶段依赖：P1/P2 的「学会 → HUD 反馈」验收依赖 P0 的桥先落地；P1 与 P2 之间无依赖，可并行。

## 接入面

- **进料**：`cultivation::technique_observe`（`OBSERVE_RANGE_BLOCKS`:12、`ObserveCooldowns`:35、`observe_learn_chance`:64、`evaluate_observe_attempt`:90，纯函数与冷却结构已备）；`cultivation::technique_mentor`（`mentor_dialog_option_appears`:55、`mentor_teaches_technique`:66）；既有 cast **成功完成**事件流（观摩只挂走完守恒结算的施法，不挂 attempt）；NPC 交互链 `ClientRequestV1::NpcInspectRequest`（`server/src/network/client_request_handler.rs:1302` 处理分支）。
- **出料**：统一学习入口 `learn_technique_if_allowed` → `KnownTechniques` → `TechniqueLearnedEvent`；**新增** S2C `TechniqueLearnedV1` payload（P0）→ client 按 `LearnSource` 路由到 `TechniqueObserveHud.showObservedLearned` / 师承 toast；天道叙事桥**不在本 plan**，归 [[plan-bughunt-technique-feedback-bridge-v1]]。
- **共享类型 / event**：复用 `LearnSource`（`cultivation/technique_scroll.rs:26-42`，六变体 `Scroll` / `Observe` / `Mentor` / `DyingMaster` / `DevCommand` / `CombatInsight`；`Mentor`:33、`DyingMaster`:36 现均不可达）、`TechniqueLearnedEvent`；**零新枚举**（S2C payload 的 learn_source 字段镜像该枚举，proto3 铁律 enum→全名字符串）。
- **跨仓库契约**：server `TechniqueLearnedEvent` → **新增** `ServerDataPayloadV1::TechniqueLearned(TechniqueLearnedV1)`（proto + 双 struct + 双 From + convert + emit + schema regenerate，按既有 server_data payload 加字段双端落地清单）→ client 新 handler → `TechniqueObserveHud`；`TechniquesSnapshotV1` 面板同步照旧（snapshot ≠ 学习时刻 toast，两者独立验收）。
- **worldview 锚点**：worldview.md §十三 L752（垂死大能传功情境）、L1139（师承关系）、L1113（道统遗物稀缺性——观摩免费习得的经济边界见开放问题 #2）。
- **qi_physics 锚点**：学习行为本身不产生真元流动；观摩判定只消费**已走完既有 cast 守恒路径**的成功施法事件，**零新增 qi 公式/常数**。

## P0 — 学习时刻跨端桥 ⬜

**证据**：`server/src/schema/server_data.rs` 无任何 TechniqueLearned 类 S2C payload；client `TechniqueObserveHud.showObservedLearned`（`client/src/main/java/com/bong/client/cultivation/TechniqueObserveHud.java:13`）全 client 零调用方。学会时刻玩家唯一可见的是 snapshot 面板数值静默变化。

**交付物**：
- schema：`TechniqueLearnedV1 { technique_id, display_name, learn_source }` + 正反 sample + pin 测试（六个 `LearnSource` 变体逐一对拍）
- server emitter system：消费 `TechniqueLearnedEvent` → **只定向发给学习者 client**；注册点写死在 `cultivation::register`（`server/src/cultivation/mod.rs:216`）或 network emit 层既有 schedule（实施时二选一并在 plan 更新落点）
- client handler：按 `learn_source` **六变体穷尽路由**——`Observe` → `TechniqueObserveHud.showObservedLearned`；`Mentor` → 师承专属 toast；`DyingMaster` → 同 Mentor 路由（运行态归 dying-master plan，但 wire 值经本桥即可出现，不留空）；`Scroll` → 既有残卷反馈，本桥不重复弹；`CombatInsight` → 既有领悟反馈，本桥不重复弹；`DevCommand` → 静默（仅面板刷新，不发玩家叙事）；未知 learn_source 字符串 → 静默忽略 + 日志（前向兼容）
- 测试：payload 双端对拍逐变体 pin；emitter 定向不广播；client 路由分支逐变体断言；残卷/首击领悟两条既有路径接上后行为不变（回归）

## P1 — 观摩偷师接线（M1）⬜

**证据**：`server/src/cultivation/technique_observe.rs:90` `evaluate_observe_attempt` 与 `:35 ObserveCooldowns`、`:64 observe_learn_chance` 全仓仅自身文件引用（单测），无任何生产 system 调用。触发场景：玩家站在施法者附近观摩，永远不会发生学习判定。

**交付物**：
- 生产 system（观摩判定 tick）：订阅附近玩家/NPC **cast 成功完成**事件（不挂 cast attempt——被拒/取消/真元不足的施法不可观摩）→ 距离门用 `OBSERVE_RANGE_BLOCKS`（technique_observe.rs:12 权威常数，**不另拍数**；距离度量语义——欧氏 vs Chebyshev、跨维度一律不判定——本阶段写死并测试）→ `evaluate_observe_attempt` → 命中走 `learn_technique_if_allowed`（realm/race/form-anchor/meridian 门照常）→ emit `TechniqueLearnedEvent { source: LearnSource::Observe }`
- **运行时注册是显式交付物**：system 注册进 `cultivation::register`（cultivation/mod.rs:216）的 Update schedule；`ObserveCooldowns` 挂玩家实体 + 断线清理
- 测试**从真实 app 构建出发**（不直接调 helper）：观摩命中/未命中、冷却窗、`OBSERVE_RANGE_BLOCKS` 边界 off-by-one、跨维度不判定、已知功法跳过、学习门拒绝分支、断线冷却清理；契约断言 `TechniqueLearnedEvent` 携带**精确** `LearnSource::Observe`，拒绝/重复学习不发成功事件

## P2 — 师承传功接线（M2）⬜

**证据**：`server/src/cultivation/technique_mentor.rs:55` `mentor_dialog_option_appears`、`:66 mentor_teaches_technique` 无消费者（`server/src/network/vfx_animation_trigger.rs:326` 注释自认「无生产调用方的休眠 helper」）；`LearnSource::Mentor`（technique_scroll.rs:33）不可达。触发场景：NPC 交互永远不出现传功选项。

**交付物**：
- 挂载点：`NpcInspectRequest` 处理分支（`client_request_handler.rs:1302`）的响应 payload 增加传功选项字段（以 `mentor_dialog_option_appears` 为条件）+ 新 C2S 选择请求（沿 Bong C2S IntentHandler 模式，不走 vanilla InteractEntityEvent）→ server 侧 `mentor_teaches_technique` → `learn_technique_if_allowed` → emit `TechniqueLearnedEvent { source: LearnSource::Mentor }`
- **生产对话链集成测试（不直接调 helper）**：真实 app 构建 + NpcInspect 请求 → 响应含传功选项 → 发选择请求 → 学会 → 断言精确 `LearnSource::Mentor` → S2C `TechniqueLearnedV1` 下发；选项条件不满足时响应不含选项；传功被学习门拒绝、重复传功幂等各一条

**划界**：`LearnSource::DyingMaster`（technique_scroll.rs:36）的可达性依赖垂死大能遭遇本体接活（spawn/交互当前零运行态），归 [[plan-woliu-dying-master-runtime-gap-v1]]；本 plan 交付的传功调用面（`mentor_teaches_technique` 接通 + S2C 桥）是其未来复用的前置。

## P3 — 防回归 ⬜

- **注册表级 wiring guard**（不数源码调用方——未注册的休眠 wrapper 也有"调用方"，正是本 plan 要修的形态）：测试构建生产 app 后断言观摩 system 在 schedule 中、NpcInspect 响应 schema 含传功选项字段；**注册被移除时 guard 必须红**
- 双链 e2e：观摩链（bot A 施法、bot B `OBSERVE_RANGE_BLOCKS` 内观摩 → B 收到 `TechniqueLearnedV1` + snapshot 更新）与师承链（inspect → 选项 → 选择 → 学会 → toast payload）各一条，端到端断言 `LearnSource` 精确变体

## 开放问题（P0 决策门前需收口）

- **#1（审查 S1，需用户拍板）cast 时是否查 `required_realm`——「学会不忘」vs「跌境封招」**。现状证据：技能栏 cast 入口检查 拥有/race/form-anchor/经脉，唯独不查 realm；全仓仅丹道用 `CastRejectReason::RealmTooLow`（`server/src/dandao/skills.rs:217,223`），枚举变体存在但几乎无人消费；同时仓内存在主动跌境机制（heaven_gate 跌境 / RealmRegressed），跌境后仍可施放化虚级招式。拍「跌境封招」→ 统一入口加 realm 门 + HUD 拒绝理由；拍「学会不忘」→ 明文注释口径并处理 RealmTooLow 的孤儿语义。
- **#2 观摩免费习得的经济边界**：会不会击穿残卷流通价值（worldview.md L1113 道统遗物稀缺）——是否要求目标功法品阶上限 / 观摩仅得极低初始熟练度；观摩对 NPC 施法（npc.\* 前缀）的 eligibility 与 [[plan-bughunt-technique-proficiency-growth-v1]] 开放问题 #2 的 npc.\* allowlist **共用同一决议**，不各自维护排除逻辑。
- **#3 观摩/传功成功时刻的 A/V 规格**：粒子基类/数量/lifetime、SFX recipe、narration 文案示例——转 active 前按 docs/CLAUDE.md §四 视听精度要求补齐，不允许"学会时冒个光"一笔带过。
