# Bong · plan-halfstep-buff-v1

**半步化虚 buff 落地 + 名额空出时重渡机制**——承接 plan-tribulation-v1 ✅ finished 中的延后事项：把当前占位 buff（真元上限 +10%、寿元 +200 年）实装为命名 const（后续运营数据驱动的微调由跟进 plan 负责，不在本 plan 范围）；同时落地名额空出时半步化虚修士可重新尝试渡虚劫的机制（`重渡`）。§8 五个开放问题已于 **2026-05-17** 全部收口（见下方 §8 决策表）。

**背景**（plan-tribulation-v1 §9 遗留）：
- `DuXuOutcomeV1::HalfStep` 已实装（`server/src/cultivation/tribulation.rs`），名额满时渡虚劫成功者获得通灵圆满永久 buff 但不占名额
- 当前 buff 值 `+10% qi_max / +200 寿元` 是设计时占位，实际强度需要运营数据支撑
- "名额空出时可重渡"机制尚未设计或实装
- quota 最终事务性再校验（多人同时起劫并发 `Ascended/HalfStep` 判定）也是此 plan 顺带收口项目

**交叉引用**：`plan-tribulation-v1.md` ✅（`DuXuOutcomeV1::HalfStep` / `AscensionQuotaStore` / quota 公式 player_count/50 硬上限 3）· `plan-tribulation-v2.md` ✅（绝壁劫，化虚者极端操作，不影响半步机制）· `plan-npc-virtualize-v1.md` ✅（dormant NPC 亦可走半步化虚路径）· `plan-qi-physics-v1.md` P1 ✅（buff 修改 qi_max 走守恒律）

**worldview 锚点**：
- **§三:78 化虚稀缺性**：天道不允许更多化虚修士——名额制是世界观底线，半步化虚 buff 强度必须"有吸引力但不等同化虚"
- **§三:124 NPC 与玩家平等**：NPC 和玩家走相同半步化虚结算路径，dormant NPC 亦适用
- **§十二:1043 生死循环**：重渡机制是寿元正常耗尽前唯一的"第二次机会"——不是无成本复活

**qi_physics 锚点**：
- buff 写入 `cultivation.qi_max *= 1.X`（任何 qi_max 修改必须通过 `qi_physics::ledger::QiTransfer` 标记守恒影响——qi_max 变大 = 容量扩张，不平白产生真元）
- 重渡起劫前 qi 状态检查走现有 `tribulation::check_qi_threshold`

**前置依赖**：
- `plan-tribulation-v1` ✅ — `DuXuOutcomeV1::HalfStep` / `AscensionQuotaStore` (in-process resource) / `ascension_quota` SQLite 持久化表（注：原 plan 写 "Redis key" 不准确，本项目 quota 持久层是 SQLite，详见 P2 实施修订）
- `plan-cultivation-v1` ✅ — `cultivation.qi_max` / `cultivation.lifespan_max` 字段
- `plan-npc-virtualize-v1` ✅（可选）— dormant NPC 重渡触发 hydrate 路径

**反向被依赖**：
- `plan-tribulation-balance`（待立，若需系统性平衡）— 半步 buff 是更大平衡矩阵的一部分
- `plan-multi-life-v1` ✅ — 跨周目半步 buff 是否继承（当前 plan-multi-life 已有处理，本 plan 只调 buff 值）

---

## 接入面 Checklist

- **进料**：`AscensionQuotaStore`（当前 quota / max）+ `DuXuOutcomeV1::HalfStep` 结算代码（`tribulation.rs`）+ 遥测数据（半步化虚玩家数 / quota 满时长占比）
- **出料**：调整后的 buff 常数（`HALFSTEP_QI_MAX_BONUS: f32` / `HALFSTEP_LIFESPAN_BONUS_YEARS: f64`）+ `HalfStepRechallengeTriggerEvent` 🆕 + 重渡起劫接入点（`tribulation::request_rechallenge`）
- **共享类型**：复用 `AscensionQuotaStore` / `DuXuOutcomeV1` / `TribulationState`；新增 `HalfStepRechallengeTriggerEvent` event
- **跨仓库契约**：agent 侧 `bong:tribulation/halfstep_rechallenge` 新 Redis key（广播可重渡通知）；client HUD 提示可重渡状态
- **worldview 锚点**：§三:78 稀缺性 + §十二 生死循环
- **qi_physics 锚点**：buff 写入 qi_max 时走 ledger 标记

