# Bong · plan-fauna-mimic-spider-v1 · 骨架

**拟态灰烬蛛行为实装**——给 `AshSpider` / `MimicSpider`（当前映射 `BeastKind::Spider`，仅有基础 spawn + drop 逻辑）添加 `SpiderDisguiseState` 三态机（Disguised / Ambush / Retreat），实装 worldview §七"外观与残灰方块一致、感知真元暴起发难"的物理逻辑，并留接口支持"玩家留活蛛阴追兵"策略玩法。

**来源**：`plan-rat-v1` ✅ §Q-RT-4 注记"待立 `plan-fauna-mimic-spider-v1` / `plan-fauna-stitched-beast-v1`（同源 sibling；RatPhase / PressureSensor 设计要给那两 plan 留延伸口）"

**交叉引用**：`plan-fauna-v1.md` ✅（MimicSpider drop 体系：骨刺 / 蛛骨 / 蛛毒囊）· `plan-rat-v1.md` ✅（RatPhase 三态 + PressureSensor 模式参考；`negative_pressure_avoidance` 组件已留 Spider 延伸口）· `plan-npc-ai-v1.md` ✅（big-brain Scorer/Action 框架）· `plan-spirit-eye-v1.md` ✅（神识识破伪装检测入口）· `plan-anqi-v1.md` ✅（骨刺作为暗器载体来源）· `plan-qi-physics-v1.md` ✅（守恒律；Disguised 期蛛真元微量吸收走 ledger）

**worldview 锚点**：
- **§七:721 拟态灰烬蛛**：外观与残灰方块完全一致，休眠趴地；感知真元玩家经过上方时暴起发难；老玩家会故意留活蛛阴追兵（策略性陷阱用途）
- **§七 生态联动**：拟态灰烬蛛出没于死域边缘（`spirit_qi < 0` 的 zone 边界），与死域地表方块融为一体
- **§七 道伥规则类比**：Disguised 蛛对玩家神识（SpiritEye）有特殊扰动（真元噪声增加但不完全隐藏）

**qi_physics 锚点**：
- Disguised 期：蛛以极低速率（1/100 正常速率）从 zone 微量吸收灵气维持伪装态，走 `qi_physics::regen_from_zone` + `QiTransfer(reason: CultivationRegen)`
- Ambush 期：蛛 qi 消耗（攻击 + 移动）按正常速率走现有战斗逻辑
- 死亡时 qi 释放走 `qi_physics::qi_release_to_zone`（fauna-v1 已有路径）

**前置依赖**：
- `plan-fauna-v1` ✅ — MimicSpider item drop 体系（骨刺/蛛骨/蛛毒囊）、`FaunaKind::MimicSpider`
- `plan-npc-ai-v1` ✅ — big-brain Scorer/Action 框架（参考现有 `ZhiNian` 类 ambush scorer `tsy_hostile.rs:1354`）
- `plan-rat-v1` ✅ — RatPhase 三态模式参考；`negative_pressure_avoidance` 已为 Spider 留接口
- `plan-spirit-eye-v1` ✅ — 神识视野系统（P2 神识识破 Disguise 依赖此接口）
- `plan-qi-physics-v1` P1 ✅ — ledger API 冻结

**反向被依赖**：
- `plan-anqi-v2` ✅ — 骨刺暗器 v2（蛛骨刺/蛛丝改进版来自活蛛剥取，本 plan 留 `SpiderTrapAlive` 接口）
- `plan-botany-v2` — 植物区危险因子（MimicSpider 在 botany hazard registry 已注册，行为升级后危险度提升）

---

## 接入面 Checklist

- **进料**：`FaunaKind::MimicSpider`（botany/registry.rs 注册）+ `BeastKind::Spider`（fauna/components.rs，当前映射目标）+ zone `spirit_qi < DEAD_ZONE_THRESHOLD`（伪装激活条件）+ `PlayerInspectRange`（感知范围，spirit-eye-v1）+ `Cultivation.qi_current`（玩家真元，触发暴起的感知目标）
- **出料**：`SpiderDisguiseState`（component，三态）+ `SpiderAmbushTriggerEvent { spider: Entity, target: Entity }` + `SpiderTrapPotential`（留活接口 component，供 anqi-v2 消费）+ 新 VFX event `bong:vfx/spider_ambush`
- **共享类型**：复用 `BeastKind::Spider` / `QiTransfer` / 现有 big-brain scorer 框架；新增 `SpiderDisguiseState` / `SpiderAmbushTriggerEvent`
- **跨仓库契约**：server `bong:vfx/spider_ambush`（client VFX）；agent NpcDigest 自然包含 Spider entity（无新字段）；client 收到 Spider 低 LOD 时显示为地面方块纹理（Disguised 期特殊渲染，P2 实装）
- **worldview 锚点**：§七:721 拟态灰烬蛛
- **qi_physics 锚点**：Disguised 微量吸收 `regen_from_zone` / 死亡 `qi_release_to_zone`

