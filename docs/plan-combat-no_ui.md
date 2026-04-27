# 战斗系统实现计划 V1 — Server / Schema（无 UI 部分）

> **拆分说明**：本文件只覆盖服务端 ECS、攻击事务、流派实现、StatusEffect 系统、死亡-重生流程、IPC schema。
> 所有客户端 UI（施法 HUD、状态条、死亡画面、伤口层绑定、法宝展开、天劫广播等）统一在 **`plan-combat-ui_impl.md`**。
> 原因：UI 不适合云端 LLM 开发（需视觉反馈）；server + schema 可安全并行推进。本文内 "客户端 UI" 小节保留为数据接入索引。

---

## 实施边界（云端 worktree v1，2026-04-13）

> 本章节由本地 Claude Code 在 M1 验收后基于代码逐行核查插入，供云端 worktree 下次 pull 时作为唯一权威约束。

### 1. 本次 worktree 范围

**只做 C1-C3（server + schema）**，拒绝 C4-C7 和任何跨 worktree 扩散。

| 阶段 | 内容 | 状态 |
|------|------|------|
| C1 | `server/src/combat/` 模块骨架：`Wounds` / `AttackIntent` / `Lifecycle` components | ✅ 已完成（2026-04-21 验收） |
| C2 | 攻击事务（距离衰减、污染写入、`MeridianCrack` 施加）、`DeathEvent` 收口 | ✅ 已完成（2026-04-21 验收） |
| C3 | IPC schema 扩展：新 Redis 通道 `bong:combat_realtime` / `bong:combat_summary`，TypeBox TS + Rust serde 双端对齐 | ✅ 已完成（2026-04-21 验收） |
| C4+ | 终结归档、亡者博物馆、重生语义、CharacterRegistry | **禁止** |

### 2. 前置契约修复清单（全部已完成 — 2026-04-21 核查）

> 以下 5 项在 2026-04-13/14 随 `fix(cultivation): add combat identity anchors` (05eafd9f) / `fix(schema): align cultivation detail and negative zones` (ccdf5036) / `feat(npc): bridge melee ai into combat resolver` (d3f29055) 等 commit 并入 `main`。本节保留为落地记录，不再是开工阻塞项。

#### 2a. `ContamSource.attacker_id` ✅ 已完成（05eafd9f）
- `server/src/cultivation/components.rs:185` — `ContamSource { amount, color, attacker_id: Option<String>, introduced_at }`
- `server/src/combat/resolve.rs:290` — 攻击命中时写入 `Some(attacker_id.clone())`
- Serde roundtrip 测试：`components.rs:392 contam_source_serde_roundtrip_preserves_attacker_id`

#### 2b. `LifeRecord.character_id` ✅ 已完成（05eafd9f）
- `server/src/cultivation/life_record.rs:131` — `character_id: String` + `#[serde(default)]` 兼容旧档
- `LifeRecord::new(canonical_player_id(...))` 建档
- 6 个相关测试（legacy serde default / canonical anchor / combat hit 归属）全绿

#### 2c. 负 `zone.spirit_qi` ✅ 已完成（ccdf5036，方案 A — 放开下限到 -1.0）
- `server/src/world/zone.rs:18` — `MIN_ZONE_SPIRIT_QI = -1.0`；`validate_zone` 接受 `[-1.0, 1.0]`
- `server/src/network/command_executor.rs:14-15` — `ZONE_SPIRIT_QI_MIN = -1.0`
- `zone.rs:220` — `spirit_qi < -0.2` 判负灵域；实配置 `blood_valley spirit_qi = -0.35`
- 测试：`accepts_zone_spirit_qi_at_full_negative_bound` / `rejects_zone_spirit_qi_below_negative_bound`

#### 2d. `ServerDataCultivationDetailV1` 漂移 ✅ 已完成（ccdf5036）
- `agent/packages/schema/src/server-data.ts:165` — 类型定义
- `server-data.ts:417` — 已加入 `ServerDataV1` union
- Rust `server/src/schema/server_data.rs:99` 对齐（realm/opened/flow_rate/flow_capacity/integrity/open_progress/cracks_count/contamination_total）

#### 2e. NPC `Attacking` brain action ✅ 已完成（d3f29055）
- `server/src/npc/brain.rs:627` — melee AI system 发 `AttackIntent` 进共享战斗解析器
- `brain.rs:660` — 冷却到期时 `attack_intents.send(AttackIntent { ... })`
- 测试：`CapturedAttackIntents` resource 验证单次触发

### 3. 禁碰目录

本次 worktree **禁止修改**以下路径：

- `client/` — 全部客户端代码
- `agent/packages/tiandao/` — 天道 Agent 业务逻辑
- `library-web/` — 前端静态站点
- `agent/packages/schema/src/client-payload.ts`（ClientPayload 不引入 combat 分支）
- `agent/packages/schema/src/client-request.ts`（ClientRequest 不引入 combat 分支）
- `server/src/schema/client_payload.rs`（同上）
- `server/src/schema/client_request.rs`（同上）

例外：`agent/packages/schema/src/server-data.ts` **允许**修改（2d 修复已并入 main，后续若需新增 combat-side 服务端 payload 仍走此文件）。

### 4. 观测通道

- 新增：Redis `bong:combat_realtime`（每次攻击事务发布，含 attacker/target/damage/contam_delta）
- 新增：Redis `bong:combat_summary`（每 200 tick 随 WorldState 发布，聚合战场摘要）
- **不扩展** `ServerDataPayloadV1` 的 CustomPayload 分支中加入战斗类型
- **不扩展** `ClientRequestV1` / `ClientPayloadV1` 加入战斗指令分支

背景：`WORLD_STATE_PUBLISH_INTERVAL_TICKS = 200`（`server/src/network/mod.rs:41`），~10 秒 @ 20 TPS，不适合实时战斗观测，故需独立通道。

### 5. 身份策略

- `Lifecycle.character_id` **直接用** `canonical_player_id(username)` — 函数位于 `server/src/player/state.rs:146`，格式为 `offline:{username}`
- 战斗持久化文件：`data/players/<canonical>.combat.json`（与现有 `data/players/<canonical>.json` 并列）
- **禁止**引入 `CharacterRegistry` — 跨重生身份查询通过 `character_id` 字段在文件系统检索
- `data/players/` 目录已存在实际文件（已验证：`offline:Kizun3Desu.json`, `offline:Player855.json`）

### 6. 双战斗系统收敛

现有 `/bong combat <target> <health>` 命令（`server/src/network/chat_collector.rs:177`）走 `GameplayAction::Combat` → `PlayerState` 直扣路径（扣 `spirit_qi`，加 `experience`）。

战斗 plan 开工后：
- `/bong combat` **降级为** `AttackIntent` 调试注入入口（保留命令，但路由到新的 `AttackIntentQueue`）
- 旧的 `apply_combat_action`（`gameplay.rs:271`）**停止承载真实战斗语义**，改为 dev-only 调试用途，加 `#[cfg(debug_assertions)]` 门控
- `GameplayAction::Combat` 枚举变体保留但标记 `deprecated`，防止其他代码路径意外引用

### 7. Escalation Triggers（触发条件）

以下情况须**暂停开工，向用户确认设计**再继续：

| 触发条件 | 需要确认的设计决策 |
|----------|-------------------|
| 要支持瞄准向量战斗（方向性攻击） | 需先确认 Valence 1.20.1 `Look` / `pose` 字段是否可在 Server 查询；若不可，方向性战斗只能靠 Chat 命令注入 |
| 要做 C4 终结归档（亡者博物馆） | 需先设计"同名玩家重连"语义：`character_id` 是否随重连自动续用，还是开新档；以及终结后文件保留策略 |
| 要在 AgentCommand 中新增战斗指令类型 | 需先扩展 `CommandType` enum 并同步 TS schema，触碰 `agent/packages/schema/` 需要专项 PR |
| —（NPC `Attacking` 骨架已移入 §2e 前置清单，不再作为 escalation） | — |

### 8. 规模估计

**Large（3d+）**：C1-C2 ECS 新模块 + 事务逻辑 + DeathEvent 收口 + C3 双端 schema + 2d/2c 契约修复，测试覆盖要求全程随行。

---

> 本计划落地 worldview.md §四/五/十二 的战斗机制与死亡-重生流程。**修炼系统单独由 `plan-cultivation-v1.md` 承载**——本计划只定义战斗事务、伤害管线、流派实现，并**统一收口死亡-重生流程**（含修炼侧上报的致死缘由）。

**前置依赖**：
- worldview.md V2（含 §四 战斗系统、§五 战斗流派、§十二 一生记录）已定稿
- `plan-cultivation-v1.md` 已定义 `Cultivation` / `MeridianSystem` / `Contamination` / `QiColor` / `LifeRecord` 等共享 component
- 客户端 inspect UI（经络层）骨架已实现，伤口层数据绑定本 plan 接管
- agent 三层（calamity/mutation/era）+ V1 schema + Redis bridge 已就绪

**与修炼 plan 的边界**：
- 本计划：定义 `Wounds` / `AttackIntent` / `WeaponCarrier` / `Lifecycle` 等战斗专属 component；定义攻击事务（距离衰减、异体侵染、过载施加）；实现四攻三防七流派；**收口** `DeathEvent` 通道（合并战斗端致死 + 修炼端 `CultivationDeathTrigger`）；实现运数判定/概率衰减/遗念/重生惩罚/终结归档；战斗端**写入**修炼 plan 定义的 `Contamination.entries` 与 `MeridianSystem.throughput_current`，由修炼 plan 的 tick 接管后续演化
- 修炼计划：定义所有修炼状态与 tick；定义突破/淬炼事务；通过 `CultivationDeathTrigger` 上报修炼侧致死缘由；监听本 plan 的 `PlayerRevived` / `PlayerTerminated` 事件应用修炼侧惩罚

---

## 0. 设计公理（不可违反）

1. **真元极度排他** — 攻击的本质是污染与置换，不是单纯扣血（worldview §四）
2. **真元极易挥发** — 距离衰减是物理定律，远程仙法在末法是败家行为（worldview §四）
3. **过载有代价** — 强行超流量必然撕裂经脉，写入 `MeridianCrack` 由修炼 plan 接管
4. **防御是处理已入体的攻击** — 不存在外放护盾，三防均针对穿透后的真元污染
5. **死亡有真实成本** — 3 次运数 + 概率衰减，最终会终结；终结归档进亡者博物馆
6. **遗念真实** — 天道不屑编造谎言，临死感知按境界递增详细度
7. **战斗不裁决修炼** — 战斗只产生伤害与污染，是否降境/走火由修炼 plan 的 tick 推演
8. **凡人即弱者** — 所有基线属性参考真实人类（见 §1.4）；NPC 与玩家共享同一基线表；强大体魄**只能通过修炼**（特别是体修 / 沉重色专精）取得，不存在出生天赋差异

---

## 1. 服务端数据模型（Bevy Components）

新增 `server/src/combat/` 模块，承载所有战斗专属状态。**Wounds / Lifecycle 是战斗 plan 独有；Contamination / MeridianCrack 由本 plan 写入但归属修炼 plan**。

### 1.1 核心 Component

```rust
// server/src/combat/components.rs

#[derive(Component)]
struct Wounds {
    entries: Vec<Wound>,
    health_current: f32,           // 物理躯体气血
    health_max: f32,
}

struct Wound {
    location: BodyPart,            // 头/胸/左臂/右臂/左腿/右腿/腹
    kind: WoundKind,               // 切割/钝击/穿刺/灼烧/震爆
    severity: f32,                 // 0-1
    bleeding_per_sec: f32,         // 持续掉气血
    created_at: Instant,
    inflicted_by: Option<Entity>,  // 谁打的（写生平卷用）
}

enum WoundKind { Cut, Blunt, Pierce, Burn, Concussion }

enum BodyPart { Head, Chest, Abdomen, ArmL, ArmR, LegL, LegR }

// 体力（与真元解耦的纯物理 stamina）
// 详细基线/消耗表见 §1.4；本结构定义状态字段
#[derive(Component)]
struct Stamina {
    current: f32,
    max: f32,                     // 默认 100，体修/沉重色专精可拉高
    recover_per_sec: f32,         // 默认 5（jog/walk 状态恢复）
    last_drain_at: Option<Instant>,
    state: StaminaState,          // 当前消耗源（用于决定是否恢复）
}

enum StaminaState {
    Idle,            // 站立 / 静坐：full recover_per_sec
    Walking,         // walk：full recover_per_sec
    Jogging,         // jog：-2/s（净 +3/s）
    Sprinting,       // sprint：-10/s
    Combat,          // 战斗中：-5/s 持续；attack/block 单次扣
    Exhausted,       // current = 0：sprint 不可用，jog 降为 walk 速度
}

// 战斗"实时态"——攻击事务期间维护的瞬时数据
#[derive(Component)]
struct CombatState {
    in_combat_until: Option<Instant>,   // 最近战斗交互 + 15s 视为战斗中（§3.7）
    last_attack_at: Option<Instant>,
    incoming_window: Option<DefenseWindow>,  // 截脉流的 200ms 判定窗口
}

struct DefenseWindow {
    opened_at: Instant,
    duration_ms: u32,
    incoming_qi_color: QiColor,
}

// 流派载体（暗器流"封真元入物"）
#[derive(Component)]
struct WeaponCarrier {
    item_id: ItemId,
    material_grade: MaterialGrade,      // 凡铁/木石 vs 异变兽骨/灵木
    sealed_qi: f32,                     // 封存的真元量
    sealed_color: QiColor,              // 封存时的染色
    sealed_at: Instant,
    decay_per_sec: f32,                 // 离体后即使封存也缓慢挥发
}

enum MaterialGrade {
    Mundane,        // 凡铁/木石（劣质）
    Beast,          // 异变兽骨（优良）
    Spirit,         // 灵木（优良）
    Relic,          // 上古遗物（极稀）
}

// 阵法流：环境方块的真元诡雷
#[derive(Component)]
struct QiTrap {
    placer: Entity,
    sealed_qi: f32,
    sealed_color: QiColor,
    trigger: TrapTrigger,               // Pressure / Proximity / Trip
    expires_at: Instant,                // 载体朽坏后真元流失
    block_pos: BlockPos,
}

// 防御流派状态
#[derive(Component)]
struct DefenseLoadout {
    style: DefenseStyle,
    fake_skin: Option<FakeSkinStack>,   // 替尸流伪灵皮
    vortex_active_until: Option<Instant>, // 涡流流激活时间
}

enum DefenseStyle { JieMai, TiShi, JueLing, None }

struct FakeSkinStack {
    layers: u8,                         // 当前剩余层数
    durability_per_layer: f32,
    total_qi_absorbed: f32,
}

// 死亡-重生 lifecycle（统一收口）
#[derive(Component)]
struct Lifecycle {
    character_id: Uuid,                 // 角色 ID（跨重生保留；与 entity id 解耦）
                                        // 登录时从 CharacterRegistry 恢复；法宝魂契、生平卷均以此为键
    death_count: u32,
    fortune_remaining: u8,              // 运数（初始 3）
    last_death_at: Option<Instant>,
    last_revive_at: Option<Instant>,
    spawn_anchor: Option<BlockPos>,     // 灵龛位置（如有）
    weakened_until: Option<Instant>,    // 重生 3min 虚弱
    state: LifecycleState,
}

enum LifecycleState {
    Alive,
    NearDeath { entered_at: Instant, deadline: Instant },  // 30s 自救窗口
    AwaitingRevival { roll_result: ReviveDecision },        // UI 等待玩家确认
    Terminated { terminated_at: Instant },
}

enum ReviveDecision {
    Fortune,                    // 运数期豁免
    RolledSurvived { p: f32 },  // 劫数期 roll 通过
    RolledFailed { p: f32 },    // 劫数期 roll 失败 → 终结
    DirectTerminate,            // 渡虚劫失败/终结归档强制
}
```

### 1.2 Resource（全局）

```rust
// 距离衰减配置：(qi_color, distance) → 残留比例
#[derive(Resource)]
struct DecayConfig {
    base_decay_per_block: f32,
    color_modifier: HashMap<ColorKind, f32>,
}

// 武器/法术注册表
#[derive(Resource)]
struct WeaponRegistry { ... }

// 致死缘由总表（合并战斗端 + 修炼端上报）
#[derive(Resource)]
struct DeathCauseRegistry { ... }
```

### 1.3 共享但归属修炼 plan 的 component

战斗 plan 仅**写入**这些字段，不负责后续 tick：

| Component | 写入时机 | 写入内容 |
|---|---|---|
| `Contamination.entries` | 攻击命中时（异种真元入体） | 新增 `ContamSource { attacker_id, amount, qi_color }` |
| `MeridianSystem.throughput_current` | 攻击/防御消耗真元时 | 累加瞬时流量；OverloadDetectionTick 检测后写 crack |
| `Cultivation.qi_current` | 攻击消耗 / 防御反应消耗 / 排异之外的扣减 | 直接扣减 |
| `LifeRecord.biography` | 战斗事件 / 死亡事件 / 重生事件 | append 战斗侧 BiographyEntry |

### 1.3b 外部 component（只读引用）

本 plan **仅读取**以下组件，由对应 plan 维护：

| Component | 归属 plan | 本 plan 读取场景 |
|---|---|---|
| `Karma { weight, debts: Vec<KarmaEdge> }` | 业力 plan（TODO，`plan-karma-v1.md`） | §8.2 DeathArbiter 运数豁免判定、§5.6 Karma ±1 影响疗愈成功率、§9 insight 触发 |
| `PlayerIdentity { character_id, name, sect }` | 账号/玩家 plan | 登录时生成，战斗仅作为 `Lifecycle.character_id` 的初始化源 |
| `Inventory { slots: Vec<ItemStack> }` | 物品/背包 plan（TODO，`plan-inventory-v1.md`） | §6 武器所在 slot、§8.2 死亡掉落 50% roll |

在对应 plan 落地前，所有 `Karma.weight` 默认 `0`，`Inventory` 用 vanilla MC 原版（Fabric 侧可直接读 `PlayerInventory`）。

### 1.4 基础属性基线（弱者世界）

**末法残土的人是凡人级别的弱**——所有数值参考真实人类，初始一拳打不死野猪，跑不过一只狼。强大体魄**只能通过修炼**（特别是体修 / 沉重色）取得，不存在"出生即天赋异禀"。NPC 同样使用此基线。

#### 移动速度（参考真人）

```
人类基线 (untrained mortal):
  walk_speed   = 1.4 m/s   (5 km/h，散步)
  jog_speed    = 3.0 m/s   (11 km/h，慢跑，长时持续)
  sprint_speed = 5.5 m/s   (20 km/h，全力冲刺，~10s 内耗尽)
  // 对比 MC 原版：walk 4.317 / sprint 5.612 → 原版"走"≈ 现实"跑"
  // 我们把原版 walk 改为 jog，新加真实 walk 档
```

体力消耗（与 worldview §三 真元解耦的纯物理 stamina；component 见 §1.1 `Stamina`，tick 见 §2.1 `StaminaTick`）：

