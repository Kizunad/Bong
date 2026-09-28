# plan-bughunt-contamination-unopened-meridian-death-gate-v1

> 来源 Issue：#1380。污染排异的致死判据把“未打通”错误当作“已毁”，导致新角色可被污染+短缺真元直接判死。

## §0 摘要

`contamination_tick` 先尝试用玩家真元排异；只要本 tick 实际释放少于目标成本就设 `any_qi_deficit`。随后 `meridians.iter().all(|m| m.integrity <= 0.0 || !m.opened)` 把 `opened == false` 的健康经脉算进 `all_broken`。新玩家默认经脉通常是完整但未打通，于是污染残留、短缺真元即可触发 `CultivationDeathCause::ContaminationOverflow`。

## §1 实际游玩体验影响

- 醒灵/引气新手还没有打通经脉时，接触污染攻击并在当前真元不足排异，可能立即死亡。
- 这绕过了经脉“污染与损伤独立、排毒后可恢复”的规则，也让新手死亡原因与真实身体状态不符。

## §2 复现路径

1. 生成默认 `MeridianSystem`（经脉 `integrity > 0`、`opened == false`），给实体添加一条 `ContamSource`。
2. 将 `Cultivation.qi_current` 设为低于 `DRAIN_RATIO` 所需成本，运行 `contamination_tick`。
3. 排异返回不足，`any_qi_deficit=true`；`all_broken` 因所有经脉均 `!opened` 而为真，且污染条目仍在。
4. 收到 `CultivationDeathTrigger::ContaminationOverflow`，尽管没有经脉被毁。

## §3 根因证据

- `server/src/cultivation/contamination.rs:155-197` 通过 `release_qi_amount_to_zone` 计算排异不足；`:200-213` 随后以 `integrity <= 0.0 || !opened` 计算 `all_broken` 并发死亡事件。
- `MeridianSystem::new/default` 的未打通经脉仍有正完整度；`opened` 表示拓扑门槛，不是伤害状态。把 `!opened` 与 `integrity <= 0.0` 并列违反该数据模型。
- 既有 `resolve_crack_target` 只给可用经脉添加裂痕，不能证明未打通经脉“已毁”。

## §4 非重复比对

- 现有文档中 `contam_purge_multiplier` / `ContaminationBoost` 的 issue 处理的是排异速率 modifier 消费，不是致死判据。
- `qi_zero_decay` 的降境关闭经脉和 `MeridianSeveredPermanent` 是不同状态转换，不应在本 plan 顺手重构。

## 接入面与跨仓契约

- **Inputs**：`Contamination`、`MeridianSystem`、`Cultivation.qi_current`、`Position`/`CurrentDimension`、`ZoneRegistry`、`WorldQiAccount`、`CultivationClock`。
- **Outputs**：污染条目更新、经脉裂痕、`CultivationDeathTrigger`；不改变现有 server→agent/client payload。
- **共享类型 / 事件**：复用 `Contamination`、`MeridianSystem`、`CultivationDeathCause::ContaminationOverflow`、`QiTransfer`。
- **server 契约符号**：`contamination_tick`、`release_qi_amount_to_zone`、`resolve_crack_target`、`CultivationDeathTrigger`。
- **agent**：无变更；agent 只消费死亡/叙事事件，判据修正在 server 内完成。
- **client**：无变更；死亡协议与 HUD 形状不变。
- **worldview 锚点**：`docs/worldview.md §四 L294-L302`（污染独立于经脉损伤、排毒可恢复）；`§三 L318-L326`（经脉崩溃才是致死原因之一）。
- **qi_physics**：在线真元权威是 `Cultivation.qi_current`；现有排异必须继续走 `release_qi_amount_to_zone`，其内部落 ledger/overflow。回归断言使用 `assert_conservation` 和 `SPIRIT_QI_TOTAL`，不要新增直接 `qi_current -=` + event-only 路径。

## §5 修复骨架

### P0 修正全毁判据

- 将 `all_broken` 限定为实际损伤状态（`integrity <= 0.0` 或项目已定义的 severed 状态），`opened == false` 不能单独触发死亡。
- 保留污染残留和真元不足时对已打通经脉添加裂痕的既有行为。

### P1 回归

- 默认未打通但完整经脉 + qi deficit：不得发 `ContaminationOverflow`。
- 已打通且完整度归零 + qi deficit + 残留污染：仍发死亡事件。
- 排异成功/失败两条路径分别核对 `Cultivation.qi_current`、zone/overflow 转账和守恒。

