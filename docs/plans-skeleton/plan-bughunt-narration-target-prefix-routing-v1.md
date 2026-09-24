# BugHunt: 三条天道 narration runtime 违反 target 路由契约，断脉/变异/真元变色叙事对玩家静默丢失

## Bug 摘要

本 plan 合并同一系统性 bug 的三个独立触发点（`_group: g3-wire-contract`）。

**分支一：断脉叙事（meridian-severed）—— 严重度 high（severity_adjust: unchanged，未调整）**

`agent/packages/tiandao/src/meridian-severed-narration.ts::renderMeridianSeveredNarration` 在已经拿到 server 正确解析出的 `event.entity_id`（形如 `"offline:<username>"` 或 `"char:<bits>"`）之后，又在它前面拼了一个字面量前缀 `"meridian_severed:"` 才写进 `target`。server 端 `normalize_player_target` 用 `target.split('|').next()` 取第一段再剥 `offline:` 前缀做小写比对，`"meridian_severed:offline:Steve"` 的冒号前缀是 `"meridian_severed"` 而不是 `"offline"`，剥壳失败，路由永远零命中，narration 被静默丢弃。

**分支二/三：变异叙事 + 真元变色叙事（mutation / qi-color）—— 严重度 medium（severity_adjust: unchanged，未调整）**

`agent/packages/tiandao/src/mutation-narration-runtime.ts::renderMutationNarration`（heavy 阶段 player scope）和 `agent/packages/tiandao/src/qi-color-narration.ts::renderQiColorNarration`（player scope）犯了同一类错误：分别拼了字面量前缀 `"mutation:"` / `"qi_color:"`，导致 `split('|')[0]` 永远无法被 `strip_offline_alias_prefix` 正确识别、永远不等于任何 recipient 的 `username_key` / `char_id_key`。更严重的是 mutation 的 **bestial 阶段（zone scope）** 现在直接把整条复合字符串 `mutation:<id>|stage:bestial|tick:...` 当 zone 名传给 `RecipientSelector::zone`（`server/src/network/mod.rs:3359-3364` 要求 target 逐字等于真实 zone 名），且更深一层——`MutationEventV1`（`agent/packages/schema/src/mutation-event.ts:22-33`）的 wire schema 里根本没有 `zone_name` 字段，`mutation_event_publish_system`（`server/src/network/mutation_event_publish.rs:41-77`）也从未查询过实体位置/zone，所以 bestial 分支不是"格式拼错"这么简单，而是**没有任何可用的真实 zone 名可以填**，属于本分支里更深的一层缺口。

三处均无集成测试验证 server 端真的路由成功；现有测试只断言渲染出的字符串包含子串（甚至 `qi-color-narration.test.ts:74` 直接把这个错误格式 `"qi_color:offline:Azure|tick:2"` 断言为"期望值"，把 bug 焊死进了测试契约里）。

## 实际游玩体验影响

- **断脉**：玩家经脉被永久 SEVERED（无论是主动截脉换命、反噬爆脉、强行调动撕裂、战斗创伤，还是渡劫失败）——这是修仙生涯里少数几个不可逆的重大惩罚性事件——却收不到天道任何叙事反馈。玩家只能通过经脉状态查询界面发现自己少了一条经脉，体验从"天道降下警示"退化成"系统内部悄悄记了一笔账"。
- **变异（heavy 阶段）**：丹道路线玩家自然服毒积累进入 heavy 变异阶段（真实修炼产出，非 dev 命令），本应收到天道对"经脉走向变得不像人"的私聊警示，实际上一条都收不到。
- **变异（bestial 阶段）**：进入兽化阶段本应向全 zone 广播"天已不认你是人"的天象级警示（其他玩家应该能感知到"这个 zone 里有人正在兽化"），但由于 target 既格式错误、又压根没有 zone 名可用，bestial 阶段的 zone 广播 100% 从未真正发出过。
- **真元变色**：修炼系统的常规产出——真元诸色转变（趋于混元 / 趋于杂乱）——天道叙事同样从未真正送达任何玩家。

