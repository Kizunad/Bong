# plan-tsy-search-cancel-v1（骨架）

> **骨架（草案）**。一句话主题：TSY 容器搜刮的**主动取消链路断路**——server 已有 `cancel_search` 协议、取消 system 与 HUD `cancelled` 分支，但 client 端没有任何输入路径会调用 `sendCancelSearch()`，导致玩家开始搜刮后无法按设计用 `ESC`/动作键主动取消。已对 `docs/plan-bughunt-r*.md`、`docs/finished_plans/plan-bughunt-r*.md` 与现有 skeleton 去重，未见同题。

> 实际游玩影响：玩家在 TSY 里一旦开始搜刮，只能靠**移动 / 受击 / 进战斗**被 server 被动打断，不能按设计原地取消；而 `plan-tsy-extract-v1` 又明确要求“先 ESC 取消搜刮，再开始撤离”，所以遇到突发危险时会出现**想撤离却先被 `AlreadyBusy` 拒绝**的主路径背离。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | TSY 搜刮主动取消断路（client 未发 `cancel_search`） | fix_pr | ⬜ |

## P0 — TSY 搜刮主动取消断路

- **高置信 bug（fix_pr）**：
  - `client/src/main/java/com/bong/client/network/ClientRequestSender.java:363` 定义了 `sendCancelSearch()`，但全仓 grep 调用点为 **0**；`ClientRequestProtocol.encodeCancelSearch()` 也只有定义无使用。
  - `server/src/world/tsy_container_search.rs:127-149` 注释与 `CancelSearchRequest` 明确写的是“玩家主动取消（点 ESC / 切武器等）”；`handle_cancel_search`（`:636-660`）会正确移除 `SearchProgress` / `IsSearching`、释放 `searched_by` 锁并发 `SearchAborted { reason: Cancelled }`。
  - `client/src/main/java/com/bong/client/hud/SearchHudState.java:29-33` 和 [SearchHudStateStore.java](/home/kiz/Code/Bong/.worktree/bughunt-loop-20260704-j/client/src/main/java/com/bong/client/hud/SearchHudStateStore.java:24) 已有 `CANCELLED` 分支，说明客户端 HUD 也预期会收到主动取消结果。
  - `docs/finished_plans/plan-tsy-container-v1.md:671` 明写“搜刮期间按 `ESC` / 任意动作键 → 发 `CancelSearchRequest`”；`docs/finished_plans/plan-input-binding-v1.md:293` 也把“主动取消走 `cancel_search`”列为验收项。
  - 对照已落地的撤离链路：`client/src/main/java/com/bong/client/tsy/ExtractInteractionBootstrap.java:39-45` 已经有显式 `sendCancelExtract()` 接线，说明“主动取消只做 server 被动中断”**不是团队统一设计**；搜刮这里是少接了一段 client 输入。

## 开放问题

1. 主动取消的输入应该怎么落：严格按文档接 `ESC`/动作键，还是允许再次按 G 作为显式 cancel fallback。
2. 搜刮进行中若玩家尝试撤离/其它 busy 行为，是否仍保持当前 `AlreadyBusy` 语义，还是允许 client 先自动发 `cancel_search` 再转发下一动作。
3. 是否补一条 client 回归测试，锁住“进入 SEARCHING 后按取消键必须发 `cancel_search`，并在收到 `search_aborted(cancelled)` 后清 HUD”。

## 审计来源

bughunt 新线程 J（限定 scope：`server/src/movement/`、`server/src/player/`、`server/src/cmd/` 交互相关、`client/.../input|environment|ui`）。候选经两轮默认怀疑式证伪后存活：

1. **第一轮怀疑**：是否只是文档没更新？反证失败，因为 server 取消 event、HUD `CANCELLED` 分支、协议编码和注释都已落地，缺的只有 client 调用点。
2. **第二轮怀疑**：是否存在隐式取消入口？反证失败，全仓无 `sendCancelSearch()` 调用；反而 `ExtractInteractionBootstrap` 存在完整取消接线，证明搜刮链路是独立断路。
