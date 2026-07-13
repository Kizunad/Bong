# plan-bughunt-craft-refund-full-inventory-loss-v1

> **active bughunt skeleton plan**。一句话主题：`craft` 显式取消或产物入包失败后的材料退款只走裸 `add_item_to_player_inventory`；当 session 期间背包空间被重新填满时，退款失败只打日志、不落地、不保留 session，导致本应返还的材料直接丢失。

## Bug 摘要

- **类型**：真实 gameplay bug，`fix_pr`
- **范围**：`server/src/craft/session.rs`、`server/src/network/craft_emit.rs`、`server/src/inventory/mod.rs`
- **一句话根因**：`cancel_craft()` 只计算退款清单，真正发放在 `craft_emit.rs` 里用裸入包函数；满包失败分支没有使用已有 `add_item_to_player_inventory_or_ground`，也没有 pending refund / rollback，随后移除 `CraftSession`
- **非重复性**：这不是 `plan-craft-close-pause-loss-v1` 的“关闭 UI 被误当显式取消，损失 30%”；本题是“已经进入退款语义后，剩余 70% 退款因满包入包失败而被吞掉”

## 实际游玩体验影响

- 玩家开始一个耗时或批量 craft 后，材料会先从背包扣走。只要中途通过拾取、交易、整理背包、异步奖励等正常玩法把腾出的格子再次占满，后续显式取消时，服务端会尝试返还 70% 材料，但满包失败的退款项不会出现在背包，也不会掉到地上。
- 显式取消分支还会把预先计算的 `material_returned` 下发给客户端；玩家可能看到“已返还 N 个材料”的反馈，但实际背包没有收到，地面也没有掉落。
- finalize 产物入包失败时，系统会取消剩余批次并尝试退款；若退款同样满包失败，失败项真实消失。对稀缺材料、长时间配方、批量制作尤其伤，因为玩家已经承担取消税，剩余退款还会二次丢失。

## 证据定位

1. `start_craft()` 在 session 创建前预扣材料，只移除/衰减栈，不预留退款容量：
   - `server/src/craft/session.rs:155-160`：`consume_materials_from_inventory()` 直接扣减材料
   - `server/src/craft/session.rs:399-405`：起 craft 后按配方材料逐项扣除
2. `cancel_craft()` 只计算退款清单和预期返回数量，不接触 inventory：
   - `server/src/craft/session.rs:446-482`：`refund_manifest` 与 `event.material_returned` 在实际入包前已算好
3. 显式取消分支裸入包，失败只 `warn`，仍发送预计算 event 并移除 session：
   - `server/src/network/craft_emit.rs:284-307`：逐项 `add_item_to_player_inventory(...)`，失败只记录 warning
   - `server/src/network/craft_emit.rs:308-318`：发送 `event`，移除 `CraftSession`
   - `server/src/network/craft_emit.rs:498-504`：client outcome 使用 `event.material_returned`
4. finalize 产物入包失败分支会取消剩余批次，但退款失败只 `error` 后丢弃：
   - `server/src/network/craft_emit.rs:389-420`：产物 grant 失败后计算退款并裸入包；失败项没有落地兜底
   - `server/src/network/craft_emit.rs:423-426`：随后移除 `CraftSession`
5. 裸入包函数在满包时返回错误，且调用方已有更合适的兜底 API：
   - `server/src/inventory/mod.rs:1679-1697`：`add_item_to_player_inventory(...)`
   - `server/src/inventory/mod.rs:1857-1898`：先用 staged 容器探测，找不到完整空间则返回 `inventory full: ...`
   - `server/src/inventory/mod.rs:1730-1790`：已有 `add_item_to_player_inventory_or_ground(...)`，满包时可写入 `DroppedLootRegistry`
   - `server/src/inventory/mod.rs:8328-8355`：已有满包落地测试证明该语义已被仓库接受

## 触发路径

