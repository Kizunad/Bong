# BugHunt: 丹道三基础招玩家技能栏接入断链

## Bug 摘要

`dandao.pill_rush` / `dandao.pill_bomb` / `dandao.pill_mist` 已注册到生产 `SkillRegistry`，并各自有境界、经脉、真元、冷却 resolver；但当前玩家技能面板与技能栏绑定所依赖的权威定义源 `TECHNIQUE_IDS` / `TECHNIQUE_DEFINITIONS` 没有这三招，`abilities_unlocked_at()` 也没有非测试消费方。因此正常客户端不会在 `techniques_snapshot` 里看到三招，`skill_bar_bind` / `skill_bar_cast` 会在 `technique_definition()` 或 KnownTechniques gate 前拒绝，resolver 不能通过玩家技能栏触发。

这不是“丹道流派整体不可用”：炼丹、丹毒变异、暴龙王等系统另有链路。这里报告的是 **基础丹道战斗招式的玩家 UI / 技能栏闭环不可达**，且后续修复必须按招式 A/V 差异化红线补齐 animation / VFX / SFX / HUD / icon。

## 实际游玩体验影响

玩家正常游玩丹道时，服务端已经有三基础招的数值与冷却逻辑，但客户端技能面板不会列出这些招式，玩家也不能把它们拖到 1-9 技能栏并施放。结果是丹道计划承诺的“醒灵服丹急行、引气投丹、凝脉丹雾”在实战 UI 中不可达；玩家只能体验炼丹/变异等周边系统，拿不到基础丹道战斗能力的成长反馈。

如果后续只把三招补进定义表但不补独立视听反馈，玩家即使能施放，也仍无法从动作、粒子、音效、HUD 或图标上区分“服丹急行 / 投丹 / 丹雾”，违反招式 A/V 差异化约束。

## 证据定位

- `server/src/dandao/mod.rs:60-63`：生产 `register_skills()` 把 `DANDAO_PILL_RUSH_SKILL_ID`、`DANDAO_PILL_BOMB_SKILL_ID`、`DANDAO_PILL_MIST_SKILL_ID` 注册进 `SkillRegistry`。
- `server/src/cultivation/skill_registry.rs:112`：生产 `init_registry()` 调用 `crate::dandao::register_skills(&mut registry)`。
- `server/src/dandao/skills.rs:23-25`：三招 ID 分别为 `dandao.pill_rush`、`dandao.pill_bomb`、`dandao.pill_mist`。
- `server/src/dandao/skills.rs:209`、`:253`、`:297`：三招 resolver 分别存在，并返回 cooldown / animation duration。
- `server/src/dandao/progression.rs:10-14`：境界解锁表把醒灵、引气、凝脉分别映射到三基础招；但全仓非测试调用只有本文件测试，未同步到玩家 `KnownTechniques`。
- `server/src/cultivation/known_techniques.rs:39-88`：`TECHNIQUE_IDS` 48 项无任何 `dandao.pill_*`；`server/src/cultivation/known_techniques.rs:133` 的 `TECHNIQUE_DEFINITIONS` 同样无丹道三招。
- `server/src/cultivation/known_techniques.rs:1014-1018`：`technique_definition()` 只从 `TECHNIQUE_DEFINITIONS` 查询。
- `server/src/network/techniques_snapshot_emit.rs:40-72`：`techniques_snapshot` 对 `KnownTechniques.entries` 做 `filter_map`，找不到 `TECHNIQUE_DEFINITIONS` 的条目会被直接丢弃。
- `client/src/main/java/com/bong/client/network/TechniquesSnapshotHandler.java:30-32`：客户端已学功法列表由 server `techniques_snapshot` 替换。
- `client/src/main/java/com/bong/client/combat/inspect/TechniquesTabPanel.java:216-232`：客户端拖拽绑定只会从当前 technique list 找到的条目发送 `skill_bar_bind`。
- `server/src/network/client_request_handler.rs:10887-10905`：`skill_bar_bind` 先调用 `technique_definition(skill_id)`，未知 skill 直接拒绝；再检查 `KnownTechniques`。
- `server/src/network/client_request_handler.rs:10122-10144`：`skill_bar_cast` 先查 `technique_definition()` 和 `KnownTechniques`，最后才 `SkillRegistry.lookup(&skill_id)`；丹道三招会在 resolver lookup 前被拒。
- `server/src/cultivation/technique_scroll.rs:85-87`：技能卷学习同样先查 `technique_definition()`，未知三招只会得到 `InvalidScroll`，不能绕过定义表。
- `server/src/network/skillbar_config_emit.rs:66-73`：技能栏配置也从 `technique_definition()` 派生 display/cast/cooldown/icon；缺定义时无法给客户端正常 skill slot 描述。
- `docs/finished_plans/plan-dandao-path-v1.md:211-258`：原计划把三招列为 P0 基础招，并明确写了服丹急行 / 投丹 / 丹雾的动画、粒子和 HUD 差异化要求。
- `client/src/main/resources/assets/bong/textures/particle/pill_glow.png`、`pill_trail.png`、`pill_burst.png`、`pill_mist.png` 已存在，不能声称粒子贴图全缺；但全仓未见 `vfx_dandao_pill_*`、`DandaoPill*Vfx`、玩家侧动画/SFX/HUD/icon 的三招接线。

