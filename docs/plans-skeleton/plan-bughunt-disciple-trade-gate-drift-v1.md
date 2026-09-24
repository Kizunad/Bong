# BugHunt: Disciple NPC 商铺 100% 购买必败——交易门禁白名单与真实交易库存漂移

## Bug 摘要

严重度：medium（skeptic 未调整，维持 unchanged）。

`server/src/npc/trade.rs::assign_npc_trade_inventory` 明确把 `NpcArchetype::Disciple` 列为可交易 archetype（2-4 件报价，按 realm 解锁），并且真实生产 spawn 路径（`server/src/npc/spawn/disciple.rs`、`server/src/npc/faction.rs`、`server/src/npc/hydrate/mod.rs`）每次生成 Disciple NPC 都会调用它并把结果作为 `NpcTradeInventory` 组件插到实体上。`server/src/network/npc_metadata.rs` 构建 `NpcMetadataS2c.trade_offers` 时对该组件不做任何 archetype 过滤，因此 Disciple 的真实报价（模板/数量/骨币价格）会原样广播给附近客户端；客户端 `NpcMetadata.tradeCandidate()` 已经是"数据驱动"实现（非敌对 + `trade_offers` 非空即视为可交易），于是玩家会看到"看看你有什么好东西"选项、能打开 `NpcTradeScreen`、能看到货物与价格。

但服务端真正处理购买请求的两处门禁——`server/src/network/client_request_handler.rs::npc_trade_catalog_entry()`（静态 `(archetype, item_id) -> (template_id, price)` 匹配表，只覆盖 `Commoner | Rogue`）和 `NpcEngagementTarget::can_trade()`（`matches!(archetype, Rogue | Commoner)` 正向白名单）——从未被同步更新为包含 `Disciple`。两处任何一处都会先短路拒绝：`npc_trade_catalog_entry` 在代码顺序上先于 `can_trade()` 被调用，对 Disciple 恒返回 `None`，所以请求会先在"没有这件货"分支被拒绝，根本走不到 `can_trade()`。净效果一致：**Disciple archetype 的每一次购买请求，无论物品/价格/信誉/境界，都 100% 必然失败**，而 UI 全程展示"可交易"，是 write 侧（数据生成 + 客户端展示）与 consume 侧（服务端购买门禁）对"谁能交易"定义发生的具体、可验证的 archetype 白名单漂移。

## 实际游玩体验影响

玩家在 `NPC_METADATA_SYNC_RADIUS`（64 格）内右键交互任意 Disciple（宗门弟子/首领）NPC，对话框正常显示"看看你有什么好东西"，点击后客户端**立即**（不等服务端确认）弹出交易屏，展示真实的模板 ID、数量与骨币价格——玩家会认为这是一次正常可用的交易入口。选择任意货物点击确认后，服务端却总是回一句"§c[NPC] {name} 没有这件货。"（少数情况下如果未来重构掉 catalog 门禁则会变成"不做买卖"），且没有任何随机性或前置条件差异掩盖这一点——不需要特定信誉、特定境界，也不是"运气不好"。玩家会把这当成"这个 NPC 的商店坏了 / 服务器 bug"，而实际上 Disciple 这整个 archetype（宗门弟子、宗门首领等末法常见人物）从设计上就应该能交易，是门禁代码没跟上数据侧扩展。UI 持续误导 + 100% 必败的组合会直接打击玩家对"宗门弟子能买东西"这一常见预期的信任。

## 证据定位

