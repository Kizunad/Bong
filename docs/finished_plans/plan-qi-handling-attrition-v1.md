# Bong · plan-qi-handling-attrition-v1 · active

**灵气操作磨损**（"零拷贝生存"实装）——把 worldview §八.2「灵物操作磨损」正典化为服务端机制：任何携带 `qi_value > 0` 的物品一旦触发 inventory 操作（拿起/转移/搜刮箱子），立即损耗 1-5% 灵气纯度，超额部分以 `QiTransfer(AttritionTax)` 逸散归还 zone，同时向客户端推送 `qi_decay_flash` 粒子反馈。核心设计意图：天道对"搬运灵物者"的隐性交易税；迫使玩家做极度节制的资源规划，实现"零拷贝生存"博弈。

## 目标

- 实装 worldview §八.2 灵物操作磨损：`inventory_op → qi_attrition(1-5%)` 守恒律接入
- 工程目标：通过 `InventoryItemMoved` / `ItemPickedUp` Bevy event hook，对 `qi_value > 0` 物品施加衰减，走 `qi_physics::ledger` 归还 zone，绝对禁止"真元凭空消失"
- 玩家感知：每次操作高灵气物品都有短促粒子闪烁（不是数字 HUD，是物品上短暂的"灵气溢出"视觉提示）
- 验收：拾起灵石 → 灵气值 ×0.97（默认 3%）→ 逸散真元进 zone `WorldQiAccount` → 守恒律单测全通过

**来源**：`worldview.md §八.2 灵物操作磨损`（原文："带有灵气值的物品只要被从箱子里拿出来，或从背包转移到炉子里（触发 inventory 操作），灵气纯度/耐久度就会固定扣除 1%-5%。天道的'交易税'——你越频繁倒腾箱子，身家缩水越快。"）

**前置条件**：
- `plan-qi-physics-v1` ✅ — `QiTransfer` + `WorldQiAccount` + ledger 守恒律 API
- `plan-qi-physics-patch-v1` ✅ — 各物品的 `qi_value` 字段规范化
- `plan-inventory-v2` ✅ — inventory 模块 + `ItemSlot` component + 物品移动事件链路（确认 `InventoryItemMoved` event 已存在）
- `plan-shelflife-v1` ✅ — 时间衰减（本 plan 是操作触发衰减，两者**叠加**不互斥；shelflife 管时间、本 plan 管搬运）
- `plan-alchemy-v1` ✅ — 丹药 `qi_value` 结构（需确认丹药操作也走本 plan hook）

**交叉引用**：`plan-craft-v1` ✅（工作台配方操作：原材料拆装是否也触发磨损——建议是，但有配方"封灵容器"可豁免）· `plan-food-v1` ⬜（骨架，灵果取出操作需走本 plan）· `plan-tsy-v1` ✅（坍缩渊内操作磨损 ×3，与 shelflife 的 ×3 倍率保持一致）

**worldview 锚点**：
- **§八.2:806 灵物操作磨损**（原文）：每次 inventory 操作扣 1-5%；越频繁倒腾越亏；天道"交易税"
- **§十.877 资源与匮乏 · 灵气搜刮**：每次提取灵草都"感觉到灵气流失了一丝"——与本 plan 的粒子反馈对应
- **§八.3:812 气运劫持**：操作磨损是天道"隐性"手段的物理基础；高频交易者比低频者更快被盯上
- **§十六.1481 坍缩渊容器搜刮**：秘境内搜刮磨损率更高（与 shelflife ×3 共振）

**qi_physics 锚点**：
- 所有磨损量**必须**归还 zone，不允许凭空消失：`qi_physics::ledger::QiTransfer { from: item_qi, to: zone, amount: attrition_amount, reason: AttritionTax { op_kind } }`
- 磨损公式：`attrition_rate = BASE_RATE × environment_multiplier`（`BASE_RATE = 0.03`，死域 ×3，坍缩渊 ×3，馈赠区 ×0.8）；数值见 §8 开放问题
- `qi_value > 0` 判定在物品的 `ItemQiValue { current, max }` component 上；`< 0.5` 的零散真元不触发磨损（避免频繁小额转账）

---

## 接入面 Checklist

