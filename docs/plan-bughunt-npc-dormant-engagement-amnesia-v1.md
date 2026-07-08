# plan-bughunt-npc-dormant-engagement-amnesia-v1

> **active bughunt plan**。一句话主题：`server/src/npc` 的 **dormant → hydrate 往返不会持久化 NPC 的玩家交互记忆与 per-player 信誉**，导致同一 `char_id` 的 NPC 只要被拉远脱水再回来，就会把“你打过我 / 交易过我 / 我对你有折扣或敌意”全部洗回首次见面的默认态。

## 结论

- **类型**：真实 bug，`fix_pr`
- **范围**：`server/src/npc/hydrate`、`server/src/npc/dormant`、`server/src/npc/interaction_memory`、`server/src/npc/brain/threat`、`server/src/network/client_request_handler`、`server/src/network/npc_bubble`，以及对应 client 交互入口 `client/src/main/java/com/bong/client/npc/NpcEngagementIntentHandler.java`
- **一句话根因**：live ECS 上会被持续写入的 `NpcMemoryComponent` / `NpcPlayerReputation` 根本没进 `NpcDormantSnapshot`，hydrate 也不回填；新实体随后被 `attach_*_components` 自动补成默认空组件，等价于“脱水一次就失忆”

## 复现路径

### 路径 A：攻击记忆 / 敌意失忆

1. 靠近一个可交互 NPC，client 通过 `NpcEngagementIntentHandler` 发起 NPC 交互（`client/src/main/java/com/bong/client/npc/NpcEngagementIntentHandler.java:15-45`）。
2. 先攻击该 NPC 一次；`record_attack_memories` 会把 `NpcInteractionType::Attack` 写入 `NpcMemoryComponent`，并设置 retaliation target（`server/src/npc/interaction_memory.rs:153-180`）。
3. 继续拉远到 `dehydrate_radius_blocks` 之外，让 `dehydrate_far_npcs_system` 把该 NPC 变成 dormant snapshot（`server/src/npc/hydrate/mod.rs:345-437`）。
4. 再回到 `hydrate_radius_blocks` 内，让同一个 `char_id` 被 `spawn_from_snapshot` 水化回来；`Lifecycle.character_id` 明确沿用旧 `snapshot.char_id`（`server/src/npc/hydrate/mod.rs:735-739`）。
5. 观察：
   - `compute_threat_assessments` 再也读不到此前的 Attack/Trade/Theft 记忆，只会按空记忆分支走 `was_attacked=false / has_traded=false / was_robbed=false`（`server/src/npc/brain/threat.rs:320-349`）。
   - world-space 气泡会从 memory bubble 退回 generic greeting，因为 `bubble_content_for_pair` 只看 `NpcMemoryComponent.interactions.last()`（`server/src/network/npc_bubble.rs:233-246`）。

### 路径 B：交易信誉 / 折扣失忆

1. 与同一 NPC 完成一轮交易，`NpcTradeRequest` 成功后会记录 `NpcInteractionType::Trade`（`server/src/network/client_request_handler.rs:1494-1506`）。
2. 若该 NPC 还有领地加成或 gossip 改价，其 `NpcPlayerReputation` 会被持续调整；例如 territory dominance 明确调用 `rep.adjust(...)`（`server/src/world/territory_perks.rs:148-156`）。
3. 把 NPC 拉远脱水，再回来触发 hydrate。
4. 再次交易时，服务端只会从 `target.npc_player_rep` 读取 per-player 分值计算资格/折扣（`server/src/network/client_request_handler.rs:1363-1386`）；但该组件已在往返中丢失，表现会退回默认 0.5 中立分（`server/src/npc/trade.rs:77-97`）。

## 证据链

### 1. dormant snapshot 根本没有 memory / reputation 字段

- `NpcDormantSnapshot` 当前只有 cultivation、lifespan、life_record、faction、patrol、loot、guardian/tsy 等字段，**没有** `NpcMemoryComponent`，也**没有** `NpcPlayerReputation`（`server/src/npc/dormant/mod.rs:276-330`）。

### 2. dehydrate 采样时也完全没读这两个组件

- `dehydrate_far_npcs_system` 构造 `NpcDormantSnapshot` 时写入了一整套字段，但没有任何 `NpcMemoryComponent` / `NpcPlayerReputation` 读取或序列化逻辑（`server/src/npc/hydrate/mod.rs:363-437`）。

