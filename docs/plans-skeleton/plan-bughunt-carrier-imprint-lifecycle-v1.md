# plan-bughunt-carrier-imprint-lifecycle-v1

## §0 摘要

**来源 Issue：#1434、#1359。** 暗器 v2 施放只检查 `CarrierImprint.qi_amount > EPSILON`，随后从玩家 `qi_current` 计算技能成本和 payload，却不减少被选中的 imprint；同一枚封骨因此可无限施放。与此同时，充能事务保存了 `CarrierCharging.instance_id`，但 `finish_charge` 只按 slot 读取当前物品，换装后会把新物品改成充能骨并把 imprint 写给旧 instance。两者都是 carrier/imprint 生命周期与物品身份脱节的根因，本骨架不改代码。

接入面：进料是 `CarrierStore.imprints_by_instance`、`PlayerInventory.equipped`、`CarrierCharging` 与 `AnqiSkillId`；出料是消耗后的 imprint、正确 instance 的充能物品、QiTransfer/拒绝结果。复用 `carrier_qi_account`、`CarrierImprint`、`CarrierCharging` 和现有 `QiTransfer`，agent/client 事件格式不必改变。worldview §四/§十的物品承载真元规则要求一个 instance 只有一个真实余额。

## §1 游玩影响

- 玩家充能一次即可重复使用暗器技能，弹药库存与真元投入不相称；这是普通战斗中的可重复刷资源路径。
- 充能期间换手/换装备会静默毁掉原本的剑、工具等物品，新物品被改成无法对应旧 imprint 的充能骨，且已扣真元难以追踪。
- 客户端看到的是正常 cast/charge 事件，无法从反馈发现 instance 错配。

## §2 复现路径

### 施放不消耗 imprint

1. 充能一枚手槽载具，使 `CarrierStore.imprints_by_instance[instance_id].qi_amount > 0`。
2. 在 `AnqiSkillId::SingleSnipe` 等技能中反复调用 `resolve_anqi_skill`。
3. `has_loaded_carrier` 只读余额，`draw_payload_after_abrasion` 只计算容器磨损；施法结束后检查 imprint，`qi_amount` 未减少。

### 充能中途换装

1. `begin_charge_carrier` 记录槽位和旧 `item.instance_id`。
2. 充能未完成前把该 slot 换成另一件物品。
3. 到时或移动中断进入 `finish_charge`，它按 slot 当前 item 的 template 推导 kind，未先校验 instance_id，就调用 `transform_equipped_item`。

## §3 根因证据

- `server/src/combat/anqi_v2.rs:420-515` 在 `has_loaded_carrier` 通过后扣玩家 qi、发送 Channeling transfer、调用 `draw_payload_after_abrasion`；该函数（约 `536-560`）没有取得或更新 `CarrierStore.imprints_by_instance` 的 `qi_amount`。
- `server/src/combat/anqi_v2.rs:850-898` 的 `has_loaded_carrier`/`imprint_matches_skill` 只要求余额大于 epsilon，不能证明施放消耗了该余额；全仓 imprint 写入主要来自充能与自然衰减。
- `server/src/combat/carrier.rs:453-476` 的 `begin_charge_carrier` 把 `slot`、`item.instance_id` 写入 `CarrierCharging`。
- `server/src/combat/carrier.rs:584-685` 的 `finish_charge` 用 `charging.slot` 读取当前 held item 并调用 `transform_equipped_item`，但没有比较当前 `item.instance_id == charging.instance_id`；随后无条件以旧 id 插入 `CarrierImprint`。
- 对照 `server/src/combat/carrier.rs:803-835` 的投掷路径会按 instance 移除 imprint，证明“发射消耗封存真元”是预期生命周期，而 anqi v2 技能路径漏接了同一语义。

## §4 非重复比对

- `plan-bughunt-carrier-projectile-miss-qi-ledger-v1` 处理投射物 despawn 后 payload 释放；本骨架处理投掷前的 imprint 消耗和充能身份校验。
- `docs/plans-skeleton/plan-bughunt-anqi-throw-imprint-drop-v1.md`（来源 #1529）只处理投掷失败时过早移除 imprint/退款边界；本骨架覆盖成功施放的扣减以及充能中途换装，不能合并。
- `plan-bughunt-tuike-v2-state-transaction-v1` 的伪皮状态事务与 carrier item instance 没有共享状态。

## §5 修复计划骨架

### P0：instance 绑定与余额消费

- 在技能成功提交前锁定具体 carrier instance，并按实际 payload/技能语义扣减其 `qi_amount`；不足时拒绝，不得只扣玩家 qi 或用事件假装转移。
- `finish_charge` 在任何物品改写前核验当前 slot instance 与 `CarrierCharging.instance_id`；不匹配时取消充能、按既有预付/未密封规则释放真元，不改写新物品、不遗留旧 imprint。

### P1：回归契约

- 同一 imprint 连续施放直到余额耗尽：每次可观察余额递减，耗尽后拒绝；不同载具类型与磨损仍按原公式。
- 充能中途换装、换回原物品、移动中断三组均断言物品 instance、imprint、qi_current 和释放事件一致。
- 成功充能只为匹配 instance 创建 imprint，不能以 slot 位置作为身份。

## §6 验证计划

实现后运行 server 栈 fmt、clippy、cargo test，重点覆盖 `anqi_v2` 与 `carrier` 生命周期测试。账本断言使用 `QiTransfer`/生产常量；本 skeleton 阶段不编译。
