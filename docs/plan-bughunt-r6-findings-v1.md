# plan-bughunt-r6-findings-v1（active）

> **Active（已从 skeleton 升级，待逐项消费）**。一句话主题：代码库自检 bug-hunt **round6**（fresh origin/main worktree ROOT，换角度：alchemy 炼丹链 · botany/shelflife · 渡劫 wave 状态机 · 装备 equip · agent Arbiter/WorldModel）确认的 **4 个新真 bug**——含 **1 critical（register_mundane_armors 撞 JSON ID 早退 → 16 种凡甲永不入 registry 静默零防御）**。已对 r1-r5 去重，全部 real-on-main。

> 立项动机：round6 用 fresh origin/main worktree 为 ROOT，5 全新角度 finder → 怀疑者对抗 → opus 逐条 Read/Grep（含**模拟核实** armor 注册早退）复核，11 候选 → **4 REAL / 7 NOT_REAL**（严格裁决，NOT_REAL 比例升高 = 易发 bug 渐枯竭 + 去重生效的健康信号；dismiss 含 AlchemyBuff 兼容兜底/v2 env_lock 生态税设计/HeartDemon choice-gate 误读/JSON-wins 设计等）。本轮主线：**alchemy/shelflife status-effect 孤岛延续**（ContaminationBoost 无 consumer，同 r4 buff 簇）+ **registry 注册健壮性**（撞 ID 早退，与 r5 同类配置加载缺口）。

## 阶段总览（按主题分组，逐项独立可修）

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 🔴 register_mundane_armors 撞 JSON ID 早退 → 16 凡甲零防御 | fix_pr | ⬜ |
| P1 | alchemy status-effect 孤岛（ContaminationBoost 无 consumer + JinZhongDan 极性反向） | plan_skeleton | ⬜ |
| P2 | shelflife Freeze 容器从不接线（enter/exit_container 零调用） | plan_skeleton | ⬜ |
| P3 | 渡劫 JueBiAfterDuXuQuota 4/5 结算路径漏清理 | plan_skeleton | ⬜ |

## P0 — 🔴 register_mundane_armors 撞 ID 早退（critical）

- **#8 critical（fix_pr）**：`server/src/armor/mundane.rs:249-254` `register_mundane_armors` 在**已填充**的 registry 上跑（`combat/armor.rs:198-248` `load_dir` 先加载 12 个唯一 JSON profile：armor_bone_*×4 / armor_iron_*×4 / cloth_robe 等）。顺序 Straw→Bone→…：Straw 4 件 OK，**第 5 件 `armor_bone_helmet` 与 JSON 撞 ID** → `register_template`（`armor.rs:188-190`）返 Err → `mundane.rs:251` 的 `?` **立即传播中止全部剩余注册**；`combat/mod.rs:188-190` 仅 log 错误后照用残缺 registry。**模拟确认**：`armor_hide_*` / `armor_copper_*` / `armor_spirit_cloth_*` / `armor_scroll_wrap_*` 共 **16 件凡甲**（均在 `assets/items/armor.toml` 可造可穿）永不入 registry → `armor_sync.rs:29-31` `armor_profiles.get(...)` 返 None 即 continue → **这 16 种甲静默零减伤**。`all_28_items_registered` 单测用空 registry 故漏掉撞 ID。（finding 自报 20 件，实测 16 件——Iron 因有 JSON 仍生效。）修：`?` 改 skip-on-duplicate `continue`，或调换注册顺序，或 register_template 改 upsert/幂等；补"凡甲 profile 全覆盖"回归测试（非空 registry 下）。**critical 防御失效，局部明确。**

## P1 — alchemy status-effect 孤岛

- **#0 major（plan_skeleton）**：`ContaminationBoost` StatusEffectKind **无运行时消费者**。`side_effect_apply.rs:25` 把 `contam_boost` 映射为 `StatusEffectKind::ContaminationBoost`，经 `client_request_handler.rs:9993` `build_side_effect_application` 真实下发存入 `StatusEffects`。但 `contamination.rs:88-129` `contamination_tick` 的 Query tuple **不含 StatusEffects**，排异速率只由 `BASE_PURGE_RATE*(1+purge_rate_bonus(alchemy_lv))` 决定，从不读 status。全仓 grep ContaminationBoost 仅命中 events.rs(定义)/side_effect_apply.rs(映射)/status_snapshot_emit.rs(HUD"丹毒加重")——零消费者。关键：`events.rs:113` 注释明示"施毒类丹药副作用，增加污染/中毒压力"（**具体行为意图**，非兜底 stub，区别于 dismiss 的 AlchemyBuff）→ 玩家炼出 flawed 丹触发 contam_boost 时本应加重污染，**实际无任何效果**。修：定义 magnitude 如何接入 `contamination.entries`（与 r4 QiCapPermMinus/QiRegenBoost 同属 alchemy effect 孤岛簇，建议合并接线）。**需设计接入点。**
- **#1 minor（plan_skeleton）**：`pill.rs:602-606` `JinZhongDan` 负面槽 `push(QiRegenBoost, 0.001, negative_duration_ticks)`——**QiRegenBoost 是增益**（outcome.rs/side_effect_apply.rs 全用于 benefit），对照紧邻 `NingJiaSan`（619-623）负面槽用 `DamageAmp`（真 debuff，同 0.001 占位），可见 negative 槽语义是"不利效果"。当前 QiRegenBoost 无 consumer 故惰性；一旦补 consumer，JinZhongDan 将通过"减益"路径反给玩家 0.001 微弱**增益**，与设计相反。修：负面槽换真 debuff 或正确极性（与 QiRegenBoost consumer 接入一并修）。**潜伏极性错误。**

