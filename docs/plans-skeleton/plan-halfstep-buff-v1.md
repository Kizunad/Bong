# Bong · plan-halfstep-buff-v1 · 骨架

半步化虚 buff 强度校准 + 重渡通知闭环

**来源**：`plan-tribulation-v1` §9 遗留（"半步化虚 buff 强度：延后决定"）+ §8 重渡窗口决策

> **注意：P0–P3 代码早于本文档实装**（在 plan-tribulation-v1 消费过程中顺带完成，
> 代码注释已标注各 P 归属）。本骨架补全文档存档，并定义剩余 P4–P5 工作。
> 全部 P0–P3 symbol 均位于 `server/src/cultivation/tribulation.rs`。

---

## 接入面 Checklist

- **进料**：`DuXuOutcomeV1::HalfStep`（结算事件）· `AscensionQuotaOpened`（名额空出事件）·
  `CombatClock`（tick 计时）· `WorldQiBudget`（名额公式输入）
- **出料**：`HalfStepState` component（玩家/NPC 实体）·
  `HalfStepRechallengeQueue`（FIFO 重渡队列 Resource）·
  `HalfStepRechallengeTriggerEvent`（P3 ECS 内部 event，P4 需转发 client）·
  `TribulationMetrics` 遥测计数
- **共享类型**：`DuXuOutcomeV1::half_step` variant（`agent/packages/schema/src/tribulation.ts` + 生成 JSON）·
  `QiTransferReason::HalfStepBuff`（ledger 记账）· `TribulationStateStore.half_step_on_success`（client Java）
- **跨仓库契约**（P4 新增）：server → client CustomPayload `HalfStepRechallenge { char_id, window_until }` ——
  当前 `HalfStepRechallengeTriggerEvent` 只在 server ECS 内部 fire，**未转发 client**；
  P4 需在 `network/tribulation_state_emit.rs` 新增 emit 路径
- **worldview 锚点**：worldview §一「化虚境界名额制」——半步化虚是名额满时的委屈版突破，
  通灵圆满永久 buff（真元上限 +10%、寿元 +200 年）+可在名额空出时重渡是正典（worldview §一:65-72）；
  真元: 通灵上限 300 / 化虚上限 500；寿元: 通灵约 2100 年 / 化虚约 10700 年
- **qi_physics 锚点**：`QiTransfer { reason: QiTransferReason::HalfStepBuff, ... }` ——
  buff 应用时真元上限扩容走 ledger 记账（P1 已实装，`server/src/qi_physics/ledger.rs`）

---

## 阶段总览

| 阶段 | 状态 | 主要交付物 | 验收标准 |
|------|------|-----------|---------|
| **P0** | ✅ code-first | `HalfStepState` · `TribulationMetrics` · `QuotaFullTracker` 数据模型 + 遥测 | 遥测字段可读；quota 满时长准确累计 |
| **P1** | ✅ code-first | buff 常数 + 不叠加守卫 + qi_physics ledger 记账 | 首次 HalfStep → qi_max +10%、寿元 +200；重复不叠加 |
| **P2** | ✅ code-first | atomic quota grant（防多人同帧竞态回退 HalfStep） | 并发结算场景唯一授予 |
| **P3** | ✅ code-first | `HalfStepRechallengeQueue`（FIFO 玩家+NPC 同池）+ dispatch system | 名额空出 → 队列头收到 `HalfStepRechallengeTriggerEvent` |
| **P4** | ⬜ | server→client IPC + client HUD 重渡通知 + 视听规格 | 玩家在游戏内收到"可重渡虚劫"提示 |
| **P5** | ⬜ | agent narration（halfstep 结算 + 重渡触发）+ 数值运营校准依据 | 天道叙事在 halfstep 和重渡时各出 1-2 条；遥测基准录档 |

---

## P0–P3 实装摘要（code-first，文档化存档）

所有 symbol 在 `server/src/cultivation/tribulation.rs`，行号为 2026-05-20 快照。

### P0 数据模型与遥测（L614–L658）

- `HalfStepState { entered_at, rechallenge_window_until, buff_applied }` — ECS Component；
  `rechallenge_window_until = entered_at + RECHALLENGE_WINDOW_TICKS`
