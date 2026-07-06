# plan-bughunt-botany-spirit-mice-spawn-kind-drift-v1

> **Active plan**。一句话主题：`BaiYanPeng` 的 `AttractsMobs(SpiritMice)` 支路并不会直接生成鼠群，而是先走 `spawn_beast_npc_at` 按通用 fauna 规则抽一只妖兽，再在返回后只把 `FaunaTag` 覆盖成 `Rat`。结果是：**外观 / raw entity kind / 血量沿用原抽中的蜘蛛或蝎蛇，掉落却按 Rat 结算，形成“看起来不是鼠、打起来也不是鼠、死了却掉鼠骨”的错配**。

> 立项动机：这是 botany 完成采集后的 fauna/mob 侧支路，玩家可达、现有测试只验 `FaunaTag` 因而会漏掉，且问题不属于刚出的灰烬蛛伪装名牌泄漏、`fauna audio fade stop ignored` 两题的重复变体。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | `SpiritMice` 引怪 species 漂移 / tag-only 覆盖 | fix_pr | ⬜ |

## P0 — `SpiritMice` 引怪 species 漂移 / tag-only 覆盖

- **复现路径**：`server/src/botany/registry.rs:155-165` 把 `BaiYanPeng` 配成 `HarvestHazard::AttractsMobs { mob_kind: FaunaKind::SpiritMice, min_count: 2, max_count: 5 }`。玩家正常采完后，`server/src/botany/harvest.rs:224-238` 会 emit `BotanyAttractsMobsEvent`；`server/src/botany/hazard.rs:228-301` 消费事件时，对 `MimicSpider` 之外的 fauna 一律走 `spawn_beast_npc_at(...)`，然后仅在 `:298-300` 用 `FaunaTag::new(beast_kind_for_botany(...))` 回写成 `Rat`。
- **根因链路**：`spawn_beast_npc_at` 在 `server/src/npc/spawn/beast.rs:61-63` 先根据 `home_zone + spawn_position` 算出 `fauna_tag`，并立刻在 `:65-67` 决定协议 `EntityKind`、在 `:117-119` 决定 `Wounds.health_*`、在 `:121-122` 决定 `FaunaVisualKind`。但 botany 支路返回后只覆盖 `FaunaTag`，不会回滚前面已经写死的 entity kind / visual / hp。也就是说，**“物种”在 spawn 时已经定案，后补 `FaunaTag` 只能改掉落路由，改不回实际实体表现**。
- **为什么在 `spawn` 区也会中招**：`spawn_beast_npc_at` 调的是 `fauna_tag_for_beast_spawn(home_zone, fauna_seed)`，底层 `server/src/fauna/components.rs:301-339` 在没有显式 `zone_qi` 时会走 `zone_qi.unwrap_or(0.3)`，即固定落到 `SPAWN_POOL_LOW_QI`。该池不是纯 Rat，而是 `Rat / Spider / JungleScorpion / CockadeSnake / GreenSpider` 混池（`server/src/fauna/components.rs:57-75, 327-339` 对应的种类和血量定义也不同：Rat 8 HP，Spider 25 HP，GreenSpider 30 HP）。因此普通 `spawn` 区采 `BaiYanPeng` 也会稳定抽到非 Rat 种类。
- **影响面**：1) 客户端看到的 custom entity kind / GeckoLib 外观可能是蜘蛛、绿蛛、蝎、蛇，而不是“SpiritMice”。2) 实体血量按原抽中的 beast 走，不按 Rat 走。3) 死亡掉落仍按 `FaunaTag` 结算；`server/src/fauna/drop.rs:129-137, 231-235` 明确 `Rat` 与 `Spider/GreenSpider` 掉落表不同，于是会出现“蜘蛛外观 / 鼠骨掉落”的错配。4) 因为 thinker 也是通用 beast thinker，这条 bug 还会把 botany 的轻度骚扰 hazard 意外抬成高体量战斗遭遇。
- **这个 bug 对实际游玩体验的影响**：玩家采 `BaiYanPeng` 时，预期是“引来 2-5 只噬元鼠”这种轻量惩罚；实际却可能刷出蜘蛛或其他低阶妖兽壳子，血量远高于 Rat，还在击杀后掉鼠骨。体感上会像“采药警报写的是鼠群，结果来的是别的怪；打起来比预期肉，掉落又对不上视觉”，直接破坏 hazard 的可读性、难度预期和掉落信任。
- **现有测试为什么没挡住**：`server/src/botany/hazard.rs:682-715` 的 `attracts_mobs_event_spawns_fauna_tagged_beasts` 只断言 `FaunaTag.beast_kind == Rat` 且 `NpcArchetype == Beast`，没有检查 `EntityKind`、`FaunaVisualKind`、`Wounds.health_max` 是否同样变成 Rat 语义，所以这条 tag-only 覆盖 bug 可以稳定漏网。
- **建议修复方向**：不要先走“通用 beast spawn 再补 tag”。修复 PR 应二选一并统一：A) 为 `SpiritMice` 建立显式 spawn helper，像 `MimicSpider` 一样在 spawn 时一次性写对 kind / visual / hp / tag；B) 给 `spawn_beast_npc_at` 增加显式 `BeastKind` 参数或专门的 override 入口，让 botany hazard 在 spawn 前就锁定 `Rat`。无论选哪条，都不能继续保留“先随机种类、后覆盖 tag”的两阶段写法。
- **验收抓手**：至少补 4 组 pin。1) `BaiYanPeng` 触发 `SpiritMice` 时，实体 `EntityKind` / `FaunaVisualKind` / `FaunaTag` / `Wounds.health_max` 必须全部一致指向 Rat。2) `spawn` 区与非关键字 zone 名下都不能再抽到低灵气混池里的其他物种。3) 击杀后掉落与外观一致，不再出现 spider shell 掉鼠骨。4) 现有 `attracts_mobs_event_spawns_fauna_tagged_beasts` 要升级为全套契约测试，而不是只看 tag。

