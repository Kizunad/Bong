# Bong · plan-faction-wars-v1 · 骨架

**玩家可参与的派系战争**——在 plan-npc-virtualize-v3 dormant 批量死亡推演的基础上，让玩家能够主动参与、影响甚至发起派系战争，实现 worldview §十一"散修江湖各派系势力消长"的玩家层面物理化身。核心设计：玩家通过宣誓加入、资助、偷袭或谈判等方式影响派系战争进程，胜负结果改写 zone 控制权 + spirit_qi 分配。

**前置条件**（全部满足才可启动）：
- `plan-npc-virtualize-v1` ✅ — FactionStore + FactionMembership + dormant SoA 底盘
- `plan-npc-virtualize-v3` 上线 — dormant 批量战斗死亡推演（为派系战争提供 NPC 死亡物理基础）
- `plan-social-v1` ✅ — 声望/关系系统（玩家-派系关系锚点）

**交叉引用**：`plan-npc-virtualize-v1` ✅（FactionStore / FactionMembership / dormant SoA）· `plan-npc-virtualize-v3` active（dormant 批量战斗推演）· `plan-social-v1` ✅（声望/关系）· `plan-npc-ai-v1` ✅（big-brain Utility AI）· `plan-qi-physics-v1` P1 ✅（守恒律 / zone spirit_qi）· `plan-agent-v2` ✅（天道 NpcDigest 通道）

**worldview 锚点**：
- **§十一:947-970 散修江湖**：各派系势力消长是"人来人往"的政治物理化身；玩家参与战争是主动推动势力变化
- **§二 守恒律**：战争死亡 release 灵气走 `qi_physics::qi_release_to_zone`；zone 控制权变化不凭空改变 spirit_qi 总量
- **§三:124 NPC 与玩家平等**：玩家死于派系战争走相同死亡结算路径（plan-death-lifecycle-v1）

**qi_physics 锚点**：
- zone 控制权转移后，胜利方 NPC 的 `qi_physics::regen_from_zone` 在该 zone 享受 10% 加成（不新增物理常数，通过 `faction_control_bonus_rate` 乘子注入现有 regen 路径）
- 所有死亡灵气释放走 plan-npc-virtualize-v3 已有的 `qi_release_to_zone` + `QiTransfer { reason: CombatDeath }`

---

## 接入面 Checklist

- **进料**：`FactionStore`（派系归属 + 敌对关系）+ `NpcDormantStore`（dormant NPC 快照）+ `DormantCombatOutcome`（v3 战斗结果）+ 玩家声望（plan-social-v1 `ReputationComponent`）+ zone spirit_qi（qi_physics）
- **出料**：`FactionWarState { zone, attacker, defender, phase, progress }` resource + `FactionControlChange { zone, old_faction, new_faction }` event + zone 控制权 flag（影响 spirit_qi regen 加成）+ `bong:faction_war` Redis channel（agent 消费）
- **共享类型**：复用 `FactionStore` / `DormantCombatOutcome` / `bong:npc/death`（扩展 `war_context` field）；新增 `FactionWarState` / `FactionControlChange`
- **跨仓库契约**：server `bong:faction_war` 新 Redis channel（战争开始/进度/结束）；agent NpcDigest 通道新增战争状态摘要；client HUD 可选显示当前 zone 控制方 + 战争进度
- **worldview 锚点**：§十一 散修江湖势力消长 + §二 守恒律 + §三 平等
- **qi_physics 锚点**：死亡 release（v3 复用）+ 胜利方 regen 加成（新增 `faction_control_bonus_rate` 注入点）

---

## 阶段总览

| 阶段 | 状态 | 主要交付物 | 验收标准 |
|------|------|-----------|---------|
| **P0** | ⬜ | 派系战争数据模型 + 触发条件 + zone 控制权系统 | `FactionWarState` 状态机 + v3 事件集成 + 15 单测 |
| **P1** | ⬜ | 玩家参与机制（宣誓/资助/偷袭）+ 声望影响 | 玩家行为正确修改战争进度；声望正确变化 |
| **P2** | ⬜ | 胜负结算 + zone 控制权转移 + agent 叙事 | zone 控制方变化；agent 生成"某派系占领某 zone" narration |
| **P3** | ⬜ | 视听完整包 + 饱和测试 + 集成联调 | e2e 玩家参与战争全流程 |

---

## P0 — 数据模型 + zone 控制权系统

- [ ] `FactionWarState { zone: ZoneId, attacker: FactionId, defender: FactionId, phase: WarPhase, attacker_strength: f64, defender_strength: f64, started_tick: u64 }` resource（`server/src/faction/war.rs`）
- [ ] `WarPhase` enum：`{ Brewing, Active, Decisive, Concluded }`
- [ ] `ZoneFactionControl { zone: ZoneId, controlling_faction: Option<FactionId>, control_strength: f32 }` resource（新增）
- [ ] 战争触发条件：v3 `DormantCombatOutcome` 累计——同 zone 内一个派系 dormant 死亡数 > N（P0 决策门收口）→ 触发 `FactionWarState::Brewing`
- [ ] ≥ 15 单测（各 WarPhase 转换 / zone 控制权初始化 / 守恒律：战争死亡 release 全部走 QiTransfer）

## P1 — 玩家参与机制

- [ ] 玩家宣誓加入：`/faction join <faction_id>` → 修改 `FactionMembership` + 声望绑定
- [ ] 玩家资助：消耗骨币/灵石 → attacker/defender strength +X（骨币价格锚定 worldview §八 经济）
- [ ] 玩家偷袭：直接攻击敌方 NPC → 伤害折算进 war strength；走 plan-combat-no_ui 战斗路径
- [ ] 声望影响：同派系战士存活 → 玩家声望 +；战败 → 玩家声望 -（复用 plan-social-v1 reputation API）

## §7 开放问题

1. **战争触发阈值 N**：同 zone 敌对派系 dormant 死亡多少次触发战争？（v3 决策门收口后再定）
2. **玩家中立选项**：是否允许玩家以"雇佣兵"身份参与任意方而不影响声望？
3. **zone 控制时长**：控制方 spirit_qi 加成持续多久？战争结束还是直到下次被夺？
4. **多 zone 同时战争**：同一派系能同时在多个 zone 开战吗？（资源摊薄 vs 真实感）
5. **天道介入**：长期战争是否触发天道"降劫"清场机制（参 plan-tribulation-v2 灾劫设计）？

---

> 骨架创建日期：2026-05-21。派生自 plan-npc-virtualize-v3 §0（`plan-faction-wars（待立）`）及 §前置依赖说明。