```
默认 max = 100, recover_per_sec = 5（基线凡人）
体修/沉重色专精可拉高 max（见下方表）

成本：
  walk     : 0      / sec
  jog      : -2     / sec   (净恢复 +3/s)
  sprint   : -10    / sec   → 100 stamina ≈ 10s 极限冲刺
  combat   : -5     / sec   (持续战斗中)
  attack   : -3     / 次
  block    : -2     / 次

current = 0 → state = Exhausted：sprint 不可用，jog 降为 walk 速度
恢复阈值 30% 才退出 Exhausted（防抖动）
```

体修 / 沉重色对 Stamina 的提升：

| 修炼路径 | stamina_max | recover/s |
|---|---|---|
| 凡人 / 醒灵 | 100 | 5 |
| 引气 + 体修 | 120 | 6 |
| 凝脉 + 沉重色 secondary | 150 | 7 |
| 固元 + 沉重色 main | 200 | 9 |
| 通灵 + 体修专精 | 280 | 12 |
| 化虚 | 400 | 15 |

非体修每境 +10 max / +0.5 recover——同样"不专精就是凡人 +一点点"。

#### 修炼 / 染色对速度的影响

只有**修炼线**能拉高基线，且要付出代价。修炼 plan §6 染色谱里"足三阳偏速"和"沉重色偏抗打"即对应物理强化：

| 修炼路径 | 走 | 跑 | 冲 | 来源 |
|---|---|---|---|---|
| 凡人 / 醒灵 | 1.4 | 3.0 | 5.5 | 基线 |
| 引气 + 任一修习 | 1.5 | 3.3 | 6.0 | 真元微强化 |
| 凝脉 + 足三阳通 | 1.6 | 3.8 | 7.5 | 经脉路径选择 |
| 固元 + 足三阳全通 | 1.8 | 4.5 | 9.0 | 真正的"快" |
| 通灵 + 足三阳 + 飘逸/沉重色专精 | 2.0 | 5.5 | 12.0 | 已不像凡人 |
| 化虚 | 2.2 | 6.5 | 15.0 | 天花板 |

**关键设计**：
- 醒灵到固元（前 11.5h 修炼时长）速度提升 < 50%，**不会让新人有"突然变快"的爽感**
- 通灵境才出现明显物理超凡（worldview §三 "半步超脱"）
- 化虚境冲刺也只有 15 m/s（54 km/h），低于现实奥运冠军——这个世界没有飞天遁地

#### 健康与战斗能力基线

```
凡人初始（醒灵）：
  health_max          = 30        (一刀劈死野猪的水平)
  qi_max              = 10        (worldview §三 醒灵真元上限)
  stamina_max         = 100
  fist_base_damage    = 2-3       (一拳打不破皮甲)
  reach.fist          = 0.9       (§3.3 基线)
  perception_radius   = 50 m      (worldview §三 醒灵感知 50 格)

异变兽 / NPC 散修同表，按境界缩放。
```

体修 / 沉重色专精的 health/伤害成长曲线：

| 境界 | 健康 | 拳基础伤害 | 备注 |
|---|---|---|---|
| 醒灵 (基线) | 30 | 2-3 | 凡人 |
| 引气 | 35 | 3-4 | 微强 |
| 凝脉 + 体修方向 | 50 | 5-7 | 能打死野兽 |
| 固元 + 沉重色 main | 80 | 10-15 | 真正的肉搏强者 |
| 通灵 + 沉重色专精 | 130 | 20-30 | 一拳碎石 |
| 化虚 | 200+ | 50+ | 天人 |

非体修的成长曲线：每境健康 +5、伤害 +1。**不专精就是凡人 +一点点**。

#### 与原版 MC 数值对照

| 项 | 原版 MC | 本计划基线 |
|---|---|---|
| 玩家 health | 20 (10 颗心) | **30** (基线) |
| 玩家 walk | 0.21586 BPS = 4.317 m/s | **1.4 m/s** (~32% 原版) |
| 玩家 sprint | 5.612 m/s | **5.5 m/s** (近似) |
| 拳基础伤害 | 1 | **2-3** |
| Reach (vanilla survival) | 3.0 | 见 §3.3 (按武器，徒手 0.9) |

实现：通过 Valence 修改 `entity.attribute` 中的 `generic.movement_speed` / `generic.max_health` / `generic.attack_damage`，每境界变化时 server 推送更新到 client。stamina 是 plan 自定义 component，`OnStaminaChanged` 推 client HUD。

#### NPC 同样基线

worldview §七 提到的所有生物：

| 生物 | 健康 | 速度 | 攻击 | 备注 |
|---|---|---|---|---|
| 噬元鼠 | 5 | 4.0 m/s (jog) | 不掉血，掉 1-2 qi | 真正的"修炼苍蝇" |
| 拟态灰烬蛛 | 15 | 1.0 (休眠) / 6.0 (暴起) | 5 + 麻痹 | 伏击 |
| 野狼 | 20 | 5.5 m/s | 4-6 | 群体威胁 |
| 拾荒散修 | 同对应境界玩家基线 | — | — | 大多数引气-凝脉 |
| 异变缝合兽 | 80-150 | 4.0 | 12-20 + 灵压狂暴 | 固元境难度 |
| 道伥 | 150-300 | 6.0 (伪装时 walk) | 25-40 | 通灵境难度 |
| 垂死大能 | 表象 100 / 实际 500+ | 0 | 300+ (一击秒杀) | 化虚境陷阱 |

**新人战术后果**：
- 醒灵玩家遇野狼 → 30 vs 4-6 dmg + 5.5 m/s 互追 → **打不过也跑不掉**，必须地形规避
- 群体噬元鼠 → 不致死但持续抽 qi → 静坐成本极高
- 一切都按真实弱者节奏，**强大是慢慢挣来的**

### 1.5 战斗基础设施

战斗模块跨多 tick 共享的四大基础设施。所有后续章节（§2-§7）读写这四件：**IntentQueue**（意图流）、**CombatEvents**（事件总线）、**DerivedAttrs**（派生属性）、**客户端自定义包**（Fabric CustomPayload）、**Server raycast 工具**。

#### 1.5.1 IntentQueue（意图总线）

战斗 plan 的所有 `XxxIntent` 通过 Bevy `Events<T>` 机制流转，**不用**独立 Resource 队列——Bevy 调度天然保证"生产者 tick → 消费者 tick"的单向流动。

```rust
// 一个 Intent 一个 Event 类型（便于 reader cursor 独立推进）
#[derive(Event)] struct AttackIntent { ... }
#[derive(Event)] struct DefenseIntent { ... }
#[derive(Event)] struct ApplyStatusEffectIntent { ... }
#[derive(Event)] struct DispelStatusIntent { ... }
#[derive(Event)] struct BreakAttemptIntent { ... }   // cultivation plan 消费
#[derive(Event)] struct ForgeWeaponIntent { ... }
#[derive(Event)] struct SoulBondIntent { ... }
#[derive(Event)] struct TreatPlayerIntent { ... }
#[derive(Event)] struct PlaceQiTrapIntent { ... }
#[derive(Event)] struct ThrowCarrierIntent { ... }
// ... 等等
```

**约定**：
- **单 tick 消费**：Intent 在下一 tick 被消费者读完即 drop（Bevy `Events` 默认 2 frame cleanup，我们改为 1 frame 确保没"上一帧遗留"语义歧义）
- **不可取消**：已发出的 Intent 不能撤销；如需"取消"语义，发消费者能识别的新 Intent（例：`DispelStatusIntent`）
- **优先级靠 tick 顺序表达**：§2 管线已定好谁先跑；Intent 在"生产者 tick 后，消费者 tick 前"窗口内写入即可
- **来源字段**：每个 Intent 必带 `source: IntentSource { Player(Uuid), Agent(AgentId), Environment, Internal }` —— 用于反作弊 + 叙事溯源
- **验证责任**：消费者系统负责校验（reach / cooldown / qi / realm），**生产者只负责"我想做什么"**
- **失败不回滚**：消费者丢弃无效 Intent，同时 emit `CombatEvent::IntentRejected { intent_id, reason }` 供客户端同步

```rust
// 注册所有 Intent event
fn combat_intents_plugin(app: &mut App) {
    app.add_event::<AttackIntent>()
       .add_event::<DefenseIntent>()
       .add_event::<ApplyStatusEffectIntent>()
       // ...
       .configure_sets(Update, IntentSet.before(ResolveSet).before(EventEmitSet));
}
```

#### 1.5.2 CombatEvents（事件总线，对外）

与 Intent 相反方向：战斗系统产出事实，发给客户端 / Agent / 其他 plan。

```rust
#[derive(Event, Serialize)]
enum CombatEvent {
    // 攻击结算
    AttackResolved { attacker, target, hit, damage, body_part, body_color, qi_color },
    AttackMissed { attacker, target, reason: MissReason },
    DefenseTriggered { defender, kind: DefenseKind, effectiveness: f32 },
    IntentRejected { intent_id, reason: RejectReason },

    // 伤口
    WoundApplied { entity, part, severity, kind, bleed_rate },
    WoundDeteriorated { entity, part, new_severity },
    WoundHealed { entity, part, remaining_severity },
    ScarFormed { entity, part },

    // 状态效果（桥接 §7 StatusEffectEvent）
    StatusEffectOp(StatusEffectEvent),

    // 流派特异事件
    BopuOverloadTriggered { entity, frozen_qi_max, duration },
    QiTrapTriggered { trap_id, victims, qi_color },
    WeaponBroken { entity, weapon_kind },
    TreasureDormant { entity, treasure_id, reason },

    // 节流控制（§见下方 1.5.7）
    CombatSummary { entity, window_start, window_end, hits_given, hits_taken, ... },
}

// 发布管道
fn combat_events_publisher(
    mut events: EventReader<CombatEvent>,
    redis: Res<RedisBridge>,
    mut payload_writer: EventWriter<FabricCustomPayload>,
) { ... }
```

**三层分发**：
1. **同进程内部**：其他 Bevy system 用 `EventReader<CombatEvent>` 直接读（如 LifeRecord 写入、narration 触发器）。注意：`chat_collector` 是**入向**玩家聊天处理器（player → agent），不消费 CombatEvent。
2. **Redis** `CHANNEL_COMBAT_EVENT`：Agent 层消费（§11.1）
3. **Fabric CustomPayload**：客户端渲染 HUD/特效（§12）

#### 1.5.3 DerivedAttrs（派生属性，聚合结果）

所有"被修饰后的最终数值"集中到一个 Component，**只读下游消费**，由 `AttributeAggregateTick`（§2 C4c / §7.6）每 tick 重算。其他 tick（Attack/Defense/Movement/Stamina）**只读 DerivedAttrs**，不直接读 base stat。

```rust
#[derive(Component, Default, Clone)]
pub struct DerivedAttrs {
    // 移动类
    pub speed_walk: f32,        // base 1.4 × 加成
    pub speed_jog: f32,         // base 3.0
    pub speed_sprint: f32,      // base 5.5
    pub stamina_drain_mul: f32, // 加速/减速影响 stamina 消耗系数

    // 攻击类
    pub attack_damage_bonus: f32,      // additive 加到 base_damage
    pub attack_damage_mul: f32,        // multiplicative
    pub attack_speed_mul: f32,         // 攻速
    pub qi_throughput_cap: f32,        // 单次最大 qi_invest
    pub qi_conduit_bonus: f32,         // 武器导流加成

    // 防御类
    pub incoming_damage_mul: f32,      // 0.8 = 减伤 20%
    pub defense_window_bonus_ms: f32,  // 截脉窗口延长
    pub dodge_chance: f32,             // 闪避
    pub resist_tags: EffectTags,       // 状态效果抗性（来自防御 + 被动功法）
    pub resist_magnitude: f32,         // 抗性强度 0-1

    // 真元类
    pub qi_regen_per_sec: f32,         // 静坐/战斗两套不同倍率
    pub qi_max_bonus: f32,

    // 感知类
    pub vision_range: f32,             // Blinded 缩至 3
    pub aura_detect_realm_diff: i32,   // 能感知比自己高几阶
    pub stealth_level: i32,            // Invisible 覆盖

    // 特殊
    pub flight_enabled: bool,          // 化虚功法激活
    pub phasing: bool,                 // 替尸 1s 无敌
    pub tribulation_locked: bool,      // 渡劫不可干预
}
```

**聚合流程**（见 §7.6 AttributeAggregateTick 伪码）：
1. `DerivedAttrs::reset_from_base(Cultivation, Lifecycle, WeaponCarrier)` —— 基线来自境界/武器/Lifecycle.weakened_until
2. 遍历 `StatusEffects.active` 的 `AttrModSpec`，按 op（Add/MulBase/MulTotal）累加
3. 遍历 `Techniques.equipped` 的被动加成（flight_enabled / aura_detect 等来自功法）
4. 最终 clamp 到合法范围（speed >= 0, damage >= 0, ...）

#### 1.5.4 客户端 → 服务端 CustomPayload 包

原版 MC `Interact` / `UseItem` packet **不够用**（无坐标、无 qi_invest、无 defense_style、无目标部位），所有战斗输入走 Fabric CustomPayload。

**Identifier 约定**：`bong:combat/<动作>`。Schema 在 `agent/packages/schema/src/combat-packets.ts` 定义（双端 codegen）。

```typescript
// 攻击
bong:combat/attack {
  target_entity_id: i32,           // 目标 entity（server 再验距离/视线）
  target_body_part?: BodyPart,     // 客户端 hint，server 最终以 raycast 为准
  weapon_slot: i32,                // 哪个 inventory slot 的武器
  qi_invest: f32,                  // 0..=DerivedAttrs.qi_throughput_cap
  style_tag?: StyleTag,            // Bopu/Anqi/...  选中流派
  timestamp_ms: i64,               // 反作弊：窗口化校验
}

// 防御（玩家主动切防御姿态）
bong:combat/defense_stance {
  kind: "JieMai" | "TiShi" | "JueLing" | "None",
  activated_at_ms: i64,
}

// 法术体积控制（§3.6）
bong:combat/spell_volume {
  radius: f32,         // 玩家自选碰撞半径
  velocity_cap: f32,   // 自选飞行速度上限
}

// 暗器投掷
bong:combat/throw_carrier { carrier_item_id, target_pos, power }

// 阵法布置
bong:combat/place_trap { trap_kind, block_positions, qi_invest, trigger }

// 死亡画面响应
bong:combat/death_choice { accept_revive: bool }

// 魂契 / 锻造 / 治疗（长期操作）
bong:combat/soul_bond_start { treasure_id }
bong:combat/forge_weapon_start { kind, materials[], quality_target }
bong:combat/treat_player { target_entity_id, treatment_kind }
```

**服务端接收管线**：
```
Valence ClientPacket → PacketListener<CombatPacket>
  → 反作弊校验（见下方 1.5.6 清单）
  → emit 对应 Intent event
  → IntentQueue 系统消费
```

**Server → Client CustomPayload**（反向）同样走 `bong:combat/<类型>`：`combat_event`（推 CombatEvent）/ `status_snapshot`（推 StatusEffects 快照）/ `derived_attrs_sync`（HUD 用）/ `death_screen` / `terminate_screen`。

#### 1.5.5 Server Raycast 工具

`§3.1 Step 4` 和 `§3.4 命中部位检测` 都需要 raycast。Valence 本身**不提供** raycast（它只是协议层 + ECS），需要自建。

```rust
// server/src/combat/raycast.rs
pub struct HitProbe {
    pub entity: Entity,
    pub hit_point: Vec3,
    pub distance: f32,
    pub body_part: BodyPart,
    pub aabb_face: AabbFace,  // 击中的是正面/背面/侧面
}

pub fn raycast_entities(
    world: &World,
    origin: Vec3,         // attacker.eye
    direction: Vec3,      // normalized look
    max_distance: f32,    // weapon_reach.max
    filter: EntityFilter, // 排除 self / 队友
) -> Option<HitProbe> {
    // 实施：遍历候选 AABB（broadphase：按 origin 附近 block grid 筛），
    // 对每个 AABB 做 slab-test 得 t_near/t_far
    // 取 t_near 最小且 > 0 的
    // 从 hit_point.y 相对 AABB.y_min 比例 + pose（站立/下蹲）分类 BodyPart
}
```

**实施选型（按优先级）**：
1. **方案 A（推荐）**：自写 slab-test AABB raycast + 简单 broadphase（按 chunk + entity bucket 分桶）
   - 优点：零依赖，对 Valence ECS 友好
   - 缺点：需要维护自己的空间索引
2. **方案 B**：引入 `bevy_rapier3d`（physics crate）做 raycast
   - 优点：成熟，带连续碰撞检测
   - 缺点：和 Valence 的 ECS pipeline 集成要包装；overkill 于"只做 raycast"
3. **方案 C**：借 `parry3d`（rapier 的几何层，无物理仿真）
   - 中间选项：拿到 AABB/OBB raycast 又不引入 physics

**决策**：C1 初版用方案 A（AABB + slab test，50 行代码）；C5 流派扩展若需要复杂形状（刀光扇形 / 矛刺 OBB）再切方案 C。

**Pose → AABB 映射**：
```
Pose::Standing   → AABB 1.8 × 0.6 × 0.6
Pose::Crouching  → AABB 1.5 × 0.6 × 0.6
Pose::Prone      → AABB 0.6 × 0.6 × 1.8
Pose::Flying     → AABB 1.8 × 0.6 × 0.6（斜向 pitch，旋转 AABB 或用 OBB）
```

BodyPart 分类 Y 比例（§3.4 已有公式，此处仅为 raycast 消费方）：
```
hit_y_ratio = (hit_point.y - aabb.y_min) / aabb.height
  ratio >= 0.88   → Head
  ratio >= 0.72   → Neck
  ratio >= 0.50   → Chest
  ratio >= 0.32   → Abdomen
  ratio >= 0.08   → Leg
  else            → Foot
Arms: hit_point.xz 偏离 center > 0.18 时覆盖上面分类
```

#### 1.5.6 反作弊校验清单（server 必重算）

CustomPayload 入站后，**以下字段一律不信客户端**：

| 字段 | 客户端传 | Server 行为 |
|---|---|---|
| 攻击者位置 / 视线 | 不传 | 用 server 权威 `Transform` + `Head.pitch/yaw` |
| 目标命中点 / body_part | 仅作 hint | **Server raycast 重算**；不一致以 server 为准（可选：如偏差 > 0.3m 记一次异常） |
| `qi_invest` 数值 | 传 | clamp 到 `[0, DerivedAttrs.qi_throughput_cap]`；超出记异常但不拒绝 |
| `weapon_slot` | 传 | 验 slot 是否存在对应武器 + 耐久 > 0 + kind 与 style_tag 兼容 |
| cooldown | 不传 | server 维护每 entity `last_action_at[ActionKind]`，未到 cd 直接丢 |
| reach | 不传 | raycast 距离 > weapon_reach.max 直接 miss |
| 目标可见性（负灵域/隐身） | 不传 | server 查 zone + target.StatusEffects |
| 境界门槛（Technique/Treasure） | 不传 | server 查 Cultivation.realm |
| durability 扣减 | 不传 | server 结算后写入；client 只读同步 |
| reward roll | 不传 | server rng |

