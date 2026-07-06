# plan-bughunt-bot-production-inventory-instance-visibility-v1（骨架）

> **骨架（草案）**。一句话主题：Bot e2e 在 `/give furnace_fantie` / `/give hoe_iron` 后，不能稳定从 `bong:server_data` 的 `inventory_snapshot` 中取得对应 item instance 与位置，导致生产系统 place/equip 类 intent 无法做深链路断言。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | `/give` 后 inventory_snapshot 可观测性复现 | bughunt | ⬜ |
| P1 | Python protobuf / snapshot 触发策略决策 | coverage | ⬜ |
| P2 | 生产 intent 深断言恢复 | coverage | ⬜ |

## P0 — `/give` 后 inventory_snapshot 可观测性复现

- 失败现象：CI 中 `production_alchemy_forge_intents` 等到 inventory_snapshot，但 bot 浅扫描找不到 `furnace_fantie`；`production_lingtian_gathering_intents` 同样找不到 `hoe_iron`。
- 追加现象：复用现成本地 server 跑降级场景时，`/give furnace_fantie 1` 与 `/give hoe_iron 1` 也没有稳定出现 `[dev] gave ...` chat 反馈；因此 PR #982 不再把生产模板 give chat 作为硬断言。
- 已确认模板存在：`server/assets/items/core.toml` 有 `furnace_fantie`，`server/assets/items/lingtian.toml` 有 `hoe_iron`。
- 待定根因：`/give` 成功后是否触发新 inventory_snapshot、bot 零依赖浅解码是否漏了当前 snapshot 结构、或该快照只包含 join 初始状态。

## P1 — Python protobuf / snapshot 触发策略决策

- 决策是否在 bot e2e 中引入生成式 Python protobuf binding，或继续维护零依赖浅解析器。
- 若保持零依赖，需要用真实 CI payload 样本 pin `InventorySnapshot` 的 placed/equipped/hotbar 解码。
- 若 server 需要在 `/give` 后稳定推送 snapshot，应另开 fix PR；本 coverage PR 不改业务逻辑。

## P2 — 生产 intent 深断言恢复

- 炼丹/炼器：恢复 `furnace_fantie` / `fan_iron_anvil` 的 instance_id 获取，再断言 place/open/forge station payload。
- 灵田：恢复 `hoe_iron` 装备与 `lingtian_start_till` 的 session payload 深断言。
- 当前 PR #982 临时保留稳定入口覆盖：dev give chat、client_request 发送后不踢、炼丹 unknown recipe chat、采集进度回流。
