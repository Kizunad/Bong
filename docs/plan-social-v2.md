# Bong · plan-social-v2 · 骨架

在 social-v1 的 Renown（fame / notoriety）基础上，增加**按具名势力分组的声望轨道**，并将**派系战争结果**接入信誉传播链——让战争不只是 zone 控制权的数字变化，而在 NPC 社会网络中留下"谁赢了那场仗"的口碑记录。

**来源**：`plan-faction-wars-v1` 骨架"反向被依赖"注记 `plan-social-v2`（信誉系统与战争结果联动，待立）

**前置条件**：
- `plan-social-v1` ✅ 已完成（Renown / fame / notoriety / RelationshipStore / NPC 信誉分级）
- `plan-faction-expansion-v1` ⬜ P2 完成（`FactionReputation` component + `FactionReputationDelta` 事件已落地）
- `plan-faction-wars-v1` ⬜ P2 完成（`FactionWarOutcome` 已落地）

**交叉引用**：`plan-social-v1` ✅（Renown / fame / notoriety / 暴露路径 / NPC 信誉分级体系）· `plan-faction-expansion-v1` ⬜ 骨架（`NamedFactionId` / `FactionReputation` / `FactionReputationDelta`，硬前置）· `plan-faction-wars-v1` ⬜ 骨架（`FactionWarOutcome` / `PlayerFactionRole`，硬前置）· `plan-narrative-political-v1` ✅（feud / pact / Renown 叙事事件通道）· `plan-identity-v1` ✅（identity 切换 / 声誉冻结）· `plan-npc-ai-v1` ✅（NPC 态度 → 行为）

**worldview 锚点**：
- **§十一:937 身份与信誉 / NPC 反应分级**：高 Renown → NPC 主动给情报；低 → 通缉——本 plan 把这个机制扩展到"战争英雄/叛徒"的口碑层
- **§十一 匿名系统**：战争中使用匿名 identity 的玩家，其 WarRole 绑定到 identity——切换 identity 时战争信誉冻结（沿用 identity-v1 冻结语义）
- **§三:124 NPC 平等**：NPC 战争参与者同样积累战功信誉，死亡时写入生平卷（death-lifecycle-v1 接口）

**qi_physics 锚点**：无直接真元计算，但口碑传播速度可与 zone 灵气浓度挂钩（高浓度 zone 信息流通更快——可选，P1 不实装）

---

## 接入面 Checklist

- **进料**：`FactionWarOutcome { war_id, winner, zone, npc_deaths, qi_redistributed }`（faction-wars P2）+ `PlayerFactionRole { player_id, faction, role: Member/Mercenary/Intercept/Neutral }`（faction-wars P1）+ `FactionReputationDelta`（faction-expansion P2）+ `Renown { fame, notoriety }`（social-v1）+ `IdentityId`（identity-v1，绑定战争记录到具体 identity）
- **出料**：`WarReputationRecord { war_id, player_id, identity_id, role, outcome: Won/Lost/Betrayed, fame_delta: i32, notoriety_delta: i32, per_faction_deltas: HashMap<NamedFactionId, i32> }` + `WarLegendEvent`（战功累积到达阈值，broadcast event）+ 更新后的 NPC 信誉反应（高战功玩家在战争相关 zone 获额外折扣 / 情报）
- **共享类型**：复用 social-v1 `SocialRenownDeltaEvent`（fame/notoriety）+ faction-expansion `FactionReputationDelta`（per_faction）；新增 `WarReputationRecord`（持久化到 `war_reputation` 表）
- **跨仓库契约**：server `bong:social/war_reputation`（新 Redis key，单场战争结算后 publish）；agent 订阅 `WarLegendEvent` 产生"某修士在派系战争中扬名"叙事；client 复用 social HUD（无新 CustomPayload）
- **worldview 锚点**：§十一 信誉分级 + 匿名系统 + §三 NPC 平等
- **qi_physics 锚点**：无直接引用（信誉是信息层，不是物理层）

