# plan-bughunt-poison-trait-hud-disconnect-v1（Skeleton）

> **Skeleton（BugHunt report-only）**。一句话主题：`PoisonTraitHudStateStore` 是跨 session 静态 HUD store，生产断线清理没有调用 `clear()`；玩家断线 / 切服 / 重连后，到首个权威 `poison_trait_state` 抵达前，上一 session 的毒性真元 HUD 会短窗口残留。

> 立项边界：这不是“永久残留”。服务端 hydrate 会给玩家插入默认 `PoisonToxicity` / `DigestionLoad`，并每 20 tick 推一次 `poison_trait_state`，正常同服重连通常约 1 秒内自愈。本 plan 只锁定首帧到首个权威包之间的 client session hygiene 缺口。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 毒性真元 HUD 断线 / 切服短窗口残留 | fix_pr | ⬜ |

## P0 — 毒性真元 HUD 断线 / 切服短窗口残留

- **bug**：`PoisonTraitHudStateStore` 有生产可用的 `clear()`，但 `BongNetworkHandler.clearClientStateOnDisconnect()` 和 `CombatHudBootstrap.resetOnDisconnect()` 都没有调用；HUD 编排每帧直接读取旧 snapshot。
- **实际游玩体验影响**：玩家上一局吃过毒丹、消化负荷较高或刚触发寿命损失 toast 后，断线 / 切服 / 进新角色时，右侧仍可能短暂显示“毒性 xx% · 轻毒/中毒/重毒”、消化条或寿命扣减提示。玩家会误判当前角色仍处于毒性增益 / 丹毒负荷 / 寿命惩罚状态，尤其在进服首秒准备战斗、嗑药或检查 build 状态时，会把上一局的状态当成当前局真实状态。
- **严重度判断**：`minor`
  - 它不是无限串档，正常 Bong 服务端会在 20 tick 周期包后覆盖为当前角色状态。
  - 但它是明确可见的 session-bound HUD 脏状态，且同类 HUD 断线短窗口问题已多次作为 BugHunt 修复目标。

### 证据链

1. `client/src/main/java/com/bong/client/hud/PoisonTraitHudStateStore.java`
   - `STATE` 是 static `AtomicReference`。
   - `clear()` 会把状态置回 `State.NONE`，说明该 store 有清理语义。
2. `client/src/main/java/com/bong/client/hud/BongHudOrchestrator.java`
   - 每帧调用 `PoisonTraitHudPlanner.buildCommands(PoisonTraitHudStateStore.snapshot(), ...)`。
3. `client/src/main/java/com/bong/client/hud/PoisonTraitHudPlanner.java`
   - `safe.active()` 为真时渲染毒性百分比、毒性等级、消化条；寿命 warning window 内还会渲染 toast。
4. `client/src/main/java/com/bong/client/BongNetworkHandler.java`
   - `clearClientStateOnDisconnect()` 已清理大量 session-bound HUD/store，例如 realm collapse、TSY、Tiandao、agent UI、halfstep、remains、craft 等。
   - 该清单没有 `PoisonTraitHudStateStore.clear()`。
5. `client/src/main/java/com/bong/client/combat/CombatHudBootstrap.java`
   - `resetOnDisconnect()` 已清理 combat HUD 大量 store，但也没有清理 `PoisonTraitHudStateStore`。
6. `server/src/network/poison_trait_emit.rs`
   - `emit_poison_trait_state_payloads` 每 20 tick 给带 `PoisonToxicity` / `DigestionLoad` 的 client 发状态，因此同服正常重连会在后续包自愈。
7. `server/src/cultivation/mod.rs`
   - 玩家 hydrate 时会构造默认 `PoisonToxicity` 和按境界的 `DigestionLoad`，进一步证明“永久残留”不是准确表述。

### 复现路径

1. session A 中让角色吃毒丹，直到毒性真元 HUD 显示毒性百分比 / 消化负荷；若触发过量，保留寿命扣减 toast 更明显。
2. 直接断线或切到另一服务器 / 新角色。
3. 在首个 `poison_trait_state` 到达前观察 HUD。
4. 实际：旧毒性 HUD 短暂继续显示。预期：断线后该 store 立即 `State.NONE`，新 session 未收到权威状态前不显示旧毒性。

### 建议修复

- 在 `BongNetworkHandler.clearClientStateOnDisconnect()` 中调用 `PoisonTraitHudStateStore.clear()`。
- 补 client 单测：先写入 active poison trait HUD state，触发断线清理入口后断言 `PoisonTraitHudStateStore.snapshot() == State.NONE`，并断言 `PoisonTraitHudPlanner.buildCommands(...)` 为空。
- 可选加固：把所有 `server_data` 驱动、静态保存、HUD 每帧读取的 store 建一张 disconnect reset 清单，防止类似短窗口残留继续散落。

## 两轮对抗复核

1. **Round 1 反方**：正常服务端 hydrate 会给每个玩家插入默认 `PoisonToxicity` / `DigestionLoad`，并且 `emit_poison_trait_state_payloads` 每 20 tick 广播一次，所以这不是实际 bug。  
   **回应**：该反方推翻的是“永久残留”，但没有推翻首帧到首个权威包之间的残留。client 断线时明明已有 `clear()` 可用却不调用，旧 snapshot 会被 HUD 编排继续消费。
2. **Round 2 反方**：影响太轻，且同一角色同服重连时旧值可能本来就是正确值；已有 HUD/session reset skeleton 覆盖了类似家族。  
   **结论**：保留但降级为 `minor`。排重未发现 poison trait 专项 PR / skeleton；相邻题是通用 HUD、dugu v2、anqi 或 false skin，不覆盖 `PoisonTraitHudStateStore`。实际影响是短窗口误导，而非持久 gameplay 破坏。

## Adversarial Conclusion

对抗子 agent 结论：**保留，置信度 0.78**。候选不是永久串档，但属于明确的 session-bound HUD 清理遗漏；建议以局部 fix PR 收口。
