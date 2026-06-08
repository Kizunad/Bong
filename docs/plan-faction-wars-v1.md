# Bong · plan-faction-wars-v1 · active

玩家可参与的**派系战争**——在 npc-virtualize-v3 dormant 批量战斗基础上，实装玩家可加入/佣兵/截胡的派系战争事件，让 worldview §十一"散修江湖派系势力消长"有玩家参与渠道。v1 以现有 `FactionId::Attack / Defend / Neutral` 为基础，扩展战争事件生命周期（宣战 → 野战 → 结算 → 余波），玩家可选择投靠、雇佣、背刺或旁观。

**前置条件**（派生自 plan-npc-virtualize-v3 §8 开放问题）：以下任一满足时启动：
- npc-virtualize-v3 dormant 批量战斗 P1 上线后，运营数据显示 dormant 派系 zone 势力长期静止（玩家参与感缺失）
- plan-faction-wars 需要 dormant 批量结算作为 NPC 死亡基础已明确准备好
- worldview §十一 散修江湖"各派系势力消长"需要玩家可交互的事件钩子

**交叉引用**：`plan-npc-virtualize-v1.md` ✅（FactionStore / FactionMembership / dormant SoA）· `plan-npc-virtualize-v3.md` ⬜（dormant 批量战斗，本 plan 硬前置）· `plan-npc-ai-v1.md` ✅（`assign_hostile_encounters` / big-brain）· `plan-narrative-political-v1.md` ✅（政治叙事五核心事件：feud/pact/灵龛抄家/通缉令/Renown）· `plan-social-v1.md` ✅（Renown + NPC 信誉体系）· `plan-qi-physics-v1.md` ✅（守恒律 + QiTransfer）

**worldview 锚点**：
- **§十一:947-970 散修江湖 / NPC 反应分级**：派系战争 = worldview 里"势力消长"的物理机制；玩家 Renown 高 → 被主动招募；Renown 低 → 只能做佣兵或旁观
- **§三:124 NPC 与玩家平等**：玩家投靠派系 = NPC 同等规则，承担相同战争风险（被敌派系通缉）
- **§二 守恒律**：战争死亡释放灵气必须走 `qi_physics::qi_release_to_zone`（继承 npc-virtualize-v3 规则）
- **§十一 匿名系统**：战争期间玩家身份可隐藏（`identity::hidden` 状态），被识破则关联到 faction

**qi_physics 锚点**：
- zone 控制权变化不产生灵气——只改变灵气**流向**；战胜方 zone 的 `spirit_qi` 吸收率提升走 `qi_physics::regen_from_zone` 修正系数
- dormant 战死灵气释放继承 npc-virtualize-v3 的 `qi_release_to_zone` + `QiTransfer(reason: CombatDeath)`

**前置依赖**：
- `plan-npc-virtualize-v1` ✅ — FactionStore / FactionMembership / dormant SoA
- `plan-npc-virtualize-v3` ⬜ — dormant 批量战斗（死亡 event + qi release 底盘）**硬前置**
- `plan-npc-ai-v1` ✅ — assign_hostile_encounters scorer（hydrated NPC 战斗逻辑已跑通）
- `plan-narrative-political-v1` ✅ — feud/pact 事件钩子（派系战争叙事消费此通道）

**反向被依赖**：
- `plan-faction-expansion-v1`（待立，具名散修势力 + 宗门系统）
- `plan-social-v2`（信誉系统与战争结果联动）

---

## 接入面 Checklist

- **进料**：`FactionStore`（Attack/Defend/Neutral + `is_hostile_pair`）+ `FactionMembership`（NPC 派系归属）+ `NpcDormantStore`（dormant NPC 数量/zone 分布）+ `DormantCombatOutcome`（npc-virtualize-v3 产出，战死 event）+ `Renown`（social-v1 玩家声望）+ `QiTransfer` ledger
- **出料**：`FactionWarEvent { war_id, attacker, defender, zone, phase, deadline_tick }` + `PlayerFactionRole { player, faction, role: Member/Mercenary/Neutral }` + `FactionWarOutcome { winner, zone, ki_delta }` + narration event（政治叙事通道）
- **共享类型**：复用 `FactionStore` / `FactionMembership` / `QiTransfer`；新增 `FactionWarEvent` / `PlayerFactionRole` / `FactionWarOutcome`
- **跨仓库契约**：server `bong:faction_war` Redis pub（agent 消费叙事）；client 新 `FactionWarHudLayer`（战争状态提示）；agent 消费 `FactionWarOutcome` 生成派系消长 narration
- **worldview 锚点**：§十一 散修江湖 + §三 NPC 平等 + §二 守恒律
- **qi_physics 锚点**：死亡走 npc-v3 的 `qi_release_to_zone`；zone 控制权走 `regen_from_zone` 修正系数

