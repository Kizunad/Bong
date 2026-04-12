# 修炼系统实现计划 V1

> 本计划落地 worldview.md §三/六 的修炼与个性机制。**战斗系统单独由 `plan-combat-v1.md` 承载，死亡-重生流程也归入战斗 plan**（worldview §十二）。本计划只定义修炼状态模型与恢复机制，战斗只是这些状态的"伤害源"，死亡只是这些状态的"终结点"。

**前置依赖**：
- worldview.md V2（含 §六 个性与差异化、§十二 一生记录）已定稿
- 客户端 inspect UI（经络双层 + 伤口）骨架已实现
- agent 三层（calamity/mutation/era）+ V1 schema + Redis bridge 已就绪

**与战斗 plan 的边界**：
- 本计划：定义 Cultivation/MeridianSystem/QiColor/Contamination/Karma/LifeRecord 等修炼相关 state component；定义 tick 推进规则（再生/排异/疗愈/打通/降境）；定义突破/淬炼事务；定义顿悟系统接入点；通过 `CultivationDeathTrigger` 事件向战斗 plan 上报"修炼侧致死缘由"（走火、爆脉、负灵域抽干、突破失败致死）
- 战斗计划：定义 Wounds/AttackIntent 事务、距离衰减、四攻三防流派、武器/法术系统、战斗节奏；**收口 DeathEvent + 重生流程**（运数/概率/遗念/重生惩罚/终结归档）；战斗端把异种真元残留**写入**本计划定义的 Contamination 状态，把过载流量**写入** MeridianCrack

---

## 进度快照 (2026-04-13, 更新)

**服务端 P1–P5 全部模块已落地（228 单元测试通过，clippy -D warnings 干净）** — `server/src/cultivation/` + `server/src/schema/`：

新增 11 模块：
- ✅ **contamination.rs** — ContaminationTick：10:15 排异、qi 耗尽对已通经脉上 Backfire 裂痕、全毁+残留污染 → `CultivationDeathCause::ContaminationOverflow`
- ✅ **overload.rs** — OverloadDetectionTick：`throughput_current / flow_rate > 1.5` 生成 Overload 裂痕 + 扩 `qi_max_frozen`（上限 0.5·qi_max）
- ✅ **heal.rs** — MeridianHealTick：zone 正灵气驱动 `healing_progress`，愈合后 integrity +0.02/条
- ✅ **negative_zone.rs** — NegativeZoneSiphonTick：负灵域 siphon = |zone|·qi_max·0.001，qi 吸干 → `NegativeZoneDrain` 致死触发
- ✅ **death_hooks.rs** — `CultivationDeathTrigger` (5 cause) / `PlayerRevived` / `PlayerTerminated`；`apply_revive_penalty` 境界-1、qi=0、composure=0.3、contam 清空、LIFO 关脉到对应境界
- ✅ **tribulation.rs** — 化虚渡劫状态机：`InitiateXuhuaTribulation` → `TribulationState` Component → 波次 `TribulationWaveCleared` → 全清 = Void；`TribulationFailed` → 致死
- ✅ **life_record.rs** — `LifeRecord` Component + `BiographyEntry` (10 variants) + `TakenInsight`；全量保留，`recent_summary(n)` 取尾
- ✅ **karma.rs** — `karma_decay_tick`：~1 单位/游戏日衰减
- ✅ **insight.rs** — 7 类 `InsightCategory` + 19 变体 `InsightEffect` 白名单 + `InsightQuota` (per-realm cap 1–6) + `validate_offer` Arbiter (SingleCap/CumulativeCap/QuotaExhausted)
- ✅ **insight_fallback.rs** — 所有 8 类 trigger 的静态 ≥3 选项池，全部通过 Arbiter
- ✅ **insight_apply.rs** — `apply_choice` 覆盖所有 19 变体 → Cultivation/MeridianSystem/QiColor/`UnlockedPerceptions`/`InsightModifiers` + LifeRecord 双写

扩展：
- ✅ **components.rs** — 新增 `MeridianCrack` (severity/healing/cause) + `CrackCause` (Overload/Attack/Backfire/ForgeFailure) + `ContamSource` + `Contamination` Component；`Meridian::{rate,capacity}_for_tier` plan §3.3.2 曲线；`Meridian.opened_at` 真实 tick 时戳
- ✅ **breakthrough.rs** — 重写为 5 阶升境（Awaken→…→Spirit，Spirit→Void 返回 `RequiresTribulation`），`compute_success_rate` plan §3.1 完整公式（integrity × composure × completeness × (1+bonus)，material_bonus clamp 0.30），`XorshiftRoll` 可测；严重失败 ≥0.7 → `BreakthroughBackfire` 致死
- ✅ **schema/cultivation.rs** — `CultivationSnapshotV1` / `LifeRecordSnapshotV1` / `InsightRequestV1` / `QiColorStateV1` / `InsightOfferV1` / `InsightChoiceV1` / `BreakthroughEventV1` / `ForgeEventV1` / `BiographyEntryV1` / `CultivationDeathV1` + `realm/meridian_id/color_kind_to_string` 辅助；serde round-trip 有测
- ✅ **schema/channels.rs** — 新增 5 channel 常量（`insight_request/offer`、`breakthrough_event`、`forge_event`、`cultivation_death`）
- ✅ **mod.rs** — 注册 15 个 Event + 17 系统依赖序；新加入 Client 自动 attach `Contamination/LifeRecord/InsightQuota/UnlockedPerceptions/InsightModifiers`

**跨仓库 TODO（未在本仓修炼模块覆盖）**：
- ✅ TS schema 镜像：`agent/packages/schema/src/{cultivation,insight-request,insight-offer,breakthrough-event,forge-event,biography,cultivation-death}.ts` + generated JSON artifacts + channels + PlayerProfile.cultivation/life_record
- ✅ Agent LLM runtime：`agent/packages/tiandao/src/insight-runtime.ts` 事件驱动订阅 `bong:insight_request` → 调 LLM (skills/insight.md 系统提示) → `applyInsightArbiter()` 白名单+magnitude cap 过滤 → 发布 `bong:insight_offer`；失败回退 `emptyOffer()` ("心未契机")；7 单元测试覆盖 contract/arbiter/fallback 路径；已接入 `main.ts` 与 tick runtime 并行
- ⏳ 战斗 plan 联调：消费 `CultivationDeathTrigger` / 发 `PlayerRevived`/`Terminated` / 写 `throughput_current` / 写 `Contamination` 条目 / 发 `TribulationWaveCleared`/`Failed`
- ✅ 客户端集成：经脉选择（复用 InspectScreen body-layer click）+ 突破/淬炼·流速/淬炼·容量/设为目标四按钮 + CustomPayload 出站 (`bong:client_request`) + `cultivation_detail` S2C 快照下发 + 服务端 `CustomPayloadEvent` 入站 handler（详见下文客户端/服务端章节）
- ✅ Redis 桥：outbound 5/5 channel 全部就位 — `breakthrough_event` / `forge_event` / `cultivation_death` / `insight_request`（`server/src/network/cultivation_bridge.rs`，以 Username 作为 character_id），以及 `insight_offer` inbound 订阅 + `RedisInbound::InsightOffer` 分发（当前仅 tracing 日志，待 agent 侧就绪后再落 InsightChosen）

---

### （历史）P1 服务端骨架已落地 — `server/src/cultivation/`:

- ✅ **components.rs** — `Realm` (6 境界) / `MeridianId` (20 条) / `Meridian` / `MeridianSystem` / `ColorKind` (10) / `QiColor` / `Cultivation` / `Karma`
- ✅ **topology.rs** — `MeridianTopology` Resource，标准子午流注循环 + 8 奇经接驳，对称双向邻接
- ✅ **tick.rs** — `QiRegenTick + ZoneQiDrainTick` 合并为零和实现（玩家 gain = zone 浓度等量扣减，`QI_PER_ZONE_UNIT=50`），含 `avg_integrity` / `qi_max_frozen` 修正
- ✅ **meridian_open.rs** — `MeridianOpenTick` + `MeridianTarget` Component，首脉特许 + 邻接校验 + `zone_qi ≥ 0.3` 阈值
- ✅ **breakthrough.rs** — Awaken→Induce 事务（qi ≥ 8 + 已开 ≥1 经 → qi_max ×2 + composure -0.1），Event 驱动
- ✅ **forging.rs** — rate / capacity 两轴独立锻造，tier 0→3（二次曲线 cost 4/16/36），integrity -0.02/次
- ✅ **composure.rs** — ComposureTick：心境按 `composure_recover_rate` 缓慢回升，封顶 1.0
- ✅ **qi_zero_decay.rs** — QiZeroDecayTick：qi ≤ 1% qi_max 持续 ≥ `DECAY_TRIGGER_TICKS`（默认 600 tick ≈ 30s demo 值）触发降境；LIFO `(tier_sum DESC, stable_ord DESC)` 封闭多余经脉；tier 保留、progress 清零
- ✅ **color.rs** — `QiColorEvolutionTick` + `PracticeLog` Component（60%/25%/15% 阈值判定 main/secondary/chaotic/hunyuan，带衰减）
- ✅ **mod.rs** — 5 个 Event 注册 + 8 系统按依赖序调度（qi_regen → meridian_open → breakthrough → forging，并行 composure/color/qi_zero_decay）
- ✅ **main.rs** — `cultivation::register(&mut app)` 接入主循环，新连入 Client 自动挂 Cultivation/MeridianSystem/QiColor/Karma/PracticeLog

**P1 剩余（客户端集成 — 需先扩展 schema §6）**：
- ✅ IPC schema 新增：`ClientRequestV1` 联合（`set_meridian_target` / `breakthrough_request` / `forge_request`）— `agent/packages/schema/src/client-request.ts` + `server/src/schema/client_request.rs`，双端 round-trip 测试齐备，4 份 JSON artifact 已产出
- ✅ WorldStateV1 扩展 `CultivationSnapshotV1` 下发（`PlayerProfile.cultivation` + `PlayerProfile.life_record` 可选字段；`collect_cultivation_snapshots` 读取 Cultivation/MeridianSystem/QiColor/LifeRecord → 每 tick 注入 world_state）
- ✅ 客户端侧消费已完成（client 仓库）：
  - 出站：`network/ClientRequestProtocol.java`（Gson 编码 + `MeridianChannel→MeridianId` 穷举映射）+ `network/ClientRequestSender.java`（注入式 Backend seam）+ 8 个 protocol/sender 单元测试
  - UI：`InspectScreen` 修炼 tab 加入 `[设为目标][突破][淬炼·流速][淬炼·容量]` 四个可点击 label；未选中经脉时三项进入灰态（`TAB_INACTIVE_COLOR`），通过 `BodyInspectComponent.addSelectionListener()` 订阅刷新
  - 入站：新增 `ServerDataPayloadV1::CultivationDetail` SoA 变体（20 条 opened/flow_rate/flow_capacity/integrity，≤1024 字节 budget 校验）+ server `network/cultivation_detail_emit.rs` 20-tick 节流 emitter + client `network/CultivationDetailHandler.java` 解析 → `MeridianStateStore.replace()`（整度→DamageLevel 离散化，!opened→blocked）+ 6 handler 测试 + router 注册更新
  - 服务端入站：`server/src/network/client_request_handler.rs` 读取 `valence::custom_payload::CustomPayloadEvent` 按 channel=`bong:client_request` 过滤 → serde 反序列化 `ClientRequestV1` → 分派 `MeridianTarget` Component / `BreakthroughRequest` Event / `ForgeRequest` Event（`v` 版本号校验 + `material_bonus=0.0` 占位 TODO）

**已落地（客户端 mock-only，未接入 server）**：

- ✅ **顿悟 UI 骨架**（P4 客户端部分）— `client/src/main/java/com/bong/client/insight/`
  - `InsightCategory` 7 类枚举 (A-G) + 类别色
  - `InsightChoice` / `InsightOfferViewModel` / `InsightDecision` records
  - `InsightOfferStore` volatile slot + listeners + dispatcher
  - `InsightOfferScreen` (owo-lib)：3 候选卡 + 倒计时 + ESC/拒绝
  - `MockInsightOfferData.firstInduceBreakthrough()` 调试数据，按 J 键注入
  - ✅ 顿悟决定回传通道已接入：`ClientRequestInsightDispatcher` 从 `InsightOfferStore` 快照解析 `choiceId→idx`，via `ClientRequestSender.sendInsightDecision` 发 `insight_decision` C2S；BongClient 启动替换 LOGGING
  - 单元测试覆盖 store / decision / viewmodel / 渲染 (13 用例)
- ✅ **经脉 inspect UI 升级到 12 正经 + 8 奇经**（P1 客户端 inspect 部分）— `client/src/main/java/com/bong/client/inventory/`
  - `MeridianChannel` 重构：12 正经 (LU/HT/PC/LI/SI/TE/SP/KI/LR/ST/BL/GB) + 8 奇经 (REN/DU/CHONG/DAI/YIN_WEI/YANG_WEI/YIN_QIAO/YANG_QIAO) + `Family` 大类
  - `BodyInspectComponent.drawMeridian` 重画：四肢三阴三阳对称分布、奇经含正中纵脉/带脉环行/维跷弧线
  - `meridianAnchor` / `isOverMeridian` 同步 20 锚点（命中改用锚点圆形区，避免遮挡）
  - `MockMeridianData` 全覆盖：心经微裂、小肠经撕裂、阴跷未通、冲脉储满、带脉腰伤等

**剩余 TODO**：
- ✅ 顿悟决定 C2S 回传通道：`ClientRequestV1::InsightDecision { trigger_id, choice_idx: Option<u32> }` 双端对齐；服务端 handler 发 `InsightChosen` Event；客户端 `ClientRequestInsightDispatcher` 从 offer 快照把 choiceId 解析为 idx（stale/未知 id 降级为 null=拒绝）；Rust+TS+Java 测试齐备
- ⏳ 战斗 plan 联调（见上）
- ⏳ 服务端 `BreakthroughRequest.material_bonus` 接入玩家背包派生（当前客户端请求固定 0.0 占位）
- 🟡 `CultivationDetail` 扩展：已接入 realm / open_progress / cracks_count / contamination_total（SoA 4 字段），客户端 handler 向前兼容可选字段，未打通经脉 `open_progress → healProgress`；仍待接入 UI 裂痕可视化 + dantians（server 尚无 Dantian Component）

---

## 0. 设计公理（不可违反）

1. **境界 = 经脉拓扑** — 不是数值等级，是身体真实的物理结构
2. **材料永远是辅助** — 突破必要条件是经脉数 + 事件，材料只加速/保险
3. **真元极度排他** — 异种真元入体 = 毒，需消耗自身真元中和（10:15 亏损）
4. **个性来自选择** — 经脉路径 / 真元染色 / 顿悟 三层叠加，零出生论
5. **天道冷漠** — 不奖善不罚恶，只执行规则；业力是诅咒不是债务
6. **修炼不裁决生死** — 致死条件由本 plan 检测并上报，但生死判定/重生/终结归档统一由战斗 plan 收口

---

## 1. 服务端数据模型（Bevy Components）

新增 `server/src/cultivation/` 模块，承载所有修炼相关状态。**Contamination/MeridianCrack 也是战斗 plan 的写入目标**——战斗端写入异种真元/过载裂痕，本计划负责其后续演化。物理伤口（Wounds）由战斗 plan 独立管辖，不在修炼范畴。