**异常记录**：server 每次"客户端 hint 与 server 实际结果严重不符"时累加 `AntiCheatCounter { entity, kind, count }`。阈值触发后推 Redis 管理侧 channel（不在本 plan 消费，交给运维 plan）。

#### 1.5.7 事件对 Agent 的节流（早期约定，实施细化延后）

高频 CombatEvent 不能每条都丢给 LLM。发布前过一层"聚合路由"：

| 事件类型 | 节流策略 |
|---|---|
| 普通 AttackResolved / WoundApplied / StatusEffectOp | 聚合：每 entity 5s 窗口 → 合并成 `CombatEvent::CombatSummary` 推 Agent |
| DeathEvent / NearDeath / Terminated / TribulationStart | 实时（critical 优先级） |
| WeaponBroken / TreasureDormant / ScarFormed | 实时（叙事价值高） |
| BopuOverload / QiTrapTriggered | 实时 |
| 客户端 HUD 同步 | 全量实时（本地 UI 不走 Agent） |

Redis 发布分 channel：`CHANNEL_COMBAT_REALTIME` / `CHANNEL_COMBAT_SUMMARY`，Agent 自行订阅所需。

具体 summary 字段 + 窗口策略在 C2 实施时细化；本节仅确立**"必须节流"**这件事。

---

## 2. Tick 管线（Bevy Systems）

按战斗物理推进，按优先级顺序：

```
// 输入层：消费 Intent
[C0a]   AttackIntentResolver (event)   消费 AttackIntent → 走 §3.1 六步 → 产出 CombatEvent
[C0b]   DefenseIntentResolver (event)  消费 DefenseIntent → 切 DefenseLoadout
[C0c]   StatusEffectApplyTick (event)  消费 ApplyStatusEffectIntent + DispelStatusIntent（§7.6）
[C0d]   ForgeSoulBondTreatResolver (event) 消费长周期 Intent（铸造/魂契/外科救援）

// 持续物理层
[C1]    QiDecayInFlightTick (10Hz)     离体真元（暗器/法术）按距离衰减
[C2]    StatusEffectTick (5Hz)         active effect 周期 on_tick + duration 扣减（§7.6）
[C3]    WoundBleedTick (1Hz)           伤口持续掉 health
[C4]    CombatStateTick (1Hz)          过期清理 in_combat_until / DefenseWindow
[C5]    QiTrapDecayTick (1Hz)          阵法诡雷真元流失
[C6]    WeaponCarrierDecayTick (1Hz)   离体武器封存真元挥发
[C7]    NearDeathTick (1Hz)            NearDeath 30s 自救计时
[C8]    WeakenedTick (1Hz)             重生虚弱 debuff 倒计时
[C9]    StaminaTick (5Hz)              体力消耗/恢复

// 聚合层（在所有修饰性 tick 之后，所有消费者之前）
[C10]   AttributeAggregateTick (5Hz)   聚合 StatusEffects + Techniques 被动 → DerivedAttrs（§1.5.3 / §7.6）

// 输出层：事件驱动收口 + 对外广播
[C11]   DeathArbiterTick (event)       合并所有 DeathEvent 走完整死亡流程
[C12]   CombatEventsPublisher (event)  CombatEvent → Redis + Fabric CustomPayload（§1.5.2）
                                       带 §1.5.7 节流：realtime vs summary 分流
```

**调度约定**：
- C0* (Intent 消费) 必须在 C1-C9 之前，保证同一帧内"动作 → 后果"可见
- C10 AttributeAggregateTick 在 C1-C9 之后、C11-C12 之前，确保下游读到的 DerivedAttrs 是最新聚合值
- Bevy `SystemSet`：`IntentSet → PhysicsSet → AggregateSet → ResolveSet → EmitSet`

### 2.1 关键 tick

#### QiDecayInFlightTick

```
for each in-flight projectile/spell:
  remaining = qi_payload × (1 - decay_per_block × distance × color_modifier)
  if remaining <= 0:
    despawn (法术虚化)
  else:
    update payload
```

距离衰减公式参考 worldview §四：
- 贴脸 (0 格): 100%
- 10 格: ~40%（默认）
- 50 格: ~0%（除非锋锐/凝实色 + 优良载体）

#### WoundBleedTick

```
for each Wounds:
  total_bleed = sum(entries.bleeding_per_sec)
  health_current -= total_bleed × dt
  if health_current <= 0:
    emit DeathEvent { cause: BleedOut, entity }
```

#### NearDeathTick

```
for each Lifecycle in NearDeath state:
  if now >= deadline:
    emit DeathEvent { cause: NearDeathTimeout, entity }
  // 期间允许：自救丹药 / 外科救援 / 队友疗愈
  // 任一恢复 health > 5% → 退出 NearDeath
```

#### StaminaTick

```
for each Stamina:
  delta = match state:
    Idle | Walking  =>  +recover_per_sec
    Jogging         =>  +recover_per_sec - 2     // 净 +3 默认
    Sprinting       =>  -10
    Combat          =>  -5
    Exhausted       =>  +recover_per_sec × 0.5   // 力竭恢复减半
  current = clamp(current + delta × dt, 0, max)
  
  // 状态机切换
  if current <= 0 AND state in {Sprinting}:
    state = Exhausted
    // 移动系统检测 Exhausted → sprint 输入降级为 walk
  if state == Exhausted AND current >= max × 0.3:
    state = Idle   // 恢复阈值，避免抖动
  
  // 按 5Hz 推 client 同步：仅在 current 跨越 25/50/75% 阈值或状态切换时推

战斗扣减（直接调用，不在 tick 内）：
  on AttackIntent execute    : current -= 3
  on BlockIntent (jiemai)    : current -= 2
  on Sprint key down + move  : state = Sprinting
```

---

## 3. 攻击事务

每次攻击是完整的 server 权威事务。

### 3.1 流程（六步）

```
Step 1 [Client → Server]:
  AttackIntent {
    attacker, weapon_or_spell_id,
    qi_invest: f32,
    target: Entity | BlockPos,
    style: AttackStyle,   // Bopǔ/Anqì/Zhènfǎ/Dúgǔ
  }

Step 2 [Server 校验]:
  - Cultivation.qi_current >= qi_invest？
  - meridian.throughput_current + qi_invest 是否超 1.5× 安全阈值？
    └ 超 → 允许出手，但写入 throughput_current（修炼 plan 的 OverloadDetectionTick 会施加 crack）
  - 染色契合：style 与 QiColor.main 不匹配时效率下降（施法效率 -20% 起）
  - **Reach 校验（近战类必查，见 §3.3）**：
      melee_distance = horizontal_dist(attacker.eye, target.aabb_nearest_point)
      if melee_distance > weapon_reach.max: 拒绝 Intent，返回 OutOfReach
  - Cultivation.qi_current -= qi_invest
  - meridian.throughput_current += qi_invest

Step 3 [传播衰减]:
  hit_qi = qi_invest × distance_decay(distance, qi_color, carrier_grade)
  if hit_qi <= 0: 攻击虚化，事务结束（仅扣施法成本）

Step 4 [命中判定]:
  - 物理武器：server raycast (attacker.eye, attacker.look, weapon_reach.max)
                求与 target.aabb 交点 → hit_point
  - 法术：作为带自身碰撞体积的 projectile 模拟（见 §3.6），与 target.aabb
          做 swept-volume 相交 → hit_point = 相交瞬间的接触点
  - 暗器流：projectile.last_segment_intersection(target.aabb) → hit_point
  - 未相交 → 攻击未命中，事务结束（qi 已扣，无伤害）
  - body_part = classify_body_part(hit_point, target.pos, target.pose)  // 见 §3.4
  - wound_damage / contam_amount 应用部位倍率（见 §3.4 表）

Step 5 [防御者结算]:
  trigger DefenseReaction (见 §4)
  if 残留 wound_damage > 0:
    add Wound { location, kind, severity, bleeding, inflicted_by }
    Wounds.health_current -= wound_damage
  if 残留 contam_amount > 0:
    Contamination.entries.push(ContamSource { attacker, amount, qi_color })
    // 修炼 plan 的 ContaminationTick 接管排异
  if Wounds.health_current <= 0:
    emit DeathEvent { cause: HealthZero }
  if Wounds.health_current <= 0.05 × health_max:
    enter NearDeath state

Step 6 [事件广播]:
  emit CombatEvent::AttackResolved { ... }（§1.5.2）
  CombatEventsPublisher 按 §1.5.7 节流：
    - 命中/未命中走 CHANNEL_COMBAT_SUMMARY（5s 聚合）
    - 触发死亡 / NearDeath / Terminated 走 CHANNEL_COMBAT_REALTIME
  反向推 client：outbound packet `bong:combat/combat_event` → 客户端伤害特效
  关键事件（致死/爆脉/截脉成功）→ 单独 priority 事件
  写入 LifeRecord.biography
```

### 3.2 距离衰减细则

```
decay_factor(distance, color, grade) =
  base_decay = 1 - (distance × 0.06)              // 默认每格 6% 衰减
  color_bonus = match color:
    Sharp:    +0.02 / block (锋锐线状流动，损耗低)
    Solid:    +0.03 / block (凝实附着载体，损耗最低)
    Light:    +0.025 / block (飘逸离体不易散)
    Mellow:   -0.02 / block (温润太散)
    others:   0
  carrier_bonus = match grade:
    Mundane: 0
    Beast:   +0.04 / block
    Spirit:  +0.05 / block
    Relic:   +0.07 / block
  return clamp(base_decay + color_bonus + carrier_bonus, 0, 1)
```

worldview §四 案例对齐：
- 体修贴脸 50 qi → 50 qi 命中 ✅
- 普通玩家 10 格火球 50 qi → ~20 qi 命中 ✅
- 异变兽骨暗器 50 格 50 qi → ~40 qi 命中（80% 保留）✅

### 3.3 Reach 与"贴脸"定义

近战 reach **按武器/招式分级**，徒手必须真正贴上去——这是 worldview §四 体修流派叙事的物理基础。原版 MC 3 格 reach 太宽松会让"贴脸"失去意义。

**参考真人数据**（成年男性）：单臂 ~75 cm + 肩部前伸 + 转体 → 纯站桩拳 ~0.9 m；带垫步直拳 ~1.3 m；长剑挥砍 ~2.5 m。

```rust
struct AttackReach {
    base: f32,        // 武器自身长度（站桩可达）
    step_bonus: f32,  // 攻击垫步额外（消耗少量 qi 才能用）
    max: f32,         // 硬上限（base + step_bonus）
}

const FIST_REACH:    AttackReach = { base: 0.9, step_bonus: 0.4, max: 1.3 };
const DAGGER_REACH:  AttackReach = { base: 1.2, step_bonus: 0.4, max: 1.6 };
const SWORD_REACH:   AttackReach = { base: 2.0, step_bonus: 0.5, max: 2.5 };
const SPEAR_REACH:   AttackReach = { base: 2.6, step_bonus: 0.4, max: 3.0 };
const STAFF_REACH:   AttackReach = { base: 2.4, step_bonus: 0.4, max: 2.8 };
// 暗器/法术不走 reach，由 §3.2 距离衰减管线管
```

**Interact 也分级**（不只是 attack）：

| 交互类型 | reach | 备注 |
|---|---|---|
| 徒手交互（采集灵草、点火、按机关） | **1.5** | 比攻击 reach 略宽（不需精准） |
| 持镰刀/铲采集 | 2.0 | 工具本身有长度 |
| 持长杆点远处机关 | 2.8 | 杆类工具的合理用途 |
| 方块交互（破/放） | 1.8 | 略低于原版 4.5，强迫"靠近做事" |
| 灵识感知（无接触）| 由境界 + 染色决定 | 不算物理 reach |

实现：在 `WeaponRegistry` / `ItemRegistry` 注册 `interact_reach: f32`，攻击/交互前都校验。

**贴脸的物理含义**：
- **0-1.0 格**：徒手拳掌可达 → "贴脸" 真元零损 → 体修黄金距离
- **1.0-1.6 格**：短刃 / 拳带步 → 仍算近战
- **1.6-2.5 格**：长兵器专属 → 体修在这里挨打无法还手
- **>3.0 格**：所有近战不可达，只能远程（进入 §3.2 距离衰减）

战术后果：
- 体修 vs 长矛修：体修必须**冲进 1 格内**才能爆发，否则被距离压制
- 长兵器修被挤进 1 格 → 武器废，**强制切入徒手 reach 0.9**（武器太长无法施展）
- worldview §四"如野兽般肉搏"的画面 = 双方都被迫挤进 1 格内 = 真元零距离倾泻

`AttackIntent` Step 2 的 reach 校验返回 `OutOfReach` 时**不扣 qi**——这是普通误操作的善意，不是惩罚。但若启用了 step_bonus（攻击垫步），消耗已扣无法返还。

### 3.4 命中部位检测

原版 MC 协议**没有命中部位概念**（玩家就是一个 1.8×0.6 AABB），必须 server 端自己算。

#### 命中点来源

| 攻击来源 | 命中点获取方式 |
|---|---|
| 近战（拳/刀/剑/矛） | server raycast：`(attacker.eye, attacker.look_dir, weapon_reach.max)` 与 `target.aabb` 求最近交点 |
| 暗器 / 投射物 | 服务端自模拟轨迹，最后一段与 target.aabb 相交即命中点 |
| 远程法术 | swept AABB 与 target.aabb 相交瞬间的接触点（见 §3.6 体积模型） |
| 阵法触发 | 按触发方式：地刺 = `LegL/LegR`；面板 = `Chest` |

**关键**：客户端 `Interact` packet 只带 `target_entity_id`，**不带命中坐标**——所以近战必须 server raycast，不能信 client。

#### 部位分类（Y 坐标分段）

```rust
fn classify_body_part(hit: Vec3, target: &EntityPos, pose: Pose) -> BodyPart {
    let h = pose_height(pose);                         // Standing 1.8 / Sneaking 1.5 / Swimming 0.6
    let rel_y = ((hit.y - target.feet.y) / h).clamp(0.0, 1.0);
    // 横向偏移：攻击者视角的左右（用 target 朝向旋转后的 local x）
    let local = world_to_target_local(hit, target);
    let lateral = local.x;  // -0.3..+0.3 通常

    match rel_y {
        y if y > 0.88 => BodyPart::Head,
        y if y > 0.55 => {
            if lateral.abs() > 0.18 {
                if lateral > 0.0 { BodyPart::ArmR } else { BodyPart::ArmL }
            } else {
                BodyPart::Chest
            }
        }
        y if y > 0.35 => BodyPart::Abdomen,
        _             => if lateral > 0.0 { BodyPart::LegR } else { BodyPart::LegL },
    }
}
```

**注意 Pose 敏感**：潜行 / 游泳 / 滑翔 身高完全不同，硬编 1.8 会让"打头"全部错位。骑乘状态以坐骑为基准另算。

#### 部位倍率表

```
BodyPart  | wound_dmg | contam | bleeding | 命中难度 | 特殊效果
----------|-----------|--------|----------|----------|--------
Head      |   ×2.0    |  ×1.5  |  ×1.5    |   难     | severity > 0.5 → 1s 眩晕
Chest     |   ×1.0    |  ×1.0  |  ×1.0    |   易     | 默认目标，无特殊
Abdomen   |   ×0.9    |  ×1.2  |  ×1.3    |   中     | 内脏出血，bleeding 衰减慢
ArmL/ArmR |   ×0.7    |  ×0.8  |  ×0.8    |   中     | severity > 0.4 → 该侧武器掉落
LegL/LegR |   ×0.6    |  ×0.7  |  ×1.0    |   易     | severity > 0.3 → 移动速度 -40%
```

#### 防作弊

- 命中点**完全 server 计算**，client 不参与
- 攻击者 eye/look 用 server 当前 tick 的位置（PlayerPosition 包已应用），不信 client 重放
- raycast 距离硬上限 `weapon_reach.max`，超出无效
- target.pose 用 server 权威态（client 上报 SneakState 但 pose 由 server 推出）

#### 与现有 inspect UI 衔接

`Wounds.entries` 已带 `BodyPart` 字段（§1.1），客户端 inspect 伤口层直接渲染 worldview §六 inspect 画面里"几条经脉断了"的视觉对应——真实部位、真实伤型。

#### 法术大体积的多部位命中

§3.6 体积法术的 radius 大到可以同时罩住多个 BodyPart 时，按"罩住面积"分配伤害，**不是单点接触**：

```
on swept-volume hit:
  if attack_kind == Spell AND spell.radius >= 0.4:
    intersected_parts = []
    for part in target.skeleton.parts:
      overlap_volume = aabb_intersect_volume(spell.aabb, part.local_aabb)
      if overlap_volume > 0:
        intersected_parts.push((part, overlap_volume))
    
    total_overlap = sum(overlap_volume)
    for (part, vol) in intersected_parts:
      share = vol / total_overlap
      apply_damage_to_part(part, wound_damage × share, contam × share)
      // 每个被罩住的部位都生成独立 Wound entry
  else:
    // 单点命中（针状法术 / 物理武器 / 暗器）
    apply_damage_to_part(body_part, wound_damage, contam)
```

部位倍率仍按 §3.4 表生效——一个 1.0m 大球同时罩住 Head + Chest + ArmL + ArmR 时：
- 各部位按重叠体积分摊基础伤害
- 每个部位仍乘自己的倍率（Head ×2.0、Arm ×0.7…）
- 写入 4 条独立 `Wound` entry（生平卷可见"被一道符篆同时打中头胸双臂"）

战术后果：
- 大球 = **保底命中多个部位**，不需要瞄头
- 大球贴脸 1.0m + 站立目标 1.8m → 必中 Head + Chest + 单 Arm，等于强制 ×3.7 部位倍率累积
- 这是法术 radius 三角的第四角："**不需要瞄准**"——配合速度慢/逸散快/必须贴脸的代价

### 3.5 法术体积调控（核心机制）

**法术不自动命中**——它是带物理体积的真元投射体，玩家**主动权衡"易命中 / 高伤害"与"逸散 / 速度慢"的三角**。

#### 公理

worldview §四 "真元极易挥发"的延伸：法术体积越大，单位时间暴露在空气中的真元表面积越大，逸散越快。强行做大球 = 给天地交更多过路费。

#### 调控参数

```rust
struct SpellShape {
    radius: f32,          // 球形碰撞半径，玩家施法时滑块选择 (0.1..2.0 m)
    qi_invest: f32,       // 注入真元
}

// 派生属性（公式见下）
struct SpellRuntime {
    radius: f32,
    velocity: f32,                   // 飞行速度（格/秒）
    decay_per_sec: f32,              // 真元逸散速率（qi/秒）
    impact_damage_factor: f32,       // 命中伤害倍率
    current_qi: f32,                 // 实时残量（每 tick 衰减）
}
```

