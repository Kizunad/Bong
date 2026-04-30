# Bong · plan-combat-anticheat-v1 · 骨架

**战斗反外挂统一计数与上报**——plan-combat-no_ui §1.5.6 设计原文已存在但未实装。当前 reach / cooldown / qi_invest 三道 clamp 分散在 `resolve.rs` 内，违规事件被静默纠正、无统一计数与运维侧上报通道。本 plan 补齐 `AntiCheatCounter` ECS component + `CHANNEL_ANTICHEAT` Redis 推送闭环。

**世界观锚点**：无新世界观——纯运维 / 安全工程层。`worldview.md` 不需改动。

**代码现状**：
- ✅ `server/src/schema/channels.rs:CH_ANTICHEAT = "bong:anticheat"`（推测；finished plan 引用了 `CHANNEL_ANTICHEAT = "bong:anticheat"` 常量名，需核对实际 const 名称）
- ❌ `server/src/combat/anticheat.rs` — 不存在（finished plan 设计时列出但未落地）
- ❌ `agent/packages/schema/src/anticheat.ts` — 不存在
- 三道 clamp 逻辑分散在 `server/src/combat/resolve.rs`：
  - reach 校验：`AttackReach` 距离过滤
  - cooldown 校验：`CombatState` 冷却窗口
  - qi_invest 校验：投入真元封顶（防客户端伪造高伤害）

**交叉引用**：`plan-combat-no_ui`（已归档，§1.5.6 设计原文）· `plan-combat-ui_impl`（active，UI 层）· 未来运维 plan（消费 CHANNEL_ANTICHEAT）

---

## §1 设计原则（继承 plan-combat-no_ui §1.5.6）

- **server 是真相**：所有 clamp 静默纠正客户端 hint，不向客户端报错（防外挂作者反推阈值）
- **可观察 ≠ 可惩罚**：本 plan 只做记录 + 上报，不做封禁 / 减益（封禁交运维 plan）
- **聚合 > 实时**：阈值触发推 Redis 通道，不是每次违规都推（防风暴）
- **kind 分类**：每类 clamp 独立计数，便于运维诊断不同攻击向量

---

## §2 ECS Component / 事件定义

**`server/src/combat/anticheat.rs`**（新模块）：

- [ ] `AntiCheatCounter` component（每个 entity 一份）：
  ```rust
  pub struct AntiCheatCounter {
      pub by_kind: HashMap<AntiCheatKind, u32>,
      pub last_report_tick: HashMap<AntiCheatKind, u64>,
  }
  pub enum AntiCheatKind {
      ReachExceeded,        // 客户端 hint 超出 server 计算的攻击距离
      CooldownBypass,       // 攻击间隔小于 server cooldown
      QiInvestOverflow,     // 投入真元超出真元池上限
      DefenseTimingFraud,   // 防御 hint 时间戳不在合法窗口
      // 未来扩展：移动速度、视角抖动、tick rate 等
  }
  ```
- [ ] `AntiCheatViolation` Bevy Event：`{ entity: Entity, kind: AntiCheatKind, hint_value: f32, server_value: f32, tick: u64 }`
- [ ] 阈值常量（可调）：每 kind 独立 `report_threshold: u32`（默认 5 次累计 → 推一次）+ `cooldown_ticks: u64`（防同 kind 风暴）

---

## §3 接入点（resolve.rs 三道 clamp）

将散落的 clamp 改为统一调用：

- [ ] **reach 接入**：`resolve_attack_intents` 距离过滤分支 → 当 hint 距离 > server 计算时 emit `AntiCheatViolation { kind: ReachExceeded }`，server 距离值仍走原 clamp
- [ ] **cooldown 接入**：`CombatState` 冷却检查 → 同上 emit `CooldownBypass`
- [ ] **qi_invest 接入**：投入真元上限校验 → emit `QiInvestOverflow`
- [ ] **defense 时序接入**：`apply_defense_intents` 防御窗口校验 → emit `DefenseTimingFraud`
- [ ] 三道 clamp 现有行为不变（continue silent clamp），仅新增 `AntiCheatViolation` event emit