- `RECHALLENGE_WINDOW_TICKS = 7 * 24 * 3600 * 20`（7 days in-game，§8 Q1 已定）
- `TribulationMetrics { halfstep_count, ascended_count, quota_full_duration_ticks }` — Resource 遥测
- `QuotaFullTracker { current_occupied, current_limit, full_since_tick }` — quota 满时长事件驱动追踪器

### P1 Buff 常数与应用（L115–L120，L8265 测试段）

- `HALFSTEP_QI_MAX_BONUS: f32 = 0.10`（真元上限 +10%，首期保守值，worldview §一 通灵上限 300）
- `HALFSTEP_LIFESPAN_BONUS_YEARS: u32 = 200`（寿元 +200 年，worldview §一 通灵寿元约 2100 年，+9.5%）
- `buff_applied` flag 防叠加守卫；`QiTransferReason::HalfStepBuff` 写 qi_physics ledger
- **测试**：`halfstep_buff_applies_qi_max_and_lifespan_on_first_settlement` · `halfstep_buff_emits_audit_qi_transfer_event` · `halfstep_buff_not_reapplied_when_state_already_marks_buff_applied` · `halfstep_buff_applies_to_qi_max_only_when_lifespan_component_absent` · `halfstep_buff_skipped_when_entity_lacks_cultivation_and_state_left_unbuffed`

### P2 Atomic Quota Grant（L1894 注释段）

- 渡虚劫结算时先 atomic check-and-grant，被抢先 → 回退 `HalfStep`；防多人同帧 unconditional increment

### P3 重渡队列与派发（L660–L733，L2198–L2235，L8450 测试段）

- `HalfStepRechallengeQueue`（VecDeque FIFO，按 `entered_at` 升序）—— 玩家 + dormant NPC 同池（§8 Q5）
- `HalfStepRechallengeEntry { char_id, entity, entered_at, rechallenge_window_until, is_dormant, buff_applied }`
- `find_by_char_id`：dormant→hydrate 换 entity 时复用旧 `entered_at`/`window`，防 FCFS 顺序乱（P4 review #2 fix）
- `dispatch_rechallenge_on_quota_opened_system`：`AscensionQuotaOpened` → 取队列头（跳过过窗 entry）→ emit `HalfStepRechallengeTriggerEvent`
- **测试**：`rechallenge_queue_enqueue_preserves_fcfs_by_entered_at` · `rechallenge_queue_remove_entity_clears_all_entries_for_target` · `halfstep_re_settle_after_dispatch_pop_re_enqueues_and_preserves_entered_at` · `halfstep_buff_applied_flag_backfills_on_resettle_when_cultivation_now_present` · `halfstep_buff_does_not_double_apply_within_same_frame`

---

## P4 server→client IPC + 重渡 HUD（待实装）

### P4.1 server 侧 IPC 扩展

- `server/src/network/tribulation_state_emit.rs`：消费 `HalfStepRechallengeTriggerEvent`，
  emit CustomPayload `HalfStepRechallenge { char_id, window_until_tick }`（新增 payload type ID）
- agent schema：`agent/packages/schema/src/tribulation.ts` 新增 `HalfStepRechallenge` type；
  `samples/server-data.tribulation-halfstep-rechallenge.sample.json` 样本文件

### P4.2 client HUD（Fabric）

- `client/src/main/java/com/bong/client/combat/handler/HalfStepRechallengeHandler.java`（新建）——
  实现 `BongPacketHandler`，接收 payload → 调 `TribulationStateStore.setRechallengePending(windowUntil)`
- `TribulationStateStore.java`：新增 `rechallengePendingUntil: long` 字段
- `TribulationBroadcastHudPlanner.java`：新增 `HALFSTEP_RECHALLENGE` 弹出层，
  检查 `rechallengePendingUntil > 0` → 渲染 toast

### P4.3 视听规格

- **HUD toast**：
  - overlay 类型：`BongToast`（复用渡虚劫结算 toast 系统）
  - 文案：「灵机涌现，可重渡虚劫」（中文，固定文案）
  - 颜色：`#D4AF37`（金色，对应化虚突破光），opacity 0.85
  - 持续时间：200 tick（约 10 秒）
  - fade in/out：20 tick linear
  - HudRenderLayer：`TRIBULATION_NOTIFICATION`（已有层，复用）
