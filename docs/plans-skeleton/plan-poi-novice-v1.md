# Bong · plan-poi-novice-v1 · 骨架

P1 引气期玩家活动范围(spawn ± 1500 格)注入新手 POI——破败炼器台 / 凡铁丹炉 / 散修聚居点 / 异变兽巢——让玩家不必远跑就能完成"第一次炼器/第一次炼丹/第一次社交/第一次战斗"。

**世界观锚点**：`worldview.md §十三.初醒原` · `§十三.青云残峰`(broken_peaks 外围,灵气 0.4-0.5)

**library 锚点**：`peoples-0007 散修百态`(散修聚居形态)

**交叉引用**：`plan-worldgen-v3.1` ✅ · `plan-spawn-tutorial-v1` ⬜ · `plan-forge-leftovers-v1` ⬜ · `plan-alchemy-client-v1` ⏳ · `plan-gameplay-journey-v1` §P1

---

## 接入面 Checklist

- **进料**：spawn_plain + broken_peaks 现有 worldgen + plan-spawn-tutorial-v1 完成的初始环境
- **出料**：spawn ± 1500 格内分散 6 处 POI
- **共享类型**：worldgen blueprint POI + `npc::scenario` ✅(NPC 聚集)
- **worldview 锚点**：§十三 spawn 0.3 → broken_peaks 外围 0.4-0.5 灵气过渡区

---

## §0 设计轴心

- [ ] **新手不远跑**：1500 格内能见到全部 5 大玩法的最低形态(炼器/炼丹/采集/战斗/社交)
- [ ] **POI 是种子**：每个 POI 给"第一次"体验,但要进阶必须远走
- [ ] **末法朽坏感**：所有 POI 都是末法残土风格——破败、半埋、有前修士尸骨

---

## §1 POI 清单

| POI | 位置(相对 spawn) | 内容 | 服务玩法 |
|---|---|---|---|
| 破败炼器台 | (300, _, 200) | 损坏的 forge Station,可用但效率减半 | 第一次炼器 |
| 凡铁丹炉 | (-400, _, 100) | 凡铁锅 + 篝火,可炼基础丹 | 第一次炼丹 |
| 散修聚居点 | (500, _, -300) | 2-3 名散修(`Rogue`),茅屋 + 死信箱 | 第一次社交/交易 |
| 异变兽巢 | (1200, _, 800) | 缝合兽 + 灰烬蛛 nest | 第一次猎兽核 |
| 残卷藏匿点 | (-800, _, -1200) | cave_network 入口,1-2 张残卷 | 第一次拾取知识 |
| 灵草谷 | (-300, _, 600) | 5+ 种基础灵草集中区 | 第一次采集 |

---

## §2 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ⬜ | 6 处 POI 的 worldgen blueprint 注入 | 玩家从 spawn 出发 5 分钟内可见任一 POI |
| **P1** ⬜ | POI 内的 entity/loot 表(散修、兽核、残卷) | 各 POI 内容正常 drop/交互 |
| **P2** ⬜ | 朽坏视觉(残灰、断壁、骨架方块组合) | 美术风格统一 |

---

## §3 数据契约

- [ ] `worldgen/scripts/terrain_gen/profiles/spawn_plain.py` + `broken_peaks.py` POI hooks
- [ ] `server/src/world/poi_novice.rs` runtime POI 加载
- [ ] `server/src/inventory/poi_loot.rs` POI 内宝箱 loot 表
- [ ] `worldgen/blueprints/poi_novice/*.json` 6 处 POI 的 blueprint

---

## §4 开放问题

- [ ] POI 是否随机生成位置(每存档不同) vs 固定坐标? 决策倾向: **半固定**——区域固定,微调随机化
- [ ] 散修聚居点是否会被玩家屠村(若屠则永久失去交易)?
- [ ] 残卷藏匿点的残卷是否限定一份(全服第一个拿到独占)? 决策倾向: 无限刷新但内容随机
- [ ] 异变兽巢刷新周期(被打完后多久重生)?

## §5 进度日志

- 2026-05-01：骨架创建。plan-gameplay-journey-v1 §P1 派生。
