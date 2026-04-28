# Bong · plan-combat-anticheat-v1 · 骨架

**战斗反作弊层**：将 plan-combat-no_ui 遗留的反作弊计数统一化——把分散在 `resolve.rs` 的 reach / cooldown / qi_invest 三道 clamp 升级为统一的 `AntiCheatCounter` component，并新增 `bong:anticheat` Redis 推送，支持 GM 审查。

**前置**：`plan-combat-no_ui`（finished；`server/src/combat/resolve.rs` 三道 clamp 已实装）

**世界观锚点**：无直接世界观对应（服务器治理层，对玩家透明，不影响叙事）

**交叉引用**：
- `plan-combat-no_ui`（finished；`resolve.rs` reach/cooldown/qi_invest clamp 源文件）
- `plan-server-cmd-system-v1`（GM 命令系统；P1 `/anticheat` 命令依赖）
- `plan-social-v1`（active；若有玩家举报系统可接入）

**阶段总览**：
- P0 ⬜ `AntiCheatCounter` component + 三道 clamp 接入计数 + warn log
- P1 ⬜ `bong:anticheat` Redis 推送 + GM 查询命令

---

## §0 设计轴心

- [ ] **不改游戏体验**：所有 clamp 逻辑不变，只在 clamp 命中时额外 ++ counter；不增加任何延迟或拒绝
- [ ] **统一可审查**：GM 可通过 admin channel 查看某玩家的 `AntiCheatCounter` 快照
- [ ] **不自动踢出**（P0/P1 范围）：计数超阈值时只 warn log + 推 Redis，人工 or 后续 plan 实装自动处罚
- [ ] **周期性重置**：5 分钟一个窗口，防止旧违规永远累积；每个窗口超阈值才上报

---

## §1 `AntiCheatCounter` Component（P0）

**现状**：`server/src/combat/resolve.rs` 中 reach / cooldown / qi_invest 三道 clamp 触发时各有 `warn!` log，但无统一 component，无计数，无跨帧聚合。

**实装**：

```rust
// server/src/combat/anticheat.rs（新文件）
#[derive(Component, Default)]
pub struct AntiCheatCounter {
    pub reach_violations: u32,
    pub cooldown_violations: u32,
    pub qi_invest_violations: u32,
    pub window_start_tick: u64,
    pub window_ticks: u64,   // 默认 6000 = 5 分钟 × 20 TPS
}

impl AntiCheatCounter {
    pub fn is_suspicious(&self) -> bool {
        self.reach_violations > 5
            || self.cooldown_violations > 10
            || self.qi_invest_violations > 20
    }

    pub fn reset(&mut self, current_tick: u64) {
        self.reach_violations = 0;
        self.cooldown_violations = 0;
        self.qi_invest_violations = 0;
        self.window_start_tick = current_tick;
    }
}
```

- `resolve.rs` 三处 clamp 命中位置各增加一行计数：
  - reach clamp → `counter.reach_violations += 1`
  - cooldown clamp → `counter.cooldown_violations += 1`
  - qi_invest clamp → `counter.qi_invest_violations += 1`
- `anticheat_tick_system`（新 system）：每 tick 检查 `current_tick - window_start_tick >= window_ticks` 时调用 `reset`，重置前若 `is_suspicious()` → warn log `[ANTICHEAT] char {} suspicious: reach={}, cd={}, qi={}`

**可核验交付物**：
- `server/src/combat/anticheat.rs` `AntiCheatCounter` + `is_suspicious` + `reset`
- `server/src/combat/resolve.rs` 三处 clamp 命中时 ++ counter（diff 仅 +3 行）
- `anticheat_tick_system` 注册（`register(&mut app)`）
- `combat::anticheat::*` 单测（至少 8 条）：
  - reach clamp 命中 1 次 → `reach_violations == 1`
  - cooldown clamp 命中 → `cooldown_violations` 递增
  - qi_invest clamp 命中 → `qi_invest_violations` 递增
  - `is_suspicious`：`reach > 5` → true；`reach = 5` → false（边界）
  - `is_suspicious`：`cooldown > 10` → true；`qi_invest > 20` → true
  - `reset` 后三个计数器归零
  - 窗口期满后自动 reset
  - 合法攻击（clamp 未触发）→ 计数不变