### 1.1 核心 Component

```rust
// server/src/cultivation/components.rs

#[derive(Component)]
struct Cultivation {
    realm: Realm,                       // 醒灵..化虚
    qi_current: f32,
    qi_max: f32,                        // 派生自 meridian.opened 总和
    qi_max_frozen: f32,                 // 过载临时冻结部分
    last_qi_zero_at: Option<Instant>,   // 真元归零起点（>10min 触发降境）
    composure: f32,                     // 心境 0-1，影响突破/抗心魔
    composure_recover_rate: f32,        // 静坐恢复速率
}

// 心境规则：
//   - 平时缓慢恢复（静坐/独处加速，战斗/受伤暂停）
//   - 大事件冲击会下降：濒死(-0.3)、重伤(-0.1)、目睹血腥(-0.05)
//   - 突破前心境越高，成功率越高（见 §3.1）
//   - 心境是当前状态属性，不是行为追踪器

enum Realm {
    Awaken,      // 醒灵 (1 正经)
    Induce,      // 引气 (3 正经)
    Condense,    // 凝脉 (6 正经)
    Solidify,    // 固元 (12 正经 + 内核)
    Spirit,      // 通灵 (奇经 4)
    Void,        // 化虚 (奇经 8)
}

#[derive(Component)]
struct MeridianSystem {
    regular: [Meridian; 12],            // 12 正经
    extraordinary: [Meridian; 8],       // 奇经八脉
}

struct Meridian {
    id: MeridianId,                      // 手太阴肺经...
    opened: bool,
    open_progress: f32,                  // 0-1，打通进度

    // —— 两条独立可锻造的属性（玩家通过淬炼升级，各自有用途）——
    flow_rate: f32,                      // 流速：单位时间允许的真元通过量
                                         //   - 战斗 plan 用作过载阈值 / 施法上限 / 爆发输出
                                         //   - 高流速 = 同时间能"喷"更多真元
    flow_capacity: f32,                  // 流量：经脉本身能承载/储存的真元总量
                                         //   - 贡献到 Cultivation.qi_max 的总池
                                         //   - 高流量 = 续航长 / 抗持续输出 / 单次蓄力上限大
    rate_tier: u8,                       // 已锻造档位（升级痕迹，影响下次淬炼成本）
    capacity_tier: u8,

    throughput_current: f32,             // 当前秒流量（战斗端实时写入）
    cracks: Vec<MeridianCrack>,
    integrity: f32,                      // 0-1，影响施法效率
}

// 经脉锻造（淬炼）：玩家主动行为，非自动成长
// - 不分"根基/主修/奇经"档次——每条经脉的 flow_rate / flow_capacity 完全由玩家投入决定
// - 升级消耗：材料 + 真元 + 时间 + 经脉 integrity（淬炼有失败导致 crack 的风险）
// - rate 与 capacity 独立锻造路径：
//     · 偏 rate → 爆发型修士（剑/雷/拳，瞬时输出高）
//     · 偏 capacity → 续航/法阵型修士（阵法/医道/炼丹，蓄力大池子）
//     · 双修 → 资源消耗远大于偏修
// 详细锻造事务 + 数值曲线：见 §3.3 经脉淬炼

struct MeridianCrack {
    severity: f32,                       // 0-1
    healing_progress: f32,
    cause: CrackCause,                   // 过载/被攻击/走火
    created_at: Instant,
}

// Wounds 由战斗 plan 定义和管理（物理躯体损伤不属修炼范畴）
// Contamination 状态由战斗 plan 写入；本计划负责后续排异 tick 演化
#[derive(Component)]
struct Contamination {
    entries: Vec<ContamSource>,
}

struct ContamSource {
    attacker_id: Entity,
    amount: f32,                         // 残留异种真元量
    qi_color: QiColor,                   // 异种真元的染色（影响排异成本）
    introduced_at: Instant,
}

#[derive(Component)]
struct QiColor {
    main: Option<(ColorKind, f32)>,      // (颜色, 强度 0-1)
    secondary: Option<(ColorKind, f32)>,
    is_chaotic: bool,                    // 三色及以上 → 杂色
    is_hunyuan: bool,                    // 混元色（均匀全修）
    practice_log: PracticeLog,           // 用于计算染色演化
}

enum ColorKind {
    Sharp,       // 锋锐 (剑修)
    Heavy,       // 沉重 (拳修)
    Mellow,      // 温润 (炼丹)
    Solid,       // 凝实 (炼器)
    Light,       // 飘逸 (御物)
    Intricate,   // 缜密 (阵法)
    Gentle,      // 平和 (医道)
    Insidious,   // 阴诡 (毒蛊)
    Violent,     // 暴烈 (雷法)
    Turbid,      // 浊乱 (魔功)
}

#[derive(Component)]
struct Karma {
    weight: f32,                         // 总业力
    sources: Vec<KarmaSource>,           // 来源记录（用于生平卷）
}

// LifeRecord 在本 plan 仅承载"修炼侧事件流"——突破/淬炼/打通/染色突变/顿悟选择
// 死亡/重生/运数/终结归档相关字段（death_count / fortune_remaining / final_snapshot）
// 由战斗 plan 扩展同一 Component（或独立 Lifecycle Component），本 plan 不感知
#[derive(Component)]
struct LifeRecord {
    character_id: Uuid,
    created_at: Instant,
    biography: Vec<BiographyEntry>,      // 不可篡改修炼事件流（全量保留）
    insights_taken: Vec<TakenInsight>,
    spirit_root_first: MeridianId,       // 首次自选的起手经脉
}
```

### 1.2 Resource（全局）

```rust
// 经脉拓扑邻接图（按中医正经实际走向硬编码）
#[derive(Resource)]
struct MeridianTopology {
    adjacency: HashMap<MeridianId, Vec<MeridianId>>,
}

// 染色配置（演化阈值；战斗加成由战斗 plan 定义）
#[derive(Resource)]
struct QiColorConfig { ... }

// 顿悟触发表
#[derive(Resource)]
struct InsightTriggerRegistry { ... }
```

---

## 2. Tick 管线（Bevy Systems）

按 worldview 物理规律推进状态，按优先级顺序：

```
[Pre]   ZoneEnvironmentTick      读 zone spirit_qi 浓度，注入到玩家上下文
[T1]    QiRegenTick (1Hz)        真元再生（静坐快速 / 站立微弱）
[T2]    QiZeroDecayTick          真元 ≤ 1% qi_max 持续 10min → 爆脉降境
[T3]    OverloadDetectionTick    检测瞬时流量超限 → 添加 cracks（流量来源由战斗端写入）
[T4]    ContaminationTick (1Hz)  排异：消耗自身真元中和 contam，10:15 亏损
[T5]    MeridianHealTick (0.1Hz) 经络裂痕修复（需馈赠区静坐）
[T6]    MeridianOpenTick (1Hz)   经脉打通进度推进
[T7]    QiColorEvolutionTick     根据 practice_log 演化 main/secondary 染色
[T8]    ComposureTick (0.1Hz)    心境恢复（受最近事件影响）
[T9]    KarmaDecayTick (每游戏日) 业力极慢衰减
[T10]   ZoneQiDrainTick (1Hz)    玩家修炼吸取 zone qi（零和守恒）
[T11]   NegativeZoneSiphonTick   负灵域反吸玩家真元（境界越高越快）
                                 真元/血肉耗尽 → 发布 CultivationDeathTrigger（战斗 plan 收口）
```

### 2.1 关键 tick 详细规则

