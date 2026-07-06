# plan-bughunt-war-emergent-group-reputation-gap-v1（骨架）

> **骨架（草案）**。一句话主题：离屏战事已经把散修稳定 seed 到 **4 个** `EmergentGroupId`，但 `award_war_winner_renown` 在把战事胜方奖励桥接到 `FactionReputationDeltaEvent` 时只认 `0/1/2` 三个 group；当合法可达的 `winner_group=3` 获胜时，玩家仍会拿到全局 fame，却被静默跳过 faction reputation，导致该战区的 NPC 态度 / 交易折扣 / faction-tier 永远不涨。

> 立项动机：这不是“匿名 war 不给派系信誉”的完整设计决议，而是**只有第 4 个合法群体掉桥**的半截实现。当前主干甚至已经有单测把“unknown winner group 不发 faction reputation”锁成了正确行为；如果不单独立 skeleton，把生产可达链、影响面和修复决策讲清，后续很容易继续把 `group_id=3` 误当“测试伪输入”。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | war 第 4 个涌现群体获胜时 faction reputation 奖励断桥 | plan_skeleton | ⬜ |

## P0 — war 第 4 个涌现群体获胜时 faction reputation 奖励断桥

- **复现路径**：
  1. `server/src/npc/faction.rs:97-100` 把离屏散修 seed 群体数固定为 `EMERGENT_GROUP_COUNT = 4`，不是 2 也不是 3。
  2. `server/src/npc/dormant/mod.rs:1445-1451` 的 `seed_emergent_group()` 直接对 `EMERGENT_GROUP_COUNT` 取模；`server/src/npc/dormant/mod.rs:3141-3177` 现有测试还专门要求“256 个 char_id 至少覆盖 3 个不同群体”且 seed 出来的 snapshot 必须带显式 `emergent_group`。这说明 **group 3 是生产可达输入，不是测试伪造值**。
  3. 玩家参与 war 后，`server/src/npc/war/settle.rs:125-157` 在 `WarPhase::Settling` 对胜方 `Enlist` / `Mercenary` 先无条件发送 `SocialRenownDeltaEvent`，随后再尝试把 `winner_group` 映射到 `NamedFactionId` 发 `FactionReputationDeltaEvent`。
  4. 但同文件 `:162-168` 的 `named_faction_for_war_group()` 只映射 `0 -> QingyunHunters`、`1 -> CangyuanMerchants`、`2 -> NorthWasteDrifters`，其他一律 `None`；于是合法可达的 `winner_group=3` 会走 `:144-149` 的 `warn! + continue`，**全局 fame 发了，faction reputation 没发**。
  5. `server/src/social/components.rs:277-307` 说明 `FactionReputation` 的唯一真相源就是 `per_faction`；`server/src/network/client_request_handler.rs:9880-9927`、`:9946-9964` 进一步把它接到 NPC 交互上：`tier_for_zone(zone)` 决定 faction tier，`reputation_to_player_score_for_npc_zone()` 把 faction reputation 直接加进 NPC 对玩家态度分值。于是 group 3 胜场奖励缺失会**真实落到玩家可感知行为**。

- **根因链路**：
  1. `plan-offscreen-war-v1` 的 reframe b 把离屏敌对关系从旧 `Attack/Defend` 二分升级成 `>2` 的匿名涌现群体，代码落地为 `EMERGENT_GROUP_COUNT = 4` + `seed_emergent_group()` 取模分发。
  2. 后续 faction reputation 体系仍是 `NamedFactionId` 三键模型，war 结算没有保存“这个匿名群体对应哪个 named faction”的真实语义，只是临时写了一个 **按数字位置硬编码的 3 项桥接表**。
  3. 这张桥接表没有和 `EMERGENT_GROUP_COUNT`、`NamedFactionId::all()`、zone anchor 或玩家挂靠关系做任何一致性约束，于是第 4 个合法群体天然掉出表外。
  4. `server/src/npc/war/settle.rs:652-687` 现有测试还把“99 这种 unknown winner group 仍给 fame、但不给 faction reputation”视为正确行为，结果把真正的生产缺口一起**合法化**了。

- **为什么这是 bug，不是设计**：
  - 如果设计真的是“匿名 war 完全不应该写 faction reputation”，那 `award_war_winner_renown()` 就不该对 `0/1/2` 三个 group 特判发 `FactionReputationDeltaEvent`；应该是**所有匿名群体都不写**。
  - 当前实现却是 0/1/2 有派系信誉收益，只有合法存在的第 4 群体没有。这不是一致的设计策略，而是**桥接表与生产 group 空间失配**造成的非对称漏发。
  - 更关键的是，代码自己已经承认“>2 群体互殴”是 reframe b 的目标（`seed_emergent_group_distribution_covers_at_least_three_groups` 的测试名与断言文案都在锁这个目标），所以把第 4 群体当成“unknown”本身就和上游设计冲突。

