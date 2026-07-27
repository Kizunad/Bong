# plan-bughunt-duxu-juebi-quota-marker-lifecycle-v1（骨架）

> 一句话主题：把 `JueBiAfterDuXuQuota` 纳入渡劫终止态统一清理不变量，杜绝旧超额 marker 泄漏到下一次正常 DuXu 并伪造 JueBi。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 枚举 DuXu/JueBi 全终止态与 marker 生命周期 | ⬜ |
| P1 | 统一 cleanup helper/系统接线 | ⬜ |
| P2 | 状态转换矩阵 + server gate | ⬜ |

## 接入面

- **进料**：`server/src/cultivation/tribulation.rs::JueBiAfterDuXuQuota`、wave completion、failure、fled、intercept-death、JueBi settlement。
- **出料**：下一次 `start_tribulation_system` 只能读取本轮 quota marker；现有 `TribulationState`/payload 不变。
- **共享类型 / event**：复用现有 marker 与终止事件；禁止新增平行 quota flag。
- **跨仓库契约**：纯 server ECS lifecycle；既有 tribulation payload 不变。
- **worldview 锚点**：JueBi 只由本次 DuXu 超额条件触发，不得跨轮次污染。
- **qi_physics 锚点**：不改变渡劫 qi 消耗/释放。

## 当前证据（origin/main @ c625d5a5）

`server/src/cultivation/tribulation.rs:305` 定义 marker，`:949` 在超额 DuXu 插入，`:3216` wave completion 读取；JueBi 正常 settlement 的 remove tuple 已含 marker，但普通 ascension、failure、fled、intercept-death 没有形成同一 cleanup invariant。旧 marker 可在失败后残留，并在下一次未超额 DuXu 完成时被误读。

## 验收

1. 状态矩阵覆盖：超额 DuXu→JueBi、普通 DuXu→Ascended、失败、逃遁、截杀死亡、重复终止事件、重新开始。
2. 每个 terminal 后 marker 均不存在；仅同一轮超额完成允许进入 JueBi。
3. A→A/重复事件幂等，不重复结算、释放 quota 或发送 payload。
4. 运行完整 server gate。

## 边界

- 不调整 quota 数值、波次数、JueBi intensity 或视听规格。
- 不与其他 tribulation cleanup plan 合并扩大范围；只锁 quota marker 的跨轮生命周期。
