# Bong · plan-npc-ai-v1

**NPC 行为 / 生命周期 / 派系 / 社交专项**。server 已有 big-brain 骨架（僵尸战斗链完整），本 plan 对齐现状、扩展多 archetype、老化代际、派系社交，并与 plan-death-lifecycle + plan-tribulation 对接 NPC 渡劫 / 寿元 / 截胡。

**交叉引用**：`CLAUDE.md` NPC AI · `plan-death-lifecycle-v1.md §8 Phase 8`（NPC 老化）· `plan-tribulation-v1.md §3`（化虚 NPC 名额）· `plan-combat-no_ui.md` / `plan-combat-ui_impl.md`（状态效果 + 战斗链路）· `plan-cultivation-v1.md`。

---

## §-1 现状（已实装，本 plan 不重做）

_本 plan 展开前的底座。写下来避免重复设计或误拆。_

| 层 | 已有实装 | 文件 |
|---|---------|------|
| **Components** | `NpcMarker` / `NpcBlackboard` / `NpcMeleeProfile` / `NpcMeleeArchetype` / `NpcCombatLoadout` / `NpcPatrol` | `server/src/npc/mod.rs` |
| **Scorers** | `PlayerProximityScorer` / `ChaseTargetScorer` / `MeleeRangeScorer` / `DashScorer` | `npc/brain.rs`（1177 行） |
| **Actions** | `FleeAction` / `ChaseAction` / `MeleeAttackAction` / `DashAction` | `npc/brain.rs` |
| **Thinker** | `FirstToScore` picker，阈值 0.05 | `npc/brain.rs` |
| **Pathfinding** | A* 块级寻路 | `npc/navigator.rs`（905 行） |
| **Movement** | Position↔Transform 同步桥 | `npc/movement.rs` / `npc/sync.rs` |
| **Patrol** | 巡逻点循环 | `npc/patrol.rs` |
| **战斗对接** | `CombatState` / `Wounds` / `Stamina` / `StatusEffects` / `DerivedAttrs` 全套组件已挂 | `combat/mod.rs` |
| **Cultivation 组件挂载** | NPC 挂 `Cultivation` + `Contamination` + `MeridianSystem`（仅伤害解析用，无推进） | `npc/scenario.rs` |
| **Archetype** | 仅 **僵尸** 一种（启动 spawn @14,66,14） | `npc/spawn.rs` |
| **Agent 命令通道** | `CommandType::NpcBehavior` 已定义 + `execute_npc_behavior()` 入口 | `network/command_executor.rs` |

**尚未实装**（本 plan 的实际工作范围）：
- 多 archetype（散修 / 宗门弟子 / 妖兽 / 凡人 / 仙家遗种）
- NPC 寿元 tick 驱动 + 老化 + 代际更替
- NPC 境界推进（cultivation lifecycle）
- 派系 / 师承 / 声望
- NPC ↔ NPC 社交（对话 / 交易 / 切磋）
- Agent 下发 NPC 行为命令的 producer 端（现在只有 consumer）

---

## §0 设计轴心

- [ ] **短期行为在 server ECS / big-brain**；**长期决策由天道 agent**（时代推演、派系兴衰、重大冲突）
- [ ] **agent 只下发意图，不写底层战斗/物理状态**——HP / 真元 / 位置 / 伤口 等全由 ECS system 维护；agent 可改 blackboard、声望、派系关系、任务队列等"决策层"状态，不直接改战斗数值
- [ ] **NPC 与玩家规则平等**：寿元、境界、渡劫、死亡都走同一套 plan-death / plan-tribulation / plan-cultivation 契约；天道不偏袒玩家或 NPC
- [ ] **archetype 即配置**：新 archetype = 新 Bundle + Scorer 组合 + 行为权重表，不新写 ECS system
- [ ] **性能优先**：大服可能上千 NPC，brain tick 分帧 + LOD（远离玩家的 NPC 降频 thinker）

## §1 NPC 分类