四种反馈路径全部悄无声息地失效，且没有任何报错或崩溃，纯粹是"看起来在跑，实际上啥也没发生"——这正是最难被察觉、最容易被误判为"已实装"的一类 bug。

## 证据定位

**agent 侧（bug 源头）**

- `agent/packages/tiandao/src/meridian-severed-narration.ts:78-83`（`renderMeridianSeveredNarration`）：L80 `target: \`meridian_severed:${event.entity_id}|${event.meridian_id}|tick:${event.at_tick}\``，字面量前缀拼在已解析好的 `entity_id` 之前。
- `agent/packages/tiandao/src/mutation-narration-runtime.ts:54-62`（`renderMutationNarration`）：L58 `target: \`mutation:${event.entity_id}|stage:${event.to_stage}|tick:${event.at_tick}\``；heavy 阶段 `scope:"player"`，bestial 阶段 `scope:"zone"`（L57 三元表达式）。
- `agent/packages/tiandao/src/qi-color-narration.ts:43-48`（`renderQiColorNarration`）：L45 `target: \`qi_color:${player.uuid}|tick:${tick}\``。
- `agent/packages/tiandao/src/void_erosion_runtime.ts:45-53`（`renderVoidErosionNarration`）：**这是正确实现的对照组**——L50 `target: \`${event.entity}|void_erosion:advance|from:...|to:...|tick:...\``，entity 严格占据 `split('|')[0]` 且不拼前缀；但 L49 的注释"对齐已工作 runtime（meridian-severed-narration.ts、mutation-narration-runtime.ts）的格式"本身是错的——它对齐的两个"已工作" runtime 恰恰就是本 plan 要修的两个坏样本，说明这个格式错误已经在传播扩散。
- `agent/packages/tiandao/src/elder-encounter-narration.ts:168-169`（`renderAppearedNarration`）与 `agent/packages/tiandao/src/scattered-cultivator-narration.ts:162`：**zone scope 正确写法对照组**——`target: payload.zone_name` / `target: payload.zone`，裸 zone 名，无任何前后缀。
- `agent/packages/schema/src/narration.ts:15`：TypeBox 文档字符串写明 `target` 应为 "zone name or player uuid, required when scope != broadcast"，但只声明为 `Type.Optional(Type.String())`，没有结构化校验强制这一契约（`validateNarrationV1Contract`，L33-53，只校验 schema 形状，不校验路由格式）。
- `agent/packages/schema/src/mutation-event.ts:22-33`：`MutationEventV1` wire schema `additionalProperties:false`，字段只有 `v/entity_id/from_stage/to_stage/cumulative_toxin/at_tick`，**没有 `zone_name`**——bestial 阶段 zone-scope 叙事从数据源头就拿不到真实 zone 名。

**server 侧（路由契约的权威实现，未改错，但没有格式校验去防御 agent 侧误用）**