- **进料**：`InventoryItemMoved { item: Entity, from_slot, to_slot, operator: Entity }` Bevy event（inventory-v2 已有，需确认字段）+ `ItemQiValue { current, max }` component（qi-physics-patch-v1 已规范化）+ `ZoneQiDensity` resource（当前 zone 灵气浓度，决定 environment_multiplier）
- **出料**：`AttritionAppliedEvent { item, amount_lost, returned_to_zone }` Bevy event（供 client VFX 系统订阅）+ `item.qi_value` 减少 + zone `WorldQiAccount` 增加（保持守恒）
- **共享类型**：新增 `AttritionTax { op_kind: AttritionOpKind }` 作为 `QiTransferReason` 变体；`AttritionOpKind { Pickup, SlotMove, ContainerSearch, ForgeLoad, AlchemyLoad }` 各有不同的 BASE_RATE 微调（搜刮 > 移动 > 拾起）；新增 `AttritionExemptTag` component（封灵容器内物品豁免）
- **跨仓库契约**：client `bong:vfx/qi_attrition` CustomPayload（item entity id + amount + zone position）→ client `QiAttritionVfxHandler` 在物品位置产生短促粒子闪光（不走 HUD，走世界粒子）；agent 不需要感知此事件（过于细粒度）
- **worldview 锚点**：§八.2 灵物操作磨损
- **qi_physics 锚点**：`QiTransfer(AttritionTax)` 归还 zone；BASE_RATE 常数归 `qi_physics/constants.rs` 统一管理

---

## 阶段总览

| 阶段 | 状态 | 主要交付物 | 验收标准 |
|------|------|-----------|---------|
| **P0** | ✅ 2026-06-10 | `AttritionTax` QiTransferReason + `AttritionConfig` 常数 + `QiAttritionSystem` server 逻辑 | 拿起 1 个 qi_value=100 灵石 → qi_value=97 + zone +3 + 守恒律单测全绿 |
| **P1** | ✅ 2026-06-10 | 多 op_kind 磨损分档（拾起/移动/搜刮/炼器/炼丹）+ 环境倍率（死域×3/坍缩渊×3/馈赠区×0.8）+ `AttritionExemptTag` | 各 op_kind 磨损率单测 + 环境倍率边界 |
| **P2** | ✅ 2026-06-11 | Client VFX：`bong:vfx/qi_attrition` 粒子闪光 + 物品 qi_value HUD 实时更新 | 客户端拿起高灵气物品 → 物品栏显示 qi_value 下降 + 粒子闪烁 |
| **P3** | ✅ 2026-06-11 | 边界情况：`qi_value < 0.5` 跳过磨损 + 封灵容器豁免 + 死坍缩渊（已 dead）无灵气可逸散的处理 | 边界单测 + 封灵容器豁免单测 |

> **P0/P1 落地（2026-06-10）**：`server/src/qi_physics/attrition.rs` 新建（`AttritionConfig` + `AttritionOpKind` + `apply_attrition` + `release_attrition_to_zone` 守恒归还 `zone.spirit_qi`）；`ledger.rs` 加 `QiTransferReason::AttritionTax{op_kind}` 变体；`constants.rs` 配置（BASE_RATE=0.03 等）。**4/5 op_kind 已接生产**：Pickup / SlotMove / ContainerSearch / AlchemyLoad。**遗留**：`ForgeLoad`（炼器）变体+rate 已定义并 pin 测试，但 forge session 生产接线 deferred follow-up。cargo fmt + clippy(-D warnings) + test **8130 passed**（attrition 专项 25，含守恒律+overflow守恒+各 op_kind率+环境倍率边界+<0.5 skip+exempt）。


---

## P0 — 数据模型 + 核心系统

- [x] `server/src/qi_physics/attrition.rs`（新文件）：`AttritionConfig { base_rate: f32, env_multipliers: HashMap<ZoneType, f32> }` Resource + `compute_attrition_amount(qi_value, op_kind, env) -> f32` 函数
- [x] `AttritionTax { op_kind: AttritionOpKind }` 变体加入 `qi_physics::ledger::QiTransferReason`（`server/src/qi_physics/ledger.rs`）
- [x] `QiAttritionSystem` Bevy System：监听 `InventoryItemMoved` event → 检查 item `ItemQiValue.current > 0.5` → 计算 `attrition_amount` → `ledger.transfer(AttritionTax)` → emit `AttritionAppliedEvent`
- [x] BASE_RATE = 0.03（3%）写入 `qi_physics/constants.rs`（禁止本 plan 内 inline 常数，必须经 const 引用）
- [x] ≥ 10 单测（pickup 磨损 3% 守恒 / qi_value 为 0 时跳过 / QiTransfer reason 正确 / WorldQiAccount 增量等于 item 减量）

---

## P1 — 多档磨损 + 环境倍率

- [x] `AttritionOpKind { Pickup(0.03), SlotMove(0.02), ContainerSearch(0.05), ForgeLoad(0.04), AlchemyLoad(0.04) }` — 括号内为 BASE_RATE 乘子
- [x] 环境倍率（`ZoneType` based）：`AshDeadZone` → ×3，`CollapseZoneActive` → ×3，`SpiritSourceZone(density > 0.7)` → ×0.8，其余 ×1.0
- [x] `AttritionExemptTag` Component：贴此 tag 的物品所在容器内操作豁免（实装"封灵匣"物品的防护效果；封灵匣 item 是 spiritwood-v1 ✅ 的产物）
- [x] ≥ 12 单测（各 op_kind 分档 / 死域×3 边界 / 封灵容器内物品豁免 / 0.8 倍率精度）

