# plan-bughunt-forge-outcome-full-inventory-loss-v1

## §0 摘要

锻造（武器）成品在随身容器无空槽时被 `forge_outcome_to_inventory` 静默丢弃——材料在起炉时已被 `consume_forge_materials_atomic` 原子扣除，成品若找不到空格只 `tracing::warn!` 后 `continue`，不落地、不入包、不退款；同时 `push_forge_outcome_on_event` 无条件把 bucket/quality/achieved_tier 当"结算成功"回执发给客户端，玩家看到的是与 Perfect/Good 一致的成功 UI，实际翻遍背包找不到成品。这与本项目已归档的 `plan-forge-leftovers-v1.md §9` 风险表明确要求的"背包满时 fallback 到地面掉落"设计意图直接相悖——该意图从未被对应 P1 commit 落地。

本 plan 仅是 BugHunt Skeleton Plan，不包含实际修复。

## §1 实际游玩体验影响

- 玩家正常锻造一把武器，投入的所有材料在起炉瞬间就已扣除且不可逆；只要结算那一刻随身容器（`main_pack`/`pack_`/`body_pocket` 等非砧内容器）恰好没有能放下成品体积的空格，成品与已投入材料会一起消失。
- 这在长时间冒险、捡装备、开箱后是很常见的背包状态，不是边缘场景。
- 客户端仍会收到与真实成功完全一致的 `forge_outcome` payload（bucket/quality/achieved_tier 正常下发），造成"明明显示锻成了却翻遍背包找不到"的假成功体验，玩家无法察觉、无法申诉、无法复现定位。
- 现有单测 `outcome_with_no_free_carried_slot_does_not_mutate_inventory` 把这一"静默跳过"行为断言为期望结果，说明这是已定型但从未被处理的缺口，不是遗漏的边界 case。

## §2 复现路径

1. 玩家放置炼炉（工作台配方产物）、学习任意图谱。
2. 玩家清空随身容器所有空闲格子，或让背包处于装满状态（例如冒险后携带大量战利品）。
3. 正常投料起炉：`handle_start_forge_requests`（`server/src/forge/mod.rs:182`）不检查随身容器是否有空位就允许起炉，随后原子扣除材料（`consume_forge_materials_atomic`，`server/src/forge/mod.rs:336`）。
4. 正常完成淬炼/铭文/开光全部步骤，触发 `ForgeOutcomeEvent`。
5. 现状预期：`forge_outcome_to_inventory`（`server/src/forge/inventory_bridge.rs:12-146`）里 `find_forge_output_slot`（170-183）在所有容器都找不到空槽，81-91 行只 `tracing::warn!` 并 `continue`，成品不落地、不退款；`push_forge_outcome_on_event`（`server/src/network/forge_snapshot_emit.rs:280-291`）仍无条件下发"成功"回执。
6. 修复后预期：找不到空槽时成品掉落在锻炉旁地面，且给玩家发一条可见反馈（背包已满，成品已落地）；客户端回执与实际入包/掉地结果对齐。

## §3 根因证据

- `server/src/forge/mod.rs:182` `handle_start_forge_requests`：起炉时用 `consume_forge_materials_atomic`（:336）原子扣除玩家材料，此后无论结果如何材料都不退还。
- `server/src/forge/inventory_bridge.rs:12-17` `forge_outcome_to_inventory` 与 `:81-91`（`find_forge_output_slot` 返回 `None` 分支）：找不到空槽时只 `tracing::warn!` 并 `continue`——不落地、不入包、不退款、不给玩家任何可见提示。
- `server/src/forge/inventory_bridge.rs:170-183` `find_forge_output_slot`：遍历所有非 `body_pocket` 容器和 `body_pocket` 本身，找不到匹配 `grid_w`/`grid_h` 的空槽即返回 `None`。
- `server/src/forge/inventory_bridge.rs:545-570` 现存单测 `outcome_with_no_free_carried_slot_does_not_mutate_inventory` 把上述"跳过不落地"行为断言为当前预期行为，证明这是已 ship 的现状。
- `server/src/network/forge_snapshot_emit.rs:280-291` `push_forge_outcome_on_event` 与 `:103-125` `send_forge_outcome_to_player`：独立消费同一 `ForgeOutcomeEvent`，无条件下发 `ForgeOutcomeDataV1`（bucket/quality/color/weapon_item/achieved_tier），无任何字段反映成品是否真正入包。
- `docs/finished_plans/plan-forge-leftovers-v1.md §9` 风险表明确写"`forge_outcome_to_inventory` system 必须……失败（背包满）时 fallback 到地面掉落（复用 inventory dropped_loot 机制）"——即该 finished plan 本身设计意图就是要做掉地兜底，但 shipped 的 P1 commit（`c9f9addf`）只改进了跨容器槽位搜索覆盖面，从未实现这个 fallback；其 Finish Evidence 也未把这一缺口列为已知/接受的遗留项。
- 既有可复用基础设施：`DroppedLootRegistry` / `discard_inventory_item_to_dropped_loot` 已在 `relic_hydrate.rs`、`race_change.rs`、`morph.rs`、`arm_wound.rs`、`container_block.rs` 中被复用，是本仓成熟的"容器满时掉地"落点，forge 路径目前完全没有调用。

## §4 非重复比对

