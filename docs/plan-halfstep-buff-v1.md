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
- `plan-tribulation-v1` ✅ — `DuXuOutcomeV1::HalfStep` / `AscensionQuotaStore` / `ascension_quota` Redis key
- `plan-cultivation-v1` ✅ — `cultivation.qi_max` / `cultivation.lifespan_max` 字段
- `plan-npc-virtualize-v1` ✅（可选）— dormant NPC 重渡触发 hydrate 路径

**反向被依赖**：
- `plan-tribulation-balance`（待立，若需系统性平衡）— 半步 buff 是更大平衡矩阵的一部分
- `plan-multi-life-v1` ✅ — 跨周目半步 buff 是否继承（当前 plan-multi-life 已有处理，本 plan 只调 buff 值）

---

## 接入面 Checklist

- **进料**：`AscensionQuotaStore`（当前 quota / max）+ `DuXuOutcomeV1::HalfStep` 结算代码（`tribulation.rs`）+ 遥测数据（半步化虚玩家数 / quota 满时长占比）
- **出料**：调整后的 buff 常数（`HALFSTEP_QI_MAX_BONUS: f32` / `HALFSTEP_LIFESPAN_BONUS_YEARS: f64`）+ `HalfStepRechallengeTriggerEvent` 🆕 + 重渡起劫接入点（`tribulation::request_rechal­lenge`）
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
| **P0** | ⬜ | 遥测计数器 + `/debug tribulation` dev 命令 | mock 10 次半步结算 → counter == 10；dev 命令可读取 |
| **P1** | ⬜ | buff 实装为命名 const + qi_physics ledger 标记 + 不叠加守卫 | `HALFSTEP_QI_MAX_BONUS=0.10` / `HALFSTEP_LIFESPAN_BONUS_YEARS=200.0` 在 settlement 生效；ledger 记账正确 |
| **P2** | ⬜ | quota 事务性再校验（复用既有 `AscensionQuotaOpened`） | 500 次并发起劫不漏判 Ascended ≤ quota_max |
| **P3** | ⬜ | 重渡触发机制（7d 窗口 + FIFO + NPC 同池）+ HUD 提示 + e2e | 名额空出后队列头部半步修士收到提示 + 可重新起劫；过窗自动出队 |

---

## P0 — 遥测计数器 + dev 命令

- [ ] 遥测计数器（`server/src/cultivation/tribulation.rs` metrics 段）：
  - `tribulation_halfstep_count` — 累计半步化虚人次
  - `tribulation_ascended_count` — 累计化虚人次
  - `ascension_quota_full_duration_ticks` — quota 满时（current == max）的累计 tick 数
  - `halfstep_stuck_duration_ticks` — 当前半步修士平均滞留 tick 数
- [ ] `/debug tribulation` 命令显示以上遥测数据（dev-only，CLAUDE.md 测试命令段；与 `/meridian` `/realm` 等同槽）
- [ ] 该数据用于后续运营观察与跟进 plan 的 buff 校准，本 plan 不在 P0 内做观察期门控

**P0 验收**：遥测计数器在 CI e2e 中可正确累计（mock 10 次半步结算 → counter == 10）；`/debug tribulation` 在 cargo test 中可调用并返回结构化数据

---

## P1 — buff 实装为命名 const + 不叠加守卫

- [ ] 把 buff 值提取为命名 const（`server/src/cultivation/tribulation.rs`）：
  ```rust
  pub const HALFSTEP_QI_MAX_BONUS: f32 = 0.10;     // 首期值，后续运营数据驱动调整
  pub const HALFSTEP_LIFESPAN_BONUS_YEARS: f64 = 200.0; // 首期值，后续运营数据驱动调整
  ```
- [ ] **在 settlement 处真实应用 buff**（当前 `server/src/cultivation/tribulation.rs:1811` 只设置 `DuXuOutcomeV1::HalfStep` 枚举，buff 未应用，是真实代码缺口）：HalfStep 分支补 `cultivation.qi_max *= 1.0 + HALFSTEP_QI_MAX_BONUS` + `lifespan.cap += HALFSTEP_LIFESPAN_BONUS_YEARS`
- [ ] qi_physics ledger 标记：`qi_max` 容量扩张走 `qi_physics::ledger::QiTransfer`（worldview §二 守恒律，参 plan-qi-physics-v1 P1 既有 API）—— 容量扩张视为 Tiandao → entity 的一次性转账记账，不破坏 SPIRIT_QI_TOTAL 恒定
- [ ] **buff 不叠加守卫**（§8 Q4 决策）：第二次起 HalfStep 不再 reapply。用 `HalfStepBuffApplied` marker component（或 `HalfStepState.buff_applied: bool` 字段）做幂等校验，已应用则 skip
- [ ] 回归测试：`assert_eq!(halfstep_buff.qi_max_factor, HALFSTEP_QI_MAX_BONUS)` 引用 const（**禁止测试写字面 0.10**，防止常数改了测试不跟）
- [ ] ≥ 5 单测（buff 应用后 qi_max 正确计算 / lifespan 正确增加 / **buff 不叠加（同一 entity 二次 HalfStep settlement 后 qi_max 不变化）** / dormant NPC 同样应用 / qi_physics ledger 记账正确）

