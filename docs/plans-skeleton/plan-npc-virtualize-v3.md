# Bong · plan-npc-virtualize-v3 · 骨架

NPC 虚拟化**派系战争批量推演**——在 v1 二态框架基础上，实装 dormant↔dormant 敌对派系 NPC 互殴的批量推演（死亡 → release 灵气 → zone 竞争），让世界线在玩家不在场时仍有真实派系势力消长。

**前置条件**（派生自 plan-npc-virtualize-v1 §8 决策门 #6 选项 C）：v1/v2 上线后以下情况激活：
- 运营数据显示 dormant 派系势力长期静止（agent 推演无法覆盖批量战争叙事）
- worldview §十一 散修江湖"各派系势力消长"需要 dormant 内部真实死亡作为物理基础
- plan-faction-wars（待立）启动时需要 dormant 批量结算支撑

**交叉引用**：`plan-npc-virtualize-v1.md` ✅（dormant SoA + NpcDormantStore + 守恒律底盘）· `plan-npc-ai-v1.md` ✅（FactionStore + FactionMembership + `faction::hostile_encounters`）· `plan-qi-physics-v1.md` P1 ✅（ledger + release）· `plan-agent-v2.md` ✅（NpcDigest + 天道长期推演）

**worldview 锚点**：
- **§十一:947-970 散修江湖**：派系势力消长 = "人来人往"的物理化身；dormant NPC 死亡推演让世界在玩家不在场时仍真实演进
- **§二 守恒律**：dormant 战斗死亡 release 灵气必须走 `qi_physics::qi_release_to_zone`；**不允许 dormant 死亡把灵气凭空消失**
- **§三:124-187 NPC 与玩家平等**：dormant NPC 死于派系战争 = 与玩家死亡同等规则（release 灵气 / emit `bong:npc/death` / 生平卷写入）

**qi_physics 锚点**：
- `qi_physics::qi_release_to_zone(amount, from, zone, zone_current, zone_cap)` — dormant 死亡灵气归还 zone
- `qi_physics::ledger::QiTransfer` — 所有灵气转移记账（reason: `CombatDeath`）
- 多 NPC 同 zone 死亡时：sequential release（先 release 的 zone_qi 回升 → 后 NPC 输入更高 zone_qi → 符合守恒律，zone 不溢出）

**前置依赖**：
- `plan-npc-virtualize-v1` ✅ — dormant SoA + 批量 tick 框架 + 守恒律底盘
- `plan-npc-ai-v1` ✅ — FactionStore / FactionMembership（敌对关系通过 FactionStore 管理）/ `faction::assign_hostile_encounters` scorer
- `plan-qi-physics-v1` P1 ✅ — release API 冻结

**反向被依赖**：
- `plan-narrative-political-v1` — 派系战争叙事（agent 以死亡数据为素材生成 narration）
- `plan-faction-wars`（待立）— 玩家可参与的派系战争需要 dormant 批量结算作为 NPC 死亡基础

---

## 接入面 Checklist

- **进料**：`NpcDormantStore`（char_id → NpcDormantSnapshot）+ `FactionStore`（faction 归属 + 敌对关系）+ `FactionMembership`（NPC 派系 component）+ 敌对 NPC 空间分布（按 zone 聚合）
- **出料**：`DormantCombatOutcome { winner: CharId, loser: CharId, zone: ZoneId, qi_released: f64 }` + `bong:npc/death`（loser 死亡 event）+ zone.spirit_qi 更新（ledger QiTransfer）+ NpcDormantStore 删除死亡 NPC
- **共享类型**：复用 `FactionStore`（敌对判定）/ `QiTransfer` / `bong:npc/death` schema；新增 `DormantCombatOutcome` event
- **跨仓库契约**：server `bong:npc/death` channel（已有）新增 `from_dormant_combat: bool` flag；agent NpcDigest 通道自然包含死亡后消失的 NPC（无需额外通道）
- **worldview 锚点**：§十一 派系消长 + §三 NPC 平等 + §二 守恒律
- **qi_physics 锚点**：`qi_release_to_zone` / `QiTransfer`

---

## §0 设计轴心

- **v1 极简原则尊重**：v1 §3 决策门 #6 选 A（"全权交天道 agent 推演"）是 v1 MVP 决策；v3 在 v1 上线 + v2 完成 + 派系战争叙事需要之后才激活
- **批量推演不是完整战斗**：dormant 战斗不走 big-brain Action / 全 ECS 路径，只是：同 zone 内 Hostile NPC 双方 → 概率伤害 roll（按境界差）→ 低境界 NPC 概率死亡 → release 灵气。**没有 VFX / 无 client 可见**
- **不允许 dormant 直接扣 HP**：dormant 状态只有"死/活"二元，没有 HP 扣减过程；`roll_combat_death` 直接返回 `Option<CharId>` 死亡者
- **守恒律零容忍**：每次 dormant 死亡 release 必须有对应 `QiTransfer`，缺失 = 阻塞 merge