#### QiRegenTick
```
if 玩家处于静坐状态 AND zone.spirit_qi > 0:
  delta = base_rate × zone.spirit_qi × meridian_avg_integrity × dt
  qi_current = min(qi_current + delta, qi_max - qi_max_frozen)
else:
  // 非静坐（移动/站立/其他动作）：极弱被动回气
  delta = base_rate × 0.1 × zone.spirit_qi × dt
  // 注：战斗等"消耗事件"由战斗 plan 直接扣减 qi_current，本 tick 不感知
```

#### QiZeroDecayTick（爆脉降境）
```
// 真元几近耗尽（被强行抽干 / 爆脉法 / 负灵域虹吸）后，
// 经脉因长期无真元濡养而萎缩，导致境界跌落。
if qi_current <= qi_max * 0.01:
  if last_qi_zero_at.is_none(): last_qi_zero_at = now
  if now - last_qi_zero_at >= 10min:
    realm = realm.previous()           // 跌一阶
    // "天塌下来高个顶着"：投入越多的经脉越显眼，先萎缩去顶
    // 排序键：(rate_tier + capacity_tier DESC, opened_at DESC)
    //   - 锻造越深的经脉真元活动越剧烈，空竭时损耗反噬最大
    //   - 同档锻造下，最后打通的先封（越靠近上阶突破越先退）
    //   - 保底护住未怎么淬炼的"原始"经脉，重新崛起的根基不灭
    while meridians.opened_count() > realm.required_meridians():
      pick = opened
        .sort_by((rate_tier + capacity_tier desc, opened_at desc))
        .first()
      pick.opened = false
      pick.open_progress = 0          // 不留半通状态，重练需重头
      pick.throughput_current = 0
      // 注：rate_tier / capacity_tier 本身不被清零——重新打通后锻造投入仍在
      //     这是降境惩罚的"温柔"之处：玩家失去的是境界，不是终身投入
```
    qi_max 重算
    last_qi_zero_at = None
    触发 narration: "经脉空竭，境界倒退"
    写入生平卷
else:
  last_qi_zero_at = None               // 回升即重置
```

#### OverloadDetectionTick
```
for each meridian:
  if throughput_current > throughput_max * 1.5:
    severity = (throughput_current / throughput_max - 1.0) × 0.3
    add MeridianCrack { severity, cause: Overload }
    qi_max_frozen += severity × 5.0
```

#### ContaminationTick
```
for each contam in Contamination.entries:
  排异速率 = 10 qi/sec / 玩家排异效率(医道+, 杂色-)
  自身消耗 = 排异量 × 1.5
  qi_current -= 自身消耗
  contam.amount -= 排异量
  if contam.amount <= 0: 移除
  if qi_current < 0: 经络受损，添加 crack
```

#### MeridianOpenTick
```
target = 玩家选择的下一条经脉（必须邻接已通经脉）
if zone.spirit_qi > 0.3:
  progress_delta = base_open_rate × zone.spirit_qi × (qi_current / qi_max) × dt
  target.open_progress += progress_delta
  qi_current -= progress_delta × cost_factor   // 打通本身耗真元
  if target.open_progress >= 1.0:
    target.opened = true
    qi_max += meridian_capacity_value
    触发 narration 事件
    检测是否满足下一境界突破必要条件
```

#### QiColorEvolutionTick
```
统计 practice_log 最近 N 小时的修习分布：
  - 各项修习比例
  - 任何项 > 60% → 该色为 main（强度由比例决定）
  - 第二项 > 25% → 该色为 secondary（强度同上）
  - 三项及以上 > 15% → is_chaotic = true，main/secondary 失效
  - 所有项均匀 < 25% → is_hunyuan = true
  
状态转换平滑：每 tick 渐变，不突变。
```

#### ZoneQiDrainTick（worldview §一 零和守恒）
```
for each cultivating player:
  drain = QiRegenTick.delta × zone_drain_factor
  zone.spirit_qi -= drain
  // 玩家修炼消耗的灵气 = zone 少掉的灵气，符合 worldview 公理
```

#### NegativeZoneSiphonTick（worldview §二 负灵域）
```
if player.zone.spirit_qi < 0:
  pressure_diff = -zone.spirit_qi
  // 境界越高（真元池越大），被抽吸越快
  siphon = pressure_diff × qi_max × siphon_factor × dt
  qi_current -= siphon
  if qi_current <= 0:
    // 真元耗尽后抽血肉
    health -= siphon × 0.5
    可能触发降境
```

---

## 3. 突破事务

完整事务由 `BreakthroughSystem` 处理。

### 3.1 流程

```
玩家输入：StartBreakthroughIntent { target_realm }

Server 检查必要条件：
  - 当前境界与 target 差 1 阶？
  - 经脉数达标？(参见 worldview §三 表)
  - 当前位置满足事件条件？(灵气浓度 / 灵眼 / 异变兽尸体...)

If 不满足 → 拒绝，返回 reason

If 满足：
  enter BreakthroughState (进入闭关，不可移动；战斗端检测此状态决定是否打断)
  推 chat_collector：BreakthroughStarted → calamity agent 可介入

After breakthrough_duration (3-5min):
  // 突破成功率仅依赖自身状态（无业力/染色契合干扰）
  success_rate = base_rate
               × meridian_integrity_avg          // 经脉完整度
               × composure                       // 心境
               × meridian_open_completeness      // 是否刚好达标 vs 已超额
  辅助材料 → success_rate × (1 + bonus)，封顶 +30%
  
  if 成功：
    realm = target_realm
    qi_max += realm_bonus
    触发 InsightRequest (FirstBreakthrough trigger)
    推 era agent：HighRealmBreakthrough（如果是固元+）
    生平卷记录
  
  if 失败：
    add cracks 到主修经脉
    qi_max_frozen += 失败惩罚
    如严重失败 → 走火入魔 → emit CultivationDeathTrigger { cause: BreakthroughBackfire }（生死由战斗 plan 裁定）
```

### 3.2 化虚渡劫特殊流程

```
通灵 → 化虚 是唯一无辅助、无条件减免的突破。

流程：
  - 玩家 InitiateXuhuaTribulation
  - 全服 broadcast："有人在渡虚劫"
  - calamity agent 强制介入，生成天劫脚本（多波次雷击/心魔/外敌）
  - 玩家在固定区域内扛过所有波次
  - 期间其他玩家可来观战 / 干预（杀渡劫者获天道 narration 关注）
  - 扛过 → 化虚境
  - 死 → emit CultivationDeathTrigger { cause: TribulationFailure, no_fortune: true }
        （战斗 plan 据此跳过运数判定直接终结）

注：天劫的具体伤害施加由战斗 plan 实现；本计划只负责状态机和触发。
```

### 3.3 经脉淬炼（锻造事务）

经脉的 `flow_rate` / `flow_capacity` 升级是玩家主动行为，不会被动成长。淬炼独立于打通——只对**已 opened** 的经脉生效。

#### 3.3.1 流程

```
玩家输入：ForgeMeridianIntent {
  meridian_id,
  axis: Rate | Capacity,        // 一次只能淬炼一个轴
  materials: Vec<ItemStack>,    // 投入的辅料（火属灵材偏 rate / 水属偏 capacity）
}

Server 检查：
  - meridian.opened == true？
  - meridian.integrity > 0.6？        // 受损经脉拒绝淬炼
  - qi_current >= forge_qi_cost？
  - 玩家在炼气炉/灵眼/特定地形（淬炼必须在合适环境）？

若满足：
  enter ForgingState (3-10min, 不可移动)
  qi_current -= forge_qi_cost
  消耗 materials