1. 玩家启动一个会消耗材料的 craft 配方，服务端预扣材料并创建 `CraftSession`。
2. session active 期间，玩家通过库存移动、拾取掉落、NPC 交易、异步采集完成奖励等路径重新填满背包空间；这些路径当前没有统一 `CraftSession` 容量锁。
3. 玩家显式取消 craft，或产物发放时因背包满导致 finalize 失败并进入自动取消剩余批次。
4. `craft_emit.rs` 尝试把 `refund_manifest` 裸加回背包。
5. `add_item_to_player_inventory` 返回 `inventory full: <template>`。
6. craft 系统只记录日志，不生成 `DroppedLootRegistry` 掉落，不保留 pending refund，最后移除 `CraftSession`；退款项从玩家视角消失。

## 反方审查记录

### 第一轮反方：是否只是理论满包，或已有 fallback？

- **反方尝试推翻点**：也许 craft 起手扣材料腾出的格子天然足够接收退款；也许外层会处理 full inventory fallback；也许只是客户端显示数量错误。
- **裁决**：推翻失败，bug 成立。
- **关键理由**：
  - `start_craft()` 只扣材料，不锁格、不预留退款空间。
  - `craft_emit.rs` 只 import 并调用裸 `add_item_to_player_inventory`，系统参数也没有 `DroppedLootRegistry`。
  - 显式取消分支失败只 `warn` 后仍发送预计算返回数量；finalize 失败分支虽然重算成功返回数，但失败项仍真实丢失。

### 第二轮反方：是否可达、是否重复、是否只是 close/cancel 旧题？

- **反方尝试推翻点**：当前 client 关闭 craft UI 会发 cancel，玩家是否有机会在 session 中填满背包；本题是否被 `plan-craft-close-pause-loss-v1` 或 PR #1030/#1034 覆盖。
- **裁决**：推翻失败，成立且不重复。
- **关键理由**：
  - server 侧没有 `CraftSession` 期间冻结库存的约束；库存移动、拾取掉落、NPC 交易、异步奖励等入包路径不查 craft session。
  - `plan-craft-close-pause-loss-v1` 关注“中性关屏被错误映射为取消，导致设计内 30% 损耗”；本题关注“已进入退款路径后，设计应返还的 70% 因满包失败继续丢失”。
  - PR #1030 是 craft outcome 网络线程反馈，PR #1034 是炼丹取丹满包吞产物，均非 craft refund 满包退款丢失。

## Skeleton Fix Plan

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 钉死 refund grant 语义：退款项必须入包或落地，不能只 log 后丢弃 | fix_pr | ✅ 2026-07-12 |
| P1 | 给 craft refund 路径接入 `DroppedLootRegistry`、玩家 `Position` 与 dimension | fix_pr | ✅ 2026-07-12 |
| P2 | 显式取消分支按实际 grant / drop 结果统计 `material_returned`，避免虚报 | fix_pr | ✅ 2026-07-12 |
| P3 | finalize 输出 grant 失败后的剩余批次退款同样使用入包或落地兜底 | fix_pr | ✅ 2026-07-12 |
| P4 | 补满包、混合成功、无 registry、配置错误等回归测试 | fix_pr | ✅ 2026-07-12 |

建议实现方向：

1. 在 `apply_craft_intents()` 与 `tick_craft_sessions()` 需要退款的分支引入 `Option<ResMut<DroppedLootRegistry>>`、玩家 `Position` 与当前 dimension 信息。
2. 对 `refund_manifest` 使用 `add_item_to_player_inventory_or_ground(...)`；仅对 `inventory full:` 走地面掉落，其它结构性错误继续报错并保留可诊断日志。
3. 显式取消分支不要复用 `cancel_craft()` 预计算的 `event.material_returned` 作为实际结果；应按成功入包和成功落地的数量重新赋值，或新增更准确的 outcome 字段。
4. finalize 产物 grant 失败后的退款分支同样必须对失败项落地或保留 pending refund，不能在移除 `CraftSession` 后丢掉 manifest。
5. 若 `DroppedLootRegistry` 不可用，应考虑保留 session / pending refund，而不是 silent loss；至少测试中要覆盖该错误路径。

## 验收测试计划