- `server/src/network/agent_bridge.rs:285-302`（`normalize_player_target`）：`trimmed.split('|').next()` 取第一段，再交给 `strip_offline_alias_prefix`。
- `server/src/network/agent_bridge.rs:391-397`（`strip_offline_alias_prefix`）：`value.split_once(':')`，只有冒号前缀恰为 `"offline"`（大小写不敏感）才剥壳，否则原样返回。
- `server/src/network/agent_bridge.rs:305-335`（`route_recipient_indices`）：`RecipientSelector::Player` 分支对 `username_key`/`char_id_key` 做等值比对。
- `server/src/network/mod.rs:3288-3313`（`process_single_narration`）：`routed_targets.is_empty()` 时只打一条 `tracing::debug!` 就 `return`，无任何用户可见反馈。
- `server/src/network/mod.rs:3354-3372`（`narration_selector`）：`NarrationScope::Zone` 分支把 `narration.target` 整串传给 `RecipientSelector::zone`。
- `server/src/network/mod.rs:3374-3407`（`collect_routed_targets`）：recipient 的 `char_id` 固定构造为 `format!("char:{}", entity.to_bits())`（L3390 附近）。
- `server/src/network/meridian_severed_emit.rs:84-92`（`resolve_entity_id`）：`entity_id` 正确构造为 `"offline:{username}"` 或 `"char:{bits}"`——证明断脉分支的 bug 纯粹出在 narration 层多拼了一层前缀，不是 entity_id 本身的问题。
- `server/src/network/mutation_event_publish.rs:41-77`（`mutation_event_publish_system`）：L72 `entity_id: entity.to_bits()` 是裸 u64，Query 只有 `(Entity, &DandaoStyle)`，**没有 `&Position`，也没有查询 `ZoneRegistry`**——bestial 阶段没有任何渠道把 zone 名塞进 wire payload。
- `server/src/player/state.rs:319-321`（`canonical_player_id`）：`format!("offline:{username}")`——`PlayerProfile.uuid`（`server/src/network/mod.rs:1621`：`uuid: canonical_player_id(name)`）实际上已经是这个格式，**不是裸 UUID**（原始 finding 文本对此有误，此处已核实纠正：`qi-color-narration.ts` 里的 `player.uuid` 是 `"offline:<username>"` 字符串，但因为外面又套了 `"qi_color:"` 前缀，依然会被 `strip_offline_alias_prefix` 判定失败，结论不变）。
- `server/src/network/mod.rs:2158-2165`（`zone_name_for_position`）：现成可复用的 `ZoneRegistry` 位置查名函数，`spirit_treasure_emit.rs:160`、`chat_collector.rs:498` 等多处已在用同名模式——bestial 阶段修复应复用这个函数，而不是发明新逻辑。

**测试侧（现有测试锁死了错误契约，从未验证真实路由）**

- `agent/packages/tiandao/tests/meridian-severed-narration.test.ts:86-94`：只 `toContain("player:abc")` / `toContain("Lung")` / `toContain("555")`，从未验证 server 能否路由命中。
- `agent/packages/tiandao/tests/mutation-narration-runtime.test.ts:98-99`：只 `toContain("mutation:42")` / `toContain("stage:heavy")`。
- `agent/packages/tiandao/tests/qi-color-narration.test.ts:74`：`toMatchObject({ target: "qi_color:offline:Azure|tick:2", ... })`——**把错误格式直接断言成期望值**，回归测试会在修复后主动撞红，必须随修复一起改。
- `server/src/network/mod.rs:4606`（`mod narration_tests`）+ `server/src/network/mod.rs:4867-4934`（`player_scope_matches_username_and_offline_id`）+ `server/src/network/mod.rs:5023-5133`（`realm_gate_producer_consumer_selector_routes_only_target_player`）：server 侧已有完整的 mock-client 路由测试基础设施（`setup_narration_app` / `spawn_test_client_with_helper` / `enqueue_single_narration` / `collect_narration_and_chat_packets`），且已有一个**真跨栈**范本——起 `tsx` 子进程跑生产 TS consumer、把真实输出喂回生产 Rust selector 验证——本 plan 的验收测试应复用这套基础设施，而不是另起炉灶。

## 触发路径

**共同机制**：三处 narration 渲染函数都在"已经拿到一个理应严格等于 recipient key 的标识符"（`entity_id` / `player.uuid`）之后，又在它前面拼了一个用于自我描述事件类型的字面量前缀（`meridian_severed:` / `mutation:` / `qi_color:`），把它塞进了 `target` 的 `split('|')[0]` 位置。server 端契约要求 `target` 的第一个 pipe 段**严格就是** recipient key 本身，辅助信息只能放在后续 pipe 段——三处实现全部违反了这一隐性契约（唯一显式记录该契约的地方是 `void_erosion_runtime.ts:47-49` 的注释，讽刺的是它把两个错误样本当成了参照对象）。

