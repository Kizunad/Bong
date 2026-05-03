# Bong · plan-multi-life-v1 · Active

> **状态**：⏳ active（2026-05-04 升级，user 拍板 + 6 决策 + reframe 对齐 lifespan-v1）。前置 plan-death-lifecycle-v1 ✅ finished + plan-tsy-loot-v1 ✅ finished + plan-spawn-tutorial-v1 ✅ finished + plan-lifespan-v1 ⏳ active（寿元数据 single source of truth）。
>
> **2026-05-04 reframe**（Q-ML0 A）：multi-life 不再独立维护寿元上限 / 死亡扣寿口径——**全部引用 lifespan-v1 §2**（`LifespanCapTable` + 死亡扣寿公式）。本 plan 仅管"角色终结后怎样开新角色"。

多周目机制:**per-life 运数**(每角色独立 3 次,**不跨角色累计**) + 寿命归零强制重开（依 lifespan-v1）+ 知识继承(玩家脑内 + 亡者博物馆) + 实力归零。**对应 plan-gameplay-journey-v1 §M.3**。

**世界观锚点**：`worldview.md §十二 死亡重生与一生记录` · `§十二 运数/劫数 Roll`（寿元上限引 lifespan-v1）

**交叉引用**：`plan-lifespan-v1` ⏳(寿元数据 source of truth，本 plan 全引用) · `plan-death-lifecycle-v1` ✅(运数实装,本 plan 检查是否 per-life) · `plan-tsy-loot-v1` ✅(道统遗物随机分散) · `plan-spawn-tutorial-v1` ✅(新角色出生 spawn_plain) · `plan-gameplay-journey-v1` §M.3/O.4/O.11

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
- [ ] **不允许跳过教学**：破坏 worldview 末法残土设定,新角色必须从醒灵走 + 第二世仍 spawn_plain（Q-ML4 决议）
- [ ] **无家族 / 无姓氏概念**（Q-ML1 B）：每个角色完全独立，避免出戏。亡者博物馆按 player_id 历代列出，但不归类家族
- [ ] **不允许主动放弃**（Q-ML2 A）：还有寿元 / 运数时不可主动重开——破坏末土残忍设定（worldview §十二"寿元宽裕"原意是逼人前进，主动重开会消解）
- [ ] **前世坐标不传承**（Q-ML3 B+ 精细化）：服务器不存"前世坐标"、客户端不显示——玩家自己脑内记。**前世物资 / 基地服务器不主动清除**（自然存在），**也不主动保护**——前世死后的物资箱子 / 基地可能被 NPC / 其他玩家搜刮 / 占据，玩家回去找发现"已被人翻过"是合理叙事（worldview §十"末土资源被翻是常态"）
- [ ] **道统遗物仅随机分散**（Q-ML5 A）：不做 plan-void-actions-v1 legacy_assign 化虚指定继承人。统一随机分散到 4 tsy 副本（worldview §十二"道统遗物天道分配"）

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

> **2026-05-04 Q-ML0 reframe**：寿元上限 + 死亡扣寿公式**全部引用 plan-lifespan-v1 §2**（`LifespanCapTable` 是 single source of truth）。本 plan 不另写一套数值。

```
角色死亡:
  寿元 -= 死亡扣除值（按 plan-lifespan-v1 §2，例如：被杀 = 当前境界寿元上限 × 5%；
                     渡劫失败 -100 年；老死即终结）
  运数池 -= 1(运数 > 0 时 100% 重生在灵龛)
  寿元 = 0 OR 运数耗尽 → 角色终结
   → LifeRecord 写入亡者博物馆（按 player_id 历代列出，不归类家族 Q-ML1）
   → NaturalDeathCorpse / 战斗死遗骸就地生成（plan-lifespan-v1 §5）
   → 前世物资 / 基地不主动清除（Q-ML3 决议，可能被 NPC/玩家搜刮）
   → 道统遗物随机分散到 4 tsy 副本（Q-ML5，不做 legacy_assign 指定继承人）
   → 玩家进入 character_select（不允许主动放弃 Q-ML2，仅在终结后进入）
   → 新角色生成: Realm = Awaken, 运数 = 3, 寿元 = 醒灵 cap（引 LifespanCapTable）,
                  spawn 位置 = spawn_plain（Q-ML4，与新玩家相同）, 物品/真元/境界 = 0

寿元上限:
  → 引 plan-lifespan-v1 §2 LifespanCapTable（醒灵 120 → 化虚 2000，详 lifespan plan）
```

---

## §3 数据契约

- [ ] `server/src/cultivation/lifespan.rs::on_death` 死亡扣寿（plan-lifespan-v1 P0 实施，本 plan **不重复**）
- [ ] `server/src/cultivation/character_lifecycle.rs::regenerate_or_terminate` 重生/终结判定（本 plan P0 主体）
- [ ] `server/src/cultivation/character_select.rs` 新角色生成 + spawn_plain 出生（Q-ML4）
- [ ] `server/src/cultivation/luck_pool.rs` per-life 运数池（**P0 验证 plan-death-lifecycle-v1 已实装是否 per-character**；不是则改）
- [ ] `agent/.../skills/era.md` "新一世" 开场 narration(协调 plan-spawn-tutorial-v1)
- [ ] `library-web/src/pages/lives/[player_id].astro` 历代生平统计（按 player_id，不按家族 Q-ML1）

---

## §4 开放问题

- [x] **Q-ML0 ✅**（user 2026-05-04 A，reframe）：完全引用 lifespan-v1 §2 寿元数据 + 扣寿公式（不另写一套）
- [x] **Q-ML1 ✅**（user 2026-05-04 B）：**无家族 / 无姓氏概念**——亡者博物馆按 player_id 历代列出
- [x] **Q-ML2 ✅**（user 2026-05-04 A）：**不允许主动放弃**——还有寿元 / 运数时不可主动重开
- [x] **Q-ML3 ✅**（user 2026-05-04 B+ 精细化）：**不传承前世坐标**——服务器不存 / 客户端不显示，玩家自己脑内记。**前世物资 / 基地服务器不主动清除**（自然存在）也不主动保护——可能被 NPC / 玩家搜刮
- [x] **Q-ML4 ✅**（user 2026-05-04 A）：第二世新角色出生位置 = spawn_plain（与新玩家相同）
- [x] **Q-ML5 ✅**（user 2026-05-04 A）：**仅随机分散** 4 tsy 副本，不做 plan-void-actions-v1 legacy_assign 化虚指定继承人

> **本 plan 无未拍开放问题**——P0 可立刻起。

## §5 进度日志

- 2026-05-01：骨架创建。plan-gameplay-journey-v1 §M.3 / O.4 / O.11 决策落点。
- **2026-05-04**：skeleton → active 升级（user 拍板）。**6 决策闭环 + reframe**：
  - Q-ML0 reframe：与 lifespan-v1 §2 数值对齐，不另维护
  - Q-ML1 无家族（亡者博物馆按 player_id）
  - Q-ML2 不允许主动放弃
  - Q-ML3 前世坐标不传承，物资不主动清也不主动保护（"末土残忍"）
  - Q-ML4 第二世仍 spawn_plain
  - Q-ML5 道统遗物仅随机分散（不做 legacy_assign）
  - 关键修正：原 §2 寿元上限表 (80/150/300/500/1000/2000) → 引 lifespan-v1 §2 LifespanCapTable (120/200/350/600/1000/2000)
  - 下一步起 P0 worktree（character_lifecycle::regenerate_or_terminate + 验证 luck_pool per-character + character_select spawn_plain 路径）
