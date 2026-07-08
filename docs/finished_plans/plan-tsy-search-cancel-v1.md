# plan-tsy-search-cancel-v1

> 一句话主题：TSY 容器搜刮的**主动取消链路断路**——server 端 `cancel_search` 协议、`handle_cancel_search` system、HUD `CANCELLED` 分支全部已落地且端到端接通，但 client 端**没有任何按键会调用 `ClientRequestSender.sendCancelSearch()`**，玩家一旦开始搜刮就只能被动等 server 用移动/受击/进战斗打断，无法主动放弃——这与 worldview §十六.三「守在旁边的修士快逼近了，要不要放弃半搜完的石匣」这一核心决策题设计意图直接冲突：玩家想主动放弃时，没有输入路径能表达这个决策。

## 阶段总览

| 阶段 | 主题 | 状态 | 验收日期 |
|------|------|------|----------|
| P0 | client 新增 `tsy_search_cancel` 按键 → 发 `cancel_search`；server 侧 `handle_cancel_search` 补回归测试锁行为 | ✅ 2026-07-06 | 2026-07-06 |

## 接入面

- **进料**：
  - 读 `com.bong.client.hud.SearchHudStateStore.snapshot().phase()`（client 本地状态，判断"当前是否在 SEARCHING"，决定按键是否应该发请求）。
  - 读 `com.bong.client.network.ClientRequestProtocol.encodeCancelSearch()` / `ClientRequestSender.sendCancelSearch()`（**均已存在，P0 不新增协议**，只新增调用点）。
- **出料**：
  - client → server：`cancel_search` envelope → `server/src/schema/client_request.rs::ClientRequestV1::CancelSearch` → `server/src/network/client_request_handler.rs:2208-2220` → emit `CancelSearchRequestEvent`（即 `crate::world::tsy_container_search::CancelSearchRequest`）。
  - server → client：`handle_cancel_search`（`server/src/world/tsy_container_search.rs:636-660`）移除 `SearchProgress` / `IsSearching`、释放 `LootContainer.searched_by`，emit `SearchAborted { reason: Cancelled }` → 下游 `tsy_container_search_emit`（已有，P0 不改）推 `SearchAbortedV1{reason:"cancelled"}` → client `SearchHudStateStore.markAborted("cancelled")`（已有）→ HUD 回 IDLE。
- **共享类型 / event**：复用 `CancelSearchRequest` event、`SearchAbortReason::Cancelled`、`SearchHudState.AbortReason.CANCELLED`、`ClientRequestV1::CancelSearch` —— **全部已存在，本 plan 不新建任何 component / event / schema**，纯粹是"接一根线"。
- **跨仓库契约**：无新增 symbol。`agent/packages/schema/src/container-interaction.ts::CancelSearchRequestV1` 已在 `plan-input-binding-v1` 落地，本 plan 不动 schema 包。
- **worldview 锚点**：`docs/worldview.md` §十六.三「容器与搜刮」（约 line 1503）——"搜刮越久、档次越高的容器，暴露时间越长……守在旁边的修士快逼近了，要不要放弃半搜完的石匣"，这段决策题的前提是玩家**能**主动放弃；当前断路使这段设计文字名不副实。

## P0 — client 补 `cancel_search` 输入路径 + server 回归测试

### 交付物

1. **新增 client 按键类** `client/src/main/java/com/bong/client/tsy/SearchCancelInteractionBootstrap.java`（新文件，镜像 `client/src/main/java/com/bong/client/tsy/ExtractInteractionBootstrap.java` 的既有模式——**不复用统一 G 交互路由 `InteractKeyRouter`**，理由见 §8.1 #1）：
   - `KeyBinding`：翻译键 `key.bong-client.tsy_search_cancel`，默认键位 `GLFW.GLFW_KEY_H`（`G`=统一交互键启动搜刮，`H` 与之相邻，镜像 `ExtractInteractionBootstrap` 里 `Y`(启动)/`U`(取消) 相邻键的既有惯例），category 沿用 `"category.bong-client.controls"`。
   - `onTick`：`while (cancelKey.wasPressed() && SearchHudStateStore.snapshot().phase() == SearchHudState.Phase.SEARCHING) { ClientRequestSender.sendCancelSearch(); }`。
   - `register()` 挂 `ClientTickEvents.END_CLIENT_TICK`。
2. **`client/src/main/java/com/bong/client/BongClient.java`**：在 `ExtractInteractionBootstrap.register();`（当前 line 140）旁追加 `SearchCancelInteractionBootstrap.register();` + 对应 import。
3. **lang 文件**新增翻译条目（en_us 为强制，zh_cn 参照已有 `key.bong-client.*` 条目风格一并补齐，弥补 `tsy_extract`/`tsy_extract_cancel` 当年漏掉 zh_cn 的债）：
   - `client/src/main/resources/assets/bong-client/lang/en_us.json`：`"key.bong-client.tsy_search_cancel": "Cancel TSY Search"`（紧邻现有 `tsy_extract_cancel` line 22 之后）。
   - `client/src/main/resources/assets/bong-client/lang/zh_cn.json`：`"key.bong-client.tsy_search_cancel": "取消搜刮"`。
