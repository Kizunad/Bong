# BugHunt: 灵气查表硬编码主世界导致坍缩渊内数据全面失真

## Bug 摘要

**严重度：high（zone-double-source-1）+ medium（zone-double-source-2），同根因合并**

1. `server/src/network/mod.rs::zone_name_for_position`（L2158-2166）无条件调用 `zone_registry.find_zone(DimensionKind::Overworld, position)`，完全无视实体真实所在维度。凡是身处坍缩渊（Tsy）的玩家，其 HUD `PlayerState` payload、`bong:world_state` 推送给天道 agent 的 `PlayerProfile.zone`、`ZoneInfo` 区域推送（灵气浓度/危险等级/感知文案）全部解析不到真实 zone，落回硬编码 `DEFAULT_SPAWN_ZONE_NAME`（`"spawn"`），并进一步用这个错误 zone 名去查 `find_zone_by_name` 拿到**真实存在的主世界 spawn zone 的 spirit_qi/danger_level/status**——即坍缩渊玩家的 ZoneInfo 会泄漏主世界 spawn 的真实数据，不只是一个通用兜底。
2. `server/src/world/tsy_container_search.rs::apply_search_attrition`（L241-289）同样硬编码 `zones.find_zone(DimensionKind::Overworld, pos.0)` 来定位搜刮者所在 zone。凡是在坍缩渊内搜刮 loot 容器（该系统存在的唯一理由——`plan-tsy-container-v1 §2`），zone 查找必然落空，`apply_attrition_checked` 在 `zone: None` 分支直接返回 `Skipped(MissingZone)`（`qi_physics/attrition.rs:268-273`），item 的 `spirit_quality` 完全不磨损，`ContainerSearch` 磨损机制对坍缩渊静默失效。

两处都是对同一个反模式的重复：`ZoneRegistry::find_zone` 严格按 `zone.dimension == dim` 过滤（`server/src/world/zone.rs:306-318`），而坍缩渊 zone 全部注册为 `dimension: "tsy"`（`server/zones.tsy.json` 12 个 zone 无一例外）。`zone.rs:324-328` 甚至已经在 `find_zone_mut_by_pos` 的文档注释上明写了这个坑：「`dim` 参数指定目标维度；调用方应从 `CurrentDimension` 组件读取实体所在维度后传入，而非硬编码 `DimensionKind::Overworld`（否则 Tsy 维度下 zone 查找永远返回 None）」——本 bug 的两处调用点正是踩进了这条已知警告。仓库里已有正确范本：`cultivation/heal.rs:36-41`（`current_dimension.map(...).unwrap_or(Overworld)` 再传入 `find_zone`）与 `world/tsy_drain.rs:130-133`（`current_dim.map(...).unwrap_or(DimensionKind::Tsy)`），证明这是遗漏而非设计。

## 实际游玩体验影响

- 玩家踏入坍缩渊传送门（非 dev 入口，核心终局玩法）后，客户端 HUD 上显示的区域名和灵气浓度条会长期停留在（或错误解析为）主世界 spawn 的数值，与玩家实际所处的坍缩渊环境完全脱节——玩家看着 HUD 说"我在 spawn"，但脚下踩的是坍缩渊。
- ZoneInfo 推送（进出坍缩渊 zone 边界触发的灵气浓度变化感知文案 `ambient_qi_perception`）会拿主世界 spawn 的灵气值做基准计算，产生错误的感知叙事（例如坍缩渊灵气暴涨/暴跌的错误播报）。
- 天道 agent 通过 `bong:world_state` 拿到的 `PlayerProfile.zone` 对坍缩渊玩家永远是错的（不是 fallback 空值，是可能命中真实 spawn zone 名），agent 的三 Agent 推演会认为这些玩家仍在主世界 spawn 附近，而非身处坍缩渊——影响天道对坍缩渊内玩家行为的感知与叙事介入。
- 玩家在坍缩渊内搜刮任何战利品容器时，`ContainerSearch` 磨损机制（本该让搜刮出的战利品 `spirit_quality` 按约 5% 比例衰减）完全不生效——搜出来的物品品质凭空"免疫"了这条本应适用的磨损规则，破坏了"坍缩渊搜刮有代价"的设计意图，且这个失效对玩家完全不可见（没有任何报错或提示，纯粹的静默机制关闭）。

## 证据定位

