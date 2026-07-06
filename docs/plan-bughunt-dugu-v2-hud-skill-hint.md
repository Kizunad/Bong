# plan-bughunt-dugu-v2-hud-skill-hint（skeleton）

> BugHunt D2 / client-combat 第二轮。仅记录高置信 bug，不消费、不归档、不修改代码或资源。

## Bug 摘要

毒蛊 v2 五招的 server visual metadata 已为每招钉住独立 `hud_hint` 与 `icon_texture`，但真正发往客户端 HUD 的 `dugu_v2_skill_cast` S2C payload 只包含 `kind/caster/target/taint_tier/reveal_probability/tick`。客户端 `DuguV2ServerDataHandler` 读取 `kind` 后只用于日志，`DuguV2HudStateStore` 和 `DuguV2HudPlanner` 也没有保存或渲染招式名、HUD 文案、图标。

结果是蚀针、侵染这类复用同一动画与粒子的毒蛊 v2 招式，在 HUD 上都只能表现为通用的“暴露 xx%”，无法按 `plan-dugu-v2` 要求用 HUD 文案区分当前释放的是哪一招。server pin 的五个 `bong:textures/gui/skill/dugu_*.png` 图标路径也没有对应客户端资源，图标链未落地。

## 实际游玩体验影响

玩家释放 `dugu.eclipse` 和 `dugu.penetrate` 时，两者都走 `bong:dugu_needle_throw` 动画与 `bong:dugu_taint_pulse` 粒子。当前客户端 HUD 不显示“蚀针 / 侵染”等招式提示，也不显示招式图标，只剩同一类暴露风险条。实战中玩家很难从 HUD 确认刚才触发的是首次注入还是二次侵染，尤其在连招、多人目标、粒子被遮挡时会误判技能阶段和后续决策。

这不是 #987 的“server 拒绝施放但 client 假施法完成”，也不是 #934 的“毒蛊 HUD 跨 session 残留”。本问题发生在 `dugu_v2_skill_cast` 已经成功到达客户端之后：HUD 反馈维度缺少每招区分。

## 证据定位

- `server/src/combat/dugu_v2/events.rs:45`：`DuguSkillVisual` 包含 `animation_id / particle_id / sound_recipe_id / hud_hint / icon_texture`。
- `server/src/combat/dugu_v2/skills.rs:987`：`visual_for()` 为五招配置独立 `hud_hint` 和 `bong:textures/gui/skill/dugu_*.png`。
- `server/src/combat/dugu_v2/tests.rs:501`：server 测试 pin 五招 visual；`dugu_visual_ids_exhaustive_all_five_skills_have_unique_hud_hint` 要求 HUD hint 跨招唯一。
- `server/src/schema/server_data.rs:791`：`DuguV2HudSkillCastV1` 只有 `kind/caster/target/taint_tier/reveal_probability/tick`，没有 `hud_hint/icon_texture`。
- `server/src/network/dugu_v2_event_bridge.rs:51`：S2C 给 caster 的 `DuguV2SkillCast` 只填 `kind`、层级、暴露概率和 tick；Redis/agent payload 另带 visual 字段，但不进入 client HUD。
- `client/src/main/java/com/bong/client/combat/handler/DuguV2ServerDataHandler.java:41`：`dugu_v2_skill_cast` 分支只把 `reveal_probability` 写入 store；`kind` 只出现在 log 文本中。
- `client/src/main/java/com/bong/client/hud/DuguV2HudStateStore.java:5`：HUD state 没有 `kind/hudHint/iconTexture` 字段。
- `client/src/main/java/com/bong/client/hud/DuguV2HudPlanner.java:36`：planner 渲染的是通用 `暴露 %.0f%%`，没有按 `kind` 渲染招式文案或 icon。
- `docs/finished_plans/plan-dugu-v2.md:328`：侵染复用蚀针姿态和 taint pulse 时，规格明确要求“HUD 文案与 payload skill 区分”。
- `docs/finished_plans/plan-combat-skill-feedback-bridges-v1.md:363`：倒蚀无专属 HUD 闪烁是设计遗留，不作为本 bug 主证。

