# Bong · plan-void-actions-v1 · 骨架

化虚专属 action:镇压坍缩渊 / 引爆区域 / 阻挡道伥扩散 / 道统传承。化虚后玩家不再以"突破"为驱动(没有更高境界),改为参与天道运维博弈 + 影响世界格局。

**世界观锚点**：`worldview.md §三 line 145-160`(化虚体验) · `§八 运维博弈` · `§十六 坍缩渊镇压` · `§十二 道统传承`

**交叉引用**：`plan-tribulation-v1` ⏳ · `plan-tsy-lifecycle-v1` ✅ · `plan-void-quota-v1` ⬜ · `plan-niche-defense-v1` ⬜(继承人选择 UI 可并入) · `plan-death-lifecycle-v1` ✅(亡者博物馆) · `plan-gameplay-journey-v1` §N

---

## 接入面 Checklist

- **进料**：化虚玩家身份 + 真元池 + 当前世界灵气分布 + 道伥分布(plan-tsy-hostile-v1 ✅)
- **出料**：4 类化虚 action + 道统遗物指定继承人 + 全服叙事
- **共享类型**：`VoidActionRequest`(`SuppressTsy | ExplodeZone | LegacyAssign | Barrier`) + `LegacyEntry`(亡者博物馆扩展)
- **跨仓库契约**：server void_actions + agent 全服 narration + client 化虚专属 UI
- **worldview 锚点**：§三 化虚 + §八 + §十六 + §十二

---

## §0 设计轴心

- [ ] **化虚不是退休模式**：4 大持续目标(寿元/传承/世界影响/天道博弈)
- [ ] **action 高消耗**：每个 action 烧大量真元(化虚池 500 单位的 30%-50%) + 寿元(50-100 年)
- [ ] **全服可见**：所有化虚 action 触发全服 narration("某化虚者镇压了 X 坍缩渊")
- [ ] **不破红线**：action 不允许"传送"、"复活"、"无成本生灵气"

---

## §1 化虚 action 列表

| Action | 效果 | 代价 | 冷却 |
|---|---|---|---|
| 镇压坍缩渊(`suppress_tsy`) | 延缓副本塌缩 1-3 in-game month | 真元 200 + 寿元 50 年 | 30 day |
| 引爆区域(`explode_zone`) | 强行升某 zone 灵气 → 然后爆(灵气 0)持续 6 month | 真元 300 + 寿元 100 年 | 90 day |
| 阻挡道伥扩散(`barrier`) | 在地理边界设阻断,持续 1 month | 真元 150 + 寿元 30 年 | 7 day |
| 道统传承(`legacy_assign`) | 死前指定继承人(死信箱/亡者博物馆刻名) | 死亡时一次性 | — |

---

## §2 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ⬜ | VoidActionRequest schema + 4 个 action 实装 + 真元/寿元消耗 | 化虚玩家可发起 action |
| **P1** ⬜ | 全服 narration + tiandao 评论(运维博弈) | 全服能感知化虚者作为 |
| **P2** ⬜ | 道统传承 UI + 亡者博物馆生平卷扩展 | 继承人可在死后获得遗物 |
| **P3** ⬜ | 化虚 action 历史记录 + library-web 公开页面 | 化虚者一生写入末法残土史 |

---

## §3 数据契约

- [ ] `server/src/cultivation/void/actions.rs` 4 action 实装
- [ ] `server/src/cultivation/void/legacy.rs` 道统传承
- [ ] `agent/packages/schema/src/void.ts` VoidActionRequest + LegacyEntry
- [ ] `client/.../void/VoidActionScreen.java` 化虚专属 UI(action 列表 + 代价显示)
- [ ] `library-web/src/pages/legacy/[name].astro` 化虚者一生页面

---

## §4 开放问题

- [ ] action 冷却(避免一个化虚者疯狂操作)?
- [ ] 引爆区域的 6 month "灵气 0"是否会逼走该 zone 全部玩家?
- [ ] 道统继承人是否可拒绝(强加身上的责任)?
- [ ] action 是否被其他化虚者反制(化虚 vs 化虚 PVP)?
- [ ] action 是否进 plan-void-quota 的"化虚活跃度" 检测(频繁 action 是否触发更多天劫)?

## §5 进度日志

- 2026-05-01：骨架创建。plan-gameplay-journey-v1 §N 派生。
