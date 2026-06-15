# plan-bughunt-r10-findings-v1（骨架）

> **骨架（草案）**。一句话主题：代码库自检 bug-hunt **round10（末轮）**（fresh origin/main worktree ROOT，角度：HUD 数据喂给完整性 · persistence 往返 · schema sample 对拍 · 最近合并 plan 深挖 · e2e 玩法链）确认的 **6 个新真 bug**——含 **critical：盾牌破盾后 ShieldBlock/ShieldBlocking 状态不清理（破碎盾仍 50% 减伤 + 持续扣体力 + Exhausted 反受硬直惩罚）** + body.guangbo_ticao 熟练度闭环断裂（揭示 r8 jump_height 失效根因）+ 两处关服刷盘盲区 + 2 处 schema source-of-truth 漂移。已对 r1-r9 去重，全部 real-on-main。

> 立项动机：round10 系统性完整性角度（HUD payload / persistence 往返 / schema sample / 最近 plan / e2e 链）。7 候选 → **6 REAL / 1 NOT_REAL**（dismiss：throughput_peak_norm 恒 0 是 schema-first 未实装占位）。**这是本夜 bug-hunt 循环的最后一轮**（用户指令 round10 结束即停）。

## 阶段总览（按主题分组，逐项独立可修）

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 🔴 盾牌破盾后状态不清理（破碎盾仍减伤 + 扣体力 + 反受硬直） | fix_pr | ⬜ |
| P1 | body.guangbo_ticao 熟练度闭环断裂（练习事件无生产者） | fix_pr | ⬜ |
| P2 | persistence 关服刷盘盲区（矿脉复生 + 领地影响力回滚） | fix_pr | ⬜ |
| P3 | schema source-of-truth 漂移（SenseKind / TribulationState.kind） | fix_pr | ⬜ |

## P0 — 🔴 盾牌破盾后状态不清理（critical）

- **#5 critical（fix_pr）**：`server/src/combat/resolve.rs:1086-1097` 盾牌耐久归零（`next_ratio<=0.0`）时**只** `consume_item_instance_once` 清空 off_hand + send `ShieldBroken`，**不移除 `ShieldBlock` component 也不移除 `ShieldBlocking` status**。`ShieldBroken` 全仓唯一 reader 是 `weapon_equipped_emit.rs:210` `emit_shield_broken_payloads`，**只发网络 payload 不做 ECS 清理**（npc/lifecycle.rs:1168、network/mod.rs:996 仅 add_event）；`shield_block.rs` 无 `EventReader<ShieldBroken>`。三条后果成立：
  - ① 破盾后下次被击 off_hand 空 → 格挡分支 fallback `("wooden_shield",0.0)`（resolve.rs:1005）→ `shield_block_profile` 返回 `block_ratio=0.50`（shield_block.rs:108-110），`fov_ok && ratio>0.0` 成立 → **破碎盾继续 50% 减伤**；
  - ② `lifecycle.rs:298` `StaminaState::ShieldBlocking` 分支**持续扣体力**直至 Exhausted；
  - ③ Exhausted 时 `force_lower_shield_on_stamina_exhausted`（`With<ShieldBlock>`）才清理且施加 `ParryRecovery` 破势硬直 → **玩家破盾后反被额外硬直惩罚**。
  - 修：`next_ratio<=0.0` 时等价调 `lower_shield_handler` 清理（remove ShieldBlock + ShieldDrainOverride、remove_status_effect ShieldBlocking、恢复 StaminaState）。**shield-block-v1 新代码，critical 状态机泄漏，局部明确。**

## P1 — body.guangbo_ticao 熟练度闭环断裂

- **#6 major（fix_pr）**：`body.guangbo_ticao` **学→绑→施→效 闭环断裂**。`GuangboTicaoPracticeEvent`（`body_conditioning.rs:45`）定义、`combat/mod.rs:225` 注册、`combat/mod.rs:315` `consume_guangbo_practice_events` 消费 → `record_guangbo_practice`（**唯一熟练度增长路径**，body_conditioning.rs:56）。但全仓 grep `.send(GuangboTicaoPracticeEvent` **仅命中定义本身、生产端零结果**。body.guangbo_ticao 是完整 Buff 技能（known_techniques.rs:798 qi_cost=1/cast=60/cd=200，scroll_body_guangbo_ticao 习得 inventory.rs:10489），但**不在 skill_registry**（skill_registry.rs:63-79 无 body::register_skills）→ 走 generic SkillBar cast；`cast_emit.rs:218-233` 自然完成时**仅 yidao.* 发完成事件**，SkillBar 源只进冷却无任何事件/效果 → **熟练度恒 0** → `apply_guangbo_ticao_bonuses`（body_conditioning.rs:87）因 prof=0 恒返回 0 bonus（move_speed/jump_height/limb_defense 全无效）。玩家学了绑了反复施放只见 60-tick 动画 + 冷却，**零熟练度零 buff**（**这是 r8 #4 jump_height_multiplier 无效的上游根因**：不仅 server 不读，连熟练度本身都恒 0）。修：cast 完成时为 body.guangbo_ticao（或通用 Buff body.* 路径）send `GuangboTicaoPracticeEvent`。**局部明确。**