---

## P2 — 客户端 VFX 反馈

- [x] `bong:vfx/qi_attrition` CustomPayload：`{ item_entity_id, amount_lost, world_pos }` → 由 `AttritionAppliedEvent` 触发，server 向操作者客户端发送
- [x] client `QiAttritionVfxPlayer`：收包后在 world_pos 处 spawn 3-5 个 `BongSpriteParticle`，颜色 `#D4A820`（暗金色），lifetime 8 tick，向上飘散 0.5m；无文字数字（避免"你损失了 3 灵气"的游戏化感觉，用隐式粒子暗示）
- [x] 物品栏 qi_value 标签实时响应 `ItemQiValue` 更新（不需要新 CustomPayload，走 `InventorySync` 已有通道）
- [x] ≥ 6 单测（VFX packet 格式 / 粒子参数范围 / 不发给非操作者客户端）

> **P2 落地（2026-06-11）**：`server/src/network/qi_attrition_emit.rs` 新增定向 S2C 发射器与 `AttritionAppliedEvent`；`client/src/main/java/com/bong/client/network/QiAttritionPayload.java` 解析并校验包版本/大小/坐标；`client/src/main/java/com/bong/client/visual/particle/QiAttritionVfxPlayer.java` 播放暗金色 3-5 粒子、8 tick、0.5m 上飘；`ItemTooltipPanel` 继续消费 `InventorySync` 快照并实时响应 `spiritQuality` 变化。验证：`cargo fmt --check`、`cargo clippy --all-targets -j 2 -- -D warnings`、`cargo test -j 2`、`./gradlew test --max-workers=2` 通过。

---

## P3 — 边界与豁免处理

- [x] `qi_value < 0.5` 时跳过磨损（避免微小真元频繁结算带来的性能损耗 + 归还量小于 ledger 最小精度的数值问题）
- [x] Zone `spirit_qi = 0.0`（死域）：磨损额仍从物品扣除，但 `qi_release` 到该 zone 由 qi_physics 正常处理（0 浓度 zone 仍有 WorldQiAccount，只是不增加密度——守恒律维护）
- [x] 死坍缩渊（zone 已塌缩）内的操作：禁止进死坍缩渊操作物品（tsy-raceout-v1 应已强制撤离，此处加断言 + warn log）
- [x] ≥ 8 单测（0.5 阈值边界 / 死域 zone 归还守恒 / 死坍缩渊 guard）

> **P3 落地（2026-06-11）**：`server/src/qi_physics/attrition.rs` 新增 `AttritionApplyOutcome` / `AttritionSkipReason` / `apply_attrition_checked`，把 `qi_value < QI_ATTRITION_MIN_QI` 显式归类为 `BelowMinimumQi`，并让 `qi_value == QI_ATTRITION_MIN_QI` 正常结算磨损；`dead_tsy_family_id` 读取 `TsyZoneStateRegistry` 中 `TsyLifecycle::Dead`，死坍缩渊操作返回 `DeadTsy` 并 warn，不扣物品真元，避免无接收方时吞真元；缺少 zone 上下文时返回 `MissingZone`，不扣 item、不发 transfer；`server/src/inventory/mod.rs` 给 `ContainerSpec` 增加 `attrition_exempt`，`client_request_handler.rs` / `tsy_container_search.rs` 在 SlotMove / Pickup / ContainerSearch / AlchemyLoad 接入容器级豁免与 checked 磨损。验证：`cargo fmt --check`、`cargo clippy --all-targets -j 2 -- -D warnings`、`cargo test -q -j 2` 通过。

---

## §8 开放问题（P0 决策门收口）