| Archetype | 行为核心 | 特有 Scorer | 生命周期 | 首版优先级 |
|-----------|---------|------------|---------|-----------|
| **僵尸**（已有） | 追击 / 近战 | Proximity / MeleeRange | 无（不老，被击杀或由 agent `DespawnNpc` 清理） | — |
| **凡人** | 耕作 / 避修士 / 村居 | Fear(Cultivator) / WorkSchedule | 80 年寿元，会老死 | P0 |
| **散修** | 漂流 / 寻机缘 / 避世 / 修炼 | CultivationDrive / Curiosity / Threat | 按境界寿元，会渡劫 | P0 |
| **妖兽** | 领地 / 捕食 / 护崽 | Territory / Hunger / ProtectYoung | 会繁衍（领地内新个体） | P1 |
| **宗门弟子** | 任务 / 日常 / 护山 | Loyalty / MissionQueue | 同散修 + 派系 buff | P1 |
| **仙家遗种** | 守护遗迹 / 考验 | GuardianDuty / TrialEval | 不老（遗迹绑定） | P2 |

**字段层复用**：所有 archetype 共用 `NpcMarker` + `Cultivation` + `Lifespan`（新增）+ 战斗组件，仅在 Bundle 里换 Scorer/Action 组合。

## §2 Scorer / Action 扩展

_在现有 4 Scorer + 4 Action 基础上扩展。_

### 新增 Scorer（按 archetype 分组激活）

| Scorer | 输入 | 输出 | 用于 |
|--------|------|------|------|
| `FearCultivatorScorer` | 50 格内最近修士境界 | 0–1 | 凡人逃跑 |
| `TerritoryScorer` | 是否在领地内 + 入侵者 | 0–1 | 妖兽驱逐 |
| `HungerScorer` | 饱食度衰减 tick | 0–1 | 妖兽/凡人 |
| `CultivationDriveScorer` | 当前境界进度 × 所在区域灵气 | 0–1 | 散修/弟子静坐 |
| `CuriosityScorer` | 附近未探索 POI / 机缘事件 | 0–1 | 散修流浪 |
| `LoyaltyScorer` | 派系任务队列长度 + 派系声望 | 0–1 | 弟子执行任务 |
| `MissionQueueScorer` | 待办任务优先级 | 0–1 | 弟子 |
| `ProtectYoungScorer` | 附近幼崽血量 | 0–1 | 妖兽 |
| `TribulationReadyScorer` | 奇经八脉全通 + 玩家周围无高威胁 | 0/1 | 散修主动起渡虚劫 |
| `AgeingScorer` | 已活 / 上限比例 | 0–1 | 触发"归隐"行为 |
| `WorkScheduleScorer` | 昼夜 + 职业（农 / 商 / 守村） | 0–1 | 凡人定时劳作 |
| `GuardianDutyScorer` | 遗迹状态 + 入侵者数量 | 0–1 | 仙家遗种护迹 |
| `TrialEvalScorer` | 附近玩家境界 / 资质 | 0–1 | 仙家遗种开启考验 |

### 新增 Action

| Action | 效果 |
|--------|------|
| `WanderAction` | 向随机有灵气梯度方向移动（散修/凡人） |
| `CultivateAction` | 原地静坐，推进经脉（接 cultivation tick） |
| `FleeCultivatorAction` | 凡人看见修士远离（differs from FleeAction 的仇敌逃跑） |
| `FarmAction` | 凡人耕作（占位：附近灵田 block 交互） |
| `TradeAction` | 凡人/散修 相遇 → 货币/物品交换 |
| `SocializeAction` | NPC 闲聊（触发 agent narration） |
| `TerritoryPatrolAction` | 妖兽巡领地 |
| `HuntAction` | 妖兽捕食低境界生物 |
| `ProtectYoungAction` | 妖兽护崽，提高攻击优先级 |
| `MissionExecuteAction` | 弟子执行任务（导航到目标点 + 任务脚本） |
| `StartDuXuAction` | 散修/弟子自主起渡虚劫（与玩家同通道） |
| `RetireAction` | 风烛 NPC 找安静处"归隐"→ 老死于此 |
| `GuardAction` | 仙家遗种留守遗迹，攻击入侵者 |
| `TrialAction` | 仙家遗种对玩家开启考验（触发 Dynamic XML UI） |
| `SeclusionAction` | NPC 化虚后"隐世"——仅低频移动 + 偶发 narration |

