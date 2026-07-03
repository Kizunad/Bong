# plan-zone-qi-economy-v1 — zone 灵气经济闭环：低浓度、活流量

> **一句话主题**：把 zone 灵气从"一次性存量"改成"低平衡浓度 + 持续流量"的闭环经济——修炼/突破消耗回流天地预算、预算按 zone 平衡点滴灌回流、NPC 吸取设让灵地板、固元 0.8 门槛靠灵潮/灵眼窗口——使 spawn 等低灵 zone 在保持末法薄灵观感的同时，供得起 2~3 名化虚 + 多名固元的完整修炼路径。

**状态**：active（§8 已收口，见 §8.1，含用户 2026-07-03 拍板的 2 条守恒/worldview 红线）。验收日期：全 P ✅ 后填。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | 消耗真归还——开脉/突破消耗回流**独立待分配池**（§8.1#1·非 WorldQiBudget），堵"记账蒸发" | ⬜ |
| P1 | 平衡回流——zones.json 配 equilibrium/inflow，heartbeat 从预算滴灌 | ⬜ |
| P2 | NPC 让灵地板——hydrated + dormant 吸取地板 0.3 | ⬜ |
| P3 | 固元突破窗口——灵潮（伪灵脉复用）/ 灵眼，含视听 | ⬜ |

---

## 背景诊断（2026-07-02，代码实证）

spawn zone `spirit_qi` 被观测到 0.00 且永不恢复。三个叠加根因：

1. **NPC 吸取无地板**：`apply_dormant_regen_with_multiplier`（`server/src/npc/dormant/mod.rs:1435`）唯一停止条件是 `zone.spirit_qi <= 0.0`；1000 dormant + 20 hydrated 流民把 spawn 的 0.9×50=45 绝对点全部吸进私池锁住。
2. **zone 无被动回流**：回补全事件驱动（伪灵脉/技能散逸/植物凋亡/NPC 死亡），见底即永久停摆。
3. **修炼消耗"假归还"（最大漏点）**：`credit_meridian_open_cost`（`server/src/cultivation/meridian_open.rs:299`）与 `credit_active_breakthrough_cost`（`server/src/cultivation/breakthrough.rs:590`）只写 `WorldQiAccount` 账本 zone 账户，**从不写回 `zone.spirit_qi`**，而账本是 audit-only（#676 系列结论）。每开一脉（5 绝对点）、每次突破（8/25/80/250/800，`breakthrough_qi_cost`，`breakthrough.rs:67`）都从可玩池永久蒸发——账本守恒、玩家世界消失。同时账本 zone 账户余额被单向注水，`summarize_world_qi` 审计口径被持续歪曲。

**量级核算**：zone 满仓仅 50 绝对点（`QI_ZONE_UNIT_CAPACITY=50.0`，`qi_physics/constants.rs:80`）。一人修到化虚累计消耗 ≈1263 绝对点（突破 1163 + 开脉 100）≈ 25 个满仓 zone；目标编制（3 化虚 + 4 固元）≈ 4500 绝对点 ≈ 90 个满仓。且冲化虚前需**瞬时持有** `qi_current ≥ 800` > 单 zone 存量上限。→ 存量模型无解，必须改流量模型。

## 接入面（docs/CLAUDE.md §二 checklist）

