# plan-bughunt-r8-findings-v1（骨架）

> **骨架（草案）**。一句话主题：代码库自检 bug-hunt **round8**（fresh origin/main worktree ROOT，角度：modifier/effect 消费层系统审计 · 数值/公式偏离正典 · 热路径 panic · NPC AI · TSY 深挖）确认的 **11 个新真 bug**——**modifier-orphan 模式大爆发**（baomai-v4 三子簇 7 字段 + InsightModifiers 再 12+ 字段 + jump_height 全"写入端齐全消费端断裂"）+ **距离衰减公式偏离 worldview §四正典 1.84×** + NPC 防御生命周期门缺失 + TSY 塌缩生命周期泄漏。已对 r1-r7 去重，全部 real-on-main。

> 立项动机：round8 modifier 审计角度穷尽扫"写入但永不读取"——证实这是**项目级系统缺口**（r4 status-effect + r6 ContaminationBoost + r7 InsightModifiers 5 字段 + 本轮 baomai-v4 7 字段 + InsightModifiers 又 12+ 字段 + jump_height，累计 25+ 字段写入端齐全但消费端断裂）。12 候选 → **11 REAL / 1 NOT_REAL**（dismiss：bone_coin 衰减 finding 数学错误，实际匹配 worldview §九 ~20%）。

## 阶段总览（按主题分组，逐项独立可修）

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 🔶 modifier/derived-attr orphan 大簇（baomai-v4 + InsightModifiers + jump_height） | plan_skeleton | ⬜ |
| P1 | 🔶 距离衰减公式偏离 worldview §四正典（0.03 → 10格 73.7% vs 应 40%） | plan_skeleton | ⬜ |
| P2 | NPC 防御 scorer/action 缺生命周期门控（NearDeath 仍格挡） | fix_pr | ⬜ |
| P3 | TSY 塌缩生命周期泄漏（TsyPresence 不清 + ghost entity + stop_fuya 广播） | fix_pr + plan_skeleton | ⬜ |

## P0 — 🔶 modifier/derived-attr orphan 大簇（顶级系统主题）

> 延续 r4（QiCapPermMinus/QiRegenBoost）+ r6（ContaminationBoost）+ r7（InsightModifiers 五字段）的"写入端齐全、消费端断裂"模式。本轮再添 **baomai-v4 7 字段 + InsightModifiers 12+ 字段 + jump_height**——这已是**项目级架构缺口**，强烈建议**统一一个"modifier/effect 消费层接入" plan** 一次性把所有 DerivedAttrs/InsightModifiers/StatusEffect 字段接到对应消费系统，而非逐个补（逐个补必再漏）。

- **#1 major**：baomai-v4 疤纹回路 `scar_circuit_derive_system`（`combat/baomai_v4/scar_circuit.rs:213-225`，已注册 `mod.rs:47`）写 `combat/components.rs` 的 `reach_bonus`/`qi_regen_multiplier`/`contam_purge_multiplier` 真值，**全仓非测试 grep 零消费**：reach_bonus 仅 `player_attack.rs:82` 用 `weapon_reach()` 常量绕过 DerivedAttrs；qi_regen_multiplier/contam_purge_multiplier 全仓零命中。三阳合流/心肺短路/肝肾交汇被动收益全落空（注：同段 `healing_rate_multiplier` 实有消费 `lifecycle.rs:235`，非孤岛）。
- **#2 major**：baomai-v4 活茧 `iron_cocoon_passive_system`（`iron_cocoon.rs:110-139`，已注册 `mod.rs:55`）按 `IronCocoonStage` 写 `bruise_threshold_multiplier`/`fracture_downgrade_chance`/`cut_pierce_downgrade`，但 `resolve.rs` 伤害链（508 `wound_kind_profile`）纯静态查表零降级逻辑，全仓 grep 三字段仅在 iron_cocoon.rs/components.rs 出现。茧皮钝伤阈值/茧骨骨折降级/茧肉割刺降档三层被动全无效。
- **#3 major**：baomai-v4 茧灵 `iron_cocoon.rs:140` 设 `attrs.scar_forged_flow_bonus=true`，注释（105）明言"实际 flow_rate 修改由 ScarForged 系统处理"——但 **ScarForged 只是 IronCocoonStage enum 变体（27），不存在任何读取该标志改 flow_rate 的系统**；经脉 `sum_rate()` 不感知此标志。茧灵阶段 flow_rate 加成对所有已解锁玩家无效（缺失系统）。
- **#5 major**：`cultivation/insight_apply.rs` `apply_tradeoff_cost`（229-254）把**全部 InsightCost 惩罚字段**（breakthrough_failure_penalty_mul/meridian_heal_slowdown_mul/qi_volatility_add/shock_sensitivity_add/sense_exposure_add/overload_fragility_add/reaction_window_penalty/opposite_color_efficiency_penalty/main_color_efficiency_penalty/chaotic_tolerance_loss）只写 modifier **无任何系统读取** → **代价真实（损失有益选项）但惩罚空洞**；`apply_choice` 的 hunyuan_threshold_mul/chaotic_tolerance_add/overload_tolerance_add 同为孤岛收益。共 12+ 字段（在 r7 五字段之外）。**唯一纠正**：composure_recover_mul 非孤岛（`insight_apply.rs:138` 直改 `cultivation.composure_recover_rate`，composure.rs:33 消费，是冗余影子字段）。
- **#4 minor**：`combat/body_conditioning.rs:97` `apply_guangbo_ticao_bonuses` 乘入 `DerivedAttrs.jump_height_multiplier`（status.rs:159 重置 1.0），但 server `movement/mod.rs` 不读（兄弟字段 move_speed_multiplier 在 `movement/mod.rs:267` 有读），且字段不在 `DerivedAttrsSyncV1`（`combat_hud.rs:178-189`）永不同步 client。广播体操跳跃加成全程无效（需 server→client 同步 + client 跳跃应用）。