- `server/src/network/client_request_handler.rs:14396-14400`（`NpcEngagementTarget::can_trade()`）：`matches!(self.archetype, NpcArchetype::Rogue | NpcArchetype::Commoner)` 正向白名单，未含 `Disciple`。
- `server/src/network/client_request_handler.rs:1409-1423`：`NpcTradeRequest` 处理链路里，`npc_trade_catalog_entry(target.archetype, &requested_item_id)` 在 `can_trade()`（第 1424 行）**之前**被调用；对 Disciple 恒返回 `None`，直接以 `"§c[NPC] {name} 没有这件货。"` 拒绝并 `continue`，`can_trade()` 根本不会被执行到。
- `server/src/network/client_request_handler.rs:14482-14508`（`npc_trade_catalog_entry`）：静态 `match (archetype, requested_item_id)` 只列出 `NpcArchetype::Commoner` 与 `NpcArchetype::Rogue` 的分支，`_ => None`，无 `Disciple` 分支——独立于 `can_trade()` 的第二套硬编码白名单。
- `server/src/network/client_request_handler.rs:1452-1469`：真正扣款前的实际报价查找，用的是活体 ECS `NpcTradeInventory.offers`（`trade_inventory.offers.iter().find(|offer| offer.template_id == template_id)`）——这是与 `npc_trade_catalog_entry` 平行的第三套"报价来源"，说明架构里已经有一份实时可信的数据（`NpcTradeInventory`），`npc_trade_catalog_entry` 只是多余的静态前置门。
- `server/src/npc/trade.rs:563-595`（`assign_npc_trade_inventory` 文档 + 实现）：注释明确写"`Disciple`: 2-4 件，按 realm 解锁"，`match archetype` 里 `NpcArchetype::Disciple => (2, 4)` 与 `Commoner`/`Rogue` 并列为可交易分支；`GuardianRelic | Daoxiang | Zhinian | Beast | SkullFiend | Fuya | Zombie | DyingElder | Mundane` 才是"非交易 archetype"（返回空 `offers`）。
- `server/src/npc/spawn/disciple.rs:156-160`：真实 spawn 路径 `spawn_disciple_npc_at()` 调用 `assign_npc_trade_inventory(NpcArchetype::Disciple, realm, entity.index() as u64)` 并 `commands.entity(entity).insert((known_techniques, NpcLastTechniqueTick::default(), trade_inv))` 把结果直接插到实体上——不是测试脚手架，是生产 spawn 路径。
- `server/src/npc/faction.rs:1042`：世界初始化时用 `spawn_disciple_npc_at` 生成宗门首领（named-faction leader），确认 Disciple archetype 在正常游玩（非 dev-only 命令）路径下必然出现。
- `server/src/npc/hydrate/mod.rs:751`：服务器重启后按持久化快照重新 hydrate 时，`NpcArchetype::Disciple => spawn_disciple_npc_at(...)` 同样是真实调用点，与 `command_executor.rs` 里的 dev-only 调用点并列但不依赖它。
- `server/src/network/npc_metadata.rs:317-330`：`trade_vec` 构建对 `trade_inventory` 只做 `Option::map(...).unwrap_or_default()`，没有任何按 `archetype` 的过滤或跳过——Disciple 的真实 offers 原样进入 `NpcMetadataS2c.trade_offers` 广播。
- `client/src/main/java/com/bong/client/npc/NpcMetadata.java:150-156`（`tradeCandidate()`）：注释明确写 `"Data-driven trade candidacy...Replaces the old hardcoded archetype check."`，实现为 `!hostile() && tradeOffers != null && !tradeOffers.isEmpty()`——完全不看 archetype。
- `client/src/main/java/com/bong/client/npc/NpcDialogueScreen.java:101-106`：`if (metadata.tradeCandidate())` 为真时渲染"看看你有什么好东西"，点击后立即 `MinecraftClient.getInstance().setScreen(new NpcTradeScreen(metadata))`——不等待服务端任何确认。
- `client/src/main/java/com/bong/client/npc/NpcTradeScreen.java:203-204`：确认购买时 `ClientRequestSender.sendNpcTradeRequest(metadata.entityId(), List.of(), offer.templateId())`——客户端发送的 `requested_item_id` 就是服务端自己生成、自己广播出去的**真实 canonical `template_id`**，不是别名字符串，说明 `npc_trade_catalog_entry` 里的别名归一化（`lingcao`/`spirit_grass` 等）对当前客户端流程而言是多余的中间层。
- 既有测试基础设施（非本 bug 但影响修复设计）：`server/src/network/client_request_handler.rs:6011-6093`（`npc_trade_request_rejects_wanted_player_through_engagement_wiring`，走真实 `CustomPayloadEvent` → ECS App 的集成测试范式）与 `:6095-6180` 附近的 `run_npc_trade_request_with_context` 测试 helper（当前硬编码 `NpcArchetype::Commoner` 生成 NPC，第 6166 行左右）——修复的回归测试应扩展/复用这套 harness，而不是另起一套。`server/src/network/client_request_handler.rs:18991-19100` 附近是 `npc_trade_catalog_entry` 现有的 pin 测试矩阵，覆盖 Commoner/Rogue/Beast/Zombie 的正反用例，重构该函数签名时必须同步改造，不能留下语义失配的死测试。

## 触发路径

