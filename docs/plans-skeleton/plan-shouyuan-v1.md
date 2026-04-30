# Bong · plan-shouyuan-v1 · 骨架

**寿元续命尾项**——plan-death-lifecycle-v1 §4c 遗留的两处未实装机制：坍缩渊换寿（续命路径之一）+ 善终/横死 DeathType 分类字段。续命丹（§4c P6）与夺舍（§4e P7）已在 commit `3ad73f90` 落地，本 plan 仅覆盖剩余缺口。

**世界观锚点**：`worldview.md §十二`（寿元系统）· `worldview.md §七`（坍缩渊/负灵域寿元加速流逝 ×2.0）· `plan-death-lifecycle-v1 §4c/§4e`（设计原文，已归档）

**代码现状**：
- `server/src/cultivation/lifespan.rs`：寿元 tick / 风烛 buff / 续命丹 / 夺舍 — ✅ 已落地
- `server/src/cultivation/life_record.rs:74`：`BiographyEntry::Terminated { cause: String }` — 仅有 cause 字符串，无 DeathType 枚举
- 坍缩渊（TSY）维度 `server/src/world/tsy.rs` 已存在，换寿钩子尚无

**交叉引用**：`plan-death-lifecycle-v1`（已归档）· `plan-alchemy-v2`（续命丹配方正典化）· `plan-tsy-zone-v1`（坍缩渊现有基建）· `plan-social-v1`（亡者博物馆 — 消费 DeathType 分类）

---

## §1 善终 / 横死 DeathType 字段

`BiographyEntry::Terminated.cause` 是自由字符串，亡者博物馆无法按死因分类。

- [ ] `DeathKind` 枚举（`server/src/cultivation/life_record.rs`）：
  ```rust
  pub enum DeathKind {
      Natural,          // 寿元耗尽，老死
      Violent,          // 战斗/横死，含凶手 ID
      SelfChosen,       // 玩家主动选择终结（tag=自主归隐，不掉物品）
      Possessed,        // 被夺舍（夺舍者 ID）
  }
  ```
- [ ] `BiographyEntry::Terminated` 追加 `kind: DeathKind`；旧序列化兼容（`#[serde(default)]`）
- [ ] `BiographyEntry::Terminated { kind: DeathKind::Violent, .. }` 携带 `attacker_id: Option<String>`
- [ ] `death_arbiter_tick` 写入 Terminated 时按死因填充 kind（`lifecycle.rs:1285`）
- [ ] 老死路径（`lifespan.rs` 寿元耗尽触发）填 `DeathKind::Natural`
- [ ] 单测：四种 kind 各一条序列化往返 + 旧格式（无 kind 字段）反序列化默认值

---

## §2 坍缩渊换寿

death-lifecycle §4c 定义了三条续命路径，坍缩渊换寿是第二条（基础代价 = 修为进度）。

**设计约束（继承 §4c）**：
- 换寿不能突破当前境界寿元上限；代价随累计换寿量递增
- 坍缩渊本身寿元流逝 ×2.0 — 高风险环境天然限制"无限刷"
- 所有换寿事件写入 `lifespan_events`（数据库表已存在）

**实装方案**：

- [ ] `TsyLifespanExchangeIntent { character_id, years_requested: u32 }` Bevy Event
- [ ] `process_tsy_lifespan_exchange_intents` 系统（`server/src/world/tsy_drain.rs` 或新文件 `tsy_lifespan.rs`）：
  1. 校验角色在 TSY 维度（`DimensionPresence` 组件）
  2. 计算代价：`cultivation_progress_cost = base_tsy × years_requested × (1 + cumulative_exchanged / realm_cap)^k`（base_tsy 占位 0.05/年，k=1.5，平衡后调整）
  3. 扣修为进度（`CultivationProgress.insight_points -= cost`）；进度不足 → 退回请求
  4. 调 `apply_lifespan_extension`（`lifespan.rs:641`，已存在）填寿
  5. 写 `LifespanEventKindV1::Extension { source: "tsy_exchange" }` 进数据库
- [ ] 客户端发起请求：`ClientRequestV1::TsyLifespanExchangeRequest { years_requested }`（schema 扩展）
- [ ] 单测：正常换寿扣进度 + 进度不足拒绝 + 换寿上限校验 + TSY 维度外拒绝

---

## §3 实施节点

- [ ] **P0**：`DeathKind` 枚举 + `Terminated` 字段追加 + 序列化兼容测试
- [ ] **P1**：`TsyLifespanExchangeIntent` + exchange 系统 + client schema 扩展

---

## §4 开放问题

- [ ] `base_tsy` 换寿代价系数（0.05 修为/年）与整体修炼进度节奏的平衡 — 需 Phase 1 上线后测
- [ ] 坍缩渊内寿元 ×2.0 加速 + 换寿填充的净效是正还是负？（防"在 TSY 无限刷"）
- [ ] 亡者博物馆 UI 按 `DeathKind` 分类展示的界面稿（交 plan-social-v1）

---

## §5 进度日志

- 2026-04-29：骨架立项——覆盖 plan-death-lifecycle-v1 两处遗留缺口（坍缩渊换寿机制、善终/横死 DeathType 字段）；续命丹/夺舍/风烛 buff 已实装，不在本 plan 范围。