- **进料**：`cultivation::meridian_open` / `cultivation::breakthrough` 的消耗流；`qi_physics::ledger::WorldQiBudget`（已有 `from_total` / `apply_era_decay` / env `BONG_SPIRIT_QI_TOTAL`，`ledger.rs:15-55`）；`server/zones.json`（`ZoneConfig`，`world/zone.rs:501`）；`world/pseudo_vein_runtime.rs`（灵潮复用）；`world/spirit_eye.rs`（`SpiritEyeRegistry`）；`npc/dormant/mod.rs` + `cultivation/tick.rs`（NPC 吸取路径）。
- **出料**：`zone.spirit_qi`（玩家/NPC 修炼吸收、开脉门槛 0.3、固元门槛 0.8 全部消费此值）；`ZoneInfo` payload（client HUD 自动受益，wire 不变）；`heartbeat::compute_world_pressure` 的 `avg_zone_qi`（天道 world_state 监控恢复有意义读数）；`QiTransfer` 审计流。
- **共享类型 / event**：复用 `QiTransfer` / `QiTransferReason`（P1 新增 variant `ZoneInflow`，理由：现有 reason 无"天地→zone"语义）；**新增独立待分配池 ledger 账户**（§8.1#1，**非复用 `WorldQiBudget.current_total`**——它是化虚名额闸门 `compute_void_quota_limit`，注入破坏 void-quota 稀缺性）；`Zone` / `ZoneConfig` 加字段不另造注册表；复用 `PSEUDO_VEIN_*_VFX_EVENT_ID`（`pseudo_vein_runtime.rs:36-40`）。
- **跨仓库契约**：纯 server plan。`ZoneInfo` / `PlayerState` payload 结构不动；`zones.json` 为 server 本地配置非 IPC schema。agent 侧仅被动受益（`world_state` 里 avg_zone_qi 不再恒 0），client 零改动。
- **worldview 锚点**：worldview §二（真元物理/守恒：全服 `SPIRIT_QI_TOTAL` 恒定，修炼消耗=别人少掉；本 plan 的"消耗→预算→回流"正是该正典的闭环实现）；末法薄灵设定（低 equilibrium 是观感要求，不是资源匮乏 bug）。
- **qi_physics 锚点**（红旗强约束）：消费 `regen_from_zone`（`excretion.rs:43`）、`qi_release_to_zone`、`WorldQiAccount::transfer`、`WorldQiBudget::apply_era_decay`。**新常数/公式全部落 `qi_physics`**：`QI_ZONE_INFLOW_*`、`QI_NPC_ABSORB_FLOOR`、`fn zone_equilibrium_inflow(...)`；zones.json 只声明参数（equilibrium/inflow_per_min），公式唯一实现在 qi_physics。

## 总体设计：天地回环

```
玩家/NPC 开脉·突破消耗 ──QiTransfer(MeridianOpen/Breakthrough)──→ 独立待分配池（§8.1#1·非 WorldQiBudget，勿碰化虚名额闸门）
                                                                       │ 每 zone 按配置滴灌
        zone.spirit_qi ←──QiTransfer(ZoneInflow)、只补到 qi_equilibrium 即停──┘
              │ regen_from_zone 吸收（NPC 受让灵地板约束）
        玩家 qi_current / NPC 私池 ──（死亡/技能散逸已有归还路径）──→ zone
```

闭环后全服总量恒定（唯一外流仍是天道时代衰减 1-3%），浓度被 equilibrium 钳在低位，累计吞吐无上限。

---

## P0 消耗真归还（堵蒸发）⬜

> **⚠️ §8.1 #1（用户 2026-07-03 拍板）改写本阶段**：回充目标从 `WorldQiBudget` 改为**新增独立"待分配池"ledger 账户**（**完全不碰 `WorldQiBudget.current_total`**——它已是化虚名额闸门基准 `compute_void_quota_limit`，注入会让名额随修炼活跃度浮动、可被玩家刷高，破坏 void-quota 稀缺性）；`DEFAULT_SPIRIT_QI_TOTAL=20000`（§8.1 #2）。下方原文（回充 WorldQiBudget）保留追溯，**实施以 §8.1 为准**。

- 改 `credit_meridian_open_cost`（`meridian_open.rs:299`）与 `credit_active_breakthrough_cost`（`breakthrough.rs:590`）：消耗从行为者账户转入 **WorldQiBudget 回充**（新 helper `qi_physics::ledger::credit_world_budget(reason, amount)` 之类），不再注水 audit-only 的 `QiAccountId::zone` 账户。选择预算而非直写 `zone.spirit_qi` 的理由：化虚成本 800 >> zone 满仓 50，直还必然大额 overflow；回环经预算再由 P1 滴灌分配。
- 保留 `QiTransfer` 审计（reason 不变：`MeridianOpen` / `Breakthrough`），to 账户改为预算侧表示——具体账户形态见 §8 #1。
- 处理历史注水：`summarize_world_qi` / `total_observed` 中 ledger zone 账户余额的既有语义偏差，迁移或重置方案见 §8 #1。
- **测试**：meridian_open / breakthrough 现有守恒测试改锁新流向（消耗后 `budget_current_total` 等额上升）；开脉→突破全链路总量不变的端到端守恒对拍；`LedgerUnavailable` 回滚分支保持（`breakthrough.rs:707`）。

