# plan-bughunt-tsy-search-extract-concurrent-busy-v1

> Skeleton Plan。C10 client-ui 线程发现，修复属性为 cross-stack：client 输入入口错配 + server gameplay 忙态互斥缺失。本文只立 bug skeleton，不消费、不归档、不改代码。

## Bug 摘要

TSY 容器搜刮进行中，client 的撤离键 `Y` 仍会发送 `start_extract_request`；server 的 `start_extract_request` 也只把既有 `ExtractProgress` 视为 `AlreadyBusy`，没有把 `SearchProgress` / `IsSearching` 计入忙态。结果是同一玩家可进入 `SearchProgress + ExtractProgress` 并发状态，违背“必须先显式取消搜刮，再启动撤离”的既定设计。

## 实际游玩体验影响

玩家在搜刮中站到撤离触发范围内按 `Y`，可能同时看到“正在搜刮”和“撤离中”两套倒计时，绕过“先决定放弃搜刮再撤离”的玩法选择。在特定空间条件下，这会让 loot 获取、容器锁释放、撤离传出结算的先后顺序变得不清。当前证据不足以声称必然复制物品、吞真元或卡死；核心问题是忙态互斥不再可信。

## 证据定位

- `client/src/main/java/com/bong/client/tsy/ExtractInteractionBootstrap.java:33-45`：`Y` 键路径只判断 `!ExtractStateStore.snapshot().extracting()`，找到最近撤离裂口后直接 `ClientRequestSender.sendStartExtract(...)`，没有读取 `SearchHudStateStore.snapshot().phase()`。
- `client/src/main/java/com/bong/client/hud/SearchHudState.java:24-29` 与 `SearchHudStateStore.java:13-30`：client 已有 `SEARCHING` 状态，且搜刮取消键 `SearchCancelInteractionBootstrap.java:40-46` 正是通过该状态门控发 `cancel_search`。
- `server/src/world/extract_system.rs:181-280`：`players` query 只有 `Option<&ExtractProgress>`；`AlreadyBusy` 仅由 `existing_progress.is_some()` 触发；通过后直接插入 `ExtractProgress`。
- `server/src/world/tsy_container_search.rs:407-420`：搜刮启动会插入 `SearchProgress` 与 `IsSearching`，但该状态没有被撤离启动查询消费。
- `server/src/world/tsy_container_search.rs:306-420`：反向也需审计，`start_search_container` 通过校验前未见 `ExtractProgress` 忙态门禁。
- `docs/finished_plans/plan-tsy-extract-v1.md:582-585`：已定设计是搜刮中启动撤离应按 `AlreadyBusy` 拒绝，玩家必须先取消搜刮。
- `docs/finished_plans/plan-tsy-search-cancel-v1.md:68-73,108`：该 finished plan 明确把 “`SearchProgress + ExtractProgress` 可能并发共存”列为独立遗留 bug，本 plan 是把遗留项转成 active skeleton，不是重复修 `cancel_search`。

## 触发路径

1. 玩家进入 TSY，开始搜刮容器，client 收到 `search_started` / `search_progress` 后进入 `SearchHudState.Phase.SEARCHING`。
2. 玩家在搜刮未结束时站到可撤离裂口触发范围内。
3. 玩家按 `Y`。
4. client `ExtractInteractionBootstrap` 因未处于 `extracting()`，直接发送 `start_extract_request`。
5. server `start_extract_request` 不检查 `SearchProgress` / `IsSearching`，可能接受并插入 `ExtractProgress`。
6. 同一玩家同时保留搜刮与撤离进度，两个系统后续各自 tick / complete / abort。

## 反方审查记录

- 第 1 轮反方：ACCEPT。反方确认这不是纯 server-side，client `Y` 键绕过 `InteractKeyRouter` 且没有天然 screen/HUD gate；server 也缺权威拦截。未发现开放 PR 或 active plan 覆盖。反方要求补强 server 最小失败测试、client 发包证据、空间可达性说明，并审反向 `start_search`。
- 第 2 轮反方：ACCEPT。反方要求标为 cross-stack；若体系单选，应偏 server-gameplay，因为 busy invariant 必须由 server 保证。`plan-tsy-search-cancel-v1` 只是 finished plan 中记录遗留项，不可替代 active plan。体验影响应克制表述为“双倒计时 / 绕过显式取消 / 结算顺序不清”，不得夸大为必然复制或卡死。

## Skeleton Fix Plan

1. client：在 `ExtractInteractionBootstrap` 的启动撤离分支加入 `SearchHudStateStore.snapshot().phase() == SEARCHING` 门禁；SEARCHING 时不发送 `start_extract_request`，并保留 `H` 键显式 `cancel_search` 语义，不做自动 `cancel_search -> start_extract` 串联。
2. server：`start_extract_request` 查询并拒绝 `SearchProgress` / `IsSearching`，返回 `ExtractRejectionReason::AlreadyBusy`，且不得插入 `ExtractProgress`。
3. server：审并补齐反向互斥。玩家已有 `ExtractProgress` 时，`start_search_container` 应拒绝启动搜刮；拒绝 reason 可复用既有 busy 语义或新增明确 wire，但必须保持客户端 HUD 可理解。
4. 文档/命名：把该修复描述为 TSY busy invariant 修复，不归类为单纯 UI polish。

## 验收测试计划

- server 单测：玩家已有 `SearchProgress + IsSearching + TsyPresence`，附近有 exit portal，发送 `StartExtractRequest`；期望 `Rejected(AlreadyBusy)`，且玩家没有新增 `ExtractProgress`。
- server 单测：玩家已有 `ExtractProgress` 时请求 `start_search_container`；期望拒绝，且不插入 `SearchProgress` / `IsSearching`，不占用容器 `searched_by`。
- client 单测或 seam：`SearchHudState.Phase.SEARCHING` 时触发撤离键逻辑，不发送 `start_extract_request`。
- client 回归：非 SEARCHING 且 portal in range 时，`Y` 仍发送 `start_extract_request`。
- e2e / smoke：搜刮中按 `Y` 不产生撤离倒计时；按 `H` 取消搜刮后再按 `Y` 可正常启动撤离。
- HUD 回归：若 server 下发 `already_busy`，撤离 HUD 的拒绝提示不退化为空白或未知错误。

## 风险

- 只修 client 会被恶意包或竞态绕过；server 权威门禁必须同修。
- 只修单向撤离会留下“撤离中再搜刮”的反向双态缺口。
- 如果新增 busy rejection wire，需同步 proto/schema/client parser；否则应优先复用已有 `AlreadyBusy` 语义以缩小协议面。
- 空间可达性依赖裂口与容器位置关系，不能把“所有场景必现”写进验收；测试应构造同范围场景锁行为。