### Thinker 组合（按 archetype）

```
凡人:      [Fear, Hunger, Ageing, WorkSchedule] → [FleeCultivator, Farm, Wander, Retire]
散修:      [CultivationDrive, TribulationReady, Curiosity, PlayerProximity, Ageing]
           → [Cultivate, StartDuXu, Wander, FleeAction(已有), Retire, SeclusionAction(化虚后)]
妖兽:      [Territory, Hunger, ProtectYoung, ChaseTarget(已有)]
           → [TerritoryPatrol, Hunt, ProtectYoung, FleeAction]
宗门弟子:  散修 + [Loyalty, MissionQueue] → + [MissionExecute]
仙家遗种:  [GuardianDuty, TrialEval] → [GuardAction, TrialAction]
```

## §3 NPC 生命周期（与 plan-death / plan-cultivation 共享规则）

### §3.1 寿元

- [ ] NPC 共用 `LifespanComponent`（plan-death §7）
- [ ] **tick rate 与玩家一致**：1 real hour = 1 year，死域 ×2，但 NPC 无"离线"概念（始终在线）
- [x] ✅ **首版支持开关**：`NpcAgingConfig { enabled: bool, rate_multiplier: f32 }` ——性能紧张时可降 NPC 老化速率（如 0.3x）
- [x] ✅ 风烛状态驱动 `RetireAction`（NPC 找偏僻处归隐，等老死）

### §3.2 境界推进

- [x] ✅ NPC 散修/弟子在 `CultivateAction` 期间消费区域灵气推进经脉（与玩家同规则）
- [x] ✅ **不走突破事件交互**——NPC 满足突破条件自动过（凡是需要 inspect UI 的分支全部自动选默认）
- [x] ✅ NPC 奇经八脉全通 → `TribulationReadyScorer` 触发 → `StartDuXuAction`
- [x] ✅ NPC 渡虚劫走 plan-tribulation 同通道：占名额 / 被截胡（玩家可截胡）/ 失败退境
- [x] ✅ **被截胡的 NPC 掉落规则**：NPC 通常无背包物品；改为按 archetype 的 `LootTable` 掉落（散修掉丹药/法器残片 / 弟子掉派系信物 / 僵尸掉素材），写在 `NpcLootTable` 配置
- [x] ✅ NPC 化虚后进入"隐世"模式（`SeclusionAction`：少活动，agent narration 变稀）

### §3.3 代际更替

- [x] ✅ **凡人**：老死后 spawn 邻居新生儿（同坐标 100 格内，年龄 0-5）
- [ ] **散修**：老死后 **不直接补**；由天道 agent 在"灵脉波动"事件触发时批量 spawn 新散修（避免满地 respawn 的观感）
- [x] ✅ **妖兽**：在领地内繁衍，不需要 agent 介入
- [ ] **NPC 老死事件**：走 `plan-death §4b` 善终路径 + 写 `bong:npc/death` channel（含生平卷 snapshot）；亡者博物馆**不展示 NPC**（只展示玩家，避免被 NPC 刷屏）
- [x] ✅ **NPC 总量上限**：全服 `max_npc_count`（首版 512，config 可调），达到上限后自然老死不再补，直到低于阈值 × 0.9 再恢复补员

### §3.4 夺舍 NPC

- [ ] 玩家可夺舍 凡人 / 醒灵 NPC（plan-death §4e）
- [ ] 被夺舍 NPC 从 ECS 移除，玩家角色继承坐标 + 肉身年龄
- [ ] 触发 `DuoSheEvent`，两卷交叉引用（NPC 卷虽不公开展示但仍写入存储）

## §4 派系 / 门派

_§4 派系数据层（FactionId / FactionState / FactionStore / Lineage / Reputation / MissionQueue / FactionMembership）已在 PR #45 落地于 `server/src/npc/faction.rs`，关系矩阵（攻/守/中和三派）走 `is_hostile_pair`。师承继承功法残篇、声望变动来源、声望影响交易/任务等行为侧 hook 仍待接入。_

### §4.1 数据模型

