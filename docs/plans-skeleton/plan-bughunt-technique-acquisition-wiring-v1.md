# plan-bughunt-technique-acquisition-wiring-v1（骨架）

> **一句话主题**：功法「获得途径」设计了四条（残卷研读 / 观摩偷师 / 师承传功 / 首击领悟），实际活着的只有残卷与首击领悟——观摩与师承两条链的判定 helper、冷却结构、学习源枚举全部齐备，却**零生产调用点**，玩家永远触发不了。本 plan 把两条死路接进运行时。

> 来源：technique 流派系统专项审查（2026-07-26，条目 M1 / M2 / S1）。骨架（草案），只记录缺陷与修复骨架，不含实施。

## 阶段总览

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | 观摩偷师接线（M1） | ⬜ |
| P1 | 师承传功接线（M2） | ⬜ |
| P2 | 防回归 wiring guard + 集成测试 | ⬜ |

## 接入面

- **进料**：`cultivation::technique_observe`（`ObserveCooldowns`:35、`observe_learn_chance`:64、`evaluate_observe_attempt`:90，纯函数与冷却结构已备）；`cultivation::technique_mentor`（`mentor_dialog_option_appears`:55、`mentor_teaches_technique`:66）；既有 cast 事件流（观摩需感知附近他人施法）；NPC 对话树。
- **出料**：统一学习入口 `learn_technique_if_allowed` → `KnownTechniques` → `TechniqueLearnedEvent`；client `TechniqueObserveHud.showObservedLearned` toast（已存在，等激活）；天道叙事桥**不在本 plan**，归 [[plan-bughunt-technique-feedback-bridge-v1]]。
- **共享类型 / event**：复用 `LearnSource`（`cultivation/technique_scroll.rs:26`；`Mentor`:33、`DyingMaster`:36 现均不可达）、`TechniqueLearnedEvent`；**零新枚举**。
- **跨仓库契约**：server 学习入口 → `TechniquesSnapshotV1` → client 功法面板；`TechniqueObserveHud`（client/src/main/java/com/bong/client/cultivation/）。
- **worldview 锚点**：worldview.md §十三 L752（垂死大能传功情境）、L1139（师承关系）、L1113（道统遗物稀缺性——观摩免费习得的经济边界见开放问题 #2）。
- **qi_physics 锚点**：学习行为本身不产生真元流动；观摩判定挂在他人真实 cast 上，沿用既有 cast 守恒路径，**零新增 qi 公式/常数**。

## P0 — 观摩偷师接线（M1）⬜

**证据**：`server/src/cultivation/technique_observe.rs:90` `evaluate_observe_attempt` 与 `:35 ObserveCooldowns`、`:64 observe_learn_chance` 全仓仅自身文件引用（单测），无任何生产 system 调用；client `TechniqueObserveHud.showObservedLearned` 因此永不显示。触发场景：玩家站在施法者附近观摩，永远不会发生学习判定。

**交付物**：
- 生产 system：订阅附近玩家/NPC cast 完成事件 → 距离门（16 格）→ `evaluate_observe_attempt` → 命中则走 `learn_technique_if_allowed`（realm/race/form-anchor/meridian 门照常生效）
- `ObserveCooldowns` 挂载玩家实体 + 断线清理
- 成功路径 emit `TechniqueLearnedEvent`，client toast 链路激活
- 测试：命中/未命中概率分支、冷却窗、距离边界（16 格 off-by-one）、已知功法跳过、学习门拒绝分支、断线冷却清理

## P1 — 师承传功接线（M2）⬜

**证据**：`server/src/cultivation/technique_mentor.rs:55` `mentor_dialog_option_appears`、`:66 mentor_teaches_technique` 无消费者（`server/src/network/vfx_animation_trigger.rs:326` 注释自认「无生产调用方的休眠 helper」）；`LearnSource::Mentor`（technique_scroll.rs:33）不可达。触发场景：NPC 对话永远不出现传功选项。

**交付物**：
- NPC 对话树接 `mentor_dialog_option_appears` 条件选项 → 选中后 `mentor_teaches_technique` → `learn_technique_if_allowed`，`LearnSource::Mentor` 可达
- 测试：选项出现条件（好感/身份前置）、传功成功/被学习门拒绝、重复传功幂等

**划界**：`LearnSource::DyingMaster`（technique_scroll.rs:36）的可达性依赖垂死大能遭遇本体接活（spawn/交互当前零运行态），归 [[plan-woliu-dying-master-runtime-gap-v1]]；本 plan 只保证传功调用面接通，不做遭遇 spawn。

## P2 — 防回归 ⬜

- wiring guard 测试：`evaluate_observe_attempt` / `mentor_teaches_technique` 必须存在非测试调用方（对齐 `test_coverage_guards.rs` 既有模式）
- server 集成测试走「他人施法 → 观摩判定 → 学会 → snapshot 下发」完整链
- 学会时刻 HUD toast 回归（`TechniqueObserveHud`）；narration 回归归 feedback-bridge plan

## 开放问题（P0 决策门前需收口）

- **#1（审查 S1，需用户拍板）cast 时是否查 `required_realm`——「学会不忘」vs「跌境封招」**。现状证据：技能栏 cast 入口检查 拥有/race/form-anchor/经脉，唯独不查 realm；全仓仅丹道用 `CastRejectReason::RealmTooLow`（`server/src/dandao/skills.rs:217,223`），枚举变体存在但几乎无人消费；同时仓内存在主动跌境机制（heaven_gate 跌境 / RealmRegressed），跌境后仍可施放化虚级招式。拍「跌境封招」→ 统一入口加 realm 门 + HUD 拒绝理由；拍「学会不忘」→ 明文注释口径并处理 RealmTooLow 的孤儿语义。
- **#2 观摩免费习得的经济边界**：会不会击穿残卷流通价值（worldview.md L1113 道统遗物稀缺）——是否要求目标功法品阶上限 / 观摩仅得极低初始熟练度。
- **#3 观摩/传功成功时刻的 A/V 规格**：粒子基类/数量/lifetime、SFX recipe、narration 文案示例——转 active 前按 docs/CLAUDE.md §四 视听精度要求补齐，不允许"学会时冒个光"一笔带过。
