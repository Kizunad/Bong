# plan-bughunt-named-faction-join-refusal-gate-v1

> Skeleton plan。只读审计产物，来源 issue #1847。

## §0 摘要

`/faction join <named_faction>` 的 handler 只检查注册表状态和当前 reputation，随后直接插入新的 `FactionMembership`。它没有复用 social 的 `FactionMembershipDecisionKind::AcceptInvite` 门禁，因此会把 `betrayal_count`、`invite_block_until_tick`、`permanently_refused` 重置为默认值，绕过永久拒绝和背弃冷却。

## §1 游玩影响

玩家在被势力逐出、背叛次数达到永久拒绝阈值后，仍可通过命令立即重新挂靠同一具名势力；既有社会后果和“洗白”边界失效。该命令还会覆盖当前 membership 的忠诚/等级状态，造成持久化记录与 live state 分叉。

## §2 复现路径

1. 通过 social 的 betray/expel 路径让玩家 `betrayal_count` 达到阈值或设置未来 `invite_block_until_tick`。
2. 确保目标 `NamedFactionRegistry` 状态为 Active、reputation 非负。
3. 执行 `/faction join qingyun_hunters`。
4. `handle_named_faction_join` 直接 `commands.entity(player).insert(FactionMembership { ... defaults ... })`，命令成功且拒绝字段被清零。

## §3 今天 `origin/main` 证据

- `server/src/cmd/gameplay/war.rs:201-245`：`handle_named_faction_join` 只解析具名势力、检查 `Decayed` 和 reputation。
- `server/src/cmd/gameplay/war.rs:248-256`：直接插入 membership，并显式写 `betrayal_count: 0`、`invite_block_until_tick: None`、`permanently_refused: false`。
- `server/src/social/mod.rs:1718-1766`：正式 `AcceptInvite` 路径从 live/persistence 读取 membership，并拒绝 `permanently_refused` 或未到期的 invite block。
- `server/src/social/mod.rs:1782-1793`：betray/expel 会增加 `betrayal_count`、写入 block，并在阈值后设置永久拒绝；这些字段本应被 join 读取而非覆盖。
- `server/src/social/components.rs:247-260`：`FactionMembership` 将三项拒绝状态定义为持久化组件字段。

## §4 非重复比对

- `plan-defense-hardening-v1` 的权限/命令防护不覆盖具名势力社会状态机；本问题是合法玩家绕过业务门禁。
- `plan-bughunt-war-lifecycle-retention-v1.md` 处理 NPC 战事 store，不处理玩家 `/faction join` membership 覆盖。
- social `AcceptInvite` 已有正确门禁，本骨架的目标是把 gameplay command 接入该既有入口，不另造第三套拒绝逻辑。

## §5 立项检查记录

- **worldview**：查 `身份与信誉`、`翻脸`、`势力`、`永久`；命中 `docs/worldview.md §十一.身份与信誉` 与 §十一危机分层，社会背叛后果不应被命令清零。
- **finished_plans**：查 `FactionMembershipDecisionKind::AcceptInvite`、`betrayal_count`、`permanently_refused`、`handle_named_faction_join`；social 状态机已有实现，命令接线未覆盖。
- **active plan**：查 `cmd/gameplay/war.rs`、`faction join`、`AcceptInvite`；未见 active plan 收口该调用点。
- **skeleton**：查 `named faction join`、`refusal`、`invite_block`、`betrayal`；无同一根因骨架。
- **reminder.md**：查 `faction join`、`拒绝`、`背弃`、`invite`；仓内无对应条目。

## §6 接入面与跨仓契约

- **Inputs**：命令 target、`NamedFactionRegistry`、玩家 `FactionReputation`、当前 tick、持久化/live `FactionMembership`。
- **Outputs**：接受时更新 `FactionMembership` 并发送既有聊天反馈；拒绝时保持 membership/持久化字段不变。
- **共享类型或事件**：复用 `FactionMembership`、`FactionMembershipDecisionEvent`、`FactionMembershipDecisionKind::AcceptInvite`、`FactionReputation`；不新造 join request schema。
- **server 符号**：`cmd::gameplay::war::handle_named_faction_join`、`social::apply_faction_membership_decisions`、`load_social_faction_membership_from_persistence`。
- **agent**：无变更；该 join 是 server command/social persistence 链路，agent 不发起或判定势力邀请。
- **client**：无变更；Minecraft command/chat 入口保持原样，拒绝/接受文本由 server 权威返回。
- **worldview 锚点**：`docs/worldview.md §十一` 的身份、信誉和社会后果。

## 阶段总览

| 阶段 | 状态 | 交付物 |
| --- | --- | --- |
| P0 | ⬜ | 让具名 join 复用 AcceptInvite 的拒绝检查与持久化 membership 来源 |
| P1 | ⬜ | 背叛阈值、冷却、低 reputation、重复 join 的回归测试 |

## P0：统一业务门禁

- 解析 target 后构造/发送同一 `FactionMembershipDecisionEvent`，或抽出共享纯校验；不得在命令内重新构造默认拒绝字段。
- 接受时只更新允许变更的 faction/rank/loyalty，保留 `betrayal_count`、block、permanent 状态；拒绝路径零 mutation。

## P1：持久化与反馈

- 覆盖 live component 缺失但 SQLite 有记录、断线重连、同 tick 重复命令、冷却刚到期/未到期。
- 聊天反馈明确区分 decay、reputation、invite block、permanent refusal，且命令与 social event 结果一致。

## 验收测试计划

- `permanently_refused=true` 或 `invite_block_until_tick > now` 时 join 不插入/覆盖 membership。
- 正常 AcceptInvite 保留历史 `betrayal_count` 等字段，成功后才更新允许字段。
- `/faction join` 与直接 `FactionMembershipDecisionEvent::AcceptInvite` 对同一 fixture 得到相同结果。

## 来源 issue

- #1847 `[flash-review][major] /faction join 具名势力绕过永久拒绝/背弃封禁门禁`