```rust
pub struct FactionId(u32);

pub struct FactionState {
    pub id: FactionId,
    pub name: String,
    pub leader: Option<CharId>,        // 可空（领袖死了未选新）
    pub members: Vec<CharId>,
    pub hq_zone: Option<ZoneId>,
    pub reputation: HashMap<FactionId, i32>, // 与其他派系关系
    pub doctrine: DoctrineTag,         // 攻流 / 守流 / 中和（对齐 plan-combat 六流）
}

pub struct FactionMembership {
    pub faction: FactionId,
    pub rank: u8,              // 0=外门, 1=内门, 2=真传, 3=长老, 4=掌门
    pub loyalty: f32,          // 影响 LoyaltyScorer
}
```

### §4.2 师承

- [x] ✅ `Lineage { master: CharId, disciples: Vec<CharId> }` Component
- [ ] 师承影响：继承部分功法残篇（NPC 内部，不影响玩家 plan-death "不继承"原则）
- [ ] 师父老死 → 真传弟子继承派系领袖（若掌门位空）

### §4.3 声望

- [x] ✅ NPC 对玩家 / 其他 NPC 的 `Reputation`（-100 ~ +100）
- [ ] 变动来源：任务完成、击杀派系成员、交易诚信、目击玩家渡劫
- [ ] 影响：NPC 主动攻击 / 交易折扣 / 任务可接度

### §4.4 派系关系矩阵

- [x] ✅ 派系 × 派系 矩阵（友/敌/中立/血仇）—— 首版 攻/守 互为敌对、其余中立，`is_hostile_pair` 入 ECS
- [x] ✅ 由 agent 长期推演（灾劫/变化/演绎三 agent 可修改）—— `CommandType::FactionEvent` 通道已通
- [ ] 关系变更触发事件：开战、结盟、瓦解

## §5 社交 AI

### §5.1 NPC ↔ NPC

- [ ] 同派系 NPC 相遇 → `SocializeAction`（寒暄、交换情报）_（`SocializeScorer/Action` 已在 `npc/social.rs` 落地，但 thinker 注册因 e2e TPS 回归暂撤回，待 LOD gate 稳定后接回）_
- [ ] 敌对派系相遇 → Threat Scorer 起跳 → Fight or Flee _（`FactionDuelScorer` 已落地，thinker 同上）_
- [ ] 交易：散修带的物品 + 货币按简单市场估价

### §5.2 NPC ↔ 玩家

- [ ] 任务系统（弟子 / 散修可派任务给玩家）——**留 hook，首版不展开**
- [ ] 师承：玩家可拜师 NPC（需派系声望达标 + 师父同意）——**留 hook，首版不展开**
- [ ] 敌对：NPC 可仇杀玩家（业力 / 派系冲突）

### §5.3 Agent 对社交的影响

- [x] ✅ Agent 可下发 `FactionEvent` 命令（战争宣告、联盟、瓦解）—— `command_executor::execute_faction_event` 已通
- [ ] Agent 可修改单个 NPC 的 `Reputation` 矩阵（用于推演）
- [ ] 重大事件触发 narration（"魏家与李家今日血仇已结"）

## §6 数据契约

### 新增 Components

```rust
pub struct NpcArchetype(ArchetypeId);  // 枚举：Zombie/Commoner/Rogue/Beast/Disciple/GuardianRelic
pub struct LifespanComponent { /* 复用 plan-death */ }
pub struct FactionMembership { faction: FactionId, rank: u8, loyalty: f32 }
pub struct Lineage { master: Option<CharId>, disciples: Vec<CharId> }
pub struct Reputation(HashMap<CharId, i32>);
pub struct Territory { center: BlockPos, radius: u32 } // 妖兽
pub struct MissionQueue(Vec<MissionId>);               // 弟子；MissionId 由 plan-quest-v1 定义
pub struct GuardianDuty { relic_id: RelicId, alarm_radius: u32 }  // 仙家遗种
pub struct TrialEval { trial_template_id: TrialId }               // 仙家遗种
pub struct NpcLootTable { archetype: ArchetypeId, entries: Vec<(ItemId, f32)> }
pub struct NpcAgingConfig { enabled: bool, rate_multiplier: f32 } // 首版 0.3
pub struct NpcDigest { char_id, archetype, realm, faction: Option<FactionId>, recent_summary: String } // 远方 NPC 压缩表示
```