---

## §0 设计轴心

- **战争 → 口碑三轨**：战争结算后，`PlayerFactionRole` 附加的参战记录触发三条信誉变化：
  1. **全局 Renown**：`fame`（赢方 Member/赢方 Mercenary）或 `notoriety`（Intercept 截胡者）增减
  2. **派系信誉**：`per_faction[winner_faction]`（Member 赢）/ `per_faction[loser_faction]`（Member 输）/ `per_faction[both]`（Intercept 被双方降）
  3. **战争记录**：`WarReputationRecord` 落盘，用于生平卷 + 天道 narration 可引用具体战功
- **NPC 口碑扩散**：战争结算后，winner_zone 内 NPC 在对话/叙事中以概率（P = 0.4）引用玩家战功标签（`war_hero::qingyun_2026_05` 之类），无需 agent 每次主动广播
- **截胡/叛变永久标记**：`role = Intercept` → 双方势力 `per_faction` -30 + `notoriety +20`；此标记写入 `WarReputationRecord`，不随 identity 切换清除（除非 `notoriety` 整体重置）
- **口碑衰减**：战争记录的 fame_delta 在 30 game-days 后衰减为 50%（仅 `WarReputationRecord` 的贡献部分，基础 Renown 不动）——末法残土没有持久的英雄史诗

---

## 阶段总览

| 阶段 | 状态 | 主要交付物 | 验收标准 |
|------|------|-----------|---------|
| **P0** | ⬜ | `WarReputationRecord` 数据模型 + 持久化 | `war_reputation` 表 PR 合并 + 战争结算触发正确写入 |
| **P1** | ⬜ | 三轨信誉更新 system + 口碑扩散 NPC 响应 | 战争胜出后 fame / per_faction / notoriety 正确计算 |
| **P2** | ⬜ | `WarLegendEvent` + agent 叙事 + 衰减 system | 30 game-days 后战功贡献衰减可观测 |

---

## P0 — 数据模型 + 持久化

- [ ] `WarReputationRecord`（`server/src/social/war_reputation.rs`）：
  - `{ war_id: WarId, player_id: EntityId, identity_id: IdentityId, role: WarRole, winner_faction: NamedFactionId, outcome: WarOutcomeTag, fame_delta: i32, notoriety_delta: i32, per_faction_deltas: [(NamedFactionId, i32); 3], recorded_tick: u64, decay_expires_tick: u64 }`
  - `WarOutcomeTag` enum：`WonAsMember / LostAsMember / WonAsMercenary / LostAsMercenary / Betrayed / Neutral`
- [ ] `war_reputation` 持久化表（`server/src/persistence/`）：`war_id TEXT, player_id TEXT, identity_id TEXT, outcome TEXT, fame_delta INTEGER, notoriety_delta INTEGER, per_faction_json TEXT, recorded_tick INTEGER, decay_expires_tick INTEGER`
- [ ] `bong:social/war_reputation` Redis publish：战争结算后 publish JSON `{ war_id, records: [...] }`（agent 叙事消费）
- [ ] ≥ 8 单测（各 WarOutcomeTag 对应 delta 值域正确 / 持久化写入读取往返 / Intercept 双方 per_faction 均为负 / Neutral 无 delta）

**P0 验收**：战争结算 e2e → `war_reputation` 表有记录 + Redis 事件 publish green

---

## P1 — 三轨信誉 system + NPC 口碑扩散

- [ ] `apply_war_reputation_system`（`server/src/social/war_reputation.rs`，FixedUpdate，在 `faction_war_settle_system` 之后）：
  - 消费 `FactionWarOutcome`；查 `PlayerFactionRole` 归档；emit `SocialRenownDeltaEvent`（fame/notoriety）+ `FactionReputationDelta`（per_faction）
  - 数值参考（P0 决策门收口）：Member 赢 → fame +30, winner_faction +20；Member 输 → fame 不变, loser_faction -10；Mercenary 赢 → fame +15（不绑派系）；Intercept → notoriety +20, both factions -30