## 触发路径

1. 玩家施放 `dugu.eclipse` 或 `dugu.penetrate`。
2. server 生成 Dugu v2 cast event，并通过 `dugu_v2_event_bridge` 给 caster 发送 `ServerDataPayloadV1::DuguV2SkillCast`。
3. S2C payload 只携带 `kind` 与 `reveal_probability`，未携带 server visual 的 `hud_hint/icon_texture`。
4. client `DuguV2ServerDataHandler` 处理 payload，只更新 `DuguV2HudStateStore.revealRisk`。
5. `DuguV2HudPlanner` 只渲染通用暴露条；蚀针/侵染无法在 HUD 上被区分。

## 反方审查记录

- Round 1：反方确认 Dugu v2 并非完全缺 A/V，动画、粒子、音效已有独立链路；但 S2C HUD contract 没有 `hud_hint/icon_texture`，client 也不保存/渲染 `kind`，候选成立但需收窄。
- Round 2：反方继续挑战是否只是 narration 字段、是否 QuickBar 才受影响、是否与 #934/#987/#976 重复。裁决：`hud_hint` 不被 agent runtime 消费，QuickBar 缺图不是主证；最小 bug 应聚焦“蚀针/侵染 HUD 丢失招式区分”。通过，建议提交 skeleton。

## Skeleton Fix Plan

1. 扩展 `DuguV2HudSkillCastV1` / proto / client bridge，使 S2C HUD payload 至少携带可渲染的 `hud_hint`，必要时携带 `icon_texture`。
2. `dugu_v2_event_bridge` 从 `event.visual` 填充 HUD hint/icon，不只填 `kind`。
3. 扩展 `DuguV2HudStateStore`，保存最近一次 skill cast 的 `kind/hudHint/iconTexture/expiryMs`，避免永久残留。
4. `DuguV2HudPlanner` 在 reveal bar 旁或同层渲染短暂招式提示；蚀针/侵染必须能肉眼区分。
5. 图标资源：server 当前 pin 的 `dugu_eclipse/self_cure/penetrate/shroud/reverse.png` 客户端资源不存在。消费本 plan 时需按仓库图标流程生成 5 个 PNG（Codex 不直接跑 `/gen-image`），或改为明确存在的资源路径并补资源存在性 pin。

## 验收测试计划

- server：`DuguV2HudSkillCastV1` 序列化与 proto roundtrip pin `hud_hint/icon_texture` 不丢。
- server：`dugu_v2_event_bridge` 对 eclipse/penetrate 的 S2C payload 分别含 “蚀针 / 侵染”。
- client：`DuguV2ServerDataHandlerTest` 验证 `kind/hudHint/iconTexture` 写入 store，且不覆盖 self-cure/shroud/qi-decay 维度。
- client：`DuguV2HudPlannerTest` 验证 eclipse 与 penetrate 产出不同 HUD 文案或 icon command。
- client：资源 pin 覆盖五个 Dugu v2 icon 路径；若选择 fallback 路径，也要测试路径存在。
- 命令：`cd client && JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64 ./gradlew test build`；server schema/proto 改动时按 server 栈运行 `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`。

## 风险

- 需要避免重引入 #934：新增最近一次招式 HUD 状态必须有过期窗口，并在断线/重连清理。
- S2C schema/proto 改动会影响 agent/schema 生成物，消费时必须同步更新 dist/generated。
- 图标补齐涉及视觉资产流程；不能用手绘占位糊弄，应按仓库 `/gen-image` 图标纪律处理。
- 不应把倒蚀无 reveal bar 写成 bug；`plan-combat-skill-feedback-bridges-v1` 已说明倒蚀无专属 HUD 闪烁是设计边界。