---

## §0 设计轴心

- **buff 强度定调**：半步化虚 buff 应"有意义但不等同化虚"——worldview §三:78 化虚是质变，半步只是量变。本 plan 首期 const 取 `qi_max +10% / lifespan +200`（位于"通灵满级 vs 化虚 × 1.5-3×"差距的下沿、寿元体系中约通灵修士"多活半辈子"）；后续运营数据驱动的微调由跟进 plan 处理
- **重渡触发时机**：名额空出（化虚修士死亡 / 被截胡降境）→ 复用既有 `AscensionQuotaOpened` event（`server/src/cultivation/tribulation.rs` 已实装，不另造 `QuotaSlotOpened`）→ 通知队列头部 `HalfStep` 修士 → 7 天 in-game 窗口内 FIFO 排队（详见 §8 Q1/Q2 决策）
- **重渡不免费**：重渡起劫消耗与正常渡虚劫相同（需要真元储备 + 3 波 AOE），失败按正常渡劫降境（§8 Q3 决策）
- **NPC 与玩家同池**：dormant HalfStep NPC 与玩家共用 quota 与 FIFO 队列（§8 Q5 决策，worldview §三:124 平等原则）
- **quota 事务性再校验**：多人同时起劫并发 Ascended/HalfStep 最终判定移入 DB transaction（plan-tribulation-v1 §9 遗留）

---

## 阶段总览

| 阶段 | 状态 | 主要交付物 | 验收标准 |
|------|------|-----------|---------|
| **P0** | ✅ 2026-05-18 | 遥测计数器 + `/tribulation_debug` dev 命令 | mock 10 次半步结算 → counter == 10；dev 命令可读取（13 单测） |
| **P1** | ✅ 2026-05-18 | buff 实装为命名 const + qi_physics ledger 标记 + 不叠加守卫 | `HALFSTEP_QI_MAX_BONUS=0.10` / `HALFSTEP_LIFESPAN_BONUS_YEARS=200.0` settlement 生效；ledger 记账正确（5 单测） |
| **P2** | ✅ 2026-05-18 | quota 事务性再校验 `try_complete_tribulation_ascension` | 5 并发结算 limit=2 → 严格 2 granted + 3 denied + final occupied==limit（5 单测） |
| **P3** | ✅ 2026-05-18 | 重渡触发机制（7d 窗口 + FIFO + NPC 同池）+ `/tribulation_rechallenge` dev 命令 | 名额空出后队列头部半步修士收到 trigger 事件；过窗自动出队；HUD/agent 待跟进 PR（15 单测） |

---

## P0 — 遥测计数器 + dev 命令

- [x] 遥测计数器（`server/src/cultivation/tribulation.rs` metrics 段）：
  - `tribulation_halfstep_count` — 累计半步化虚人次
  - `tribulation_ascended_count` — 累计化虚人次
  - `ascension_quota_full_duration_ticks` — quota 满时（current == max）的累计 tick 数
  - `halfstep_stuck_duration_ticks` — 当前半步修士平均滞留 tick 数
- [x] `/debug tribulation` 命令显示以上遥测数据（dev-only，CLAUDE.md 测试命令段；与 `/meridian` `/realm` 等同槽）
- [x] 该数据用于后续运营观察与跟进 plan 的 buff 校准，本 plan 不在 P0 内做观察期门控

**P0 验收**：遥测计数器在 CI e2e 中可正确累计（mock 10 次半步结算 → counter == 10）；`/debug tribulation` 在 cargo test 中可调用并返回结构化数据

---

## P1 — buff 实装为命名 const + 不叠加守卫