4. **server 回归测试**（新增，`server/src/world/tsy_container_search.rs` 现有 `mod tests`，仿照 `tick_search_progress_consumes_key_before_placing_loot_into_freed_slot`（line 999 起）的 `ScenarioSingleClient` 搭建套路）：
   - `handle_cancel_search_removes_progress_and_releases_lock`：玩家挂 `SearchProgress` + 容器 `searched_by = Some(player)`，发一条 `CancelSearchRequest { player }`，跑 `handle_cancel_search`，断言：① `SearchProgress` / `IsSearching` 从玩家实体移除 ② 容器 `searched_by == None` ③ `SearchAborted` 事件恰好 1 条且 `reason == SearchAbortReason::Cancelled`、`player`/`container` 字段匹配。
   - `handle_cancel_search_is_noop_without_search_progress`：玩家没有 `SearchProgress` 时发 `CancelSearchRequest`，断言 system 不 panic、`SearchAborted` 事件 0 条（覆盖幂等/误按分支）。
   - `handle_cancel_search_leaves_other_players_container_lock_untouched`：容器 `searched_by = Some(other_player)`，`player` 自己没有 `SearchProgress` 就发 cancel（例如客户端竞态下重复点击），断言 `other_player` 的锁不受影响（覆盖"取消别人的搜刮"边界）。
5. **client 回归测试**（新增，`client/src/test/java/com/bong/client/hud/SearchHudStateStoreTest.java`，当前**全仓无此文件**，是纯断路——顺手补齐）：
   - `markAborted_cancelled_setsCancelledReasonAndAbortedFlashPhase`：断言 `phase() == ABORTED_FLASH` 且 `abortReason() == CANCELLED`。
   - 覆盖 `markAborted` 其余三个 reason 分支（`moved` / `combat` / `damaged`）+ 未知字符串 → `NONE`，做 enum 全分支 pin（§ CLAUDE.md 饱和测试要求）。

### 验收标准

- 玩家进入 SEARCHING 状态后按 `H`：`ClientRequestSender.sendCancelSearch()` 必须被调用（由 `SearchHudStateStore` 状态门控，非 SEARCHING 时不发）。
- server 收到 `cancel_search` 后：`SearchProgress`/`IsSearching` 移除、容器锁释放、`SearchAborted{Cancelled}` 送达 → client HUD 经 `SearchAbortedV1` 回 IDLE（该段链路已有代码 + 新增 3 条 server 测试锁定）。
- `cargo test -p bong-server tsy_container_search` 新增 3 测试全绿；`./gradlew test` 新增 `SearchHudStateStoreTest` 全绿。

## §8 开放问题（P0 决策门前需收口，原表保留供追溯）

1. 主动取消的输入应该怎么落：严格按文档接 `ESC`/动作键，还是允许再次按 G 作为显式 cancel fallback。
2. 搜刮进行中若玩家尝试撤离/其它 busy 行为，是否仍保持当前 `AlreadyBusy` 语义，还是允许 client 先自动发 `cancel_search` 再转发下一动作。
3. 是否补一条 client 回归测试，锁住"进入 SEARCHING 后按取消键必须发 `cancel_search`，并在收到 `search_aborted(cancelled)` 后清 HUD"。

**全部已在 §8.1 收口。原表保留以备追溯，实施时以 §8.1 决议为准。**

## §8.1 决议（pre-P0 收口，2026-07-04）

### #1 主动取消的输入应该怎么落

