# plan-refactor-cast-av-contract-v1 — 施法同步/技能栏/AV 单一事实源契约（重构轨 R9）

> 所属总纲：`plan-refactor-master-v1.md`。一句话：cast_sync 事件补全"来源+目标+阶段"契约、技能栏定义源统一、每招 AV（动画/粒子/音效/HUD/图标）注册收敛为单一事实源并加"注册即校验"——AV 双源重复播放/错接/缺失、skillbar 断链整簇（12+ 份 plan）收口。

## 现状证据（2026-07-27 侦察）

- cast_sync 缺来源字段致 HUD 错位（skillbar-cast-source-drift）；SkillConfig 缺失时拒绝不推同步事件（skillconfig-castsync）；#1249 review 揭出参数表 5 招里 3 招服务端从不产生权威 CASTING（死注册项）——注册表与运行时脱节无校验。
- AV 双源：baomai-v3 同 cast 双 A/V 发射源、tuike-v2 重复播放、dugu 侵染错接倒蚀动画——emit 点分散且无"每招唯一 AV 绑定"约束。
- skillbar 断链：丹道三招未接技能栏定义源、dugu-v2 缺 HUD 提示图标、真脉断脉标记误显增幅、五招缺 PlayerAnimator JSON 不播放。
- 停止语义缺失：逃劫后抱臂动画不停止（tribulation-fled-brace-stop）——cast 生命周期没有权威 STOP。
- 基线：#1287（冷却按 skill_id 全局重构，14 resolver + cast_emit/skillbar_config_emit 大改）必须先 merge。

## 接入面

- **进料**：`SkillRegistry`/`SkillDef`（既有）；R5 qi 访问器、R6 emit builder、R2 store 生命周期是 production activation 的接缝输入，不是 contract-first 的启动门。
- **出料**：cast_sync/S2C AV 事件 → client `VfxBootstrap`/`BongAnimationRegistry`/audio recipe/HUD/SkillIcon 单点绑定。
- **共享类型**：`SkillAvBinding { anim, vfx_event, audio_recipe, hud_hint, icon }` 每招一条，注册即校验（缺任何一项 = 启动 fail-fast，对齐「招式 A/V 差异化」红线）；cast 生命周期枚举补 STOP/INTERRUPT 权威事件。
- **worldview/AV 锚点**：每招独立可辨五件套是 CLAUDE.md 红线；audio 沿用 Pattern A（cast_center 施法快照，不读实时 Position）。

## 阶段

- ⬜ P0 设计收口 + 吸收清单验真：全部已注册招式普查（server 权威可达性 × client AV 五件套齐备度矩阵，#1249 已给出方法先例）；冻结 `SkillAvBinding` 与 cast_sync 契约增量。
- ⬜ P1 契约落地：contract-first 先补 cast_sync 的 source/target/phase 字段与 STOP/INTERRUPT 权威事件、`SkillAvBinding` 注册表及 fail-fast 校验；**这些 artifacts 的首次提交必须同时携 pin suite**：TypeBox 正例覆盖每个 phase/discriminant，反例覆盖 source/target/phase 任一缺失与 invalid/unknown phase；client reducer/state-machine 覆盖每条合法转换、非法转换拒绝及 STOP/INTERRUPT 终止路径；registry 覆盖五件套齐备成功与任一缺项 fail-fast。不得把这些 pins 延后到 P4 bot/e2e。production activation 仅按总纲 §3/§4.1 的跨轨顺序、ownership 与 atomicity invariants 放行，具体接缝与验收由 owner track plans 定义。
- ⬜ P2 修复批次 A：双源去重（baomai-v3/tuike-v2）、错接纠正（dugu penetrate）、停止语义接线（tribulation brace/打坐腿 pitch 红线修正）。
- ⬜ P3 修复批次 B：skillbar 定义源统一（丹道三招接入）、HUD 提示/图标补齐（dugu-v2、zhenmai sever 标记语义修正）、缺失动画 JSON 补齐（woliu voidpath 五招，遵守 PlayerAnimator 四大坑）。
- ⬜ P4 bot 验收 + 吸收 plan 批量归档。

## 吸收清单（短名省略 plan-bughunt- 前缀与 -v1 后缀）

active：dugu-v2-technique-definition-gap、woliu-voidpath-missing-animations。
skeleton：dandao-basic-skillbar-bridge、dugu-v2-hud-skill-hint、skillbar-cast-source-drift、skillconfig-castsync、zhenmai-sever-marker-hud、baomai-v3-av-double-source、dugu-penetrate-av-mismatch、meditate-sit-leg-pitch、tribulation-fled-brace-stop、tuike-v2-duplicate-av、combat-event-juice-runtime-bridge-gap（cast/combat 事件字段补全部分）。
**不吸收**：`plan-fpv-cast-av-v1`（active feature，有实质进度，独立收尾；本轨 P1 契约冻结前与其对齐字段，避免撞车）。

## 文件所有权与边界

- 独占：server `combat/` 的 cast/AV emit 点、`skill/` 注册表、`network/cast_emit.rs`；client `combat/` handler/store 的 cast 部分、`VfxBootstrap`/动画注册的绑定结构。
- 不碰：qi 扣费语义（R5 访问器，本轨消费）；emit 传输层（R6 builder，本轨消费）；FPV 手臂动画（fpv-cast-av plan 域）。
- **依赖**：#1287、#1249 是基线；P0 与 P1 contract-first 可独立开工，不等待其他 track 的 production artifacts。production activation/cutover 只按总纲 §3 Wave 与 §4.1 的 ownership/atomicity invariants 放行；具体接缝、artifact 和验收由各 owner track plan 定义。P0 普查可先行。

## bot 验收场景

1. `cast_registry_reachability`：dev 命令枚举全部注册招式→逐招触发→断言每招产生权威 CASTING 与对应 cast_sync（死注册项清零）。
2. `cast_stop_semantics`：打断/逃跑/断线三种终止→断言 STOP 事件下发（P6 protobuf 深断言配合）。
3. `cast_av_uniqueness`：同 cast 的 vfx/audio 事件计数 == 1（双源去重回归）。
4. AV 视听差异化人工回归（远处可辨"X 不是 Y"）按 CLAUDE.md 红线走 runClient 人工单。

## 开放问题（pre-P0 收口）

1. `SkillAvBinding` fail-fast 对占位资源的容忍度（[BLOCKED: 需 /gen-image] 的招允许显式 placeholder 标记，禁止静默缺失）。
2. cast_sync 契约增量与 fpv-cast-av-v1 P4/P5 的字段对齐窗口。
