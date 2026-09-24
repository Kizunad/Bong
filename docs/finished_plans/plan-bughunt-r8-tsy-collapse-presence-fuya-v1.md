# plan-bughunt-r8-tsy-collapse-presence-fuya-v1

> **已完成（2026-07-08，归档审计确认）**。

## 范围

本 plan 只处理 r8 TSY 局部机械项：

- #11：`TsyCollapseCompleted` 后仍残留 `TsyPresence`，导致玩家已被 collapse 死亡链路接管后仍被视作 TSY 内玩家，进而触发复活/入场锁异常。
- #12：Fuya 压力嗡鸣 stop sound 对任意 `DeathEvent` 发送，并且广播给 `All`，导致非 Fuya 死亡也会污染所有客户端音频状态。

明确不处理：

- #10 ghost entity：需要实体生命周期与可见性设计决策，本 plan 不顺手修。

## 约束

- 不引入新的真元/灵气流动路径，不新增 qi 物理公式或常数。
- 不改 TSY 死亡掉落主链路的 `DeathEvent { cause: "tsy_collapsed" }` 语义。
- 不扩大 Fuya 音频系统职责；只把 stop 事件限定到真实 Fuya 死亡，并收窄 recipient。
- 只补 targeted Rust tests，不做跨栈改动。

## TODO

- [x] #11 在 collapse completed 化灰玩家后清理对应 `TsyPresence`（拆成一次性 `PendingTsyDeathDrop`，见 `server/src/world/extract_system.rs:775-778`）。
- [x] #11 补测试：collapse completed 后玩家收到 `DeathEvent`，同时 `TsyPresence` 被移除，避免复活锁残留（`server/src/world/extract_system.rs:1326-1327` 等）。
- [x] #12 Fuya stop sound 只响应带 `FuyaAura` 的死亡实体（`server/src/npc/tsy_hostile.rs` 相关系统）。
- [x] #12 Fuya stop sound recipient 使用局部半径，不再 `All` 广播。
- [x] #12 补测试：非 Fuya `DeathEvent` 不发 stop；Fuya 死亡 stop recipient 为 Radius（`server/src/npc/tsy_hostile.rs:2441,2519,2526` 等）。
- [x] 运行 targeted Rust tests，并通过 read-only validator 审核（commit message 记录 e2e PASS + 0-context validator PASS）。

## Finish Evidence

### 落地清单

- **#11**：`server/src/inventory/mod.rs` 新增 `PendingTsyDeathDrop { presence: TsyPresence }`；`server/src/world/extract_system.rs` 的 collapse-completed 死亡路径把 live `TsyPresence` 拆成一次性 `PendingTsyDeathDrop` 并 `remove::<TsyPresence>()`，避免复活/入场 gate 继续把玩家判为 TSY 内实体；复活掉落优先消费该 pending 上下文以保持 `family_id` 与入场背包快照不变量。
- **#12**：`server/src/npc/tsy_hostile.rs` 的 Fuya 压力嗡鸣 stop sound 改为只绑定仍带 `FuyaAura` 的死亡目标，recipient 从 `All` 收窄为与播放一致的局部 `Radius`。

### 关键 commit

- `5b477b45`（2026-07-08）：修复 r8 TSY 坍缩存在状态与 Fuya 音频闭环（#11 + #12 双修复 + targeted 测试，4 文件 283 行）。

### 测试结果

- 归档审计时未重跑，以 plan 内既有记录及 commit message 为准：commit message 记录 "e2e PASS；0-context validator PASS；CodeRabbit 为额度失败，review 为输出解析基础设施失败，均无代码级 finding"。审计时通过 grep 复核 `PendingTsyDeathDrop`、`FuyaAura` 相关测试函数（`server/src/world/extract_system.rs:1326`、`server/src/npc/tsy_hostile.rs:2441,2519,2526` 等）均存在，确认已落地在 `origin/main`。

### 跨仓库核验

- **server**：`TsyPresence` → `PendingTsyDeathDrop` 转移链路（`inventory/mod.rs` + `world/extract_system.rs`）、`FuyaAura` 死亡门控 + Radius recipient（`npc/tsy_hostile.rs`）均可 grep 命中。
- **client / agent**：本修复不引入新真元/灵气流动路径，不改 `DeathEvent { cause: "tsy_collapsed" }` 语义，不扩大 Fuya 音频系统职责，无跨端协议改动。

### 遗留 / 后续

- #10 ghost entity 问题明确不在本 plan 范围，需要独立的实体生命周期与可见性设计决策。