- `server/src/network/mod.rs:2158-2166`：`zone_name_for_position` 无条件 `find_zone(DimensionKind::Overworld, position)`。
- `server/src/network/mod.rs:1473-1489`：`build_world_state_snapshot` 的 `clients` 查询完全没有 `CurrentDimension` 字段。
- `server/src/network/mod.rs:1700-1751`：`collect_player_snapshots` 在 L1751 调用 `zone_name_for_position(zone_registry, position.get())` 构造喂给 `bong:world_state` 的 `PlayerProfile.zone`，同样拿不到维度。
- `server/src/network/mod.rs:2179-2191`：`PlayerStateEmitQueryItem` 已经携带 `Option<&CurrentDimension>`（L2184）。
- `server/src/network/mod.rs:2262-2296`：`send_player_state_payload_to_client` 的形参 `current_dimension`（L2267）在 L2283 被用于 `local_neg_pressure_at`，但 L2280 调用 `zone_name_for_position` 时完全没有传入——同一函数内维度信息被算了两次却只用了一次。
- `server/src/network/mod.rs:2286-2288`：错误 `zone_name` 进一步被 `find_zone_by_name` 用来查 `zone_spirit_qi`，把主世界 spawn 的真实 spirit_qi 灌给坍缩渊玩家的 payload。
- `server/src/network/mod.rs:2343-2350`、`2352-2363`：`ZoneInfoClientItem` 查询无 `CurrentDimension` 字段，`emit_zone_info_on_zone_transition` 在 L2363 同样调用无维度版本的 `zone_name_for_position`。
- `server/src/world/zone.rs:306-318`：`ZoneRegistry::find_zone` 严格按 `zone.dimension == dim` 过滤。
- `server/src/world/zone.rs:324-328`：`find_zone_mut_by_pos` 文档注释已明确警告"否则 Tsy 维度下 zone 查找永远返回 None"——本 bug 正是撞上这条已知坑。
- `server/zones.tsy.json`：12 个 zone 全部 `"dimension": "tsy"`（已用脚本核验）。
- `server/src/world/tsy_portal.rs:328-555`：`CurrentDimension(DimensionKind::Tsy)` 在真实传送门穿越流程中被设置（非 dev 命令）。
- `server/src/world/tsy_container_search.rs:241-258`：`apply_search_attrition` 的 zone 查找硬编码 `zones.find_zone(DimensionKind::Overworld, pos.0)`。
- `server/src/world/tsy_container_search.rs:268-289`：`zone_name` 为 `None` 时走 `else` 分支，调用 `apply_attrition_checked(item, ContainerSearch, None, None, None)`。
- `server/src/qi_physics/attrition.rs:268-273`：`zone: None` 时 `apply_attrition_checked` 立即 `return Skipped(MissingZone)`，`item.spirit_quality` 完全不变。
- `server/src/qi_physics/attrition.rs:285-297`：zone 存在时的正常路径会算 `attrition_abs` 并调用 `release_attrition_to_zone` 把损耗量守恒地归还 zone（即修好这个 bug 后坍缩渊搜刮会走上这条既有的守恒路径，不需要新写归还逻辑）。
- `server/src/world/tsy_container_search.rs:1371-1417`：现有唯一测试 `apply_search_attrition_emits_qi_attrition_vfx_event` 用 `ZoneRegistry::fallback()`（`world/zone.rs:219`，内部 `Zone::spawn()` 固定 `dimension: DimensionKind::Overworld`，`zone.rs:76-95`）且玩家 entity 没有挂 `CurrentDimension` 组件——测试恰好只覆盖了这个硬编码"碰巧正确"的主世界分支，从未覆盖模块自己文档注释里说的坍缩渊场景。
- `server/src/world/mod.rs:184`：`tsy_container_search::register(app)` 在启动时无条件注册进 `Update` schedule，不是 dev 专用系统。
- 正确范本对照：`server/src/cultivation/heal.rs:36-41`、`server/src/world/tsy_drain.rs:130-133` 均从 `CurrentDimension` 读维度后传入 `find_zone`。

## 触发路径

**Path A（network 三处 payload）**：
1. 玩家从坍缩渊传送门进入坍缩渊，`CurrentDimension` 被设为 `DimensionKind::Tsy`（`tsy_portal.rs`）。
2. 每 tick `PlayerState` payload 推送、`bong:world_state` 定期快照、坍缩渊内跨 zone 移动触发的 `ZoneInfo` 推送，三条路径各自独立调用 `zone_name_for_position(zone_registry, position.get())`，都不传维度。
3. `find_zone(Overworld, pos)` 在坍缩渊坐标系下永远查不到坍缩渊 zone（坍缩渊 12 个 zone 全部注册为 `dimension: "tsy"`），回落到 `"spawn"`。
4. `find_zone_by_name("spawn")` 命中真实主世界 spawn zone，把其 `spirit_qi` 灌进坍缩渊玩家的 HUD/ZoneInfo/world_state。