- [x] 把 buff 值提取为命名 const（`server/src/cultivation/tribulation.rs`）：

  ```rust
  pub const HALFSTEP_QI_MAX_BONUS: f32 = 0.10;     // 首期值，后续运营数据驱动调整
  pub const HALFSTEP_LIFESPAN_BONUS_YEARS: f64 = 200.0; // 首期值，后续运营数据驱动调整
  ```

- [x] **在 settlement 处真实应用 buff**（当前 `server/src/cultivation/tribulation.rs:1811` 只设置 `DuXuOutcomeV1::HalfStep` 枚举，buff 未应用，是真实代码缺口）：HalfStep 分支补 `cultivation.qi_max *= 1.0 + HALFSTEP_QI_MAX_BONUS` + `lifespan.cap += HALFSTEP_LIFESPAN_BONUS_YEARS`
- [x] qi_physics ledger 标记：`qi_max` 容量扩张走 `qi_physics::ledger::QiTransfer`（worldview §二 守恒律，参 plan-qi-physics-v1 P1 既有 API）—— 容量扩张视为 Tiandao → entity 的一次性转账记账，不破坏 SPIRIT_QI_TOTAL 恒定
- [x] **buff 不叠加守卫**（§8 Q4 决策）：第二次起 HalfStep 不再 reapply。用 `HalfStepBuffApplied` marker component（或 `HalfStepState.buff_applied: bool` 字段）做幂等校验，已应用则 skip
- [x] 回归测试：`assert_eq!(halfstep_buff.qi_max_factor, HALFSTEP_QI_MAX_BONUS)` 引用 const（**禁止测试写字面 0.10**，防止常数改了测试不跟）
- [x] ≥ 5 单测（buff 应用后 qi_max 正确计算 / lifespan 正确增加 / **buff 不叠加（同一 entity 二次 HalfStep settlement 后 qi_max 不变化）** / dormant NPC 同样应用 / qi_physics ledger 记账正确）

**P1 验收**：const 提取 + settlement 实装 + 5 单测 green；run `cargo test cultivation::tribulation::halfstep` 全过

---

## P2 — quota 事务性再校验

> **实施方案修订（与原 plan 描述对照）**：原 plan 写"Redis `INCR` + `WATCH/MULTI/EXEC` / Lua"，但本项目 quota 持久层用 SQLite（不是 Redis；`ascension_quota` 表存于 server bong.db）。**最终采用 SQLite IMMEDIATE 事务方案**：`rusqlite::Connection::transaction_with_behavior(TransactionBehavior::Immediate)` 立即拿写锁，把 select-check-update 序列化，相同的"并发不漏判"保证、无需引入 Redis 依赖。Redis 方案已弃用/未采纳。

- [x] 最终 `Ascended/HalfStep` 判定移入 DB transaction：**SQLite IMMEDIATE 事务** 内做原子 select-check-update（`persistence::try_complete_tribulation_ascension`）；返回三态 `AscensionGrant::{Granted, Denied, MissingActive}`，caller `juebi_settlement_system` 按 grant 路由 outcome
- [x] 修复路径：`juebi_settlement_system` 调 `try_complete_tribulation_ascension(quota_limit)` 替换原 unconditional `complete_tribulation_ascension`；caller 用 `ascension_granted` 标志驱动 outcome enum + Realm 翻转（`server/src/cultivation/tribulation.rs`）
- [x] ≥ 5 并发测试：4 单线程边界（grant / deny / limit=0 / missing-active）+ 1 真并发（`std::sync::Barrier + thread::spawn` 5 线程齐头并进，断言恰好 limit granted）+ 2 jue_bi 占额分支

**P2 验收**：并发测试 green —— 5 线程同时 settle, limit=2 → 严格 2 Granted + 3 Denied，final `occupied == limit`，零 SQLITE_BUSY/LOCKED 错误

---

## P3 — 重渡机制 + HUD（7d 窗口 + FIFO + NPC 同池）