### Bundles

```rust
CommonerBundle  = [NpcMarker, Archetype=Commoner, Cultivation=凡人, Lifespan, FearScorer, HungerScorer, ...]
RogueBundle     = [NpcMarker, Archetype=Rogue, Cultivation, Lifespan, CultivationDriveScorer, ...]
BeastBundle     = [NpcMarker, Archetype=Beast, Territory, Hunger, ProtectYoung, ...]
DiscipleBundle  = RogueBundle + FactionMembership + Lineage + MissionQueue
RelicGuardBundle= [NpcMarker, Archetype=GuardianRelic, GuardianDuty, TrialEval]
```

### Channel / Store

- [ ] `bong:npc/spawn` — server 主动 spawn（生育 / 天道投放 / 夺舍残留）
- [ ] `bong:npc/death` — NPC 死亡 / 老死（含 cause、archetype、faction）
- [x] ✅ `bong:npc/behavior_cmd` — **已有** agent 下发
- [ ] `bong:faction/event` — 派系大事件（战争 / 盟约 / 瓦解）_（command 通道在，dedicated Redis channel 待补）_
- [x] ✅ `NpcRegistry`（server 全局，所有 NPC 查询 / 总量上限）
- [x] ✅ `FactionStore`（所有派系状态）

### Agent Intent（producer 侧待补）

- [x] ✅ `SpawnNpc { archetype, pos, faction?, realm? }` —— `CommandType::SpawnNpc` consumer 已通（commoner / rogue archetype）
- [x] ✅ `DespawnNpc { char_id, reason }` —— `CommandType::DespawnNpc` consumer 已通
- [x] ✅ `ModifyBehavior { char_id, blackboard_patch }` — 已有 consumer
- [x] ✅ `FactionEvent { kind, source, target }` —— `CommandType::FactionEvent` 已通

## §7 实施节点

**Phase 0 — LifespanComponent 挂到 NPC + 老化 tick + 总量上限**
- [ ] NPC 共用 plan-death §7 `LifespanComponent`
- [x] ✅ `NpcAgingConfig { enabled: true, rate_multiplier: 0.3 }`
- [x] ✅ `max_npc_count = 512`（可调）+ 满员后停止补员直到降至 0.9× 阈值
- [ ] NPC 老死 → 走 plan-death §4b 善终路径（遗骸容器 + `bong:npc/death`）
- [x] ✅ 风烛 → `RetireAction`

**Phase 1 — 凡人 archetype** ✅（PR #45）
- [x] ✅ `CommonerBundle` + `FearCultivatorScorer` + `HungerScorer`
- [x] ✅ `FleeCultivatorAction` / `FarmAction` / `WanderAction`
- [x] ✅ 凡人老死后邻居生子（同坐标 100 格内 spawn 新凡人，年龄 0-5）
- [ ] 凡人被夺舍路径（plan-death §4e）_（`possession.rs` 仅 stub log，待 plan-death §4e 接入）_

**Phase 2 — 散修 archetype + 境界推进** ✅（PR #45）
- [x] ✅ `RogueBundle` + `CultivationDriveScorer` + `CuriosityScorer`
- [x] ✅ `CultivateAction`（消费区域灵气推经脉）
- [x] ✅ NPC 境界自动突破（无 UI 交互）
- [x] ✅ 散修流浪 / 寻机缘 / 避世（`WanderScorer` + `FleeAction` + Curiosity baseline）

**Phase 3 — NPC 渡虚劫 + 隐世** ✅（PR #45）
- [x] ✅ `TribulationReadyScorer`（三重门槛：条件满足 + 100 格无敌意 + `CultivationDrive > 0.6` 持续 30 min）+ `StartDuXuAction`
- [x] ✅ 与 plan-tribulation §2 共用流程（全服广播 / 截胡窗口 / 心魔劫）
- [x] ✅ NPC 心魔劫：自动走"坚心"默认（`npc_tribulation_auto_wave_tick` 每 100 tick 推一波）
- [x] ✅ NPC 化虚名额：占用 `AscensionQuotaStore(4)`
- [x] ✅ NPC 被玩家截胡 → 按 `NpcLootTable` 掉落（非空背包规则）
- [x] ✅ NPC 化虚后 `SeclusionAction`：低频移动 + 偶发 narration