**Path B（tsy_container_search 磨损）**：
1. 玩家在坍缩渊内对战利品容器发起搜刮，`tick_search_progress` 完成后 emit `SearchCompleted`。
2. `apply_search_attrition` 用玩家 `Position`（坍缩渊坐标系）调 `zones.find_zone(DimensionKind::Overworld, pos.0)`，同样查不到坍缩渊 zone，`zone_name = None`。
3. 落入 `else` 分支调用 `apply_attrition_checked(item, ContainerSearch, None, None, None)`。
4. `apply_attrition_checked` 因 `zone: None` 直接 `Skipped(MissingZone)`，item 的 `spirit_quality` 原封不动，磨损机制静默无效。

## 反方审查记录

- 第一轮质疑：
  - 两处是否本就是设计如此（例如坍缩渊本无 zone 概念，只有主世界才需要 zone 感知）？——不成立：`zones.tsy.json` 存在 12 个专门为坍缩渊注册的 `dimension: "tsy"` zone，坍缩渊显然是有 zone 语义的，只是查找时维度参数没传对。
  - 是否已有 server 端保护绕过这条路径（例如坍缩渊玩家走另一套 HUD 更新函数）？——未发现；`send_player_state_payload_to_client` / `collect_player_snapshots` / `emit_zone_info_on_zone_transition` 是所有玩家共用的唯一路径。
  - 是否为已知/已接受的 fallback 行为？——不成立：`zone.rs:324-328` 的文档注释明确把"硬编码 Overworld 导致 Tsy 下 zone 查找永远 None"列为反模式，说明这是团队已知要避免的坑，本 bug 正是没避开。
  - 查找开放 PR / skeleton 覆盖：`plan-world-social-cross-dimension-witness-leak-v1.md` 命中的是 `social/mod.rs` 与 `chat_collector.rs` 里同名但代码不同的 `zone_name_for_position` 辅助函数（聊天/死亡见证与 Feud 定位场景），不涉及 `network/mod.rs` 的 PlayerState/world_state/ZoneInfo 路径，也不涉及 `tsy_container_search.rs`；`plan-bughunt-skull-fiend-drain-zone-shadow.md`、`plan-bughunt-alchemy-takeback-full-inventory-loss-v1.md` 仅在无关 bug 域里提到"不要硬编码 Overworld"作为前瞻提醒，未覆盖本 bug 的具体调用点。当前在跑 PR（#1275/#1261/#1260/#1259/#1254/#1253/#1249）均未触及这些代码路径。
  - 初裁：两处均为独立可复现真 bug，同根因（`find_zone` 维度参数硬编码），合并入一份 plan 处理。
- 第二轮补证：
  - 补充 `zones.tsy.json` 全部 12 zone 维度实测结果、`tsy_portal.rs` 中 `CurrentDimension(Tsy)` 的真实赋值点、`heal.rs`/`tsy_drain.rs` 两个"正确做法"范本、`apply_attrition_checked` 内 zone 存在时的守恒归还路径（`release_attrition_to_zone`）、现有唯一测试恰好只覆盖 Overworld 分支的具体证据（`ZoneRegistry::fallback()` → `Zone::spawn()` 固定 `dimension: Overworld` 且测试玩家无 `CurrentDimension` 组件）。
  - 让步：finding 2（tsy_container_search）本身不是真元守恒律违规——`apply_attrition_checked` 在 `zone: None` 时只是跳过磨损，没有凭空创造或销毁真元；bug 影响的是"磨损机制该生效却没生效"这一玩法完整性问题，severity 定为 medium 而非 critical 合理，不应因守恒关键词强行拔高。
  - 终裁：两个 finding 均通过，合并为同一份 plan（同一根因：调用方硬编码 `DimensionKind::Overworld` 而非从 `CurrentDimension` 读取真实维度）。
- 主循环复核：已亲读关键行确认（`network/mod.rs` L1473-1489/1700-1751/2158-2363、`world/zone.rs` L299-346、`world/tsy_container_search.rs` L241-289/1371-1417、`qi_physics/attrition.rs` L248-298、`world/tsy_portal.rs` CurrentDimension 赋值点、`cultivation/heal.rs` L29-49、`world/tsy_drain.rs` L118-133、`zones.tsy.json` 12 zone 维度字段）。