1. 世界初始化（`faction.rs:1042` 生成宗门首领）或服务器重启 hydrate（`hydrate/mod.rs:751`）或常规宗门弟子刷新，都会走 `spawn_disciple_npc_at()` 真实 spawn 出一个 `NpcArchetype::Disciple` 实体，并调用 `assign_npc_trade_inventory(Disciple, realm, seed)` 生成 2-4 条真实报价，`insert` 为该实体的 `NpcTradeInventory` 组件。
2. `npc_metadata.rs` 构建广播 payload 时对 `NpcTradeInventory` 不做 archetype 过滤，把这些真实报价（`template_id`/`display_name`/`count`/`price_bone_coins`）原样塞进 `NpcMetadataS2c.trade_offers`，通过 `bong:npc_metadata` 发给 64 格半径内的客户端。
3. 玩家右键交互该 Disciple NPC；客户端 `NpcMetadata.tradeCandidate()`（非敌对 + `trade_offers` 非空）判定为可交易，`NpcDialogueScreen` 渲染"看看你有什么好东西"选项。
4. 玩家点击该选项，`NpcDialogueScreen` **立即**（不等服务端任何确认）`setScreen(new NpcTradeScreen(metadata))`，展示服务端广播过来的真实货物列表与骨币价格。
5. 玩家在 `NpcTradeScreen` 里选中任意一条展示出的货物并确认，客户端发送 `sendNpcTradeRequest(entityId, [], offer.templateId())`——`templateId` 就是第 2 步服务端自己生成、自己广播出去的 canonical 值。
6. 服务端 `NpcTradeRequest` 处理链：`resolve_npc_engagement_target()` 成功解析出 `target`（`archetype = Disciple`）→ 先调用 `npc_trade_catalog_entry(Disciple, template_id)`；该函数只覆盖 `Commoner | Rogue`，对 Disciple 恒返回 `None`，于是**在到达 `can_trade()` 之前**就以 `"§c[NPC] {name} 没有这件货。"` 拒绝并 `continue`。
7. 即便未来有人重构掉这道 catalog 前置门（比如误以为只有 `can_trade()` 才是权威门禁），请求仍会在紧接着的 `can_trade()`（`matches!(archetype, Rogue | Commoner)`）处第二次被拦下，改以 `"§c[NPC] {name} 不做买卖。"` 拒绝。
8. 结果：不论 offer 是什么、价格多少、玩家骨币是否充足、信誉/境界如何，Disciple archetype 的购买请求 100% 必然失败；而客户端 UI 从第 3 步起就一直展示"此 NPC 可交易"，二者产生持续性的体验断裂，不是偶发也不需要特殊触发条件——只要世界里存在一个 Disciple NPC（宗门弟子/首领在正常游玩中大量存在）即可复现。

## 反方审查记录

- 第一轮质疑：
  - 怀疑"也许 Disciple 从设计上本来就不该能交易，`assign_npc_trade_inventory` 里那两行注释只是历史遗留没清理"。经查 `server/src/npc/trade.rs:565-595` 的注释与实现同时存在——文档明确写"`Disciple`: 2-4 件，按 realm 解锁"，并与紧随其后的"非交易 archetype"注释块（`GuardianRelic`/`Daoxiang`/`Zhinian`/`Beast`/`SkullFiend`/`Fuya`/`Zombie`/`DyingElder`/`Mundane`）在同一处 `match` 里显式对立分类，不是孤立注释，是当前代码有意区分的两类。
  - 怀疑"也许 Disciple NPC 只在 dev-only 命令下才会生成，不算正常游玩可达"。经查 `server/src/npc/faction.rs:1042`（世界初始化生成宗门首领）与 `server/src/npc/hydrate/mod.rs:751`（服务器重启后重新 hydrate 持久化 NPC）都是生产路径的真实调用点，与 `command_executor.rs` 里的 dev-only 调用点并列但互不依赖——Disciple 在完全不使用任何 `/give`/`/technique` 之类 dev 命令的情况下就会出现在世界里。
  - 怀疑"也许客户端根本不会对 Disciple 展示交易选项，所以玩家永远走不到发请求这一步"。经查客户端 `NpcMetadata.tradeCandidate()`（`NpcMetadata.java:150-156`）已经明确重构为"数据驱动"（非敌对 + `trade_offers` 非空），注释直接写"Replaces the old hardcoded archetype check"——不存在任何 archetype 层面的客户端拦截，服务端广播出真实 offers 就必然展示。
