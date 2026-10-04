# plan-bughunt-zhenfa-sealed-qi-release-v1

> 骨架：来源 #1837、#1883。拆阵/缓阵移除时密封真元释放链存在重复/虚假入账风险。

## §0 摘要

`server/src/zhenfa/mod.rs:4331-4417` 的释放 helper 计算 zone capacity 后调用 `qi_release_to_zone`，同时在 `outcome.overflow` 上调用 `send_zhenfa_release_overflow`；拆阵/缓阵耗尽路径未证明 sealed account 余额已先通过 ledger 扣减，可能只发事件或重复记 overflow。两个 issue 共同指向“密封真元没有单一权威账户”。

## §1 游玩影响

拆除阵法时玩家看不到被封真元回到区域，或同一笔在 zone/overflow 记两次；长期运行会凭空增加/减少灵气。

## §2 复现路径

1. 创建带 sealed qi 的 zhenfa，触发 disarm 或 slow removal/耗尽。
2. 记录 sealed account、zone 与 overflow 的余额变化及 transfer events。
3. 观察 helper 只 emit event、或 outcome overflow 又被外部再次入账，导致总量不守恒。

## §3 今天 `origin/main` 根因证据

- `zhenfa/mod.rs:4331-4385` 缺 zone 时直接 `send_zhenfa_release_overflow`。
- `:4386-4417` 调 `qi_release_to_zone`，对 outcome transfer/overflow 分别发事件，但没有显式从 `zhenfa_sealed_qi_account` 扣 ledger 余额。
- `:4420-4430` 定义 sealed account，调用链未形成唯一 debit→credit 事务。

## §4 非重复比对

`plan-bughunt-qi-ledger-asymmetry-v1` 处理通用 overflow；本骨架拥有 zhenfa sealed account 与 disarm/slow-removal 生命周期，不把其他阵法问题混入。

## §5 立项检查记录

本记录已按 `docs/worldview.md`、`docs/finished_plans/`、`docs/plan-*.md`、`docs/plans-skeleton/` 与 `docs/plans-skeleton/reminder.md` 逐项检索。

- **worldview**：查“阵法、封印、拆阵、真元归还”；`worldview.md §十` 要求阵法撤除不销毁灵气。
- **finished_plans**：查 `zhenfa_sealed_qi_account`、`qi_release_to_zone`、disarm；未见 sealed account 的真实 debit 契约。
- **active plan**：查 zhenfa release/qi physics；无同一调用链修复。
- **skeleton**：查 `zhenfa sealed qi`、`disarm`、`overflow`；无重复骨架。
- **reminder.md**：查 `zhenfa`、`sealed qi`、`release`；仓内无对应条目。

## §6 接入面与跨仓契约

- **Inputs**：sealed amount、`zhenfa_sealed_qi_account`、owner identity、zone/overflow availability、disarm event。
- **Outputs**：一次 `QiTransfer { from, to, amount, reason }`、zone field/ledger 或稳定 overflow、阵法移除状态。
- **共享类型或事件**：必须核对并复用 `WorldQiAccount::transfer`、`transfer_external_qi_to_ledger`、`transfer_ledger_qi_to_zone`/`qi_release_to_zone`；原因 `QiTransferReason::ReleaseToZone`；使用 `qi_physics::ledger::assert_conservation`，测试引用 `schema::common::SPIRIT_QI_TOTAL`（不是 `DEFAULT_SPIRIT_QI_TOTAL`）。调用方只审计返回值，不能二次入账。
- **server 符号**：`zhenfa::{disarm,remove_slow,release_sealed_qi,zhenfa_sealed_qi_account}`、`qi_physics::ledger`。
- **agent**：无变更；阵法账本不在 agent schema。
- **client**：无变更；阵法 VFX/状态 payload 不需改 wire。
- **worldview 锚点**：`docs/worldview.md §十` 阵法封印与真元守恒。

## 阶段总览

| 阶段 | 状态 | 交付物 |
| --- | --- | --- |
| P0 | ⬜ | 以 sealed ledger account 为 from，统一 zone/overflow 单次转账和失败回滚 |
| P1 | ⬜ | disarm、slow removal、满区、无区、重复事件与守恒回归测试 |

## 来源 issue

- #1837 `[flash-review][major] 缓阵充能耗尽移除时不归还封印真元`
- #1883 `[flash-review][major] 拆阵/强拆不返还密封真元`