---

## §0 设计轴心

- **战争触发**：npc-virtualize-v3 dormant 批量战斗的死亡累计超阈值 → `FactionWarDeclarationEvent`（由 server 自动触发，不需要 admin 介入）
- **战争阶段**：宣战（Declaring）→ 野战（Active：dormant + hydrated NPC 互战）→ 结算（Settling：超时或一方 NPC 耗尽）→ 余波（Aftermath：zone 控制权转移 + 信誉更新）
- **玩家参战选项**（四选一，互斥）：
  1. **投靠**（Member）：与 faction 共享胜负，Renown ≥ 100 才被邀请；胜则 Renown 大涨 + zone 灵气分成；败则被敌派系通缉
  2. **佣兵**（Mercenary）：无条件参战 + 击杀计费（骨币 / 时间），战后身份不绑定
  3. **截胡**（Intercept）：两派系在 Active 阶段最弱时趁乱劫资源，被双方视为共同敌人
  4. **旁观**（Neutral）：保持匿名，不受通缉但也不得收益
- **Zone 控制权**：战后胜方 zone 的 `spirit_qi` 基础吸收率 +5-15%（通过 `regen_from_zone` 修正系数），败方 NPC 人口骤减

---

## 阶段总览

| 阶段 | 状态 | 主要交付物 | 验收标准 |
|------|------|-----------|---------|
| **P0** | ⬜ | 决策门确认 + 战争事件数据模型 | npc-virtualize-v3 P1 上线 + `FactionWarEvent` 数据模型 PR 合并 |
| **P1** | ⬜ | 战争生命周期 system + 玩家参战选项 server 逻辑 | 战争事件 declare → active → settle e2e |
| **P2** | ⬜ | Zone 控制权 + Renown 联动 + 战争结算 | 胜方 zone 灵气吸收率正确更新 |
| **P3** | ⬜ | HUD 提示 + narration + agent 整合 | agent 消费 FactionWarOutcome 生成派系消长叙事 |

---

## P0 — 决策门 + 数据模型

**派生触发验收**（满足任一）：
- [ ] npc-virtualize-v3 P1 完成（dormant 批量战斗 system 运行）
- [ ] 运营 / 测试数据确认"派系势力长期静止"违和感 or plan-faction-expansion 立项需要此底盘

**FactionWarEvent 数据模型**：
- [ ] `FactionWarEvent { war_id: WarId, attacker: FactionId, defender: FactionId, zone: ZoneId, phase: WarPhase, declared_tick: u64, deadline_tick: u64 }` （`server/src/npc/faction_war.rs`）
- [ ] `WarPhase` enum：`Declaring / Active / Settling / Aftermath`
- [ ] `PlayerFactionRole { player_id: EntityId, faction: FactionId, role: WarRole }` component；`WarRole` enum：`Member / Mercenary / Intercept / Neutral`
- [ ] `FactionWarOutcome { war_id, winner: FactionId, zone, npc_deaths: u32, qi_redistributed: f64 }` struct
- [ ] 战争触发条件：同 zone 内双方 dormant NPC 死亡累计 ≥ FACTION_WAR_THRESHOLD（P0 决策门，推 10 次 / game-week）
- [ ] ≥ 8 单测（触发条件正确 / FactionWarEvent 序列化 / WarPhase 转换正确 / Neutral 不触发战争）

**P0 验收**：数据模型 PR 合并 + 8 单测 green

---

## P1 — 战争生命周期 system

- [ ] `faction_war_declare_system`（FixedUpdate，`server/src/npc/faction_war.rs`）：
  - 扫描所有 zone 的 dormant 死亡事件（订阅 `DormantCombatOutcome`）
  - 累积超阈值 → emit `FactionWarEvent { phase: Declaring }`
  - 发布 `bong:faction_war` Redis channel