### 3. hydrate 时不会恢复；缺组件会被自动补成默认空值

- `spawn_from_snapshot` 只把 snapshot 里的 archetype / cultivation / faction / patrol / guardian / tsy 等组件插回新实体，没有恢复 memory / reputation（`server/src/npc/hydrate/mod.rs:724-791`）。
- `interaction_memory::register` 里有两个 catch-all system：
  - 缺 `NpcMemoryComponent` → `insert(NpcMemoryComponent::default())`
  - 缺 `NpcPlayerReputation` → `insert(NpcPlayerReputation::default())`
  见 `server/src/npc/interaction_memory.rs:113-141`。
- 即使补默认组件的系统尚未跑到，本轮相关消费者也都是 `Option<&...>` 读取；缺失就等价于“空记忆 / 中立信誉”，同样丢语义。

### 4. 这两个组件不是“可丢的 UI cache”，而是 gameplay 状态

- 攻击会写入 `NpcMemoryComponent` 并驱动 retaliation（`server/src/npc/interaction_memory.rs:153-180`）。
- 交易成功会写入 `NpcInteractionType::Trade`（`server/src/network/client_request_handler.rs:1498-1506`）。
- brain threat 直接用 memory 影响 `decide_self_interest_with_memory(...)`（`server/src/npc/brain/threat.rs:320-349`）。
- bubble 直接用 memory 决定给玩家显示 memory bubble 还是 generic greeting（`server/src/network/npc_bubble.rs:233-246`）。
- 交易资格 / 折扣直接读 `NpcPlayerReputation`（`server/src/network/client_request_handler.rs:1363-1386`），而该组件默认分就是 0.5（`server/src/npc/trade.rs:77-97`）。

## 根因链路

1. 玩家通过 client 交互链路靠近 NPC 并发起 inspect / dialogue / trade（`client/src/main/java/com/bong/client/npc/NpcEngagementIntentHandler.java:15-45`）。
2. 服务端在 live ECS 上不断积累 NPC 对玩家的交互历史与信誉：
   - 攻击写 `NpcMemoryComponent`
   - 成功交易写 `NpcMemoryComponent`
   - territory/gossip 等修改 `NpcPlayerReputation`
3. NPC 进入 dormant 时，`dehydrate_far_npcs_system` 只保存 snapshot 白名单字段，没有保存上面两类状态（`server/src/npc/hydrate/mod.rs:363-437`）。
4. hydrate 时，`spawn_from_snapshot` 用相同 `char_id` 生成了一个**身份连续但状态缺口**的新实体（`server/src/npc/hydrate/mod.rs:735-739`）。
5. 后续 catch-all attach system 把缺失的 memory / reputation 补成默认空组件（`server/src/npc/interaction_memory.rs:125-141`）。
6. brain / bubble / trade 读到的因此是“首次见面”默认态，而不是该 NPC 真实积累出来的玩家关系。

## 这个 bug 对实际游玩体验的影响

- 玩家把一个被自己打伤、理应记仇的 NPC 拉远再回来后，对方会像没见过你一样重新 greeting，仇怨 / 提防 / 记忆气泡直接蒸发。
- 玩家好不容易靠多次交易、领地驻守或 gossip 积累出来的专属折扣 / 交易态度，只要该 NPC 脱水一次就会掉回中立；体感上就是“我刚刷出来的关系值被读档洗没了”。
- 这类失忆不会改 `char_id`，所以从玩家视角它不是“换了一个 NPC”，而是**同一个人翻脸不认账 / 失去记忆**，非常破坏 NPC 持续关系感。
- 因为 dormant/hydrate 是常规视距机制，不需要极端操作；正常跑图、离开 zone、回头再来都会触发。

## 影响面

- **brain**：`compute_threat_assessments` 的 memory bias 永久失效到下次重新积累（`server/src/npc/brain/threat.rs:320-349`）。
- **engagement / trade**：per-player 信誉回到默认 0.5，影响价格、稀有品拒售、彻底拒绝交易等分支（`server/src/network/client_request_handler.rs:1363-1386`）。
- **bubble / 反馈**：NPC 不再给出“上次你打过我 / 骗过我”的记忆气泡，只剩泛化 greeting（`server/src/network/npc_bubble.rs:233-246`）。
- **长期关系系统**：territory dominance / gossip 对单个 NPC 的关系沉淀会被 dormant 往返吞掉（`server/src/world/territory_perks.rs:148-156`）。