## P1 平衡回流（低浓度、活流量）⬜

- `ZoneConfig` / `Zone` 加 `qi_equilibrium: f64`（默认 0.0 = 不回流，向后兼容）与 `qi_inflow_per_min: f64`；`zones.json` 给 spawn 配 `qi_equilibrium: 0.35`（> 开脉门槛 `MIN_ZONE_QI_TO_OPEN=0.3`）+ inflow 初值（标定见 §8 #2）。
- qi_physics 新增：`fn zone_equilibrium_inflow(spirit_qi, equilibrium, inflow_per_min, dt) -> f64` + 常数；**只补到 equilibrium 即停**（浓度钳制），来源扣**独立待分配池**（§8.1 #1：非 `WorldQiBudget`，勿碰化虚名额闸门），余额不足则缩量、绝不透支。
- heartbeat（`world/heartbeat.rs`）新增回流 system：直写 `zone.spirit_qi`（遵循 #676 后守恒写法：改真实池 + 记 `QiTransfer(ZoneInflow)` 审计），负灵域（spirit_qi<0）zone 不回流（保留死域/负灵域设定）。
- **测试**：钳制在 equilibrium 不过冲；**待分配池余额不足缩量**；负灵域跳过；回流↔吸收长跑总量守恒；`REALM_COLLAPSE` 事件 zone 跳回流（§8.1 #5）。

## P2 NPC 让灵地板 ⬜

- qi_physics 新常数 `QI_NPC_ABSORB_FLOOR = 0.3`（与 `MIN_ZONE_QI_TO_OPEN` 同值、独立常数，语义：NPC 只喝地板以上的溢出层，玩家开脉永远有底仓）。
- 落点：`apply_dormant_regen_with_multiplier`（`dormant/mod.rs:1435`，现行 `<= 0.0` 改 `<= FLOOR`）+ `cultivation/tick.rs` regen 的 NPC 分支（`NpcMarker` 判定）。玩家吸取不受地板约束。
- war_multiplier 路径（plan-offscreen-war-v1 P9）同样过地板，不给战事 zone 开后门。
- **测试**：NPC 在 floor±ε 边界停手/恢复；玩家不受限；dormant 批 tick 大规模跑不把带回流 zone 压到 floor 以下稳态。

## P3 固元突破窗口（灵潮 / 灵眼）⬜

baseline 0.35 永远过不了 `MIN_ZONE_QI_TO_GUYUAN=0.8`（`breakthrough.rs:357-390`），需要窗口机制。路线二选一或并行（§8 #3 拍板）：

- **灵潮**：复用伪灵脉 runtime（`pseudo_vein_runtime.rs`，phase 机 + settlement——**现仅收回 30%、70% 留 zone 凭空创生**，须按 §8.1 #3 改造为借还款），周期性把目标 zone 短暂推到 0.85 再回落；qi 来源走**独立待分配池借还**（§8.1 #3：inject/settle 改借还款、修 70% 凭空创生，非 WorldQiBudget），不凭空。
- **灵眼**：spawn 附近落一口 `SpiritEyeRegistry` POI（`breakthrough.rs:368` 已有 `in_spirit_eye` 替代门槛 + 环境加成路径，纯配置接入）。
- **视听（须在升 active 时写到实现精度，docs/CLAUDE.md §四）**：灵潮复用 `PSEUDO_VEIN_RISING/ACTIVE/DISSIPATING` VFX 事件族与其既有 audio_recipe；narration 走天道 zone-scope（示例文案 ≥2 条、标 scope/style）；HUD 无新增常驻元素（对齐 HUD 沉浸极简约束）。
- **测试**：灵潮窗口内固元前置通过 / 窗口外拒绝（`MIN_ZONE_QI_TO_GUYUAN` 边界）；潮起潮落全程守恒对拍。