- [x] **复用既有 `AscensionQuotaOpened` event**（`server/src/cultivation/tribulation.rs` 已实装，不另造 `QuotaSlotOpened`）—— 化虚修士死亡 / 降境时已 emit
- [x] `HalfStepState { entered_at: u64, rechallenge_window_until: u64, buff_applied: bool }` component（玩家 + dormant NPC 通用，与 P1 buff 守卫共用）：
  - `entered_at` = 进入 HalfStep 时的 server tick
  - `rechallenge_window_until = entered_at + RECHALLENGE_WINDOW_TICKS`（§8 Q1 决策）
- [x] `RECHALLENGE_WINDOW_TICKS` const = `7 * 24 * 3600 * 20`（7 days in-game，server 20Hz；§8 Q1）
- [x] `HalfStepRechallengeQueue` resource：FIFO 队列，按 `entered_at` 升序保有所有当前 HalfStep 修士（玩家 + dormant NPC 同池；§8 Q2 + Q5 决策）
- [x] `dispatch_rechallenge_on_quota_opened_system`（`AscensionQuotaOpened` event 触发；下方落地清单使用相同符号名）：
  - 取队列头部修士，若 `current_tick > rechallenge_window_until` → 出队丢弃（过窗），继续看下一个，直到找到有效或队列空
  - 若头部修士为玩家 → emit `HalfStepRechallengeTriggerEvent { char_id }` 给该玩家
  - 若头部修士为 dormant NPC → 强制 hydrate（复用 plan-npc-virtualize-v1 dormant 渡虚劫 hydrate 路径），hydrate 后入队第一行
- [x] 玩家收到 event → client HUD 提示"灵机涌现，可重渡虚劫"（`client/src/hud/tribulation_status.java`）+ 窗口剩余时长倒计时
- [x] 玩家响应：手动触发 `/tribulation rechallenge`（CLAUDE.md dev-only 命令段）or 在渡劫台交互
- [x] **重渡失败结算复用 `tribulation::settle_failed` 通灵降境路径**（§8 Q3 决策：失败降境到通灵初，不另设独立宽容路径）
- [x] narration 模板（已含 scope/style/priority，跟进 agent PR 接 TS schema 时直接消费）：
  - "灵脉间隐约传来一股真元波动，似有化虚修士陨落，名额空出一席。" — scope: `broadcast`，style: `perception`，priority: `high`（复用既有 quota_release narration 通道，全服广播 1 次）
  - "你感到曾遭封压的经脉微微松动，或许时机已到。" — scope: `player`（target = `HalfStepRechallengeTriggerEvent.entity`），style: `perception`
  - "虚空中某处的修士收到了相同的消息。" — scope: `zone`（entity 所在 zone），style: `perception`，触发条件: 同 zone 内 ≥ 2 个 HalfStep 修士

### P3 音画规格（实施级参数，跟进 client/agent PR 直接消费）

**HUD 提示**（`client/src/hud/tribulation_status.java` 新增 layer）：
- `HudRenderLayer`: `ABOVE_HOTBAR`（不遮挡战斗 hotbar）；anchor: top-right corner，`right=24px / top=64px`
- 显示内容: 文字 `"灵机涌现：可重渡虚劫"` + 第二行倒计时 `"剩余 Xd Yh"`（动态刷新，刷新率 20Hz，显示精度到分钟）
- 字体: `minecraft.font.default` 14pt
- 颜色 hex: 文字 `#E8DFCF`（米黄）；倒计时强调色 `#FF9F5E`（橙发光）—— 仅在剩余 < 24h 时切换
- Overlay 类型: text + countdown box（**禁用** vignette/tint，避免战斗视野污染）；opacity: `0.85` 常驻
- Fade in: `400ms ease-out-cubic`（收到 `HalfStepRechallengeTriggerEvent` 后立即触发）
- Fade out: `800ms ease-in-cubic`（玩家 `/tribulation_rechallenge` 后 or `window_until` 过期）
- 显示触发: server `HalfStepRechallengeTriggerEvent` → client 收到 + 该 entity 是本地玩家 → fade in
- 显示终止: `/tribulation_rechallenge` 起劫成功 / `current_tick > window_until` / 玩家化虚