## P2 — persistence 关服刷盘盲区

> 两处持久化系统只挂 Update + 节流 flush，**无 Last-schedule + EventReader<AppExit> 关服 hook**（对照 `player/mod.rs:36+116` `flush_connected_players_on_shutdown` 是正确先例）。关服/崩溃落在节流窗口内 → 末次窗口数据丢失。

- **#1 major（fix_pr）**：`server/src/mineral/` `record_exhausted_minerals` 仅挂 Update（mod.rs:93-102），节流 `flush_interval_ticks=600`(~30s)，无 Last/AppExit hook。**永不再生矿脉**（respawn_at_tick=None）被挖穿后 30s 内关服/崩溃 → 重启 `hydrated()` 读不到该 entry → **worldgen 复生 ore** → 违反 §2.1 矿脉有限性不变量（有具体玩法后果：永久损耗矿脉复活）。修：mineral::register 加 Last+AppExit + 强制 `log.flush()`。
- **#2 minor（fix_pr）**：`server/src/persistence/mod.rs:595-618` `persist_zone_influence_system` 仅挂 Update，节流 `ZONE_RUNTIME_SNAPSHOT_INTERVAL_SECS=300s`，无 AppExit hook。`territory_tick`（territory.rs:39,339）每 ~60s mutate ZoneInfluenceMap + death 即时 recompute_dominance → 重启前最后 0-300s 窗口的影响力/霸主转移关服永不落盘 → `hydrate_zone_influence` 回滚到最多 5 分钟前快照。同 register 内 `persist_zone_runtime_system` 同病。修：加 Last-schedule AppExit handler 调 persist_zone_influence_snapshot + persist_zone_runtime。

## P3 — schema source-of-truth 漂移

> 两处 TypeBox（CLAUDE.md 定为 source-of-truth）相对 Rust 运行时 emit 的实际值**缺字面量**。⚠️ **均无运行时 payload-drop**（ServerData/SpiritualSense/TribulationState 是 server→client CustomPayload，agent 端不 ingest 不校验，Java client 当原始字符串透传），故为**契约/source-of-truth 漂移**而非破坏行为——据严格门从 major 降 minor，但仍是 code-gen/未来校验工具会漏的真实缺口。

- **#3 minor（fix_pr）**：`agent/packages/schema/src/spiritual-sense.ts:5-16` `SenseKindV1` 缺 `"DyingElderQi"`——Rust `realm_vision.rs:49` 有且运行时真 emit（scanner.rs:163、push.rs:201 收集 Plea/Recovering 态垂死大能），Java `SenseKind.java:18,38` 真处理，唯 TypeBox 漏。修：加 `Type.Literal("DyingElderQi")` + 补 sample。
- **#4 minor（fix_pr）**：`agent/packages/schema/src/server-data.ts:771-773` `ServerDataTribulationStateV1.kind` inline union 仅 du_xu/zone_collapse/targeted，而 canonical `tribulation.ts:7-12` `TribulationKindV1` 含 jue_bi + ascension_quota_open；Rust 运行时真 emit jue_bi（tribulation_state_emit.rs `TribulationKind::JueBi=>"jue_bi"`）。**inline union 是 TribulationKindV1 的手抄副本**，新增 JueBi 时漂移。修：kind 改引用 `TribulationKindV1`（消除手抄）或补两字面量 + jue_bi sample。

## §N 开放问题

1. #5 破盾清理：直接调 `lower_shield_handler` vs 给 ShieldBroken 加 ECS-清理 reader（后者更解耦但多一跳）——建议前者（同步清理避免破盾后单帧窗口仍减伤）。
2. #6 guangbo_ticao：是否把 body.* 纳入 skill_registry（统一 cast 路径发 practice/完成事件）vs 仅在 SkillBar 通用完成路径补 body 技能的 practice emit——前者根治"SkillBar 通用招式完成路径只 yidao.* 发事件"的同类断裂（其他通用 Buff 技能可能同病，值得一并查）。
3. #1/#2 关服刷盘：是否抽一个通用 "register_shutdown_flush(system)" helper 让所有节流持久化系统统一挂 Last+AppExit（根治"逐个 persist 系统忘记关服 hook"，player 已有先例可抽公共）。
4. #3/#4 schema 漂移：建议加"TypeBox SenseKindV1/TribulationStateV1.kind 必须覆盖 Rust enum 全变体"的双端对拍 pin 测试（防再漂）；inline union 一律改引用 canonical 类型。

## 审计来源

bug-hunt round10（**末轮**，workflow，5 系统性角度 finder + 怀疑者对抗 + opus 逐条全树复核，7 候选）。**ROOT = fresh origin/main worktree**（方法论修正后第八轮，也是末轮）。已对 r1-r9 去重。**report-only**：critical 破盾状态泄漏（shield-block-v1）优先；#6 guangbo_ticao 闭环断裂（揭 r8 jump_height 根因）+ #1/#2 关服刷盘 + #3/#4 schema 漂移均局部明确可 fix_pr。**末轮收尾**：HUD/persistence/schema/recent-plan/e2e 系统性角度仍挖出 critical + broken-loop，证明全新角度持续有效；10 轮累计为后续 consume 提供完整 roadmap。
