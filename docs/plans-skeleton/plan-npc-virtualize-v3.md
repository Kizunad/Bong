# Bong · plan-npc-virtualize-v3 · 骨架

NPC 隐式更新框架 **dormant↔dormant 批量战争推演**——在 plan-npc-virtualize-v1/v2 三态 MVP 基础上，实装 dormant NPC 之间的派系战争批量推演（敌对派系 dormant NPC 互殴 → 伤亡 → 灵气释放回 zone），让 worldview §十一「散修江湖人来人往」+ §三「派系兴衰」在 dormant 层有真实力学基础。

**启动条件（来自 plan-npc-virtualize-v1 §8 决策门 #6）**：plan-npc-virtualize-v1 P3 实测后，决策门 #6 选择 **C 选项**（"v1 实装 dormant↔dormant faction 战争"）时启动。默认 v1 推 A（全权交 agent 推演），若天道 agent 推演不足以支撑叙事丰度、或 faction 兴衰速度不符合 worldview 预期 → 升级到本 plan。

**交叉引用**：`plan-npc-virtualize-v1.md` ✅（dormant SoA 结构 + §8 决策门 #6）· `plan-npc-virtualize-v2.md` ⬜（三态稳定后 v3 才启动）· `plan-npc-ai-v1.md` ✅（FactionStore / FactionMembership / 派系敌对关系注册）· `plan-qi-physics-v1.md` P1 ✅（ledger + release）· `plan-agent-v2.md` ✅（天道 agent NpcDigest 推演）

**worldview 锚点**：

- **§十一:947-970 散修江湖**：人来人往的背后需要真实的"人去"——派系内斗 / 灭门 / 吞并在 dormant 层批量发生，天道 agent 只知道结果
- **§三:124-187 NPC 与玩家平等**：dormant NPC 被敌对派系 dormant NPC 杀死等同于玩家 PvP 死亡路径——走相同的死亡事件链（`bong:npc/death` → release 灵气 → 遗骸生成）
- **§二 真元守恒**：dormant 战斗伤亡必须 release qi 回 zone，守恒律底线不变
- **§P 派系矩阵**：敌对关系由 FactionStore 驱动，dormant 推演按同 zone 内敌对 NPC 密度算概率（不逐个模拟战斗，走期望值 batch）

**qi_physics 锚点**：

- `qi_physics::ledger::QiTransfer` —— 所有 dormant 战亡灵气流动走 ledger（守恒律底线）
- `qi_physics::release::qi_release_to_zone` —— dormant 战亡 release 灵气回 zone（同 v1 老死路径复用）
- `qi_physics::collision::repulsion` —— 多 dormant NPC 同 zone 高密度时排斥（v1 已用 sequential regen 实现，v3 在战斗推演中引入显式 collision 系数）

**前置依赖**：

- `plan-npc-virtualize-v1` ✅ → dormant SoA 结构 + §8 决策门 #6 选 C 成立
- `plan-npc-virtualize-v2` ⬜（可选，三态稳定后 v3 才有意义；若 v2 不启动则 v3 依赖 v1 直接）
- `plan-npc-ai-v1` ✅ → FactionStore 派系敌对关系 + NpcDigest（v3 战争推演结果发 NpcDigest 给 agent）
- `plan-qi-physics-v1` P1 ✅ → ledger + release API 冻结

**反向被依赖**：

- `plan-narrative-political-v1` ✅ active → 派系兴衰叙事（agent 需要 dormant 战争结果作为叙事素材）
- `plan-npc-ai-v1 §3.3` 代际更替 → dormant 战争是代际更替（宗门灭门 / 派系重组）的物理基础

---

## 接入面 Checklist

- **进料**：
  - `NpcDormantStore`（v1 结构，v3 按 zone 分桶敌对对）
  - `FactionStore`（派系敌对关系矩阵，plan-npc-ai-v1 实装）
  - `ZoneRegistry`（zone spirit_qi / 中心位置）
  - `qi_physics::env::EnvField`（zone 浓度）
- **出料**：
  - `dormant_war_tick_system`（GlobalTick 1/min，同 dormant 推演频率：对同 zone 内敌对派系 dormant NPC 配对，批量按期望伤亡计算，输出死亡 + 灵气释放事件）
  - `DormantFactionWarEvent`（战亡 NPC id / 胜方派系 / release 量）→ 走 `bong:npc/death` + ledger
  - `FactionDominanceSnapshot`（每 zone 派系势力分布快照，输出给天道 agent NpcDigest）
