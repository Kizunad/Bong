# plan-bughunt-r8-tsy-collapse-presence-fuya-v1

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

- [ ] #11 在 collapse completed 化灰玩家后清理对应 `TsyPresence`。
- [ ] #11 补测试：collapse completed 后玩家收到 `DeathEvent`，同时 `TsyPresence` 被移除，避免复活锁残留。
- [ ] #12 Fuya stop sound 只响应带 `FuyaAura` 的死亡实体。
- [ ] #12 Fuya stop sound recipient 使用局部半径，不再 `All` 广播。
- [ ] #12 补测试：非 Fuya `DeathEvent` 不发 stop；Fuya 死亡 stop recipient 为 Radius。
- [ ] 运行 targeted Rust tests，并通过 read-only validator 审核。
