# plan-bughunt-alchemy-intervention-qi-ledger-v1

## §0 摘要

**来源 Issue：#1725。** 验真基线 `origin/main@7496457d0` 仍显示 C2S 炼丹注灵请求在 `handle_alchemy_intervention` 中直接调用 `AlchemySession::apply_intervention(InjectQi(q))`；该方法只把数值累加到 `session.qi_injected`，不读取玩家 `Cultivation`、不检查余额、也不产生 `QiTransfer`。本计划已落地守恒账本、玩家付款事务、取消/断线收口和回归契约测试。

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

### P0：注灵付款事务 ✅ 2026-09-29

- 在网络 handler 或专用 alchemy service 中先验证 `InjectQi(q)` 为有限正数、玩家 `Cultivation.qi_current` 足够、炉体/session 可接受，再调用 `qi_physics::ledger::transfer_external_qi_to_ledger(&mut ledger, QiAccountId::player(player_id), QiAccountId::container(furnace_id), q, QiTransferReason::Crafting)`；该 helper 成功后才扣 `Cultivation.qi_current`、更新 session canonical balance，真实消费者必须更新余额而非只发事件。
- 炉体结算读取已付款的 canonical balance；重复请求、拒绝请求、断线/炉体销毁必须幂等退款或回灌 zone/overflow，不能靠客户端 q 值造余额。
- 复用 `qi_physics` 的单位、overflow 与 `QiTransferReason`，不要在 session.rs 自定义第二套真元 ledger。

### P1：回归契约 ✅ 2026-09-29

- 足额注灵、余额不足、负数/NaN、collapsed zone、非炉主和重复请求分别断言玩家 qi、session 注入量、zone/overflow、事件和结算门。
- 炉体成功产丹时验证总量守恒；取消/过期/断线时验证未消费注资有明确去向。
- 现有 `AlchemySession` 纯逻辑单测继续只测状态机，需新增 handler/ledger 契约而不是把付款逻辑藏进 fixture。

## §6 验证计划 ✅ 2026-09-29

运行 server 栈 fmt、clippy、cargo test，并覆盖 alchemy 网络 handler、session 结算和 qi ledger；完整门禁已通过。

## §7 跨仓契约与可核验锚点

- **Inputs：** `handle_alchemy_intervention` 收到 `Intervention::InjectQi(q)`、炉体位置/所有权、`AlchemyFurnace.session`、玩家 `Cultivation.qi_current` 与 `WorldQiAccount`。
- **Outputs：** 付款成功才增加 `AlchemySession.qi_injected` 和炉体账户；拒绝不改状态，取消/过期余额经 zone/overflow 回灌。
- **共享类型/事件：** `Intervention`、`AlchemySession`、`AlchemyFurnace`、`QiAccountId`、`QiTransfer`、`QiTransferReason::Crafting`、`WorldQiAccount`；server 入口/结算符号为 `handle_alchemy_intervention`、`apply_intervention`、`summarize_with_alchemy_effective_lv`。
- **三端契约符号：** Server 使用上述 handler/session/ledger；Agent：无变更，理由是只继续消费既有 alchemy snapshot；Client：无变更，理由是请求、炉体 snapshot 与 VFX payload 不增字段。
- **Qi：** 在线玩家不是 ledger player 余额，必须调用 `transfer_external_qi_to_ledger(&mut ledger, from, to, amount, reason)`；helper 内部才以 `ledger.transfer(QiTransfer { from, to, amount, reason })` 临时镜像外部 source。zone 回灌用 `qi_release_to_zone`/`QiTransferReason::ReleaseToZone`/`QI_ZONE_UNIT_CAPACITY`，断言用 `qi_physics::ledger::assert_conservation`、`QI_EPSILON` 与 `crate::schema::common::SPIRIT_QI_TOTAL`。
- **worldview 锚点：** `docs/worldview.md` §十的真元零和与炼丹生产链；session 数值不能替代真实付款。

## Finish Evidence

### 落地清单

- P0：`server/src/alchemy/qi.rs` 提供 `debit_player_qi_to_furnace`、`refund_furnace_qi_to_player`、`release_furnace_qi_to_overflow` 和 `AlchemyQiReservationBook`；`server/src/network/client_request_handler.rs` 的 `settle_alchemy_inject_qi_requests` 在 ledger 成功后才提交 `Cultivation.qi_current` 与 session，取丹、炉体移除、断线和关服均有明确调度收口。
- P1：`server/src/alchemy/qi.rs` 的守恒测试引用 `SPIRIT_QI_TOTAL`、`summarize_world_qi` 和 `assert_conservation`，覆盖足额付款、余额不足、容量不足退款/overflow、同帧注灵提交、断线退款、炉体移除退款和关服转入持久化 overflow；网络契约测试覆盖坍缩区、非炉主、无 session、注灵后取丹顺序。

### 关键 commit

- `e4398b5c9`（2026-09-29）：推进炼丹注灵真元守恒修复计划。
- `620c65e74`（2026-09-29）：接入玩家到炉体的 ledger 转移、退款、清理和契约测试。
- `13d1c2080`（2026-09-29）：改用 `EventReader<AppExit>`，确保关服事件在 Last 阶段可见且无关服请求时不动账本。

### 测试结果

- `scripts/build-token.sh cargo fmt --check`：PASS。
- `scripts/build-token.sh cargo clippy --all-targets -- -D warnings`：PASS。
- `scripts/build-token.sh cargo test alchemy_ --lib`：71 passed，1 ignored。
- `scripts/build-token.sh cargo test`：PASS；lib 10425 个测试及全部集成测试、文档测试无失败。

### 跨仓库核验

- Server：`Cultivation.qi_current`、`AlchemySession.qi_reserved`、`WorldQiAccount`、`QiTransferReason::Crafting`、`assert_conservation`。
- Agent：无 schema 或 Redis 契约变更，继续消费既有炼丹 session snapshot。
- Client：无请求或 snapshot 字段变更，既有 `InjectQi` 请求仍由 server 侧扣款后才生效。

### 遗留 / 后续

- `FurnaceQiReserve` 的 AutoProfile 路径继续是炉体自有储量，不计入玩家可退款余额；BlockEntity 炉体持久化仍由既有 plan 负责。本计划范围内所有玩家注灵余额在成功结算、取消、断线、炉体移除和关服路径都有 ledger 去向。
