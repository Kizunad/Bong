# Bong · plan-tsy-raceout-v1 · 骨架

坍缩渊塌缩 race-out **3 秒**撤离机制。当副本最后一件上古遗物被取走 → 触发塌缩 → 全副本玩家收到 `tsy::CollapseStarted` ✅ → 负压翻倍 + 随机 3-5 个塌缩裂口 + **3 秒**撤离窗口(非标准 7-15s)。**化虚级也可能死在 race-out**。

**世界观锚点**：`worldview.md §十六.六 坍缩渊塌缩`(最后一件遗物被取 → 塌缩 → race-out)

**library 锚点**：`world-0003 诸渊录·卷一·枯木崖`(坍缩渊叙事范例)

**交叉引用**：`plan-tsy-lifecycle-v1` ✅(已支持 collapse 触发) · `plan-tsy-extract-v1` ✅(标准撤离仪式) · `plan-narrative-v1` ⏳(race-out 专属台词) · `plan-gameplay-journey-v1` §M.1

---

## 接入面 Checklist

- **进料**：`tsy::CollapseStarted` event ✅ 已 emit + 副本上古遗物状态
- **出料**：3 秒倒计时 client UI + race-out 专属 narration + 慢一秒变死域的实质后果(真死)
- **共享类型**：复用 `StartExtractRequest` ✅ 但要求 `timeout=3s`(标准是 7-15s)
- **跨仓库契约**：`tsy_collapse_started` + `extract-aborted/completed` schema 已 ✅
- **worldview 锚点**：§十六.六

---

## §0 设计轴心

- [ ] **3 秒不是标准撤离**：用同样的 ExtractRequest API 但 timeout 强制 3s
- [ ] **化虚级也可能死**：大真元池在塌缩负压下吃亏更大,平衡设计
- [ ] **专属 narration**：风格台词 "它要塌了。它不在乎你身上还揣着什么。"

---

## §1 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ⬜ | 3 秒倒计时 client UI + race-out 信号识别 | 玩家在塌缩时见 3 秒红色倒计时 |
| **P1** ⬜ | 3-5 个随机塌缩裂口生成(独立位面内) | 全副本玩家可向最近裂口跑 |
| **P2** ⬜ | 慢一秒 → 副本化为死域(玩家随之消失,真死) | extract-aborted 触发,无重生 |
| **P3** ⬜ | tiandao race-out 专属 narration | "它要塌了..." 在塌缩开始时全副本广播 |

---

## §2 关键流程

```
1. 副本内最后一件上古遗物被取走 (loot table 监听)
2. tsy::CollapseStarted event ✅ broadcast
3. 全副本玩家 client 显示红色 3 秒倒计时 + race-out 专属 narration
4. server 生成 3-5 个随机塌缩裂口位置(避免聚集)
5. 玩家选择最近裂口冲过去 + 启动 race-out ExtractRequest(timeout=3s)
6. 3 秒到 → 仍在副本内的玩家全部触发"真死":
   - 物品 100% drop
   - 寿元 -5%
   - 干尸成道伥(参考 plan-tsy-hostile-v1 ✅)
   - 副本化为死域,玩家位置坐标无法 respawn
7. 化虚玩家 race-out 死亡有额外惩罚(寿元 -100 年,符合 worldview §十二)
```

---

## §3 数据契约

- [ ] `server/src/world/tsy_raceout.rs` 3 秒撤离逻辑(短 timeout 路径)
- [ ] `server/src/world/tsy_collapse_rifts.rs` 3-5 裂口生成
- [ ] `client/.../tsy/RaceoutCountdownHud.java` 3 秒倒计时 UI(红色高警告)
- [ ] `agent/packages/tiandao/skills/calamity.md` race-out 专属 narration(并入 narrative-v1)

---

## §4 开放问题

- [ ] 3 秒是否随玩家境界调整(化虚 3s vs 引气 3s 公平么)? 决策倾向: **统一 3s**——这是物理约束不是技能
- [ ] 塌缩裂口位置是真随机还是按玩家分布生成(避免某些玩家直接被困)?
- [ ] 慢一秒的玩家是"真死"还是"特殊死亡(干尸成道伥)"? 决策倾向: **真死** + 干尸成道伥
- [ ] 多个玩家同时挤一个裂口的判定?(碰撞? 并发撤离?)
- [ ] race-out 期间是否允许 PVP(被推下裂口)?

## §5 进度日志

- 2026-05-01：骨架创建。plan-gameplay-journey-v1 §M.1 派生。