- 第二轮补证：
  - 补充服务端购买链的完整代码路径，发现拒绝点其实有**两处**而非一处：`npc_trade_catalog_entry()`（`client_request_handler.rs:14482-14508`）先于 `can_trade()`（同文件 :14396-14400）被调用，且前者同样只覆盖 `Commoner | Rogue`。这意味着最初的 fix 建议（只改 `can_trade()`）并不完整——若只加 Disciple 到 `can_trade()` 白名单，请求仍会先在 `npc_trade_catalog_entry` 处以"没有这件货"失败，症状变了但 bug 没修好。
  - 核查客户端真实发送的 `requested_item_id`：`NpcTradeScreen.java:203-204` 确认发送的是 `offer.templateId()`，即服务端自己广播出去的 canonical `template_id`，不是历史遗留的别名字符串（如 `lingcao`）——为"改成直接对照活体 `NpcTradeInventory.offers` 做数据驱动匹配"这一修复方向提供了直接依据，而不只是主观偏好。
  - 查重：`git log`/`gh pr list` 历史上 PR #1164（`plan-npc-trade-bundle-count-loss-v1`）动过同一批文件/同一区域，但修的是完全不同的失败模式（Rogue/Commoner 静态 catalogue 与活体报价数量不一致导致的"礼包数量少发"），未触碰 archetype 白名单或 `npc_trade_catalog_entry` 的 Disciple 缺口，两者不冲突不重复。
  - 查重：`docs/plans-skeleton/plan-npc-trade-gate-desync-v1.md` 也涉及 `can_trade()`，但根因是"Wanted 声望档位对客户端不可见导致的 UI/服务端展示分叉"（Rogue/Commoner 场景下，玩家因该 zone 声望被通缉时 UI 仍误显示可交易），与本 bug（Disciple 整个 archetype 被排除在白名单外）是同一函数上的两个独立缺口，非重复——但两个修复若并行推进需注意谁先合并、后合并者需在最新 `can_trade()` 基础上补自己的判定分支，避免互相覆盖。
  - 查重：`docs/plans-skeleton/plan-bughunt-player-trade-npc-gate-v1.md` 处理的是完全不同的模块（`server/src/social/mod.rs` 的玩家对玩家 `TradeOfferRequest` 误套 `npc_should_decline_trade()`），不是 NPC 购买门禁，不重复。
  - 让步：当前没有任何测试把"Disciple 应该可交易"钉成预期行为——这是一个纯静态代码复现（match 分支枚举缺失），没有运行时日志或线上反馈佐证，但代码证据本身是决定性的（`matches!` 穷尽性 + 独立的 catalog 静态表都能直接读出缺口）。
  - 终裁：通过。这是具体的、可验证的 archetype 白名单不同步（两处独立硬编码 + 一处已经数据驱动的广播/客户端），不是设计意图，且完全在正常游玩路径内 100% 可达。
- 主循环复核：已亲读关键行确认（`client_request_handler.rs:1409-1423,1424-1436,1452-1469,14396-14400,14482-14508`；`npc/trade.rs:563-595`；`npc/spawn/disciple.rs:95-167`；`npc/faction.rs:1030-1049`；`npc/hydrate/mod.rs:740-759`；`npc_metadata.rs:300-330`；`NpcMetadata.java:140-164`；`NpcDialogueScreen.java:90-114`；`NpcTradeScreen.java:203-204`），并核对 HEAD 与 findings 记录的 `b398c4071042` 一致。

## Skeleton Fix Plan