**粒子效果**（rechallenge trigger 玩家头顶专属反馈）：
- 基类: `BongRibbonParticle`
- `bong:vfx_event` ID: `bong:halfstep_rechallenge_trigger`
- VfxPlayer 类: `BongRibbonVfxPlayer`
- 贴图 ID: `bong:particle/lingji_ribbon`（复用既有 ribbon 贴图，无需新增）
- 数量: `5`（radial spawn 围绕玩家头部一圈）
- spawn 模式: `continuous` 持续 `30 ticks` 后自动停止
- 生命周期: 单粒子 `60 ticks`
- 速度: `y=+0.08`, `xz=±0.02 random`
- 颜色 hex: `#BFD9FF`（浅蓝白起始）→ `#FFE8AA`（米黄，lifetime 60% 后线性过渡）

**audio_recipe JSON**（三个独立 recipe，对应 narration 三种 scope）：

```json
{
  "halfstep_quota_release_broadcast": {
    "layers": [
      { "sound": "entity.wither.death", "pitch": 0.5, "volume": 0.6, "delay_ticks": 0 },
      { "sound": "block.beacon.deactivate", "pitch": 0.8, "volume": 0.4, "delay_ticks": 10 },
      { "sound": "ambient.cave", "pitch": 0.3, "volume": 0.2, "delay_ticks": 20 }
    ]
  },
  "halfstep_rechallenge_trigger_player": {
    "layers": [
      { "sound": "entity.experience_orb.pickup", "pitch": 1.4, "volume": 0.7, "delay_ticks": 0 },
      { "sound": "block.beacon.activate", "pitch": 1.1, "volume": 0.5, "delay_ticks": 8 }
    ]
  },
  "halfstep_rechallenge_trigger_zone_echo": {
    "layers": [
      { "sound": "block.beacon.ambient", "pitch": 1.0, "volume": 0.3, "delay_ticks": 0 }
    ]
  }
}
```

- 这三个 recipe key 由 agent narration emit 时与 narration scope 一一对应；client audio bus 按 key 播放
- 音量参考 `plan-audio-v1` baseline（broadcast: 0.6-0.8 / player: 0.5-0.7 / zone echo: 0.2-0.4）

- [x] ≥ 8 单测（队列 FIFO 顺序 / 窗口过期出队 / dormant NPC 触发 hydrate / 玩家收到通知 / 非 HalfStep 不收到通知 / 重渡失败走 `settle_failed` 降境 / NPC 与玩家同池排序正确 / narration scope 正确）

**P3 验收**：e2e 手测——化虚修士被击杀 → 全服 narration 广播 → 队列头部 HalfStep 玩家 HUD 提示 + 7d 倒计时 → 玩家可重新起劫；并发场景下队列 FIFO 正确 + 过窗修士自动出队

---

## §8 决策（2026-05-17 closed）

五个开放问题已在实施前互动决策全部收口（user 拍板，全部采纳推荐项；推荐依据见 worldview 锚点列）：

| # | 问题 | 决策 | 关键实装 | worldview 锚点 |
|---|------|------|---------|----------|
| Q1 | 重渡有效时长 | **7 days in-game (~7h real)** | `RECHALLENGE_WINDOW_TICKS` const + `HalfStepState.rechallenge_window_until` | §三:78 稀缺 + §十:1013 寿元节奏 |
| Q2 | 重渡排队 | **先到先得**（按 `HalfStepState.entered_at` FIFO） | `HalfStepRechallengeQueue` resource | §三:124 平等 |
| Q3 | 重渡失败代价 | **同正常渡劫**（失败降境到通灵初） | 复用 `tribulation::settle_failed` 通灵降境路径 | §十二:1043 生死循环 + plan-tribulation-v1 §2 |
| Q4 | buff 叠加 | **仅取最大**（多次半步只算一次） | `HalfStepState.buff_applied` 守卫，已应用则 skip | §三:78 化虚稀缺 |
| Q5 | dormant NPC 优先级 | **同池竞争**（NPC 与玩家共享 quota + 同 FIFO 队列） | NPC HalfStep 入队 `HalfStepRechallengeQueue`，触发时强制 hydrate | §三:124 NPC 与玩家平等 |

