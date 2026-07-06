# BugHunt: SkillConfig 缺失拒绝未推 cast_sync

## Bug 摘要

陈旧或持久化的 skillbar 槽位可以仍绑定 `zhenmai.sever_chain`，但对应玩家的 `SkillConfigStore` 中缺少该招配置。玩家按数字键时 client 会先本地预测施法条，server 随后在 `validate_skill_config_before_cast` fail-close 拒绝 cast，却只 `warn + return`，没有推 `CastSyncV1{phase=Idle,outcome=Reject*}`。结果 client 无法把预测施法纠正为拒绝态，也不会显示 finished plan 要求的“未选定经脉 / 攻击类型”HUD 红字。

## 实际游玩体验影响

玩家看到 1-9 槽里仍有绝脉断链图标，按键后 cast bar 正常起跑并在约 400ms 后显示完成，但服务端没有插入 `Casting`、没有触发招式效果、没有扣代价，也没有任何红字解释。战斗中这会表现为“技能好像放出去了但实际没生效”，尤其在重连、旧存档迁移、配置被服务端重新校验丢弃后，玩家无法从 HUD 判断是配置缺失而不是距离、目标、冷却或网络问题。

## 证据定位

- `client/src/main/java/com/bong/client/combat/SkillBarKeyRouter.java:38` `route(...)` 从 `SkillBarStore.snapshot()` 取槽位，不检查 `SkillConfigStore`；`SkillBarKeyRouter.java:59` 先 `CastStateStore.beginSkillBarCast(...)`，`SkillBarKeyRouter.java:60` 才发送 C2S。
- `client/src/main/java/com/bong/client/combat/CastStateStore.java:76` `tick(...)` 在没有 server 纠偏时会把 `CASTING` 自动转成 `COMPLETE`，再于 `CastStateStore.java:87` 后回到 idle。
- `server/src/network/client_request_handler.rs:9551` `handle_skill_bar_cast` 调 `validate_skill_config_before_cast(...)`；失败分支到 `client_request_handler.rs:9557` 直接 `return`，没有调用 `push_cast_sync`。
- `server/src/network/client_request_handler.rs:9660` `validate_skill_config_before_cast(...)` 在 schema 存在但 store 无配置时返回 `MissingRequiredField("config")`。
- `server/src/network/client_request_handler.rs:7359` `skill_bar_cast_requires_config_for_schema_fixture` 已构造 `known + bound zhenmai.sever_chain + missing config`，第一次 cast 断言没有 `Casting`；随后写入 config 再 cast 成功。该测试证明服务端状态可达，但没有断言拒绝时的 `CastSyncV1`。
- `server/src/player/state.rs:57` `PlayerUiPrefs` 独立持久化 `skill_bar` 与 `skill_configs`；`server/src/player/state.rs:102` `skill_bar_bindings(...)` 还原 `SkillSlotPersist::Skill` 时不校验对应 config。
- `server/src/player/mod.rs:227` 登录恢复分别还原 skillbar 与 skill configs；`server/src/player/mod.rs:237` `replace_player_configs(...)` 只处理配置，不清理已还原的 skillbar 槽。
- `server/src/network/skillbar_config_emit.rs:66` 下发 skillbar 技能槽时只要求 `technique_definition(skill_id)` 存在，不校验 `SkillConfigStore`；`client/src/main/java/com/bong/client/network/SkillBarConfigHandler.java:55` 直接替换 `SkillBarStore`。
- 对照正常拒绝：`server/src/network/client_request_handler.rs:9579` 经脉门控拒绝会推 `CastSyncV1{phase=Idle,outcome=MeridianGated}`；`client_request_handler.rs:9631` resolver 拒绝会走 `push_skill_cast_rejected_sync(...)`；`client_request_handler.rs:10029` 注释明确 Idle+Reject 用于 client 警示 HUD。
- `client/src/main/java/com/bong/client/network/CastSyncHandler.java:37` 收到 idle + non-none outcome 才会合成拒绝态；`CastSyncHandler.java:64` 才会发布 warning。
- `docs/finished_plans/plan-zhenmai-v2.md:256` 到 `docs/finished_plans/plan-zhenmai-v2.md:258` 明确未配置保护应 “cast 失败 + HUD 红字未选定经脉 / 攻击类型”。

## 触发路径

1. 玩家已有或恢复出一个 skillbar 槽位：`SkillSlot::Skill { skill_id: "zhenmai.sever_chain" }`。
2. 同一玩家的 `skill_configs` 中没有 `zhenmai.sever_chain`，或配置在 `replace_player_configs` 校验时被丢弃。
3. server 登录恢复后仍把该 skillbar 槽位通过 `skillbar_config` 下发给 client。
4. client 按 1-9 数字键，`SkillBarKeyRouter` 先启动本地预测施法条，再发送 `SkillBarCast`。
5. server 因缺 `SkillConfig` 拒绝，但没有发 `cast_sync`。
6. client 预测条自然 complete，右侧事件流没有“未选定经脉 / 攻击类型”警示。

