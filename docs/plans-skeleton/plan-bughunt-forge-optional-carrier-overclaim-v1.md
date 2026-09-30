# plan-bughunt-forge-optional-carrier-overclaim-v1

> 骨架：来源 #1362。锻造投料只裁剪 required 材料，optional carrier 整堆消费。

## §0 摘要

`server/src/forge/mod.rs:331-345` 将请求材料聚合后仅按 `billet_profile.required` 做 `min` 裁剪；optional carrier 继续保留请求整量，后续消费会把多出的载体一并销毁。

## §1 游玩影响

玩家从背包点选一整堆可选载体时，配方只需要一部分却消耗全部，稀有灵兽/灵木材料会无提示丢失。

## §2 复现路径

1. 选择带 optional carrier 的 blueprint，背包放入数量大于配方所需的同种材料。
2. 发起 forge，观察 `inputs` 只对 required 做封顶。
3. 锻造失败或成功后核对库存，超出需求的 optional 数量已被消费。

## §3 今天 `origin/main` 根因证据

- `server/src/forge/mod.rs:337-345` 的注释明确写“非 required 不裁剪”。
- `:346-365` 将裁剪后的 `inputs` 交给 billet 解析并进入会话/结果，未保留 optional 超量。
- 消费路径没有向玩家返还未声明需求的 optional carrier。

## §4 非重复比对

`plan-forge-v1` 定义材料准备和失败结果，`plan-bughunt-forge-item-material-validation-v1` 关注类型合法性；本骨架只处理数量上限，不改变 carrier 的合法来源。

## §5 立项检查记录

本记录已按 `docs/worldview.md`、`docs/finished_plans/`、`docs/plan-*.md`、`docs/plans-skeleton/` 与 `docs/plans-skeleton/reminder.md` 逐项检索。

- **worldview**：查“锻造、载体、材料、消耗”；`worldview.md §八` 要求材料作为可追溯资源。
- **finished_plans**：查 `MaterialStack`、`optional carrier`、`ForgeOutcome`；无 optional 数量结算契约。
- **active plan**：查 `resolve_billet`、`material_preparation`；未见同一消费修复。
- **skeleton**：查 `forge optional`、`overclaim`、`carrier count`；无重复骨架。
- **reminder.md**：查 `forge`、`carrier`、`材料超量`；仓内无对应条目。

## §6 接入面与跨仓契约

- **Inputs**：`ForgeRequest.materials`、`BilletProfile.required/optional`、`MaterialPreparation`。
- **Outputs**：`ForgeOutcomeEvent`、库存扣除/返还、`ForgeSession` 快照。
- **共享类型或事件**：复用 `MaterialStack`、`ForgeOutcomeEvent`、`ForgeBucket`；不另造 optional 物品类型。
- **server 符号**：`forge::handle_forge_request`、`resolve_billet`、`ForgeOutcomeEvent`、`network::forge_snapshot_emit`。
- **agent**：无变更；锻造材料请求没有 agent schema。
- **client**：无变更；客户端已有材料快照，只需接受 server 正确库存结果。
- **worldview 锚点**：`docs/worldview.md §八` 锻造与资源损耗。

## 阶段总览

| 阶段 | 状态 | 交付物 |
| --- | --- | --- |
| P0 | ⬜ | 按每个 optional stack 的配方需求封顶，超量留在背包并保持失败语义 |
| P1 | ⬜ | 成功、失败、重复请求和多 carrier 数量回归测试 |

## 来源 issue

- #1362 `[flash-review][major] 可选载体材料整堆投料过量扣减销毁资源`
