# plan-bughunt-shield-break-state-cleanup-v1（骨架）

> 一句话主题：盾牌耐久归零时同步清理举盾 ECS/status/stamina 状态，禁止空 offhand 继续套 wooden shield fallback 减伤、扣体力并触发错误硬直。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 冻结破盾同步终止状态集合与调用边界 | ⬜ |
| P1 | resolve 同步 cleanup + 状态转换矩阵 | ⬜ |
| P2 | server/client 既有 `ShieldBroken` 反馈回归 | ⬜ |
| P3 | server gate + shield bot e2e | ⬜ |

## 接入面

- **进料**：`server/src/combat/resolve.rs` shield durability path、`ShieldBlock`、`ShieldBlocking`、`ShieldDrainOverride`、`StaminaState`、既有 lower-shield handler。
- **出料**：`ShieldBroken` payload/AV 保持；同一 resolver batch 的后续攻击立即不再进入盾格分支。
- **共享类型 / event**：复用现有 `ShieldBroken` 与 lower-shield cleanup primitive；不另造第二破盾事件。
- **跨仓库契约**：server 状态修复；client 既有 `shield_broken` VFX/SFX/HUD 不改形状。
- **worldview 锚点**：盾是凡人最低门槛防御，实体盾破碎后防御立即终止。
- **qi_physics 锚点**：不改 qi 攻击污染削减/守恒。

## 当前证据（origin/main @ c625d5a5）

- `server/src/combat/resolve.rs:1234-1264` 在 offhand 缺失时仍 fallback `wooden_shield`。
- `server/src/combat/resolve.rs:1347-1359` 耐久归零路径消费物品并 emit `ShieldBroken`，但未同步移除 `ShieldBlock`、`ShieldBlocking`、`ShieldDrainOverride` 或恢复 stamina state。
- `server/src/combat/lifecycle.rs:291-312` 仍按 `StaminaState::ShieldBlocking` 持续 drain。

## 验收

1. 正面命中恰好破盾后，同 tick 清空 offhand 与全部举盾状态，恰好发一条 `ShieldBroken`。
2. **同一 resolver batch pin**：在一次 `app.update()` 前按顺序写入同一防御者的两条 `AttackIntent`；第一条恰好耗尽最后耐久，第二条仍在同一 `resolve_attack_intents` EventReader batch 中结算。断言第一条只发一条 `ShieldBroken`，第二条 `defense_kind != ShieldBlock` 且没有 `wooden_shield` fallback 减伤；batch 结束后 `ShieldBlock`、`ShieldBlocking`、`ShieldDrainOverride` 均不存在，`StaminaState` 不为 `ShieldBlocking`。清理必须对后续 intent 同步可见，不能只投递 `LowerShieldIntent`、依赖 deferred `Commands` 或等下一帧。
3. 后续 tick 的攻击保持零盾减伤；stamina 不再 drain，也不因旧状态进入 Exhausted/ParryRecovery。
4. 未破盾、主动放盾、耐力耗尽、重复破盾事件、盾实例在结算前被移走等边界全覆盖。
5. 既有 client 破盾反馈只发一次；不新增 DefenseKind 或第二 AV 入口。
6. 完整 server gate + bot e2e。

## 去重边界

- `plan-defense-hardening-v1` 只做全局减伤 cap/失败反馈等结构加固；本 plan 是破盾状态泄漏唯一 implementation owner。
- `plan-bughunt-shield-feedback-network-thread-ui-v1` 只管反馈线程/UI；本 plan 不重复。