#### 派生公式

```
velocity         = base_speed × (R0 / radius)^0.7         // 体积越大越慢（截面阻力）
decay_per_sec    = base_decay × (radius / R0)^2           // 表面积 ∝ r²
impact_damage    = qi_invest × radius_damage_curve(radius)
                   // 大球罩住更多部位 + 真元密度更厚 → 伤害高
                   // radius_damage_curve(0.1) = 1.0
                   // radius_damage_curve(0.5) = 1.4
                   // radius_damage_curve(1.0) = 1.8
                   // radius_damage_curve(2.0) = 2.2
其中 R0 = 0.3（基准小球，针状法术）
```

举例（base_speed = 30 格/秒，base_decay = 5 qi/秒，qi_invest = 50）：

| radius | velocity | decay/sec | 飞 10 格剩余 qi | 命中伤害 | 战术定位 |
|---|---|---|---|---|---|
| 0.1 (针) | ~60 格/s | 0.6 | ~49 | ×1.0 → 49 | 远程精狙，要瞄准 |
| 0.3 (球) | 30 格/s | 5.0 | ~48 | ×1.2 → 58 | 标准法术 |
| 0.6 (大球) | 17 格/s | 20 | ~38 | ×1.5 → 57 | 中距压制，省瞄准 |
| 1.0 (罩) | 11 格/s | 55 | ~0（在到之前散光） | ×1.8 → 0 | **只能贴脸打** |
| 1.5 (壁) | 7.5 格/s | 125 | 飞 1 格就散完 | — | 仪式 / 阵法专用 |

公式三角形：
- **想要伤害高（大 radius）** → 速度慢、逸散快 → **必须近距使用**
- **想要远程命中（活到目标）** → 必须小 radius → 伤害低 + 难瞄
- **唯一两全** = 大球 + 贴脸 = 体修把"罩"按在敌人胸口引爆（worldview §四 "把法术直接塞进敌人身体里引爆"）

#### Swept-volume 相交（命中判定）

每 server tick 把法术的当前 AABB 从 `prev_pos` 扫到 `next_pos`（capsule swept），与 target.aabb 求是否相交：
- 命中：取首次接触点为 `hit_point`，伤害 = `current_qi × impact_damage_factor`
- 未命中且 `current_qi <= 0`：despawn（"虚化"）
- 未命中且飞出 max_range：despawn

#### 与暗器流的边界

| 维度 | 法术 (§3.6) | 暗器 (§5.2) |
|---|---|---|
| 真元载体 | 真元自身就是体积 | 真元封在物理载体（骨/木）里 |
| 逸散 | 表面积 ∝ r² 快速逸散 | 载体锁住真元，缓慢挥发 |
| 远程衰减 | radius 越大越被天地吸 | 载体越好越保 qi |
| 命中后 | 真元当场释放为 contam + wound | 载体破碎瞬间注射 |
| 适用境界 | 染色匹配的所有人 | 重资产，需炼器材料 |

法术 = "**临时 / 即用**"，暗器 = "**预储 / 可远射**"——选哪个取决于 carrier 资源是否充足。

#### 客户端 UI

施法 HUD：
- radius 滑块（0.1-2.0 m），实时预览：球体大小、velocity、预估飞行距离（按当前 qi_invest 算 current_qi 归零的距离）
- qi_invest 滑块
- 二者联动显示 "在 N 格内可命中" 提示——**这就是玩家的物理直觉**：贴脸用大球，远程用细针

#### 对染色的依赖

- **Sharp（锋锐）**：天然偏小 radius，配针状法术 → 远程精狙
- **Heavy（沉重）**：可承受大 radius，逸散惩罚减半 → 大球贴脸压制
- **Light（飘逸）**：所有 radius 的逸散 -25% → 唯一适合中距大球的染色
- **Solid（凝实）**：法术命中后 contam 残留时间 +30%（真元附着力强）

具体加成数值在 `QiColorConfig` 中调，C2 起初版。

### 3.6 异体排斥写入契约

战斗 plan 写入 `Contamination` 后即"撒手"，由修炼 plan 推演排异：
- 写入：`{ attacker_id, amount, qi_color }`
- 修炼 plan ContaminationTick：按 10:15 亏损消耗防御者真元中和
- 若防御者真元为负 → 修炼 plan 添加 MeridianCrack 并可能 emit CultivationDeathTrigger { cause: ContaminationOverflow }

### 3.7 脱战判定

`CombatState.in_combat_until: Option<Instant>` 控制"战斗中/脱战"二态，决定 stamina / qi / bleeding 等恢复速率。

#### 3.7.1 进入战斗（刷新 in_combat_until = now + 15s）

任一事件触发：
- 本人 emit `AttackIntent`（主动攻击）
- 本人成为 `AttackResolved.target`（被命中，命中才算，miss 不算）
- 本人 emit `DefenseIntent` 切防御姿态
- 本人被施加任何 `tags & EffectTags::DEBUFF != 0` 的 StatusEffect
- 本人被挂 `SoulMarked` 且 marker 在 30 格内（持续 marking 视同战斗）

#### 3.7.2 保持战斗（续时）

进入战斗后，上述任一事件再次发生 → `in_combat_until = now + 15s`（滑动窗口）。

#### 3.7.3 脱战条件（全部满足才脱战）

```
now >= in_combat_until
  AND 30 格内无敌意实体：
      - PvP 对立玩家（karma 判定或双方在仇恨列表）
      - 敌意 NPC（big-brain Scorer 输出 Aggro > 阈值）
  AND 不在 NearDeath 状态
  AND 不在 TribulationLocked 状态
→ CombatState.in_combat = false
→ emit CombatEvent::Disengaged { entity }
```

脱战检测放在 `CombatStateTick` (1Hz，§2 C4) 内。

#### 3.7.4 战斗中 vs 脱战的数值差

| 字段 | 战斗中 | 脱战 |
|---|---|---|
| `qi_regen_per_sec` | base × 0.3 | base × 1.0 |
| `stamina.recover_per_sec` | base × 0.5 | base × 1.0（+ 静坐时 ×2） |
| Bleeding 效果可否被 `SelfMeditate` 中断 | ❌ 不行 | ✅ 可以 |
| 允许 `StartBreakthroughIntent`（修炼突破） | ❌ 拒绝 | ✅ 允许 |
| 允许长周期 Intent（铸造/魂契/外科救援） | ❌ 拒绝 | ✅ 允许 |
| Scar 形成概率 | base | base ×0.7（脱战后疗愈更稳） |
| `WithdrawSafely` insight 可触发 | — | 脱战瞬间 emit，作为 §9 insight 候选 |

#### 3.7.5 强制脱战

- 传送（worldview 允许的 spawn_anchor 召回）→ 立即 in_combat = false
- Lifecycle 进入 AwaitingRevival / Terminated → 立即 false
- 渡劫 TribulationStart → 切 TribulationLocked（战斗状态"冻结"但不算脱战）

#### 3.7.6 与"搜打撤"循环的关系

本节只定义**脱战的技术实现**。worldview 的"搜打撤"（Scout-Hit-Retreat）玩法循环——包括**逃跑可行性、追击速度差、隐匿脱敌、痕迹追踪、复仇追查**——属于独立的**移动/侦察/追击 plan**（TODO: `plan-stealth-chase-v1.md`，待定）。本 plan 仅负责：
- 提供脱战原语（in_combat 状态 + 15s 滑窗 + 30 格敌意检测）
- 暴露 `CombatEvent::Disengaged` 给侦察 plan 消费
- 在 `DerivedAttrs` 中预留 `stealth_level: i32`（§1.5.3）字段供隐匿 plan 写入

---

## 4. 防御反应

worldview §五 三流派的服务端实现。

### 4.1 截脉/震爆流（JieMai）

```
触发：Step 5 命中前，server 给玩家 200ms DefenseWindow
玩家在窗口内按防御键 → BlockIntent
  if Cultivation.qi_current >= jiemai_cost (默认 5 qi):
    qi_current -= jiemai_cost
    contam_amount × 0.2  (中和 80%)
    // 代价：皮下震爆形成外伤
    add Wound { location: 命中点, kind: Concussion, severity: 0.3 }
    成功 → 推 narration "极限弹反"
  else:
    判定失败，正常承伤
窗口超时未操作 → 正常承伤
```

### 4.2 替尸/蜕壳流（TiShi）

```
被动触发：命中时若 DefenseLoadout.fake_skin.layers > 0
  layer_durability -= contam_amount + wound_damage × 0.5
  if layer_durability <= 0:
    fake_skin.layers -= 1
    主动切断联系：本次攻击的 contam 全部带走
    add particle: 伪皮飞灰
  else:
    contam_amount × 0.6 进入本体（伪皮吸收 40%）
    wound_damage × 0.7
所有层数耗尽后：DefenseStyle 临时无效，需补伪灵皮
```

### 4.3 绝灵/涡流流（JueLing）

```
玩家主动激活：VortexActivateIntent
  - qi_current >= vortex_cost (默认 15 qi/sec)
  - vortex_active_until = now + 3s
  
攻击命中时检测 vortex_active：
  if active AND attack 是远程 / WeaponCarrier 投射:
    sealed_qi 全部归零（"致命骨刺变成普通朽木棍"）
    contam_amount = 0
    wound_damage 仅保留物理基础（武器本身硬度）
  
反噬：维持过久或时机错误
  if vortex_duration > 3s: 
    add MeridianCrack { severity: 0.4, cause: VortexBackfire } 到手少阴心经
    qi_max_frozen += 10
    永久性手指经络损伤（debuff: 暗器/拳修效率 -30%）
```

### 4.4 流派克制矩阵

| 攻 \ 防 | 截脉 | 替尸 | 涡流 |
|---|---|---|---|
| **爆脉** | 中和 | 蜕壳 | 涡流抓不住贴脸贯穿 → **克制涡流** |
| **暗器** | 反应不及 | **克制蜕壳**（一发穿透） | **被涡流克制**（真元被没收） |
| **阵法** | 已触发不在窗口 | 蜕壳无效（地刺穿底） | 涡流无效（封在方块里） |
| **毒蛊** | 量太小防不住 | 蜕壳无效（量小穿过） | **克制涡流**（为 1 点毒开黑洞极度亏本） |

worldview §五克制三角对齐：毒蛊→涡流→暗器→蜕壳。

---

## 5. 流派攻击实现

### 5.1 爆脉流（Bopǔ）

```rust
// 主动调用超流量上限
ExecuteBopuIntent { qi_invest: f32 (允许 > throughput_max × 1.5) }
  - throughput_current = qi_invest（直接写满）
  - 触发修炼 plan 的 OverloadDetectionTick → 立即施加多条 MeridianCrack
  - 攻击伤害 ×1.5 倍率
  - 战后 qi_max_frozen += qi_invest × 0.3（临时冻结）
  - 严重时 emit CultivationDeathTrigger { cause: BopuBackfire }
```

### 5.2 暗器流（Anqì）

```rust
// 准备阶段（脱战）
ForgeWeaponCarrierIntent { item, qi_to_seal, color_to_seal }
  - 持续 30min（可被打断）
  - 创建 WeaponCarrier component

// 战斗阶段
ThrowCarrierIntent { target }
  - QiDecayInFlightTick 推进
  - 命中时 sealed_qi 注入为 contam_amount
  - 未命中 → 载体落地，可拾回但 sealed_qi 已损 50%
```

### 5.3 阵法流（Zhènfǎ）

```rust
PlaceQiTrapIntent { block_pos, qi_to_seal, trigger_type }
  - 需要花 5min 在原地铺设阵纹
  - 创建 QiTrap component（绑定到方块）
  - expires_at = now + 24h（载体朽坏）

QiTrapTriggerSystem:
  - 监听 BlockSteppedOn / EntityProximity
  - 触发时执行 §3.6 类似的 contam 注入流程到触发者
  - 阵法消耗，移除 component
```

### 5.4 毒蛊流（Dúgǔ）

```rust
// 1 点脏真元微针
ShootDuguIntent { target, toxin_type }
  - qi_invest = 1.0
  - 衰减极少（针太小）
  - 命中后写入特殊 Contamination { amount: 1.0, qi_color: Insidious + toxin_tag }
  - 触发**慢性侵蚀**：修炼 plan 的 ContaminationTick 检测 Insidious 染色 →
    每小时 Cultivation.qi_max -= 1（永久），直到玩家强排
  - 强排代价：花 20 qi 一次性逼出（worldview §五原文）
```

### 5.5 功法 / 技巧系统（Techniques）

worldview §十 提到 **残卷** 是"学习术法（战斗能力核心）"的稀缺资源——本节落地。功法是**学到的主动/被动技巧**，与流派（§5.1-5.4）正交：流派定义"打法风格"，功法定义"具体能做什么动作"。

#### 5.5.1 公理

1. **功法不是天赋是学到的** — 必须从残卷 / 师承 / 顿悟解锁，没人凭空会
2. **功法消耗真元** — 主动功法持续/单次扣 qi，无 qi 自动失效（保持 worldview §四 真元约束）
3. **高阶功法门槛硬绑境界** — 踏空步 = 通灵，飞行 = 化虚，**没有破例**（worldview §三 化虚是天花板）
4. **功法增幅是乘法不是替代** — 在 §1.4 基线上加成，不绕过基线（保持"凡人即弱"公理）
5. **功法可被打断** — 持续型功法在受伤/真元耗尽/经脉断时立即中止

#### 5.5.2 数据模型

```rust
#[derive(Component)]
struct TechniqueBook {
    learned: Vec<Technique>,
    active: Vec<ActiveTechniqueState>,   // 当前生效的持续型
}

struct Technique {
    id: TechniqueId,
    grade: TechniqueGrade,                // 黄/玄/地/天 阶（残卷品级）
    category: TechniqueCategory,
    realm_required: Realm,
    qi_cost: QiCost,                       // 单次 / 持续/秒 / 充能
    learned_at: Instant,
    proficiency: f32,                      // 0-1，使用次数累积，影响效率
}

enum TechniqueCategory {
    AttackBoost,         // 攻击增幅（剑气斩 / 重拳 / 雷火指）
    Movement,            // 身法（疾风步 / 缩地）
    AirStep,             // 踏空步（通灵境解锁）
    Flight,              // 御空飞行（化虚境解锁）
    Defense,             // 主动防御技（金钟罩临时硬抗）
    Perception,          // 灵识 / 神识 (扩展感知半径)
    Utility,             // 辅助（隐匿气息 / 假死）
}

enum TechniqueGrade { Yellow, Mystic, Earth, Heaven }

enum QiCost {
    Instant(f32),                          // 单次释放
    Sustained { per_sec: f32 },            // 持续维持
    Charged { charge_qi: f32, hold_sec: f32 },  // 蓄力释放
}

struct ActiveTechniqueState {
    technique_id: TechniqueId,
    started_at: Instant,
    accumulated_qi_spent: f32,
    interrupt_threshold: f32,              // 受伤超过此值自动中断
}
```

#### 5.5.3 学习路径（与 worldview §十 对齐）

| 来源 | 品阶 | 备注 |
|---|---|---|
| 击杀道伥掉残卷 | Yellow / Mystic | 主流来源 |
| 遗迹探索 | Mystic / Earth | 配合解谜/陷阱 |
| 顿悟解锁（cultivation §5 F 类） | 任意 | 非常稀有 |
| 师承传授（玩家间） | 取决于师傅 | 需达成信任，可能传错或带毒 |
| 垂死大能交易（worldview §七） | Earth / Heaven | 高风险陷阱 |

学习残卷流程：
```
LearnTechniqueIntent { scroll_item, technique_id }
  - 检查 realm >= scroll.realm_required
  - 检查 meridian 路径契合（剑诀需要手三阴某条已通）
  - 静坐参悟 30min ~ 6h（按品阶）
  - 完成后 TechniqueBook.learned.push(...)
  - 写 LifeRecord.biography
  - 残卷消耗
失败可能：经脉不契合 → 走火，添加 MeridianCrack
```

#### 5.5.4 各类别效果（接入 §3 攻击事务 / §1.4 基线）

##### A. AttackBoost — 攻击增幅

接入 §3.1 Step 4，命中前对 wound_damage / contam_amount 应用乘子：

```
on AttackIntent { technique: Some(t) if t.category == AttackBoost }:
  qi_total = qi_invest + t.qi_cost.instant
  damage_multiplier = 1.0 + t.grade_bonus × (0.5 + 0.5 × t.proficiency)
  // 黄阶熟练满 = ×1.4；天阶熟练满 = ×3.0
  
  // 染色契合再加成（worldview §六 染色谱）
  if t.aligned_color matches QiColor.main:
    damage_multiplier *= 1.2
```

例：「裂石拳」（黄阶 AttackBoost，pure，沉重色匹配）：
- 凡人引气境 + 黄阶熟练 0% → ×1.05 = 拳基础 3 → 3.2 dmg
- 凝脉 + 沉重色 main + 熟练 80% → 1.4 × 1.2 = ×1.68 → 拳基础 6 → 10 dmg
- **远低于"一拳一座山"——成长是慢慢叠的**

##### B. Movement — 身法

接入 §1.4 移动速度 / Stamina。**功法用真元替代部分体力**——这就是修士比凡人能跑更久的原因：

```
on ActivateMovementTechnique { id }:
  while active:
    speed = base_speed_for(current_move_state) × speed_multiplier
    qi_drain per_sec
    stamina_drain = base_stamina_cost × stamina_relief_factor
  
  base_stamina_cost (来自 §1.4 / §2.1 StaminaTick)：
    walk:    0    /s      ← 几乎不扣，功法基本没意义
    jog:    -2    /s      ← 微扣，配合身法可长时奔袭
    sprint: -10   /s      ← 大扣，功法主战场
  
  典型：
    「疾风步」（黄阶）：×1.3 速度 / 2 qi/s / stamina_relief 0.7
        → 冲刺时实际扣 -7/s（仍累，但比纯凡人 -10/s 持久 40%）
    「缩地」（地阶）：×2.0 速度 / 8 qi/s / stamina_relief 0.3 + 短瞬移
        → 冲刺时实际扣 -3/s（接近不掉力，但 8 qi/s 烧真元）
    「踏雪无痕」（玄阶）：×1.0 速度 / 1 qi/s / stamina_relief 0.0
        → 不省时间但完全不掉 stamina + 不留脚印（worldview §二 残灰方块脚印反追踪）
```

设计意图：
- **走路功法基本无价值**——凡人也不累，没人会浪费真元
- **跑步功法是性价比之王**——长时奔袭跨地图必备
- **冲刺功法是战斗核心**——决定追击/逃命是否能维持

化虚境前**所有身法速度乘子上限 ×2.0**（worldview §一二 化虚冲刺 15 m/s 即此处天花板）。

##### C. AirStep — 踏空步（**通灵境解锁**）

worldview §三 通灵 = "奇经初启，与天地共鸣" → 第一次能短暂借力虚空。

