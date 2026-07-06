# Bong · BugHunt · 毒蛊侵染 A/V 错接骨架

> 状态：Skeleton Plan。来源：BugHunt 线程 D3（client-combat 第三轮）。本文件只记录高置信 bug 与修复骨架，不消费、不归档、不改实际代码。

## Bug 摘要

`dugu.penetrate`（侵染）在服务端运行时发出的动画/音效与自身 visual metadata 不一致：`PenetrateChainEvent` 使用 `visual_for(DuguSkillId::Penetrate)`，该 metadata pin 为 `bong:dugu_needle_throw` + `dugu_needle_hiss` + HUD hint `侵染`；但 `apply_penetrate()` 随后直接播放 `bong:dugu_pointing_curse` 与 `dugu_curse_cackle`，这两者是倒蚀/诅咒语义。

这不是 #997 的 HUD 字段缺失问题。#997 记录 `dugu_v2_skill_cast` 只更新通用 `revealRisk`，HUD 丢失招式区分；本 bug 是 runtime A/V 已经发错，即使 HUD 后续修好，侵染仍会播放错误动作和声音。

## 实际游玩体验影响

玩家释放侵染时，画面会出现化虚级远指/诅咒式动作，声音也会变成倒蚀式 cackle，而不是侵染应有的针掷/针嘶反馈。实战中玩家会把“二次侵染联级”误读成“倒蚀/诅咒类高阶动作”，尤其在毒渍粒子被遮挡、多人混战或连招判断时，会误判当前技能阶段、目标是否已被联级触发，以及下一步是否该继续叠毒或准备倒蚀。

## 证据定位

- `server/src/combat/dugu_v2/skills.rs:389`：`PenetrateChainEvent` 使用 `visual: visual_for(DuguSkillId::Penetrate)`。
- `server/src/combat/dugu_v2/skills.rs:405`：同一 `apply_penetrate()` 分支随后直接 `emit_audio(world, "dugu_curse_cackle", pos)` 与 `emit_anim(world, caster, "bong:dugu_pointing_curse")`。
- `server/src/combat/dugu_v2/skills.rs:1003`：`visual_for(DuguSkillId::Penetrate)` pin 为 `animation_id="bong:dugu_needle_throw"`、`sound_recipe_id="dugu_needle_hiss"`、`hud_hint="侵染"`。
- `docs/finished_plans/plan-dugu-v2.md:328`：规格允许侵染复用蚀针掷针姿态 + `taint_pulse`，不是复用倒蚀远指。
- `docs/finished_plans/plan-dugu-v2.md:326`、`:332`：`dugu_pointing_curse` 与 `dugu_curse_cackle` 是倒蚀化虚远指/远程嘲笑语义。
- `server/src/network/vfx_animation_trigger.rs:967`：Dugu v2 的 anim/audio 已由 `skills.rs` 内联发出，后续 trigger 只补粒子，不会覆盖这次错接。

## 触发路径

1. 玩家将 `dugu.penetrate` 绑定到 1-9 技能栏并对已有 `TaintMark` 的目标施放。
2. server `resolve_dugu_v2_skill(..., DuguSkillId::Penetrate)` 进入 `apply_penetrate()`。
3. `apply_penetrate()` 发送 `PenetrateChainEvent`，事件 metadata 仍声明为侵染 visual。
4. 同函数直接发送 `dugu_curse_cackle` 与 `bong:dugu_pointing_curse`。
5. `bong:audio/play` 与 `bong:vfx_event` 到 client 后按实际 payload 播放，玩家看到/听到错接的倒蚀语义 A/V。

## 反方审查记录

- 第一轮：反方确认 bug 真实，但要求收窄表述。结论为“不是完全伪装成 Reverse”，因为 Reverse runtime 声音是 `dugu_poison_signature`，粒子也不同；但侵染确实播放了不属于自身 metadata 的远指动画和 cackle 音效。
- 第二轮：反方尝试以“plan 允许复用”“#997 已覆盖”“只是 metadata 文档层”推翻，均失败。`plan-dugu-v2` 明确侵染复用蚀针掷针姿态；#997 只覆盖 HUD 字段；`emit_audio()`/`emit_anim()` 会进入真实 S2C 播放链。

结论：通过两轮对抗，成立为独立 runtime A/V 错接。

## Skeleton Fix Plan

- [ ] 在 `apply_penetrate()` 内消除硬编码 `dugu_curse_cackle` / `bong:dugu_pointing_curse`，改为从 `visual_for(DuguSkillId::Penetrate)` 读取 `sound_recipe_id` / `animation_id`，或提取统一 `emit_dugu_skill_av(world, caster, pos, skill)` helper。
- [ ] 补 server pin 测试：侵染成功施放时发出的 `PlaySoundRecipeRequest.recipe_id == "dugu_needle_hiss"`，`VfxEventPayloadV1::PlayAnim.anim_id == "bong:dugu_needle_throw"`。
- [ ] 补反向测试：倒蚀仍保留自己的远指/倒蚀音效语义，不被侵染修复误改。
- [ ] 检查 Shroud/SelfCure/Reverse 是否存在同类 “event visual metadata 与 direct emit 漂移”。

## 验收测试计划

- server：`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test combat::dugu_v2`
- server：新增定向测试覆盖 `dugu.penetrate` runtime audio/anim payload 与 `visual_for(Penetrate)` 一致。
- client：若修复涉及 client 解析或资源，按 JDK 17 约定在 `client/` 跑 `./gradlew test build`；若只改 server direct emit，可不触碰 client。
- 手工/联调：施放蚀针、侵染、倒蚀三招，确认侵染是针掷/针嘶 + `dugu_taint_pulse`，倒蚀仍是远指/爆发语义。

## 风险

- 侵染与蚀针复用动画/音效是规格允许的，但必须靠 HUD/粒子/目标状态区分；不要误判为“侵染需要新动画资源”。
- 修复时不要改变 `PenetrateChainEvent.returned_zone_qi` 与 qi ledger 逻辑，本 bug 只碰 A/V 发射路径。
- 若抽 helper，要避免影响 Dugu v1 基础两招 `dugu_cast` / `dugu_poison_cast`。