---

## §8 开放问题（升 active / P0 决策门前收口）

1. **P0 回充的账本表示**：`WorldQiBudget` 现无账户语义（只是 total + era_decay_accum）——是给 ledger 加 `QiAccountId::world_source` 特殊账户并与 budget 同步，还是 budget 直接加 `credit()` 方法 + QiTransfer 只留审计？历史注水的 ledger zone 账户余额如何清算（一次性迁移 vs 起服重置）？
2. **数值标定**：`SPIRIT_QI_TOTAL`（`DEFAULT_SPIRIT_QI_TOTAL`，env `BONG_SPIRIT_QI_TOTAL`）现值是否 >> 全 zones 存量和 + 目标编制峰值持有（3×800 + NPC 私池 + zone 底仓）？spawn `qi_inflow_per_min` 取值（草案 1~2 绝对点/分钟）对应的化虚攒气时长是否符合期望节奏？需盘 `zones.json` 全量 + 实测 NPC 私池稳态占用。
3. **固元窗口路线**：灵潮 / 灵眼 / 两者并行？灵潮周期与时长（对齐季节系统？`season_success_modifier` 已有季节耦合）。化虚走渡劫（`tribulation.rs`），需确认渡劫是否有独立 zone_qi 门槛。
4. **其他抽取源是否过地板**：天道监视（`QI_TIANDAO_WATCH_ZONE_DRAIN_PER_MINUTE`）、灵田（`LINGTIAN_DRAIN_ZONE_RATIO`）、TSY 抽干是否也应止步于 floor / equilibrium 之下？
5. **事件 zone 边界**：`REALM_COLLAPSE` / TSY 抽干中的 zone 是否暂停回流？负灵域恢复（回正）是否属本 plan（倾向：不属，负灵域保持只出不进的设定）。
6. **邻接待办不占用声明**：`reminder.md` 中 `practice_session_tick` 接活的守恒补课是独立 plan，本 plan 不吞其 scope；但 P0 落地的"消耗→预算"helper 应设计为它将来可复用。

> 全部已在 §8.1 收口。原表保留以备追溯，**实施时以 §8.1 决议为准**。

## §8.1 决议（pre-P0 收口，2026-07-03，Explore 实地核查 qi_physics 17 文件 + 用户拍板 2 条守恒/worldview 红线）

### #1 P0 回充账本表示 + 历史注水清算 ★守恒核心（用户拍板）

**决议**：
1. **P0 消耗回充目标 = 新增独立"待分配池" ledger 账户，完全不碰 `WorldQiBudget.current_total`**（用户 2026-07-03 拍板）。原因（Explore 揪出的结构性冲突，骨架未提）：`WorldQiBudget.current_total` 已是 `plan-void-quota-v1`/`plan-tribulation-balance-v1` 的**化虚名额闸门基准**（`compute_void_quota_limit(budget.current_total, quota_k)`，`tribulation.rs:118-190`）+ `plan-void-actions-v1` 爆区/结界借还款池（`void/ledger_hooks.rs:88-106,146-162`）。若注入它 → 全服开脉/突破越频繁 → 化虚名额上限越高（玩家可组队刷高化虚名额）→ 破坏 void-quota 稀缺性。
2. 新增独立账户（建议 `QiAccountId::zone("__pending_inflow")` 或复用 `Overflow` kind，实施定）；`credit_meridian_open_cost`(`meridian_open.rs:299-314`)/`credit_active_breakthrough_cost`(`breakthrough.rs:590-605`) 的 `to` 从"注水 audit-only zone 账户"改指向此待分配池；P1 滴灌从此池出（**不从 WorldQiBudget**）。
3. **记账范本照抄 `dormant/mod.rs:1470-1495`**（`apply_dormant_regen_with_multiplier` 双账本严格同步：`ledger.set_balance` + `ledger.transfer` + 真实字段变更），**不照抄 `credit_meridian_open_cost` 现在"只写 ledger 不动真实字段"的写法（那正是 bug）**。
4. 历史注水清算 = **起服重置**（用户拍板）：bug 累积的错误 ledger 余额升级时清零不迁移。
5. helper 通用签名 `credit_pending_inflow(account, zone_name, from, amount, reason)` 供 #6 practice_session 未来复用。

