# BugHunt: 炼丹投料 freshness 按全鲜结算

## 0. 结论

`alchemy_feed_slot` 真实投炉入口已经选中了具体材料实例，但调用 `AlchemySession::feed_stage` 时固定传 `quality_factor = 1.0`。这绕过了 `Freshness` / shelflife 衰减：陈药、枯药、死物在炼丹结算中与新鲜药材贡献一致。

实际游玩体验影响：玩家可以把已经衰减的旧药材当全鲜材料炼出同等回气贡献的丹药，保鲜、及时采收、冷藏/容器管理和灵田新鲜产出的价值都会被抹平；炼丹经济会倾向囤积陈货，而不是围绕鲜度做风险和路线选择。

## 1. 证据

- `server/src/alchemy/session.rs:111` 明确要求 `feed_stage` 的第三参 `quality_factor` 来自 `shelflife::decay_current_qi_factor`；无 Freshness / 凡俗物品才传 `1.0`。
- `server/src/alchemy/resolver.rs:137` 会把 `session.staged.quality_factor` 应用到结算结果，说明该字段不是展示数据，而是丹药效果的一部分。
- `server/src/shelflife/consume.rs:116` 已有 `decay_current_qi_factor` helper，可按 `Freshness`、profile、tick、storage multiplier 计算当前贡献因子。
- `server/src/network/client_request_handler.rs:12233` 已经选出即将消费的材料实例；但 `server/src/network/client_request_handler.rs:12246` 固定调用 `session.feed_stage(recipe, slot_idx as usize, &[(material.clone(), count, 1.0)])`。
- `server/src/network/client_request_handler.rs:12252` 之后才对同一批实例做 `AlchemyLoad` attrition；这不等于读取入炉前 freshness 贡献，且当前 `feed_stage` 已经先把全鲜 factor 记入 session。

## 2. 触发步骤

1. 准备一座自己的炼丹炉和同一丹方所需材料。
2. 准备两批同 template 的炼丹材料：一批新鲜，一批带 `Freshness` 且已衰减到较低 current qi。
3. 分别用两批材料走 `alchemy_ignite` -> `alchemy_feed_slot` -> `alchemy_intervention` -> `alchemy_take_back`。
4. 预期：陈旧材料的 `quality_factor` 应低于 1.0，最终 `qi_gain` 或等价效果下降。
5. 实际：真实 feed 入口硬传 `1.0`，两批材料在 session/resolver 里都按全鲜结算。

## 3. 去重

- 不重复 #1034：#1034 是炼丹取丹满包吞产物；本问题发生在投料阶段的品质因子接线。
- 不重复 #1072：#1072 bot 生产场景只覆盖普通炼丹成功链，没有 stale freshness 负例。
- 不重复 #1086：#1086 是炼丹 UI 断线串会话；本问题是 server 权威结算输入错误。
- 语义上关联早期 M5c shelflife 接入，但当前代码只完成了 session/resolver 能力，真实 `alchemy_feed_slot` caller 仍假绿，因此这是未闭环回归。

## 4. 修复 TODO

- [ ] TODO(server): 在 `handle_alchemy_feed_slot` 中，按被选中的 `selected_consumption` 实例读取投料前 freshness，计算每个实例的 `decay_current_qi_factor`；无 Freshness 或无适用 profile 的物品保留 `1.0`。
- [ ] TODO(server): 传给 `feed_stage` 的材料列表必须反映实际消费实例的加权 factor；多 stack / 多实例混合时按 count 做 weighted average，不再一律 `1.0`。
- [ ] TODO(server): 保持 rollback 语义：`feed_stage` 失败或后续 `consume_item_instance_once` 失败时，session 与 inventory 都恢复到投料前状态。
- [ ] TODO(test): 增加 server 单测覆盖 fresh=1.0、half-decayed=0.5、dead=0.0 三类材料投炉，断言 `session.staged.quality_factor` 和最终 `qi_gain` 按比例变化。
- [ ] TODO(bot-e2e): 增加 alchemy freshness 黑盒场景：dev 准备衰减药材，炼同一丹方后观察陈药产物贡献低于新鲜药材；若当前协议无法观察 `qi_gain`，先补可观察 server_data/chat 回执。

## 5. 对抗结论

第一轮对抗：
- inventory/crafting/alchemy reviewer 提出三个候选，其中“炼丹注入真元不扣账”属于 server-qi 分区，已排除；“任意 learn/ignite”与残卷 handoff 已知题相邻，需收窄；“freshness 固定 1.0”进入第二轮。

第二轮对抗：
- reviewer A 认为 `ignite` 缺 `LearnedRecipes` gate 成立，但建议避开已知 `alchemy_learn_recipe` 残卷 handoff 主题。
- reviewer B 推荐采用 freshness 候选：证据链更独立，直接命中真实 `alchemy_feed_slot` caller，且不撞 #969-#1088。

裁决：采用 freshness 投炉固定全鲜结算候选；本 plan 只记录问题和验收，不包含任何代码修复。