- [ ] **统一真相源（推荐主方向，数据驱动）**：把 `NpcEngagementTarget::can_trade()` 从"正向 archetype 白名单"改为查询该 NPC 实体是否挂着**非空** `NpcTradeInventory`（与 `assign_npc_trade_inventory` 是否生成 offers 保持单一真相源，与客户端 `tradeCandidate()` 的"数据驱动"语义对齐）。因为 `can_trade()` 目前只持有 `archetype`/`reputation_to_player`/`faction_reputation_tier` 等字段、没有直接访问 ECS Query 的能力，需要在 `resolve_npc_engagement_target()` 里新增一个字段（如 `has_tradeable_offers: bool`），在解析阶段用已有的 `trade_inventories` Query 判定并写入 `NpcEngagementTarget`。
- [ ] **同步修复 `npc_trade_catalog_entry()`**：这是与 `can_trade()` 平行的第二处独立硬编码白名单（同样只覆盖 `Commoner | Rogue`），且在调用顺序上先于 `can_trade()`。最小改法是补一个 `NpcArchetype::Disciple` 分支覆盖 Disciple 会卖的品类（对照 `TRADE_CATALOGUE` 里 `realm_min` 允许 Disciple realm 解锁到的条目）；更彻底的改法是让这一步直接对照传入的活体 `trade_inventory.offers` 做 `template_id` 精确匹配（该函数后面第 1452-1469 行本来就会再做一次一模一样的活体查找），把两套报价来源合并成一套，只保留一个不依赖 archetype 的输入归一化/别名表（`lingcao→spirit_grass` 等）。**两处任一漏改都只是把当前 bug 换个失败文案复现**，必须同 PR 一起改。
- [ ] 若采用"合并成活体查找"的彻底方案，需要同步改造 `client_request_handler.rs:18991-19100` 附近 `npc_trade_catalog_entry` 现有的 pin 测试矩阵（当前直接调用该函数并断言 `(archetype, item_id) -> Option<(template_id, price)>`）——保留"给定 archetype+id 能否解析出正确条目"这份契约的测试意图，但改写成通过真实 `NpcTradeInventory`/集成测试驱动，不能留下签名对不上的死测试。
- [ ] **server gate 是最终权威，client 隐藏只作 UX**：本次修复的核心是让服务端门禁覆盖到 Disciple 这个真实可交易的 archetype，而不是反过来靠客户端过滤——不允许因为"client 已经数据驱动了所以服务端可以继续硬编码"这种思路继续留白名单漂移的空间。修复后即使客户端因版本不同步仍误发 Disciple 购买请求，服务端也必须能按真实 `NpcTradeInventory` 正确放行或拒绝，不依赖客户端自律。
- [ ] 为 9 个"非交易 archetype"（`GuardianRelic`/`Daoxiang`/`Zhinian`/`Beast`/`SkullFiend`/`Fuya`/`Zombie`/`DyingElder`/`Mundane`）补充/保留回归：这些 archetype 的 `assign_npc_trade_inventory` 恒返回空 `offers`，修复后必须**继续**保持购买请求必然失败（`can_trade()` 为 false 或 offers 查找为空），不能因为改成"数据驱动"而误伤这条既有边界。
- [ ] 扩展现有测试 helper `run_npc_trade_request_with_context`（`client_request_handler.rs:6108-6180` 附近，当前硬编码 spawn `NpcArchetype::Commoner`）支持传入任意 `NpcArchetype` 参数，避免为 Disciple 场景另起一套平行 harness。
- [ ] 确认修复不涉及真元/灵气守恒——购买链路只操作 `PlayerInventory.bone_coins`（骨币）与物品实例，不接触 `qi_physics::ledger` / `zone.spirit_qi`，无需额外守恒改造；但要保留现有"先 `add_item_to_player_inventory` 成功才 `bone_coins.saturating_sub(price)`"的原子顺序（`client_request_handler.rs:1582-1597`），修复门禁逻辑时不得打乱这个顺序（不能吞骨币不给货，也不能货给了两次）。
- [ ] 与 `docs/plans-skeleton/plan-npc-trade-gate-desync-v1.md`（同一 `can_trade()` 函数上的 Wanted 声望档位 UI/服务端分叉）协调实施顺序：若两个修复并行推进，后合并的一方需要在最新 `can_trade()`/`resolve_npc_engagement_target()` 版本上补自己的判定分支，不得互相覆盖对方新增的字段或分支。

## 验收测试计划

全部在 `server/` 用 `cargo test`，复用/扩展 `client_request_handler.rs` 内既有的 `CustomPayloadEvent` → ECS App 集成测试范式（对齐 `npc_trade_request_rejects_wanted_player_through_engagement_wiring` 的写法，走真实 wire 而非绕过 handler 直接调用内部函数）：