完成时：
  base_success = base_by_current_tier(target_tier)   // tier 越高基础成功率越低
  bonus_from_materials = 材料契合度 + 数量加成 (封顶 +30%)
  bonus_from_environment = 灵眼 +15% / 普通炉 +0%
  success_rate = base_success × (1 + bonus_from_materials + bonus_from_environment)

  if 成功：
    if axis == Rate:    rate_tier += 1; flow_rate = curve_rate(rate_tier)
    if axis == Capacity: capacity_tier += 1; flow_capacity = curve_cap(capacity_tier)
    重算 Cultivation.qi_max（capacity 升级才需要重算）
    触发 narration（高 tier 升级会推 mutation agent）

  if 失败（轻微）：
    materials 半数返还
    integrity -= 0.05
    add MeridianCrack { severity: 0.1, cause: ForgeFailure }

  if 失败（严重，高 tier 概率）：
    integrity -= 0.3
    add MeridianCrack { severity: 0.5+, cause: ForgeFailure }
    qi_max_frozen += 严重惩罚
    极端情况下经脉 opened = false（"淬炼炸炉"）
```

#### 3.3.2 数值曲线（初版，待平衡）

```
rate_tier:     0  1  2  3  4   5   6   7    8    9    10
flow_rate:     1  2  3  5  8   12  17  23   30   40   55     // 渐进非线性

capacity_tier: 0  1  2  3  4   5   6   7    8    9    10
flow_capacity: 1  2  3  4  6   9   13  18   25   35   50

forge_qi_cost(target_tier) = 50 × 1.6 ^ target_tier
forge_duration(target_tier) = (180 + 60 × target_tier) seconds
base_success(target_tier) = max(0.30, 0.95 - 0.06 × target_tier)
```

#### 3.3.3 双修代价

无机制限制玩家同时 forging rate 和 capacity，但：

- 资源成本叠加（一条经脉同时高 rate 高 capacity ≈ 两条偏修经脉的总成本）
- 高双修经脉对 `meridian_avg_integrity` 影响更大（淬炼累积痕迹）
- 突破事务的 `meridian_open_completeness` 不看 tier，故双修不直接帮突破——纯纯靠"同一条经脉两边都强"换战斗多面性

#### 3.3.4 与战斗 plan 的边界

- 本计划：定义 `ForgeMeridianIntent` 事务、tier 增长、失败惩罚、`flow_rate` / `flow_capacity` 数值曲线
- 战斗 plan：定义 `flow_rate` 如何转化为单次施法的 qi 投射上限、过载倍率、爆发伤害；`flow_capacity` 如何转化为持续战斗续航

---

## 4. 致死缘由的对外契约（Outbound Death Hooks）

死亡-重生本身归战斗 plan 收口（运数/概率/遗念/重生惩罚/终结归档全部在那边）。本计划只**上报修炼侧致死缘由**，让战斗 plan 据此调用统一死亡流程。

### 4.1 修炼侧产生的致死触发点

| 来源系统 | 触发条件 | 上报事件 payload |
|---------|--------|----------------|
| Breakthrough（§3.1） | 严重失败/走火入魔 | `{ cause: BreakthroughBackfire, realm, severity }` |
| Tribulation（§3.2） | 化虚渡劫扛不过任意波次 | `{ cause: TribulationFailure, wave, realm }` |
| QiZeroDecayTick（§2.1） | 爆脉降境后已无境可降 | `{ cause: MeridianCollapse }` |
| NegativeZoneSiphonTick（§2.1） | qi=0 后血肉抽干至 health≤0 | `{ cause: NegativeZoneDrain, zone }` |
| ContaminationTick（§2.1） | 排异不及导致经络全毁 + qi/health 双零 | `{ cause: ContaminationOverflow }` |

### 4.2 上报机制

```rust
// 修炼侧仅 emit Bevy event，由战斗 plan 的 DeathArbiter 系统消费
struct CultivationDeathTrigger {
    entity: Entity,
    cause: CultivationDeathCause,
    context: serde_json::Value,   // 附带触发时的修炼快照（境界/经脉/染色）
}
```

战斗 plan 收到后：合并战斗侧的 DeathEvent 通道 → 进入运数判定 → 调遗念 agent → 决定重生/终结。本 plan 不感知最终结果，但会在玩家 entity 被战斗 plan 复活/终结时正确响应（见 §4.3）。

### 4.3 重生时的修炼侧响应

战斗 plan 完成重生流程后 emit `PlayerRevived { entity, penalty: RebirthPenalty }`，本计划监听并应用修炼侧惩罚：

```
on PlayerRevived:
  cultivation.realm = realm.previous()        // 境界 -1（worldview §十二 重生惩罚）
  cultivation.qi_current = 0
  cultivation.composure = 0.3                 // 心境受创
  contamination.entries.clear()               // 排异清零
  meridians: 关闭最高 tier 经脉至匹配新境界（复用 §2.1 LIFO 规则）
  LifeRecord.biography.push(BiographyEntry::Rebirth { ... })
  触发 InsightRequest::PostRebirth（顿悟挂钩，由战斗 plan 决定是否调）
