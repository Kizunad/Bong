# Bong · plan-npc-virtualize-v3 · 骨架（条件占位）

NPC 隐式更新框架 **dormant 批量战争推演** —— 让 dormant NPC 之间能进行批量派系战斗、师承演变、社交博弈，**无需 hydrate 到 ECS**。承接 `plan-npc-virtualize-v2` ✅（三态 Drowsy 已落）+ `plan-npc-virtualize-v1` ✅（dormant 基本推演已落）。

> **⚠️ 条件 plan**：本 plan **仅在 `plan-npc-virtualize-v2` P3 决策门 #6 选 C 时启动**。触发条件：v2 P3 实测后，Drowsy/Dormant NPC 间互动（同 zone 碰撞 / 战斗意图 / 派系合并）复杂度超过天道 agent 演绎能力，需要服务端批量物理化。若天道 agent 推演足够（不触发 #6），本 plan 永久不启动。

**设计哲学**：worldview §十一 「散修江湖人来人往」+ §十三「末法时代」不只是玩家可见世界——远方 dormant NPC 之间的派系消长、战死、传承应有真实物理基础，而非全部委托给 agent 叙事。本 plan 在 dormant SoA 层面提供批量推演引擎，为 agent narration 提供真实数据支撑。

**worldview 锚点**：`worldview.md §十一:947-970`（散修江湖人来人往，背后必须有实际 NPC 流动）· `§三:124-187`（NPC 与玩家平等，远方战争也有真实结果）· `§二`（守恒律——战争中灵气损耗必须走 ledger，战死 NPC 释放灵气回 zone）

**交叉引用**：`plan-npc-virtualize-v1` ✅ · `plan-npc-virtualize-v2` ✅（前置）· `plan-npc-ai-v1` ✅（FactionStore / 6 archetype 战斗基线）· `plan-qi-physics-v1` P1 ✅（战争灵气损耗走 ledger）

---

## 接入面 Checklist

- **进料**：
  - `npc::dormant::NpcDormantStore`（v1 已建）
  - `NpcDormantSnapshot`（v1 已建，扩展战斗相关字段）
  - `FactionStore`（v1/npc-ai 已建）
  - `qi_physics::ledger::QiTransfer`（战争灵气损耗记账）
  - `qi_physics::qi_release_to_zone`（战死 NPC 灵气释放）
  - 天道 agent `NpcDigest` 通道（战争结果事件发布给 agent 做 narration）
- **出料**：
  - `dormant_combat_batch_system`（全局低频 tick，dormant NPC 之间战斗批量推演）
  - `dormant_social_batch_system`（师承演变 / 派系合并 / 离队批量推演）
  - `DormantCombatOutcomeEvent`（战斗结果：胜者 / 败者 / 死亡 / 领地变更）
  - `DormantFactionShiftEvent`（派系变更：合并 / 分裂 / 新生）
  - `bong:npc/dormant_war` Redis 通道（天道 agent 监听，用于 narration）
- **共享类型 / event**：
  - 复用 `FactionStore`（不新建 dormant 专属 faction 副本）
  - 复用 `bong:npc/death`（dormant 战死同通道）
  - 复用 `qi_physics::ledger`（战争灵气守恒）
- **跨仓库契约**：
  - server: `npc::dormant::warfare::*` 批量战争系统
  - agent: 新增 `bong:npc/dormant_war` 通道消费（narration 生成）
  - client: 无变化（战争发生在远方 dormant 层，玩家不可见；但 narration 会广播）
- **worldview 锚点**：§十一:947-970 · §三:124-187 · §二
- **qi_physics 锚点**：`qi_physics::ledger::QiTransfer`（战争伤害 = 灵气损耗，走 ledger）· `qi_physics::qi_release_to_zone`（战死释放）

---

## §0 设计轴心

**dormant 战争 ≠ ECS 战斗模拟**

dormant 层战争是**统计 / 概率引擎**，不是帧级物理战斗：
- 每次推演为一场"战役"（5-60 in-game 分钟），输出结果（胜/败/平/撤退/死亡数/领地变更）
- 结果由 faction 强度（战力评分：cultivation × 人数 × 地形系数）+ 随机因子决定
- 不模拟每一击，只输出最终状态 delta

**守恒律在战争层的实现**：
- 战死 NPC 的 qi_current 必须释放回所在 zone（走 `qi_release_to_zone`）
- 战争中灵气消耗（技能施法估算）按 faction 强度比例扣减，走 `QiTransfer`
- 领地变更后 zone 归属变更 → 后续 dormant NPC 修炼 zone 参数更新

---

