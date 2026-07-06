# plan-g-interact-search-nearest-hijack-v1（骨架）

> **骨架（草案）**。一句话主题：统一交互键 `G` 的 TSY 搜刮主路径没有按既定的“准星命中容器”语义走，而是直接取 **3 格内最近** `TsyContainerView`；结果是**附近只要有可搜刮干尸/骨架/石匣，`G` 就会优先发 `start_search`，截胡本应落到准星目标上的 NPC 对话、玩家交易或开箱交互；反过来，玩家准星明明对着 4-5 格内容器，也完全不会产出搜刮候选**。影响是：TSY/废墟常见“贴着尸体与 NPC/箱子交互”的真实游玩路径会被错误改写，交互体验变得不稳定且难以预测。

> 立项动机：`plan-input-binding-v1` 明确把 `SearchContainer` 定义成“5 格内 raycast/准星命中可搜刮容器”，并把冲突规则写成“准星命中容器 > 准星命中玩家/NPC > 最近掉落物”；但当前实现里，`TsyContainerSearchIntentHandler` 既不读准星，也不保留 candidate 对应的实体，而是两次重算 `nearestInteractable(...)`。这不是“数值微调”，而是交互语义被改成了另一套模型，值得先以 skeleton 收口证据、玩家影响、修复面与验收抓手，再单独出 fix PR。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 统一交互键 `G` 的 TSY 搜刮候选/派发改回准星语义 | fix_pr | ⬜ |

## P0 — 统一交互键 `G` 的 TSY 搜刮候选/派发改回准星语义

- **现象**：`client/src/main/java/com/bong/client/tsy/TsyContainerSearchIntentHandler.java:16-26` 的 `candidate()` 不读 `client.crosshairTarget`，只从 `TsyContainerStateStore.nearestInteractable(client.player, 3.0)` 拿最近容器；`dispatch()`（`:30-36`）也再次调用 `nearest(client)`，直接把当下最近容器的 `entityId` 发给 `ClientRequestSender.sendStartSearch(...)`，完全不校验传入 `candidate.debugLabel` 对应的目标。
- **与既有交互模型的硬冲突**：容器开启路径 `client/src/main/java/com/bong/client/inventory/ContainerOpenIntentSupport.java:21-58`、NPC 对话 `client/src/main/java/com/bong/client/npc/NpcEngagementIntentHandler.java:15-45`、玩家交易 `client/src/main/java/com/bong/client/social/TradeOfferIntentHandler.java:18-43` 都是“先看准星命中的实体，再把同一个实体 id 带进 dispatch”。只有 TSY 搜刮走“按距离最近对象自动吸附”的独立语义，导致同一把 `G` 键下不同 handler 的目标选择口径不一致。
- **为什么这是 bug，不是设计**：`docs/finished_plans/plan-input-binding-v1.md:71-82` 把 `SearchContainer` 的触发条件写死为“**5 格内 raycast/准星命中可搜刮容器**”，并明确冲突规则是“**准星命中容器 > 准星命中玩家/NPC > 最近掉落物**”。当前实现把搜刮目标从“准星命中容器”退化成“3 格内最近容器”，等价于把 `SearchContainer` 从“准星类候选”偷偷改成了“距离类候选”，直接破坏了该文档锁定的优先级体系。
- **对实际游玩体验的影响**：玩家在 TSY/废墟/营地这类“容器、NPC、玩家、掉落物混站”的高频场景里，按 `G` 会出现**看着 A，却去搜 B** 的体感。典型例子是：脚边有可搜刮干尸/骨架时，准星对着 NPC、交易对象或箱子按 `G`，路由先看到 `SearchContainer(priority=100)`，会把本应触发的对话/交易/开箱截胡成 `start_search`；反过来，玩家准星对着 4-5 格内容器时，因 `MAX_INTERACT_DISTANCE=3.0` 又完全拿不到候选，表现成“明明瞄准了也按不动”。
- **建议修复范围 / 模块**：优先收口 `client/src/main/java/com/bong/client/tsy/TsyContainerSearchIntentHandler.java`、`client/src/main/java/com/bong/client/tsy/TsyContainerStateStore.java`、必要时补 `client/src/test/java/com/bong/client/tsy/*` 与 `client/src/test/java/com/bong/client/input/*`。修复方向应统一成与其他 interaction handler 一致的“candidate 读准星、dispatch 校验同一目标、距离门限只做准入而不做自动选目标”；若 3 格是 server 侧确需保留的门限，也应把“3 格”与“准星命中”同时表达清楚，而不是保留 nearest 语义。
- **验收抓手**：至少补 5 组 pin。1) 准星命中可搜刮容器且在合法距离内时，`G` 发 `start_search(container_hit_id)`。2) 脚边有别的更近容器时，准星命中的 NPC/玩家/箱子不会被 TSY 搜刮截胡。3) dispatch 不能忽略 `candidate`，同 tick store 变化也不应改发到另一具容器。4) 超出设计距离但准星命中时，候选缺失/拒绝原因与文档一致。5) 最近掉落物 fallback 仍只在无更高优先级准星候选时触发。

## 反方裁决摘要

1. Round 1（默认怀疑）主张“nearest 搜刮也许是故意的 QoL，文档可能过时”。人工复核后，这个说法被 `plan-input-binding-v1.md:73/81-82` 的两处硬约束直接否掉，因为文档写的是“5 格内 raycast/准星命中可搜刮容器”，不是“最近容器自动吸附”。
2. Round 2 继续怀疑“即使不看文档，nearest 也未必伤害实际体验”。补证后仍站不住：`SearchContainer` 的 priority=100（`ReservedInteractionIntents.java:4-5`）高于 `TalkNpc/TradePlayer` 的 90（`:6-7`），而 container/NPC/trade handler 都依赖准星实体；这意味着只要玩家 3 格内存在可搜刮容器，`G` 的目标选择就会与准星脱钩，真实把交互截成 `start_search`，不是抽象的代码洁癖问题。
3. 人工复核再补一层：`dispatch()` 二次 `nearest(client)` 而不是使用 `candidate.debugLabel`，说明问题不止是“候选筛选宽了”，连最终发包目标都能与最初候选脱钩；因此该候选在两轮对抗后继续存活。

## 开放问题

1. server 侧真实想保留的搜刮距离究竟是 3 格还是 5 格？当前 client 测试把 3.0 写成“matches server search validation”，但 `plan-input-binding-v1` 写的是 5 格；修复 PR 需要顺手把文档与双端门限统一。
2. 搜刮是否应该像开箱/对话一样完全依赖准星 `EntityHitResult`，还是允许“准星命中 + 轻微吸附到同实体包围盒”的弱纠偏？这属于实现细节，但不影响本 skeleton 的结论：**不能继续用纯 nearest 取代准星语义**。

## 审计来源

bug-hunt 线程 AJ（限定 `movement/interaction` 主路径，基线 `origin/main@fb41c96a4`）。本轮只收窄 `client/src/main/java/com/bong/client/input/`、`client/src/main/java/com/bong/client/interaction/`、`client/src/main/java/com/bong/client/movement/`、`client/src/main/java/com/bong/client/tsy/` 与 `server/src/network/client_request_handler.rs` 的交互接线；结论为 **report-only**：先提交 skeleton plan 固化玩家影响、证据链与修复抓手，再由后续 fix PR 单独落地。