1. **断脉**：玩家永久断脉（5+ 真实触发源之一）→ `publish_meridian_severed_events`（`meridian_severed_emit.rs`）正确解析 `entity_id` 并发布到 `bong:meridian_severed` → `MeridianSeveredNarrationRuntime`（在 `main.ts:208` 正式启动）消费 → `renderMeridianSeveredNarration` 拼出坏 target → 发布到 `bong:agent_narrate` → server `process_agent_narrations_with_dedupe` → `process_single_narration` 路由零命中 → `tracing::debug!` 后静默 return，玩家什么都收不到。
2. **变异 heavy（player scope）**：`mutation_advance_system`（`dandao/mutation.rs`，自然毒素积累的常规系统）推进到 heavy → `mutation_event_publish_system` 发布 `bong:mutation_event` → `MutationNarrationRuntime`（`main.ts:220`/`640-652` 正式启动）消费 → `renderMutationNarration` 拼出坏 target（`mutation:<bits>|stage:heavy|...`）→ 同一条 server 路由链路零命中丢弃。
3. **变异 bestial（zone scope）**：同上直到 `renderMutationNarration`，但走 `scope:"zone"` 分支，`narration_selector` 把整条复合字符串当 zone 名传给 `RecipientSelector::zone`，且因为 `MutationEventV1` 从未携带 zone 名，即使字符串格式修好也无真实 zone 名可填——bestial 阶段目前从数据源头到路由终点全链路不可能命中任何 zone。
4. **真元变色**：`QiColorNarrationTracker.ingest`（在每次 world_state 轮询中被调用，`main.ts` runtime 主循环）检测到任一玩家真元色变化 → `renderQiColorNarration` 拼出坏 target（`qi_color:offline:<username>|tick:...`）→ 同一条 server 路由链路零命中丢弃。

## 反方审查记录

- 第一轮质疑：
  - 是否只是断脉这一处孤立的字符串拼接失误？查证后发现 `mutation-narration-runtime.ts` 和 `qi-color-narration.ts` 各自独立犯了完全同构的错误——不是复制粘贴同一段代码，而是同一份隐性契约理解错误在多处扩散，判定为同一系统性 bug 的三个触发点，合并成一份 plan。
  - `qi-color-narration.ts` 的 `player.uuid` 是否真是裸 UUID（原 finding 文本如此描述）？读 `server/src/player/state.rs:319-321` 与 `server/src/network/mod.rs:1621` 后确认 `PlayerProfile.uuid` 实际上是 `canonical_player_id(name)` 即 `"offline:<username>"` 格式，**不是**裸 UUID——但这个更正不改变结论：因为外层又套了 `"qi_color:"` 前缀，`strip_offline_alias_prefix` 依然识别不出 `"offline"` 前缀，路由依然零命中。此处已在"证据定位"节纠正。
  - 是否已有已知 plan / 开放 PR 覆盖这个契约问题？未在已知 skeleton / active plan / 开放 PR 列表中找到任何以 "narration target" / "target 路由" / "meridian_severed narration" / "mutation narration" 为主题的条目。
  - 初裁：两条 finding 均倾向真 bug，合并处理。
- 第二轮补证：
  - `void_erosion_runtime.ts:47-49` 的注释本身声称"对齐已工作 runtime"，但被对齐的两个 runtime 恰恰是坏的——这是强反证：说明格式错误已经从坏样本"传染"到了新代码的心智模型里，不修复会继续扩散到未来的 narration runtime。
  - `qi-color-narration.test.ts:74` 里现有测试**直接把错误格式断言为期望输出**（`toMatchObject({ target: "qi_color:offline:Azure|tick:2" })`），证明这不是"漏测"，而是"测试本身把 bug 焊成了契约"——修复必须同步改测试，否则回归测试会主动拒绝正确实现。
  - 补查 mutation bestial 分支发现比原 finding 描述更深一层缺口：`MutationEventV1` wire schema（`mutation-event.ts:22-33`）压根没有 `zone_name` 字段，`mutation_event_publish_system` 的 Query 也没有 `&Position`，bestial 阶段的 zone 广播从数据源头就不具备可用的真实 zone 名——这部分需要新增 server 端字段 + 查询，而不仅是 agent 端格式修正。
  - server 侧路由函数虽标 `#[allow(dead_code)]`，但经确认是**生产热路径**（`mod.rs:3402` `collect_routed_targets` 调 `route_recipient_indices`，`process_agent_narrations_with_dedupe` → `process_single_narration` → `narration_selector` → `collect_routed_targets`），标注只是历史遗留，不代表未接线。
  - 终裁：两条 finding 均通过，合并成一份 plan；风险点是 bestial 分支的修复范围比 player-scope 三处更大（需要 server schema 扩展），必须在 Skeleton Fix Plan 里单独列出，不能和另外三处简单同构修复混在一起。