---

## 阶段总览

| 阶段 | 状态 | 主要交付物 | 验收标准 |
|------|------|-----------|---------|
| **P0** | ⬜ | 决策门确认 + 批量推演数据模型 | 触发条件成立 + 数据模型 PR 合并 |
| **P1** | ⬜ | `dormant_combat_system` + 守恒律死亡 release | 500 dormant 1h in-game 灵气守恒；死亡 NPC 正确删除 |
| **P2** | ⬜ | 天道 agent 派系战争叙事 + zone 竞争可观测 | agent narration 包含派系消长叙事；zone spirit_qi 可见变化 |

---

## P0 — 决策门 + 数据模型

**派生触发验收**（满足任一）：
- [ ] v1 上线后天道 agent 推演能力不足以覆盖派系消长叙事（评估期 ≥ 4 weeks）
- [ ] plan-faction-wars 立项需要 dormant 批量死亡支撑
- [ ] worldview 测试中"派系势力静止"违和感明显

**数据模型**：
- [ ] `DormantCombatOutcome { winner: CharId, loser: CharId, zone: ZoneId, qi_released: f64, tick: u64 }` event（`server/src/npc/virtualize/dormant_combat.rs`）
- [ ] `roll_combat_death(a: &NpcDormantSnapshot, b: &NpcDormantSnapshot) -> Option<CharId>` 纯函数（按 realm 差 + RNG）：境界差 1 → 低境界 60% 死亡；境界差 2+ → 低境界 85% 死亡；同境界 → 各 50%
- [ ] `dormant_combat_batch_system` 定时触发策略（P0 决策门收口）：选项 A = 每次 `dormant_global_tick` 结束后跑（1/min）；选项 B = 独立低频 timer（每 in-game 10min）
- [ ] ≥ 10 单测（`roll_combat_death` 境界差概率覆盖 / 灵气守恒：死亡 release == QiTransfer amount / 死亡 NPC 从 Store 删除 / 空 zone 无战斗）

**P0 验收**：数据模型 + `roll_combat_death` 纯函数 + 10 单测 green

---

## P1 — dormant_combat_system + 守恒律

- [ ] `dormant_combat_batch_system`（`server/src/npc/virtualize/dormant_combat.rs`）：
  - 遍历所有 zone，收集 zone 内 Hostile faction pair 的 dormant NPC
  - 同 zone 每对 Hostile NPC → `roll_combat_death` → 获胜者存活 / 失败者：
    1. `qi_physics::qi_release_to_zone(snapshot.qi_current, from_dormant, zone, ...)` + emit `QiTransfer`
    2. emit `bong:npc/death`（含 `from_dormant_combat: true` flag）
    3. `NpcDormantStore.remove(char_id)`
  - 每 zone 最多 N 次战斗 / tick（防止单 tick 大量死亡，N = P0 决策门）
- [ ] `bong:npc/death` schema 扩展 `from_dormant_combat: bool` 字段（`agent/packages/schema/samples/npc_death_v2.json`）
- [ ] ≥ 20 单测（批量死亡 + 守恒律 / 空 zone / 单 faction 无战斗 / 达战斗上限 N 截断 / NpcDormantStore 正确缩减）

**P1 验收**：500 dormant 1h in-game qi 守恒（zone 收支 == ledger 累计）

---

## P2 — 天道 agent 整合 + zone 竞争可观测

- [ ] 天道 agent 消费 `bong:npc/death`（`from_dormant_combat: true`）生成派系消长 narration（broadcast scope）
- [ ] zone.spirit_qi 变化可观测：`/zone_qi list` 命令显示战斗后 zone spirit_qi 上升（战斗死亡 release 灵气）
- [ ] ≥ 3 e2e 测试（agent 收到死亡 event → 发出 narration / zone spirit_qi 正确上升 / 玩家进入战场 zone 看到 spirit_qi 高于预期）

**P2 验收**：agent narration 包含"某派系在某 zone 折损数名散修"类型叙事

---

## §8 开放问题（P0 决策门收口）

1. **战斗频率**：每 dormant_global_tick（1/min）跑 vs 独立低频 10min 一轮（前者更活跃但 zone 灵气波动频繁）
2. **单 zone 每 tick 战斗上限 N**：N=3 vs N=5 vs 无上限（影响 zone 灵气波动幅度 + server 批量 roll 开销）
3. **同境界战斗 50/50**：是否加熟练度 / 真元当前值修正（更真实 vs 纯随机更简单）
4. **死亡 NPC 生平卷**：dormant 战死是否写完整 BiographyEntry（开销 vs 叙事完整性）
5. **docs/CLAUDE.md §四 红旗**：是否加"dormant 战斗死亡未走 release 守恒"红旗独立一条