**落点**：`server/src/qi_physics/ledger.rs`（新 helper + 待分配池账户）；`meridian_open.rs:299`；`breakthrough.rs:590`；范本 `dormant/mod.rs:1470-1495`；plan §P0。

### #2 数值标定 ★worldview 红线（用户拍板）

**决议**：
1. `DEFAULT_SPIRIT_QI_TOTAL = 100.0`（`constants.rs:64`）→ **20000.0**（用户拍板保守默认，≈目标编制峰值 4500 的 4.4×，留 NPC 私池+波动冗余）。实测全 zones 满仓仅 ≈453 绝对点、100 小一个数量级 = 经济死局根因。`DEFAULT_VOID_QUOTA_K` 导出常量自动跟随缩放，"满预算 2 化虚名额"不受影响。
2. **`worldview.md:874`「SPIRIT_QI_TOTAL = 100」走单独 docs PR 改**（用户授权，§10.0）：硬编 100 → 引用 const/加注"早期占位值，已按实际经济规模重标定"。**consume 归档前该 worldview PR 必 land**（docs/CLAUDE.md §6.3）；zone-qi 代码 PR 只改常量 20000，不碰 worldview 文本。
3. `qi_inflow_per_min` 保守默认 **0.3~0.5 绝对点/分钟**（草案 1~2 偏快；0.3~0.5 对应 spawn equilibrium≈17.5 点攒满 35~58 分钟，符合 worldview §三"苦修极慢"，远快于当前"永不恢复"）。
4. **残留（用户已知）**：NPC 私池稳态占用静态审计给不出，需起服遥测 dump `account:npc:*`（`ledger.rs:500-537` 导出口）后二次校准 inflow；PR 阶段 0.3~0.5 为保守估计（§10.7 待办）。

**落点**：`constants.rs:64`；`worldview.md:874`（单独 PR §10.0）；`zones.json`；plan §P1 §P2。

### #3 固元窗口路线

**决议**：**灵眼优先 + 灵潮并行补偿**。
1. 灵眼：worldview §十已锚"灵眼=凝脉→固元突破必需"（`worldview.md:889`），`breakthrough.rs:368` `in_spirit_eye` 替代门槛路径已存在、纯配置接入，风险最低（`spirit_eye.rs:67-70` POI）。
2. 灵潮**非简单复用**：`inject_zone_for_pseudo_vein`（`pseudo_vein_runtime.rs:391-407`）注入到 `PSEUDO_VEIN_MAX_QI=0.6`（<`MIN_ZONE_QI_TO_GUYUAN=0.8`），且 `settle_pseudo_vein_qi`（`:555-574`）只收回 30%、**70% 永久留 zone = 既有凭空创生缺陷**。复用灵潮须先改造两函数：注入从 §8.1#1 待分配池借出、settlement 按比例还（抄 `ledger_hooks.rs:88-106` `borrow_explode_zone_qi`），`PSEUDO_VEIN_MAX_QI` 0.6→≥0.85（提前 grep 确认无下游依赖，§10.7）。
3. 化虚渡劫走 `VoidQuotaConfig` 名额闸门、与 zone.spirit_qi 无关，本 plan 不补渡劫 zone_qi 门槛。
4. 视听：灵潮沿用 `PSEUDO_VEIN_RISING/ACTIVE/DISSIPATING_VFX_EVENT_ID`（`pseudo_vein_runtime.rs:36-39`）+ 既有 audio_recipe；narration 天道 zone-scope（示例：「灵潮涌动，此地灵气一时丰沛，正是冲击固元的良机。」scope=zone/style=perception）；灵眼纯配置无新视听。

**落点**：`pseudo_vein_runtime.rs:391-407,555-574,23`；`breakthrough.rs:368,45`；`spirit_eye.rs:67-70`；plan §P3。