---

## §0 设计轴心

- **三态机**（类比 RatPhase）：
  - `Disguised`：趴地伪装（同地表残灰方块颜色 `#B8D0C8`）；entity 可见但 client 渲染为方块形态；player 正常行走会踩过去
  - `Ambush`：感知到 `Cultivation.qi_current > SPIDER_QI_SENSE_THRESHOLD` 的玩家在 `SPIDER_SENSE_RADIUS`（8 格）内 → 暴起，进入全速战斗模式
  - `Retreat`：蛛 HP < 20% 且距离玩家 > 4 格时逃跑；Retreat 结束条件：逃出 `SPIDER_RETREAT_RADIUS`（32 格）or 逃进 `spirit_qi < -0.3` 的 zone 边缘（重新进入 Disguised）
- **留活接口**：玩家可对 Disguised 态蛛用"不击杀"动作（如 anqi 的"压制"或道具 `蛛制陷阱笼`）捕获，获得 `SpiderTrapPotential` component；用于布置陷阱（追兵踩到触发 Ambush，对追兵生效）
- **神识识破**：玩家 SpiritEye 激活时，Disguised 蛛在神识视野内显示为"蛛形轮廓（橙色）"而非地面方块
- **VFX 暴起**：Disguised → Ambush 瞬间：`BongSpriteParticle` burst 16 个残灰方块碎片（颜色 `#B8D0C8`，速度向外径向，lifetime 8 tick）+ 音效 `entity.spider.step`（pitch 1.8，volume 0.6）

---

## 阶段总览

| 阶段 | 状态 | 主要交付物 | 验收标准 |
|------|------|-----------|---------|
| **P0** | ⬜ | SpiderDisguiseState 三态数据模型 + 感知触发器设计 | 数据模型 PR 合并 + ≥ 8 单测 green |
| **P1** | ⬜ | 暴起 + 伏击 AI（big-brain Scorer/Action）+ VFX | 蛛感知玩家 → 暴起 → 战斗正确执行 |
| **P2** | ⬜ | Retreat 态 + 神识识破 + client 伪装渲染 | SpiritEye 激活可见蛛轮廓；退出战斗重回伪装 |
| **P3** | ⬜ | 留活接口 + SpiderTrapPotential + anqi-v2 钩子 | 玩家压制活蛛、布置陷阱对追兵生效 |

---

## P0 — SpiderDisguiseState 数据模型

- [ ] `SpiderDisguiseState` enum（`server/src/fauna/mimic_spider.rs`）：
  ```rust
  pub enum SpiderDisguiseState { Disguised, Ambush, Retreat }
  ```
- [ ] `MimicSpiderBlackboard { state: SpiderDisguiseState, ambush_target: Option<Entity>, retreat_start_tick: u64 }` component
- [ ] `SPIDER_QI_SENSE_THRESHOLD: f32`（感知触发真元阈值，P0 决策门：推 0.1 qi_max 占比）
- [ ] `SPIDER_SENSE_RADIUS: f32`（感知半径，推 8.0 格；同 worldview §七"经过上方"的物理半径）
- [ ] `SPIDER_RETREAT_RADIUS: f32`（逃跑阈值，推 32.0 格）
- [ ] Disguised 期 qi 吸收：`regen_from_zone(zone.spirit_qi, SPIDER_DISGUISE_REGEN_RATE, 1.0, qi_room)` 其中 `SPIDER_DISGUISE_REGEN_RATE = 0.001`（正常速率 / 100）+ emit `QiTransfer`
- [ ] ≥ 8 单测（三态转换条件 / Disguised 期 qi 吸收守恒 / 感知阈值边界 / Retreat 距离判断）

**P0 验收**：数据模型 PR 合并 + 8 单测 green（无 AI 行为，仅数据结构 + 感知触发器逻辑）

---

## P1 — 暴起 AI + VFX

- [ ] `SpiderAmbushScorer`（big-brain Scorer，`server/src/fauna/mimic_spider.rs`）：
  - 当 `SpiderDisguiseState::Disguised` + 范围内玩家 `qi_current > SPIDER_QI_SENSE_THRESHOLD` → score 1.0
  - 其他情况 score 0.0
- [ ] `SpiderAmbushAction`（big-brain Action）：
  - 激活时：`SpiderDisguiseState → Ambush` + emit `SpiderAmbushTriggerEvent` + emit VFX event
  - 执行：追击目标（复用现有 `ChaseAndAttack` action，priority 提升）
  - 退出：目标死亡 or 蛛 HP < 20% → 转 Retreat
