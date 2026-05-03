# Bong · plan-anticheat-v1 · Active

> **状态**：⏳ active（2026-05-04 升级，user 拍板）。前置 plan-combat-no_ui ✅ finished + plan-agent-v2 ✅ finished，无依赖阻塞。开放问题全为上线后阈值调整级，不是 P0 block。

反作弊上报系统：`AntiCheatCounter` component + `CHANNEL_ANTICHEAT` Redis 推送。对应 `plan-combat-no_ui.md §1.5.6`（finished）中设计但未实装的模块。

**世界观锚点**：无直接世界观对应——纯服务端运维基础设施，不影响玩家可见玩法。

**交叉引用**：
- `plan-combat-no_ui.md`（finished）§1.5.6 — 原始设计：reach/cooldown/qi_invest 三道 clamp 分散在 `server/src/combat/resolve.rs`，无统一违规计数和上报通道
- `plan-agent-v2.md`（finished）— agent 消费 CHANNEL_ANTICHEAT 做异常记录（本 plan 仅推送，不定义消费端行为）

---

## 接入面 Checklist

- **进料**：现有 `server/src/combat/resolve.rs` 中三道 clamp（reach、cooldown、qi_invest）的违规判定结果
- **出料**：`AntiCheatViolationEvent` → Redis `bong:anticheat` → 运维侧消费（非玩家可见）
- **共享类型**：新增 `AntiCheatCounter` ECS component；新增 `AntiCheatViolationReport` IPC schema
- **跨仓库契约**：
  - server：`server/src/combat/anticheat.rs`（新文件）
  - agent/schema：`agent/packages/schema/src/anticheat.ts`（新文件，TypeBox）
  - client：**无**（纯 server-side，玩家不可见）
- **worldview 锚点**：无（运维基础设施）

---

## §0 设计轴心

- [ ] **不封禁，只上报**——ban 决策交给运维人工判断，server 只做计数 + 推送
- [ ] **三道 clamp 统一**——当前 reach/cooldown/qi_invest 三个违规判定分散在 `resolve.rs`，本 plan 整合到统一 `AntiCheatCounter` 并给每类维护独立计数
- [ ] **阈值可配置**——通过 `server/assets/config/anticheat.toml` 配置，不硬编码
- [ ] **日志与 Redis 双写**——达阈值时写 server log（ERROR 级别）+ 推 `bong:anticheat` channel
- [ ] **不影响战斗流程**——违规被 clamp 后战斗照常处理（已有行为不变），仅额外做计数

---

## §1 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ⬜ | `AntiCheatCounter` ECS component + 三道 clamp 计数接入 | 单元：三类 violation 计数单测；clamp 前后行为不变测试 |
| **P1** ⬜ | 阈值触发 + `bong:anticheat` Redis 推送 + server log | 集成：模拟 N 次违规 → channel 收到上报 |
| **P2** ⬜ | `anticheat.ts` TypeBox schema + JSON Schema 导出 + Rust serde 对齐 | schema 双端 roundtrip 测试 |

---

## §2 核心数据契约

```rust
// server/src/combat/anticheat.rs（新文件）

#[derive(Component, Default)]
pub struct AntiCheatCounter {
    pub reach_violations: u32,
    pub cooldown_violations: u32,
    pub qi_invest_violations: u32,
    pub last_report_tick: u64,
}

pub struct AntiCheatViolationReport {
    pub char_id: String,
    pub entity_id: u64,
    pub at_tick: u64,
    pub kind: ViolationKind,
    pub count: u32,           // 本次推送时的累计数
    pub details: String,      // e.g. "reach: client=6.2 server_max=4.0"
}

pub enum ViolationKind { ReachExceeded, CooldownBypassed, QiInvestExceeded }
```

```typescript
// agent/packages/schema/src/anticheat.ts
import { Type, Static } from "@sinclair/typebox";

export const ViolationKindV1 = Type.Union([
  Type.Literal("reach_exceeded"),
  Type.Literal("cooldown_bypassed"),
  Type.Literal("qi_invest_exceeded"),
]);

export const AntiCheatReportV1 = Type.Object({
  type: Type.Literal("anticheat_report"),
  char_id: Type.String(),
  at_tick: Type.Number(),
  kind: ViolationKindV1,
  count: Type.Number(),
  details: Type.String(),
});
```