## §1 阶段总览（草案）

| 阶段 | 内容 | 验收（草案） |
|------|------|------|
| **P0** ⬜ | **决策门 + 战争推演引擎设计**：faction 强度评分公式 + 战役周期定值 + dormant 战争 IPC schema（`bong:npc/dormant_war`）+ 守恒律约束清单 + 开放问题（§5）收口 | 设计文档 + P0 决策门全收口 |
| **P1** ⬜ | **批量战役推演 MVP**：`dormant_combat_batch_system`（低频 GlobalTick，每 in-game 30min 一轮）+ 战死通道 + ledger 守恒 | 1000 dormant NPC 跑 24h in-game，战死 NPC 灵气 ledger 守恒 |
| **P2** ⬜ | **派系演变**：`dormant_social_batch_system`（师承传承 / 派系合并 / 分裂）+ agent narration 接入 | 天道 agent 消费 `bong:npc/dormant_war` 产出可读 narration |
| **P3** ⬜ | **性能 + e2e**：5000 dormant NPC 混战 48h in-game，TPS ≥ 18 全程保持 | CI e2e green，战争结果分布符合 worldview 设定（化虚 NPC 获胜率 > 凡人）|

---

## §2 战争推演模型（草案）

```rust
// 每次推演产出的战役结果
pub struct DormantCombatOutcome {
    pub attacker_faction: FactionId,
    pub defender_faction: FactionId,
    pub result:           CombatResult, // Victory / Defeat / Retreat / Stalemate
    pub attacker_deaths:  u32,
    pub defender_deaths:  u32,
    pub territory_change: Option<ZoneId>,
    pub qi_released:      f64,          // 战死 NPC 的灵气总量（走 release_to_zone）
}

// 战力评分（P0 决策门确定公式）
fn faction_combat_score(faction: &FactionSnapshot, zone: &ZoneSnapshot) -> f64 {
    let base = faction.members.iter()
        .map(|m| realm_to_score(m.realm) * m.qi_current / m.qi_max)
        .sum::<f64>();
    let terrain_modifier = zone.spirit_qi / zone.spirit_qi_cap;
    base * terrain_modifier
}
```

---

## §3 守恒律约束（与 v1 §3 同级红线）

- **战死 NPC 灵气必须归还**：战死 dormant NPC 走 `qi_physics::qi_release_to_zone`，灵气回战场所在 zone
- **战争消耗走 ledger**：技能消耗估算按战力比例扣减，走 `QiTransfer { reason: CombatBattle }`
- **不允许战争创造灵气**：战胜方不增加 qi_current（灵气从败方战死 NPC 归 zone，再由 zone 被正常修炼吸收）
- **禁止战争期间 dormant NPC 直接扣 HP**：所有损耗通过战役结果 `deaths` 表达，死亡才扣除

---

## §4 IPC Schema（草案）

新增 Redis 通道 `bong:npc/dormant_war`，天道 agent 订阅用于生成远方战争 narration：

```typescript
// agent/packages/schema/npc_dormant_war.ts（草案）
const DormantWarEvent = Type.Object({
  event_type:       Type.Literal("dormant_war"),
  attacker_faction: Type.String(),
  defender_faction: Type.String(),
  result:           Type.Union([
                      Type.Literal("victory"),
                      Type.Literal("defeat"),
                      Type.Literal("retreat"),
                      Type.Literal("stalemate"),
                    ]),
  location_zone:    Type.String(),
  deaths:           Type.Number(),
  territory_gained: Type.Optional(Type.String()),
  timestamp_ingame: Type.Number(),
});
```

---

## §5 开放问题（P0 决策门收口）

1. **战役周期**：每 in-game 30min 推演一次 vs 每 in-game 1h 一次？频率影响 CPU 开销 + narration 密度
2. **战力评分公式**：纯境界加权 vs 境界 × 人数 × 地形系数三项？化虚 NPC 单人 vs 100 引气 NPC 的期望胜率是多少（worldview §三:187 ×5 质变 应有多强？）
3. **天道 agent 推演边界**：战争完全由 server 物理化 vs agent 演绎 + server 执行结果？v3 的存在意义取决于 agent 能否可靠覆盖这块
4. **领地变更对 zone 影响**：派系占领 zone 后 zone.spirit_qi_cap 是否变化？还是纯叙事层面的"归属"
5. **师承代际传承**：dormant 老 NPC 战死 → 是否自动培养继承人？与 plan-npc-ai-v1 §3.3 代际更替接口如何对齐

---

## Finish Evidence

（本 plan 完成全部阶段并 merge 后填写）