```

终结时本计划同样监听 `PlayerTerminated` 但不做处理，仅停止该 entity 的所有修炼 tick。

---

## 5. 顿悟系统

顿悟是修炼系统的"质变接口"——绝大多数成长是 tick 内的量变累积，顿悟则在特定瞬间让玩家做一次**永久且不可逆**的微调。设计要害：**给得到选择的仪式感，但拿不到破坏平衡的奖励**。

### 5.1 设计公理

1. **顿悟不能突破修炼公理** — 不能直接 +realm、+qi_max、跨体效果、洗白业力、撤销 crack
2. **顿悟是质变不是堆叠** — 同类型效果有上限，不可无限累积；单次效果幅度小（≤ 5-10%）
3. **顿悟有额度** — 每境界缓增（1/2/3/4/5/6），不是每次触发都给
4. **顿悟有性格** — 选项之间是"风格分歧"而非显然优劣，agent 写贴脸 flavor
5. **顿悟可以拒绝** — "心未契机" 是合法选择，不消耗额度
6. **顿悟即生平** — 选择即写 biography 不可逆

### 5.2 顿悟效果白名单（Arbiter 强约束）

agent 只能从以下 7 类生成选项，超出即被 arbiter 判废 fallback：

每条示例格式：**【效果】数值改动 ／ 【flavor】agent 生成的描述 ／ 【适用场景】**

#### A. 经脉类（单次 +5% / 同类累计 +20%）

> 触发例：刚打通手太阴肺经 → trigger `meridian_opened`

- **A1**：`Meridian[手太阴肺经].flow_rate *= 1.05`
  - flavor："肺经吐纳如松涛，气流过半增三分"
  - 选这个 = 偏爆发：这条经的瞬时输出更猛
- **A2**：`Meridian[手太阴肺经].open_progress 在再次淬炼时基础 +10%`（一次性）
  - flavor："你看清了这条经脉的脉络走向，下次淬炼省力三成"
  - 选这个 = 省锻造资源
- **A3**：`Meridian[手太阴肺经].overload_tolerance += 0.05`（过载阈值放宽 5%）
  - flavor："你能感觉到肺经在过载边缘的颤动，不再轻易裂开"
  - 选这个 = 战斗安全余量

#### B. 真元类（单次 +5% / 同类累计 +25%）

> 触发例：连续静坐 50h 累计 → trigger `practice_dedication_milestone`

- **B1**：`Cultivation.qi_regen_factor *= 1.05`
  - flavor："你的呼吸与天地灵气节奏更贴合"
  - 选这个 = 通用提升
- **B2**：`Contamination.排异效率[QiColor::Sharp] *= 1.05`
  - flavor："锋锐之气入体的瞬间，你已知如何拨开"
  - 选这个 = 专克剑修对手
- **B3**：`Cultivation.qi_max_frozen *= 0.95`（解封 5% 冻结上限）
  - flavor："过去过载的旧伤略有松动，真元池微微扩展"
  - 选这个 = 修复历史伤

#### C. 心境类（单次 +10% / 同类累计 +30%）

> 触发例：渡过一次走火 → trigger `breakthrough_failed_recovered`

- **C1**：`Cultivation.composure_recover_rate *= 1.10`
  - flavor："经此一遭，你心如止水的速度更快"
  - 选这个 = 万金油心境恢复
- **C2**：`composure_shock_discount[BreakthroughFailure] = 0.5`（突破失败心境下降减半）
  - flavor："你已不畏走火，再经一次，心已无澜"
  - 选这个 = 鼓励反复冲关
- **C3**：`composure_immune_during[BreakthroughState] = true`（突破期间心境不下降）
  - flavor："闭关时外界纷扰再不能扰你"
  - 选这个 = 稀有但强力，下次突破基线提升

#### D. 染色类（单次 ±5% / 同类累计 +15%）

> 触发例：杂色态稳定 100h 后转混元的瞬间 → trigger `chaotic_to_hunyuan_pivot`

- **D1**：`QiColor.color_max[ColorKind::Sharp] += 0.05`（锋锐色强度上限提升）
  - flavor："你的剑意更纯，再练剑可达更深之境"
  - 选这个 = 专精剑修
- **D2**：`QiColor.chaotic_tolerance += 0.05`（杂色判定阈值放宽，可以多修一项不变杂色）
  - flavor："你能在多修之间保持本心，不易混乱"
  - 选这个 = 保留转向自由
- **D3**：`QiColor.hunyuan_threshold *= 0.95`（达成混元色更易）
  - flavor："你已窥见万法归一的门径"
  - 选这个 = 走全能流

#### E. 突破类（每境界各 1 次上限）

> 触发例：首次成功突破到引气 → trigger `first_breakthrough_to_Induce`

- **E1**：`next_breakthrough_success_rate += 0.05`（一次性，下次突破生效）
  - flavor："你已知冲关时神识凝聚的诀窍，下次心会更稳"
  - 选这个 = 保下一关
- **E2**：`breakthrough_event_condition[Condense] -= 1`（凝脉境突破事件条件减一项）
  - flavor："你已通晓凝脉之法，无需再寻三象齐聚"
  - 选这个 = 长期减负，针对未来某境界
- **E3**：`tribulation_prediction_window = 1`（渡劫前可预知第一波天劫类型）
  - flavor："你能在劫云聚拢前，听见第一道雷的脉搏"
  - 选这个 = 化虚渡劫保险

#### F. 流派类（单项一次性，无累计上限）

> 触发例：单经脉 rate+capacity 总 tier 达 10 → trigger `meridian_forge_tier_milestone`

- **F1**：`forge_cost[同一经脉双修] *= 0.85`（同经脉 rate+capacity 双修折扣 15%）
  - flavor："你看懂了双修的损耗节律，材料能省下一截"
  - 选这个 = 锁双修流派
- **F2**：`affinity[QiColor::Solid][天材::寒铁] += 0.10`（凝实色对寒铁亲和）
  - flavor："你的真元与寒铁共鸣，淬炼时事半功倍"
  - 选这个 = 锁炼器流派
- **F3**：`unlock_practice[同体三色调和]`（解锁高阶练法）
  - flavor："你领悟了三色相济的法门——杂色不再是诅咒"
  - 选这个 = 走杂修但保护性强（与 D2 协同）

#### G. 感知类（解锁信息，不可重复同项）

> 触发例：负灵域 qi=0 存活 3 分钟 → trigger `survived_negative_zone`

- **G1**：`unlock_perception[zone_qi_density]`（inspect UI 多一行：方圆 100m 灵气浓度热图）
  - flavor："你能感知方圆百米灵气浓淡，再不会盲目静坐于枯地"
  - 选这个 = 战略侦察
- **G2**：`unlock_perception[meridian_crack_detail]`（裂痕从模糊条变成具体严重度+成因+预计愈合时间）
  - flavor："你能听见经脉里每一道裂纹的呜咽"
  - 选这个 = 自我诊断
- **G3**：`unlock_perception[tribulation_first_wave_preview]`（化虚时，第一波天劫提前 5s 显示类型）
  - flavor："劫云中的第一道雷形已在你识海预演"
  - 选这个 = 化虚保险（与 E3 区别：E3 是预知，这是 5s 预警可走位）

**显式禁止**：
- 直接修改 realm / qi_max / meridians.opened
- 跨体效果（治他人/给他人 buff）
- 业力相关（增减 Karma.weight）
- 撤销已有 MeridianCrack（疗愈走 MeridianHealTick）
- 复活 / 重生豁免（属战斗 plan）
- 资源直接给予（材料/物品）

### 5.3 顿悟额度（按境界）

```
realm_insight_quota = {
  Awaken:    1,
  Induce:    2,
  Condense:  3,
  Solidify:  4,
  Spirit:    5,
  Void:      6,
}
// 总上限 21 次。每次境界突破刷新当前境界额度（已用 = 0）
// 多余的触发条件如果没有额度，agent 不被调用，玩家收到 "心有所感但未及悟" 提示
```

### 5.4 触发点

| 系统 | 触发 ID | 频率 |
|------|--------|------|
| Breakthrough | `first_breakthrough_to_<realm>` | 每境界首次突破成功 |
| Breakthrough | `breakthrough_failed_recovered` | 突破失败后未死亡，缓 1 小时给一次 |
| Forging | `meridian_forge_tier_milestone` | 单经脉 rate+capacity 总 tier 达 5/10/15 |
| Tribulation | `first_tribulation_survived` | 首次扛过任意天劫波次 |
| Tribulation | `witnessed_xuhua_tribulation` | 围观他人化虚渡劫（成败均触发） |
| Zone | `survived_negative_zone` | 在负灵域 qi=0 状态下存活超 3 分钟 |
| Technique | `practice_dedication_milestone` | 单一染色累计修习 50/200/1000 小时 |
| Technique | `chaotic_to_hunyuan_pivot` | 杂色态稳定持有 100h 后转混元色瞬间 |
| Combat（战斗 plan 触发本 plan agent） | `killed_higher_realm` / `killed_by_higher_realm_survived` | 战斗 plan 调用 |
| PostRebirth（战斗 plan 通知） | `post_rebirth_clarity` | 重生后 30s 内 |

### 5.5 流程

```
Server (修炼 tick 或 战斗 plan): 触发条件满足
  ↓
检查：玩家剩余顿悟额度 > 0？
  no → 推送 "心有所感而悟未至" narration，结束
  yes →
PUBLISH bong:insight_request {
  trigger_id, character_id, realm, qi_color_state,
  recent_biography (最近 N 条修炼事件),
  composure, available_categories (按额度过滤),
  global_caps (每类剩余可叠加额度)
}
  ↓
Insight Agent (独立 runtime, 模型可选 mini):
  - 读 prompt skills/insight.md
  - 工具：query-biography（取更多上下文）
  - 输出：2-3 个选项 { category, effect, magnitude, flavor_text, narrator_voice }
  ↓
Server Arbiter (validateInsightOffer):
  1. category 在白名单
  2. magnitude ≤ 单次上限 AND (existing + magnitude) ≤ 累计上限
  3. effect 引用的 meridian_id / color 等存在
  4. 选项之间互斥（没有"显然劣项"——粗略检查 flavor 长度均衡）
  - 全过 → forward to client
  - 任一不过 → fallback 到预定义池中按 trigger_id 取 2-3 个静态选项
  ↓
Client InsightOfferUI:
  - 显示选项卡片（标题 / 数值 / flavor / 副作用提示）
  - 60s 倒计时
  - 第四个按钮："心未契机"（拒绝，不消耗额度）
  ↓