**决议**：
1. 不接字面 `ESC` 键，也不复用统一交互键 `G` 做"再按一次取消"。新增一个**独立于 `InteractKeyRouter` 之外的专属按键类**，镜像 `ExtractInteractionBootstrap` 现有模式（该类的 cancel 键实际绑定的是 `GLFW_KEY_U`，不是字面 ESC——`docs/finished_plans/plan-tsy-extract-v1.md:556` 写的"按 ESC"与代码实现之间本来就有文档漂移，本 plan 不重复这个漂移，直接以代码惯例为准）。
2. 默认键位定 `GLFW_KEY_H`（全仓 `GLFW_KEY_*` 使用扫描确认未被占用；与启动搜刮的统一交互键 `G` 相邻，呼应 `ExtractInteractionBootstrap` 里 `Y`(启动撤离)/`U`(取消撤离) 的相邻键惯例）。
3. 拒绝"复用 G 做 toggle"路线的理由（代码验证过，非拍脑袋）：`TsyContainerSearchIntentHandler.candidate()`（`client/src/main/java/com/bong/client/tsy/TsyContainerSearchIntentHandler.java:17-27`）依赖 `TsyContainerStateStore.nearestInteractable()`，其过滤条件 `TsyContainerView.interactable()`（`client/src/main/java/com/bong/client/tsy/TsyContainerView.java:14-16`）在 `searchedByPlayerId` 非空时返回 `false`——玩家开始搜刮自己那一刻，server_data 会把该容器的 `searched_by` 置为自己，`interactable()` 随即变 `false`，`candidate()` 不会再命中该容器。也就是说 SEARCHING 状态下再按 `G`，`InteractKeyRouter` 根本收不到这个容器的候选，无法在不改路由候选逻辑的前提下做"再按 G 取消"——要做就得给 router 引入"自己正在搜刮"的特例分支，侵入现有优先级系统，得不偿失。专属按键（绕过 router，直接查 `SearchHudStateStore` 状态）零侵入、与 `ExtractInteractionBootstrap` 完全对称。

**落点**：新文件 `client/src/main/java/com/bong/client/tsy/SearchCancelInteractionBootstrap.java`（仿 `client/src/main/java/com/bong/client/tsy/ExtractInteractionBootstrap.java:1-42` 结构）/ `client/src/main/java/com/bong/client/BongClient.java:140`（`ExtractInteractionBootstrap.register();` 旁追加）/ plan §P0 交付物 #1-#3。

### #2 搜刮中主动撤离时的 `AlreadyBusy` 语义是否维持

**决议**：
1. **维持现状**：不做"client 先自动发 cancel_search 再转发下一动作"的静默自动取消。`docs/finished_plans/plan-tsy-extract-v1.md:582-583` 已经把这条设计原则钉死——"玩家必须先 ESC 取消搜刮，再启动撤离……这让玩家明确决策，避免'一键撤退'省略搜刮放弃的显式行为"。本 plan 的 P0 恰好是把这条设计原则缺的那一半（client 主动取消入口）补上，而不是绕开它。
2. **审计发现一个超出本 plan P0 范围的真实缺口，记录但不在本 plan 修**：`server/src/world/extract_system.rs:181-199` 的 `start_extract_request` 校验查询 `Option<&ExtractProgress>`（line 189），**全文件 grep `IsSearching`/`SearchProgress` 命中数为 0**——也就是说当前代码里，玩家在 SEARCHING 状态下按启动撤离键，`ExtractRejectionReason::AlreadyBusy` **不会触发**（该 reason 目前只覆盖"已经在 Extract 中"，不覆盖"正在 Search 中"），与 `docs/finished_plans/plan-tsy-extract-v1.md:585` 自称的"`AlreadyBusy` 覆盖 `AlreadyExtracting` + `IsSearching` 两种情况"不符——这是文档⚠️多报的红旗，实际是 `SearchProgress` + `ExtractProgress` 可能并发共存的独立 bug，触发条件、影响面（server 是否会同时跑两套 tick system 导致状态冲突）都需要单独审计，不在"client 从不发 cancel_search"这个断路的因果链上，不应该混进本 plan 的 P0 顺手改，否则会把一个纯粷输入接线的 PR 变成跨两个系统的行为变更 PR，审查面爆炸。**留给下一个 skeleton 立项**（建议命名 `plan-tsy-search-extract-concurrent-busy-v1`），本 plan 不动 `extract_system.rs`。
3. P0 验收范围维持窄口径：只锁"SEARCHING → 按 H → cancel_search → server 释放 Busy → HUD 回 IDLE"这一条链路，不新增/不修改 `ExtractRejectionReason` 相关校验。

**落点**：`server/src/world/extract_system.rs:181-199`（记录现状，本 plan 不改）/ `docs/finished_plans/plan-tsy-extract-v1.md:582-585`（文档↔代码漂移证据，本 plan 不改该归档文档）/ plan §P0 验收标准（范围已按此收窄）。

### #3 是否补 client/server 回归测试锁住取消链路

