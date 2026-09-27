# plan-bughunt-alchemy-intervention-qi-ledger-v1

## §0 摘要

**来源 Issue：#1725。** C2S 炼丹注灵请求在 `handle_alchemy_intervention` 中直接调用 `AlchemySession::apply_intervention(InjectQi(q))`；该方法只把数值累加到 `session.qi_injected`，不读取玩家 `Cultivation`、不检查余额、也不产生 `QiTransfer`。炼丹配方只在结算时用 `qi_injected` 与 `recipe.fire_profile.qi_cost` 比较，因此客户端可免费注入任意数量真元并满足火候门槛。本 skeleton 不改生产代码。

接入面：进料是玩家 C2S `alchemy_intervention`、所属 `AlchemyFurnace`/`AlchemySession`、玩家 `Cultivation` 和当前 zone；出料是玩家扣减、炉体/炼丹 session 的注入余额、zone/overflow 与结算结果。复用 `QiTransfer`、`qi_release_to_zone`/炉体 qi reserve 的既有边界，不能在 `AlchemySession` 内另造物理公式。server 内部修复不需要 agent/client schema 字段变化；worldview §十的真元零和与炼丹生产链是锚点。

## §1 游玩影响

- 玩家可以在没有真元成本的情况下把任意炼丹 session 推过 `qi_cost` 门槛，领取应由真元支付的丹药收益。
- 重复注灵会让炼丹收益脱离玩家和 zone 资源，形成可刷的生产经济漏洞；当前日志/快照只显示增加后的 `qi_injected`，不显示付款失败。
- 生产可达：正常拥有自己的炼丹炉、起炉后发送 `InjectQi` 即可触发。

## §2 复现路径

1. 玩家起炉并拥有一个需要 `recipe.fire_profile.qi_cost` 的配方。
2. 发送 `alchemy_intervention` 的 `InjectQi(q)`，即使玩家真元不足或 q 为任意大值。
3. `handle_alchemy_intervention` 通过炉主检查后直接 `session.apply_intervention`；`AlchemySession` 累加 q 并发布成功快照。
4. 结算时 `session.qi_injected + 1e-9 < qi_cost` 只检查 session 字段，免费注入即可通过。

## §3 根因证据

- `server/src/network/client_request_handler.rs:5304-5339` 的 `handle_alchemy_intervention` 只做 collapsed-zone、session 存在和炉主校验，`InjectQi` 没有 `Cultivation`/Position/CurrentDimension/ledger 参数，随后直接 `session.apply_intervention(intervention.clone())`。
- `server/src/alchemy/session.rs:25-31, 99-105` 的 `Intervention::InjectQi(f64)` 与 `apply_intervention` 只执行 `self.qi_injected += q.max(0.0)`，没有真元扣减或 transfer。
- `server/src/alchemy/session.rs:200-210` 的结算门只比较 `qi_injected` 与 recipe `qi_cost`；这把 session 统计量当成已付款余额。
- 当前 `server/src/alchemy` 与 `client_request_handler` 的注灵路径没有 `QiTransfer`/`Cultivation.qi_current` 命中；已有 `forge` 结算（`server/src/forge/mod.rs:604-674`）则先校验/扣款并写 transfer，说明炼丹是漏接。
- `server/src/alchemy/auto_profile.rs` 使用独立 `FurnaceQiReserve` 的后台路径，应在实现时明确它与玩家 C2S 付款的边界；本 skeleton 的 blocker 只针对 `handle_alchemy_intervention`，不把两种资源池混为一谈。

## §4 非重复比对

- `docs/finished_plans/plan-alchemy-v1.md` 定义 session 与配方火候，不代表玩家注灵已经接入 qi ledger。
- `plan-bughunt-yidao-treatment-qi-destination-v1` 处理医道技能扣款后的去向；本 skeleton 的 producer 是炼丹炉 C2S 注入，文件和结算所有权不同。
- `plan-bughunt-combat-qi-max-shrink-ledger-v1` 处理 qi_max 缩容，不处理 alchemy session 的免费注资。

## §5 修复计划骨架

### P0：注灵付款事务

- 在网络 handler 或专用 alchemy service 中先验证 `InjectQi(q)` 为有限正数、玩家余额足够、炉体/session 可接受，再一次性扣玩家并把金额转入炉体/炼丹账户；真实消费者必须更新余额而非只发事件。
- 炉体结算读取已付款的 canonical balance；重复请求、拒绝请求、断线/炉体销毁必须幂等退款或回灌 zone/overflow，不能靠客户端 q 值造余额。
- 复用 `qi_physics` 的单位、overflow 与 `QiTransferReason`，不要在 session.rs 自定义第二套真元 ledger。

### P1：回归契约

- 足额注灵、余额不足、负数/NaN、collapsed zone、非炉主和重复请求分别断言玩家 qi、session 注入量、zone/overflow、事件和结算门。
- 炉体成功产丹时验证总量守恒；取消/过期/断线时验证未消费注资有明确去向。
- 现有 `AlchemySession` 纯逻辑单测继续只测状态机，需新增 handler/ledger 契约而不是把付款逻辑藏进 fixture。

## §6 验证计划

实现后运行 server 栈 fmt、clippy、cargo test，并覆盖 alchemy 网络 handler、session 结算和 qi ledger。此 docs-only skeleton 阶段不编译。