```
realm_required: Tōnglíng
qi_cost: Sustained { per_sec: 5 } + 每次踏空额外 3 qi
stamina_cost: 每次踏空 -5（瞬时类似 attack，腿部用力）
              维持期间 +Combat 状态消耗（-5/s，因为持续蹬虚空）

机制：
  - 不是真飞行，是"在空中可以再起跳一次"
  - 每次空中 jump 消耗 3 qi + 5 stamina
  - 持续维持期间 fall_damage 减半
  - 单次空中 step 高度 ≤ 1.5 格，水平 ≤ 3 格
  - stamina 耗尽 → 仅 qi 不足以维持，强制下落
  - 实现：server 在玩家空中按 jump 键时下发 ClientboundPlayerAbilities + Velocity 包

典型功法：
  「踏云步」（玄阶）：空中可 step 1 次
  「九霄步」（地阶）：空中可 step 3 次 + step 距离 ×1.5

战术后果：
  - stamina 100 = 最多空中 step 20 次（不算战斗消耗）
  - 通灵 + 体修 sprint 战场切入：sprint 烧 stamina → 起跳 step → 落地继续 → stamina 见底
    → "高速空战只能维持十几秒"
```

##### D. Flight — 飞行（**化虚境解锁，唯一**）

worldview §三 化虚 = "半步超脱"，全服 1-2 人。飞行是真正的天花板能力。

```
realm_required: Huàxū
qi_cost: Sustained { per_sec: 20 }   // 极昂贵
stamina_cost: 0（悬空不靠肉身蹬力，纯真元托举）
              ⚠ 飞行**不消耗 stamina** — 唯一与体力解耦的移动方式
              ⚠ 但飞行后落地若立即 sprint，stamina 仍是上次未恢复的值
max_speed: 8 m/s 平飞 / 4 m/s 上升
fuel_burn: 化虚境 qi_max ~ 500，纯飞 ~25 秒就空

机制：
  - server 按 abilities packet 下发 fly = true，禁用 fall_damage
  - 每 tick 检查 qi_current > 0，否则强制下落（自由落体）
  - 飞行中不可施法（真元全用于维持悬浮，worldview §四 远程仙法本就败家）
  - 飞行中战斗 qi_drain ×3（边战边飞 ~8 秒空池）
  - 飞行不可在负灵域使用（灵压差直接抽干）

典型功法：
  「御风诀」（地阶）：基础飞行
  「凌虚」（天阶）：飞行速度 ×1.5 + 可悬停免维持成本（每 5 秒自动微动）

战术意义：
  - 化虚之间的战斗是空中战，但极难维持（双方都在烧 qi）
  - 化虚 vs 通灵：通灵踏空步只能短暂腾跃，化虚可俯冲压制
  - 化虚 vs 化虚：先 qi 空者下落 = 输
```

##### E. Defense / Perception / Utility

简述（实施时再展开）：
- **Defense**：「金钟罩」临时承伤上限 +50% / 4s（10 qi/s 维持），被打穿后内伤
- **Perception**：「神识」感知半径临时 ×3 / 5 qi/s，可识破隐匿
- **Utility**：「敛息」隐匿气息让其他玩家无法 inspect 你的境界 / 染色 / 真元状态

#### 5.5.5 与流派的协同

| 流派 | 推荐功法类别 |
|---|---|
| 爆脉流 | AttackBoost (重拳类) + Defense (硬抗反噬) |
| 暗器流 | AttackBoost (远程飞剑) + Movement |
| 阵法流 | Utility (隐匿) + Defense |
| 毒蛊流 | Utility (敛息) + Perception |
| 通用 | Movement / AirStep（境界到了都想要） |

化虚境玩家至少有 1 个 Flight 功法（不会飞的化虚被嘲笑）。

#### 5.5.6 客户端 UI

- inspect 新加 "已学功法" 列表（品阶色 + 熟练度环）
- 快捷栏支持绑定主动功法（数字键释放 / 长按维持）
- 飞行/踏空 HUD：剩余 qi 倒计时 + "强制下落预警"

#### 5.5.7 实施阶段

- **C2**：AttackBoost 黄阶单功法（验证伤害乘子接入 §3.1）
- **C5**：Movement / Defense / Perception / Utility 全类（配合四攻三防）
- **C6**：AirStep（通灵境配合天劫流程）
- **C7**（新增）：Flight（化虚境，配合渡劫成功后的能力授予）

---

### 5.6 伤口疗愈与急救

worldview §三 真元规则要求"打赢了但满身伤可能输了生存"——本节落地伤口怎么愈合 / 怎么救命 / 严重伤怎么留疤。

#### 5.6.1 公理

1. **没有自然回血** — 原版 MC 的"吃饱饭自动回血"在此被禁用；伤口必须主动处理
2. **疗愈三层** — 自疗（最慢/免费）→ 草药/丹药（中速/资源）→ 外科救援（最快/需人）
3. **不处理会恶化** — 伤口被忽略 → bleeding 持续 + 感染加重 + 可能化脓变永久
4. **严重伤留疤** — severity > 0.7 且未在 1h 内处理 → Scar，永久 debuff（呼应 worldview "断肺经的飞剑手就废了"）

#### 5.6.2 数据模型扩展

扩展 §1.1 `Wound`：

```rust
struct Wound {
    location: BodyPart,
    kind: WoundKind,
    severity: f32,                   // 0-1
    bleeding_per_sec: f32,
    created_at: Instant,
    inflicted_by: Option<Entity>,
    
    // 新增疗愈相关字段
    healing_state: HealingState,
    treatment_log: Vec<Treatment>,   // 用过哪些处理（防止刷草药）
    infection: f32,                  // 0-1，感染度
    last_treated_at: Option<Instant>,
}

enum HealingState {
    Bleeding,                        // 默认，持续掉 health
    Stanched,                        // 已止血（草药/包扎），不掉血但仍未愈
    Healing { rate_per_sec: f32 },   // 主动愈合中
    Scarred,                         // 永久疤痕，留 debuff
}

struct Treatment {
    kind: TreatmentKind,
    applied_at: Instant,
    applied_by: Option<Entity>,
    cooldown_until: Instant,         // 同种处理冷却（防刷）
}

enum TreatmentKind {
    SelfSeated,                      // 自疗静坐（人人可用，最慢）
    Herb(HealingItem),               // 草药/丹药
    Surgery,                         // 外科救援（他人 TreatPlayerIntent）
    QiHealing(TechniqueId),          // 真元疗愈：必须学到对应功法（见 §5.5）
                                     //   归 TechniqueCategory::Defense 子类
                                     //   平和色/医道修士专属功法可治他人
                                     //   消耗大量真元，但速度远超自疗
    ScarRemoval(TechniqueId | ItemId), // 疤痕消除（高阶丹药/天阶功法）
}

#[derive(Component)]
struct Scars {
    entries: Vec<Scar>,              // 永久疤痕集合，影响 inspect 显示
}

struct Scar {
    location: BodyPart,
    debuff: ScarDebuff,
    origin_severity: f32,
    inflicted_at: Instant,
}

enum ScarDebuff {
    HeadConcussion,        // 心境 -10% / 神识精度下降
    ChestInternal,         // qi_max -10
    AbdomenStomach,        // stamina_recover ×0.7
    ArmCrippled(Side),     // 该侧持物施法效率 -25%
    LegLimp(Side),         // 移动速度上限 -10%
}
```

#### 5.6.3 三层疗愈机制

##### A. 自疗（静坐 + 真元）

```
玩家进入 SeatedHealing state（不可移动）：
  for each Wound where state in {Bleeding, Stanched, Healing}:
    qi_drain_per_sec = 0.5 × wound.severity
    heal_per_sec     = 0.02 × Cultivation.composure × meridian_avg_integrity
    wound.severity  -= heal_per_sec × dt
    Cultivation.qi_current -= qi_drain_per_sec × dt
    
  if wound.severity <= 0.05:
    remove Wound entry
    health 恢复对应量（不是瞬时，是渐进）
  
基线（凡人引气，severity=0.5 切割伤）：
  自疗时间约 25 min，消耗 ~6 qi
温润色 / 平和色 / 医道修士：heal_per_sec ×2
受伤期间不打坐：bleeding 持续，不会自然好
```

##### B. 草药 / 丹药

```rust
enum HealingItem {
    StaunchHerb,         // 止血草：Bleeding → Stanched，瞬时；冷却 5 min
    BoneSetGrass,        // 接骨草：Pierce/Blunt severity -0.2，冷却 30 min
    BurnSalve,           // 灼伤膏：Burn 专属，severity -0.3，冷却 20 min
    JieDuDan,            // 解毒丹：清除毒蛊 contam（非伤口本身）
    HuiYuanDan,          // 回元丹：health +20 + qi +10，冷却 1 h
    JinChuangSan,        // 金疮散：所有伤口 severity -0.15，冷却 15 min
    RenMaiDan,           // 任脉丹：经脉 crack 修复 +20%（稀有）
}

UseHealingItemIntent { item, target_wound: Option<WoundId> }
  - 检查 cooldown
  - 检查 inventory 持有
  - 应用效果，写 wound.treatment_log
  - 严重伤（severity > 0.6）需配合包扎动作（5s 不可移动）
```

**反刷设计**：同种处理 cooldown 共享同一伤口；治了 5 个伤要等 5 个独立 cooldown。强制玩家**优先处理重伤**，不能糊上一堆草药一秒满血。

##### C. 外科救援（他人 / NPC）

```
TreatPlayerIntent { healer, patient, technique }
  - 双方距离 <= 1.5 格 + 患者同意（自动同意 if NearDeath）
  - 持续 10-30s 不可移动
  - 健康者可救濒死者：
      health_current 提至 30%
      移除 NearDeath 状态
      所有 Bleeding → Stanched
  - 平和色 / 医道修士效率 ×2，可治愈他人 contam（worldview §六）
  - 救人者获得 Karma -1（积德），写 LifeRecord.biography
  - 失败可能：技术不够 → 患者多失血 / Karma + 1
  
NPC 救援：拾荒散修中医者收骨币（贵）；炼丹疯子免费但用毒药（worldview §九）
```

#### 5.6.4 恶化机制

不处理的伤口会变糟：

```
WoundDeteriorationTick (0.1Hz):
  for each Wound state == Bleeding:
    age = now - created_at
    if age > 10 min AND last_treated_at.is_none():
      infection += 0.01 × dt
      severity += 0.005 × dt   // 化脓加重
    if infection >= 1.0:
      severity = 1.0
      health_current -= 5 × dt  // 高烧加速死亡
      可能 emit DeathEvent { cause: Infection }
    if age > 1h AND severity > 0.7:
      → 转 HealingState::Scarred
      → 创建对应 Scar entry，长期 debuff
      → bleeding 停止（伤口"长死了"，永远歪了）
      → 写生平卷 BiographyEntry::Scarred
```

**新人战术后果**：野狼咬一口 severity 0.4 → 不处理 1 小时变 0.7 → 再不处理变 Scarred → 长期 debuff 跟着走。**逼玩家随身带止血草**。

**Scar 可逆**（worldview §三 "灵草修补经脉" 的延伸）：

```
ScarRemoval 来源：
  - 「化痕丹」（地阶）：消单个 Scar，6h 静坐疗愈，需稀有材料
  - 「洗髓丹」（天阶）：消所有 Scar，需化虚境监督（worldview §九 大能交易）
  - 「枯木回春诀」（天阶 Defense 功法）：医道修士可为他人除疤，消耗大量真元
  - 自然不会消失——必须主动求医或苦修

设计意图：
  - Scar 不是一辈子枷锁，但消除成本极高（鼓励玩家先苟住活下来）
  - 高阶玩家可以"洗白"过去的伤；低阶只能带着伤继续
  - 给医道修士（worldview §六 平和色）一个高价值生态位
```

#### 5.6.5 health 与 wound 的恢复关系

```
health_current 不会因为 wound 治好就瞬时回满。
wound 移除时：health_recover_pool += wound.original_severity × 20

每秒推 health_current += 1（来自 pool），需要主动静坐；
站立时仅 +0.2/s（worldview §四"打赢但耗尽真元"语境）。

凡人基线 30 health 完全治愈一场 0.5 wound 战斗：
  ~25 min 自疗清 wound
  + ~10 min 静坐回 health
  = ~35 min "战后修养"
```

战斗后**至少半小时不能再打**——这就是 worldview "搜打撤" 循环里"撤"的物理基础。

##### 食物 / 睡眠加成

保留 MC 原版"吃饭" 和"睡觉"的入口，但**只作为加成**，不做主回血源：

```
状态：WellFed (吃饱) — 持续 30 min，需进食触发
  → health_recover_pool drain rate ×1.1
  → stamina recover_per_sec ×1.1

状态：Rested (睡足) — 床上睡完一觉触发，持续 1 h
  → health_recover_pool drain rate ×1.1
  → composure 恢复速率 ×1.2

两者可叠加 = ×1.21（基本 17% 加速），不足以颠覆基线
食物种类、饱腹时长、来源（狩猎/采集/炼丹/炊事）由独立**食物/生存 plan**
定义（TODO: `plan-food-v1.md`，待定）。本 plan 只约定：
- 消费方：§5.6 的 `health_recover_pool drain rate` 和 `stamina.recover_per_sec`
- 进入途径：食物 plan 发出 `ApplyStatusEffectIntent { id=WellFed | Rested, ... }`，通过 §7 StatusEffect 统一容器应用
- 在 food plan 落地前，调试阶段用命令 `/effect give WellFed` 手动模拟
```

worldview 没明说"灵气世界还要不要吃饭"——保留是为了让玩家有"营地建设、生火做饭、回家睡觉"的生活感（搜打撤循环里的"撤回基地修整"），但不能让饱腹睡眠替代真元疗愈。

#### 5.6.6 经脉裂痕的疗愈（cross-plan）

`MeridianCrack` 由修炼 plan 拥有（cultivation §1.1），其 `MeridianHealTick`（cultivation §2 T5）独立运转。本 plan 仅提供加速手段：

| 来源 | 加速效果 |
|---|---|
| 任脉丹 | 当前 crack 愈合进度 +20% |
| 医道修士外科疗愈 | crack heal_rate ×2，持续 30 min |
| 馈赠区 spirit_qi > 0.7 静坐 | 修炼 plan 自带 ×2，本 plan 不重复 |

#### 5.6.7 与 NearDeath 自救的衔接

§2.1 NearDeathTick 的 30s 自救窗口具体动作：

```
NearDeath 状态下玩家可执行：
  - SelfFirstAid: 自己使用回元丹 / 金疮散（如果还能动且持有）
      → 退出 NearDeath 阈值：health 回到 5% 以上
  - 等待救援：完全不动等 healer 来 TreatPlayerIntent
  - 主动放弃：跳过 30s 立即 DeathEvent（玩家可选）

NearDeath 期间：
  - 视野变红/模糊（client 渲染）
  - 不可施法 / 不可攻击 / 移动速度 ×0.3
  - 真元自然消耗（hold-on cost: 0.5 qi/s）
```

#### 5.6.8 与 inspect UI 衔接

inspect 伤口层（已有骨架）显示：
- 各部位 wound 的 severity（圆圈大小）+ kind 颜色
- HealingState 图标（红=Bleeding / 黄=Stanched / 绿=Healing / 黑=Scarred）
- 感染度进度环（从 0 转到 1 时高亮警告）
- Scar 永久标记（持续显示，提醒玩家此处易复伤）

#### 5.6.9 实施阶段

- **C1**：基础 Wound 数据 + 自疗静坐（已规划）
- **C2**：HealingItem 三种基础药（止血草/金疮散/回元丹）
- **C3**：外科救援 TreatPlayerIntent + 平和色加成
- **C4**：恶化机制 + Scar 系统 + 感染致死
- **C5**：医道功法（B 类 Defense / Perception / Utility 内的疗愈类）

---

## 6. 武器与法宝系统

worldview §五 / §九 / §十 散见武器线索（暗器载体、骨币原料、残篇法宝、游商傀儡）。本节统一落地：**普通武器**（拳/刀/剑/矛/杖）+ **法宝**（魂契类高阶器物）。

### 6.1 公理

1. **武器是放大器，不是来源** — 没武器也能打（拳），有武器是为了 reach / 伤害 / 真元导流
2. **材质决定真元亲和** — 凡铁锁不住真元，异变兽骨/灵木才能（呼应 §1.1 `MaterialGrade`、worldview §五）
3. **法宝必须魂契** — 高阶器物认主，他人拿到只是块石头；契主死则法宝休眠
4. **耐久会损** — 凡器战中易碎；法宝靠真元养，缺养则失光
5. **没有橙装爆炸** — 凡人捡到法宝也用不了；法宝威能与持有者境界绑定

### 6.2 武器类型表（接入 §3.3 reach）

```
WeaponKind         | reach档位          | base_damage | qi_conduit | durability_max | 主修染色
-------------------|--------------------|-------------|------------|----------------|--------
Fist (徒手)        | FIST_REACH 0.9/1.3 | 2-3         | 0.0        | ∞              | Heavy
Dagger (匕首)      | DAGGER 1.2/1.6     | 4-6         | 0.3        | 200            | Sharp / Insidious
Sword (长剑)       | SWORD 2.0/2.5      | 6-9         | 0.5        | 350            | Sharp
Saber (刀)         | SWORD 2.0/2.5      | 7-10        | 0.4        | 400            | Heavy / Sharp
Spear (长矛)       | SPEAR 2.6/3.0      | 8-12        | 0.4        | 500            | Heavy
Staff (法杖)       | STAFF 2.4/2.8      | 3-5         | 0.8        | 250            | Intricate / Mellow
Bow (弓)           | 远程 0-30 格       | 4-7         | 0.6        | 300            | Light / Sharp
Throwing (暗器载体) | 远程 0-50 格       | 1-3 + sealed| 0.2        | 1 (一次性)     | Solid
Catalyst (法器)    | 0 (持物即可)       | 0           | 1.0        | ∞              | 任意
```

字段说明：
- **`base_damage`** —— 武器自身物理伤害（凡人就靠这个），与 qi_invest 无关
- **`qi_conduit`** —— 真元导流系数；`hit_qi = qi_invest × qi_conduit`，意思是真元有多少能"走武器出去"
  - 拳 0.0 = 真元只能从皮肉迸出（worldview §四 体修零距离灌入）
  - Catalyst 1.0 = 法器是纯导体，自身不参与伤害，专为施法服务
- **`durability_max`** —— 凡器耐久基线，每次出手 -1，被防御挡下 -3，承受过载 -10

### 6.3 数据模型

