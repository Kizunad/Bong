# Bong · plan-economy-v1

骨币半衰期 + 末法节律影响价格 + 100h 经济曲线。承接 worldview §九经济与 §十七末法节律。**plan-gameplay-journey-v1 §E 多处依赖**。

**世界观锚点**：`worldview.md §九 封灵骨币 / 半衰期 / 盲盒死信箱` · `§十七 末法节律(夏冬汐转)`

**library 锚点**：`world-0004 骨币半衰录`

**交叉引用**：`plan-qi-physics-v1` 🆕(skeleton, **P1 衰变直接调底盘 `qi_excretion`**) · `plan-fauna-v1` ✅(骨币原料异变兽骨 + **P0 顺手实装**) · `plan-social-v1` ✅(死信箱/盲盒/匿名交易) · `plan-mineral-v2` ✅(灵石燃料,**非货币**) · `plan-tsy-loot-v1` ✅(上古遗物 jackpot) · `plan-lingtian-weather-v1` ⏳(汐转节律,P2 节律乘数源) · `plan-gameplay-journey-v1` §E/§Q

> **2026-05-05 现状对齐**：P0 在 plan-fauna-v1（PR #105，2026-05-02）顺手落地，**字段名是 `spirit_quality (0..=1)` 而非 `remaining_qi`**；衰变机制并入 `shelflife` 走 3/7/14 day 线性归零。
>
> **2026-05-05 上钻**：原 §1.5 "shelflife 线性 vs 30 day 半衰" 二选一**已废弃** —— 上钻发现 worldview §二「真元极易挥发」是全局现象（骨币半衰、食材腐败、距离衰减、TSY 抽真元等同源），各 plan 各自拍数才是问题根源。已立 **`plan-qi-physics-v1`**（修仙物理底盘，唯一物理实现入口）。本 plan P1 不再自行决定衰变曲线 —— 等 qi-physics P1 通用算子 `qi_excretion(coin, ContainerKind::SealedInBone, elapsed, env)` 落地后直接调用，shelflife 现有的 `bone_coin_5/15/40_v1` profile 在 qi-physics P2 阶段一并迁出。

---

## 接入面 Checklist

- **进料**：`fauna` 兽骨 drop ✅(plan-fauna-v1) + `mineral` 灵石产量 ✅ + `tsy_loot` 上古遗物 ✅(`spirit_quality=0` 不参与衰变) + `lingtian-weather` 节律状态 ⏳
- **出料**：骨币 item NBT(`spirit_quality` 字段，**复用通用真元残量**) + 半衰 tick(目前由 `shelflife::DecayProfileRegistry` 兜底) + 价格指数计算 + 节律影响系数
- **共享类型**：**不新增 `remaining_qi`**——直接复用 `inventory::ItemInstance.spirit_quality (0..=1)`，制作时 `spirit_quality = sealed_qi / qi_cap`（已在 `bone_coin.rs:197`）。新造一份 NBT 字段是 §四"近义重名"红旗
- **跨仓库契约**：`bone-coin-tick` schema(P2/P3) + client 显示真元残量(目前已通过通用 tooltip 显 spirit_quality，缺"骨币语义化"展示) + agent 经济 narration("天下灵气总价")
- **worldview 锚点**：§九(骨币半衰) + §十七(节律对价格)
- **qi_physics 锚点**：P1 调 `qi_physics::qi_excretion(coin, ContainerKind::SealedInBone, elapsed, env)`；P2 价格指数的节律乘数取 `qi_physics::constants::QI_RHYTHM_*` + `EnvField`；本 plan **不引入新物理常数**

---

## §0 设计轴心