---

## §2 `bong:anticheat` Redis 推送（P1）

**Schema**：

```typescript
// agent/packages/schema/src/anticheat.ts（新文件）
export const AntiCheatEventV1 = Type.Object({
  v: Type.Literal(1),
  char_id: Type.String(),
  violations: Type.Object({
    reach: Type.Number(),
    cooldown: Type.Number(),
    qi_invest: Type.Number(),
  }),
  window_start_tick: Type.Number(),
  suspicious: Type.Boolean(),
});
```

- Redis channel：`bong:anticheat`（publish only；agent 可选择性订阅用于异常行为归档）
- **触发条件**：窗口期满 + `is_suspicious()` 为 true 时 publish 一次（不在每次 clamp 时推，避免高频噪音）
- **GM 查询命令** `/anticheat <player>`：
  - 接入 `plan-server-cmd-system-v1` admin 命令路由
  - 返回当前 `AntiCheatCounter` 快照（当前窗口 + 上一窗口是否 suspicious）
  - 仅 GM 权限可用（非玩家可查）

**可核验交付物**：
- `agent/packages/schema/src/anticheat.ts` `AntiCheatEventV1` schema + 单测 round-trip
- `anticheat_publish_system`（server，suspicious 窗口结束时 publish）
- GM 命令 `/anticheat` 接入 `plan-server-cmd-system-v1`（返回快照 JSON）

---

## §3 数据契约

| 契约 | 位置 |
|---|---|
| `AntiCheatCounter` component | `server/src/combat/anticheat.rs`（新文件）|
| `anticheat_tick_system` | `server/src/combat/anticheat.rs`（新 system）|
| reach / cooldown / qi_invest clamp 接入点 | `server/src/combat/resolve.rs`（+3 行）|
| `AntiCheatEventV1` schema | `agent/packages/schema/src/anticheat.ts`（新文件）|
| `bong:anticheat` Redis channel | publish-only，agent 可选订阅 |

---

## §4 实施节点

- [ ] **P0**：`anticheat.rs` component + `is_suspicious` + `reset` + `anticheat_tick_system` + `resolve.rs` 三处 +1 + 单测（≥8）
- [ ] **P1**：`anticheat.ts` schema + `anticheat_publish_system`（suspicious 窗口推送）+ GM `/anticheat` 命令

---

## §5 开放问题

- [ ] 阈值（reach > 5 / cooldown > 10 / qi_invest > 20 per 5 分钟）是否合理？需 play-testing 后调整
- [ ] 是否需要"降权"而非只 warn（如：超阈值玩家战斗伤害减半或进入观察模式）？（建议 P0 只 warn，自动降权另立 plan）
- [ ] `AntiCheatCounter` 的窗口重置是否需要两级窗口（短窗口用于检测爆发 / 长窗口用于检测持续行为）？
- [ ] 历史记录：是否需要保存最近 N 个 suspicious 窗口的快照供 GM 事后审查（而非只看当前窗口）？
- [ ] 三道 clamp 阈值的合法频率边界：高延迟玩家 reach clamp 可能被误判，是否需要延迟补偿？

---

## §6 进度日志

- 2026-04-28：骨架立项。来源：`docs/plans-skeleton/reminder.md` plan-combat-no_ui 节（`AntiCheatCounter component + CHANNEL_ANTICHEAT 推送`）。代码核查：`server/src/combat/resolve.rs` 已有 reach/cooldown/qi_invest 三道 warn log，无统一 component 和 Redis 上报，`AntiCheatCounter` 不存在。