- 已读 `docs/plans-skeleton/plan-bughunt-alchemy-takeback-full-inventory-loss-v1.md`：同类失败模式（`add_item_to_player_inventory_or_ground` 掉地工具已存在但未被对应结算路径使用）出现在炼丹取回路径，是不同子系统、不同文件，不构成重复；两者可视为同一 bug class 的姊妹案例。
- 已读 `docs/finished_plans/plan-botany-harvest-full-inventory-loss-v1.md`：该 plan 处理的是野外采集 harvest 满背包丢失，已修复并用 `DroppedLootRegistry`/掉地兜底解决，本 finding 是同一失败模式在 forge 模块的独立复发，文件、触发路径均不重合。
- 已读 `docs/plans-skeleton/plan-bughunt-forge-c2s-session-wiring-v1.md`：只讲 `ForgeStartSession`/`BlueprintTurnPage`/`LearnBlueprint` 缺 handler 的 C2S 分发问题，不含 outcome→inventory 结算路径。
- 已读 `docs/plans-skeleton/plan-bughunt-forge-ui-session-stale-v1.md`：明确限定于客户端 store 断线陈旧问题，不含此服务端结算路径。
- 已读 `docs/plan-bughunt-meridian-forge-zone-shadow-v1.md`：是 `cultivation::forging` 经脉淬炼真元记账问题，模块和失败模式均不同。
- 已读 `docs/finished_plans/plan-forge-session-entry-wiring-v1.md`：唯一相关的"遗留"条目是砧无 abort/断线释放路径（不同失败模式，已存在文档记录，故本轮未重复上报）。
- Grep `docs/plans-skeleton` 与 `docs/plan-*.md` 未见任何以 `forge_outcome_to_inventory` / `find_forge_output_slot` / "锻造产物入袋" 为关键词的既有 forge 专项 skeleton。

## §5 修复计划骨架

### P0 掉地兜底 + 结算回执对齐

- 给 `forge_outcome_to_inventory` 增加 `Option<ResMut<DroppedLootRegistry>>` + 施法者 `Position`/`CurrentDimension` 查询，在 `find_forge_output_slot` 返回 `None` 时复用本模块 `artifact_meridian.rs` 已用到的 `discard_inventory_item_to_dropped_loot`（或 `inventory/mod.rs` 的 `add_item_to_player_inventory_or_ground`）把成品掉落在锻炉旁地面，而不是直接 `continue` 丢弃。
- 掉地兜底触发时发一条玩家可见反馈（扩展 `MineralFeedbackEvent` 或新增 forge 专属提示），说明"背包已满，成品已在锻炉旁落地"。
- 让 `push_forge_outcome_on_event` 的结算回执与实际入包/掉地结果对齐（或补一条修正 payload），避免客户端在物品实际丢失时仍展示"成功"结果。

### P1 测试与回归

- 补单测：背包无空槽时成品掉落到地面（而非静默丢弃）、玩家收到"已落地"反馈、原有 `outcome_with_no_free_carried_slot_does_not_mutate_inventory` 断言语义需同步更新为"掉地而非静默跳过"。
- 补单测：背包有空槽时行为不变（回归保护）。
- 补单测：结算回执 payload 与实际落点（入包 vs 掉地）字段对齐。

## §6 验证计划

- `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
- 手工/bot 复现矩阵：背包满 + 成功锻造（掉地兜底生效）、背包有空槽 + 成功锻造（正常入包，回归不受影响）。

## §7 接入面与守恒说明

- 进料：`ForgeOutcomeEvent`、`PlayerInventory`、`ItemRegistry`、玩家 `Position`/`CurrentDimension`。
- 出料：`DroppedLootRegistry` 落地物、玩家可见反馈事件、`ForgeOutcomeDataV1` 结算 payload。
- 跨端契约：C2S/S2C payload 结构不必变；服务端新增掉地兜底 + 反馈事件即可，客户端按现有掉落物/提示渲染路径消费。
- qi_physics：本问题涉及的是物品（成品武器 + 消耗材料）在库存系统内的归属转移，不涉及真元/灵气转移，不新增 qi 常数或 ledger 流。

## §8 对抗复核结论

- 候选证据：`handle_start_forge_requests` 起炉即原子扣料且从不检查/预留输出槽位；`forge_outcome_to_inventory`/`find_forge_output_slot` 找不到空槽时只 warn+continue，无退款/无掉地/无玩家可见信号；`push_forge_outcome_on_event` 是独立 `EventReader`，无条件下发"成功"payload，客户端无法区分真实成功与静默丢失；现存单测把这一行为锁定为当前预期。
- 反方质疑：该行为是否已被项目接受为已知限制？是否与既有 forge/alchemy 类似 finding 重复？
- 修正/反驳：`docs/finished_plans/plan-forge-leftovers-v1.md §9` 风险表明确要求掉地 fallback，证明这是设计意图未兑现的真实缺口而非已接受限制；已确认的落地 P1 commit（`c9f9addf`，核对仍是本 HEAD 现状）只做了槽位搜索覆盖面改进，未实现该 fallback，Finish Evidence 也未把此列为已知遗留；去重比对 `plan-bughunt-alchemy-takeback-full-inventory-loss-v1`（不同子系统同类模式）、`plan-botany-harvest-full-inventory-loss-v1`（已修复的姊妹案例，证明该 bug class 已被项目认定为真实可修）、`plan-bughunt-forge-c2s-session-wiring-v1`/`plan-bughunt-forge-ui-session-stale-v1`/`plan-bughunt-meridian-forge-zone-shadow-v1`（均为不同模块/失败模式）均不重叠。
- 反方最终裁决：通过（`is_real: true`, `reachable: true`, `severity_adjust: unchanged`，保持 high）。可达性无需任何 dev 命令，是正常"投料起炉→完成全部步骤"锻造玩法闭环里背包已满这一常见状态即可触发；置信度高，非重复，适合开 Skeleton Plan PR。