---

## §4 计数累加 + 阈值推送

**`server/src/combat/anticheat.rs::anticheat_tick` 系统**：

- [ ] 消费 `AntiCheatViolation` events → 累加 entity 的 `AntiCheatCounter.by_kind[kind] += 1`
- [ ] 检查阈值：`counter[kind] >= report_threshold` && `current_tick - last_report_tick[kind] > cooldown_ticks`
  - 触发 → 发 `AntiCheatReport { entity_id, kind, count, hint_sample, server_sample, ticks_since_last }` 到 Redis outbound
  - 重置该 kind counter 为 0（或衰减），更新 last_report_tick
- [ ] 系统注册：`mod.rs` 加 `anticheat_tick` 到 `EmitSet`（在 combat_bridge 之后）

---

## §5 Redis 通道 + Schema

- [ ] `server/src/schema/channels.rs`：确认 `CH_ANTICHEAT = "bong:anticheat"` 已存在；若无则补
- [ ] `RedisOutbound::AntiCheatReport(AntiCheatReportV1)` 变体（`server/src/network/redis_bridge.rs`）
- [ ] `agent/packages/schema/src/anticheat.ts`：TypeBox 定义 `AntiCheatReportV1`：
  ```typescript
  Type.Object({
    v: Type.Literal(1),
    entity_id: Type.String(),
    kind: Type.Union([Literal('reach_exceeded'), ...]),
    count: Type.Number(),
    hint_sample: Type.Number(),
    server_sample: Type.Number(),
    ticks_since_last: Type.Number(),
    at_tick: Type.Number(),
  })
  ```
- [ ] `agent/packages/schema/samples/anticheat-report.sample.json` 双端往返样本
- [ ] `agent/packages/schema/src/schema-registry.ts` 注册

---

## §6 实施节点

- [ ] **P0**：`AntiCheatKind` 枚举 + `AntiCheatCounter` component + `AntiCheatViolation` event
- [ ] **P1**：三道 clamp 接入（reach / cooldown / qi_invest） + defense 时序
- [ ] **P2**：`anticheat_tick` 阈值聚合 + Redis 推送 + schema 双端同步
- [ ] **P3**：饱和单测（每 kind 触发 + 阈值聚合 + 通道节流 + sample 往返）

---

## §7 饱和测试要求（CLAUDE.md 测试原则）

- [ ] **每 kind 一条独立 case**：4 种 `AntiCheatKind` 全覆盖，违规事件正确累加
- [ ] **阈值聚合**：threshold=5，前 4 次违规无 Redis 推送；第 5 次触发 push
- [ ] **节流冷却**：阈值触发后 cooldown 内再 5 次违规不推第二次
- [ ] **多 kind 独立**：同 entity 触发 ReachExceeded 不影响 CooldownBypass 计数
- [ ] **schema 往返**：`AntiCheatReportV1` server JSON ↔ agent TypeBox 一致性
- [ ] **clamp 行为不变**：开启 anticheat 后 reach 仍按 server 值结算（hint 不污染战斗结果）

---

## §8 开放问题

- [ ] threshold / cooldown 默认值如何设定？（待真实玩家行为数据）
- [ ] `hint_sample` / `server_sample` 是否需要脱敏（防外挂作者通过日志反推阈值）？
- [ ] entity_id 用 `EntityId`（Valence）还是 `character_id`（持久化）？运维侧需要哪种？
- [ ] 是否分级：minor（reach 误差 < 0.5m）vs severe（reach 误差 > 5m）？

---

## §9 进度日志

- 2026-04-29：骨架立项——补齐 plan-combat-no_ui §1.5.6 未落地的 `AntiCheatCounter` + `CHANNEL_ANTICHEAT` 闭环。schema 通道常量已在 `channels.rs` 占位，core 实现（component / event / tick / Redis push）全部缺。