```rust
#[derive(Component)]
struct Weapon {
    kind: WeaponKind,
    material: MaterialGrade,              // 已在 §1.1 定义（Mundane/Beast/Spirit/Relic）
    quality: WeaponQuality,               // 制作精度
    base_damage: f32,                     // 派生自 kind + material + quality
    qi_conduit: f32,                      // 派生自 kind + material
    durability_current: f32,
    durability_max: f32,
    enchant_slot: Option<EnchantInscription>, // 单插槽刻印
    forged_by: Option<Uuid>,              // 铸造者（炼器师签名）
}

enum WeaponQuality { Crude, Standard, Fine, Masterwork }

struct EnchantInscription {
    pattern: InscriptionId,               // 锋利/破甲/导引/轻盈...
    magnitude: f32,                       // 0-1
    sealed_qi: f32,                       // 刻印自身储存少量真元（缓慢挥发）
}

// 法宝 = 武器的特殊形态
#[derive(Component)]
struct Treasure {
    base_weapon: Weapon,                  // 法宝也是一种武器
    grade: TreasureGrade,                 // 黄/玄/地/天 阶（同功法品阶）
    soul_bond: Option<SoulBond>,
    qi_pool: f32,                         // 法宝自身真元池（魂契后由主人养）
    qi_pool_max: f32,
    abilities: Vec<TreasureAbility>,      // 主动技能（类似 Technique 但绑器物）
    dormant: bool,                        // 主人死则休眠（worldview §九）
    history: Vec<PrevOwner>,              // 历代契主（最多 5 条）
}

enum TreasureGrade { Yellow, Mystic, Earth, Heaven }

struct SoulBond {
    owner: Uuid,                           // 角色 ID（不是 entity，跨重生保留）
    bonded_at: Instant,
    bond_strength: f32,                    // 0-1，养护时长累积
    realm_required: Realm,                 // 至少要这个境界才能契
}

struct TreasureAbility {
    id: AbilityId,
    qi_cost: QiCost,                       // 同 Technique
    realm_required: Realm,                  // 持有者境界至少 X 才能用
    cooldown: Duration,
    last_used_at: Option<Instant>,
}

struct PrevOwner {
    character_id: Uuid,
    bond_period: (Instant, Instant),
    severed_reason: BondSeverReason,
}

enum BondSeverReason { OwnerTerminated, ForciblyBroken, Voluntary }
```

### 6.4 材质分级系数（接入 §1.1 MaterialGrade）

```
material      | base_damage ×  | qi_conduit ×  | durability ×  | qi_retention*
--------------|----------------|---------------|---------------|-------------
Mundane (凡铁/木石) | 1.0       | 1.0           | 1.0           | 0.25 (50格保留 25%)
Beast (异变兽骨)    | 1.3       | 1.5           | 1.5           | 0.80
Spirit (灵木)       | 1.2       | 1.7           | 1.3           | 0.85
Relic (上古遗物)    | 2.0       | 2.5           | 5.0           | 0.95
```

* qi_retention 用于 §3.6 法术 / 暗器投射衰减——已在 §3.2 决策表里给的"飞 50 格 75% / 80% / 95%"对齐。

### 6.5 真元导流与 qi 投射（接入 §3.1 Step 2）

```
on AttackIntent { weapon, qi_invest }:
  effective_qi = qi_invest × weapon.qi_conduit
  // 拳 conduit=0 → effective_qi = 0，但仍可贴脸用 qi_invest 直接灌入身体
  //   （worldview §四 体修打法：拳头是触媒，真元从皮肉迸出，不走武器）
  //   实现：拳类攻击 effective_qi = qi_invest（绕过 conduit），但 reach=0.9 强制贴脸
  // Catalyst conduit=1.0 → 全部走武器，自身 base_damage=0
  //   适合阵法师 / 法术专修，远距施法

durability:
  weapon.durability_current -= 1 (普通命中)
                              -3 (被绝灵涡流没收 / 被防具硬挡)
                              -10 (爆脉流过载使用)
  if durability_current <= 0:
    武器损坏，从持有栏中移除（碎片化为材料）
    法宝不会损坏（耐久来自 qi_pool 养护）
```

### 6.6 法宝魂契机制

```
SoulBondIntent { treasure, character_id }
  - treasure.soul_bond.is_none() OR treasure.dormant == true
  - character.realm >= treasure.bond.realm_required
  - 静坐 30 min ~ 6 h（按 grade）
  - 期间不可被打扰（受伤 → 失败 + 法宝伤害反噬）
  - 完成后 SoulBond 写入，dormant = false
  
养护：
  while bonded:
    持有者每天为法宝注入真元（玩家手动 InfuseTreasureIntent）
    treasure.qi_pool += infused
    treasure.bond_strength += 0.001 / day
  缺养 (qi_pool == 0 持续 24h) → bond_strength -= 0.05
  bond_strength == 0 → bond severed，法宝 dormant
  
持有者死亡：
  if 重生（运数/劫数）:
    treasure.dormant = true (临时休眠)
    重生后可重新激活（不需要再次魂契）
  if 终结（Terminated）:
    treasure.dormant = true，soul_bond.severed_reason = OwnerTerminated
    法宝掉落到死亡点，等待下一位有缘人魂契
    PrevOwner 写入 history（worldview §十二 "亡者博物馆"可查到法宝传承链）

强行夺取：
  非持有者拿到 dormant 法宝可以触发 RebondIntent
  非持有者拿到激活态法宝（持有者还活着）：
    treasure 视为普通武器使用 base_damage（无 ability、无 qi_pool）
    持续持有 7 天 + 持有者已超过 7 天未碰过 → 触发 ForciblyBroken
    （worldview §九 游商傀儡的"主人收到坐标"机制可作为反制 hook）
```

### 6.7 法宝 Ability vs 功法 Technique 的边界

| 维度 | Technique（功法） | TreasureAbility（法宝技能） |
|---|---|---|
| 学习方式 | 残卷 / 师承 / 顿悟，永久 | 内置于法宝，跟物走 |
| 跨角色 | 学到了就是你的（终结时随生平卷归档） | 法宝传承，新契主可继承 |
| 真元来源 | 持有者 qi_current | 法宝 qi_pool（持有者养护） |
| 升级路径 | proficiency 累积 | bond_strength 累积 |
| 失去方式 | 不会失去 | 法宝丢了/被夺/契破 |

设计意图：法宝是"**外挂能力**"，比功法更强但脆弱——丢了就没了。鼓励玩家"**苟住法宝活到化虚**"而不是"**炫耀法宝被人盯死**"（worldview §十一 化虚渡劫被截胡的设计同源）。

### 6.8 制作与修复

```
ForgeWeaponIntent { kind, materials, quality_target }
  - 需要在炼器炉处（worldview §九 暗示稀缺设施）
  - 持续 1-12 h 按 grade
  - 成功率：炼器师染色契合（凝实色 main）+ 材料品质
  - 失败：材料半返还，无成品
  - 炼器师签名 forged_by 写入

ForgeTreasureIntent { ... }
  - 法宝铸造仅 Solidify+ 凝实色 main 玩家可尝试
  - 需要稀有材料（异变兽核 / 灵眼结晶 / 道伥残卷）
  - 持续数天（real time），期间炼器师不可远离
  - 成功率极低（~10%），大多为 Yellow/Mystic 阶
  - Earth/Heaven 阶法宝**无法新造**——只能从遗迹/终结角色继承

RepairWeaponIntent { weapon, materials }
  - 凡器：补充原材料即可
  - 法宝：注入真元养护，不走"修"流程
```

### 6.9 与功法的协同（cross §5.5）

| 功法 | 武器要求 |
|---|---|
| 「裂石拳」（AttackBoost / Heavy） | 必须徒手或拳套（dagger/sword 持物时无效） |
| 「飞剑诀」（AttackBoost / Sharp） | 必须 sword + sharp 染色 |
| 「御剑诀」（Movement + AttackBoost / Light） | sword 必须先魂契（按法宝逻辑）才能御出去 |
| 「九霄步」（AirStep） | 无武器要求，但持长矛会降效（重） |
| 「踏雪无痕」（Movement / Utility） | 无要求 |

实现：`Technique.weapon_requirement: Option<WeaponKindMask>`，AttackBoost 类自动在 §3.1 Step 4 校验。

### 6.10 客户端 UI

- inspect 新加 "持有武器" 槽：显示 kind、material、quality、durability 进度条
- 法宝特殊 UI：展开后看 grade、bond_strength、qi_pool、abilities 列表、prev_owners
- 修复/养护界面：拖材料/丹药 → 进度条
- 炼器面板：选 kind + 选材料 + 显示成功率预测

### 6.11 实施阶段

- **C2**：`Weapon` component + 凡器三种（dagger / sword / spear） + base_damage 接入 §3.1 Step 4
- **C3**：耐久损耗 + 武器损坏移除
- **C4**：材质分级 + qi_conduit 实装 + 异变兽骨/灵木掉落
- **C5**：`Treasure` component + 魂契机制 + dormant/重激活
- **C6**：炼器制作 + 修复流程 + 炼器师签名
- **C7**：法宝 Ability 系统 + history 传承 + 强行夺取
- 道统遗物（§8.4 Solidify+ 终结掉落）从 C5 起接入"掉落 dormant 法宝"

---

## 7. 状态效果库（StatusEffect）

跨流派、武器、功法、天劫共用的短/中效 buff/debuff 统一容器。参考 Pumpkin-MC 的 `LivingEntity::active_effects` 设计（`pumpkin/src/entity/living.rs`），保留其结构优点，按本项目需求做 5 处关键偏离。

### 7.1 设计公理

1. **统一容器** — 所有流派、功法、武器、天劫施加的短/中效走同一个 Component，避免 ad-hoc 字段污染
2. **静态 spec + 运行时 instance 分离** — 借鉴 Pumpkin `StatusEffect`（静态注册表）vs `Effect`（实例）
3. **fn pointer 分派，不硬编码 match** — 新增 effect kind 不改 tick 主循环
4. **属性加成走聚合，不覆写 base stat** — 每 tick 从 active effects 聚合出最终属性
5. **可驱散可抗性** — `DispelTag` + `EffectTags` bitflags，支持按标签精准驱散
6. **客户端可见** — inspect 状态栏 + HUD 图标；**不走原版 `CUpdateMobEffect` packet**（我们的效果空间远超 vanilla 36 槽），走 Redis + CustomPayload

### 7.2 参考与偏离：Pumpkin 对照

| 维度 | Pumpkin 做法 | 本 plan 做法 |
|---|---|---|
| 存储 | `Mutex<HashMap<StatusEffect, Effect>>` on `LivingEntity` | `HashMap<EffectId, ActiveEffect>` 作为 Bevy Component（无 Mutex，Bevy 调度保证） |
| 静态注册 | codegen 自 `assets/effect.json` | 手写 `EFFECT_REGISTRY`（非原版效果，参考 `LAYER_REGISTRY` 套路） |
| 周期 tick 分派 | `apply_effect_tick` 硬编码 match POISON/REGENERATION/WITHER/HUNGER | `EffectSpec.on_tick: fn pointer`，分派表驱动 |
| 强度 | `amplifier: u8` 档位 | `magnitude: f32` 连续值（衰减/染色插值更自然） |
| 叠加 | 新强度 ≥ 旧则覆盖，否则忽略 | 按 kind 分策略表（stacks 独立 / max / 续时上限 / 单例） |
| 溯源 | 无 | `source: EffectSource` + `source_color: QiColor` 必填 |
| 驱散 | 牛奶清全部 / 特定 | `DispelTag{Easy/Hard/Ironclad}` + `tags` bitflags |
| 属性修饰 | add_effect 时 apply + remove_effect 时反转 | tick 内全量聚合，不需要反转（更幂等） |
| 客户端同步 | `CUpdateMobEffect` / `CRemoveMobEffect` packet | Redis `CHANNEL_STATUS_EFFECT` + Fabric CustomPayload + owo-lib 自渲染 |
| 持久化 | NBT read/write on entity | 跨重生 Ironclad 效果随 LifeRecord JSON 归档 |
| duration=-1 无限 | ✅ 抄 | ✅ 用于 NearDeath / TribulationLocked / SoulMarked |

### 7.3 数据模型

```rust
// ========== 标识 ==========
// EffectKind 是静态枚举（在 registry 里 const），EffectId 是运行时复合 key
//（基础 kind + 可选 source tag，供 PerSource policy 区分同 kind 不同来源）
pub enum EffectKind {
    Bleeding, Burning, Poisoned, QiCorrosion, ContaminationOverflow,
    Stunned, Rooted, Silenced, Disarmed, Slowed, Hastened,
    Invisible, Revealed, Blinded, AuraMasked,
    DamageAmp, DamageReduction, QiThroughputBoost, QiRegenBoost, DefenseWindowExt,
    Phasing, SoulMarked, TribulationLocked, NearDeath, QiBonded,
}

#[derive(Hash, Eq, PartialEq, Clone)]
pub struct EffectId {
    pub kind: EffectKind,
    pub source_tag: Option<Uuid>,   // Some 仅用于 PerSource policy；其他 policy 为 None
}

impl EffectId {
    pub fn single(kind: EffectKind) -> Self { Self { kind, source_tag: None } }
    pub fn per_source(kind: EffectKind, src: EffectSource) -> Self {
        let tag = match src {
            EffectSource::Entity(uuid) => Some(uuid),
            _ => None,   // 非实体来源退化为 single
        };
        Self { kind, source_tag: tag }
    }
}

// ========== 静态注册表（编译期常量） ==========
pub struct EffectSpec {
    pub kind: EffectKind,
    pub default_dispel: DispelTag,
    pub tags: EffectTags,                 // bitflags
    pub tick_interval: Option<Duration>,  // Some(dt) → 周期性
    pub stack_policy: StackPolicy,
    pub attribute_modifiers: &'static [AttrModSpec],
    pub on_apply:  Option<fn(&mut World, Entity, &ActiveEffect)>,
    pub on_tick:   Option<fn(&mut World, Entity, &ActiveEffect)>,
    pub on_remove: Option<fn(&mut World, Entity, &ActiveEffect)>,
}

// 全局注册表（phf::Map 或 once_cell::Lazy<HashMap>）
pub static EFFECT_REGISTRY: Lazy<HashMap<EffectKind, &'static EffectSpec>> = ...;

// ========== 运行时实例 ==========
#[derive(Component, Default)]
pub struct StatusEffects {
    pub active: HashMap<EffectId, ActiveEffect>,
}

pub struct ActiveEffect {
    pub spec: &'static EffectSpec,
    pub magnitude: f32,                // 连续值
    pub stacks: u8,
    pub duration: Duration,            // Duration::MAX 表示无限（Pumpkin 的 -1 等价物）
    pub next_tick_at: Option<Instant>,
    pub source: EffectSource,
    pub source_color: Option<QiColor>,
    pub applied_at: Instant,
}

pub enum EffectSource { Self_, Entity(Uuid), Environment(ZoneId), Tribulation }

pub enum StackPolicy {
    Refresh,           // 取 max(magnitude)，续 duration
    StackIndependent,  // 每次 Apply 叠一层，独立结算（DoT 类），上限 cap
    ExtendDuration { cap: Duration },  // 续时 capped（Stun/Silence 防无限控制）
    SingletonIgnore,   // 已有则忽略（NearDeath/TribulationLocked）
    PerSource,         // 同 source 续时，不同 source 独立（SoulMarked）
}

pub struct AttrModSpec {
    pub target: AttributeKind,         // Speed/AttackDamage/QiRegen/DefenseWindow/...
    pub op: AttrOp,                    // Add / MulBase / MulTotal
    pub value_per_magnitude: f32,      // 最终 = value × magnitude × stacks
}

bitflags! {
    pub struct EffectTags: u32 {
        const PHYSICAL  = 1 << 0;
        const QI        = 1 << 1;
        const MENTAL    = 1 << 2;
        const MOVEMENT  = 1 << 3;
        const PERCEPTION= 1 << 4;
        const DOT       = 1 << 5;
        const CONTROL   = 1 << 6;
        const BUFF      = 1 << 7;
        const DEBUFF    = 1 << 8;
    }
}

pub enum DispelTag { Easy, Hard, Ironclad }
```

### 7.4 EffectKind 清单（初版）

分五类，每个对应 `EffectSpec` 常量：

**A. 持续伤害（DoT，tag = QI|DOT 或 PHYSICAL|DOT，StackIndependent）**
- `Bleeding{rate}` — 接入 §5.6 Wounds.bleed_rate
- `Burning{rate}` — 扣 health + 概率烧经脉（接入 §1.1 MeridianCrack）
- `Poisoned{rate, infectious}` — §5.4 Insidious 染色慢性
- `QiCorrosion{rate}` — 扣 qi_current（异体染色常驻）
- `ContaminationOverflow` — contam 超阈扣神识

**B. 控制（tag = CONTROL|DEBUFF，ExtendDuration cap=8s）**
- `Stunned` — 禁移动/攻击/施法
- `Rooted` — 禁移动，可攻击/施法
- `Silenced` — 禁施法
- `Disarmed` — 禁武器攻击（拳仍可）
- `Slowed{pct}` → AttrMod(Speed, MulTotal, -magnitude)
- `Hastened{pct}` → AttrMod(Speed, MulTotal, +magnitude)

**C. 感知/隐匿（tag = PERCEPTION）**
- `Invisible{to_realm_below}` — 低境界看不见
- `Revealed` — 强制可见（反隐身）
- `Blinded` — 视野缩至 3 格
- `AuraMasked` — NPC 敌意判定失效

**D. 属性加成（tag = BUFF/DEBUFF，Refresh）**
- `DamageAmp{pct}` → AttrMod(AttackDamage, MulTotal, +)
- `DamageReduction{pct}` → AttrMod(IncomingDamage, MulTotal, -)
- `QiThroughputBoost{pct}` → AttrMod(QiThroughputMax, MulBase, +)
- `QiRegenBoost{rate}` → AttrMod(QiRegen, Add, +)
- `DefenseWindowExt{ms}` → AttrMod(DefenseWindow, Add, +)

**E. 特殊状态（tag 组合 + SingletonIgnore / PerSource）**
- `Phasing` — 替尸流蜕壳后 1s 无敌
- `SoulMarked{by}` — PerSource，追踪 + 复仇 karma
- `TribulationLocked` — 渡劫中，免疫外部干预
- `NearDeath` — §5.6 濒死（接 §8.2）
- `QiBonded{treasure}` — 法宝激活，qi_pool 持续消耗

### 7.5 Intent / 事件

```rust
// Intent（写入 IntentQueue）
pub struct ApplyStatusEffectIntent {
    pub target: Entity,
    pub kind: EffectKind,             // 不直接传 EffectId，由 apply_tick 按 policy 组合
    pub magnitude: f32,
    pub duration: Duration,
    pub source: EffectSource,
    pub source_color: Option<QiColor>,
}

pub struct DispelStatusIntent {
    pub target: Entity,
    pub tag_mask: EffectTags,
    pub max_dispel_level: DispelTag,   // Easy 只清 Easy，Hard 清 Easy+Hard
}

// Event（发出 CombatEvent 侧流）
pub enum StatusEffectEvent {
    Applied { entity, id, magnitude, duration, source, color },
    Refreshed { entity, id, new_magnitude, new_duration },
    Stacked { entity, id, stacks },
    Expired { entity, id },
    Dispelled { entity, id, by: DispelSource },
}
```