**Redis Channel**：`bong:anticheat`（对应 `server/src/schema/channels.rs::CH_ANTICHEAT`）

**阈值配置**（`server/assets/config/anticheat.toml`）：
```toml
[anticheat]
reach_threshold = 10        # 触发上报的单实体累计次数
cooldown_threshold = 5
qi_invest_threshold = 20
report_cooldown_ticks = 1200   # 同实体两次上报间最小间隔（60s）
```

---

## §3 接入点（resolve.rs 改动最小化）

现有 `server/src/combat/resolve.rs` 的三道 clamp 位置：
- **reach clamp**：攻击距离校验（clamp 超距）
- **cooldown clamp**：攻击冷却校验（拒绝过快攻击）
- **qi_invest clamp**：真元投入上限校验

P0 改动：在每道 clamp 判定后插入一行 `anticheat_counter.{kind}_violations += 1`，不修改 clamp 本身逻辑。

---

## §4 开放问题

- [ ] 阈值合理性：初始值（10/5/20 次）仅为占位，需上线后根据正常玩家行为分布回归调整
- [ ] 同实体重复上报去重窗口：当前设 1200 tick（60s），是否够长？
- [ ] 运维消费端：agent 消费 `bong:anticheat` 后如何记录？是否需要独立运维面板（本 plan 不定义，留运维 plan）

---

## §5 进度日志

- 2026-05-01：从 plan-combat-no_ui reminder 整理立项。现有代码：三道 clamp 分散在 `server/src/combat/resolve.rs` ✅（已有行为）；`AntiCheatCounter` component / `bong:anticheat` channel / `anticheat.ts` 均未实装。
- **2026-05-04**：skeleton → active 升级（user 拍板，技术 plan 无 worldview 阻塞）。下一步起 P0 worktree（AntiCheatCounter ECS + 三道 clamp 计数接入 + bong:anticheat channel）。

## Finish Evidence

### 落地清单

- P0：`server/src/combat/anticheat.rs` 新增 `AntiCheatCounter` component、阈值配置加载、阈值上报系统；`server/src/combat/resolve.rs` 接入 reach / cooldown / qi_invest 三类违规计数，不改变原战斗结算结果。
- P1：`server/src/network/anticheat_bridge.rs` 桥接 `AntiCheatViolationEvent` 到 Redis outbound；`server/src/network/redis_bridge.rs` 发布 `AntiCheatReportV1` 到 `bong:anticheat`；`server/assets/config/anticheat.toml` 提供 10 / 5 / 20 / 1200 初始阈值。
- P2：`agent/packages/schema/src/anticheat.ts`、`agent/packages/schema/generated/*.json`、`server/src/schema/anticheat.rs` 对齐 TypeBox / JSON Schema / Rust serde 契约；`agent/packages/schema/src/channels.ts` 与 `server/src/schema/channels.rs` 对齐 `bong:anticheat`。

### 关键 commit

- `77af1859` · 2026-05-04T02:26:29+12:00 · `实现反作弊计数与上报`

### 测试结果

- `cd server && cargo fmt --check` ✅
- `cd server && cargo clippy --all-targets -- -D warnings` ✅
- `cd server && cargo test anticheat` ✅ 9 passed
- `cd server && cargo test` ✅ 2172 passed
- `cd agent && npm run build` ✅
- `cd agent/packages/schema && npm test` ✅ 9 files / 265 tests passed

### 跨仓库核验

- server：`AntiCheatCounter`、`AntiCheatViolationEvent`、`emit_anticheat_threshold_reports`、`RedisOutbound::AntiCheatReport`、`CH_ANTICHEAT`。
- agent/schema：`AntiCheatReportV1`、`ViolationKindV1`、`CHANNELS.ANTICHEAT`、`anticheat-report-v1.json`。
- client：无改动，符合本 plan “纯 server-side，玩家不可见”范围。

### 遗留 / 后续

- 阈值合理性、1200 tick 去重窗口、运维消费端记录/面板均为上线后观测项，不在本 plan 范围内。
