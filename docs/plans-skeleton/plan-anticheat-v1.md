# Bong · plan-anticheat-v1 · 骨架

**战斗防作弊子系统**。统一 reach/cooldown/qi_invest 违规检测——从 `resolve.rs` 分散 clamp 中提取为 `AntiCheatCounter` component，加 Redis 上报通道 `bong:anticheat_violation`，以及可选的天道业力惩戒接入。

**世界观锚点**：直接锚点为空（纯技术系统）。叙事层可借用`worldview.md §八`（"天道全知，无事不察"——违规检测即天道注视的实现）。

**接入面**：
- **进料**：`server/src/combat/resolve.rs` 现有 reach decay clamp / cooldown guard / qi_invest 合规检查 → 提取到统一检测层
- **出料**：`AntiCheatViolationEvent` → Redis `bong:anticheat_violation` → agent 可选消费（业力标记）
- **共享类型**：复用 `combat::resolve` 攻击流程，不引入新 ECS 系统；新增 component 独立存在
- **worldview 锚点**：§八 天道行为准则（全知叙事）

**交叉引用**：`plan-combat-no_ui`（完成，基座 resolve.rs）· IPC schema 体系（TypeBox + serde 双端）· `plan-tribulation-v1`（定向天罚 = 天道对高消耗行为的响应，与 anticheat 共享"天道注意阈值"语义）

---

## §0 设计轴心

- [ ] **不破坏现有 clamp 逻辑**——只是在静默 clamp 的同时增加计数和上报
- [ ] **server-authoritative**：客户端发什么服务端都校验，违规被 reject + 记录
- [ ] 违规上报给 agent 是**可选语义增强**（天道业力 narration），不是核心功能
- [ ] 阈值触发，非实时上报——避免 Redis 洪流

## §1 AntiCheatCounter component（P0）

**阶段状态**：⬜

**可核验交付物**：
- `AntiCheatCounter` component（`server/src/combat/components.rs`）：
  ```rust
  #[derive(Component, Default)]
  pub struct AntiCheatCounter {
      pub reach_violations: u32,
      pub cooldown_violations: u32,
      pub qi_violations: u32,
      pub last_violation_tick: u64,
      pub total_violations: u32,
  }
  ```
- 加入玩家 spawn bundle（`combat/mod.rs:81` 和 `104`）
- 加入 NPC spawn bundle（`combat/mod.rs:133` 附近）
- 测试：`anticheat::counter_defaults_zero`、`counter_bundle_in_player_spawn`（2 单测）

## §2 违规检测统一化（P1）

**阶段状态**：⬜

**可核验交付物**：
- `server/src/combat/anticheat.rs` 新模块（或作为 `resolve.rs` 内的独立 fn）：
  - `fn check_reach_violation(intent: &AttackIntent, distance: f32) -> bool`
  - `fn check_cooldown_violation(intent: &AttackIntent, last_attack_tick: u64, current_tick: u64) -> bool`
  - `fn check_qi_violation(intent: &AttackIntent, qi_current: f64) -> bool`
- `resolve.rs` 攻击事务中调用上述检查：违规时 `counter.{reach,cooldown,qi}_violations += 1` + `total_violations += 1` + `last_violation_tick = current_tick`，**仍然执行现有 clamp（不改行为）**
- `AntiCheatViolationEvent { entity: Entity, violation_type: ViolationType, severity: f32, tick: u64 }` event
- 累积触发：`total_violations >= ANTICHEAT_THRESHOLD`（配置常量，建议 5）→ emit `AntiCheatViolationEvent`，reset counter
- 测试：`anticheat::reach_violation_increments_counter`、`cooldown_violation_increments_counter`、`qi_violation_increments_counter`、`threshold_emits_event`、`counter_resets_after_emit`、`no_violation_no_event`（6 单测）

## §3 CHANNEL_ANTICHEAT + schema（P2）

**阶段状态**：⬜

**可核验交付物**：
- TypeBox schema（`agent/packages/schema/src/anticheat.ts`）：
  ```typescript
  export const AntiCheatViolationV1 = Type.Object({
    v: Type.Literal(1),
    player_id: Type.String(),
    violation_type: Type.Union([
      Type.Literal("reach"),
      Type.Literal("cooldown"),
      Type.Literal("qi_invest"),
    ]),
    severity: Type.Number({ minimum: 0 }),
    total_violations_at_emit: Type.Integer({ minimum: 1 }),
    tick: Type.Integer({ minimum: 0 }),
  });
  ```
- Rust serde struct `AntiCheatViolationV1`（`server/src/schema/anticheat.rs`）
- Redis publisher：`AntiCheatViolationEvent` → serialize → `bong:anticheat_violation` PUBLISH
- 双端 schema 对齐测试（`agent/packages/schema`）+ Rust serde roundtrip
- 测试：`schema::anticheat_v1_serde`、`schema_pin_violation_type_variants`（2 schema 单测）

## §4 天道业力标记（P3，可选）

**阶段状态**：⬜

**可核验交付物**：
- agent `packages/tiandao/src/anticheat-runtime.ts`（参照 death-insight-runtime.ts 模式）：
  - 订阅 `bong:anticheat_violation`
  - 高严重度违规（`severity > 2.0`）→ emit 天道 narration（`"此地有人逆天而为，天道甚觉有趣"`）
  - 低严重度 → 记录进 agent 内部日志，不对外广播
- **业力标记**（可选进一步实装）：高违规玩家进入定向天罚候选池（plan-tribulation §1 定向天罚接口）
- 测试：`anticheat_runtime_high_severity_emits_narration`（1 单测）

## §5 开放问题

- [ ] `ANTICHEAT_THRESHOLD = 5` 是否合理？低阈值误报正常高频玩家，高阈值漏报真实作弊
- [ ] 违规计数是否需要**时间窗口**（N violations in last M seconds）而非累计？
- [ ] NPC 的 AntiCheatCounter 是否有意义（NPC 由 server 发 AttackIntent 本身就是授信的）？
- [ ] 上报给 agent 的 narration 是否应对被违规攻击的**受害者**也发一条遗念（"被奇怪的力量刺伤"）？