- **happy path**：构造 `NpcArchetype::Disciple` NPC + 非空 `NpcTradeInventory`（含至少一条真实 offer）+ 玩家骨币充足 + 非 Wanted 声望，发送 `NpcTradeRequest { requested_item_id: <该 offer 的 template_id> }`，断言：① 玩家 `PlayerInventory` 收到对应 `template_id` 的物品且数量等于 `offer.count`；② `bone_coins` 精确减少 `price`（按 `TradeEligibility` 定价规则折算后的最终值，不是 catalogue 原价）；③ `PlayerInventory.revision` 自增；④ 收到的聊天反馈包含"你用 {price} 枚骨币从 {name} 手中买下"，不包含"不做买卖"/"没有这件货"。
- **边界 —— realm 分档**：对 Disciple 在多个 `Realm` 档位（醒灵/引气/凝脉/固元/通灵/化虚，覆盖 `realm_rank` 的边界值）分别生成 `NpcTradeInventory`，断言每一档生成的 offers 都能被对应 realm 的购买请求成功解析（不因为 `effective_rank` 计算错位导致某些档位仍然 404）。
- **边界 —— 空库存**：强制构造一个 `NpcArchetype::Disciple` 但 `NpcTradeInventory.offers` 为空的实体（模拟"刚 spawn 还没来得及生成报价"或未来数据异常），断言购买请求仍被拒绝（"当前没有可成交的货物"/"没有这件货"），不能因为改成数据驱动就对空库存报"必然成功"的假阳性。
- **错误分支 —— 非交易 archetype 未被误放行**：对 9 个非交易 archetype 逐一构造实体（`assign_npc_trade_inventory` 对它们返回空 `offers`），逐一发送购买请求，断言全部仍然失败——把"新旧两套白名单必须保持同步"这件事做成单条参数化测试矩阵（对每个 `NpcArchetype` 变体各一条专属 case），任何一个变体的可交易性反转都会撞红。
- **错误分支 —— Wanted 声望不因本次修复被绕过**：复用/参考现有 `npc_trade_request_rejects_wanted_player_through_engagement_wiring` 的构造方式，对 `NpcArchetype::Disciple` + `FactionReputationTier::Wanted` 的组合发送购买请求，断言仍然被拒绝并回"不做买卖"——确认本次修复只补齐了 archetype 维度，没有削弱既有的声望门禁维度。
- **状态转换 —— 两处门禁一致性**：新增一条测试直接断言"对 `assign_npc_trade_inventory` 判定为非空 offers 的每个 archetype（`Commoner`/`Rogue`/`Disciple`），`can_trade()` 必须返回 `true`；对判定为空 offers 的每个 archetype，`can_trade()` 必须返回 `false`"，把两个函数的耦合关系锁进测试而不是靠人工同步记忆。
- **回归 —— PR #1164 行为不倒退**：跑一遍 `Commoner`/`Rogue` 的既有购买 + 礼包数量测试，确认本次改动（尤其是若合并/精简了 `npc_trade_catalog_entry`）不影响 PR #1164 已修好的"活体报价数量与实际发放数量一致"行为。
- **回归 —— 现有 `npc_trade_catalog_entry` pin 矩阵**：`client_request_handler.rs:18991-19100` 附近现有的正反用例（`ling_xi_wan_flawed`/`ju_ling_dan_flawed`/别名/非交易 archetype 返回 `None` 等）全部保留或等价改写后仍需全绿。

## 风险

- `npc_trade_catalog_entry` 里的别名归一化（`lingcao→spirit_grass`、`ling_xi_wan_次品→ling_xi_wan_flawed` 等）如果在"合并成活体查找"方案里被直接删掉，需要先 grep 确认没有其他调用者（聊天指令、旧客户端兼容路径等）依赖这些别名——当前证据只确认了 `NpcTradeScreen.java` 这一个客户端调用点始终发送 canonical `template_id`。
- `can_trade()` 改成"数据驱动"（查询该实体是否挂非空 `NpcTradeInventory`）需要在 `resolve_npc_engagement_target()` 里新增字段/引用已有的 `trade_inventories` Query，注意不要引入额外的可变借用冲突或超出 Bevy SystemParam 数量上限。
- 两处独立白名单（`can_trade()` 与 `npc_trade_catalog_entry()`）只改一处 = 只是把 bug 的失败文案从"没有这件货"换成"不做买卖"（或反过来），不算修复完成，必须同 PR 一起改并在测试里把两者锁在一起。
- 与 `docs/plans-skeleton/plan-npc-trade-gate-desync-v1.md`（同一个 `can_trade()` 上的 Wanted 声望展示分叉）如果并行修复，需注意 merge 顺序——后合并的分支需要基于最新版本补自己的判定分支，不能互相覆盖对方新加的字段/逻辑。
- 若重构/精简 `npc_trade_catalog_entry` 的函数签名（比如去掉 `archetype` 参数），必须同步改掉 `client_request_handler.rs:18991-19100` 附近直接调用该函数的现有 pin 测试，否则会编译不过或留下语义已经对不上却仍然"通过"的死测试。
- 骨币扣款/物品发放的原子顺序（先加物品成功才扣款）是现有正确行为，修复门禁逻辑时如果重新组织了这段代码路径，必须显式验证没有破坏这个顺序（吞骨币不给货 / 给两次货 都是新的、比原 bug 更严重的回归）。