## 反方裁决摘要

1. **Round 1（退化处理：当前会话无 subagent / delegate 工具，只能由主代理执行本地反方裁决）**
反方论点：也许这是有意设计，`SpiritMice` 只是掉落语义，不要求外观真是 Rat。
驳回理由：`server/src/botany/registry.rs:160-163` 的 hazard 字段名就是 `mob_kind`，`server/src/botany/hazard.rs:327-330` 也把 `SpiritMice -> BeastKind::Rat` 明确建模成“刷鼠”；若只想借 Rat 掉落表，不会在 spawn 后额外覆盖 `FaunaTag`。当前实现明显是“想刷 Rat，但刷怪 helper 选错入口”。
2. **Round 2（同样退化为主代理本地反方裁决）**
反方论点：即便先随机 species，后面覆盖 `FaunaTag` 也许已经足够，因为 combat/loot 主要看 tag。
驳回理由：`spawn_beast_npc_at` 在覆盖前已经写入 `EntityKind`、`FaunaVisualKind`、`health_max`（`server/src/npc/spawn/beast.rs:61-63, 65-67, 117-122`）；这些不是后续读 `FaunaTag` 就会自动同步的派生字段。再加上 `drop_table_for` 按 tag 走、视觉和血量按原 species 走，已经构成用户可见的不一致，不是“内部实现细节不同但对玩家无感”。
3. **人工终裁**
两轮反方都没能提出任何能把 entity kind / visual / hp 与 tag 自动重新对齐的后续系统；相反，现有单测只验 tag 恰好解释了为何 bug 长期存活。因此将该候选保留为高置信 REAL，并建议后续 fix PR 直接从 spawn helper 入口做结构性收口。

## 审计来源

bug-hunt 定点轮（当前 worktree / fauna-mob sidepaths，避开灰烬蛛伪装名牌泄漏与 fauna audio fade stop ignored）。本结论为 **report-only**：本次只新增 skeleton，不改源码；后续修复应单独出 fix PR。
