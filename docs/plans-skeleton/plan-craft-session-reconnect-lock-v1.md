# plan-craft-session-reconnect-lock-v1（骨架）

> **骨架（2026-07-05）**。一句话主题：`client/craft/CraftStore` 在断线时不清空，而 `server/network/craft_emit.rs` 在玩家重连时只补发 `craft_recipe_list`、不会补一条 idle `craft_session_state`；如果玩家断线前正有手搓/制作台任务，重连后客户端会把“制作进行中”状态永久带到新会话，把 craft 主路径锁死。

> **这个 bug 对实际游玩体验的影响**：玩家在手搓或制作台制作途中掉线、切服或重连后，重新打开 `CraftScreen` / `WorkbenchScreen` 会一直看到“制作进行中”，`CraftActionBar` 的一键填充、数量调整、开始制作全部灰掉；关界面还会继续发 `craft_cancel`，但 server 对“当前无 session”的 cancel 是 noop，不会自愈。结果是玩家常规游玩里会遇到“断线一次后合成系统卡死”，直到再次触发真正的 craft 状态推送，甚至重启客户端。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 断线清理 + join hydration 补齐，消除跨会话 stale craft state | fix_pr | ⬜ |

## P0 — 断线清理 + join hydration

- **client 断线清理缺口**：`client/src/main/java/com/bong/client/BongNetworkHandler.java:131-170` 的 `ClientPlayConnectionEvents.DISCONNECT` 会清一长串 HUD/store，但没有调用 `CraftStore.clear()`；而 `CraftStore.clear()` 明明已经实现（`client/src/main/java/com/bong/client/craft/CraftStore.java:112-120`），能把 recipes/session/outcome 一次性复位为 idle/empty。
- **server join hydration 缺口**：`server/src/network/craft_emit.rs:553-584` 的 `emit_recipe_list_on_join()` 只在 join 后补发 `CraftRecipeList`；`emit_craft_session_state()` 则只扫描 `With<CraftSessionStateDirty>`（`:437-457`），没有“新连接且当前无 session 时补一条 idle”的路径。
- **卡死链路**：
  - 断线前：client 收到过 active `craft_session_state`，`CraftStore.session` 被写成 active（`client/network/CraftSessionStateHandler.java:14-29`）。
  - 断线时：`CraftStore.session` 不清，旧 active 留在静态仓。
  - 重连后：server 若当前没有 `CraftSession`，不会把该玩家标 dirty，也不会主动发 idle `craft_session_state`。
  - UI 读 stale session：`CraftScreen` / `WorkbenchScreen` 都直接取 `CraftStore.sessionState()` 刷 action bar（`client/craft/CraftScreen.java:231-234`、`client/craft/WorkbenchScreen.java:187-189`）。
  - `CraftActionBar.refresh()` 中 `activeSession=true` 会把 fill/加减/开始全部禁用，并显示“制作进行中”（`client/craft/CraftActionBar.java:93-101,124-127`）。
  - 玩家关界面时，`CraftScreen.removed()` / `WorkbenchScreen.removed()` 还会因为 stale active 再发一次 `sendCraftCancel()`（`client/craft/CraftScreen.java:100-104`、`client/craft/WorkbenchScreen.java:108-112`）；但 server 在 `existing == None` 时直接 noop（`server/src/network/craft_emit.rs:253-265`），所以不会把 client 状态纠正回来。
- **修复方向**：
  - client：把 `CraftStore.clear()` 接到 disconnect。
  - server：join/hydrate 时无论有无活跃 `CraftSession`，都给该 client 发一次当前 `CraftSessionStateV1`；无 session 明确发 idle。
  - 两端都做，避免“断线靠 client 自清、重连靠 server 自愈”任一侧单点失手。
- **测试**：
  - client：断线后 `CraftStore` 归零；stale active session 不会跨 session 泄漏到新连接。
  - server：新连接无 session 时仍收到 `active=false` 的 `CraftSessionStateV1`；有 session 时收到 active hydration。
  - e2e：制作中断线/重连后，手搓与制作台都能再次开始制作，不再出现永久灰按钮。

## 开放问题

1. `CraftStore` 的 `lastOutcome/lastUnlocked` 是否也应在 disconnect 一并清空，避免上一局 toast/解锁提示串味。
2. `emit_recipe_list_on_join()` 是否顺带合并成单次 craft hydration（recipe list + session state），统一“craft 登录态”收口，避免后续再出现只补一半的跨会话 store。
3. 是否要给 `InspectScreen.craftStatusLine()` 补断线回归，防止检查页继续显示“当前任务进行中”。

## 审计来源

bughunt 线程 AK（2026-07-05，economy/craft 主路径）。候选点集中在 `server/src/network/craft_emit.rs`、`client/src/main/java/com/bong/client/craft/`、`client/src/main/java/com/bong/client/BongNetworkHandler.java`。结论：这是一个 **player-facing、可稳定复现、非设计取舍** 的真 bug，且与已排除的 `craft close pause`、`npc trade bundle 少发货`、`trade first item autopick` 不重复。