### 7.6 Tick 系统（接入 §2 管线）

与 §2 主管线对齐（本节三个 tick 对应 §2 的 C0c / C2 / C10）：

```
C0c  StatusEffectApplyTick     // event-driven
                                // 消费 ApplyStatusEffectIntent + DispelStatusIntent
                                // 按 StackPolicy 写入 / 刷新 / 叠层，过 resist_tags 滤除
C2   StatusEffectTick (5Hz)    // 遍历 active，decr duration，到点触发 on_tick
                                // duration<=0 → on_remove + 移除 + 发 Expired 事件
C10  AttributeAggregateTick    // 全量聚合 active effects 的 AttrMod，写入 DerivedAttrs
                                // 下游 tick（Attack/Defense/Movement）只读 DerivedAttrs
```

**伪码**：
```rust
fn status_effect_apply_tick(
    mut q: Query<&mut StatusEffects>,
    defense_q: Query<&DefenseLoadout>,
    mut reader: EventReader<ApplyStatusEffectIntent>,
    mut events: EventWriter<CombatEvent>,
) {
    for intent in reader.read() {
        let Ok(mut se) = q.get_mut(intent.target) else { continue };
        let Some(spec) = EFFECT_REGISTRY.get(&intent.kind) else { continue };

        // 抗性过滤
        let mut magnitude = intent.magnitude;
        if let Ok(defense) = defense_q.get(intent.target) {
            if defense.resist_tags.intersects(spec.tags) {
                magnitude *= 1.0 - defense.resist_magnitude;
                if magnitude <= 0.01 { continue; }
            }
        }

        // 根据 stack_policy 决定存储 key
        let id = match spec.stack_policy {
            StackPolicy::PerSource => EffectId::per_source(intent.kind, intent.source),
            _ => EffectId::single(intent.kind),
        };

        // 按 policy 写入/更新
        let entry = se.active.entry(id).or_insert_with(|| ActiveEffect::new(spec, &intent));
        match spec.stack_policy {
            StackPolicy::Refresh => {
                entry.magnitude = entry.magnitude.max(magnitude);
                entry.duration  = entry.duration.max(intent.duration);
            }
            StackPolicy::StackIndependent => {
                entry.stacks = (entry.stacks + 1).min(STACK_CAP);
                entry.duration = intent.duration;
            }
            StackPolicy::ExtendDuration { cap } => {
                entry.duration = (entry.duration + intent.duration).min(cap);
            }
            StackPolicy::SingletonIgnore => { /* or_insert 已处理 */ }
            StackPolicy::PerSource => {
                entry.duration = intent.duration;
            }
        }

        if let Some(on_apply) = spec.on_apply { on_apply(world, intent.target, entry); }
        events.send(CombatEvent::StatusEffectOp(StatusEffectEvent::Applied { ... }));
    }
}

fn status_effect_tick(mut q: Query<(Entity, &mut StatusEffects)>, time: Res<Time>) {
    for (entity, mut se) in &mut q {
        let mut to_remove = vec![];
        for (id, e) in se.active.iter_mut() {
            e.duration = e.duration.saturating_sub(time.delta());
            if let Some(iv) = e.spec.tick_interval {
                if now >= e.next_tick_at.unwrap_or(now) {
                    (e.spec.on_tick.unwrap())(world, entity, e);
                    e.next_tick_at = Some(now + iv);
                }
            }
            if e.duration.is_zero() { to_remove.push(*id); }
        }
        for id in to_remove { /* on_remove + emit Expired */ }
    }
}

fn attribute_aggregate_tick(q: Query<(&StatusEffects, &mut DerivedAttrs)>) {
    for (se, mut attrs) in &mut q {
        attrs.reset_from_base();
        for e in se.active.values() {
            for m in e.spec.attribute_modifiers {
                let v = m.value_per_magnitude * e.magnitude * e.stacks as f32;
                attrs.apply(m.target, m.op, v);
            }
        }
    }
}
```

### 7.7 与各流派/防御/功法的接入

- **§4 三种防御**
  - JieMai（截脉）：`resist_tags = QI`，`resist_magnitude = 0.5`，命中前反弹时额外 `DispelStatusIntent { tag_mask=QI, max_dispel_level=Hard }`
  - TiShi（体势）：`resist_tags = DOT|PHYSICAL`，`resist_magnitude = 0.5`
  - JueLing（绝灵）：`resist_tags = QI`，免疫 QiCorrosion/QiBonded
- **§5.1-§5.4 四攻流派**：各 Intent 产出 `ApplyStatusEffectIntent` 而非自写一次性 debuff
- **§5.5 Technique**：新增 `Technique.on_hit_effects: Vec<EffectPrototype>`，命中后自动 emit ApplyIntent
- **§5.6 HealingState**：Bleeding effect 持续施加 `Wound.bleed_rate`（双向耦合）
- **§6.7 TreasureAbility**：法宝技能可挂 effect（例：「朱雀印」→ `Stunned{3s}` + `Burning{rate=2}`）
- **§8.2 DeathArbiter**：死亡时清除所有 `!= Ironclad` 效果；Ironclad 随 LifeRecord 归档
- **§10.1 雷劫波次**：`ApplyStatusEffectIntent { id=Burning, magnitude=0.8, duration=10s }`
- **§10.1 心魔波次**：`ApplyStatusEffectIntent { id=Silenced, duration=5s }` + 扣 composure

### 7.8 客户端 UI

- inspect 新增 "状态" 面板：按 kind 分组，图标 + 剩余时间条 + 叠层数
- HUD 顶部状态栏：最多 8 个最紧急效果（DoT 优先 → 控制 → 加成）
- 图标描边用 `source_color` 染色（被谁打的一目了然）
- Tooltip：来源 entity / zone / 染色 + dispel 难度 + 剩余时间精确到 0.1s

### 7.9 IPC Schema

```typescript
// agent/packages/schema/src/status-effect.ts
export const CHANNEL_STATUS_EFFECT = "bong:status_effect";

export const StatusEffectEvent = Type.Object({
  entity: EntityRef,
  op: Type.Union([
    Literal("applied"), Literal("refreshed"),
    Literal("stacked"), Literal("expired"), Literal("dispelled"),
  ]),
  effect_id: Type.String(),
  magnitude: Type.Number(),
  stacks: Type.Number(),
  duration_ms: Type.Number(),
  source: EffectSource,
  source_color: Type.Optional(QiColor),
});

// WorldStateV1.players[].status:
status: {
  active_effects: Array<{
    id: string, magnitude: number, stacks: number,
    remain_ms: number, source_color?: QiColor,
  }>,
}
```

### 7.10 实施阶段

- **C2**（完整攻击事务）：StatusEffects component + EffectSpec 注册基础设施 + 最小子集 `Bleeding/Stunned/Slowed/DamageAmp` + Apply/Dispel Intent + AttributeAggregateTick
- **C4**（恶化机制时同步）：扩展 DoT 全家（Burning/Poisoned/QiCorrosion） + 抗性过滤接入三防
- **C5**（四攻三防完整流派）：Mental-tag（Silenced/Blinded/AuraMasked） + 替尸 `Phasing` + 毒蛊 infectious 传染
- **C6**（天劫）：`TribulationLocked` 单例 + 雷劫 Burning + 心魔 Silenced
- **C7**（飞行）：`QiBonded` 接入御剑/飞剑类功法
- **后期**：`SoulMarked` PerSource + 跨重生 Ironclad 归档 LifeRecord

### 7.11 与 Pumpkin 等价表（对 agent/开发者）

| Pumpkin | 本 plan |
|---|---|
| `LivingEntity::active_effects` | `StatusEffects` component |
| `LivingEntity::add_effect` | `ApplyStatusEffectIntent` + `StatusEffectApplyTick` |
| `LivingEntity::tick_effects` | `StatusEffectTick` |
| `LivingEntity::remove_effect` | duration=0 or `DispelStatusIntent` → `on_remove` |
| `apply_effect_tick` match | `EffectSpec.on_tick` fn pointer |
| `attribute_modifiers` | `AttrModSpec` + `AttributeAggregateTick` 聚合 |
| `CUpdateMobEffect` packet | Redis `CHANNEL_STATUS_EFFECT` + Fabric CustomPayload |
| `CRemoveMobEffect` packet | `StatusEffectEvent::Expired/Dispelled` |
| `send_active_effects` on join | WorldStateV1 快照下发 |
| NBT 持久化 | LifeRecord JSON（仅 Ironclad） |

---

## 8. 死亡-重生流程（统一收口）

### 8.1 致死事件来源

```rust
// 战斗端产生
emit DeathEvent { entity, cause: HealthZero | BleedOut | NearDeathTimeout
                                 | HeartChannelSevered | SoulShattered }

// 修炼端产生（cultivation plan §4 上报）
on CultivationDeathTrigger { entity, cause, context }:
  emit DeathEvent { entity, cause: FromCultivation(cause), context }

// 渡劫强制终结（修炼端标记 no_fortune: true）
on CultivationDeathTrigger { cause: TribulationFailure, no_fortune: true }:
  emit DeathEvent { entity, cause, force_terminate: true }
```

### 8.2 DeathArbiter 系统

```
on DeathEvent:
  1. 锁定玩家位置，禁止移动/施法
  2. 若 force_terminate → 直接进入终结流程（跳过运数）
  3. 计算运数/劫数（worldview §十二）：
     if Lifecycle.death_count < 3:
       check 运数豁免条件：
         - 死前 24h 内未死过
         - 死亡地点不在死域/负灵域
         - Karma.weight < 阈值
         - Lifecycle.spawn_anchor.is_some()
       if 任一满足：decision = Fortune, fortune_remaining -= 1
       else：进入概率期
     if 进入概率期：
       n = death_count + 1
       p = max(0.05, 0.80 - 0.15 × (n - 3))
       roll = random()
       decision = if roll < p { RolledSurvived(p) } else { RolledFailed(p) }
  4. 生成遗念（按境界详细度，调 deathInsight tool）
  5. 推送 DeathScreen { decision, p_shown, deathnote } 到客户端
  6. 掉落 50% 物品到死亡点（任何人可拾取）
  7. 写入 LifeRecord.biography
  8. emit CombatEvent::Died → CHANNEL_COMBAT_REALTIME（§1.5.2/§1.5.7 critical 级）
  9. enter LifecycleState::AwaitingRevival { roll_result: decision }
```

### 8.3 重生流程

```
client confirms revive (or 60s 自动确认):
  if decision in {Fortune, RolledSurvived}:
    - 传送到 Lifecycle.spawn_anchor 或世界出生点
    - Wounds.entries.clear()
    - Wounds.health_current = health_max
    - emit PlayerRevived { entity, penalty: RebirthPenalty }
      └ 修炼 plan 监听：境界 -1, qi=0, composure=0.3, contam.clear, 关闭最高 tier 经脉
    - Lifecycle.weakened_until = now + 3min
    - Lifecycle.death_count += 1
    - Lifecycle.last_revive_at = now
    - 触发 InsightRequest::PostRebirth（30s 内）→ 转发给 cultivation insight runtime
    - LifecycleState = Alive
  else (RolledFailed | DirectTerminate):
    enter 终结流程（§8.4）
```

### 8.4 终结归档

```
on Terminate:
  1. Lifecycle.state = Terminated
  2. emit PlayerTerminated { entity, character_id }
     └ 修炼 plan 监听：停止该 entity 所有修炼 tick
  3. 调 deathInsight tool 生成「终焉之言」→ 写入 LifeRecord.biography 末页
  4. 生成生平卷快照：
     write data/biography/<character_id>.json
     // 由 library-web 独立项目读取并展示，server 不实现展示层
  5. 道统遗物：
     if 角色在 Solidify+ 境界终结：
       掉落功法残篇 + 法宝（从 LifeRecord 持有物中按规则选）
       推 era agent narration："某地有道统遗落"
  6. 客户端显示终结画面，提供"创建新角色"按钮
  7. 该 entity 从世界 despawn
```

### 8.5 遗念生成

worldview §十二 已决策：**真实信息，无谎言**，按境界递增详细度。

```
deathInsight tool 输入：
  - character_id, realm, last_position
  - 死因 + 攻击者 ID（如有）
  - 该位置 zone 数据（spirit_qi, terrain）
  - 周围 200 格内显著实体/资源点

输出按境界过滤：
  Awaken/Induce: 仅死因 + 附近最显著的 1 条灵气波动
  Condense/Solidify: + 方向距离 + 敌人弱点 + 地理细节
  Spirit/Void: + 灵脉走向 + 其他高手位置 + 天道层面洞察

特殊规则：
  RolledFailed 时追加："此次运数 p%，下次更低"
  Terminated 时追加「终焉之言」（agent 生成的告别词）
```

---

## 9. Insight 触发点（向修炼 plan 转发）

战斗 plan emit 以下 trigger，转发给 cultivation plan 的 insight runtime：

| 触发 ID | 触发条件 |
|---|---|
| `killed_higher_realm` | 击杀比自己高一阶以上的对手 |
| `killed_by_higher_realm_survived` | 被高境界打到 NearDeath 后救回 |
| `near_death_survival` | 进入 NearDeath 状态后未触发 DeathEvent |
| `last_chance_survived` | 劫数期 RolledSurvived |
| `post_rebirth_clarity` | 重生后 30s 内（无论运数还是劫数） |
| `first_tribulation_survived` | 扛过任意天劫波次（含化虚渡劫） |
| `witnessed_xuhua_tribulation` | 50 格内目击他人化虚渡劫（成败均触发） |

战斗 plan 还需为顿悟白名单 **F 类（流派类）** 注册 sub-whitelist 给 cultivation arbiter 引用：
- `unlock_practice[carrier_qi_seal_efficiency]` 暗器流封存效率
- `affinity[QiColor::Solid][material::beast_bone]` 凝实色 × 异变兽骨亲和
- `discount[defense_qi_cost][JueLing]` 涡流流维持成本折扣
- 等等（具体由战斗 plan 在 P3-P4 期间扩充）

---

## 10. 天劫伤害施加

修炼 plan 触发渡劫状态机，**本 plan 实施伤害施加**。

### 10.1 普通天劫（突破/业力触发）

```
on TribulationStart { entity, script: TribulationScript }:
  for each wave in script.waves:
    sleep wave.delay
    apply wave.damage to entity:
      - 雷劫：穿透防御，直接 wound_damage + contam (Violent color)
      - 心魔：神识攻击，扣 Cultivation.composure
      - 外敌劫：刷新临时敌对 NPC（big-brain 接管）
    if Wounds.health_current <= 0:
      emit DeathEvent { cause: TribulationFailure }
      break
```

### 10.2 化虚渡劫（worldview §三 全服广播）

```
on InitiateXuhuaTribulation { entity }:
  全服 broadcast: "有人在渡虚劫"
  calamity agent 生成 TribulationScript（多波次，强度极高）
  设定 50 格观战圈：圈内玩家可干预（杀渡劫者获天道 narration 关注）
  
  执行波次（同 10.1）但任一波失败即触发：
    emit DeathEvent { cause: TribulationFailure, force_terminate: true }
    // §8.1 收到后跳过运数判定直接终结
  
  扛过所有波次：
    通知 cultivation plan: realm = Void
    触发 InsightRequest::FirstTribulationSurvived
    全服 narration："有人渡过虚劫"
```

---

## 11. IPC Schema 扩展（战斗相关）

新增 `agent/packages/schema/src/`:

### 11.1 新增 Channels

```typescript
// channels.ts
export const CHANNEL_COMBAT_REALTIME  = "bong:combat_realtime";   // critical 事件实时
export const CHANNEL_COMBAT_SUMMARY   = "bong:combat_summary";    // 聚合 5s 窗口（§1.5.7）
export const CHANNEL_DEATH_EVENT      = "bong:death_event";
export const CHANNEL_TRIBULATION_EVENT= "bong:tribulation_event";
export const CHANNEL_STATUS_EFFECT    = "bong:status_effect";     // §7.9
export const CHANNEL_ANTICHEAT        = "bong:anticheat";         // §1.5.6 AntiCheatCounter
// CHANNEL_INSIGHT_* 由修炼 plan 定义；战斗 plan 通过内部 event bus 转发
```

### 11.2 新增 Schema 文件

```
agent/packages/schema/src/
├── combat-event.ts          # 攻击/防御/命中事件流（含 CombatSummary）
├── combat-packets.ts        # 客户端→服务端 CustomPayload 包 schema（§1.5.4）
├── status-effect.ts         # StatusEffectEvent + EffectId 枚举（§7.9）
├── death-event.ts           # 死亡事件 + 运数判定 + 重生/终结
├── tribulation-event.ts     # 天劫脚本 + 波次执行
├── deathnote.ts             # 遗念 + 终焉之言结构
├── anticheat.ts             # AntiCheatCounter 上报 schema（§1.5.6）
└── biography-combat.ts      # 战斗侧 BiographyEntry 扩展
```

### 11.3 WorldStateV1 扩展

```typescript
WorldStateV1.players[].combat: {
  health_current, health_max,
  wound_count: number,
  in_combat: boolean,
  defense_style: "JieMai" | "TiShi" | "JueLing" | "None",
}
WorldStateV1.players[].lifecycle: {
  death_count, fortune_remaining,
  state: "Alive" | "NearDeath" | "AwaitingRevival" | "Terminated",
  weakened_remaining_sec: number,
}
WorldStateV1.players[].stamina: {
  current: number, max: number,
  state: "Idle" | "Walking" | "Jogging" | "Sprinting" | "Combat" | "Exhausted",
}
WorldStateV1.players[].status: {       // §7.9
  active_effects: Array<{
    id: string, magnitude: number, stacks: number,
    remain_ms: number, source_color?: QiColor,
  }>,
}
WorldStateV1.players[].derived_attrs: { // §1.5.3 精简快照（agent 可读 buff 总览，不传全部字段）
  speed_sprint: number,
  attack_damage_mul: number,
  incoming_damage_mul: number,
  flight_enabled: boolean,
  phasing: boolean,
  tribulation_locked: boolean,
}
```

---

## 12. 客户端集成

新增/扩展 `client/src/main/java/com/bong/client/combat/`。草图总索引：[`docs/svg/README.md`](./svg/README.md)。