- [ ] `faction_war_active_system`：Active 阶段 hydrated NPC 优先攻击敌派系目标（调整 `assign_hostile_encounters` scorer 权重）
- [ ] 玩家参战接口：`/faction join <attack|defend>` / `/faction mercenary` / `/faction intercept`（brigadier 命令，dev-only 入口 + 正式 gameplay UI P3 补）
- [ ] 战争截止时间：`deadline_tick` 超时 → 强制 Settling 阶段（人口少的一方判负）
- [ ] narration 模板（scope: broadcast / zone，style: perception）：
  - "某处战鼓声传来——攻讨派与护山派于青云残峰开战，战场方圆三百格。"（宣战，broadcast）
  - "这片废墟上的血腥味还没散尽，看来战事激烈。"（Active 阶段，zone perception）
  - "远处尘埃落定——攻讨派在青云残峰占得先机，护山派散修向南撤退。"（结算，broadcast）
- [ ] ≥ 15 单测（宣战触发 / Active → 超时 Settling / 玩家加入 Member 被记录 / 截胡者被双方标记敌对 / 守恒律：战死 qi release 有对应 QiTransfer）

**P1 验收**：war declare → active → settle 完整 e2e（无玩家参与的纯 NPC 战争）green

---

## P2 — Zone 控制权 + Renown

- [ ] `faction_war_settle_system`：
  - 统计双方存活 dormant + hydrated NPC 数量 → 多者为胜方
  - 胜方 zone：`regen_from_zone` 基础修正 +10%（写入 `ZoneRegistry` / `ZoneSpiritBonus { faction: FactionId, bonus: f64 }`）
  - 败方 zone：修正 -5%（灵气吸收效率下降）
  - emit `FactionWarOutcome` + 更新 `FactionStore.loyalty_bias`
- [ ] Renown 联动：
  - Member 赢 → Renown += 50（走 social-v1 Renown 接口）
  - Mercenary 参战但输 → 无 Renown 奖惩（只看击杀数计费）
  - Intercept 截胡 → 双方 Renown -30（被双方标为"卖战者"）
- [ ] ≥ 8 单测（Zone 修正正确计算 / loyalty_bias 更新 / Renown 正确加减 / 多 zone 战争互不干扰）

**P2 验收**：战后 zone spirit_qi 基础修正可观测（`/zone_qi list` 显示 bonus 变化）

---

## P3 — HUD + narration + agent 整合

- [ ] `FactionWarHudLayer`（client，`client/src/hud/faction_war_hud.java`）：
  - 当前 zone 内有激活战争时显示战争状态 mini-tag（"战区"标识 + 双方存活 NPC 比例条）
  - 玩家当前 WarRole 显示（Member/Mercenary/Intercept 时显示身份标）
  - 战争倒计时（距 deadline_tick）
  - HUD 视觉：`#C04020`（攻）vs `#2040C0`（守）双色血条，透明度 60%，宽 80px
- [ ] 天道 agent 消费 `bong:faction_war`（outcome）生成派系消长叙事（broadcast，style: narrative）：
  - "攻讨派吞并青云残峰，此后数月，那片高地上再不见护山散修的身影。"
  - "两派在荒原上打得两败俱伤，灵脉无主，有心人的机会来了。"
- [ ] ≥ 5 e2e 测试（agent 消费 outcome event / HUD 正确渲染战区标识 / Member 赢战后 HUD Renown 变化）

**P3 验收**：e2e 手测——NPC 战争结算 → agent narration → 玩家 HUD 战区消失（outcome = Aftermath）

---

## §8 开放问题（P0 决策门收口）

1. **FactionId 扩展**：Attack/Defend/Neutral 是 MVP 简化；v2 是否扩展为具名散修势力（青云残峰猎人会 / 沧渊盐商帮等）？v1 不改变 FactionId enum，只使用现有变体
2. **多 zone 战争**：同时多个 zone 有战争时，NPC 跨 zone 增援是否实装（v1 仅同 zone 内战）
3. **玩家参战上限**：同一场战争允许多少玩家加入（防止大量玩家倒向一方一击必杀）
4. **战争频率**：dormant 触发战争的阈值（10 次/game-week）是否导致战争过于频繁，影响 zone 稳定性
5. **截胡机制边界**：Intercept 截胡能劫什么资源（骨币 / 战死 NPC 掉落物 / zone 灵脉控制权临时占领）
