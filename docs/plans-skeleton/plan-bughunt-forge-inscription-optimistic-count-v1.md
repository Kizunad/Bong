# plan-bughunt-forge-inscription-optimistic-count-v1

> 来源 Issue：#1835。铭文面板把 server 的已填槽计数与永不回收的本地乐观列表相加，实际可投残卷数被提前耗尽。
>
> 阶段总览：P0 ⬜ 让 server 快照成为唯一门禁来源；P1 ⬜ 补投入、拒绝和回推快照的 UI 回归。

## §0 摘要

InscriptionPanelComponent.currentRenderState 先读 ForgeSessionStore 的 filled_slots，再把 acceptedInscriptionIds 全部追加。onScrollDropped 在发出 C2S 请求后立即把 inscriptionId 放入列表，但当前 forge_session 只回推 filled_slots、max_slots、failed，不回推已投入的 ID，也没有 ack 对应列表清理。一次真实投入因此被算作 server 已填一槽加本地 pending 一槽，门禁可能提前认为铭文槽已满。

## §1 实际游玩体验影响

- 玩家投一张残卷后，面板可能显示多占一个槽，后续仍有空槽却无法继续投入。
- server 已拒绝或延迟处理的请求也可能留在 acceptedInscriptionIds，重开面板或收到快照后表现不一致；背包扣除仍由 server 决定，UI 的错误不能当作物品已成功消耗的证据。

## §2 复现路径

1. 收到 inscription step 的 filled_slots=0、max_slots=2 快照。
2. 调用 InscriptionPanelComponent.tryDropScroll；onScrollDropped 发出 sendForgeInscriptionScroll 并追加一个本地 ID。
3. server 处理 InscriptionScrollApplied 后通过 push_forge_session_snapshot_on_interaction 回推 filled_slots=1。
4. currentRenderState 合并 server 的一项和 acceptedInscriptionIds 的一项，filledCount=2；第二张真实残卷被错误拒绝。

## §3 根因证据

- client/src/main/java/com/bong/client/forge/screen/InscriptionPanelComponent.java:53-63 把 base.filledSlots 与 acceptedInscriptionIds 合并，并以合并长度计算 maxSlots。
- :92-113 在发送 ClientRequestSender.sendForgeInscriptionScroll 后无条件 add，且没有 server ack、拒绝或关闭路径清理。
- server/src/network/forge_snapshot_emit.rs:240-271 监听 InscriptionScrollApplied 后回推 session；:379-414 的 Inscription step 只发送 filled_slots、max_slots、failed。
- server/src/schema/forge.rs:133-138 的 ForgeStepStateDataV1::Inscription 没有 inscription IDs；agent/packages/schema/src/server-data.ts:1662-1669 只包装 ForgeSessionDataV1。
- server/src/network/client_request/forge.rs:92-99 将 C2S 铭文请求路由到 ForgeRequest::InscriptionScroll；server/src/forge/mod.rs:529-576 校验实例并实际扣除物品后才递增 session 状态。

## §4 非重复比对

- #1836 处理 forge outcome 结束态 active 标记，不涉及 inscription step 的显示计数。
- forge-session-range/session-enum 等骨架处理会话权限和枚举，不拥有 acceptedInscriptionIds；本 plan 只修客户端乐观状态与 server authoritative snapshot 的合并。

### 立项检查记录（2026-09-28）

- docs/worldview.md：检索“炼器、铭文、残卷、装备、物品消耗、锻造”，核对器修/装备与物品消耗章节；未发现客户端乐观计数的规则。
- docs/finished_plans/：检索 InscriptionPanelComponent、ForgeStepStateDataV1、filled_slots、acceptedInscriptionIds；命中 forge 基础文档，未发现本双计根因的完成证据。
- active plan（docs/plan-*.md）：检索同上符号及 forge inscription；未发现另一个 active plan 改同一面板状态。
- docs/plans-skeleton/：检索 forge_session、filled_slots、inscription IDs、acceptedInscriptionIds；未发现同主题骨架。
- docs/plans-skeleton/reminder.md：检索 forge inscription、filled_slots、乐观、残卷；仓内无该主题延后事项。

## 接入面与跨仓契约

- **Inputs**：client ForgeSessionStore 快照、InscriptionPanelComponent 拖拽、ClientRequestProtocol 的 ForgeInscriptionScroll、server ForgeSession/PlayerInventory。
- **Outputs**：server 权威 filled_slots、failed、max_slots 快照和 client 面板门禁；投入成功、拒绝、会话关闭都能收敛到同一状态。
- **共享类型 / 事件**：ForgeSessionDataV1、ForgeStepStateDataV1::Inscription、InscriptionScrollSubmit、InscriptionScrollApplied、ClientRequestV1::ForgeInscriptionScroll、forge_session。
- **server 契约符号**：try_into_forge_request、handle_scroll_submits、consume_item_instance_once、push_forge_session_snapshot_on_interaction、build_step_state。
- **agent**：有变更可能性；现有 agent/packages/schema/src/server-data.ts 仅镜像计数，若选择新增 request-id/已填 IDs，必须同步 TypeBox、registry 和 sample；若保持现有字段，则 agent 无变更并在实现 PR 中注明依据。
- **client**：有变更；InscriptionPanelComponent.currentRenderState/onScrollDropped、ForgeSessionHandler/ForgeSessionStore 是唯一 UI 状态入口。
- **worldview 锚点**：docs/worldview.md 中器修、装备和物品消耗相关章节；本 plan 不改变锻造成功率、真元成本或物品所有权规则。
- **qi_physics**：铭文投入不直接新增真元；若未来面板展示 consecration 的 qi_injected，仍只读 ForgeSession 快照，不得在 client 重算或扣真元。

## §5 修复骨架

### P0 取消“已确认计数 + pending 计数”的双重门禁

- 以 server filled_slots/max_slots/failed 为唯一可提交门禁。pending 只能作为视觉中的等待标记，不能增加 filledCount 或 maxSlots。
- 收到任意同 session 的 forge_session 快照时，按 authoritative count 收敛 pending；拒绝、session 变更、断线和关闭都必须清空旧 pending。若产品确需显示 ID，必须让 server 回推稳定 IDs 或 request-id，而不是猜测。
- 保留 ClientRequestSender.sendForgeInscriptionScroll 和 server 的实例校验/扣除顺序，不以本地 add 代替 server ack。

### P1 回归与验收

- 0/2 槽投入成功后收到 filled_slots=1 时仍可继续投第二张；同一请求重复回推不会增加计数。
- server 拒绝、网络延迟、会话关闭和断线后，pending 不会阻塞下一次合法投入。
- 现有 forge_session wire 字段与 agent/client parser 在没有新增字段时保持兼容；若新增字段，补三端 fixture。