- **共享类型**：
  - 复用 `qi_physics::release::qi_release_to_zone`（战亡路径同 v1 老死路径）
  - 复用 `bong:npc/death` Redis channel（战亡走同一通道）
  - **禁止**为 dormant 战斗新建独立伤害计算公式（战斗期望值用 v1 已有的 realm 差 + ρ 矩阵 + 随机因子，不引入新公式）
- **跨仓库契约**：
  - server: `npc::dormant::war_tick` 子模块
  - agent: 接收 `FactionDominanceSnapshot` via NpcDigest，天道 agent 把战争结果转为叙事 narration
  - client: 无直接变化（战亡走 `bong:npc/death` 自然清除；派系势力变化体现在 NPC 分布上）

---

## 阶段概览

| 阶段 | 目标 | 状态 |
|------|------|------|
| P0 | 决策门 #6 确认选 C + 战争推演算法设计 | ⬜ |
| P1 | `dormant_war_tick_system` + 期望伤亡计算 + ledger 集成 | ⬜ |
| P2 | `FactionDominanceSnapshot` + agent NpcDigest 接入 | ⬜ |
| P3 | e2e 验收：1000 dormant + 5 敌对派系 1h 派系势力演化 | ⬜ |

---

## §0 设计轴心

- [ ] **期望伤亡算法（P0 核心）**：dormant↔dormant 战斗不逐个模拟，走批量期望值：`killed = floor(n_attacker × (realm_adv_factor) × rho_ij × time_step)`，realm_adv_factor 由双方 Realm 中位数差计算，rho_ij 复用 qi_physics ρ 矩阵。**禁止引入全新战斗公式**，复用 v1 + qi_physics 已有常数
- [ ] **zone 敌对配对边界**：只有同 zone 或相邻 zone（± 1 zone 距离）的敌对 NPC 才计入战争推演，避免跨地图 "战争"（无物理意义）
- [ ] **守恒律强约束**：所有战亡 release 必须走 `qi_release_to_zone`，不允许 `zone.spirit_qi += killed * factor`（守恒律红旗）
- [ ] **天道 agent 边界**：v3 只产出事件数据（谁死了、哪个派系赢了、zone 灵气变化），叙事 narration 全权交 agent。**server 不做叙事判断**
- [ ] **大规模派系战争熔断**：单 tick 最大处理 dormant pair 数量上限（防止 1000+ NPC 全打 O(N²) 风暴）

---

## §1 开放问题（P0 决策门收口）

- [ ] **Q1** 期望值 vs 蒙特卡洛：纯期望值（确定性）vs 每 NPC 随机 roll（随机性更丰富但计算更重）？
- [ ] **Q2** 战争频率：1/min（同 dormant tick）vs 1/5min（更低频，减少抖动）？
- [ ] **Q3** 派系联盟支援：同盟派系是否加入战争期望值计算（复杂但叙事更丰富）？
- [ ] **Q4** 战争结果通知：NPC 死亡事件发 `bong:npc/death`，派系势力变化发独立 `bong:faction/dominance` channel 还是塞进 NpcDigest？
- [ ] **Q5** 玩家目击触发 hydrate：玩家进入战争 zone 时（64 格内），战争进行中的 dormant NPC 是否强制 hydrate 走正常 ECS 战斗链？

---

## §9 进度日志

- **2026-05-07** 骨架立项。源自 plan-npc-virtualize-v1 §8 决策门 #6 C 选项占位 + reminder.md `plan-npc-virtualize-v3 占位` 记录。本 plan **条件性启动**：v1 决策门 #6 选 C 时方开始消费。

---

## Finish Evidence（待填）

迁入 `finished_plans/` 前必须填：

- **落地清单**：`server/src/npc/dormant/war_tick.rs` + `FactionDominanceSnapshot` + ledger 集成
- **关键 commit**：各 phase hash + 日期 + 一句话
- **测试结果**：`cargo test npc::dormant::war` / 守恒律战亡 e2e / 1000 dormant + 5 派系 1h 演化
- **跨仓库核验**：server `npc::dormant::war_tick` / agent `FactionDominanceSnapshot` 接收 / client 无变化
- **遗留 / 后续**：派系联盟 / NPC 代际更替战争叙事精细化（若需要派生 v4）
