# Bong · plan-npc-virtualize-v3

NPC 隐式更新框架 **dormant↔dormant 大规模战争批量推演**。占位骨架，由 plan-npc-virtualize-v1 决策门 #6「dormant NPC 间互动」选 **C（server 侧自主批量推演）** 时启动。

**激活前提**：
- `plan-npc-virtualize-v1` ✅（dormant SoA 架构基础实装）
- `plan-npc-virtualize-v2` ✅（三态稳定，防止 race condition 复杂化）
- 天道 agent 侧 dormant 推演（v1 决策门 #6 A/B 轻量路径）已满足不了大规模冲突叙事需求——即：派系战争 / 资源争夺在"全权交天道 agent 推演"模式下，延迟或精度无法满足 worldview §十一「散修江湖」叙事丰度要求

| 阶段 | 内容 | 状态 | 验收日期 |
|------|------|------|----------|
| P0 | 决策门 #6 评估 + 战争推演架构选型 | ⬜ | — |
| P1 | `DormantBattleBatch` 推演引擎 | ⬜ | — |
| P2 | 战争结果写 `DormantNpcSnapshot` + 叙事事件 emit | ⬜ | — |
| P3 | e2e 验收（1000 dormant 战争推演 1h in-game 守恒） | ⬜ | — |

---

## 背景 / v1 决策门 #6 回顾

plan-npc-virtualize-v1 §8 开放问题 #6 「dormant↔dormant 互动边界」设计了三条路径：
- **A（最轻）**：全权交天道 agent 推演，server 只读结果写 snapshot
- **B（中间）**：agent 推演 + server 侧简化冲突解算（伤害数字）
- **C（最重）**：server 侧自主批量战争推演，agent 仅出 narrative 叙事层

v1 选了 A 路径作为 MVP。本 plan 在 A 路径不够用时，补充 C 路径实装。

---

## 接入面 Checklist

- **进料**：
  - `plan-npc-virtualize-v1` ✅：`DormantNpcSnapshot`（含 `cultivation` / `faction` / `position` / `meridian_severed`）/ `GlobalTick` 批量推演 scheduler / `qi_physics::ledger::QiTransfer` 守恒账本
  - `plan-npc-ai-v1` ✅：`FactionStore` / `FactionMembership` / `Reputation` / `NpcArchetype`（决定战斗力参数基线）
  - `plan-qi-physics-v1` ✅：`collision::repulsion`（dormant 同 zone ρ 矩阵）/ `ledger::QiTransfer`
  - `plan-agent-v2` ✅：天道 agent Arbiter 仲裁层 → server 侧执行战争结果（`bong:agent_cmd` 通道）

- **出料**：
  - `DormantBattleBatch` 推演引擎（`server/src/npc/dormant_battle.rs` 新模块）
  - `DormantBattleResult` struct（伤亡 / 灵气转移 / snapshot 变更清单）
  - `DormantBattleEvent` emit（叙事事件 → `bong:world_state` publish → 天道 agent 出 narrative）
  - 扩展 `DormantNpcSnapshot`：战争相关字段（`hp_current: f64` / `battle_cooldown_ticks: u32` / `alliance_tags: Vec<String>`）

- **共享类型 / event**：
  - 复用 `FactionStore` / `qi_physics::ledger::QiTransfer` / `DormantNpcSnapshot`
  - **禁止**另建 dormant 专属派系 / 灵气结构体（孤岛红旗）
  - 战争结果走 `qi_physics::ledger::QiTransfer`（死亡 NPC 灵气释放回 zone / 胜方吸收战利品）——守恒律不因 dormant 战争豁免

- **跨仓库契约**：
  - server：`npc/dormant_battle.rs` 新模块 + `bong:world_state` 发布战争事件
  - agent：`bong:world_state` 订阅 `DormantBattleEvent` → 天道 agent 出 narrative / 调整宏观局势推演
  - client：无直接接口（战争结果通过 world_state → agent → narration 通道到达玩家）

- **worldview 锚点**：
  - §十一:947-970 散修江湖人来人往：5000+ NPC 的派系战争是「散修江湖」叙事的物理基础——谁占 POI / 谁被歼灭 / 谁扩张，都需要 server 侧真实推演而非 agent 凭空叙事
  - §二 真元守恒：dormant 战争死亡 NPC 的灵气**必须**释放回所在 zone（`qi_physics::release`），不允许凭空消失（守恒律底线）
  - §三:124-187 NPC 与玩家平等：dormant NPC 死亡走 `CultivationDeathTrigger`（与 hydrated NPC / 玩家同路径），等级高的 NPC 死亡有境界倒退风险（不因 dormant 豁免）

- **qi_physics 锚点**：
  - `qi_physics::ledger::QiTransfer`：战争伤害（攻方真元消耗 / 守方真元减损）必须有对应账本项
  - `qi_physics::release::release_to_zone`：dormant NPC 死亡灵气归还
  - `qi_physics::collision::repulsion`：同 zone 多 dormant NPC ρ 矩阵（战争后幸存者 / 新来者的排斥重算）

---

## §0 P0 决策门