## Skeleton Fix Plan

### A. `network::zone_name_for_position` 及三处调用点

- [ ] 给 `zone_name_for_position` 增加 `dimension: DimensionKind` 形参：`fn zone_name_for_position(zone_registry: &ZoneRegistry, position: DVec3, dimension: DimensionKind) -> String`，内部改用 `zone_registry.find_zone(dimension, position)`。默认值只在组件缺失时兜底 `DimensionKind::Overworld`（镜像 `heal.rs:37-39` / `identity/gossip.rs::dimension_kind` 的既有写法），调用方一律传实际读到的维度而不是让函数自己默认。
- [ ] `build_world_state_snapshot` 的 `clients` 查询（L1476-1489）新增 `Option<&CurrentDimension>` 字段；`collect_player_snapshots`（L1700 起）在解构里接住这个字段，L1751 调用改为 `zone_name_for_position(zone_registry, position.get(), current_dimension.map(|c| c.0).unwrap_or(DimensionKind::Overworld))`。
- [ ] `send_player_state_payload_to_client`（L2262）已有 `current_dimension: Option<&CurrentDimension>` 形参（L2267），L2280 的调用改为把它解出的 `DimensionKind` 传进去，而不是像 L2283 那样"算出来只给 `local_neg_pressure_at` 用完就扔"。
- [ ] `ZoneInfoClientItem`（L2343-2350）新增 `Option<&'a CurrentDimension>` 字段；`emit_zone_info_on_zone_transition`（L2352 起）解构接住后，L2363 调用同样传入实际维度。
- [ ] 确认 `find_zone_by_name`（L2286-2288、以及 `emit_zone_info_on_zone_transition` 内部 L2370/2397）在拿到正确 `zone_name` 后天然指向坍缩渊 zone——这一步不需要改，因为它按名字查找不按维度，只要 `zone_name` 本身对了下游就对。

### B. `tsy_container_search::apply_search_attrition`

- [ ] `apply_search_attrition` 的 `inventories` 查询增加 `Option<&CurrentDimension>`：`Query<(&mut PlayerInventory, &Position, Option<&CurrentDimension>)>`。
- [ ] L255-258 的 zone 查找改为：`let dim = current_dim.map(|c| c.0).unwrap_or(DimensionKind::Tsy);`（对齐 `tsy_drain.rs:132` 的既有写法——这个模块本身就是为坍缩渊而生，缺失组件时兜底 Tsy 比兜底 Overworld 更贴合模块语义）再 `zones.find_zone(dim, pos.0)`。
- [ ] 确认 `apply_attrition_checked` 拿到 `Some(zone)` 后走的既有 `release_attrition_to_zone` 归还路径（`attrition.rs:285-297`）本身已经是真元守恒实现（`from_id = QiAccountId::container(...)` → 归还进 zone），本次修复**不需要**新写任何真元流动逻辑，只是让这条已存在的守恒路径对坍缩渊 zone 也能被触发。**严禁**为了"绕过 find_zone 麻烦"而在 else 分支手写一条直接扣 `item.spirit_quality` 又不归还 zone 的捷径——那会制造真正的守恒律违规，比当前"跳过磨损"更糟。

### C. 通用约束

- [ ] server gate 是最终权威：本 bug 不涉及 C2S 请求校验，但两处修复都必须让 server 内部的维度判定成为唯一真相源——不允许引入"client 上报维度"这类可被客户端伪造的旁路。
- [ ] 不新增任何 `*_DECAY*`/`*_ATTEN*` 等真元物理常数；`apply_attrition_checked` 内的 `QI_ATTRITION_BASE_RATE` 等既有常数不变。

## 验收测试计划

均在 `server/` 用 `cargo test` 跑，位置分别在 `server/src/network/mod.rs` 与 `server/src/world/tsy_container_search.rs` 的 `#[cfg(test)] mod tests`：

- **network 回归（happy path，覆盖回归）**：构造 Overworld 玩家（`CurrentDimension(Overworld)` 或不挂该组件）在既有 spawn zone 范围内，断言 `zone_name_for_position` 与三处调用点的输出与修复前一致（不破坏已有行为）。
- **network 边界（坍缩渊命中，核心新增）**：注册一个 `dimension: DimensionKind::Tsy` 的测试 zone，玩家挂 `CurrentDimension(Tsy)` 且 `Position` 落在该 zone AABB 内，分别断言：
  - `collect_player_snapshots` 产出的 `PlayerProfile.zone` 等于该坍缩渊 zone 名，不是 `"spawn"`。
  - `send_player_state_payload_to_client` 序列化出的 `PlayerState.zone_spirit_qi` 等于该坍缩渊 zone 的 `spirit_qi`，不是主世界 spawn 的 `DEFAULT_SPAWN_SPIRIT_QI`。
  - `emit_zone_info_on_zone_transition` 推送的 `ZoneInfo.zone` 与 `spirit_qi` 均来自该坍缩渊 zone。