## 触发路径

1. 玩家达到丹道基础招对应境界：醒灵应有 `dandao.pill_rush`，引气应有 `dandao.pill_bomb`，凝脉应有 `dandao.pill_mist`。
2. 服务端 `SkillRegistry` 已具备三招 resolver，但 `KnownTechniques` 定义源没有三招。
3. 服务端下发 `techniques_snapshot` 时，找不到 definition 的丹道三招不会进入 payload。
4. 客户端技能面板列表没有三招，玩家无法从 UI 选择并绑定。
5. 即使手动构造 `skill_bar_bind` / `skill_bar_cast` 请求，服务端也会在 `technique_definition()` 或 KnownTechniques gate 前拒绝，resolver 不会执行。

## 反方审查记录

第一轮反方：

- 质疑 1：也许三招只是 server-only skeleton，不应视为玩家 bug。
- 结论：未推翻。`plan-dandao-path-v1` 明确把三招列为 P0 基础招，并写出境界解锁、动画、粒子、HUD；`SkillRegistry` 生产初始化已注册 resolver，不是纯文档占位。
- 质疑 2：也许 `abilities_unlocked_at()` 已经把三招同步给玩家。
- 结论：未推翻。该函数只在自身测试中使用；全仓未见它写入 `KnownTechniques`、`techniques_snapshot` 或客户端 technique list。
- 质疑 3：也许已有 A/V 资产，所以不是 bug。
- 结论：需要收窄。粒子贴图 `pill_*` 已存在，不能说所有视觉资产缺失；但玩家技能栏入口仍不可达，且三招缺少实际 VFX player / animation / SFX / HUD / icon 接线。

第二轮反方：

- 质疑 1：也许“唯一玩家客户端入口”过强，可能有技能卷、观摩、传功、quick slot、dev 命令或 NPC 路径绕过。
- 结论：表述收窄为“正常玩家客户端施放这些 resolver 的入口是技能栏链路”。技能卷、观摩、传功、dev 命令同样依赖 `technique_definition()`；quick slot 是物品入口，不走 `SkillRegistry`；NPC AI 不是玩家实战 UI 闭环。
- 质疑 2：也许客户端能从别处列出 `abilities_unlocked_at()`。
- 结论：未推翻。客户端列表由 `TechniquesSnapshotHandler` 替换，client/agent 无 `abilities_unlocked_at` 或 `dandao.pill_*` 列表来源。
- 质疑 3：是否重复 #1041 或 `plan-dandao-mutation-gameplay-v1`。
- 结论：不重复。#1041 是 technique 叙事反馈桥，未点名丹道基础三招；`plan-dandao-mutation-gameplay-v1` 是骨架且主线是变异衍生技能，基础三招只是背景。

最终裁决：高置信可报告。必须保留限定：这是基础丹道招式的玩家技能栏接入断链，不声称丹道整体不可用，不声称粒子贴图全缺。