**Phase 4 — 妖兽 archetype**（部分 / PR #45）
- [x] ✅ `BeastBundle` + `Territory` + `ProtectYoungState` 数据层 + `TerritoryIntruderScorer` / `ProtectYoungScorer` / `TerritoryPatrolAction` / `HuntAction` / `ProtectYoungAction` 落地于 `npc/territory.rs`
- [ ] 领地巡逻 / 捕食 / 护崽 _（Scorer/Action ECS 注册因 e2e TPS 回归撤回，beast thinker 当前降级到 Wander/Melee/Chase 核心链；待 LOD gate 稳定后接回）_
- [x] ✅ 妖兽繁衍（领地内自然 spawn 幼崽 + 满员 200 格外开新领地，方向按成体偏移派生）

**Phase 5 — 派系 / 宗门弟子**（部分 / PR #45）
- [x] ✅ `FactionState` / `FactionStore` / 关系矩阵
- [x] ✅ `Lineage` 师承
- [x] ✅ `Reputation` 组件
- [x] ✅ `DiscipleBundle` + `FactionMembership` + `MissionQueue` + `LoyaltyScorer` / `MissionQueueScorer` + `MissionExecuteAction` 数据/符号层落地于 `npc/faction.rs`
- [ ] `MissionExecuteAction` 行为接通 _（disciple thinker 当前降级到 Rogue 行为链；ECS 注册同 Phase 4 待恢复）_

**Phase 6 — 社交（NPC ↔ NPC）**（部分 / PR #45）
- [x] ✅ `SocializeScorer` / `SocializeAction` / `FactionDuelScorer` 落地于 `npc/social.rs`
- [ ] NPC 交易市场估价
- [ ] 敌对派系相遇自动交战 _（thinker 注册同 Phase 4 待恢复）_

**Phase 7 — Agent producer 端**（部分 / PR #45）
- [x] ✅ 天道 agent 发 `SpawnNpc` / `DespawnNpc` / `FactionEvent` —— consumer 端已通；producer 触发逻辑由 agent 侧决定何时发
- [ ] 灾劫 agent 可触发"灵脉波动批量 spawn 散修"
- [ ] 演绎 agent 控制派系兴衰
- [ ] **NPC 主动截胡玩家渡虚劫**：三重门槛（敌对派系弟子 + 业力 > 玩家 + agent 推演确认）→ 派 NPC 团前往截胡坐标

**Phase 8 — 仙家遗种**（骨架 / PR #45）
- [x] ✅ `RelicGuardBundle` + `GuardianDuty` + `TrialEval` + `GuardianDutyScorer` / `TrialEvalScorer` / `GuardAction` / `TrialAction` 落地于 `npc/relic.rs`
- [ ] 绑定 worldgen 遗迹 POI
- [ ] 考验脚本（agent 驱动，Dynamic XML UI）_（relic_guard thinker 当前降级到 Wander，Scorer/Action ECS 注册同 Phase 4 待恢复）_

**Phase 9 — 性能 / LOD** ✅（PR #45）
- [x] ✅ 远离玩家的 NPC brain 降频（`NpcLodConfig.reassess_interval = 20 tick` + 3 核心 scorer 走 `should_skip_scorer_tick` gate）
- [x] ✅ 超远 NPC 卸载到 "dormant" 列表（`NpcLodTier::Dormant`，无玩家时 100 rogue 全部 dormant，scorer 早退）
- [ ] 大服压测：1000 NPC / 50 玩家 同时 tick 的开销 _（e2e CI 单核暂以 `BONG_ROGUE_SEED_COUNT=0` 绕开；正式压测后续单独立项）_

## §8 已决定

