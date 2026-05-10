# Bong · plan-tribulation-v3 · 骨架

**半步化虚校准包** —— 两项遗留于 plan-tribulation-v1 §8 的占位参数落地：
1. **半步化虚 buff 强度校准**：+10% 真元上限 / +200 年寿元 当前为占位值，需观察玩家卡关比例后数据驱动调整
2. **名额空出时可重渡机制**：化虚名额满时主动申请入队；名额释放时通知队列首位可重渡

**设计哲学**：不是「降低难度」，是「让等候状态有尊严」——半步化虚应感觉是「伏鲲待跃」而非「被系统卡住」。worldview §三:78 天道不故意为难，只是名额有限；名额释放时第一时间通知，是天道公正的体现。

**世界观锚点**：`worldview.md §三:78`（天道冷漠公正，名额有限不是惩罚）· `§三:187`（化虚质变，到了就是到了，不是攒经验就能过）· 半步化虚状态（灵台蓄满却无名额的等候状态）

**交叉引用**：`plan-tribulation-v1.md` ✅（渡虚劫 + 半步化虚 + AscensionQuotaStore 基础）· `plan-tribulation-v2.md` ✅（绝壁劫，名额超额判定已调整）· `plan-npc-virtualize-v1.md` ✅（dormant NPC 参与名额竞争，重渡需同步处理）· `plan-death-lifecycle-v1.md` ✅（玩家死亡释放名额路径）

---

## 接入面 Checklist

- **进料**：`cultivation::tribulation::TribulationState::HalfStepVoid`（v1 已建）+ `AscensionQuotaStore { max: 4, current: usize }`（v1 已建）+ 新增滞留时长 telemetry
- **出料**：校准后的 `HALF_STEP_QI_MAX_BONUS_RATIO` / `HALF_STEP_LIFESPAN_BONUS_YEARS` 常数 + `ReascensionRequested` event + `pending_reascension` 等候队列 + HUD 通知
- **共享类型**：复用 `TribulationState::HalfStepVoid` + `AscensionQuotaStore`（新增 `pending_reascension: VecDeque<Entity>` 字段）
- **跨仓库契约**：可选 client HUD 显示排队位置（`TribulationHud` 已有基础）；无新 IPC schema
- **worldview 锚点**：§三:78 + §三:187
- **qi_physics 锚点**：buff 修改 qi_max 需走 `QiTransfer`（bonus 是 zone → player 合法增益，reason: `CultivationBonus`）；不新增物理常数

---

## §0 设计轴心

### ① 半步化虚 buff 校准流程

当前占位常数：
```rust
const HALF_STEP_QI_MAX_BONUS_RATIO: f64 = 0.10;   // +10% 真元上限
const HALF_STEP_LIFESPAN_BONUS_YEARS: u32 = 200;  // +200 年寿元
```

数据驱动校准流程：
1. P0：新增 `HalfStepEnteredAt(Instant)` Component + `bong:metrics/half_step_duration` telemetry（60s 汇总一次平均滞留时长，不按 tick 写）
2. P1：跑实测数据（约 7 天 in-game 时间）→ 分析 half_step_duration 分布
3. P2：根据分布校准：
   - 若 >30% 玩家卡超过 7 天 in-game → buff 太弱，上调 ratio / years
   - 若 <5% 玩家卡超过 2 天 in-game → 名额稀缺性体验弱，酌情下调
   - P0 **不猜测**、不预先调整常数

### ② 名额空出时可重渡机制

**队列模型**：`AscensionQuotaStore` 新增 `pending_reascension: VecDeque<Entity>`

触发流程：
```
玩家/NPC 进入 HalfStepVoid 时可手动执行「申请重渡」命令
  → 若名额已满 → 入 pending_reascension 队列（获得排队位置 HUD 提示）
  → 若名额未满 → 直接开始渡劫准备（TribulationState::Preparing）

名额释放事件（玩家死亡 / 化虚 NPC 老死 / 周目重置）
  → 检查 pending_reascension 队列头部
  → emit ReascensionRequested { entity }
  → 对应实体收到 event → HUD 提示「天时已至，可再渡」
  → 玩家确认（hotbar 激活渡劫仪式）→ 重走 TribulationState::Preparing
```