Player choice / timeout:
  - 选择某项 → InsightChosen { trigger_id, choice_idx }（一个 trigger 对应一份 PendingInsightOffer，trigger_id 即 offer 的唯一标识）
  - 拒绝/超时 → InsightDeclined { trigger_id }
  - 服务端消费时**必须校验** `ev.trigger_id == pending.trigger_id`，否则丢弃（防止 stale 客户端请求覆盖新 offer）
  ↓
Server 应用：
  - 修改对应 component (e.g. Cultivation.composure_recover_rate += 0.10)
  - LifeRecord.insights_taken.push(TakenInsight {
      trigger_id, choice, magnitude, flavor, taken_at, realm_at_time
    })
  - LifeRecord.biography.push(BiographyEntry::Insight { ... })
  - realm_insight_quota_used += 1
  - 推 narration 全场广播（仅 high-magnitude 类 E/F）
```

### 5.6 Fallback 静态池

每个 trigger_id 必须有 ≥ 3 条静态选项（保证 agent 失败时 UI 仍可玩）。静态选项数值取上限的 60%，作为"保底但不强"的兜底。位置：`server/src/cultivation/insight_fallback.rs`。

### 5.7 与战斗 plan 的边界

- 本计划：定义白名单、额度规则、Arbiter 校验、效果应用到 Cultivation/MeridianSystem/QiColor/LifeRecord
- 战斗 plan：`killed_higher_realm` / `post_rebirth_clarity` 等战斗触发的 trigger_id 由战斗 plan emit 后转发给本 plan 的 insight runtime；选项中如涉及战斗加成（流派类 F 的某些子项），由战斗 plan 注册自己的 sub-whitelist 给本 plan 的 Arbiter 引用

---

---

## 6. IPC Schema 扩展（修炼相关）

新增 `agent/packages/schema/src/`:

### 6.1 新增 Channels

```typescript
// channels.ts
export const CHANNEL_INSIGHT_REQUEST = "bong:insight_request";
export const CHANNEL_INSIGHT_OFFER = "bong:insight_offer";
export const CHANNEL_BREAKTHROUGH_EVENT = "bong:breakthrough_event";
export const CHANNEL_FORGE_EVENT = "bong:forge_event";
// CHANNEL_DEATH_EVENT / CHANNEL_COMBAT_EVENT 由战斗 plan 定义
```

### 6.2 新增 Schema 文件

```
agent/packages/schema/src/
├── insight-request.ts
├── insight-offer.ts
├── breakthrough-event.ts
├── forge-event.ts
└── biography.ts          # 修炼侧 LifeRecord 事件流（不含死亡终结快照）
# death-event.ts / 终结归档结构由战斗 plan 定义
```

### 6.3 WorldStateV1 扩展

```typescript
WorldStateV1.players[].cultivation: {
  realm, qi_current, qi_max, qi_max_frozen,
  meridians_opened: number, meridians_total: number,
  qi_color_main, qi_color_secondary,
  composure: number,
}
WorldStateV1.players[].life_record: {
  recent_biography_summary: string,         // 最近若干修炼事件摘要
  // death_count / fortune_remaining 由战斗 plan 在同一 player 对象下扩展
}
```

---

## 7. 客户端集成

复用现有 inspect UI 骨架（`client/src/main/java/com/bong/client/inventory/`），仅扩展数据绑定与新增覆盖层：

| 现有 UI | 新增数据展示 |
|---------|------------|
| 经络层 | 颜色显示 integrity；裂痕显示 cracks；流动动效显示 throughput |
| 真元条 | qi_current / qi_max，frozen 部分灰色 |
| 染色显示 | inspect 顶部新增"真元色"标识（圆形色块 + 强度环）|
| 心境指示 | inspect 角落小图标 |
| 经脉路径选择 UI | 醒灵首次/打通新经脉时弹出邻接选择对话框 |
| 突破 UI | StartBreakthroughIntent 触发，闭关动画 + 进度条 + 干扰检测 |
| 顿悟 UI | InsightOffer 弹窗，2-3 选项卡片 |
| 经脉淬炼 UI | ForgeMeridianIntent 选轴/选材/进度条 |

战斗操作 UI（攻击/防御/武器切换）、现有 inspect 的**伤口层数据绑定**、**死亡/遗念/重生 UI** 均由战斗 plan 单独规划。

---

## 8. 阶段化实施路线

按可独立验证的最小切片划分。**不包含战斗端实施**——战斗 plan 自带 P 阶段。

### P1：修炼基础循环
**验证标准**：玩家可静坐回气 → 选经脉 → 慢慢打通 → 满足条件后突破到引气

```
✓ Cultivation + MeridianSystem + QiColor + Karma Component
✓ MeridianTopology resource + 拓扑邻接逻辑（按中医正经走向硬编码）
✓ QiRegenTick + ZoneQiDrainTick + MeridianOpenTick + QiColorEvolutionTick
✓ ComposureTick + QiZeroDecayTick
✓ 简单突破事务（醒灵→引气）
✓ Client: inspect UI 显示真元/经络/染色/心境
✓ Client: 经脉路径选择对话框
✓ 染色记录（练剑/拳/丹...）—— 仅状态演化，战斗效果由战斗 plan 提供
✓ 经脉淬炼基础事务（rate/capacity 升级，到 tier 3 验证流程）
```

### P2：受伤、污染、疗愈（与战斗 plan 联动；本阶段先用 mock）
**验证标准**：mock 添加 contam/crack → 系统正确处理排异/疗愈/裂痕修复

```
✓ Contamination Component
✓ ContaminationTick + MeridianHealTick + OverloadDetectionTick
✓ 调试命令：/contam add, /crack add
✓ Client: 裂痕/排异进度实时刷新
✓ 自疗机制（静坐 + 馈赠区，仅经络/排异维度）
✓ NegativeZoneSiphonTick（负灵域抽吸）
```

### P3：生平记录 + 死亡对外契约
**验证标准**：mock 走火/爆脉/抽干 → 正确 emit `CultivationDeathTrigger`；mock `PlayerRevived` 事件 → 修炼侧惩罚正确应用

```
✓ LifeRecord Component + Biography 事件流（修炼事件全量保留）
✓ 突破/淬炼/打通/染色突变/顿悟选择 → 写入 biography
✓ CultivationDeathTrigger Bevy event 定义 + 各 tick 触发点接入
✓ on PlayerRevived 监听器：境界-1、qi=0、心境受创、contam 清空、LIFO 关脉
✓ on PlayerTerminated 监听器：停止该 entity 所有修炼 tick
✓ Schema: breakthrough-event.ts / forge-event.ts / biography.ts
注：DeathEvent 主流程 / 运数概率 / 遗念 agent / 终结归档 → 战斗 plan 同步推进
```

### P4：顿悟系统
**验证标准**：完成 P1-P3 任意触发条件 → 顿悟 agent 生成选项 → arbiter 校验通过 → 玩家选择生效 → biography 记录

```
✓ InsightRequest/Offer schema（含 trigger_id / categories / global_caps）
✓ realm_insight_quota 资源 + 触发点 quota 检查
✓ Insight Agent (新 runtime，事件驱动，独立模型路由)
✓ skills/insight.md prompt（含 7 类白名单 + flavor 风格规范）
✓ Arbiter validateInsightOffer（白名单 + 数值上限 + 累计上限 + 引用合法性）
✓ Fallback 静态选项池 (insight_fallback.rs，每个 trigger ≥ 3 选项)
✓ insight_apply.rs：7 类效果到 Cultivation/MeridianSystem/QiColor 的具体应用
✓ LifeRecord.insights_taken + biography 记录
✓ 拒绝/超时路径（"心未契机"不消耗额度）
✓ Client: 顿悟选择 UI（4 按钮：3 选项 + 1 拒绝；60s 倒计时）
✓ 高 magnitude 类（E/F）选择后全场 narration
```

### P5：高境界与天劫
**验证标准**：玩家可至化虚（依赖战斗 plan 提供天劫伤害实施）

```
✓ 中后期突破事务（凝脉/固元/通灵/化虚）
✓ 灵眼随机刷新（mutation agent 控制）
✓ 化虚渡劫专属流程 + 全服广播
✓ 与战斗 plan 联动：天劫脚本由 calamity agent 生成，战斗 plan 实施伤害
```

---

## 9. 测试策略

### 单元测试
- Bevy systems：每个 tick 输入/输出快照断言
- 排异公式 / 淬炼成功率 / 染色演化 边界测试
- 拓扑邻接：合法/非法选择路径
- LIFO 降境：高 tier 经脉先封闭排序正确

### 集成测试
- 完整突破事务（成功/失败/走火 → emit CultivationDeathTrigger）
- 经脉淬炼成功/失败/炸炉 链路
- mock PlayerRevived → 修炼侧惩罚一致性
- 顿悟生成 → arbiter 校验 → 应用
- 染色演化：长期日志驱动主色变化

### E2E 测试
- mock 玩家从醒灵突破到引气
- mock 玩家走火 → 验证 CultivationDeathTrigger 上报正确
- mock 玩家淬炼某经脉 rate tier 0→3

### 平衡测试（手动）
- 单玩家从醒灵走到化虚记录耗时（目标 ~50h+）
- 染色演化稳定性
- 单条经脉淬炼至 tier 5 的资源消耗合理性

---

## 10. 文件规划

```
server/src/cultivation/
├── mod.rs
├── components.rs           # 所有 Component 定义
├── topology.rs             # MeridianTopology 邻接图
├── qi_systems.rs           # QiRegen/ZeroDecay/Overload tick
├── meridian_systems.rs     # MeridianOpen/Heal tick
├── contamination.rs        # ContaminationTick + 排异逻辑
├── color.rs                # QiColor 演化逻辑
├── composure.rs            # 心境 tick
├── breakthrough.rs         # BreakthroughSystem
├── forging.rs              # 经脉淬炼事务 (ForgeMeridianIntent)
├── death_hooks.rs          # CultivationDeathTrigger emit + PlayerRevived 监听
├── insight_trigger.rs      # 顿悟触发点收集
├── insight_apply.rs        # InsightChosen 应用到 Cultivation/QiColor/LifeRecord
├── insight_fallback.rs     # 静态选项池（agent 失败兜底）
├── life_record.rs          # 修炼侧 biography 记录
├── karma.rs                # 业力累积 + 衰减
├── zone_drain.rs           # 修炼吸 zone qi
└── negative_zone.rs        # 负灵域抽吸