- `cancel_refund_full_inventory_drops_to_ground`：起 craft 后填满背包，显式取消，退款项写入 `DroppedLootRegistry`，背包未收到也不丢失。
- `cancel_refund_reports_actual_returned_count`：显式取消时部分入包、部分落地，`CraftOutcomeV1::Failed.material_returned` 与实际成功返还总数一致。
- `finalize_failure_refund_full_inventory_drops_to_ground`：产物入包失败触发剩余批次取消，退款项满包时落地，不被吞。
- `refund_structural_error_does_not_mask_config_bug`：unknown template / no containers 等非满包错误不被静默转成掉落。
- `mixed_refund_manifest_no_partial_loss`：多材料退款中，部分材料可合并、部分需新格、部分落地，最终总数守恒。

## 风险

- `craft_emit.rs` 的 system 签名会增加 dropped-loot 和位置/dimension 依赖，需要避免与现有调度、测试 app 最小资源集冲突。
- 如果只修显式取消分支而漏掉 finalize 失败分支，批量 craft 仍会在产物 grant 失败时吞剩余退款。
- 如果继续沿用预计算 `material_returned`，客户端会保留“显示返还但实际没返还/落地”的反馈漂移。
- 不应把 unknown template、无容器、allocator 错误全部伪装成地面掉落；只有 `inventory full:` 适合走 `DroppedLootRegistry` fallback。

## Finish Evidence

- **落地清单**：`server/src/network/craft_emit.rs` 对 start/cancel/finalize 做 clone staging，成功写入 SQLite 后才发布 inventory/session/事件；退款满包落入 durable `DroppedLootRegistry`，结构错误整批回滚，同帧重复请求幂等。
- **持久化闭环**：`server/src/persistence/mod.rs` v37 增加 `dropped_loot`；`server/src/player/state.rs` 将 inventory、craft session、cultivation qi、pending inflow 与退款掉落同事务提交，拾取时 inventory 与 durable row 删除同事务提交；重启 hydrate 同时推进 instance allocator 高水位。
- **生命周期**：`server/src/player/mod.rs`、`server/src/network/mod.rs`、`server/src/cmd/dev/reset.rs` 覆盖通用 inventory flush 排序、失败 dirty 重试、登录恢复、断线/停服保存和 dev reset 清理。
- **协议与 Bot**：`scripts/bot/proto_min.py` 解码 `craft_outcome.material_returned` 与 `dropped_loot_sync`；两个 production 场景验证满包双 cancel、唯一掉落/逐份拾取、断线暂停恢复和精确一次退款。
- **关键 commit**：本 PR 的三笔最终提交分别锁定 durable 持久化、退款运行时守恒和 Bot/evidence；具体 hash 以 PR #1142 最终 HEAD 为准。
- **测试结果**：`cargo check --all-targets` PASS；`cargo fmt --check` PASS；craft emit 39/39、craft session 41/41、事务故障注入与重启往返定向测试 PASS；Python protocol 50/50 PASS；production Bot 2/2 PASS。全量 `cargo test` 为 11226 PASS、1 ignored、唯一共享 POI 并发耗时阈值失败，单线程定向复跑 PASS（7.10s）。
- **跨仓库核验**：server `CraftSessionPersistenceDirty` / `save_player_craft_checkpoint` / `dropped_loot`；Bot `craft_session_state` / `craft_outcome` / `dropped_loot_sync`；本修复不改 client 或 agent schema。
- **遗留 / 后续**：`cargo clippy --all-targets -- -D warnings` 仍被 Rust 1.96 引入的 66 项共享基线 lint 阻断，本 PR 新增代码无诊断；不在 #1142 范围内追改。

## 审计说明

- BugHunt skeleton 已由 PR #1142 消费并完成修复；未改依赖或生产配置。
- 已用 `gh pr list --state open --limit 100` 检查开放 PR；已避开 #973/#981/#990/#1004/#1007/#1014/#1022/#1029/#1034 以及相邻 craft close / craft outcome 题目。
- 反方 subagent 已完成两轮对抗审查，结论均为“成立且不重复”。