主循环复核：已亲读关键行确认（`meridian-severed-narration.ts:78-83`、`mutation-narration-runtime.ts:54-62`、`qi-color-narration.ts:43-48`、`void_erosion_runtime.ts:45-53`、`elder-encounter-narration.ts:168-169`、`scattered-cultivator-narration.ts:162`、`agent_bridge.rs:285-397`、`mod.rs:3288-3407`、`meridian_severed_emit.rs:84-92`、`mutation_event_publish.rs:41-77`、`mutation-event.ts:22-33`、`state.rs:319-321`、三处 `.test.ts` 断言、`mod.rs:4606-5133` 现有 server 测试基础设施），并对 mutation bestial 分支补充核实了原 finding 未点破的 `zone_name` 字段缺失问题。

## Skeleton Fix Plan

**共享 helper（先做这一步，避免三处各修各的又留出新的格式漂移）**

- [ ] 新建 `agent/packages/tiandao/src/narration-target.ts`，导出：
  - `resolvePlayerNarrationTarget(recipientKey: string, extra?: Record<string, string | number>): string` —— `recipientKey` 必须已经是 `"offline:<username>"` 或 `"char:<bits>"` 格式（不做二次包装/加前缀），`extra` 里的键值对按 `key:value` 拼在后续 pipe 段。
  - `resolveZoneNarrationTarget(zoneName: string): string` —— 直接返回裸 `zoneName`，不接受额外拼接（zone scope target 必须逐字等于真实 zone 名）。
  - 两个函数都在非法输入（空字符串 / 已含 `|` 的 `recipientKey`）时 `throw`，在编译期/单测阶段就拦住误用，而不是放到生产环境静默丢弃。

**分支一：断脉叙事（`meridian-severed-narration.ts`）**

- [ ] `renderMeridianSeveredNarration`（L80）改为 `target: resolvePlayerNarrationTarget(event.entity_id, { meridian: event.meridian_id, tick: event.at_tick })`，产出形如 `${event.entity_id}|meridian:Lung|tick:123`，与 `void_erosion_runtime.ts` 的正确写法对齐（entity_id 严格占 `split('|')[0]`）。
- [ ] 更新 `meridian-severed-narration.test.ts:86-94`：断言改为验证 `target.startsWith(event.entity_id + "|")` 或直接 `toBe` 完整期望字符串，而不只是 `toContain` 子串。

**分支二：变异叙事 heavy 阶段（player scope，`mutation-narration-runtime.ts`）**

- [ ] `renderMutationNarration`（L58）heavy 分支改为 `target: resolvePlayerNarrationTarget(\`char:${event.entity_id}\`, { stage: event.to_stage, tick: event.at_tick })`——`event.entity_id` 是裸 u64（wire schema 未变），必须在 agent 侧包一层 `char:` 前缀才能匹配 server 端 `char_id_key = format!("char:{}", entity.to_bits())`（`server/src/network/mod.rs` `collect_routed_targets` 附近）。
- [ ] 更新 `mutation-narration-runtime.test.ts:98-99`：断言 target 首段严格等于 `char:42`，而不只是 `toContain("mutation:42")`。

**分支三：变异叙事 bestial 阶段（zone scope，需要 server 端 schema 扩展——范围比前两处大）**

- [ ] `agent/packages/schema/src/mutation-event.ts` 的 `MutationEventV1` 新增字段 `zone_name: Type.String()`（`additionalProperties:false` 下必须显式加字段，不能塞进 target）。
- [ ] `cd agent && npm run build -w @bong/schema` 重建 dist（否则 agent 侧 import 崩 `SyntaxError`，见 CLAUDE.md 已知坑）。
- [ ] `server/src/network/mutation_event_publish.rs::mutation_event_publish_system` 的 `dandao_query` 增加 `&Position`，新增 `Option<Res<ZoneRegistry>>` 参数，用现成的 `zone_name_for_position`（`server/src/network/mod.rs:2158-2165`，或提取成 pub(crate) 共享函数供两处调用）解析 entity 当前 zone，写入 `MutationEventV1.zone_name`。
- [ ] `renderMutationNarration`（L57/58）bestial 分支改为 `scope: "zone"`, `target: resolveZoneNarrationTarget(event.zone_name)`。
- [ ] 补 `mutation_event_publish_system` 单测：断言 bestial 事件携带的 `zone_name` 与实体实际所在 zone 一致。

