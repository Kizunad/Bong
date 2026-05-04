# Bong · plan-halfstep-void-v1 · 骨架

**半步化虚**精细化：buff 强度校准 + 名额空出后的重渡机制。

当前实装（占位）：`HALF_STEP_QI_MAX_MULTIPLIER = 1.10`（真元上限 +10%）、`HALF_STEP_LIFESPAN_YEARS = 200`（寿元 +200 年），位于 `server/src/cultivation/tribulation.rs:73-74`。

**世界观锚点**：`worldview.md §三`（化虚境仅容 1-2 人；通灵寿元上限 1000 年，化虚 2000 年）· `§八`（天道不偏袒，但也不虐待"等队的人"）

**交叉引用**：`plan-tribulation-v1` ✅（半步结算分支 `DuXuOutcomeV1::HalfStep` 已落）· `plan-void-quota-v1`（骨架，世界灵气驱动的名额公式）· `plan-lifespan-v1` ✅（`LifespanCapTable`）

---

## 接入面 Checklist

- **进料**：`cultivation::tribulation::TribulationState.half_step_on_success`（已有）· `AscensionQuotaStore`（当前 quota 计数）· `LifespanComponent`（寿元）· `CultivationComponent.qi_max`（真元上限）
- **出料**：修改 `HALF_STEP_QI_MAX_MULTIPLIER` / `HALF_STEP_LIFESPAN_YEARS` 常量 → 结算时应用新值 · 重渡时 emit `DuXuRetryReady` 通知 client
- **共享类型**：`DuXuOutcomeV1::HalfStep`（已有）· 新增 `DuXuRetryReady` event schema（半步玩家可重渡广播）
- **跨仓库契约**：server 结算常量 + agent narration（"天地余位已开，半步者可往"）+ client HUD 显示重渡可用状态
- **worldview 锚点**：§三 通灵/化虚寿元表 · §八 天道不偏袒

---

## §0 设计轴心

- [ ] **buff 有意义但不鸡肋**：半步者扛过了天劫，理应有实质奖励，但不能让人"宁愿半步也不等名额"
- [ ] **buff 不逼近化虚**：最终值应在通灵（1000 年）与化虚（2000 年）之间明显靠近通灵侧
- [ ] **重渡门槛低**：已扛过天劫证明实力；名额空出后无需重新积累突破进度，直接可再起劫
- [ ] **重渡通知明确**：天道广播"化虚有位"时，半步者额外得到有针对性的提示（不同于普通广播）

---

## §1 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ⬜ | buff 强度决策：基于通灵/化虚寿元比例确定最终常量值 | `HALF_STEP_QI_MAX_MULTIPLIER` / `HALF_STEP_LIFESPAN_YEARS` 改写 + 单测更新 |
| **P1** ⬜ | 重渡机制：名额释放时对半步玩家额外广播 `DuXuRetryReady` + HUD 提示 | client HUD "重渡" 状态可见；再起劫不需重走突破流程 |
| **P2** ⬜ | agent narration：半步结算专属叙事 + 重渡可用叙事 | tiandao 收到 `DuXuRetryReady` 后出"天地余位已开" narration |

---

## §2 Buff 强度参考区间

```
通灵寿元上限 = 1000 年
化虚寿元上限 = 2000 年
半步化虚（目标）= 明显高于通灵但远低于化虚

建议落点：
  HALF_STEP_LIFESPAN_YEARS  ∈ [250, 400]   → 最终寿元 1250~1400 年 ≈ 通灵 25-40% 加成
  HALF_STEP_QI_MAX_MULTIPLIER ∈ [1.12, 1.20] → 真元上限 +12~20%（通灵 300 → 336~360）

占位值（当前）：200 年 / +10% — 偏小，玩家感受"几乎没奖励"
上限红线：不超过 500 年 / +30%（那就太接近化虚了）
```

决策标准（观察期）：
- 若 `卡在半步化虚 / 全服化虚玩家` > 2:1，说明名额稀缺、buff 需要更有吸引力 → 上调
- 若 < 0.5:1，说明名额充裕、半步者迅速重渡，buff 占位影响不大 → 保守值即可

---

## §3 重渡机制设计

```
名额释放触发链（已有）：
  死亡/降境 → quota release → AscensionQuotaOpened event

新增：
  AscensionQuotaOpened → 查询所有 HalfStep 玩家
                        → emit DuXuRetryReady { entity, reason: "quota_freed" } per player
                        → client: HUD 新标签"重渡通道开启" (类似 TribulationLocked badge)
                        → agent: "天地余位已开，等候者可往" narration（批量，不逐一）

重渡起劫条件（简化）：
  半步玩家无需再次满足奇经全通条件（已经过了天劫）
  → server: HalfStep 状态 = 已满足 can_start_tribulation 前置
  → 名额检查照常（如果又被占满则仍走 HalfStep 或等）
```

- [ ] `server/src/cultivation/tribulation.rs`：`apply_half_step_result` 使用新常量
- [ ] `server/src/cultivation/tribulation.rs`：`ascension_quota_released_system` 扫 HalfStep 玩家 emit `DuXuRetryReady`
- [ ] `agent/packages/tiandao/skills/calamity.md`：`DuXuRetryReady` narration 模板
- [ ] `client/src/.../TribulationHud.java`：渲染 `DuXuRetryReady` badge

---

## §4 开放问题

- [ ] 是否允许半步玩家在 **名额未开时** 主动起劫（即走 HalfStep → HalfStep 的闭环）？当前应拒绝（没意义）
- [ ] 多个半步玩家同时重渡，quota 争抢顺序？FCFS or 先到先得（server tick 序）
- [ ] 半步 buff 应用时机：是结算时 **一次性永久应用**（当前），还是持续 component？当前实现是永久改写 qi_max / lifespan，保持即可

---

## §5 进度日志

- 2026-05-04：骨架创建。源自 `plans-skeleton/reminder.md` plan-tribulation-v1 遗留条目（占位 buff + 重渡机制待确认）。实装位置 `server/src/cultivation/tribulation.rs:73-74`。