- **影响面**：
  - `SocialRenownDeltaEvent` 与 `FactionReputationDeltaEvent` 从一次战事结算开始分叉：同一场 group 3 胜利，玩家 fame 上涨，但 faction reputation 维持旧值。
  - `FactionReputation.tier_for_zone()` 不涨，意味着该 zone 对应 named faction 的 NPC 交互不会进入更高 tier。
  - `reputation_to_player_score_for_npc_zone()` 少掉 faction reputation 这段加成，NPC 对玩家的态度分会长期低于同等战果的 group 0/1/2 玩家。
  - 交易侧 `NpcEngagementTarget::can_trade()` / 价格分层继续读旧 tier，group 3 战功玩家拿不到原本应有的高信誉折扣与态度改善。

- **这个 bug 对实际游玩体验的影响**：
  - 玩家从体感上会遇到**“明明打赢了战事、系统也给了 fame，但本地势力 NPC 还是把你当路人”**的割裂。
  - 同样是参战获胜，投到 group 0/1/2 的玩家能累计 faction reputation，投到合法可达的 group 3 的玩家却只有半套奖励；这会把一次本该稳定反馈到 NPC 态度/交易价格的 war 结果，变成**看不见规则的随机吃亏**。
  - 长期玩下来，玩家会误以为 war 的 faction reputation 奖励“不稳定”或“有时失灵”，因为缺口不是整条链都坏，而是只在部分胜方群体上漏发。

- **建议修复方向**：
  - 先拍板语义，再动代码。至少要在下面两条里二选一，不能继续停在现在这种 3/4 群体半接线状态：
  - 方案 A：**war 奖励确实应该写 faction reputation**。那就不要再用 `winner_group -> NamedFactionId` 的裸数字桥接；改为从真实 named-faction 语义源派生（例如 zone anchor、玩家挂靠的 `NamedFactionMembership`、或 war runtime 显式携带的 faction key），并补一个 pin：`EMERGENT_GROUP_COUNT - 1` 的合法最高 group 获胜时也必须产生 faction reputation。
  - 方案 B：**匿名 war 不应该写 faction reputation**。那就把 `award_war_winner_renown()` 里对 0/1/2 的 `FactionReputationDeltaEvent` 全部移除，仅保留 fame；同时回标相关文档，避免继续制造“有的匿名群体能改 faction reputation，有的不能”的伪规则。

- **验收抓手**：
  1. 构造 `winner_group = EmergentGroupId(EMERGENT_GROUP_COUNT - 1)` 的 Settling 事件，断言奖励链行为与其它合法群体一致，不再出现“fame 有、faction reputation 无”的分叉。
  2. 补一致性 pin：war 奖励侧若仍保留 group→faction 桥接，则桥接覆盖范围必须和合法生产 group 空间显式对齐，不能再默默把合法 group 当 unknown。
  3. 加交互回归：同 zone 下，group 3 获胜后的玩家在 NPC engagement / trade 路径里，其 `faction_reputation_tier` 与 `reputation_to_player` 必须发生预期变化。

## 反方裁决（两轮）

> **退化处理说明**：本会话当前无法再开 subagent；以下两轮改为主代理手工做“反方论点 → 证据复核 → 驳回理由”的对抗裁决，保持记录诚实。

1. **Round 1 反方论点**：`group_id=3` 也许只存在于测试，生产仍然只会落到 0/1/2，所以桥接表少一项不构成真实 bug。
   **驳回理由**：`EMERGENT_GROUP_COUNT = 4` 是生产常量；`seed_emergent_group()` 直接对 4 取模；而且 `seed_emergent_group_distribution_covers_at_least_three_groups` 与 `dormant_rogue_seed_snapshot_assigns_explicit_emergent_group` 两个现有测试都在锁“生产 seed 必须覆盖 >2 群体、并写入显式 emergent_group”。这不是 test-only 输入。

2. **Round 2 反方论点**：也许“unknown emergent group 不写 faction reputation”是刻意设计，因为匿名 war 本就不该影响 named faction 信誉。
   **驳回理由**：如果这是设计，当前实现就不该让 `0/1/2` 三个匿名群体继续写 `FactionReputationDeltaEvent`。现状是只有第 4 个合法群体被排除，形成非对称行为；同时 NPC 交互与交易真实消费 `FactionReputation`，说明这条链在产品层面本来就是要让玩家感知到的。故“匿名 war 一律不写 faction reputation”不能解释为什么只有 group 3 掉桥，只能说明桥接实现不完整。

## 开放问题

1. `FactionReputation` 的真实语义锚点到底该是 `NamedFactionId`、`zone anchor`，还是“玩家在该 war 中投靠的某个 named faction”而非匿名 `EmergentGroupId`？修复前需要先定单一真相源。
2. 现有 `settle_skips_faction_reputation_for_unknown_winner_group` 测试是否应该直接改成“合法最高 group 也必须拿到完整奖励”，还是拆成“非法 group 保持拒绝、合法 group 全覆盖”的双测试，避免继续把生产缺口合法化。

## 审计来源

本轮 bughunt 限定在 combat / skill / war 侧路径，刻意避开近期已出的 tribulation concurrent broadcast、movement dash HUD、war preview、npc particles 等题。当前结论来自对 `npc/dormant`、`npc/war`、`social`、`network/client_request_handler` 的代码链复核；属于 **report-only skeleton**，不包含源码修复。