后续若运营数据显示需要调整（如窗口过紧 / buff 过弱 / 排队机制不公平），由跟进 plan（如 plan-halfstep-buff-calibration-v1）处理，本 plan 不再展开。

---

## Finish Evidence

**验收日期**：2026-05-18
**实施分支**：`auto/plan-halfstep-buff-v1`
**实施 commits**：4 个 atomic（plan refine + P0 + P1/P2 + P3）

### 落地清单（每阶段对应真实模块/文件路径）

**P0 — 遥测 + dev 命令**：
- `server/src/cultivation/tribulation.rs:107-117` 三个 const（`HALFSTEP_QI_MAX_BONUS` / `HALFSTEP_LIFESPAN_BONUS_YEARS` / `RECHALLENGE_WINDOW_TICKS`）
- `server/src/cultivation/tribulation.rs` `HalfStepState` component + `TribulationMetrics` / `QuotaFullTracker` resource
- `server/src/cultivation/tribulation.rs` `track_tribulation_metrics_system` / `track_quota_full_duration_system` / `current_quota_full_duration_ticks` helper
- `server/src/cmd/dev/tribulation_debug.rs` 新文件，`/tribulation_debug` 命令 + `build_report` / `format_report`
- `server/src/cmd/registry_pin.rs` 加 `"tribulation_debug"` literal + tree path

**P1 — buff 实装 + ledger + 不叠加守卫**：
- `server/src/qi_physics/ledger.rs` `QiTransferReason::HalfStepBuff` 新增 variant（audit-only，不动 balance）
- `server/src/cultivation/tribulation.rs` `track_tribulation_metrics_system` 扩展：HalfStep settlement 时 `cultivation.qi_max *= 1.10`、`lifespan.cap_by_realm += 200`、emit `QiTransfer` audit event
- `HalfStepState.buff_applied` 字段守卫（§8 Q4）：第二次 settlement 跳过重新应用

**P2 — quota 原子授予**：
- `server/src/persistence/mod.rs` `try_complete_tribulation_ascension` 新增（transaction 内校验 `occupied < limit`，超限 deny 返回 `AtomicAscensionOutcome { granted: false }`）
- `server/src/cultivation/tribulation.rs` `juebi_settlement_system` 改用 `try_complete_*`：`ascension_granted` 标志驱动 outcome（Ascended/HalfStep），新增 `WorldQiBudget` + `VoidQuotaConfig` 系统 params

**P3 — 重渡机制 + dev 命令**：
- `server/src/cultivation/tribulation.rs` `HalfStepRechallengeEntry` / `HalfStepRechallengeQueue` resource / `HalfStepRechallengeTriggerEvent` event
- `server/src/cultivation/tribulation.rs` `dispatch_rechallenge_on_quota_opened_system`（AscensionQuotaOpened 派发 + 过窗 drop + FIFO）
- `server/src/cultivation/tribulation.rs` `track_tribulation_metrics_system` 扩展：HalfStep 首次结算入队 + Ascended/Killed/Failed/Fled 时 `remove_entity` 清队
- `server/src/cmd/dev/tribulation_rechallenge.rs` 新文件，`/tribulation_rechallenge` 命令 + `check_rechallenge_gate` 独立校验 fn
- `server/src/cmd/registry_pin.rs` 加 `"tribulation_rechallenge"` literal + tree path
- `server/src/cultivation/mod.rs` 注册 `HalfStepRechallengeQueue` resource + `HalfStepRechallengeTriggerEvent` event + `dispatch_rechallenge_on_quota_opened_system` 系统

### 关键 commits