- [x] **骨币不是堆叠数字**：每枚都有独立的真元残量(`spirit_quality`),半衰是 per-coin 而非整堆 —— ✅ 已实装(`fauna/bone_coin.rs`)
- [ ] **持有 = 贬值（受地点制约）**：worldview §二 / §三 压强法则物理推导——骨币内真元逸散下限 = 当地 zone 浓度，**不是 0**。后果：
  - **聚灵阵 / 灵田 / 高浓度 zone 储藏** → 骨币长期保值（逸散后 spirit_quality ≈ 高 zone 浓度）
  - **废地 / 普通 zone** → 渐近 zone 基底（worldview §九「1 月只剩 ~20%」对应"末法残土普通 zone 的平均浓度水位"）
  - **死域 / 坍缩渊** → 真归零（zone_qi=0 → 容器无下限保护 → worldview §十六.三「满灵骨币变普通骨头」）
  - 与 §十一 「灵物密度阈值天道注视」形成张力——储太多在聚灵阵反招天道
  - 公式由 **`plan-qi-physics-v1`** 唯一定义（不在本 plan 拍数）；本 plan P1 只负责注册 `ContainerKind::SealedInBone` 容器参数 + 阈值消失检查 + narration
- [ ] **节律影响**：夏(炎汐)灵气活跃 → 价格 ↑;冬(凝汐)灵气稳态 → 价格 ↓;汐转期价格剧烈波动
- [x] **灵石不是货币**：严格 worldview §九 — 灵石是燃料,不能作为通货交易 —— ✅ plan-mineral-v2 已落
- [ ] **顶级资产 = 情报**：worldview §九 强调坐标/丹方/路线比骨币值钱 —— **P3 narration 表达**

---

## §1 阶段总览

