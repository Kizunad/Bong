# plan-zone-qi-economy-v1 — zone 灵气经济闭环：低浓度、活流量

> **一句话主题**：把 zone 灵气从"一次性存量"改成"低平衡浓度 + 持续流量"的闭环经济——修炼/突破消耗回流天地预算、预算按 zone 平衡点滴灌回流、NPC 吸取设让灵地板、固元 0.8 门槛靠灵潮/灵眼窗口——使 spawn 等低灵 zone 在保持末法薄灵观感的同时，供得起 2~3 名化虚 + 多名固元的完整修炼路径。

**状态**：骨架（skeleton）。升 active 前须按 docs/CLAUDE.md §五 收口 §8 开放问题。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | 消耗真归还——开脉/突破消耗回流 WorldQiBudget，堵"记账蒸发" | ⬜ |
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
- **共享类型 / event**：复用 `QiTransfer` / `QiTransferReason`（P1 新增 variant `ZoneInflow`，理由：现有 reason 无"天地→zone"语义）；复用 `WorldQiBudget`（不另造预算资源）；`Zone` / `ZoneConfig` 加字段不另造注册表；复用 `PSEUDO_VEIN_*_VFX_EVENT_ID`（`pseudo_vein_runtime.rs:36-40`）。
- **跨仓库契约**：纯 server plan。`ZoneInfo` / `PlayerState` payload 结构不动；`zones.json` 为 server 本地配置非 IPC schema。agent 侧仅被动受益（`world_state` 里 avg_zone_qi 不再恒 0），client 零改动。
- **worldview 锚点**：worldview §二（真元物理/守恒：全服 `SPIRIT_QI_TOTAL` 恒定，修炼消耗=别人少掉；本 plan 的"消耗→预算→回流"正是该正典的闭环实现）；末法薄灵设定（低 equilibrium 是观感要求，不是资源匮乏 bug）。
- **qi_physics 锚点**（红旗强约束）：消费 `regen_from_zone`（`excretion.rs:43`）、`qi_release_to_zone`、`WorldQiAccount::transfer`、`WorldQiBudget::apply_era_decay`。**新常数/公式全部落 `qi_physics`**：`QI_ZONE_INFLOW_*`、`QI_NPC_ABSORB_FLOOR`、`fn zone_equilibrium_inflow(...)`；zones.json 只声明参数（equilibrium/inflow_per_min），公式唯一实现在 qi_physics。

## 总体设计：天地回环

```
玩家/NPC 开脉·突破消耗 ──QiTransfer(MeridianOpen/Breakthrough)──→ WorldQiBudget（天地）
                                                                       │ 每 zone 按配置滴灌
        zone.spirit_qi ←──QiTransfer(ZoneInflow)、只补到 qi_equilibrium 即停──┘
              │ regen_from_zone 吸收（NPC 受让灵地板约束）
        玩家 qi_current / NPC 私池 ──（死亡/技能散逸已有归还路径）──→ zone
```

闭环后全服总量恒定（唯一外流仍是天道时代衰减 1-3%），浓度被 equilibrium 钳在低位，累计吞吐无上限。

---

## P0 消耗真归还（堵蒸发）⬜

- 改 `credit_meridian_open_cost`（`meridian_open.rs:299`）与 `credit_active_breakthrough_cost`（`breakthrough.rs:590`）：消耗从行为者账户转入 **WorldQiBudget 回充**（新 helper `qi_physics::ledger::credit_world_budget(reason, amount)` 之类），不再注水 audit-only 的 `QiAccountId::zone` 账户。选择预算而非直写 `zone.spirit_qi` 的理由：化虚成本 800 >> zone 满仓 50，直还必然大额 overflow；回环经预算再由 P1 滴灌分配。
- 保留 `QiTransfer` 审计（reason 不变：`MeridianOpen` / `Breakthrough`），to 账户改为预算侧表示——具体账户形态见 §8 #1。
- 处理历史注水：`summarize_world_qi` / `total_observed` 中 ledger zone 账户余额的既有语义偏差，迁移或重置方案见 §8 #1。
- **测试**：meridian_open / breakthrough 现有守恒测试改锁新流向（消耗后 `budget_current_total` 等额上升）；开脉→突破全链路总量不变的端到端守恒对拍；`LedgerUnavailable` 回滚分支保持（`breakthrough.rs:707`）。

## P1 平衡回流（低浓度、活流量）⬜

- `ZoneConfig` / `Zone` 加 `qi_equilibrium: f64`（默认 0.0 = 不回流，向后兼容）与 `qi_inflow_per_min: f64`；`zones.json` 给 spawn 配 `qi_equilibrium: 0.35`（> 开脉门槛 `MIN_ZONE_QI_TO_OPEN=0.3`）+ inflow 初值（标定见 §8 #2）。
- qi_physics 新增：`fn zone_equilibrium_inflow(spirit_qi, equilibrium, inflow_per_min, dt) -> f64` + 常数；**只补到 equilibrium 即停**（浓度钳制），来源扣 `WorldQiBudget`，预算不足则按余额缩量、绝不透支。
- heartbeat（`world/heartbeat.rs`）新增回流 system：直写 `zone.spirit_qi`（遵循 #676 后守恒写法：改真实池 + 记 `QiTransfer(ZoneInflow)` 审计），负灵域（spirit_qi<0）zone 不回流（保留死域/负灵域设定）。
- **测试**：钳制在 equilibrium 不过冲；预算耗尽缩量；负灵域跳过；回流↔吸收长跑总量守恒；`REALM_COLLAPSE` 事件 zone 是否回流见 §8 #5。

## P2 NPC 让灵地板 ⬜

- qi_physics 新常数 `QI_NPC_ABSORB_FLOOR = 0.3`（与 `MIN_ZONE_QI_TO_OPEN` 同值、独立常数，语义：NPC 只喝地板以上的溢出层，玩家开脉永远有底仓）。
- 落点：`apply_dormant_regen_with_multiplier`（`dormant/mod.rs:1435`，现行 `<= 0.0` 改 `<= FLOOR`）+ `cultivation/tick.rs` regen 的 NPC 分支（`NpcMarker` 判定）。玩家吸取不受地板约束。
- war_multiplier 路径（plan-offscreen-war-v1 P9）同样过地板，不给战事 zone 开后门。
- **测试**：NPC 在 floor±ε 边界停手/恢复；玩家不受限；dormant 批 tick 大规模跑不把带回流 zone 压到 floor 以下稳态。

## P3 固元突破窗口（灵潮 / 灵眼）⬜

baseline 0.35 永远过不了 `MIN_ZONE_QI_TO_GUYUAN=0.8`（`breakthrough.rs:357-390`），需要窗口机制。路线二选一或并行（§8 #3 拍板）：

- **灵潮**：复用伪灵脉 runtime（`pseudo_vein_runtime.rs`，phase 机 + settlement 已有守恒结算），周期性把目标 zone 短暂推到 0.85 再回落；qi 来源同样走 WorldQiBudget，不凭空。
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

## §10（升 active 时补）

scope 预估 4 PR（P0/P1/P2/P3 各一），按 docs/CLAUDE.md §六写实施工作流；P3 含视听资产走 3 轮打磨 + PROMISE 仅当新增资产（现倾向全复用伪灵脉资产则豁免）。
