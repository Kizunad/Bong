# Bong · plan-economy-v1 · 骨架

骨币半衰期 + 末法节律影响价格 + 100h 经济曲线。承接 worldview §九经济与 §十七末法节律。**plan-gameplay-journey-v1 §E 多处依赖**。

**世界观锚点**：`worldview.md §九 封灵骨币 / 半衰期 / 盲盒死信箱` · `§十七 末法节律(夏冬汐转)`

**library 锚点**：`world-0004 骨币半衰录`

**交叉引用**：`plan-fauna-v1` ⬜(骨币原料异变兽骨) · `plan-social-v1` ✅(死信箱/盲盒/匿名交易) · `plan-mineral-v2` ✅(灵石燃料,**非货币**) · `plan-tsy-loot-v1` ✅(上古遗物 jackpot) · `plan-lingtian-weather-v1` ⏳(汐转节律) · `plan-gameplay-journey-v1` §E/§Q

---

## 接入面 Checklist

- **进料**：`fauna` 兽骨 drop + `mineral` 灵石产量 + `tsy_loot` 上古遗物 + `lingtian-weather` 节律状态
- **出料**：骨币 item NBT(`remaining_qi` 字段) + 半衰 tick + 价格指数计算 + 节律影响系数
- **共享类型**：`BoneCoin` item 与现有 `inventory::ItemStack` 集成,真元字段是 NBT 而非 stack count
- **跨仓库契约**：`bone-coin-tick` schema + client 显示真元残量 + agent 经济 narration("天下灵气总价")
- **worldview 锚点**：§九(骨币半衰) + §十七(节律对价格)

---

## §0 设计轴心

- [ ] **骨币不是堆叠数字**：每枚都有独立的 `remaining_qi` NBT,半衰是 per-coin 而非整堆
- [ ] **持有 = 贬值**：不允许"无限囤积"——骨币每 30 in-game day 衰 50%
- [ ] **节律影响**：夏(炎汐)灵气活跃 → 价格 ↑;冬(凝汐)灵气稳态 → 价格 ↓;汐转期价格剧烈波动
- [ ] **灵石不是货币**：严格 worldview §九 — 灵石是燃料,不能作为通货交易
- [ ] **顶级资产 = 情报**：worldview §九 强调坐标/丹方/路线比骨币值钱

---

## §1 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ⬜ | BoneCoin item 定义 + remaining_qi NBT + 制作 recipe(异变兽骨 + 阵法 + 真元注入) | 玩家可制作 + 拾取 + 真元残量可见 |
| **P1** ⬜ | 半衰 tick 系统(per-coin 30 day 50%) + 衰至阈值自动消失 | shelflife 类似机制 |
| **P2** ⬜ | 价格指数(基于世界总骨币真元 + 节律) + 商人 NPC 报价应用 | NPC 价格随节律波动 |
| **P3** ⬜ | 100h 经济曲线 telemetry + tiandao "天下灵气总价" narration(每 in-game month 一次) | 经济曲线落地 §E 表 |

---

## §2 关键公式

```
半衰期:
  remaining_qi(t) = remaining_qi(0) × 0.5^(t / 30_days)
  当 remaining_qi < 5% 初始值 → 自动消失(物品 entity drop 死亡)

价格指数:
  base_price(item)  = item 内在价值
  market_factor     = log(supply / demand)
  rhythm_factor     = 1.0(冬/凝汐) | 1.2(夏/炎汐) | random[0.7, 1.5](汐转 7 天)
  final_price       = base × market_factor × rhythm_factor
```

100h 经济曲线(plan-gameplay-journey-v1 §E 移植):

```
P0-P1 醒灵-引气:    0-10 骨币(凡人级)
P2 凝脉:            10-50 骨币(凝脉小富)
P3 固元:            100-500 骨币(固元买卖)
P4 通灵:            500-5000 骨币(巨贾,但每 30 day 半衰)
P5 化虚:            骨币失意义,权力换算为天道注意力
```

---

## §3 数据契约

- [ ] `server/src/economy/bone_coin.rs` item + NBT
- [ ] `server/src/economy/bone_coin_decay.rs` 半衰 tick
- [ ] `server/src/economy/price_index.rs` 价格指数 + 节律
- [ ] `server/src/npc/merchant.rs` 商人 NPC 应用价格
- [ ] `agent/packages/schema/src/economy.ts` BoneCoinTick + PriceIndex schema
- [ ] `client/.../economy/BoneCoinTooltip.java` 显示真元残量

---

## §4 开放问题

- [ ] 节律检测依赖 plan-lingtian-weather-v1(汐转期判定),需 Wave 3 协调
- [ ] 商人 NPC 价格策略是否有 AI(根据玩家境界涨价 / "看人下菜")?
- [ ] 30 day 半衰是 in-game 还是 real-time? worldview 倾向 in-game(参考 1h real ≈ 1 year in-game)
- [ ] 上古遗物如何作为"一次性大爆发"接入价格指数(脆化 = 永久消失,不参与市场)?
- [ ] 玩家能否自制骨币 vs 必须 NPC 制造? 决策倾向: **玩家自制**(需阵法封灵 + 异变兽骨)

## §5 进度日志

- 2026-05-01：骨架创建。plan-gameplay-journey-v1 §E 派生。