**决议**：
1. **补，且两端都补**——审计发现 `server/src/world/tsy_container_search.rs` 的 `handle_cancel_search`（line 636-660）自 `plan-tsy-container-v1` 落地以来**从未有专属测试**（`mod tests` 内 grep `cancel_search` 命中 0），`client/src/test/java/com/bong/client/hud/` 目录**没有 `SearchHudStateStoreTest.java`**（`SearchHudStateStore.markAborted` 的 `CANCELLED` 分支同样零覆盖）——这正是 CLAUDE.md「饱和化测试」要求的"新加函数无测试=没写"红旗，且是本 plan 触碰的代码，理应顺手补齐而非把断路留在原地。
2. server 侧覆盖 3 个分支（happy path 释放锁 / 无 `SearchProgress` 时的幂等 no-op / 取消请求与容器锁 owner 不一致时不误伤他人锁），client 侧覆盖 `AbortReason` 全 4 个变体 + 未知字符串兜底，满足 CLAUDE.md 「enum 变体至少一条专属 case」。
3. 不新增端到端 e2e（client 真实发包 → server 真实收包）测试——`ClientRequestProtocolTest.java:534-539` 已经 pin 了 `encodeCancelSearch()` 的 envelope 形状，`client_request.rs:2207-2209` 已经 pin 了 server 侧 decode，两端协议格式已有专属测试；本 plan 新增的 3+5 条测试补的是"协议之外、两端各自状态机内部"的行为覆盖，两者互不重叠、合起来才是完整链路。

**落点**：`server/src/world/tsy_container_search.rs:758`（`mod tests`，新增 3 个 `#[test] fn handle_cancel_search_*`）/ 新文件 `client/src/test/java/com/bong/client/hud/SearchHudStateStoreTest.java` / plan §P0 交付物 #4-#5。

## Finish Evidence

### 落地清单（P0）
- **client 取消输入**：新文件 `client/src/main/java/com/bong/client/tsy/SearchCancelInteractionBootstrap.java`（`GLFW_KEY_H` 专属按键，`onTick` 门控 `SearchHudStateStore.snapshot().phase()==SEARCHING` 时调 `ClientRequestSender.sendCancelSearch()`，镜像 `ExtractInteractionBootstrap`；绕过 `InteractKeyRouter`，理由见 §8.1 #1）+ `client/src/main/java/com/bong/client/BongClient.java`（`register()` 接线）。
- **lang**：`client/src/main/resources/assets/bong-client/lang/en_us.json`（`key.bong-client.tsy_search_cancel`）+ `zh_cn.json`（`tsy_search_cancel` + 回填历史缺失的 `tsy_extract`/`tsy_extract_cancel`，弥补 §P0 #3 的 zh_cn 债）。
- **server 回归测试**：`server/src/world/tsy_container_search.rs` `mod tests` 新增 3 条 `handle_cancel_search_*`（happy path 释放锁 / 无 `SearchProgress` 幂等 noop / owner 守卫防误清他人锁——该条经变异测试验证：破坏守卫为无条件清锁则撞红）。未改动 `handle_cancel_search` 生产逻辑（协议/system/HUD 分支本就落地，本 plan 只补断掉的 client 输入线 + 测试）。
- **client 回归测试**：新文件 `client/src/test/java/com/bong/client/hud/SearchHudStateStoreTest.java`（`markAborted` 全 `AbortReason` 分支 pin：cancelled/moved/combat/damaged + 未知→NONE + null→NONE + 空标签兜底）。

### 关键 commit
- `d6ae5bf2`（2026-07-06）feat(tsy): 补 TSY 搜刮主动取消输入路径（H 键）+ 两端回归测试（PR #963）。

### 测试结果
- server：`cargo test handle_cancel_search` → 3 passed；`cargo test tsy_container_search` → 27 passed / 0 failed。owner 守卫测试经变异验证（破坏守卫 → `actual=None, expected Some(other_player)` 撞红）。
- client：`./gradlew test build` → BUILD SUCCESSFUL（`SearchHudStateStoreTest` 7 条通过）。
- lang JSON 合法性校验通过；确认 `bong-client/lang` 不在 `scripts/build-resourcepack.sh` 的 `INCLUDE_PREFIXES`，改 lang 不影响资源包 sha1。

### 跨仓库核验
- **client**：`SearchCancelInteractionBootstrap`（新）、`ClientRequestSender.sendCancelSearch()`、`SearchHudStateStore.snapshot().phase()` / `SearchHudState.Phase.SEARCHING` / `AbortReason.CANCELLED`（复用，未新建）。
- **server**：`handle_cancel_search`、`CancelSearchRequest`、`SearchAborted{reason: SearchAbortReason::Cancelled}`、`ClientRequestV1::CancelSearch`（复用，未新建 component/event/schema）。
- 协议契约（`encodeCancelSearch` / `ClientRequestV1::CancelSearch` decode）此前已有专属 pin，本 plan 未改 schema 包。

### 遗留 / 后续
- **`extract_system.rs` 的 `AlreadyBusy` 不覆盖 `IsSearching`**（§8.1 #2 审计发现）：玩家 SEARCHING 中启动撤离时 `ExtractRejectionReason::AlreadyBusy` 不触发，`SearchProgress` + `ExtractProgress` 可能并发共存，是独立 bug，**明确排除本 plan 范围**，留待独立 skeleton（建议 `plan-tsy-search-extract-concurrent-busy-v1`）。本 plan 未动 `extract_system.rs`。