- **network 错误分支**：`CurrentDimension` 组件缺失（`Option::None`）时的默认维度行为——断言仍兜底 `Overworld`，不 panic、不 unwrap 崩溃。
- **network 状态转换**：玩家从 Overworld zone 走到 Tsy zone（跨维度传送）前后各触发一次 `emit_zone_info_on_zone_transition`，断言两次推送的 `zone` 字段分别正确对应各自维度的真实 zone，且 `transitioned` 判定为 true。
- **tsy_container_search happy path（坍缩渊真实场景，核心新增）**：注册一个 `Tsy` 维度测试 zone，玩家挂 `CurrentDimension(Tsy)`，`Position` 落在该 zone 内，发 `SearchCompleted` 携带一件 `spirit_quality > 0` 的战利品，断言事件处理后该 item 的 `spirit_quality` **确实下降**（当前代码这条会失败，证明今天的 Overworld-only 测试没覆盖真实路径），且 zone 的 `spirit_qi` 按 `release_attrition_to_zone` 的既有守恒公式相应增加（断言用 `qi_physics::ledger` 口径，不写字面数值）。
- **tsy_container_search 回归（既有 Overworld 测试不变）**：保留 `apply_search_attrition_emits_qi_attrition_vfx_event`，确认修复后该测试仍然通过（Overworld 分支不受影响）。
- **tsy_container_search 边界**：`CurrentDimension` 组件缺失时的默认维度改为 `Tsy`（而非 Overworld）——补一条测试验证"玩家没挂维度组件、但站在一个 Tsy zone 坐标范围内"时磨损仍能生效（因为默认值现在是 Tsy，贴合模块语义）；再补一条"玩家没挂维度组件、站在 Overworld zone 坐标范围内"验证此时磨损**不**生效（因为默认成了 Tsy，找不到 Overworld zone）——用于明确记录这个默认值切换的行为边界，供 review 时确认这是预期取舍而非新引入的回归。
- **tsy_container_search 错误分支**：zone 查找仍然失败（例如玩家坐标既不在任何 Overworld zone 也不在任何 Tsy zone 内）时，断言走既有 `else` 分支、`item.spirit_quality` 不变、无 panic——保留"真的找不到 zone 时优雅跳过"的既有语义。
- **可选 e2e**：`scripts/smoke-test-e2e.sh` 起服后走真实坍缩渊传送门流程，人工/bot 核验 HUD 灵气条与坍缩渊 zone 一致、搜刮一次容器后战利品 `spirit_quality` 确有下降。

## 风险

- `tsy_container_search` 的 `CurrentDimension` 缺失默认值从"隐式 Overworld"改为"显式 Tsy"（对齐 `tsy_drain.rs` 既有约定）是一个行为切换点：如果生产环境存在没有 `CurrentDimension` 组件却站在 Overworld zone 坐标范围内搜刮的边缘情况（正常流程下不应存在，因为所有玩家 entity 出生即挂该组件），磨损会从"生效"变成"不生效"。修复时需要用测试明确锁定这个边界（见验收测试计划 tsy_container_search 边界项），review 时需要确认这个取舍。
- `zone_name_for_position` 增加维度参数是签名变更，三处调用点的查询都要同步加字段——如果漏改其中一处（例如只改了 `network/mod.rs` 忘了检查是否还有其他文件引用同名函数），会造成"部分修好、部分还是硬编码"的半吊子状态；修复时必须 grep 全仓确认 `zone_name_for_position` 的唯一定义与调用点集合（`social/mod.rs` 与 `chat_collector.rs` 里同名函数是另一个既存 skeleton 的范围，不属于本 plan，不要顺手改）。
- 本 bug 不是真元凭空增减类守恒律违规（两处都是"该做的事没做"而非"多做/少做真元"），但 fix 后 `apply_attrition_checked` 的既有守恒归还路径会在坍缩渊场景首次被真实触发——需要在测试里确认 `release_attrition_to_zone` 对坍缩渊 zone 的写入同样遵守 `qi_physics::ledger` 口径，不要假设"以前没走过这条路所以不用管"。