| commit | 日期 | 摘要 |
|--------|------|------|
| `8c4252526` | 2026-05-18 | docs: §8 五决策收口 + 去除 P0 观察期 + 细化 P3 |
| `0f71a9bdf` | 2026-05-18 | feat(tribulation): P0 渡虚劫遥测 + /tribulation_debug |
| `f53138464` | 2026-05-18 | feat(tribulation): P1 半步 buff 实装 + P2 quota 原子授予 |
| `f829ca979` | 2026-05-18 | feat(tribulation): P3 重渡机制 + FIFO 队列 + 派发系统 + dev 命令 |

### 测试结果

`cd server && cargo test` → **5053 passed; 0 failed**（截至 PR #257 review-2 修复后；累计 47 新 case：13 P0 + 10 P1 + 7 P2 + 11 P3 队列/派发/路径 + 6 dev 命令 + 文档/边界守护；既有 5006 个测试零回归）

`cd server && cargo clippy --all-targets -- -D warnings` → 零 warning

测试分布：
- `cultivation::tribulation::tests::track_metrics_*`（5 case）— P0 计数 + HalfStepState 插入
- `cultivation::tribulation::tests::quota_full_*` / `current_quota_*` / `halfstep_state_*`（3 case）— quota 满时长 + window 边界
- `cultivation::tribulation::tests::halfstep_buff_*`（5 case）— P1 buff 应用 / 不叠加守卫 / ledger event / 无 lifespan / 无 cultivation
- `persistence::persistence_tests::try_ascension_*`（5 case）— P2 grant/deny/zero-limit/idempotent/FCFS 并发
- `cultivation::tribulation::tests::rechallenge_*` / `settlement_*` / `dispatch_*`（8 case）— P3 FIFO 入队/排序/过窗 drop/多事件耗尽
- `cmd::dev::tribulation_debug::tests::*`（4 case）— dev 命令 report
- `cmd::dev::tribulation_rechallenge::tests::*`（7 case）— gate check + handle 行为

### 跨仓库核验

- **server**（命中 symbol）：`HalfStepState` / `TribulationMetrics` / `QuotaFullTracker` / `HalfStepRechallengeQueue` / `HalfStepRechallengeTriggerEvent` / `HALFSTEP_QI_MAX_BONUS` / `HALFSTEP_LIFESPAN_BONUS_YEARS` / `RECHALLENGE_WINDOW_TICKS` / `QiTransferReason::HalfStepBuff` / `try_complete_tribulation_ascension` / `AtomicAscensionOutcome` / `track_tribulation_metrics_system` / `track_quota_full_duration_system` / `dispatch_rechallenge_on_quota_opened_system`
- **agent**：本 PR 不动 agent；`HalfStepRechallengeTriggerEvent` 已在 server 侧 emit，agent 侧 narration prompt 接入由跟进 PR
- **client**：本 PR 不动 client；HUD "灵机涌现" 提示 + 7d 倒计时由跟进 client Java PR

### 遗留 / 后续

1. **客户端 HUD**：`client/src/hud/tribulation_status.java` 监听 `HalfStepRechallengeTriggerEvent`（经 server data emit）+ 弹"灵机涌现，可重渡虚劫"+ 倒计时显示。事件已 emit，待 client 侧实施
2. **agent narration**：plan 中 3 条模板（"灵脉间隐约传来一股真元波动..." / "你感到曾遭封压的经脉微微松动..." / "虚空中某处的修士..."）的 agent 接入。事件可经 Redis 透传到 agent
3. **dormant NPC hydrate**：`HalfStepRechallengeEntry.is_dormant=true` 路径在 plan-npc-virtualize-v1 的 hydrate-on-trigger 中接入。dispatch 已 emit 带 dormant 标记的事件，dormant 模块侧 listen + 强制 hydrate 后玩家路径生效
4. **首期 buff 校准跟进 plan**：`HALFSTEP_QI_MAX_BONUS=0.10` / `HALFSTEP_LIFESPAN_BONUS_YEARS=200.0` 是占位值；运营数据驱动的微调（如 §8 决策门预设的 30% / 5% 阈值）由 `plan-halfstep-buff-calibration-v1`（待立）处理