- [ ] `SpiderRetreatAction`（big-brain Action）：
  - 逃跑路径：向 `spirit_qi < -0.2` 方向逃（按 zone spirit_qi 梯度寻路）
  - 到达 SPIDER_RETREAT_RADIUS 外 or 进入低 spirit_qi zone → `SpiderDisguiseState → Disguised`
- [ ] VFX 暴起（`bong:vfx/spider_ambush`）：
  - `BongSpriteParticle`：count=16，color `#B8D0C8`，速度径向向外 2.0m/s，lifetime 8 tick，spawn_mode burst，贴图复用 `ash_fragment`
  - 音效 recipe：`entity.spider.step`，pitch 1.8，volume 0.6，delay 0
- [ ] ≥ 12 单测（Ambush scorer 阈值边界 / 追击正确执行 / Retreat 方向朝 spirit_qi 负区 / VFX event 正确 emit）

**P1 验收**：测试 zone（spirit_qi=0.0）中 Disguised 蛛感知玩家（qi_current=100）→ 暴起追击 → 蛛 HP < 20% → Retreat → 找到 spirit_qi=-0.3 区域 → 重回 Disguised

---

## P2 — Retreat 稳定 + 神识识破 + client 渲染

- [ ] 神识识破（需 spirit-eye-v1 API）：
  - 玩家激活 SpiritEye 且 Disguised 蛛在神识视野（SPIRIT_EYE_RANGE 格）内 → server emit `RevealDisguisedSpider { spider, player }` event
  - client 收到 event → 对应 entity 渲染切换为"蛛形轮廓"（橙色 `#FF8040`，透明度 70%）
- [ ] client 伪装渲染（`client/src/hud/entity_disguise.java`）：
  - 收到 `bong:spider_disguise_enter` CustomPayload → entity 渲染为地面方块形态（`ash_block` 纹理）
  - 收到 `bong:spider_ambush_trigger` → 立即切换回正常 Spider 渲染
- [ ] Disguised 态蛛不出现在 client entity list（不触发 nameplate）——走 `EntityVisibilityFilter`
- [ ] ≥ 6 集成测试（SpiritEye 视野内识破 / SpiritEye 视野外不识破 / Ambush 切换渲染 / Disguised 时 client nameplate 不显示）

**P2 验收**：玩家未激活 SpiritEye 时蛛不可见；激活后可见轮廓；蛛暴起后渲染切换正常

---

## P3 — 留活接口

- [ ] `SpiderTrapPotential { trap_owner: Entity, placed_at: ChunkPos, placed_tick: u64 }` component（蛛被捕获后添加）
- [ ] 捕获动作：需 `item.spider_cage`（蛛制陷阱笼，anqi-v1 新增 item，或由本 plan P3 定义）对 Disguised 态蛛使用 → 蛛进入 Disguised-Trapped 子状态（不再自主暴起，等主人布置）
- [ ] 布置命令：玩家持被捕获蛛对地面 interact → 在该位置放置 Disguised-Trapped 蛛（`SpiderTrapPotential` 转移到 world entity）
- [ ] 陷阱触发：其他玩家（`trap_owner` 以外）进入感知范围 → 蛛暴起（Ambush）对该玩家（非原主人）攻击
- [ ] ≥ 8 单测（捕获成功 / 布置正确 / 陷阱仅对他人触发 / 陷阱超时（72h in-game）自动进入 Retreat 并释放）

**P3 验收**：e2e 手测——玩家捕获 Disguised 蛛 → 在路上布置 → 追兵玩家经过 → 蛛暴起对追兵攻击 → 原主人通过

---

## §8 开放问题（P0 决策门收口）

1. **感知阈值**：`SPIDER_QI_SENSE_THRESHOLD = 0.1 × qi_max` 是否合理（太低 = 蛛对空场玩家也暴起；太高 = 蛛永不触发）
2. **伪装视觉实现**：Disguised 蛛渲染为 ash_block 纹理 vs 完全隐形（invisible packet）vs 半透明（两者各有 client 复杂度权衡）
3. **多蛛同巢**：同一区域多只 Disguised 蛛是否同步暴起（聚集效果）or 各自独立感知
4. **陷阱笼 item**：由本 plan 定义 `item.spider_cage` or 归 anqi-v2 定义（材料链归属问题）
5. **蛛骨刺陷阱 vs 活蛛陷阱**：两种陷阱是否共存（anqi-v1 已有骨刺抛掷，活蛛陷阱是新玩法维度；是否过度复杂）