## 反方审查记录

第一轮反方质疑：
- 新 UI 首次绑定会被 `TechniquesTabPanel.bindTechniqueToSlot(...)` 的 `lockReason(...)` 拦住，不能主张“玩家正常拖拽未配置技能后施放”。
- `zhenmai.sever_chain` 的 `cast_ticks=8`，假条约 400ms，体验严重度不能描述成长期卡死。
- `SkillConfigSchemas` 缺失是资源异常，只能做旁证，不宜作为主复现。

补证与让步：
- 让步：不再主张新 UI 首次绑定路径；主 bug 收窄为陈旧/持久化 skillbar 绑定。
- 补证：`PlayerUiPrefs.skill_bar` 与 `skill_configs` 独立持久化，登录恢复和 `skillbar_config_emit` 都不会用配置状态过滤槽位。
- 补证：已有 server 单测构造了“已学会、已绑槽、无配置”的 `zhenmai.sever_chain` 并确认服务端拒绝施放。
- 补证：client 只有收到 `cast_sync` 才会把本地预测纠正为拒绝态并发布 warning；`skill_config_snapshot` 只同步配置镜像，不会打断 cast bar。

最终裁决：
- 反方通过。收窄后的链路闭合：陈旧/迁移状态可达，按键侧无额外配置校验，server 早退无 `cast_sync`，client 无其它 chat/toast/server_data 替代纠偏。
- 保留项：新 UI 首次绑定保护存在；假条时间较短。但“服务端未施放，客户端显示完成且无红字”仍是实际战斗反馈 bug，且与 finished plan 的未配置保护预期相反。

## Skeleton Fix Plan

- [ ] 在 `CastOutcomeV1` / proto / agent schema / client `CastOutcome` 中补一个明确的配置拒绝 outcome，例如 `RejectSkillConfigMissing`，并给 client warningText 映射“未选定经脉 / 攻击类型”或更通用的“技能配置未完成”。
- [ ] 在 `handle_skill_bar_cast` 的 `validate_skill_config_before_cast` 错误分支推 `CastSyncV1{phase=Idle,duration_ms=0,outcome=RejectSkillConfigMissing}`，语义对齐经脉门控和 resolver 拒绝。
- [ ] 覆盖 `MissingRequiredField("config")`、字段缺失、字段非法、`SkillConfigSchemas` / store 缺失的 fail-close 路径；资源缺失可使用同一通用配置拒绝文案，日志保留具体 reason。
- [ ] 保持服务端结果不变：拒绝时不插入 `Casting`、不扣 qi、不触发绝脉断链效果、不写 cooldown。
- [ ] 视风险决定是否在 `skillbar_config_emit` 中标注或过滤配置缺失技能槽；若选择过滤，需同时处理玩家失去可见入口的问题，避免只靠隐藏槽位掩盖 server 拒绝反馈。
- [ ] 更新 client cast warning 折叠 source_tag 或文案，使连续按键不会刷屏，但每次拒绝都能纠正预测施法条。

## 验收测试计划

- server：补 `skill_bar_cast_requires_config_for_schema_fixture` 的回归断言，缺 config 第一次 cast 后 `collect_cast_syncs(...)` 应包含 `phase=Idle` 且 outcome 为配置拒绝；仍断言没有 `Casting`。
- server：补字段缺失/非法配置样例，确认均推同类拒绝 `CastSyncV1`。
- server：补资源缺失 fail-close 路径测试，确认拒绝有同步、无 `Casting`，日志 reason 不影响 wire outcome。
- client：补 `CastSyncHandler` 单测，idle + 配置拒绝 outcome 会让 `CastStateStore` 进入 interrupt/fade 语义并向 `UnifiedEventStore` 发布中文警示。
- client：补 `SkillBarKeyRouter` 状态机测试，已存在 skillbar entry 且无后续 `cast_sync` 会预测；收到配置拒绝 `cast_sync` 后预测条被立刻覆盖为拒绝态。
- 联调：构造旧 prefs：slot 0 为 `zhenmai.sever_chain`、`skill_configs` 空；登录后按 1，验收 HUD 出“技能配置未完成/未选定经脉或攻击类型”，且没有绝脉断链 gameplay 效果。

## 风险

- schema/proto/client enum 扩展需要同步生成产物，并按仓库约定重建 `@bong/schema` dist，避免 agent 引用旧构建产物。
- 若复用现有 `RejectInvalidTarget` 可少改 schema，但会给玩家错误文案“目标无效”，不满足 zhenmai-v2 未配置保护语义。
- 如果同时清理或隐藏陈旧槽位，可能让玩家失去发现配置丢失的入口；更稳妥的是先保证拒绝同步和警示，再单独评估槽位修复策略。
- 该修复触及通用 skillbar cast 拒绝链路，需要回归经脉门控、resolver 拒绝、cooldown、同槽重复施放和本地预测覆盖。