# server/src/combat/  ← 由战斗 plan 负责，不在本计划范围

server/src/network/
└── (扩展) redis_bridge.rs  # 新增 cultivation channels 发布

agent/packages/schema/src/
├── insight-request.ts
├── insight-offer.ts
├── breakthrough-event.ts
├── forge-event.ts
└── biography.ts            # 修炼侧 biography（不含死亡终结快照）
# death-event.ts 由战斗 plan 维护

agent/packages/tiandao/src/
├── insight-runtime.ts      # 事件驱动 agent runtime（订阅 insight_request）
├── skills/insight.md       # 顿悟 prompt（含白名单引用 + flavor 风格指南）
├── tools/query-biography.ts  # 工具：查玩家修炼生平
└── (扩展) arbiter.ts        # validateInsightOffer

client/src/main/java/com/bong/client/
└── cultivation/            # 新模块（扩展现有 inventory/inspect 实现）
    ├── inspect/            # 在现有经脉 UI 上加染色、心境、裂痕图层
    ├── breakthrough/       # 突破/闭关 UI
    ├── forge/              # 经脉淬炼 UI
    ├── insight/            # 顿悟选择 UI
    └── path-select/        # 经脉路径选择
# death/ UI（死亡/遗念/重生确认）由战斗 plan 负责
```

---

## 11. 已决策事项

1. **经脉拓扑邻接图** ✅ 按中医正经实际走向落到代码（参考标准经络循行图，硬编码到 `topology.rs`）
2. **突破成功率公式** ✅ **仅依赖自身状态**：
   ```
   success_rate = base × 经脉完整度 × 心境 × 经脉达标度
   辅助材料 +30% 封顶
   ```
   不掺业力、不掺染色契合度——突破是修士与自己的较量。
3. **死亡-重生 — 不在本计划范围** ✅
   - 修炼侧只 emit `CultivationDeathTrigger` + 监听 `PlayerRevived` 应用惩罚
   - 运数/概率/遗念/终结归档/亡者博物馆全部由战斗 plan + library-web 负责
4. **生平卷（修炼侧部分）** ✅ **全量保留**修炼事件，不做 sliding window
   - biography 只记录修炼相关事件（突破/淬炼/打通/染色/顿悟）
   - 死亡终结快照由战斗 plan 在同一 character_id 下扩展并归档
5. **客户端 inspect UI** ✅ **复用现有经脉实现**
   - 仅扩展数据绑定 + 新增覆盖层
6. **经脉无类型预设，强度由玩家锻造决定** ✅
   - `flow_rate` / `flow_capacity` 二维独立可升级，无"根基/主修/奇经"硬分档
   - 流派分化（爆发型/续航型）由玩家锻造取向自然涌现，非系统强加
   - 降境 LIFO 排序键 = `(rate_tier + capacity_tier, opened_at)`，tier 不清零保护终身投入
7. **染色状态演化在本计划，染色战斗加成在战斗 plan** ✅
   - 本计划：QiColorEvolutionTick 维护 main/secondary/混元/杂色状态
   - 战斗 plan：定义每种染色的攻防加成数值（PvP 平衡责任在战斗 plan）

### 仍需在实施中确定（非阻塞）

- 心境恢复速率与受冲击下降具体值（P1 实施时初版 + 后期调）
- 突破 base_rate 各境界初值（P1/P5 实施时定）
- 染色演化的具体阈值（60% main / 25% secondary / 15% chaotic）需实测调
- ZoneQiDrainTick 的 drain_factor 需测试不会让 zone 太快枯竭
- NegativeZoneSiphonTick 的 siphon_factor 需测试不同境界的存活时间

---

## 12. 与现有计划的关系

- **plan-server.md**：本计划是其下修炼模块的细化分支
- **plan-combat-v1.md**（待写）：本计划提供修炼状态模型；战斗 plan 自带 Wounds Component、收口 DeathEvent 全流程（运数/概率/遗念/重生/终结归档），并向本计划的 Contamination/MeridianCrack 写入数据；本计划通过 `CultivationDeathTrigger` 上报修炼侧致死缘由，监听 `PlayerRevived` 应用境界惩罚
- **plan-agent-v2.md**：本计划新增的 insight agent 复用所有基础设施（telemetry/persistence/arbiter）
- **plan-client.md**：本计划要求 client 扩展 inspect UI
- **plan-worldgen-v3.1**：依赖 zone spirit_qi 数据（已有）

---

## 13. 验收里程碑

```
M2.1 — 修炼可跑：P1 完成，玩家能从醒灵走到引气
M2.2 — 受伤可愈：P2 完成，伤口/污染/裂痕的恢复机制工作
M2.3 — 生平可溯：P3 完成，修炼事件全量入 biography，CultivationDeathTrigger / PlayerRevived 契约通过
M2.4 — 个性可见：P4 完成，顿悟 agent 接入
M2.5 — 修炼成型：P5 完成，全境界突破 + 化虚渡劫（与战斗 plan 联合验收）
```

完成 M2.5 + 战斗 plan 对应里程碑后，末法残土修炼系统进入 **Beta** 阶段。