## P2 — shelflife Freeze 容器从不接线

- **#4 major（plan_skeleton）**：`server/src/shelflife/container.rs:82/94` `enter_container`/`exit_container` 完整定义并单测，但全仓 grep（排除 container.rs/mod.rs）**零外部调用**——无任何库存事件接线。后果：`frozen_since_tick`/`frozen_accumulated` 永不被生产路径置位（`compute.rs:247` `effective_dt_ticks` 的冻结减除逻辑因输入恒 0 而死）。消费路径双重确认：`client_request_handler.rs:10424` 硬编码 `ContainerFreshnessBehavior::Normal`，`cast_emit.rs:270` consume_food multiplier 硬编码 1.0。`ling_xia`（`spiritwood/mod.rs:606`=Freeze）中存放数日的食物，sweep 期 `container_mul=0.0` 保持新鲜，但 FoodRegen 消费时按全墙钟 + Normal 重算 effective_dt，**可能跌破 `0.1×spoil_threshold` 触发 CriticalBlock**（`cast_emit.rs:272` 拒绝消费）→ 玩家冻存的食物被误判腐坏拒食。机制（类型/compute/container.rs）齐备但缺接库存进出事件的接线。修：在库存存入/取出 Freeze 容器时调 enter/exit_container；消费路径读真实 ContainerFreshnessBehavior 而非硬编码 Normal。**孤岛接线，需定接线点。**

## P3 — 渡劫状态机泄漏

- **#6 major（plan_skeleton）**：`server/src/cultivation/tribulation.rs` `JueBiAfterDuXuQuota` **仅在** `juebi_settlement_system`（2019 remove tuple）清理，而 wave-complete Ascended（3139）、failure（3257）、fled（3494）、intercept-death（3568）**四条结算/终止路径全漏**；`start_tribulation_system`（983-985）仅在再次超额时插入新 marker，无 `.remove::<JueBiAfterDuXuQuota>()`。触发链：① 超额开 DuXu 插入 marker → ② 失败/逃遁/截杀 → TribulationState 移除但 **marker 残留** → ③ 再开**未超额** DuXu → 不插入新 marker，旧 marker 残留 → ④ 完成全波 → `tribulation_wave_system`（3053-3058）读到陈旧 marker **无条件追加 JueBi**（intensity 由陈旧 occupied_slots/quota_limit 推导）→ **伪造一场不该发生的绝壁渡劫**。修：四条终止路径都 `.remove::<JueBiAfterDuXuQuota>()`（与 r3 JueBi 断线孤儿行同属渡劫生命周期清理，建议合并）。**状态机泄漏，多路径清理决策。**

## §N 开放问题

1. #8 修法：skip-on-duplicate vs 调换注册顺序 vs register_template 幂等 upsert——哪个不破坏"JSON 权威覆盖 mundane 默认"语义（JSON-wins 是设计，见本轮 dismiss #9）。建议 skip-on-duplicate（JSON 已注册的跳过 mundane 默认）。
2. #0/#1 alchemy effect 簇：ContaminationBoost/QiRegenBoost/QiCapPermMinus(r4) 是否一并接线（统一 alchemy status-effect → consumer 的接入层），避免逐个补。
3. #4 容器接线点：在 inventory move intent / 容器存取 handler 处调 enter/exit_container；消费路径如何拿到物品所在容器的 ContainerFreshnessBehavior。
4. #6 渡劫清理：JueBiAfterDuXuQuota 与 r3 JueBi 断线孤儿行（plan-bughunt-r3）是否并入一个"渡劫生命周期清理"plan（统一所有终止路径的组件/DB 清理不变量）。

## 审计来源

bug-hunt round6（workflow，5 全新角度 finder + 怀疑者对抗 + opus 裁决，11 候选）。**ROOT = fresh origin/main worktree**（方法论修正后第四轮）。已对 r1-r5 去重（无一重复）。**report-only**：critical 凡甲零防御优先；#8 局部明确可直接 fix_pr，#0/#1/#4/#6 需 alchemy effect 接入层/容器接线/渡劫清理设计。**健康信号**：本轮 4 REAL / 7 NOT_REAL，NOT_REAL 比例较前几轮升高，提示易发 bug 趋于枯竭、去重机制有效——后续轮次应转向更深的集成/registry 健壮性/系统调度角度。
