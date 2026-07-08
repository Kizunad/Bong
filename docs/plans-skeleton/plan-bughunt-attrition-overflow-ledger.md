# BugHunt Skeleton: attrition overflow 未落 WorldQiAccount

日期：2026-07-06

线程：BugHunt A2 server-qi 第二轮

范围：server Rust 真元/灵气守恒、`qi_physics` ledger、灵物操作磨损

重复性排除：已查开放 PR；本 bug 不重复 #975 `release_dormant_qi_to_zone` 负灵域 `.max(0.0)` 问题。本问题与负灵域无关，触发点是灵物 attrition 在正灵域 zone 接近或达到满仓时的 overflow 真实余额缺失。

## Bug 摘要

`server/src/qi_physics/attrition.rs` 的 `apply_attrition_checked` 会先扣 `ItemInstance.spirit_quality`，再调用 `release_attrition_to_zone` 将磨损量归还 zone。zone 未满时，accepted 部分会写入 `zone.spirit_qi`；但 zone 接近满仓或满仓时，`outcome.overflow` 只被构造成 `QiTransfer(to=overflow:attrition_overflow:<zone>, reason=AttritionTax)` 并 `events.send(t)`，没有进入 `WorldQiAccount`。

本仓 `test_coverage_guards.rs:100-103` 明确声明 `QiTransfer` 是守恒审计事件，真实余额由 `WorldQiAccount` 或调用点直接 apply，不能要求 `EventReader` 消费。因此现状是：物品真元质量真实减少，zone 只接收可容纳部分，溢出部分只留在无余额消费者的事件里，实际世界状态少掉这部分真元。

## 实际游玩体验影响

玩家在高灵气/接近满仓区域反复拾取、移动、搜刮或炼丹加料高灵气物品时，物品会按天道税降低 `spirit_quality`，但 zone 已满无法接纳的那部分真元不会进入任何真实账户。玩家看到的是灵物品质正常损耗，服务器账本却把未接收的损耗吞掉。

这会让高资源区的灵物搬运比设计更亏：不是“倒腾越频繁，真元回到环境或 overflow 账户”，而是“倒腾越频繁，真元从世界预算中消失”。长期看会扭曲灵物经济、区域灵气 telemetry 和后续守恒审计，尤其影响批量拾取、TSY 容器搜刮、炼丹材料搬入等高频操作。

## 证据定位

- 历史规格要求所有磨损量必须归还 zone，不允许凭空消失：`docs/finished_plans/plan-qi-handling-attrition-v1.md:29-40`。
- 历史 P0 任务明确要求 `ledger.transfer(AttritionTax)`，并要求 `WorldQiAccount` 增量等于 item 减量：`docs/finished_plans/plan-qi-handling-attrition-v1.md:63-67`。
- 当前 helper 注释承诺 `accepted + overflow == amount` 且 overflow 进入 `overflow_account`：`server/src/qi_physics/attrition.rs:153-162`。
- 当前 helper 只有 `Events<QiTransfer>` 参数，没有 `WorldQiAccount` 参数：`server/src/qi_physics/attrition.rs:162-168`。
- 当前实现先写 `zone.spirit_qi`，随后 overflow 只 `events.send(t)`：`server/src/qi_physics/attrition.rs:184-211`。
- 当前 `apply_attrition_checked` 在落账前已真实扣减 `item.spirit_quality`：`server/src/qi_physics/attrition.rs:285-295`。
- 现有 overflow 单测只把 `QiTransfer events` 合计当守恒锁，没有断言 `WorldQiAccount` 余额：`server/src/qi_physics/attrition.rs:1205-1278`。
- `QiTransfer` 白名单说明真实余额必须由 `WorldQiAccount` 或调用点直接 apply：`server/src/test_coverage_guards.rs:99-103`。

## 触发路径

- 背包槽位移动：`server/src/network/client_request_handler.rs:11170-11184` 调用 `apply_attrition_checked(... AttritionOpKind::SlotMove ...)`。
- 拾取地面物品：`server/src/network/client_request_handler.rs:11708-11726` 调用 `apply_attrition_checked(... AttritionOpKind::Pickup ...)`。
- 炼丹加料：`server/src/network/client_request_handler.rs:12271-12290` 调用 `apply_attrition_checked(... AttritionOpKind::AlchemyLoad ...)`。
- TSY 容器搜刮：`server/src/world/tsy_container_search.rs:260-279` 调用 `apply_attrition_checked(... AttritionOpKind::ContainerSearch ...)`。

最小复现形态：

1. zone `spirit_qi=0.9999` 或 `1.0`。
2. 操作 `spirit_quality=1.0, stack_count=100` 的灵物。
3. attrition 约 3.0 绝对真元；zone room 仅约 0.005 或 0。
4. item 降质约 3.0；zone 只接收 room；剩余 overflow 只发 `QiTransfer` event。
5. `WorldQiAccount` 中没有 `overflow:attrition_overflow:<zone>` 的真实余额。

