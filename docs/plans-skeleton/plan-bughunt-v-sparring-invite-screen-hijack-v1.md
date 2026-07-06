# plan-bughunt-v-sparring-invite-screen-hijack-v1（骨架）

> **骨架（草案）**。一句话主题：`client/social` 的切磋邀请在目标玩家已经打开别的 GUI 时，会被 `SparringInviteScreenBootstrap` 每 tick 强制顶屏，和 `plan-social-v1` 已定的“HUD 闪烁避免漏看”相违，实际游玩中会打断背包/炼丹/检视/其他交互操作。

> 立项动机：本轮 bughunt V 聚焦 `npc-war/social/preview` 主路径，排除已立项的 identity/social 名声分叉、npc trade gate、ambient audio、world environment resync 后，`sparring_invite` 这条 client social 主链存在一个高置信、直接影响玩家操作体验的 UI 抢占 bug。它不是“trade gate”重复题：问题不在交易或权限门控，而在 **切磋邀请的 GUI 调度策略**。

## Bug 摘要

- **high（plan_skeleton）**：`client/src/main/java/com/bong/client/social/SparringInviteScreenBootstrap.java:17-33` 每个 client tick 读取 `SocialStateStore.sparringInvite()` 后，只要当前 screen 不是“同一个 inviteId 的 `SparringInviteScreen`”，就直接 `client.setScreen(new SparringInviteScreen(invite))`。这意味着目标玩家在打开任何其他 GUI 时，只要收到切磋邀请，就会在下一 tick 被强制切到切磋弹窗；若尝试回到原 GUI，只要邀请还没过期，又会继续被顶掉。
- server 侧没有“目标必须空闲/未开屏”前置条件：`server/src/social/mod.rs:789-843` 的 `dispatch_sparring_invites` 只校验发起者/目标存在、未终结，并直接下发 `ServerDataPayloadV1::SparringInvite`。
- client 收包后也没有“仅提示、不抢屏”的缓冲层：`client/src/main/java/com/bong/client/network/SocialServerDataHandler.java:181-207` 只会把 payload 写入 `SocialStateStore.replaceSparringInvite(invite)`，随后 bootstrap 在 tick 中无条件接管 screen。
- 与之形成直接反证的是同目录下的 `TradeOfferScreenBootstrap`：它已经在 F4 修过“其他 GUI 挡住时静默/打断”的 UX 问题，引入 `ScreenKind`/`Decision` 和 `BLOCKED_TOAST`，明确选择“不抢占当前 GUI，只给提示”。切磋邀请链没有跟上这套处理。

## 这个 bug 对实际游玩体验的影响

- 玩家在整理背包、看检视面板、炼丹/炼器、阅读其他弹窗时，只要被人发起切磋邀请，当前界面会被强制切走；10 秒有效期内几乎无法继续手头操作。
- 这不是一次性打断，而是 **持续抢屏**：玩家关掉切磋弹窗回到原 GUI 后，下一 tick 只要邀请还在，`SparringInviteScreenBootstrap` 会再次 `setScreen(...)`。
- `plan-social-v1` 已明确该需求应是“10s 倒计时 + HUD 闪烁避免漏看”，不是 modal 抢占式交互。当前实现把“提醒”做成了“强制接管”，实际体感会比交易邀请更差，因为交易邀请至少已经修成了“被挡时 toast 提示”。

## 证据链

1. `server/src/social/mod.rs:789-843`：`dispatch_sparring_invites` 对 target 直接发 `ServerDataPayloadV1::SparringInvite`，不看目标是否正处于其他 GUI / busy state。
2. `client/src/main/java/com/bong/client/network/SocialServerDataHandler.java:181-207`：收到 `sparring_invite` 后只写入 `SocialStateStore.replaceSparringInvite(invite)`，说明 screen 调度完全交给 client tick bootstrap。
3. `client/src/main/java/com/bong/client/social/SparringInviteScreenBootstrap.java:17-33`：若当前 screen 不是同一份 `SparringInviteScreen`，直接 `client.setScreen(new SparringInviteScreen(invite))`；没有 `OTHER` 分支、没有 blocked toast、没有 defer。
4. `docs/finished_plans/plan-social-v1.md:372`：定稿设计写的是“切磋邀请 UI 10s 倒计时；对方无响应自动取消，HUD 闪烁避免漏看”。
5. `client/src/main/java/com/bong/client/social/TradeOfferScreenBootstrap.java:31-96` + `client/src/test/java/com/bong/client/social/TradeOfferScreenBootstrapTest.java`：同类社交邀请已经被证明“不该在其他 GUI 打开时抢屏”，而应走 `BLOCKED_TOAST` 提示。

## 两轮反方裁决摘要

- **Round 1 反方**：这可能是故意的 modal 设计，切磋邀请需要玩家立刻响应，所以强制顶屏不算 bug。
  **裁决**：否。`plan-social-v1.md:372` 的已决条目写的是“HUD 闪烁避免漏看”，不是“强制抢占当前 GUI”；同为 10 秒限时邀请的 `TradeOfferScreenBootstrap` 也已经明确修成“其他 GUI 打开时只提示不抢屏”。若切磋真要 modal，设计文档和同目录实现不会出现这种反向证据。

- **Round 2 反方**：即便会抢屏，实际只会发生在少数空闲场景，影响有限，未必值得立项。
  **裁决**：否。server `dispatch_sparring_invites` 没有任何“目标必须空闲”的门控，client bootstrap 还是每 tick 重试型 `setScreen(...)`；因此只要目标在线且收到邀请，不论他正在背包、inspect、炼丹还是别的 GUI，都属于可触发面。问题不是边缘 case，而是切磋邀请的常规到达路径本身。

## 建议修复方向（供后续 fix_pr）

- 参照 `TradeOfferScreenBootstrap` 抽出 `ScreenKind` / `Decision`，把 `OTHER` 屏幕分支改成“提示但不抢屏”。
- 增加 blocked toast / expired toast，避免“既不抢屏也无反馈”的另一种静默失败。
- 为 `SparringInviteScreenBootstrap` 补纯决策单测，覆盖 `invite == null`、`expired`、`matching invite screen`、`other screen blocked` 四类路径。
- 验收应包含“打开 Inspect/Alchemy/Inventory 时收到切磋邀请，不应被强制顶屏，但应有提示；关闭当前 GUI 后可进入邀请弹窗”。