| UI | 内容 | 草图 |
|---|---|---|
| inspect 伤口层 | 渲染 `Wounds[]` 实际位置/严重度（已有骨架，本 plan 接数据） | [svg](./svg/inspect-wounds.svg) |
| 真元条 | 战斗中 throughput_current 显示在真元条上方（峰值高亮） | [svg](./svg/hud-combat.svg) |
| 攻击 HUD | 武器/法术快捷栏，qi_invest 滑块，攻击键绑定 | [svg](./svg/hud-combat.svg) · [panels](./svg/attack-panels.svg) |
| 状态效果 HUD | 顶部状态栏 8 槽 + 图标染色 + 剩余时长条 + 叠层数（§7.8） | [svg](./svg/hud-combat.svg) |
| inspect 状态面板 | 按 kind 分组的全量 active_effects 列表 + 来源 tooltip | [svg](./svg/inspect-status.svg) |
| Stamina 条 | 独立于 qi/health，跑/冲刺时明显消耗 + state 彩色 | [svg](./svg/hud-combat.svg) |
| DerivedAttrs HUD | 飞行/虚化/渡劫锁定等特殊状态顶部大图标提示 | [svg](./svg/hud-combat.svg) |
| 法术体积滑块 | radius / velocity_cap 双滑块（§3.5） | [svg](./svg/attack-panels.svg) |
| 防御 UI | 截脉极限弹反指示器（200ms 窗口提示）/ 涡流激活键 / 伪皮层数 | [svg](./svg/defense-ui.svg) |
| 暗器制作面板 | ForgeWeaponCarrier UI（选物 + 注真元 + 计时） | [svg](./svg/attack-panels.svg) |
| 阵法布置 UI | 选方块 + 选触发类型 + 注真元 | [svg](./svg/attack-panels.svg) |
| 武器/法宝检视 | 普通武器 tooltip + 法宝展开（bond/qi_pool/abilities/prev_owners）（§6.10） | [svg](./svg/weapon-treasure.svg) |
| 死亡画面 | 运数信息 + 遗念文本 + 重生/终结按钮 + 60s 自动确认倒计时 | [svg](./svg/death-screens.svg) |
| 终结画面 | 终焉之言 + 创建新角色按钮 | [svg](./svg/death-screens.svg) |
| 全服天劫广播 | 屏幕顶部红字 + 雷云图标 + 距离/方向指引 | [svg](./svg/tribulation-ui.svg) |
| 渡劫观战镜头 | 50 格内自动提示是否前往观战 | [svg](./svg/tribulation-ui.svg) |

修炼相关 UI（经络层数据、突破闭关、顿悟选择、淬炼）由修炼 plan §7 接管。

**草图仅表达布局与数据位置**，视觉风格（像素尺寸、字体、配色）由 plan-client.md 定。

---

## 13. 阶段化实施路线

按可独立验证的最小切片划分。**前置依赖修炼 plan 的 P1-P2**（Cultivation/MeridianSystem 已上线）。

### C1：基础设施 + 受伤与气血 ✅（2026-04-21 验收）
**验证标准**：玩家手动添加 wound → 持续掉血 → 死亡触发 DeathEvent（暂不走完整流程）

```
✓ Wounds + CombatState + Stamina + Lifecycle + DerivedAttrs component
✓ Intent Event 基础设施（§1.5.1）+ CombatEvent 发布管道（§1.5.2）
✓ Server raycast 工具（§1.5.5 方案 A slab-test）
✓ CustomPayload 入/出站骨架（§1.5.4 先通 attack 一条路径）
✓ WoundBleedTick + CombatStateTick + StaminaTick
✓ AttributeAggregateTick（空聚合也先连起来）
✓ 反作弊清单最小子集（reach + cooldown + qi_invest clamp）
✓ 调试命令：/wound add, /health set, /stamina set
✓ Client: 伤口层 + stamina 条 + DerivedAttrs HUD 占位
✓ DeathEvent emit（仅打日志，无重生）
```

### C2：完整攻击事务 + 最小状态效果 ✅（2026-04-21 验收，server 侧；客户端 HUD 归 plan-combat-ui_impl.md）
**验证标准**：两玩家 PvP，体修拳能打出血 + 写 contam，对方截脉防御部分中和；命中会施加 Bleeding + Slowed

```
✓ AttackIntent → 6 步事务（含 raycast body_part 分类）
✓ 距离衰减管线（QiDecayInFlightTick + DecayConfig）
✓ 写入修炼 plan 的 Contamination + throughput_current
✓ 截脉防御（DefenseWindow + DefenseIntent）
✓ StatusEffects 注册表基础设施 + 最小子集 {Bleeding, Stunned, Slowed, DamageAmp}
✓ StatusEffectApplyTick + StatusEffectTick + AttributeAggregateTick 全链路联通
✓ CombatEvent 推 Redis（realtime + summary 分流）
✓ Client: 攻击/防御 HUD + 命中特效 + 状态效果 HUD 条
```

### C3：死亡-重生流程 ✅（2026-04-21 验收，server + IPC schema；客户端死亡画面/遗念显示归 plan-combat-ui_impl.md）
**验证标准**：玩家被击杀 → 运数判定 → 重生 → 修炼 plan 应用境界 -1

```
✓ Lifecycle component
✓ DeathArbiter 系统（合并战斗 + 修炼侧致死源）
✓ 运数豁免 + 概率衰减 roll
✓ 遗念 agent（deathInsight tool）
✓ PlayerRevived event → 修炼 plan 应用惩罚
✓ Client: 死亡画面 + 遗念显示 + 重生确认
✓ NearDeath 状态 + 30s 自救窗口
```

### C4：终结归档
**验证标准**：第 N 次死亡 RolledFailed → 角色 Terminated → 生平卷写盘 → library-web 可读

```
✓ Terminate 流程
✓ 「终焉之言」生成
✓ 生平卷 JSON 快照写 data/biography/
✓ 道统遗物掉落（Solidify+ 角色）
✓ Client: 终结画面 + 新角色入口
```

### C5：四攻三防完整流派
**验证标准**：所有流派可选可用，克制关系生效

```
✓ 爆脉流（ExecuteBopuIntent + 过载施加 + qi_max_frozen）
✓ 暗器流（WeaponCarrier + ForgeWeaponCarrierIntent + ThrowCarrierIntent）
✓ 阵法流（QiTrap + PlaceQiTrapIntent + 触发系统）
✓ 毒蛊流（ShootDuguIntent + Insidious 染色慢性侵蚀）
✓ 替尸流（FakeSkinStack 物品 + 蜕壳逻辑）
✓ 涡流流（VortexActivate + 反噬）
✓ 染色对战斗加成的 QiColorConfig 数值表
✓ PvP 平衡测试：单染色胜率 45-55%，克制关系 ~60/40
```

### C6：天劫伤害施加
**验证标准**：突破触发普通天劫 + 化虚渡劫全服广播 + 观战圈

```
✓ TribulationStart 监听 + 波次执行
✓ 雷劫 / 心魔 / 外敌劫三种波次类型
✓ 化虚渡劫强制 force_terminate
✓ 50 格观战圈 + 干预机制
✓ 全服广播 UI
```

### C7：飞行能力（化虚专属）
**验证标准**：化虚境玩家学到 Flight 功法后可空中飞行，qi 耗尽强制下落

```
✓ Flight 类 Technique + qi sustain 检测
✓ Valence ClientboundPlayerAbilities fly = true 控制
✓ 飞行状态禁用施法 + 战斗 qi_drain ×3
✓ 负灵域内禁用飞行
✓ 御风诀 / 凌虚 两个示例功法
✓ 空中战斗 PvP 验证（化虚 vs 化虚）
```

---

## 14. 测试策略

### 单元测试
- 距离衰减公式边界（0/10/50 格 × 各染色 × 各载体）
- 异体排斥写入：contam.amount / qi_color 正确写入
- 运数判定矩阵：4 个豁免条件全组合
- 概率衰减：第 4-10 次死亡 p 值正确性
- 防御反应：截脉窗口 / 蜕壳层数 / 涡流反噬时机

### 集成测试
- 完整攻击事务（命中/未命中/被防/虚化）
- 完整死亡链路：DeathEvent → ArbiterTick → 遗念 → PlayerRevived → 修炼 plan 响应
- 终结链路：RolledFailed → Terminated → 生平卷写盘 → entity despawn
- 跨 plan：CultivationDeathTrigger → 战斗 plan 收口 → PlayerRevived 回送修炼 plan

### E2E 测试
- 扩展 `scripts/smoke-test-e2e.sh`，新增 PvP 战斗场景
- mock 玩家死 1-10 次，验证运数耗尽 → 终结链路
- mock 化虚渡劫，验证全服广播 + 强制终结

### 平衡测试（手动）
- 不同染色 PvP 胜率统计（worldview 强制要求 45-55%）
- 不同流派对战胜率（克制关系 ~60/40）
- 终结实际发生率（前 50 玩家平均存活死亡数）

---

## 15. 文件规划

```
server/src/combat/
├── mod.rs
├── components.rs           # Wounds / CombatState / WeaponCarrier / QiTrap / DefenseLoadout / Lifecycle / Stamina
├── intents.rs              # 所有 Intent Event 定义 + IntentSource（§1.5.1）
├── events.rs               # CombatEvent + publisher（§1.5.2）+ 节流路由（§1.5.7）
├── derived_attrs.rs        # DerivedAttrs component + AttributeAggregateTick（§1.5.3）
├── raycast.rs              # server 端 AABB slab-test raycast + BodyPart 分类（§1.5.5）
├── anticheat.rs            # AntiCheatCounter + 校验辅助（§1.5.6）
├── packets/                # Fabric CustomPayload 入/出站（§1.5.4）
│   ├── inbound.rs          # bong:combat/attack / defense_stance / spell_volume / ...
│   └── outbound.rs         # server → client 事件/快照同步
├── attack.rs               # AttackIntent 6 步事务
├── decay.rs                # 距离衰减 + QiDecayInFlightTick
├── defense.rs              # 三种防御反应
├── stamina.rs              # Stamina component + StaminaTick（§2 C9）
├── styles/
│   ├── bopu.rs             # 爆脉流
│   ├── anqi.rs             # 暗器流 + WeaponCarrierDecayTick
│   ├── zhenfa.rs           # 阵法流 + QiTrapDecayTick
│   └── dugu.rs             # 毒蛊流
├── wound.rs                # WoundBleedTick + 伤口逻辑 + Treatment
├── healing.rs              # HealingState + Scar + 三层疗愈（§5.6）
├── techniques.rs           # Technique system + 学习/挂载/校验（§5.5）
├── weapon.rs               # Weapon / Treasure / SoulBond / 铸造修复（§6）
├── status_effects/         # §7 StatusEffect 库
│   ├── spec.rs             # EffectSpec + EFFECT_REGISTRY
│   ├── active.rs           # StatusEffects component + ActiveEffect
│   ├── apply_tick.rs       # StatusEffectApplyTick（§7.6 C0c）
│   ├── tick.rs             # StatusEffectTick（§7.6 C2）
│   └── kinds/              # 各 EffectKind 的 on_apply/on_tick/on_remove
├── lifecycle/
│   ├── death_arbiter.rs    # DeathEvent 收口 + 运数/概率判定
│   ├── revive.rs           # 重生流程 + PlayerRevived 发布
│   ├── terminate.rs        # 终结归档 + 生平卷写盘
│   └── deathnote.rs        # 遗念生成 tool 调用
└── tribulation.rs          # 天劫波次执行

server/src/network/
└── (扩展) redis_bridge.rs  # 新增战斗相关 channels 发布

agent/packages/schema/src/
├── combat-event.ts
├── combat-packets.ts       # 客户端 CustomPayload 包 schema（§1.5.4）
├── status-effect.ts        # StatusEffectEvent（§7.9）
├── anticheat.ts            # AntiCheatCounter 上报
├── death-event.ts
├── tribulation-event.ts
├── deathnote.ts
└── biography-combat.ts

agent/packages/tiandao/src/
├── tools/death-insight.ts       # 遗念生成工具
├── tools/tribulation-script.ts  # 天劫脚本生成
└── (扩展) calamity-runtime.ts   # 监听 BreakthroughStarted 触发天劫

client/src/main/java/com/bong/client/combat/
├── attack/                 # 攻击 HUD + 暗器/阵法布置 + 法术体积滑块
├── defense/                # 截脉/涡流/蜕壳 UI
├── status/                 # 状态效果 HUD 条 + inspect 状态面板（§7.8）
├── stamina/                # Stamina 条 + 状态颜色
├── derived_attrs/          # 飞行/虚化/渡劫特殊状态大图标
├── inspect-wound/          # 伤口层数据绑定（扩展现有 inspect）
├── packets/                # CustomPayload 入/出站 Fabric codec
├── death/                  # 死亡画面 + 遗念 + 重生确认
├── terminate/              # 终结画面 + 新角色入口
└── tribulation/            # 全服广播 + 观战圈
```

---

## 16. 已决策事项

1. **死亡-重生流程归战斗 plan 收口** ✅
   - 修炼侧致死缘由通过 `CultivationDeathTrigger` 上报，由本 plan 的 DeathArbiter 统一处理运数/概率/遗念/重生/终结
   - 修炼 plan 监听 `PlayerRevived` / `PlayerTerminated` 应用修炼侧响应
2. **遗念真实，无高阶谎言** ✅（worldview §十二）
3. **生平卷全量保留** ✅，由 library-web 负责分页/搜索
4. **亡者博物馆不在 server 实现** ✅
   - server 仅生成 `data/biography/<character_id>.json`
   - library-web 独立读取并提供查阅 UI
5. **客户端 inspect UI 复用现有骨架** ✅
   - 经络层归修炼 plan，伤口层归本 plan
   - 已有 `client/.../inventory/` 经脉骨架，本 plan 只接伤口数据绑定
6. **化虚渡劫死即终结** ✅（force_terminate 跳过运数）
7. **染色平衡硬约束** ⚠️ TODO：
   - 任何单一染色 PvP 胜率必须 45-55%
   - 克制关系明确但不绝对（克制方 ~60%）
   - C5 实施时通过 mock PvP 强制验证
8. **Intent 走 Bevy `Events<T>`，不用独立 Resource 队列** ✅（§1.5.1）
   - 生产者/消费者单向流动，Bevy 调度保证 1-frame 生命周期
   - 失败不回滚，emit IntentRejected 通知客户端
9. **Server raycast 自写 AABB slab-test（方案 A）** ✅（§1.5.5）
   - C1-C4 用方案 A；需要扇形/OBB 时再切 parry3d（方案 C）
   - 不引入 bevy_rapier（overkill）
10. **凡人级弱者基线 + NPC 同基线** ✅（§1.4）
    - 走 1.4 / 跑 3.0 / 冲刺 5.5 m/s；体修可进化到 15 m/s
    - 初始 health 30，化虚 200+
11. **反作弊 server 权威重算** ✅（§1.5.6）
    - 10 项 hint 字段一律不信，server raycast 为准
    - AntiCheatCounter 阈值触发推 CHANNEL_ANTICHEAT
12. **CombatEvent 双 channel 节流** ✅（§1.5.7）
    - realtime: Death/NearDeath/Terminated/Tribulation/WeaponBroken/ScarFormed
    - summary: 普通 AttackResolved/WoundApplied/StatusEffectOp 聚合 5s 窗口
13. **StatusEffect 采纳 Pumpkin 结构 + 5 处关键偏离** ✅（§7.2）
    - 静态 Spec + 运行时 ActiveEffect；fn pointer 分派；magnitude f32 连续值
    - 聚合式属性修饰（不覆写 base）；客户端走 Fabric CustomPayload 不走 vanilla MobEffect packet
14. **法宝魂契 soul-bond 跨重生保留，Terminate 才 severed** ✅（§6.6）
    - Ironclad 效果也随 LifeRecord JSON 归档（§7）

### 仍需在实施中确定（非阻塞）

- 各武器/法术的 wound_damage / contam_amount 系数（C2 起初版 + 后期调）
- 染色对战斗的具体加成数值（C5 实施时通过 PvP 模拟调）
- 天劫各波次的伤害基线（C6 实施时定）
- 道统遗物掉落规则（C4 实施时定，仅 Solidify+ 触发）

---

## 17. 与现有计划的关系

### 已存在的 plan

- **plan-cultivation-v1.md**：定义本 plan 写入的 `Contamination` / `MeridianCrack` / `Cultivation.qi_current` 等共享 component；本 plan emit 的 `PlayerRevived` / `PlayerTerminated` / `InsightRequest` 由其消费
- **plan-server.md**：本 plan 是其下战斗细化分支
- **plan-agent-v2.md**：本 plan 新增 deathInsight / tribulationScript tool，复用现有 calamity agent runtime
- **plan-client.md**：本 plan 要求 client 扩展 inspect 伤口层 + 战斗/死亡 UI
- **library-web**：消费本 plan 写入的 `data/biography/` 生平卷快照

### 待定 plan（占位引用）

- **plan-food-v1.md**（TODO）—— 食物/生存 plan
  - 定义食物种类、饱腹度、来源（狩猎/采集/炼丹/炊事）、睡眠机制
  - 对本 plan 的接入点：emit `ApplyStatusEffectIntent { id: WellFed | Rested }`；效果接入 §5.6 `health_recover_pool` 和 §2 C9 `StaminaTick`
  - 在该 plan 落地前，调试阶段用 `/effect give WellFed` 模拟

- **plan-stealth-chase-v1.md**（TODO）—— 移动/侦察/追击 plan（"搜打撤"玩法循环）
  - 定义痕迹追踪、隐匿脱敌、追击速度差、复仇追查
  - 对本 plan 的接入点：
    - 消费 `CombatEvent::Disengaged`（§3.7.3）作为"撤"的信号
    - 写入 `DerivedAttrs.stealth_level`（§1.5.3 预留字段）影响敌意检测
    - 消费 `SoulMarked` PerSource StatusEffect 做追踪定位
    - 写入 `CombatState.in_combat` 的敌意检测逻辑（30 格内敌意实体列表由侦察 plan 推演提供）

- **plan-npc-ai-v1.md**（TODO）—— NPC 战斗 AI 接入
  - big-brain Scorer/Action 消费本 plan 的 Wounds / StatusEffects / DefenseLoadout
  - 决定 NPC 何时切截脉 vs 体势、何时爆脉自爆、何时逃跑
  - 在该 plan 落地前，NPC 只发 AttackIntent，默认无防御

---

## 18. 验收里程碑

```
M3.1 — 受伤可见：C1 完成，玩家有血有伤
M3.2 — 战斗可玩：C2 完成，PvP 能打出血出 contam
M3.3 — 死亡有重量：C3 完成，运数机制工作
M3.4 — 终结归档：C4 完成，亡者博物馆可读
M3.5 — 流派齐全：C5 完成，四攻三防可选可用
M3.6 — 天劫成型：C6 完成，化虚渡劫全服广播
```

---

## 19. 进度日志

- 2026-04-21：C1+C2+C3 验收通过——`server/src/combat/{components,resolve,lifecycle,raycast,status,events,debug}.rs` 全数落地，IPC 双通道 `bong:combat_realtime` / `bong:combat_summary` 双端对齐（`agent/packages/schema/src/combat-event.ts` + `server/src/network/combat_bridge.rs`），`DeathEvent` → `DeathArbiter` → `PlayerRevived` / `PlayerTerminated` 收口，C4+ 终结归档与全部客户端 UI 仍按 worktree v1 边界禁碰。
- 2026-04-25：本地核查代码确认 C1/C2/C3 server 侧已实装（`Wounds` / `AttackIntent` / `Lifecycle` / `DefenseWindow` / `StatusEffectKind::{Bleeding,Stunned,Slowed,DamageAmp}` 全在）；阶段表与小节标题同步勾 ✅。