## 修复建议

1. 给 `NpcDormantSnapshot` 增加 `memory: Option<NpcMemoryComponent>` 与 `player_reputation: Option<NpcPlayerReputation>`（都用 `#[serde(default)]` / `skip_serializing_if` 做非破坏迁移）。
2. `dehydrate_far_npcs_system` 把 live ECS 上的 `NpcMemoryComponent` 与 `NpcPlayerReputation` 一并写进 snapshot。
3. `spawn_from_snapshot` 在 hydrate 时优先恢复这两个组件，避免落入 `attach_*_components` 的默认补丁分支。
4. 回归测试至少补四条：
   - `dehydrate_snapshot_carries_npc_memory_and_player_reputation`
   - `hydrate_roundtrip_preserves_attack_memory_for_same_char_id`
   - `hydrate_roundtrip_preserves_trade_rep_tier`
   - `threat_and_trade_semantics_survive_dormant_roundtrip`

## 现有测试为什么没挡住

- `server/src/npc/hydrate/mod.rs` 的测试 helper `snapshot(...)` / `disciple_snapshot(...)` 本身就不含 memory / player_reputation 字段（`server/src/npc/hydrate/mod.rs:845-882`, `1099-1142`）。
- 现有测试覆盖 guardian relic / TSY hostile / tribulation / combat_dead_pending_release 等路径，但全仓对 `NpcMemoryComponent` / `NpcPlayerReputation` 在 dormant roundtrip 的断言是空白。
- 这意味着当前测试集已经把“丢 memory / 丢 per-player reputation”默认为合法状态，所以不会报红。

## 两轮反方裁决

> 本会话未启用可用的 subagent / delegate 能力；以下按用户要求做了**同会话两轮自对抗裁决**。退化处理已如实记录，但论点与驳回均基于实读代码，不是凭空猜测。

### 第一轮反方：这会不会只是“故意不持久化软状态”？

- **反方论点**：`NpcMemoryComponent` / `NpcPlayerReputation` 可能只是近场 UI/气氛状态，不值得进 dormant snapshot；脱水后重置也许是刻意做的简化。
- **驳回理由**：
  - `NpcMemoryComponent` 不只喂 UI。它直接参与 retaliation 与 `brain/threat` 的自利决策（`server/src/npc/interaction_memory.rs:153-180`、`server/src/npc/brain/threat.rs:320-349`）。
  - `NpcPlayerReputation` 不只喂显示。它直接参与交易资格与价格分支（`server/src/network/client_request_handler.rs:1363-1386`）。
  - 现有 snapshot 已经持久化 `life_record`、`death_registry`、`faction` 等“关系/经历类”状态，说明设计目标不是“所有软状态都可以丢”，而是“同一 char_id 的关键持续状态应连续”。
  - 因此这不是“可接受的近场 cache 丢失”，而是 gameplay 语义断裂。

### 第二轮反方：会不会只是短暂空窗，下一次交互又能重新积累，不算 bug？

- **反方论点**：memory / rep 就算丢了，玩家再打一次、再交易一次就能重新写回；问题也许只是一次性轻微退化。
- **驳回理由**：
  - 触发条件不是边缘 case，而是常规视距脱水；玩家正常跑图就会反复发生。
  - 丢失的是**已积累的关系历史**，不是单次即时效果。尤其高信誉折扣、gossip 传播、记仇态度，本来就依赖时间累积，洗回默认值的破坏性远高于“重新打一拳就好”。
  - 同一 `char_id` 被 hydrate 回来（`server/src/npc/hydrate/mod.rs:735-739`），玩家明确会把它认成同一个 NPC；在这种身份连续前提下失去历史关系，属于玩家可直接感知的行为回退。

## 审计说明

- 本轮只做 bughunt，不修代码。
- 本轮只新增 skeleton，不改源码与其他 docs。
- 已与最近明确题目去重：这不是 `npc trade gate`、不是 `TSY sentinel`、不是 `social renown`、不是 `craft`、也不是 `tribulation`；核心是 **dormant/hydrate 对 NPC engagement state 的持久化缺口**。
