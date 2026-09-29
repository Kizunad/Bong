# plan-bughunt-forge-failure-tier-gate-v1

> 骨架：来源 #1830。淬炼失败先降到 tier 1，但后续铭文/开光仍以 max(3/4) 写回。

## §0 摘要

`server/src/forge/steps.rs:293-321` 失败路径把 tier 限为 1，随后成功的 inscription/consecration 分支又在 `max(3/4)` 处提升结果，没有携带“失败已锁定”的 gate。失败淬炼因而可以绕过品阶门槛。

## §1 游玩影响

失败锻造仍能产出高阶物品，破坏 tier 经济、失败风险和配方平衡；玩家会反复利用失败路径刷品质。

## §2 复现路径

1. 选择会触发 tempering failure 的高 tier 图谱。
2. 观察 tier 被设为 1 后继续完成 inscription/consecration。
3. 核对结果 `achieved_tier` 被 `max(3/4)` 抬高。

## §3 今天 `origin/main` 根因证据

- `forge/steps.rs:293-321` 失败只改变当前 tier 数值，没有写入不可提升的失败状态。
- 后续分支仍以 `max(3)`/`max(4)` 计算结果，未检查失败标记。
- `ForgeOutcomeEvent.achieved_tier` 因此与失败契约矛盾。

## §4 非重复比对

`plan-forge-v1` 定义各步骤，`plan-bughunt-forge-session-abort-cleanup-v1` 处理会话回收；本骨架只处理失败 tier gate 与结果表达。

## §5 立项检查记录

本记录已按 `docs/worldview.md`、`docs/finished_plans/`、`docs/plan-*.md`、`docs/plans-skeleton/` 与 `docs/plans-skeleton/reminder.md` 逐项检索。

- **worldview**：查“锻造失败、品阶、铭文、开光”；`worldview.md §八` 要求失败有代价。
- **finished_plans**：查 `ForgeBucket`、`achieved_tier`、tempering/inscription/consecration；无失败锁定字段。
- **active plan**：查 forge steps；无同一 tier gate 修复。
- **skeleton**：查 `failure tier`、`max(3)`、`tempering`；无重复骨架。
- **reminder.md**：查 `forge`、`tier`、`failure`；仓内无对应条目。

## §6 接入面与跨仓契约

- **Inputs**：tempering outcome、inscription/consecration inputs、blueprint tier cap。
- **Outputs**：`ForgeOutcomeEvent.achieved_tier`、物品模板 tier、snapshot。
- **共享类型或事件**：复用 `ForgeBucket`、`ForgeOutcomeEvent`、blueprint tier fields；不另造平行 tier。
- **server 符号**：`forge::steps::{resolve_tempering,resolve_inscription,resolve_consecration}`、`ForgeOutcomeEvent`。
- **agent**：无变更；锻造结果不经 agent。
- **client**：无变更；客户端显示结果 payload 中的 tier。
- **worldview 锚点**：`docs/worldview.md §八` 锻造品阶与失败代价。

## 阶段总览

| 阶段 | 状态 | 交付物 |
| --- | --- | --- |
| P0 | ⬜ | 以失败锁定状态约束后续步骤与最终 tier |
| P1 | ⬜ | 各失败阶段、成功阶段、tier cap 和 snapshot 回归测试 |

## 来源 issue

- #1830 `[flash-review][major] 淬炼失败未锁凡器`