## Skeleton Fix Plan

TODO:

- [ ] 在 `KnownTechniques` 的权威定义源中补齐 `dandao.pill_rush`、`dandao.pill_bomb`、`dandao.pill_mist` 的 `TECHNIQUE_IDS` 与 `TECHNIQUE_DEFINITIONS`，display、realm、required meridians、qi/cast/cooldown/range 与 resolver 和 `plan-dandao-path-v1` 对齐。
- [ ] 明确三招如何进入玩家 `KnownTechniques`：境界解锁同步、技能卷学习、导师/观摩，或其它既有学习路径只能选一个权威入口，不能让 `abilities_unlocked_at()` 继续孤岛化。
- [ ] 补三招 icon：`SkillDef` / `TechniqueDefinition.icon_texture` / schema / client skill icon 查图路径一致；Codex 不能生成最终 PNG，若缺新 icon 需标注需 `/gen-image item` 生成。
- [ ] 补三招玩家侧 A/V/HUD：每招独立 animation、VFX event id、VFX player、SFX/audio recipe、HUD 反馈；不能共用通用 cast 条当作完成。
- [ ] PlayerAnimator 动画实现必须遵守四大坑：循环轴补尾帧、不滥用 leg pitch、不误用 `body.*` 做上半身扭转、`bend` 依赖 bendy-lib。
- [ ] 确认 `skillbar_config` 与 `techniques_snapshot` 能从补齐后的 definition 派生出三招 display/cast/cooldown/icon，并且 `skill_bar_bind` / `skill_bar_cast` 不再在 definition gate 前拒绝。
- [ ] 保持丹道 resolver 的真元守恒路径不退化：施放扣除仍必须走现有 `drain_dandao_qi` / `qi_release_to_zone` / `QiTransfer` 审计。

## 验收测试计划

- [ ] server 单测：`technique_definition("dandao.pill_rush" / "dandao.pill_bomb" / "dandao.pill_mist")` 均返回定义，且 `TECHNIQUE_IDS`、`TECHNIQUE_DEFINITIONS`、`KnownTechniques::dev_default()` 三者一致。
- [ ] server 单测：满足境界与经脉条件时，三招可以进入 `KnownTechniques`；不满足境界、经脉断裂、未学习时分别拒绝。
- [ ] network 单测：带三招 `KnownTechniques` 的玩家收到 `techniques_snapshot`，payload 包含三招；缺定义条目不再被 filter 掉。
- [ ] network 单测：`skill_bar_bind` 可绑定三招，`skill_bar_cast` 能进入对应 resolver 并设置 cooldown；未知/未学习仍按现有 gate 拒绝。
- [ ] client 单测：`TechniquesSnapshotHandler` 收到三招后，`TechniquesTabPanel.bindTechniqueToSlot()` 能发送 `skill_bar_bind`，`SkillBarConfigHandler` 能显示三招 icon/cast/cooldown。
- [ ] A/V 回归：远处观察可区分服丹急行、投丹、丹雾；每招有独立 animation、粒子、音效、HUD 反馈和热栏 icon。
- [ ] 视觉资产回归：缺最终 PNG 时必须显式阻塞并列出 `/gen-image item` 清单，不能以手绘或空白资源糊弄。

## 风险

- 把三招加入 `TECHNIQUE_DEFINITIONS` 会把原先不可达 resolver 暴露给玩家，需要重新核对数值、冷却、range、目标选择和真元守恒测试。
- 如果同时存在境界自动解锁与技能卷学习，可能造成重复学习或 UI 列表状态混乱；必须选定一个权威获得路径。
- 三招部分粒子贴图已存在，但 event id / player / animation / audio / HUD 未闭环；修复时容易只补定义表，让技能“能按但无差异化反馈”。
- icon 资源生成不应由 Codex 伪造；若最终资源缺失，需要把生成任务显式交给 `/gen-image item` 流程。
- 丹道计划早期文档里的经脉缩写与当前 `MeridianId` 命名可能有映射差异，补 definition 时要以现有 server 常量为准，避免再引入经脉 gate 漂移。