**分支四：真元变色叙事（`qi-color-narration.ts`）**

- [ ] `renderQiColorNarration`（L45）改为 `target: resolvePlayerNarrationTarget(player.uuid, { tick })`——`player.uuid` 已经是 `canonical_player_id` 产出的 `"offline:<username>"`，不需要再包一层前缀，去掉字面量 `"qi_color:"` 即可。
- [ ] 更新 `qi-color-narration.test.ts:74`：把断言里的期望值从 `"qi_color:offline:Azure|tick:2"` 改成 `"offline:Azure|tick:2"`（**这一行必须随修复一起改，否则回归测试会主动拒绝正确修复**）。

**跨仓库契约测试（新增，三处都要有）**

- [ ] agent 侧新增 `narration-target.test.ts`：对 `resolvePlayerNarrationTarget` / `resolveZoneNarrationTarget` 做 happy path + 边界（空 key / 已含 pipe 的非法 key 抛错）+ 错误分支覆盖。
- [ ] server 侧在 `server/src/network/mod.rs` 的 `mod narration_tests`（紧邻 `player_scope_matches_username_and_offline_id`、`realm_gate_producer_consumer_selector_routes_only_target_player`）新增：
  - 一条真跨栈集成测试，起 `tsx` 子进程实跑生产 `renderMeridianSeveredNarration` / `renderMutationNarration` / `renderQiColorNarration`（可仿 `consume_agent_ui_response_through_tiandao` 的模式写一个 narration runner 脚本），把真实渲染出的 `Narration` 喂进 `enqueue_single_narration` → `process_single_narration`，断言目标玩家（或 zone）**确实收到** payload，非目标玩家收不到——不能只测 TS 渲染函数的字符串内容。
- [ ] （不涉及真元/灵气流动——narration 是纯反馈层，不接触 `qi_physics::ledger`，无需守恒改造。）
- [ ] server gate（`normalize_player_target` / `narration_selector` / `route_recipient_indices`）本身不改——它是路由的最终权威，本次修复的落点全部在 agent 侧渲染函数 + 一处 server schema/query 扩展（bestial zone_name），不新增 client 隐藏逻辑（这条 bug 与 C2S 无关，纯 server→agent→server→client narration 反馈链路）。

## 验收测试计划

**agent（vitest，`cd agent/packages/tiandao && npm test`）**