**入队约束**：
- **主动申请**（不自动入队）——防止离线玩家占坑
- 同一实体不重复入队（幂等检查）
- dormant NPC 满足条件时直接走 plan-npc-virtualize-v1 渡虚劫 hydrate 路径，无需手动确认

---

## §1 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ⬜ | `HalfStepEnteredAt` Component + `bong:metrics/half_step_duration` telemetry（60s 汇总）+ 现有 buff 常数暂不动 | telemetry 数据可在 Redis 读出；`cargo test` 绿 |
| **P1** ⬜ | 跑实测数据（7+ 天 in-game）→ 统计 half_step_duration 分布 → 落 `docs/balance/half_step_v3_calibration.json` | 数据文件存在且有 ≥20 条样本 |
| **P2** ⬜ | 更新 buff 常数（`HALF_STEP_QI_MAX_BONUS_RATIO` / `HALF_STEP_LIFESPAN_BONUS_YEARS`）+ qi_physics 守恒单测 | `cargo test cultivation::tribulation::half_step*` 绿 + QiTransfer 守恒验证通过 |
| **P3** ⬜ | `pending_reascension` 队列 + `ReascensionRequested` event + 名额释放 hook + dormant NPC 重渡路径 | e2e：名额满 → 申请入队 → 玩家死亡释放名额 → 队列首位收到 `ReascensionRequested` → 重渡成功 |
| **P4** ⬜ | client HUD：排队位置显示 + 「天时已至」提示动画 | client runClient 可见入队提示 + 到位通知动画 |

---

## §2 数据契约

- [ ] `server/src/cultivation/tribulation.rs` — `HalfStepEnteredAt(std::time::Instant)` Component；进入 `HalfStepVoid` 时 insert
- [ ] `server/src/cultivation/tribulation.rs` — `telemetry_half_step_system`：每 60s 统计 HalfStepVoid 实体平均滞留时长 → publish `bong:metrics/half_step_duration` JSON
- [ ] `server/src/cultivation/tribulation.rs` — `HALF_STEP_QI_MAX_BONUS_RATIO` / `HALF_STEP_LIFESPAN_BONUS_YEARS`：P2 按数据更新
- [ ] `server/src/cultivation/tribulation.rs` — `AscensionQuotaStore` 加 `pending_reascension: VecDeque<Entity>` + 幂等入队检查
- [ ] `server/src/cultivation/tribulation.rs` — `ReascensionRequested { entity: Entity }` event
- [ ] `server/src/cultivation/tribulation.rs` — `quota_release_hook_system`：名额释放时检查队列 → emit ReascensionRequested → 通知 client HUD
- [ ] `server/src/npc/virtualize.rs` — dormant NPC ReascensionRequested 走渡虚劫 hydrate 路径（不入 pending_reascension 队列）

---

## §3 开放问题

- [ ] **telemetry 采样粒度**：60s 汇总平均值是否够用？还是需要保留分位数（P50/P90/P99）？
- [ ] **入队命令 UI 形式**：hotbar 特殊渡劫格激活 vs 专属 `/reascension` chat 命令 vs 进入 HalfStepVoid 状态后弹 UI 选择
- [ ] **dormant NPC 与玩家名额优先级**：先入先出 vs 玩家优先 vs NPC 优先（worldview §三:124 NPC 与玩家平等原则倾向先入先出）
- [ ] **重渡劫失败后 buff 状态**：失败是恢复 HalfStepVoid（保留 buff 继续等）还是按普通渡劫失败（境界跌落 / 死亡）？

## §4 进度日志

- 2026-05-10：骨架创建。源自 plan-tribulation-v1 §8 两条遗留待办：半步化虚 buff 强度占位 + 名额空出重渡机制。plan-tribulation-v2 已 ✅ finished（绝壁劫）。