| 阶段 | 内容 | 验收 | 状态 |
|---|---|---|---|
| **P0** | BoneCoin item 定义 + 真元残量(`spirit_quality`)+ 制作 recipe(异变兽骨 + 阵法 + 真元注入 + 可选 ZHEN_SHI_CHU 催化剂) | 玩家可制作 + 拾取 + 真元残量可见 | 🔄 **已落地于 plan-fauna-v1 (PR #105, 2026-05-02)** —— `BoneGrade::{Rat,Spider,Hybrid,General}`、`plan_bone_coin_craft`、`apply_bone_coin_craft_session`、5 单测 |
| **P1** | 注册 `ContainerKind::SealedInBone` 物理参数 + 调 `qi_physics::qi_excretion` 衰变 + 阈值消失自动从 inventory 移除 + narration | 月内大幅贬值,无人当守财奴 | ⬜ **等 plan-qi-physics-v1 P1 算子落地后启动**（前置硬依赖） |
| **P2** | 价格指数(基于世界总骨币真元 + 节律) + 商人 NPC 报价应用 | NPC 价格随节律波动 | ⬜ —— `npc/social::rarity_base_price()` 已是占位 baseline，待此 plan 替换 |
| **P3** | 100h 经济曲线 telemetry + tiandao "天下灵气总价" narration(每 in-game month 一次) | 经济曲线落地 §E 表 | ⬜ |

---

## §1.5 P1 前置依赖（已上钻）

> **2026-05-05 重写**：原 §1.5 三选一（shelflife 线性 / 指数半衰 / BoneCoin 独立 decay）**全部废弃** —— 三选一本身就是 §四"近义重名"红旗的体现：worldview §二 是全局物理（骨币半衰、食材腐败、距离衰减、TSY 抽真元同源），不该让 economy plan 单独决定衰变曲线形态。

**当前裁决**：本 plan 不再自定衰变公式。骨币衰变形态由 **`plan-qi-physics-v1`** 通过 `qi_excretion(initial, container, elapsed, env)` 统一表达：

- 容器侧：本 plan P1 在 `qi_physics::ContainerKind` 注册 `SealedInBone {grade}`(对应 5/15/40 三档骨币的密封等级)
- 公式侧：曲线形态（线性 / 指数 / Stepwise）由 qi-physics P1 决定（其 §5 三红线决策门之一）
- 现状清理：`shelflife/registry.rs` 已挂的 `bone_coin_5/15/40_v1` profile 由 qi-physics P2 阶段统一迁出，本 plan 不重复迁

**P1 启动条件**（硬前置）：
1. plan-qi-physics-v1 P1 完成 → `qi_excretion` 算子可用
2. plan-qi-physics-v1 P0 §5 红线 1（distance decay 0.06 vs 0.03）已结案——侧面验证底盘正典化路径走得通

在那之前本 plan P1 阻塞。注意：**shelflife 现行 3/7/14 day 线性归零到 0 实际违反 worldview §二 / §三 压强法则**（容器逸散下限应为当地 zone 浓度而非 0），qi-physics P2 迁移会修复——届时正常 zone 里骨币不会归零，只会渐近 zone 基底；只有死域 / 坍缩渊（zone_qi=0）才真归零，正好对应 worldview §十六.三「满灵骨币变普通骨头」。

---

## §2 关键公式

```
半衰期: 不在本 plan 定义 —— 调用 plan-qi-physics-v1 通用算子:

  spirit_quality(t) = qi_physics::qi_excretion(
      initial = sealed_qi / qi_cap,
      container = ContainerKind::SealedInBone { grade: BoneGrade },
      elapsed_ticks = now - issued_at_tick,
      env = EnvField { rhythm, local_qi_density, tsy_intensity },
  )
  spirit_quality < threshold(由 qi_physics 决定) → 自动消失

  → 曲线形态(线性/指数/Stepwise)是 qi_physics P1 的事,本 plan 不关心

价格指数(本 plan 自有):
  base_price(item)  = item 内在价值(目前由 npc::social::rarity_base_price 占位:
                                   Common 4 / Uncommon 12 / Rare 40 / ...)
  market_factor     = log(supply / demand)  -- supply 用世界总骨币 spirit_quality 求和
  rhythm_factor     = qi_physics::constants::QI_RHYTHM_NEUTRAL  (冬/凝汐)
                    | qi_physics::constants::QI_RHYTHM_ACTIVE   (夏/炎汐)
                    | random in QI_RHYTHM_TURBULENT_RANGE        (汐转 7 天)
  final_price       = base × market_factor × rhythm_factor
```

100h 经济曲线(plan-gameplay-journey-v1 §E 移植):

```
P0-P1 醒灵-引气:    0-10 骨币(凡人级)
P2 凝脉:            10-50 骨币(凝脉小富)
P3 固元:            100-500 骨币(固元买卖)
P4 通灵:            500-5000 骨币(巨贾,但月内大幅半衰)
P5 化虚:            骨币失意义,权力换算为天道注意力
```

---

## §3 数据契约

- [x] `server/src/fauna/bone_coin.rs` — `BoneGrade` + `BoneCoinCraftSession/Request/Crafted` + `plan_bone_coin_craft` + `apply_bone_coin_craft_session` ✅(PR #105)
- [x] `server/src/shelflife/registry.rs` — `bone_coin_5/15/40_v1` profile 已挂(线性 3/7/14 day)；**plan-qi-physics-v1 P2 阶段统一迁出**
- [x] `server/src/inventory::ItemInstance.spirit_quality` — 真元残量字段(通用),骨币复用
- [ ] **(P1)** 在 `qi_physics::ContainerKind` 注册 `SealedInBone { grade: BoneGrade }` + 调 `qi_excretion` 替换 shelflife 的骨币 profile
- [ ] **(P1)** 阈值消失：检查 `shelflife` 已有此能力还是要在 qi_physics 提供（前者直接复用，后者归 qi-physics scope 不归本 plan）
- [ ] **(P1)** narration: 「你怀里的骨币又少了几枚」类逐月提示
- [ ] **(P2)** `server/src/economy/price_index.rs` 价格指数(用 qi_physics::constants::QI_RHYTHM_* 取节律乘数,替换 `npc::social::rarity_base_price` placeholder)
- [ ] **(P2)** `server/src/npc/merchant.rs` 商人 NPC 应用价格(plan-fauna 之外的新模块,或扩 npc::social)
- [ ] **(P2/P3)** `agent/packages/schema/src/economy.ts` BoneCoinTick + PriceIndex schema
- [ ] **(P3)** `client/.../economy/BoneCoinTooltip.java` 显示"封灵真元残量"语义化(目前通用 tooltip 已显 spirit_quality 数值,缺骨币专属说明)
- [ ] **(P3)** tiandao agent narration 模板("天下灵气总价")

---

## §4 开放问题

收口的：

- [x] **玩家能否自制骨币 vs 必须 NPC 制造** → **玩家自制**(PR #105 已实装,需阵法封灵 + 异变兽骨,可选 ZHEN_SHI_CHU 催化剂免 20% seal cost)
- [x] **上古遗物如何作为"一次性大爆发"接入价格指数** → 上古遗物 `spirit_quality = 0`(`inventory/ancient_relics.rs:84`,worldview §十六 锚定),**不参与 remaining_qi 衰变**;P2 价格指数计算 supply 时**只统计 `spirit_quality > 0` 的骨币**,上古遗物作为离群 jackpot 不进市场公式
- [x] **P1 衰变曲线裁决** → **上钻为 plan-qi-physics-v1**（见 §1.5）。本 plan P1 不再自定曲线，调底盘 `qi_excretion`
- [x] **死亡时携带骨币如何处理** → worldview §十六.三 「满灵骨币 → 普通骨头」是 TSY 抽真元的体现，归 plan-qi-physics-v1 `EnvField.tsy_intensity` 物理处置；本 plan 只在 P3 narration 表达，不再独立决策

未决的（P2 启动前需收口）：

- [ ] **节律检测依赖** plan-lingtian-weather-v1(汐转期判定):需 Wave 3 协调,**P2 的硬前置**;qi-physics 提供 `EnvField.rhythm` 通道
- [ ] **商人 NPC 价格策略是否 AI 化**(根据玩家境界涨价 / "看人下菜")?或单纯按节律乘数?**P2 决策点**
- [ ] **价格指数 supply 求和的尺度**:全服骨币 spirit_quality 求和 vs 区域级求和(避免单服扫全图性能问题)?**P2 决策点**

## §5 进度日志

- 2026-05-01：骨架创建。plan-gameplay-journey-v1 §E 派生
- 2026-05-02：plan-fauna-v1 (PR #105 commit `c5895641`) 顺手实装 **P0 全部交付**(`bone_coin.rs` + 三档 shelflife profile)。骨架本身未同步更新
- 2026-05-05 (上午)：首次"文档↔代码"核验对齐。发现 P0 已落地、字段名 `spirit_quality` 取代了草案的 `remaining_qi`、shelflife 线性归零和 worldview "半衰期" 描述形式不同效果近似。P1 启动前曾设 §1.5 三选一裁决（shelflife 线性 / 指数半衰 / BoneCoin 独立 decay）
- 2026-05-05 (下午)：上钻——audit 全库发现 worldview §二「真元极易挥发」是 9 类 plan 的同源现象（骨币/食材/距离/异体排斥/吸力/节律/末法残土/灵田漏液/搜刮磨损），各 plan 拍数才是问题根源。立 **`plan-qi-physics-v1`** 物理底盘骨架；本 plan §1.5 三选一**全部废弃**，P1 改为"等底盘算子 + 注册 ContainerKind::SealedInBone"。"死亡时携带骨币" 也归底盘 `EnvField.tsy_intensity` 处置
- 2026-05-05 (下午-2)：qi-physics 骨架补"压强法则"为第二公理（worldview §二 line 20 / §三 line 32）。骨币逸散物理下限 = 当地 zone 浓度而非 0；shelflife 现行"归零到 0"违反 worldview，P2 迁移修复。同步本 plan §0「持有=贬值」补地点制约推导（聚灵阵保值/废地渐近基底/死域真归零，对应 §十六.三 "满灵骨币变普通骨头"），§1.5 末段补 shelflife 违反正典提示
- **2026-05-09**：升 active（`git mv docs/plans-skeleton/plan-economy-v1.md → docs/plan-economy-v1.md`）。触发条件：
  - **plan-qi-physics-patch-v1 ✅ finished**（PR #162，2026-05-08）—— `qi_excretion(ContainerKind::SealedInBone, env)` 底盘已就位，骨币 shelflife 已切走压强法则 clamp 路径，本 plan §1.5 三选一裁决无解的最大障碍清除
  - P0 已实装（PR #105 commit `c5895641`）+ P1 上钻已收口 → 直接进 P2（节律 × 价格指数）
  - 下一步：等 plan-lingtian-weather-v1 汐转期 API 完整暴露后，启动 P2「价格指数 = base × 节律乘数」实装；同步收口 §4 三个未决（节律检测依赖 / 商人 AI 化 / 价格指数尺度）

## Finish Evidence

### 落地清单

- P0 骨币物料：沿用 `server/src/fauna/bone_coin.rs`、`server/src/inventory::ItemInstance.spirit_quality` 和 `server/src/shelflife/registry.rs` 已落地结果；本次未新增 `remaining_qi` 近义字段。
- P1 真元逸散底盘：沿用 `plan-qi-physics-patch-v1` 已完成的 `ContainerKind::SealedInBone` / `qi_excretion` 路径；本 plan 不重复定义衰变公式。
- P2 价格指数：`server/src/economy/mod.rs` 新增 `BoneCoinSupply` / `EconomyPriceIndex`，按玩家 inventory 内骨币面额与 `spirit_quality` 聚合世界供给，使用 `QI_RHYTHM_ACTIVE` / `QI_RHYTHM_NEUTRAL` / `QI_RHYTHM_TURBULENT_RANGE` 计算节律乘数，按供需 log 因子输出价格倍数。
- P2 NPC 报价入口：`server/src/npc/social.rs` 保持 `estimate_item_price()` neutral 行为兼容，并新增 `estimate_item_price_for_index()` 供商人 / NPC 交易在拿到市场快照时套用 economy 指数。
- P2/P3 IPC：`server/src/schema/economy.rs`、`server/src/schema/channels.rs`、`server/src/network/redis_bridge.rs` 新增 `bong:bone_coin_tick` 与 `bong:price_index` 发布契约；`agent/packages/schema/src/economy.ts`、samples 与 generated schema 对拍。
- P3 月度 telemetry：`server/src/economy/mod.rs` 注册 `publish_economy_telemetry_system`，每 30 个 vanilla day tick 发布骨币真元供给与价格指数快照。
- P3 天道叙事：`agent/packages/tiandao/src/economy-analyzer.ts`、`redis-ipc.ts`、`runtime.ts` 消费 `PRICE_INDEX`，输出「天下灵气总价」类 narration，并保留 cross-system buffer 观测。
- P3 client 展示：`client/src/main/java/com/bong/client/inventory/model/InventoryItem.java` 与 `ItemTooltipPanel.java` 将骨币 `spirit_quality` 显示为「封灵真元 XX%」，普通物品仍显示「纯度」。

### 关键 commits

- `ddf74cdc8` (2026-05-09) `plan-economy-v1: 接入骨币价格指数`
- `e79fb9f5e` (2026-05-09) `plan-economy-v1: 补齐经济 IPC 与天道叙事`
- `a4e5d023b` (2026-05-09) `plan-economy-v1: 语义化骨币真元 tooltip`

### 测试结果

- `agent/packages/schema`: `npm run generate` → 314 schemas exported。
- `agent`: `npm run build` → passed。
- `agent/packages/schema`: `npm test` → 12 files / 330 tests passed。
- `agent/packages/schema`: `npm run build` → passed。
- `agent/packages/tiandao`: `npm test` → 43 files / 308 tests passed。
- `client`: `JAVA_HOME="/usr/lib/jvm/java-17-openjdk-amd64" ./gradlew test build` → BUILD SUCCESSFUL。
- `server`: `cargo fmt --check` → passed。
- `server`: `cargo clippy --all-targets -- -D warnings` → passed。
- `server`: `cargo test` → 3205 tests passed。

### 跨仓库核验

- server: `EconomyPriceIndex`、`RedisOutbound::BoneCoinTick`、`RedisOutbound::PriceIndex`、`estimate_item_price_for_index`。
- agent schema: `BoneCoinTickV1`、`PriceSampleV1`、`PriceIndexV1`、`CHANNELS.PRICE_INDEX`。
- tiandao: `EconomyAnalyzer`、`RedisIpc.drainPriceIndexEvents()`、`processEconomyEvents()`。
- client: `InventoryItem.isBoneCoin()`、`ItemTooltipPanel.formatStatusLine()`。

### 遗留 / 后续

- 无本 plan 阻塞项。商人 AI 化与区域级供给尺度没有在当前代码库形成稳定 runtime 接口；本次按 KISS/YAGNI 收敛为全服供给指数 + 显式 NPC 估价入口，后续若出现真实 merchant 系统再接入当前 `EconomyPriceIndex`。
