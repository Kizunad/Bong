# Bong · plan-tribulation-balance-v1 · 骨架

**半步化虚后置调校**：① 接入遥测监测"卡在半步化虚"玩家比例，据数据调整 `plan-tribulation-v1` 中的占位 buff（+10% 真元上限 / +200 年寿元）至合理强度；② 实装**重渡升级路径**——化虚名额空出时主动通知半步化虚玩家，允许跳过自然前置条件直接重触渡虚劫。来源：`plan-tribulation-v1` §8 占位标注。

**worldview 锚点**：
- §三:78 天道对化虚的主动关注——名额是天道的约束机制，非惩罚；卡关有解
- §五:506 末土后招原则——半步化虚是"卡关"而非终点，重渡是正当路径
- §三:124 NPC 与玩家平等受名额约束——NPC 半步化虚路径待 v2 对齐

**交叉引用**：
- `plan-tribulation-v1` ✅（`DuXuOutcomeV1::HalfStep` + `AscensionQuotaStore` + §8 占位标注）
- `plan-tribulation-v2` ✅（绝壁劫，不动 HalfStep 路径，本 plan 不冲突）
- `plan-void-quota-v1` ✅（化虚世界灵气预算名额底层逻辑，`AscensionQuotaStore` 实装来源）

---

## 接入面 Checklist

- **进料**：
  - `cultivation::tribulation::DuXuOutcomeV1::HalfStep`（v1 已实装，`HalfStepBuff` component 已挂载）
  - `cultivation::tribulation::AscensionQuotaStore`（名额增减 event，v1 已建）
  - server stdout log + Redis `bong:telemetry` hash（新建遥测上报路径）
- **出料**：
  - `TribulationQuotaOpenedEvent` 🆕 → agent narration + client HUD banner
  - `ReAscensionEligibleComponent` 🆕（标记可重渡的半步化虚玩家，挂载于 PlayerEntity）
  - 调整后的 `qi_physics::constants::HALF_STEP_QI_MAX_BONUS` / `HALF_STEP_LIFESPAN_BONUS`（从 tribulation 模块迁入 qi_physics 统一管理）
- **共享类型**：复用 `AscensionQuotaStore` + v1 `on_quota_released` hook，不新建名额系统
- **跨仓库契约**：
  - server: `cultivation::tribulation` 扩展 `ReAscension` 检查 + 名额通知 system + telemetry emit
  - agent: narration template `quota_opened_halfstep`（天道冷漠语调，示例见 §0 narration 规格）
  - client: `BannerHud::QuotaOpened` 类型 HUD banner（5s 显示，可交互触发重渡流程）
- **worldview 锚点**：§三:78 + §五:506
- **qi_physics 锚点**：`HALF_STEP_QI_MAX_BONUS` / `HALF_STEP_LIFESPAN_BONUS` 常数迁入 `qi_physics::constants` 统一管理（不允许在 tribulation 模块内写字面量，符合 docs/CLAUDE.md §四 红旗约束）

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | 遥测接入：监测半步化虚玩家比例 + Redis 上报 | ⬜ |
| P1 | buff 调参：按 P0 数据将 `HALF_STEP_*` 常数迁入 qi_physics 并调整 | ⬜ |
| P2 | 重渡机制：名额空出通知 + `ReAscensionEligible` 快速重渡通道 | ⬜ |

---

## §0 设计轴心

**buff 强度基线**（P1 调整前先确认 P0 遥测数据）：

| 参数 | v1 占位值 | 调参目标 |
|------|-----------|----------|
| `HALF_STEP_QI_MAX_BONUS` | +10% | "安慰奖"但不破坏化虚稀缺感；观察与通灵圆满极限的差距 |
| `HALF_STEP_LIFESPAN_BONUS` | +200 年 | 按不同境界玩家寿元基数（通灵 ~500 年）评估相对强度 |

**P0 遥测指标**：

- `half_step_player_ratio`：当前服务器内半步化虚玩家 / 全体玩家（Redis `bong:telemetry.half_step_ratio`，每 5min 刷新）
- `half_step_wait_duration`：各半步化虚玩家等待名额的时长分布（histogram，p50 / p95）
- 阈值判断：`half_step_player_ratio > 0.3` 持续 > 1h → buff 偏弱，需上调

**重渡机制设计**（P2）：

- `AscensionQuotaStore` 在名额 `+1` 时：
  1. 查询服务器内所有携带 `ReAscensionEligibleComponent` 的玩家
  2. 按"卡关时长"排序（最久者优先）
  3. emit `TribulationQuotaOpenedEvent { eligible_players: Vec<PlayerUuid>, quota_remaining: u8 }`
- server → client：`BannerHud::QuotaOpened`（显示 5s，含"立即重渡"按钮，点击发 `/reascension request` 命令）
- agent narration（zone broadcast）：
  - style: perception + narrative，scope: zone（仅在 quota_opened 玩家所在 zone broadcast）
  - 示例 1：「此域又空出一位化虚的位置。那个在通灵圆满滞留了七十年的修士，想必已在等待了。」
  - 示例 2：「名额松动。滞留者的机会又来了一次。」
  - 示例 3：「空了。去不去，随你。」
- 玩家选择重渡时（点击 HUD 或命令）：
  - 跳过自然前置检查（无需重满修炼条件），直接进入渡虚劫排期
  - 消耗占位（暂定：无额外代价，见 §5 开放问题 #2）
  - `ReAscensionEligibleComponent` 移除，防止重复触发

**视听规格（P2 HUD banner）**：

- 类型：`BannerHud::QuotaOpened`
- 颜色：金色 `#E8C87A` 文字 + 深蓝 `#1A2744` 背景 overlay，opacity 0.85
- 显示时长：300 tick（5s），淡入 20 tick / 淡出 40 tick
- 文案：「修炼名额有空余——汝之机会已至」（主文）+ 「[立即重渡]」（可点击按钮）
- 仅对 `ReAscensionEligibleComponent` 玩家显示，不全服广播

---

## §5 开放问题（P0 决策门收口）

1. `HALF_STEP_QI_MAX_BONUS` 上调上限：+20% 是否过强（令半步化虚在某些 build 下优于真化虚）？
2. 重渡是否需要额外代价（骨币 / 灵石 / 业力消耗）？worldview §三:78 未明确，由 P2 设计门决定
3. 多个半步化虚玩家同时等待时，名额空出优先权："卡关最久"vs"先到先得（最近一次尝试）"？
4. 半步化虚 buff（`HalfStepBuff`）在重渡成功真化虚后是否保留（真化虚 buff + 半步 buff 叠加）？
5. NPC 半步化虚路径：v1 `AscensionQuotaStore` 已对 NPC 判断，但 NPC 无 HUD；通知方式留 v2 对齐（`plan-tribulation-balance-v2` 占位）
