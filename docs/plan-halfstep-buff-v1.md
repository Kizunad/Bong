# Bong · plan-halfstep-buff-v1 · 骨架

**半步化虚 buff 强度校准 + 名额空出时重渡机制**——承接 plan-tribulation-v1 ✅ finished 中的延后事项：当前 buff 值（真元上限 +10%、寿元 +200 年）为占位，待观察"卡在半步化虚"玩家比例后校准；同时设计名额空出时半步化虚修士可重新尝试渡虚劫的机制（`重渡`）。

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

- **buff 校准原则**：半步化虚 buff 应"有意义但不等同化虚"——worldview §三:78 化虚是质变，半步只是量变。参考基线：通灵满级 vs 化虚的差距在 `qi_max × 1.5-3×`，则半步 buff 10-15% 是合理区间；寿元 +200 年在世界观寿元体系中约是通灵修士"多活半辈子"
- **重渡触发时机**：名额空出（化虚修士死亡 / 被截胡降境）→ `AscensionQuotaStore` 广播 `QuotaSlotOpened` event → 通知所有 `HalfStep` 状态修士 / dormant NPC → 可手动申请重渡 or 自动排队
- **重渡不免费**：重渡起劫消耗与正常渡虚劫相同（需要真元储备 + 3 波 AOE），不是"再点一次"
- **quota 事务性再校验**：多人同时起劫并发 Ascended/HalfStep 最终判定移入 DB transaction（plan-tribulation-v1 §9 遗留）

---

## 阶段总览

| 阶段 | 状态 | 主要交付物 | 验收标准 |
|------|------|-----------|---------|
| **P0** | ⬜ | 遥测仪表盘 + 观察期（≥ 2 weeks 数据）| 半步玩家比例 / quota 满时占比可观测 |
| **P1** | ⬜ | buff 常数校准 + 常数提取为命名 const | `HALFSTEP_QI_MAX_BONUS` / `HALFSTEP_LIFESPAN_BONUS_YEARS` 写入配置 |
| **P2** | ⬜ | quota 事务性再校验 + `QuotaSlotOpened` event | 并发起劫不漏判 Ascended/HalfStep |
| **P3** | ⬜ | 重渡触发机制 + HUD 提示 + e2e | 名额空出后半步修士收到提示 + 可重新起劫 |

---

## P0 — 遥测仪表盘 + 观察期

- [ ] 遥测计数器（`server/src/cultivation/tribulation.rs` metrics 段）：
  - `tribulation_halfstep_count` — 累计半步化虚人次
  - `tribulation_ascended_count` — 累计化虚人次
  - `ascension_quota_full_duration_ticks` — quota 满时（current == max）的累计 tick 数
  - `halfstep_stuck_duration_ticks` — 当前半步修士平均滞留 tick 数
- [ ] `/zone_qi list` 或 `/debug tribulation` 命令显示以上遥测数据（dev-only，CLAUDE.md 测试命令）
- [ ] 观察期 ≥ 2 weeks（或服务器 100h in-game 等效），记录数据后再做 P1 校准

**P0 验收**：遥测计数器在 CI e2e 中可正确累计（mock 10 次半步结算 → counter == 10）

---

## P1 — buff 常数校准

- [ ] 将 buff 值提取为命名 const（`server/src/cultivation/tribulation.rs`）：
  ```rust
  pub const HALFSTEP_QI_MAX_BONUS: f32 = 0.10;    // 待 P0 数据后调整
  pub const HALFSTEP_LIFESPAN_BONUS_YEARS: f64 = 200.0; // 待 P0 数据后调整
  ```
- [ ] 基于 P0 观察数据选定最终值（P1 决策门：若 > 30% 玩家卡在半步超过 1 month in-game → 提高 buff 强度；若 < 5% 才遇半步 → 维持现值）
- [ ] 回归测试：`assert_eq!(halfstep_buff.qi_max_factor, HALFSTEP_QI_MAX_BONUS)` 引用 const（**禁止测试写字面 0.10**，防止常数改了测试不跟）
- [ ] ≥ 5 单测（buff 应用后 qi_max 正确计算 / lifespan 正确增加 / buff 不叠加（多次半步只取一次）/ dormant NPC 同样应用）

**P1 验收**：const 提取 PR 合并 + 5 单测 green

---

## P2 — quota 事务性再校验

- [ ] 最终 `Ascended/HalfStep` 判定移入 DB transaction：并发起劫结算时，quota 校验用 Redis `INCR` + `WATCH/MULTI/EXEC` 原子操作（或 Lua script）防止多人同时 Ascended
- [ ] 修复路径：`tribulation::settle_ascension` 函数加 `atomic_quota_check` 内层校验（`server/src/cultivation/tribulation.rs`）
- [ ] ≥ 5 并发测试（10 人同时结算，Ascended 数量 ≤ quota_max；剩余人走 HalfStep 不漏掉）

**P2 验收**：并发测试 green（500 次随机并发结算，Ascended 数量严格 ≤ quota_max）

---

## P3 — 重渡机制 + HUD

- [ ] `QuotaSlotOpened { new_quota: u32, timestamp: u64 }` event（`server/src/cultivation/tribulation.rs`）：化虚修士死亡 / 降境时 emit
- [ ] `HalfStepRechallengeTriggerEvent { char_id: CharId }` event：广播给所有 HalfStep 状态玩家 / dormant NPC
- [ ] 玩家收到 event → client HUD 提示"灵机涌现，可重渡虚劫"（`client/src/hud/tribulation_status.java`）
- [ ] 玩家响应：手动触发 `/tribulation rechallenge`（CLAUDE.md dev-only 命令段）or 在渡劫台交互
- [ ] dormant NPC HalfStep → QuotaSlotOpened → `dormant_tribulation_rechal­lenge` 触发强制 hydrate（同 v1 dormant 渡虚劫路径）
- [ ] narration 模板（scope: broadcast，style: perception）：
  - "灵脉间隐约传来一股真元波动，似有化虚修士陨落，名额空出一席。"（quota 空出时全服广播）
  - "你感到曾遭封压的经脉微微松动，或许时机已到。"（HalfStep 玩家收到 rechal­lenge event）
  - "虚空中某处的修士收到了相同的消息。"（多人同为 HalfStep 时 scope 缩为 zone）
- [ ] ≥ 8 单测（QuotaSlotOpened emit 条件 / HalfStep 玩家收到通知 / dormant NPC 触发 hydrate / 非 HalfStep 不收到通知 / narration 正确 scope）

**P3 验收**：e2e 手测——化虚修士被击杀 → 全服 narration 广播 → HalfStep 玩家 HUD 提示 → 玩家可重新起劫

---

## §8 开放问题（P0 决策门后收口）

1. **重渡时间限制**：名额空出后多少 in-game 时间内有效（永久有效 vs 24h in-game 窗口）
2. **重渡排队机制**：多个 HalfStep 修士同时请求时先到先得 vs 境界积分（当前真元 × 修炼时长）排序
3. **重渡失败代价**：重渡失败是否降境（同正常渡劫失败，plan-tribulation-v1 §2 规则）or 有宽容机制（首次重渡失败不降境）
4. **buff 值上限**：半步化虚可叠多次（如连续多次达到半步）→ buff 是否叠加 or 仅取最大值
5. **dormant NPC 重渡优先级**：dormant HalfStep NPC 是否与玩家竞争同一名额（worldview §三 平等原则 = 应竞争）or NPC 单独 quota 池