## P1 — 🔶 距离衰减公式偏离 worldview §四正典

- **#6 major（plan_skeleton）**：`server/src/combat/decay.rs` 距离衰减 `QI_DECAY_PER_BLOCK=0.03`，`distance.rs:13` 用 `(1-0.03)^10=0.737` → 10 格保留 **73.7%**，但 **worldview §四:339 锚点"10 格 → 环境吃掉 30 点只剩 20 点"= 40% 保留**（偏离 **1.84×**）；§五:412"异变兽骨/灵木 飞 50 格保留 80%"，实际 Beast+Solid@50=`0.98^50≈0.364`（非 0.80）。`plan-anqi-v1 §3.1.D`（335-339）显式校准表 + Q41（341/672）明令"用 Mellow+Mundane 校 10 格 0.40、Solid+Beast 校 50 格 0.80"——**从未落地**。`fits_worldview_anchor_points`（decay.rs:41-54）固化了 0.737/0.494 并用 Relic（非校准目标 Beast）@50，**测试名义拟合 worldview 实则锁定偏离值**；Finish Evidence（691-704）无 Q41 校准记录。后果：远程战斗显著强于"拼刺刀/远程是败家行为"的正典设计意图。修：按 Q41 重校 `QI_DECAY_PER_BLOCK` + 纯度因子使 Mellow+Mundane@10=0.40、Solid+Beast@50=0.80，修正测试锚点（涉共享 qi_physics 衰减常数 + 多调用方，数值校准设计决策）。**worldview 锚点级偏离。**

## P2 — NPC 防御生命周期门控缺失

- **#7 major（fix_pr）**：`server/src/npc/brain/scorers_combat.rs:348-356` `npc_defense_scorer_system` 查询无 `Option<&Lifecycle>`，评分闭包（367-381）只判 nearest_player/blocking_status **无 lifecycle 检查**。对比同文件 chase（111/131-134）、melee（188/207-210）、dash（247/271-274）三 combat scorer 均查 Lifecycle 且 `if state != Alive { score.set(0.0) }`。**defense scorer 是 combat scorer 中唯一模式破口** → NearDeath NPC 仍按 realm 打 0.7 分可赢评分竞赛。修：补 Lifecycle 查询 + 门控 + NearDeath 测试。**局部明确。**
- **#8 major（fix_pr）**：`server/src/npc/brain/actions_combat.rs:448-450` `npc_defense_action_system` 查询无 Lifecycle；Requested 臂（465-486）检查 realm/qi_cost/ParryRecovery 但不检查 lifecycle，Executing 臂（488-504）**无任何 lifecycle 门控直接 emit DefenseIntent**（497-501）。对比 `melee_attack_action_system`（245/253）查 Lifecycle 且 Executing 臂 `if state != Alive { Failure }`。垂死 NPC 开启活跃格挡窗口被 resolve_defense_intents 当真 parry。与 #7 同模式破口配对。修：补 Lifecycle 门控 + 测试。**局部明确。**

