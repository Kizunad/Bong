# Bong · plan-multi-life-v1 · 骨架

多周目机制:**per-life 运数**(每角色独立 3 次,**不跨角色累计**) + 寿命归零强制重开 + 知识继承(玩家脑内 + 亡者博物馆) + 实力归零。**对应 plan-gameplay-journey-v1 §M.3**。

**世界观锚点**：`worldview.md §十二 死亡重生与一生记录` · `§十二 运数/劫数 Roll` · `§十二 寿元`(凡人 80 → 化虚 2000 年)

**交叉引用**：`plan-death-lifecycle-v1` ✅(运数实装,本 plan 检查是否 per-life) · `plan-tsy-loot-v1` ✅(道统遗物随机分散) · `plan-spawn-tutorial-v1` ⬜(新角色出生) · `plan-gameplay-journey-v1` §M.3/O.4/O.11

---

## 接入面 Checklist

- **进料**：玩家死亡事件 + 当前角色寿元 + 运数池
- **出料**：角色终结后的新角色生成 + 满运数(3 次)重置 + 知识继承(亡者博物馆生平卷可读)
- **共享类型**：复用 `LifeRecord` ✅ + `Aging` ✅ + `Karma` ✅
- **跨仓库契约**：server character_lifecycle + agent legacy narration + client character_select UI
- **worldview 锚点**：§十二

---

## §0 设计轴心

- [ ] **per-life 运数**(O.4 决策)：每角色独立 3 次,**不跨角色累计**——每新角色满运数重置
- [ ] **化虚 per-life 可达**(O.11 决策)：n 世玩家化虚不受影响,只看本世能否走通
- [ ] **知识继承不影响实力**：亡者博物馆可读 → 后人玩家可用脑内知识缩短路径,但物品/真元/境界仍归零
- [ ] **不允许跳过教学**：破坏 worldview 末法残土设定,新角色必须从醒灵走

---

## §1 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ⬜ | 检查 plan-death-lifecycle-v1 运数是否 per-life;若不是则改 | 单元测试: 新角色满运数 3 次 |
| **P1** ⬜ | 寿命归零强制重开流程 | 寿元归零玩家自动进 character_select |
| **P2** ⬜ | 道统遗物随机分散到 4 tsy 副本(已 ✅) + 知识继承 narration | 新角色可在副本拾到前世遗物 |
| **P3** ⬜ | 角色史(亡者博物馆) library-web 公开页面 + 同玩家历代统计 | 玩家可看自己历代生平 |

---

## §2 关键流程

```
角色死亡:
  寿元 -= 死亡扣除值
    渡劫失败:    -100 年
    被杀:        -5% 当前寿元
    老死:        -100% 即终结
  运数池 -= 1(运数 > 0 时 100% 重生在灵龛)
  寿元 = 0 OR 运数耗尽 → 角色终结
   → LifeRecord 写入亡者博物馆
   → 道统遗物随机分散到 4 tsy 副本
   → 玩家进入 character_select
   → 新角色生成: Realm = Awaken, 运数 = 3, 寿元 = 80 年, 物品/真元/境界 = 0

寿元上限:
  醒灵: 80 年
  引气: 150 年
  凝脉: 300 年
  固元: 500 年
  通灵: 1000 年
  化虚: 2000 年(real ~2000h)
```

---

## §3 数据契约

- [ ] `server/src/cultivation/lifespan.rs::on_death` 死亡扣寿(已 ✅,需检查 per-life)
- [ ] `server/src/cultivation/character_lifecycle.rs::regenerate_or_terminate` 重生/终结判定
- [ ] `server/src/cultivation/character_select.rs` 新角色生成
- [ ] `server/src/cultivation/luck_pool.rs` per-life 运数池(检查是否 per-character)
- [ ] `agent/.../skills/era.md` "新一世" 开场 narration(协调 plan-spawn-tutorial-v1)
- [ ] `library-web/src/pages/lives/[player_id].astro` 历代生平统计

---

## §4 开放问题

- [ ] 同玩家多角色之间是否有"姓氏/家族"概念(亡者博物馆按家族归类)?
- [ ] 玩家可主动放弃(还有寿元时主动重开)? 决策倾向: **不允许**——违反末法残土设定
- [ ] 知识继承是否包括"我家基地坐标"(玩家脑内记得,但坐标可能已被他人占据)?
- [ ] 第二世新角色出生位置: 仍 spawn_plain 还是其他?
- [ ] 道统遗物可否专门指定继承人(plan-void-actions-v1 的 legacy_assign)而非随机分散?

## §5 进度日志

- 2026-05-01：骨架创建。plan-gameplay-journey-v1 §M.3 / O.4 / O.11 决策落点。