### #4 其他抽取源过地板

**决议**：真正会击穿地板的 zone-drain 抽取源纳入 P2 scope 加 floor（同"地板"物理概念，不拆 P4）——两条：① **天道监视** `QI_TIANDAO_WATCH_ZONE_DRAIN_PER_MINUTE`（`constants.rs:70`）→ `world/tiandao_hunt.rs:1024` `apply_watch_zone_qi_drain`（`:1051` 直扣 `zone.spirit_qi`）；② **灵田区域抽吸** `ReplenishSource::Zone`（`lingtian/systems.rs:1262-1267` `*z -= amount`，`amount = plot_qi_amount()` = 0.5 硬编）。**⚠️ 更正（round-2 博弈）**：`LINGTIAN_DRAIN_ZONE_RATIO`（`constants.rs:86`）**不是抽取常数、不钳**——它是灵田偷灵结算把已扣 plot_qi 的 20% **回 credit 给 zone** 的散逸比例（`systems.rs:1116→1133 *zone_qi += actual_to_zone`，往 zone 加钱），给它加 floor 是 no-op。

**落点**：`tiandao_hunt.rs:1024,1051`（天道监视）；`lingtian/systems.rs:1262-1267`（灵田区域抽吸，`plot_qi_amount`=0.5，`:519` 已有 `zone_qi >= amount` 预检）；`constants.rs:70`；plan §P2。

### #5 事件 zone 边界

**决议**：
1. P1 inflow system **显式排除** `zone.active_events` 含 `EVENT_REALM_COLLAPSE` 的 zone——现成范式在 `heartbeat.rs:1480`（`active_events.contains(...EVENT_REALM_COLLAPSE)`，正是 P1 inflow 要复用的模板），非 `network/mod.rs:2294`（那是灵蝗潮无关代码）。
2. 负灵域（`zone.spirit_qi < 0.0`）inflow `continue`（用 `< 0.0`，不对齐 botany `-0.2`）；负灵域回正不属本 plan。

**落点**：`heartbeat.rs`（新 inflow system，紧邻 `heartbeat_tick:569`，复用 `:1480` EVENT_REALM_COLLAPSE 判断范式）；`zone.rs:481-485`；plan §P1。

### #6 邻接待办不占用

**决议**：`practice_session_tick`（`cultivation/practice_session.rs:69,78`）现 dead_code、`*current_qi -= cost` 不走 ledger，是独立守恒待办，与本 plan 不重叠（走 MeridianOpen/Breakthrough reason）。本 plan #1 helper 通用签名供其未来复用。

**落点**：`reminder.md:27`；`practice_session.rs:69,78`；plan §6。

## §10 实施工作流

scope ~4 代码 PR + 1 前置 worldview docs PR，单 plan 内序列化（`docs/CLAUDE.md` §六）。**本 plan 是 ★qi 守恒 plan，每 PR 博弈自检尤其严查"灵气凭空产生/消失"。**

- **§10.0 前置 worldview PR（人工 review，consume 归档前必 land）**：`docs/worldview.md:874`「SPIRIT_QI_TOTAL = 100」改引用 const/加注重标定（用户已授权，§8.1 #2）。**单独 docs PR，不混进代码 PR**（docs/CLAUDE.md §6.3 worldview 硬约束）。zone-qi 归档前确认此 PR 已 land。
- **§10.1 拆分点**（依赖顺序，前一个 merge 后开下一个）：
  1. **PR-1 P0**：qi_physics 新增独立**待分配池**账户 + `credit_pending_inflow` helper（照抄 `dormant/mod.rs:1470-1495` 双账本同步范本）+ `credit_meridian_open_cost`/`credit_active_breakthrough_cost` 改 `to` 指向待分配池（**不碰 WorldQiBudget**）+ `DEFAULT_SPIRIT_QI_TOTAL=20000` + 起服重置历史注水。守恒测试锁新流向（消耗后待分配池等额升、总量不变）。
  2. **PR-2 P1**：`Zone`/`ZoneConfig` 加 `qi_equilibrium`/`qi_inflow_per_min` + `zones.json` spawn 配 0.35/0.3~0.5 + qi_physics `zone_equilibrium_inflow` 公式&常数 + heartbeat 回流 system（从待分配池出、钳 equilibrium、排除 REALM_COLLAPSE/负灵域）+ `QiTransferReason::ZoneInflow`。
  3. **PR-3 P2**：`QI_NPC_ABSORB_FLOOR=0.3`(qi_physics) + dormant/tick NPC 吸取过地板 + TiandaoWatch/Lingtian drain 过地板（P2 定位调用现场）。玩家吸取不受地板。
  4. **PR-4 P3**：灵眼 POI 配置 + 灵潮改造（inject/settle 走待分配池借还、**修 70% 凭空创生**、MAX_QI 0.6→0.85）+ 视听（复用伪灵脉 VFX + narration）。