## 反方审查记录

第一轮反方结论：通过。未发现 `AttritionTax` 或 `QiTransfer` 事件消费者会把 overflow 入账；调用点只传 `Events<QiTransfer>`，没有同步 `WorldQiAccount`；现有测试以 event 合计证明守恒，正好踩中 `QiTransfer` 只是审计事件的白名单语义。最强反驳是“QiTransfer event 本身就是审计轨迹，所以只发 event 也许是设计选择”，但不成立，因为本仓 guard 明确 event 不能承担真实余额。

第二轮反方结论：继续通过，但修复计划必须收窄账本语义。`AttritionTax` 不应改成 audit-only；裸 `WorldQiAccount::transfer(from=container:item:<id>)` 会因源账户没有余额失败，必须使用临时影子源余额。accepted 部分不能在已写 `zone.spirit_qi` 后再盲目加 ledger，必须采用 field-authority 镜像范式，先同步 zone ledger before，再 transfer，最后从 ledger balance 写回 field。overflow 不应改成“不扣 item”，因为历史玩法就是 inventory 操作扣 1-5% 天道税；满仓不是免税条件。

## Skeleton Fix Plan

- [ ] 改造 attrition 落账 API：`release_attrition_to_zone` 或其上层返回结构必须携带 `accepted`、`overflow`、`from_id`、`zone_account`，并允许生产调用点传入 `WorldQiAccount`。
- [ ] 不新增 `QiTransfer EventReader`。`QiTransfer` 继续是审计/外部可视化事件，真实余额由 helper 或调用点同步 apply。
- [ ] 对 `AttritionTax` 使用真实 `WorldQiAccount::transfer`，不要加入 audit-only 拦截名单。
- [ ] 使用源影子余额范式：对 `container:item:<instance_id>` 临时 `set_balance(from, accepted + overflow)`，完成 transfer 后源账户余额必须归零，不跨 tick 留存。
- [ ] 使用 zone field-authority 镜像范式：在 accepted transfer 前，把 `zone:<name>` 账本镜像同步到变更前的 `zone.spirit_qi * QI_ZONE_UNIT_CAPACITY`；transfer 后以 ledger balance 写回 `zone.spirit_qi`。参考 `server/src/world/heartbeat.rs:2131-2159` 和 `server/src/world/pseudo_vein_runtime.rs:487-502`。
- [ ] overflow 继续落到 `QiAccountId::overflow("attrition_overflow:<zone>")`，不要改投 `pending_inflow_account`，避免丢失局部性并被 heartbeat 滴灌到其它 zone。
- [ ] 如果 `WorldQiAccount` 缺失且本次会产生 overflow，必须 fail-closed：不要扣 item 后只发 event。生产路径应保证 ledger resource 存在。
- [ ] 保留 `Events<QiTransfer>` 兼容，但 event 应来自同一笔 ledger transfer 的 clone；不要 `transfer()` 后再 `push_transfer_audit()` 造成审计重复。

## 验收测试计划

- [ ] helper 单测：`zone.spirit_qi=1.0` 时，attrition 后 `overflow:attrition_overflow:<zone>` balance 等于 item lost，zone 不变，source item ledger balance 为 0。
- [ ] near-full 单测：`zone.spirit_qi=0.99` 且 attrition 大于 room 时，断言 zone 到 1.0，`zone:<name>` ledger mirror 到 cap，overflow balance 等于 `item_lost - accepted`。
- [ ] 生产路径单测至少覆盖 Pickup 或 ContainerSearch，证明真实入口不是只在 helper 里绿。
- [ ] repeated attrition 单测：同一 zone 多次 overflow 累加，不覆盖旧 balance。
- [ ] 缺 `WorldQiAccount` 边界：会 overflow 时不扣 item、不只发 event；不会 overflow 且可全额写 zone 时行为明确并有测试锁定。
- [ ] 审计兼容：`QiTransfer` event 合计仍等于 item lost，但测试主断言必须查具体 `WorldQiAccount` 账户余额，而不是只看 event 合计。

建议 server 验证命令：

```bash
cd server
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

## 风险

- 双记风险：`summarize_world_qi` 同时统计原始 `zone_qi` 和 `ledger_qi`，测试不能盲用 `total_observed()` 判定 attrition 守恒，应断言具体账户余额。
- 账本覆盖风险：`zone:<name>` ledger 账户是 field mirror，必须先同步 before 再 transfer 后写回 field；只手写余额会被后续系统覆盖或形成陈旧镜像。
- 源账户失败风险：`WorldQiAccount::transfer` 会检查 from 余额；`container:item:<id>` 必须临时引燃源余额，否则 transfer 会失败。
- 行为风险：把满仓 overflow 改成“不扣 item”会改变历史玩法意图，使满仓区域变成免税搬运区，不建议。
- 架构风险：新增 `QiTransfer EventReader` 会违反当前 `test_coverage_guards` 对 `QiTransfer` 的设计语义。
