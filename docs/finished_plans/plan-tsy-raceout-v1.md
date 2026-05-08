# Bong · plan-tsy-raceout-v1 · Active

> **状态**：⏳ active（2026-05-04 升级，user 拍板）。前置 plan-tsy-lifecycle-v1 / plan-tsy-extract-v1 / plan-tsy-hostile-v1 / plan-death-lifecycle-v1 全 ✅ finished。§4 全部 5 决策闭环（Q-RC1/RC2/RC3/RC4/RC5 详 §4）。

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
   - **Q-RC4 决策**：单裂口同时只许 1 人；挤的人撞墙必须换下一个裂口
   - **Q-RC5 决策**：race-out 期间允许 PVP（互相推下裂口 / 抢裂口）
6. 3 秒到 → 仍在副本内的玩家全部走标准死亡终结流水（**Q-RC3 决策 A+C 复合**）：
   - 进 `plan-death-lifecycle-v1::terminate_character`
   - 寿元 -5%（化虚额外 -100 年，worldview §十二）
   - 干尸转道伥（复用 plan-tsy-hostile-v1 ✅ CorpseEmbalmed 链路）
   - **inventory 全副留副本**——副本化死域后无人能取，自然消失（与 worldview §十六"塌缩只喷干尸不喷物品"自洽）
7. ~~化虚玩家额外惩罚~~ 已并入 Step 6 标准流水（按 worldview §十二 寿元等比扣）
```

---

## §3 数据契约

- [ ] `server/src/world/tsy_raceout.rs` 3 秒撤离逻辑(短 timeout 路径)
- [ ] `server/src/world/tsy_collapse_rifts.rs` 3-5 裂口生成
- [ ] `client/.../tsy/RaceoutCountdownHud.java` 3 秒倒计时 UI(红色高警告)
- [ ] `agent/packages/tiandao/skills/calamity.md` race-out 专属 narration(并入 narrative-v1)

---

## §4 开放问题

- [x] **Q-RC1 ✅**（user 2026-05-04 A）：**统一 3s**——物理约束不是技能。化虚也死，符合 worldview §十六.六"化虚级也可能死"。
- [x] **Q-RC2 ✅**（user 2026-05-04 C）：**真随机 + 玩家可见远处闪光**——服务端真随机生成 3-5 个塌缩裂口位置（地形约束内），但 client 给所有玩家可见的闪光指示（远视觉钩）；不保证近，玩家自己跑。天道"不在乎你"语义保留。
- [x] **Q-RC3 ✅**（user 2026-05-04 A+C 复合）：**走标准死亡终结流水**——慢一秒玩家:
  - 进 plan-death-lifecycle-v1::terminate_character 终结流水（寿元正常扣 -5%；化虚额外 -100 年按 worldview §十二）
  - 干尸转道伥（plan-tsy-hostile-v1 ✅ CorpseEmbalmed → 道伥转化链路，无新代码）
  - **inventory 全副留副本**——副本化死域后无人能取，自然消失（与 worldview §十六"塌缩外溢只喷出干尸不喷物品"自洽）
- [x] **Q-RC4 ✅**（user 2026-05-04 B）：**单裂口同时只许 1 人**——先到先得，挤的人撞墙（撞不进，必须找下一个裂口；增加 race-out 紧迫感）。3-5 裂口足够 ≤5 人副本同时撤；规模更大副本会有人被卡死，符合 worldview"塌缩残忍"。
- [x] **Q-RC5 ✅**（user 2026-05-04 A）：**允许 PVP**——race-out 期间互相推下裂口/抢裂口可正常 PK。"末土残忍"设计：盟友也可能在塌缩瞬间反目。需 telemetry 监控（plan-style-balance-v1 联动），评估是否过度恶化合作环境。

## §5 进度日志

- 2026-05-01：骨架创建。plan-gameplay-journey-v1 §M.1 派生。
- **2026-05-04**：skeleton → active 升级（user 拍板）。§4 全部 5 决策闭环：
  - 3s 统一（化虚也死，符合 worldview §十六.六）
  - 真随机裂口 + 远视觉闪光指示（不保证近）
  - 慢一秒 = 标准死亡终结流水（terminate_character + 寿元 -5%/-100 年 + 干尸转道伥 + inventory 留副本）
  - 单裂口 1 人（先到先得 / 撞墙换下一个）
  - race-out 期间允许 PVP
  下一步起 P0 worktree（tsy_raceout.rs 短 timeout 路径 + 3-5 裂口生成 + 客户端红色倒计时 HUD）。