**P1 验收**：const 提取 + settlement 实装 + 5 单测 green；run `cargo test cultivation::tribulation::halfstep` 全过

---

## P2 — quota 事务性再校验

- [ ] 最终 `Ascended/HalfStep` 判定移入 DB transaction：并发起劫结算时，quota 校验用 Redis `INCR` + `WATCH/MULTI/EXEC` 原子操作（或 Lua script）防止多人同时 Ascended
- [ ] 修复路径：`tribulation::settle_ascension` 函数加 `atomic_quota_check` 内层校验（`server/src/cultivation/tribulation.rs`）
- [ ] ≥ 5 并发测试（10 人同时结算，Ascended 数量 ≤ quota_max；剩余人走 HalfStep 不漏掉）

**P2 验收**：并发测试 green（500 次随机并发结算，Ascended 数量严格 ≤ quota_max）

---

## P3 — 重渡机制 + HUD（7d 窗口 + FIFO + NPC 同池）

- [ ] **复用既有 `AscensionQuotaOpened` event**（`server/src/cultivation/tribulation.rs` 已实装，不另造 `QuotaSlotOpened`）—— 化虚修士死亡 / 降境时已 emit
- [ ] `HalfStepState { entered_at: u64, rechallenge_window_until: u64, buff_applied: bool }` component（玩家 + dormant NPC 通用，与 P1 buff 守卫共用）：
  - `entered_at` = 进入 HalfStep 时的 server tick
  - `rechallenge_window_until = entered_at + RECHALLENGE_WINDOW_TICKS`（§8 Q1 决策）
- [ ] `RECHALLENGE_WINDOW_TICKS` const = `7 * 24 * 3600 * 20`（7 days in-game，server 20Hz；§8 Q1）
- [ ] `HalfStepRechallengeQueue` resource：FIFO 队列，按 `entered_at` 升序保有所有当前 HalfStep 修士（玩家 + dormant NPC 同池；§8 Q2 + Q5 决策）
- [ ] `dispatch_rechallenge_system`（`AscensionQuotaOpened` event 触发）：
  - 取队列头部修士，若 `current_tick > rechallenge_window_until` → 出队丢弃（过窗），继续看下一个，直到找到有效或队列空
  - 若头部修士为玩家 → emit `HalfStepRechallengeTriggerEvent { char_id }` 给该玩家
  - 若头部修士为 dormant NPC → 强制 hydrate（复用 plan-npc-virtualize-v1 dormant 渡虚劫 hydrate 路径），hydrate 后入队第一行
- [ ] 玩家收到 event → client HUD 提示"灵机涌现，可重渡虚劫"（`client/src/hud/tribulation_status.java`）+ 窗口剩余时长倒计时
- [ ] 玩家响应：手动触发 `/tribulation rechallenge`（CLAUDE.md dev-only 命令段）or 在渡劫台交互
- [ ] **重渡失败结算复用 `tribulation::settle_failed` 通灵降境路径**（§8 Q3 决策：失败降境到通灵初，不另设独立宽容路径）
- [ ] narration 模板（scope: broadcast，style: perception）：
  - "灵脉间隐约传来一股真元波动，似有化虚修士陨落，名额空出一席。"（quota 空出时全服广播，复用既有 quota_release narration）
  - "你感到曾遭封压的经脉微微松动，或许时机已到。"（队列头部 HalfStep 玩家收到 rechallenge event；player scope）
  - "虚空中某处的修士收到了相同的消息。"（队列后续多人均为 HalfStep 时；zone scope）
- [ ] ≥ 8 单测（队列 FIFO 顺序 / 窗口过期出队 / dormant NPC 触发 hydrate / 玩家收到通知 / 非 HalfStep 不收到通知 / 重渡失败走 `settle_failed` 降境 / NPC 与玩家同池排序正确 / narration scope 正确）

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