- **§10.2 撞车防护**：每 PR 开前 `git fetch origin && git log origin/main` 比对 `qi_physics/`（ledger.rs/constants.rs 被其他 qi plan 动过？）/ `zones.json` / `breakthrough.rs` / `meridian_open.rs`。
- **§10.3 测试要求**：P0 消耗守恒对拍（待分配池等额升、总量不变）+ 起服重置；P1 钳 equilibrium/预算缩量/负灵域跳/REALM_COLLAPSE 跳/长跑守恒；P2 NPC 地板边界/玩家不受限/dormant 批 tick 不压穿；P3 灵潮窗口边界/潮起潮落守恒。**守恒断言取 const 引用不写字面（含 20000）**（docs/CLAUDE.md §四）。
- **§10.4 CR 等待**：每 PR ScheduleWakeup 1200s×≤3 等 CR（[[feedback_wait_coderabbit_approve]]）；限流时博弈过+e2e 绿即 merge。
- **§10.5 subagent 实施**：每 PR 独立 `claude` subagent（opus+`ultrathink`），主线收 result+merge；**每 PR push 前跑对抗博弈自检**（sonnet 控方/辩方/端到端 → opus 裁决，[[feedback_consume_presubmit_debate]]），★守恒 plan 控方专攻"灵气凭空产生/消失"。P3 视听全复用伪灵脉资产则豁免 3 轮 PROMISE。
- **§10.6 单次 consume 全自动到 merge**：收口已完成（本 §8.1 + 用户拍板 2 红线），`/consume-plan` 即可。worldview PR（§10.0）可与 consume 代码 PR 并行推进，但 **zone-qi 归档（进 finished_plans）前必确认 worldview PR 已 land**（时序口径以 §10.0 为准）。
- **§10.7 实施残留待办**（§8.1 已标）：① NPC 私池稳态遥测后二次校准 `qi_inflow_per_min`；② TiandaoWatch/Lingtian drain 调用现场 P2 定位；③ `PSEUDO_VEIN_MAX_QI` 0.6→0.85 前 grep 确认无下游依赖；④ `inject_zone_for_pseudo_vein` 70% 凭空创生（本 plan P3 顺修；**若最终只选灵眼不选灵潮，此既有缺陷需单列 P 修，勿搁置**）。

## 落地证据链

- 收口调研（2026-07-03，Explore agent 实地核查 qi_physics 17 文件 + zones.json + meridian_open/breakthrough/tribulation/void/dormant/heartbeat/pseudo_vein/spirit_eye + 用户拍板 2 守恒/worldview 红线）：**WorldQiBudget 双重语义冲突 → 独立待分配池**（保护 void-quota 稀缺性）；**SPIRIT_QI_TOTAL 100→20000 + worldview PR**；六条决议 file:line + 保守默认数值。
- 相关 plan：`plan-qi-physics-v1`（ledger/QiTransfer 唯一物理实现）；`plan-void-quota-v1`/`plan-tribulation-balance-v1`（WorldQiBudget 化虚名额闸门，本 plan **刻意不碰**）；`plan-offscreen-war-v1`（dormant 守恒范本）；`#676` 系列（记账蒸发历史结论）。