- `renderMeridianSeveredNarration`：happy path 断言 `target === \`${entity_id}|meridian:${meridian_id}|tick:${at_tick}\`\`（entity_id 覆盖 `offline:` 和 `char:` 两种格式两条 case）；边界：`meridian_id`/`at_tick` 为 0 或极端值不破坏拼接。
- `renderMutationNarration`：heavy 阶段断言 `target === \`char:${entity_id}|stage:heavy|tick:${at_tick}\`\`；bestial 阶段断言 `target === zone_name`（逐字相等，不含任何前后缀）且 `scope === "zone"`；状态转换覆盖 `to_stage` 全部 5 个变体各一条专属 case（none/subtle/visible/heavy/bestial），确认只有 heavy/bestial 触发 narration（`shouldNarrate`）。
- `renderQiColorNarration`：断言 `target === \`${player.uuid}|tick:${tick}\`\`；边界覆盖 `player.uuid` 为空字符串时的错误分支（`resolvePlayerNarrationTarget` 应 throw，函数应返回 null 或向上抛错，不能生产出畸形 target）。
- `narration-target.ts` 新 helper 专属测试：`resolvePlayerNarrationTarget` 4 组 case（正常 offline key / 正常 char key / 空字符串抛错 / 已含 `|` 的 key 抛错）；`resolveZoneNarrationTarget` 3 组 case（正常 zone 名 / 空字符串抛错 / 含 `|` 的字符串抛错）。

**server（cargo test，`cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`）**

- `mutation_event_publish_system` 新增单测：实体位于已注册 zone 内 → 断言 `MutationEventV1.zone_name` 等于该 zone 名；实体位于任何 zone 边界外 → 断言 `zone_name` 回退到 `DEFAULT_SPAWN_ZONE_NAME`（对齐 `zone_name_for_position` 现有 fallback 行为）。
- `mod narration_tests` 新增跨栈集成测试（复用 `setup_narration_app` / `spawn_test_client_with_helper` / `enqueue_single_narration` / `collect_narration_and_chat_packets`，参照 `realm_gate_producer_consumer_selector_routes_only_target_player` 的 tsx 子进程模式）：
  - happy path：目标玩家收到 payload，非目标玩家（同 zone 不同 username）收不到，逐条覆盖断脉/变异 heavy/真元变色三个 player-scope narration；变异 bestial 覆盖 zone-scope（目标 zone 内玩家收到、目标 zone 外玩家收不到）。
  - 边界：`entity_id`/`player.uuid` 恰好是已存在 recipient 但大小写不同（如 `"OFFLINE:STEVE"` vs 实际 username `"Steve"`）——沿用 `strip_offline_alias_prefix` 的大小写不敏感行为，断言仍能命中。
  - 错误分支：目标不存在的 recipient（entity_id 指向已下线/已重命名的角色）——断言 `routed_targets` 为空、无 payload 发出、无 panic（对齐 `process_single_narration` 现有静默丢弃行为，但要确认新格式下这仍是"预期的找不到人"而不是"格式错误导致的找不到人"）。
- 回归断言：现有 `player_scope_matches_username_and_offline_id` 保持绿（本次修复不改变 server 端路由逻辑本身）。

**worldgen / client**：本 bug 不涉及地形或客户端渲染，无需 `raster_check` / JUnit。

## 风险

- **测试焊死错误契约**：三处 `.test.ts`（`meridian-severed-narration.test.ts`、`mutation-narration-runtime.test.ts`、`qi-color-narration.test.ts`）现有断言直接编码了错误的 target 字符串，修复实现却不同步改测试会导致这几条测试主动撞红——必须把"改实现"和"改测试期望值"当作同一个原子 commit 的两半，不能只改一边。
- **mutation bestial 分支范围超出 agent 单侧修复**：这一支必须新增 `MutationEventV1.zone_name` 字段并重建 `@bong/schema` dist，是本 plan 里唯一需要跨 server/schema/agent 三端联动的部分；如果只在 agent 侧改格式而不补 zone_name 字段，bestial 广播依然 100% 打不中任何 zone（因为压根没有真实 zone 名可用），必须完整走完 server 端两步才算收口。
- **`char:` 前缀的大小写/格式假设**：mutation heavy 分支修复依赖 `server/src/network/mod.rs` `collect_routed_targets` 里 `char_id` 固定构造为 `format!("char:{}", entity.to_bits())` 这个具体格式——如果未来该格式变化（比如加了校验和/版本号前缀），agent 侧的 `char:${event.entity_id}` 硬编码前缀也要同步跟着改，建议在 server 侧把这个格式常量化（如 `pub const CHAR_ID_PREFIX: &str = "char:"`）供两端引用，避免又是各写一份字符串字面量。
- **不要顺手扩大成"通用 narration target 校验器"大改**：`validateNarrationV1Contract`（`narration.ts:33-53`）目前只校验 schema 形状；本 plan 的 helper（`resolvePlayerNarrationTarget`/`resolveZoneNarrationTarget`）只覆盖新写代码路径，**不强制retrofit** 仓库里所有已存在的 narration 渲染函数去调用它——`elder-encounter-narration.ts`/`scattered-cultivator-narration.ts` 已经手写正确，不需要因为本 plan 而重构；后续新 narration runtime 才应默认使用这两个 helper。