- [ ] **v1 A 路径满足度评估**：在 v1 + v2 稳定运行后，统计天道 agent 侧 dormant 推演的叙事质量（派系边界 / 战争结果是否符合 worldview §十一 规律），判断是否需要 C 路径
- [ ] **战争推演粒度**：P0 决策「一次 GlobalTick 批推多少 dormant NPC 战斗」——是 zone 级（一个 zone 内所有敌对 NPC 一次推）还是 pair 级（每对敌对 NPC 独立回合）？影响复杂度和守恒账本写入量
- [ ] **与天道 agent 的分工边界**：server 侧推演「战斗数字结果」，agent 侧出「叙事 narrative」——边界在 `DormantBattleResult` 还是更细粒度的 NPC 行为？
- [ ] **战争 cooldown**：dormant NPC 战后多少 in-game 时间不参与下一场战争（防止同 tick 反复死亡再 hydrate loop）
- [ ] **大规模冲突的守恒律压力**：100 NPC 同 tick 战死的灵气 release 量级，是否超过 zone spirit_qi 上限，需要 WorldQiAccount overflow 处理方案

---

## P1 DormantBattleBatch 推演引擎

交付物：
- `server/src/npc/dormant_battle.rs`：新模块
- `DormantBattleBatch` 系统：每次 `GlobalTick` 扫描 dormant snapshot，找出同 zone 敌对 NPC pair，按简化战斗公式推演结果（基于 `cultivation.realm` / `NpcArchetype` 战斗力参数）
- 简化战斗公式：境界差 → 胜率（worldview §五 对应规则），随机数决定结果，伤亡记录在 `DormantBattleResult`
- `qi_physics::ledger::QiTransfer` 写入：战斗消耗 + 死亡灵气 release
- 单测：境界差异胜率分布 + 守恒律（战前 / 战后 zone spirit_qi 总量误差 < 0.01%）+ battle_cooldown 防止 loop

---

## P2 战争结果写 snapshot + 叙事事件 emit

交付物：
- 战败 NPC：`DormantNpcSnapshot` hp_current → 0 + `CultivationDeathTrigger` emit（走 plan-death §4b 老死路径的 dormant 分支）
- 战胜 NPC：`DormantNpcSnapshot` 更新（经验 / 境界推演 / 战利品 snapshot 更新）
- `DormantBattleEvent` emit → `bong:world_state` publish（payload 含：zone / 参战双方 NPC id / 胜负 / 境界 / 叙事 tag）
- 天道 agent 侧消费 `DormantBattleEvent` 出 narrative（按 plan-agent-v2 `WorldModel` 更新路径）
- 单测：战败 NPC snapshot 清除 + CultivationDeathTrigger 事件链 + 战利品 qi 守恒

---

## P3 e2e 验收

交付物：
- e2e 测试：1000 dormant NPC 两派对立，1h in-game GlobalTick 推演，验收：
  - 战争结果符合境界规律（高境界胜率 > 低境界）
  - WorldQiAccount 守恒误差 < 0.1%（5000 dormant 混合）
  - 天道 agent 消费 DormantBattleEvent 出 narration（smoke test：至少 1 条 narrative 到达 `bong:world_state`）
  - TPS ≥ 18（100 hydrated + 5000 dormant 混合，GlobalTick 战争推演不阻塞服务器帧）
- CI：`cargo test npc::dormant_battle` 含守恒律 + 胜率 + cooldown 回归

---

## §8 开放问题

1. **战争推演粒度**：zone 级 vs pair 级（P0 决策门收口）
2. **境界差胜率公式**：是否参考 worldview §五 具体数值还是自定义简化线性公式
3. **大规模战死的 qi release 溢出处理**：100 NPC 同 tick 战死，zone spirit_qi 写入量是否超上限，如何 clamp + 溢出分配到相邻 zone
4. **与 plan-yidao-v1 的接续命术**：dormant NPC 战死后是否触发医道 NPC hydrate 进行续命（叙事价值 vs 复杂度）
5. **叙事 tag 设计**：`DormantBattleEvent` payload 中的叙事 tag 由 server 预生成（基于 archetype / zone / 境界）还是全留 agent 推演

---

## 前置依赖

- `plan-npc-virtualize-v1` ✅（dormant SoA 架构 + GlobalTick 批量推演 + 守恒律接入）
- `plan-npc-virtualize-v2` ⬜（三态稳定后再叠战争推演层，防止 Drowsy↔Dormant 切换 race）
- `plan-qi-physics-v1` ✅（`ledger::QiTransfer` / `release::release_to_zone` / `collision::repulsion`）
- `plan-agent-v2` ✅（天道 agent Arbiter 仲裁 + `bong:world_state` publish/subscribe 通道）
- `plan-npc-ai-v1` ✅（`FactionStore` / `Reputation` / NPC 战斗力参数来源）

## 反向被依赖

- `plan-narrative-political-v1`（大规模派系战争是政治叙事前提，server 侧推演数字 → agent 侧出 narrative 是叙事丰度基础）
- `plan-faction-war-v1`（占位）（完整的派系战争玩法 plan，本 plan 是其 server 侧底层推演引擎）
