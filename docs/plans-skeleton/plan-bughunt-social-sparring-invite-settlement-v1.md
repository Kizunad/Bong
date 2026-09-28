# BugHunt：接受切磋邀请时其余邀请被客户端静默丢弃

> 来源 Issue：#1430。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 单个 invite 的接受/拒绝与其余 pending 的结算语义 | ⬜ |
| P1 | server/client 回执和状态收敛回归 | ⬜ |

## §0 摘要

SocialStateStore.acceptSparringInvite 接受一个邀请时把全部 pending invite 写入 settled 并清空，但 SparringInviteScreen 只向 server 回应被接受的 invite。其它发起者的 pending 仍留在 SparringInviteRegistry，直到超时静默删除，造成两端状态分叉和发起者无反馈。

## §1 游玩影响

同时收到两份切磋邀请时，接受 A 会让 B 在目标客户端消失，却不会向 B 的发起者发送拒绝/超时；发起者继续看到旧邀请或等待到无声过期。

## §2 复现路径

1. 两名玩家同时向同一目标发起 sparring_invite。
2. 目标在 SparringInviteScreen 接受 A。
3. 观察 client 清空 A、B，而只发送 A 的 sparring_invite_response；server 的 B 仍在 pending。

## §3 今天 origin/main 的根因证据

- client/src/main/java/com/bong/client/social/SocialStateStore.java:113-122 遍历所有 sparringInvites.keySet() 后 clear()。
- client/src/main/java/com/bong/client/social/SparringInviteScreen.java:85-93 只调用被接受 invite 的 sendSparringInviteResponse。
- server/src/social/mod.rs:891-948 由 dispatch_sparring_invites 写入 SparringInviteRegistry，:948-1007 的 handle_sparring_invite_responses 只处理收到的 invite，expire_sparring_sessions 才会清理未回执项。

## §4 非重复比对与立项检查记录

- worldview：查 docs/worldview.md 的切磋、社交承诺与声誉关键词；本骨架只修邀请结算，不改变胜负规则。
- finished_plans：查 plan-social-v2.md、plan-bughunt-sparring-invite-screen-hijack-v1.md；未发现“接受一个时批量清其它 pending”的修法。
- active plan：查 docs/plan-*.md 的 SparringInviteRegistry、SocialStateStore、SparringInviteResponse；未发现同一状态分叉修法。
- skeleton：查 sparring_invite_response、acceptSparringInvite；未发现同根因骨架。
- reminder.md：仓内无 docs/plans-skeleton/reminder.md，无相关条目。

## §5 修复骨架

- 接受 A 时只结算 A；对 B 等仍 pending 的 invite，按产品语义逐一发送 decline/withdraw 回执，或在 server 增加“目标已选择其它邀请”的权威批量结算事件。
- client 不得在没有 server 回执时把其它 invite 标记 settled；回执按 invite_id 幂等处理。
- 验收：A accepted、B declined/withdrawn，双方 registry/store 终态一致；重复回执和过期回执不复活邀请。

## §6 接入面与跨仓契约

- Inputs：sparring_invite S2C 的 invite_id/initiator/target/expires_at_ms，以及 screen 的接受/拒绝动作。
- Outputs：每个 invite 的 sparring_invite_response 或明确的 server settlement payload；client SocialStateStore 只保留权威 pending。
- 共享类型或事件：复用 SparringInvitePayloadV1、SparringInviteResponseEvent、SparringInviteRegistry，不新增并行 pending map。
- server 符号：dispatch_sparring_invites、handle_sparring_invite_responses、expire_sparring_sessions、SparringInviteResponseKind。
- agent：无变更；切磋邀请走 server social payload，不经 agent IPC。
- client：SocialStateStore.acceptSparringInvite、SparringInviteScreen.settle 和 SocialServerDataHandler 需按 invite_id 收敛。
- worldview / qi_physics：对应社交承诺与切磋关系；本骨架不涉及真元流或 qi_physics。