- **粒子**：无新粒子（server 已有渡虚劫 VFX；重渡通知仅 HUD，不加额外粒子）
- **音效**：`audio_recipe` ——
  ```json
  { "layers": [{ "sound": "entity.experience_orb.pickup", "pitch": 1.4, "volume": 0.6, "delay_ticks": 0 }] }
  ```
  复用金属音轻播（传递"机会来了"感，不与渡虚劫雷鸣撞车）

### P4.4 测试要求

- `HalfStepRechallengeHandlerTest.java`：接收 payload → state 字段正确写入
- `TribulationStateStoreTest.java`：`rechallengePendingUntil` 字段 getter/setter 边界（0 / 正值 / 过期判断）
- server 侧：`halfstep_rechallenge_trigger_emits_ipc_payload` 单测（消费 `HalfStepRechallengeTriggerEvent`，验证 network emit）

---

## P5 Agent narration + 数值运营校准（待实装）

### P5.1 Agent narration

**触发点 1：渡虚劫结算 HalfStep**（scope: player, style: narrative）

- 「天道此刻不容你跨越，你伫立在虚境门槛——既非门内之人，亦非门外游魂。那一丝真元的扩张，如同窗纸上的微光，虚实之间，悬而不落。」
- 「虚境有定额，叩关者众。你扛过了全部的雷，却终究只能在门外等候名额空出。」
- 「半步。门开了一条缝，又合上了。但你的真元多撑了一分，寿元多续了两百年——这是天道留给你的话，也是下次叩关的凭据。」

**触发点 2：重渡窗口触发**（scope: player, style: perception）

- 「虚境有名额空出——你感到那一道裂缝再次召唤，重渡之机，就在眼前。」
- 「有人离开了虚境，那道门又有了空隙。半步修士，你的窗口仍在，可否再叩？」

narration 由 `agent/packages/tiandao/src/tribulation-runtime.ts` 中 `half_step` case 扩展；
触发点 2 监听新 Redis key `bong:tribulation/halfstep_rechallenge`（或与 tribulation settle 同 channel 新 variant）。

### P5.2 数值校准依据与运营决策

首期值（plan-tribulation-v1 §8 延后决定，已编入代码）：
- 真元上限 +10% ≈ 通灵上限 300 × 10% = +30 真元
- 寿元 +200 年 ≈ 通灵寿元 2100 × 9.5%

后续调参依据（需上线后遥测数据）：
- `TribulationMetrics.quota_full_duration_ticks`：若大多数半步修士等待超过 14 days in-game，
  则 buff 太弱 → 考虑提高 `HALFSTEP_QI_MAX_BONUS` 到 0.15 或增加寿元至 400 年
- 半步 vs 化虚比例：若半步占所有 HalfStep+Ascended 结算 > 80%，
  说明名额瓶颈过紧 → 检查 `DEFAULT_VOID_QUOTA_K`（quota 基数公式）而非 buff 幅度
- **调参约束**：真元上限 buff 不超过 +20%（不让半步修士强于通灵圆满太多）；
  寿元 buff 参考 worldview §一「化虚寿元 10700」，保持半步<化虚的明显鸿沟

---

## §8 开放问题（P4/P5 实装时收口）

### Q1 重渡窗口时长（已定：7 days in-game）
`RECHALLENGE_WINDOW_TICKS = 7 * 24 * 3600 * 20`，首期合理，不动。

### Q2 FIFO vs 等待时间加权（已定：FIFO 先到先得）
worldview §一无排序约束，FCFS 最公平也最简单。

### Q3 重渡时是否要求再次满足前置条件（**待 P4 收口**）
当前 dispatch 无检查——名额空出就通知，不验证修士当前境界/奇经状态。
P4 实装时决定：A 无门槛（保持现状，玩家退境期间也收通知）；
B 有门槛（dispatch 时 query entity 验证仍在通灵圆满 + 奇经八脉全通）。

### Q4 不叠加守卫（已定：`buff_applied` flag）
同 char_id 重复结算 HalfStep 不叠加 buff，已测。

### Q5 NPC 同池（已定）
dormant NPC 与玩家共用 `HalfStepRechallengeQueue`，按 `entered_at` FIFO。

### Q6 dormant NPC 重渡通知路径
对 `is_dormant=true` entry，dispatch 时需 hydrate NPC（plan-npc-virtualize-v1 hydrate-on-demand）；
P4 实装时验证 `HalfStepRechallengeTriggerEvent.is_dormant=true` 这条路径是否与 v1 hydrate 桥接。