- [ ] NPC 口碑扩散：`war_reputation_npc_response_system`（zone-scoped，每次 hydrated NPC 交互时以概率 P=0.4 检查当前 zone 的最近战争记录）：
  - 检查玩家 `WarReputationRecord.winner_faction == zone_anchor_faction`：有战功 → NPC 对话附加 war_hero tag（如"上次的仗你打得不错"）
  - `WarOutcomeTag::Betrayed` → NPC 拒绝服务提示（"盐商会里的人都说你这种人不可信"）
- [ ] ≥ 12 单测（三轨 delta 各 WarOutcomeTag 正确 / NPC P=0.4 响应触发条件 / Betrayed NPC 拒绝逻辑 / identity-v1 切换后 WarReputationRecord 绑定旧 identity，新 identity 无战功）

**P1 验收**：e2e — 玩家 Member 赢战争 → `/social status` 显示 fame 上升 + winner_faction per_faction 上升

---

## P2 — WarLegendEvent + agent 叙事 + 衰减

- [ ] `WarLegendEvent { player_id, identity_id, cumulative_wins: u32, trigger_fame: i32 }`：`fame` 因战争累计超过阈值（推 100）时 emit，agent 消费并 broadcast narration：
  - "有人在战场上出了名——青云残峰的猎盟散修都在传那个无名修士的事。"
  - "沧渊商会那边传来消息：有个家伙连赢三仗，已经有人想找他谈价钱了。"
- [ ] `war_reputation_decay_system`（每 game-day tick 运行）：查 `WarReputationRecord.decay_expires_tick <= current_tick` → 对应 `SocialRenownDeltaEvent { fame_delta: -(original_delta / 2) }` 补偿（只回扣战争贡献部分，基础 fame 不动）
- [ ] `bong:social/war_reputation` subscribe + narration 模板（scope: broadcast / zone，style: narrative）：3 条已列于上
- [ ] ≥ 8 单测（WarLegendEvent 触发阈值 / 衰减 system 正确计算 decay_expires_tick / agent 模板至少一条对应 WarOutcomeTag::Betrayed 的叙事格式）

**P2 验收**：模拟 30 game-days → 战功 fame 贡献减半可观测（`/social status` 前后对比）

---

## §8 开放问题（P0 决策门前需收口）

1. **信誉数值校准**：Member 赢 +30 fame 是否过高（player 快速刷信誉）？需结合 faction-wars 战争频率（FACTION_WAR_THRESHOLD）估算每 real-day 战争场数再拍
2. **截胡永久标记**：`WarOutcomeTag::Betrayed` 目前不随 identity 切换清除——是否应该允许切 identity 洗白（一致性 vs 代价，需与 identity-v1 决策对齐）
3. **口碑扩散概率 P=0.4**：是否合适？过高则 NPC 每次见面都说战争，沉浸感过重；过低则玩家感知不到口碑效果。待 faction-wars P3 上线后 telemetry 观察
4. **WarLegendEvent 阈值**：cumulative_wins 100 fame 是否合理？若单场 +30，约 3-4 场战争触发——频率与 narration 广播噪音需平衡
5. **多 identity 多战争记录**：同一玩家多个 identity 各参与不同战争，生平卷展示方式（按 identity 分组 vs 时间线合并）？留 plan-identity-v2 决策

**P0 启动前必须**：核查 `faction-expansion-v1` P2 中 `FactionReputationDelta` 接口已稳定；核查 `faction-wars-v1` P1 中 `PlayerFactionRole` 存储路径；核查 social-v1 `SocialRenownDeltaEvent` 的 consumer 链路不会被本 plan 二次触发（防止 fame 重复计算）。