## P3 — TSY 塌缩生命周期泄漏

- **#11 major（fix_pr）**：`server/src/world/extract_system.rs:762-770` `on_tsy_collapse_completed` 对持 `TsyPresence` 玩家发 `DeathEvent{cause=tsy_collapsed}` 但**不 `remove::<TsyPresence>`**。全仓仅两处移除 TsyPresence（extract 成功 `extract_system.rs:495`、portal 出关 `tsy_portal.rs:171`）；复活掉落 `apply_death_drop_on_revive`（`inventory/mod.rs:3167`）只读 presence 不移除；death_hooks/on_player_revived 不碰 TSY；无 DeathEvent 消费者移除它 → **塌缩死亡+复活后玩家保留 stale TsyPresence，入场门 `Without<TsyPresence>` 致永久 TSY 锁出**。修：on_tsy_collapse_completed（或 DeathEvent 消费/revive 路径）remove TsyPresence。**局部明确，比报告更严重。**
- **#10 major（plan_skeleton）**：`server/src/world/tsy_lifecycle.rs:665-668` `tsy_collapse_completed_cleanup` 的 4b 循环 `if !matches!(archetype, Daoxiang) { continue }` **显式 skip 所有非 Daoxiang 变体**。Zhinian/Fuya/SkullFiend/Sentinel spawn 时挂 `TsyHostileMarker{family_id}`（tsy_hostile.rs:914/968/1006）在 layers.tsy 维度层，cleanup 仅 despawn Daoxiang+corpse + 移 zone + 标 Dead，**无系统按 TsyHostileMarker/family despawn 这些 NPC** → Zhinian/Fuya/SkullFiend/Sentinel 成 **ghost entity**（AI scorer/action 继续 tick、family_id 指向已 Dead family）。（注：drain 已被 zone 门控，`tsy_drain_tick:124` zone 移除后 None skip，不再抽真元——finding 守恒论点夸大；但结构性 ghost 泄漏属实。）修：cleanup 按 TsyHostileMarker/family_id despawn 全部变体（喷出/despawn/转移策略，与 #11 相关）。**需设计决策。**
- **#12 minor（fix_pr）**：`stop_fuya_pressure_hum_audio_on_death_system` 对**任意实体 DeathEvent 以 All 广播** Fuya 停音 → 大量无效 `StopSoundRecipeRequest`。play 受 FuyaAura 门控、stop 不门控的**不对称**。修：stop 也按 Fuya/FuyaAura 门控（与 play 对称）。**局部明确。**

## §N 开放问题

1. **P0 是否立"modifier/effect 消费层接入"总 plan**：累计 r4+r6+r7+r8 共 25+ 字段"写入端齐全消费端断裂"，强烈建议统一一个 plan 系统性接入（DerivedAttrs/InsightModifiers/StatusEffect → 消费系统），并加一条"新增 modifier 字段必须同时写消费读取"的回归测试/lint 防再漏。逐个 fix_pr 会反复漏。
2. **#6 距离衰减重校**：按 plan-anqi-v1 Q41 校准（Mellow+Mundane@10=0.40、Solid+Beast@50=0.80）反解 QI_DECAY_PER_BLOCK + 纯度因子；涉共享 qi_physics 常数，需确认所有调用方（combat/anqi/needle）一致。是否并入 r1 `plan-qi-conservation-leaks-v1` 或独立数值校准 plan。
3. #7/#8 NPC 防御门控 + #11/#12 TSY fix 可合机械 fix PR（与 r1/r3/r4/r6 机械 fix 同性质）。
4. #10 TSY ghost：despawn vs 转移到主维度——需确认 collapse 后这些 hostile 的叙事归宿。

## 审计来源

bug-hunt round8（workflow，5 角度 finder + 怀疑者对抗 + opus 逐条全树复核 + 数值类引 worldview 章节佐证，12 候选）。**ROOT = fresh origin/main worktree**（方法论修正后第六轮）。已对 r1-r7 去重。**report-only**：P0 modifier orphan 大簇是**顶级系统主题**（累计 25+ 字段，建议统一接入层）；P1 距离衰减偏离 worldview §四是 worldview 锚点级数值偏差；#7/#8/#11/#12 局部明确可机械 fix。**外部 review 价值印证**（[[feedback_external_review_catches_semantic]]）：#6 这类"测试名义拟合 worldview 实则锁定偏离值 + plan 校准从未落地"靠对照正典才抓得出，纯代码自洽检查会漏。