1. **BASE_RATE 校准（区间内取值，非推翻 P0）**：worldview §八.2 给定 1-5% 区间，骨架统一取**中位 0.03** 作为 P0/P1/示例全文一致的起始默认值（目标段、P0 表、P0 交付物均按 0.03 算：qi=100 → 97）。校准变量：100 次操作后 0.97^100 ≈ 4.8% 残留——若 telemetry 显示多步炼丹被磨废，**在 1-5% 区间内向下调**（如 0.01），仅改 `qi_physics/constants.rs` 内单个 const，不动任何 P0 结构。**禁止** §8 与 P0 之间留两个互相矛盾的写死值
2. **操作磨损与 shelflife 叠加是否过于严苛**：高品质丹药同时承受时间衰减和操作衰减，新玩家进秘境一趟出来可能发现丹药已报废。是否加"封灵包装"物品（使用时才拆封）豁免 shelflife 但不豁免操作磨损
3. **搬运骨币是否算 qi_value > 0**：骨币是"异变兽骨封灵"所得，确实有 qi_value——频繁倒腾骨币被征税是设计意图还是意外惩罚？建议：骨币封灵值按当前 qi_value 征 1%（更低税率），避免货币体系被磨损机制摧毁
4. **炼器/炼丹工序拆解**：一次炼器需要 3-4 步 inventory 操作，每步 4% = 总损耗约 15%。设计上这是"专业技术有保真溢价"的体现，但数值需实测
5. **封灵容器的制作门槛**：spiritwood-v1 ✅ 的封灵匣需要高级灵木，低境界玩家根本买不起——他们只能硬扛磨损或极度减少操作次数，这是 worldview "末法残土万物皆有代价"的体现，无需豁免

## Finish Evidence

### 落地清单

- **P0/P1 server 核心**：`server/src/qi_physics/attrition.rs`、`server/src/qi_physics/ledger.rs`、`server/src/qi_physics/constants.rs` 落地 `AttritionOpKind`、`AttritionConfig`、`QiTransferReason::AttritionTax`、环境倍率、`release_attrition_to_zone` 守恒归还；`server/src/network/client_request_handler.rs`、`server/src/world/tsy_container_search.rs` 接入 Pickup / SlotMove / ContainerSearch / AlchemyLoad。
- **P2 client 反馈**：`server/src/network/qi_attrition_emit.rs` 定向发射 `bong:vfx/qi_attrition`；`client/src/main/java/com/bong/client/network/QiAttritionPayload.java`、`client/src/main/java/com/bong/client/visual/particle/QiAttritionVfxPlayer.java` 解析并播放暗金粒子；物品栏 qi_value 继续走 `InventorySync` 实时刷新。
- **P3 边界豁免**：`server/src/qi_physics/attrition.rs` 落地 `AttritionApplyOutcome`、`AttritionSkipReason::{BelowMinimumQi, DeadTsy, MissingZone}`、`apply_attrition_checked`、`dead_tsy_family_id`；`server/src/inventory/mod.rs` 落地 `ContainerSpec.attrition_exempt` 与 `inventory_instance_container_attrition_exempt`；生产接线覆盖 SlotMove / Pickup / ContainerSearch / AlchemyLoad。

### 关键 commit

- `a29bf371f`（2026-06-10）：P0/P1 真元搬运磨损，含守恒归还 zone、op_kind 分档和环境倍率。
- `c9dcea5cc` / `42eba7bda` / `a87fc20a0` / `1e6982dfb`（2026-06-11）：P2 客户端粒子反馈、搜刮与炼丹投料磨损反馈补齐。
- `a3bed2dfb` / `18cd57e20` / `97c378d95`（2026-06-11）：P3 阈值边界、封灵容器显式豁免、死坍缩渊 guard、缺 zone 守恒 skip 与饱和单测。

### 测试结果

- `cd server && cargo fmt --check`：通过。
- `cd server && BONG_SKIP_SKIN_PREFETCH=1 CARGO_BUILD_JOBS=2 nice -n 10 ionice -c3 cargo clippy --all-targets -j 2 -- -D warnings`：通过。
- `cd server && BONG_SKIP_SKIN_PREFETCH=1 CARGO_BUILD_JOBS=2 nice -n 10 ionice -c3 cargo test -q -j 2 attrition`：通过，`47 passed; 0 failed`。
- `cd server && BONG_SKIP_SKIN_PREFETCH=1 CARGO_BUILD_JOBS=2 nice -n 10 ionice -c3 cargo test -q -j 2`：通过，`8532 passed; 0 failed; 1 ignored`。
- `cd client && ./gradlew test --max-workers=2`：P2 阶段通过，覆盖 VFX packet 与粒子参数。

### 跨仓库核验

- **server**：`AttritionOpKind`、`AttritionApplyOutcome`、`AttritionSkipReason`、`apply_attrition_checked`、`QiTransferReason::AttritionTax`、`ContainerSpec.attrition_exempt`、`inventory_instance_container_attrition_exempt`。
- **client**：`QiAttritionPayload`、`QiAttritionVfxPlayer`、`InventorySync` 物品 qi_value 更新链路。
- **agent**：本 plan 为服务端物理与客户端粒子反馈，不新增 agent schema / Redis 契约。

### 遗留 / 后续

- `ForgeLoad` 变体与 rate 已定义并测试，炼器生产接线仍依赖后续 forge session 流程接入，不阻塞本 plan。
- 骨币搬运税率是否降为 1% 仍属数值校准问题，待 telemetry 或独立 balance plan 决策。
