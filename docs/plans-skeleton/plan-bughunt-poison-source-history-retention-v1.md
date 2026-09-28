# plan-bughunt-poison-source-history-retention-v1

> 来源 Issue：#1841。`PoisonToxicity.source_history` 每次服毒都追加且整体持久化，没有容量或时间裁剪。

## §0 摘要

`consume_poison_pill_now` 每次把 `PoisonDoseRecord` push 到 `PoisonToxicity.source_history`，`PoisonToxicity::normalized` 只裁剪 level，不裁剪 history。持久化系统又把整个组件序列化进 `cultivation_json`，玩家可通过反复服毒让 ECS 和数据库记录无限增长。

## §1 实际游玩体验影响

- 长期角色的内存占用和存档大小随服毒次数线性增长，没有自然上限。
- 登录/保存/迁移时间会被无关的历史记录拖慢。
- 清理 level 或 decay 并不会减少 source history，问题会跨重启累积。

## §2 复现路径

1. 对同一角色重复调用 `consume_poison_pill_now`，每次记录不同 tick。
2. 观察 `PoisonToxicity.source_history.len()` 只增不减。
3. 通过 `persist_player_cultivation_bundle` 保存并重新加载，确认整个 Vec 都进入 cultivation JSON。
4. 运行 `normalized`，确认它仍保留全部历史。

## §3 根因证据

- `server/src/cultivation/poison_trait/handlers.rs:16-34` 无条件 `source_history.push`，没有 retain/truncate 上限。
- `server/src/cultivation/poison_trait/components.rs:257-285` 的组件和 `normalized` 没有历史裁剪或 `serde(skip)`。
- `server/src/persistence/mod.rs:6191` 附近的 `persist_player_cultivation_bundle` 序列化整个 cultivation bundle，放大了无限 Vec 的持久化影响。
- 同仓 `combat/baomai_v3/state.rs` 和 `cultivation/contamination.rs` 已有按时间窗/阈值裁剪的可复用模式。

## §4 非重复比对

- 这是持久化容量/组件生命周期问题，不是 poison level 数值或 qi ledger 守恒问题。
- `plan-bughunt-dugu-baomai-qi-max-shrink-ledger-v1.md` 的毒蛊经脉缩容与本组件无共享 source history。
- 未发现 active/finished plan 已为 `PoisonToxicity.source_history` 定义上限或迁移策略。

### 立项检查记录（2026-09-28）

- `docs/worldview.md`：检索“毒蛊、毒性、经脉代价、服毒”，核对 §四 L423-L426。
- `docs/finished_plans/`：检索 `PoisonToxicity`、`source_history`、`PoisonDoseRecord`；命中 `plan-poison-trait-v1.md` 的组件契约，但没有 history 上限。
- active plan：检索 `source_history`、`cultivation_json`、`persist_player_cultivation_bundle`；未发现 active plan 为该 Vec 定义保留策略。
- `docs/plans-skeleton/`：检索 `source_history`、`PoisonToxicity`、`retain`；除本文件外未发现同主题 skeleton。

## 接入面与跨仓契约

- **Inputs**：`PoisonPillKind`、`PoisonToxicity`、`DigestionLoad`、当前 tick、持久化 cultivation bundle。
- **Outputs**：保留有限、可解释的 `PoisonDoseRecord` 窗口；level/decay 语义不变，旧存档加载时自动裁剪。
- **共享类型 / 事件**：`PoisonToxicity`、`PoisonDoseRecord`、`persist_player_cultivation_bundle`；不新增 agent/client payload。
- **server 契约符号**：`consume_poison_pill_now`、`PoisonToxicity::normalized`、`PoisonDoseRecord`、`persist_player_cultivation_bundle`。
- **agent**：无变更。agent 不消费完整 source history，仍使用已有玩家状态摘要。
- **client**：无变更。history 不是客户端协议字段，现有 cultivation detail 不受影响。
- **worldview 锚点**：`docs/worldview.md §四 L423-L426`（毒蛊造成真元/经脉代价）。
- **qi_physics**：本 issue 不改变真元流动，不调用 `ledger.transfer` 或守恒断言；若服毒副作用另有 qi 扣除，应沿既有 `Cultivation.qi_current` 事务单独核验，不能把 history 裁剪当作 qi 结算。

## §5 修复骨架

### P0 有界历史

- 明确 history 的业务用途，采用固定最近窗口或按 tick/条数上限 retain；在 push 后和反序列化后都执行同一规范化函数。
- 旧存档超限时保留最新记录并可观测地丢弃旧记录，不拒绝整个玩家 bundle。

### P1 回归与迁移

- 覆盖连续服毒、跨 decay、保存/加载、旧存档超限和默认空 history。
- 断言 level/side effect 不被裁剪逻辑改变，持久化 JSON 大小有上界。