- ✅ **短期 server / 长期 agent**：行为 ECS 驱动，意图 agent 注入
- ✅ **NPC 与玩家同规则**：寿元 / 境界 / 渡劫 / 死亡共用 plan-death / plan-tribulation
- ✅ **亡者博物馆不收 NPC**（避免刷屏；NPC 生平仅 server 内部存储）
- ✅ **NPC 心魔劫走简化版**：agent 可生成，也可自动判"坚心"（减少 LLM 调用压力）
- ✅ **总量上限 + 老死不即时补**：代际更替要显得自然
- ✅ **archetype = 配置**：新 archetype 不写 ECS system，仅 Bundle + Scorer 组合
- ✅ **NPC 老化速率**：`rate_multiplier = 0.3`（NPC 老得比玩家慢 3 倍，约 270h 换一代凡人），config 可调
- ✅ **NPC 起渡虚劫条件**：条件满足 + 100 格内无敌意 + `CultivationDrive > 0.6` 持续 30 min（防止挤 Agent 队列）
- ✅ **派系初始数据**：首版手配 3 个派系（六流中的攻/守/中和各一），agent 运行期可增删
- ✅ **任务 / 师承归属**：新立 **`plan-quest-v1`**；本 plan 仅保留 hook 字段（`MissionQueue` / `Lineage.master`）和 UI 草图交接
- ✅ **NPC 生平卷分级存储**：活跃 NPC（500 格内有玩家）→ 完整卷在 Redis；远方 NPC → `NpcDigest`（id+archetype+realm+faction+一行近况）压缩 10x；老死 → append-only log，不占 Redis
- ✅ **妖兽繁衍**：领地容量 = `round(半径/10)`，满员后幼崽移出 200 格外开新领地
- ✅ **NPC 截胡玩家渡虚劫**：允许但罕见，仅"敌对派系弟子 + 业力 > 玩家 + agent 推演触发"三重门槛

## §9 剩余开放问题

_（无未决项，所有设计问题均已收口）_

## §10 后续派生 plan

- [ ] `plan-quest-v1` —— 任务系统（NPC 派任务、师承、玩家接任务、奖励结算）
  - 本 plan 已留 hook：`MissionQueue` / `Lineage` / `Reputation`
  - UI 草图见 `docs/svg/quest-ui-v1.svg`（见下）
  - 具体任务内容、奖励曲线、师承拜师流程由 plan-quest-v1 承接

---

## §11 进度日志

- 2026-04-25：PR #45（commit d51038f2）落地 §§4–9 + LootTable + LOD 不依赖外部 plan 的部分；Phase 1/2/3/9 完整闭环，Phase 4/5/6/8 数据/符号层就绪但 Scorer/Action ECS 注册因 e2e TPS 回归暂降级，待 LOD gate 稳定后接回；Agent intent 四条 consumer 全通。

## Finish Evidence

- 2026-04-28：`eceea0e8 feat(npc): 衔接寿元老化与夺舍事件` 完成 NPC 寿元 source-of-truth 同步、非老化 archetype 过滤、老死/战斗/despawn/夺舍 death notice 与夺舍事件转发。
- 2026-04-28：`1b94a2cc feat(network): 发布 NPC 与派系事件通道` 完成 `bong:npc/spawn`、`bong:npc/death`、`bong:faction/event` dedicated Redis channel 与 server wire schema。
- 2026-04-28：`f7d0aa26 feat(agent): 订阅 NPC runtime 事件` 完成 agent schema、RedisIpc runtime event 订阅/缓存与 dedicated channel contract tests。
- 2026-04-28：`4792ebfe feat(agent): 推演 NPC 长期意图` 完成 deterministic NPC producer、`spawn_npc.count` batch/clamp、agent/server schema 与 runtime tests。
- 2026-04-28：`1921bacc feat(npc): 接回派系社交与守护行为` 恢复 Beast/Disciple/GuardianRelic 的 territory、mission、social、relic scorer/action 注册，并用 LOD gate 包住新增 scorer 以避免重现历史 TPS 回归。
- 验证通过：`cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`，server tests `1676 passed`。
- 验证通过：`cd agent && npm run build && npm test -w @bong/schema && npm test -w @bong/tiandao`，schema tests `192 passed`，tiandao tests `176 passed`。
