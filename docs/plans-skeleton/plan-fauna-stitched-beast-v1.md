# Bong · plan-fauna-stitched-beast-v1 · 骨架

**异变缝合兽行为实装**——给 `HybridBeast`（当前有基础 stats：HP=400，攻击范围 3，`FaunaVisualKind::HybridBeast`）添加**融合触发机制**（普通野兽极度饥饿后互相吞噬融合）和**灵压狂暴**特性（HP% 越低 → 真元吸收率越快），击杀后兽核吸收触发幻觉 HUD 效果（UI/信息干扰）。

**来源**：`plan-rat-v1` ✅ §Q-RT-4 注记"待立 `plan-fauna-mimic-spider-v1` / `plan-fauna-stitched-beast-v1`（同源 sibling）"+ worldview §七 异变缝合兽描述

**交叉引用**：`plan-fauna-v1.md` ✅（HybridBeast drop 体系：兽核 item + `item.beast.core_mutant`）· `plan-rat-v1.md` ✅（`negative_pressure_avoidance`：Rat 见 HybridBeast 逃跑，本 plan 激活时反向依赖）· `plan-npc-ai-v1.md` ✅（big-brain Scorer/Action）· `plan-qi-physics-v1.md` ✅（守恒律：灵压狂暴吸收灵气必须走 ledger）· `plan-cultivation-v1.md` ✅（兽核吸收触发境界推进 / 幻觉效果接口）· `plan-vfx-v1.md` ✅（VFX 粒子框架）

**worldview 锚点**：
- **§七:725 异变缝合兽**：不是凭空刷新——由几只普通野兽在极度饥饿下互相吞噬融合而成。可见猪体/狼头/蜘蛛腿的拼接形态。特殊能力「灵压狂暴」：血量越低对周围区块灵气吸收越快。不速战 → 周围变负灵域 → 用环境压强耗死你。击杀掉落兽核是突破必需品，但吸收时引发强烈幻觉（UI/信息干扰）
- **§七 生态联动**：大迁徙中 HybridBeast 是"高阶生物"（rat 躲避它）；战胜场的遗骸可能成为 plan-tsy 系列的特殊素材
- **§十 资源与匮乏**：异变兽核 = 固元→通灵突破必需，极稀；高危高值

**qi_physics 锚点**：
- **灵压狂暴**（核心机制）：HybridBeast HP% 越低，对周围 zone 的灵气吸收率越高：
  ```
  absorption_rate(hp_pct) = BASE_RATE * (1.0 + RAGE_MULTIPLIER * (1.0 - hp_pct))
  ```
  所有吸收必须走 `qi_physics::regen_from_zone(zone.spirit_qi, rate, integrity, room)` + `QiTransfer(reason: CultivationRegen)`
- **融合时灵气守恒**：N 只野兽融合为 HybridBeast 时，各野兽 `qi_current` 加和转移到 HybridBeast（走 `QiTransfer(reason: FusionMerge, from: beast_i, to: hybrid)`）
- **死亡 qi 释放**：走 `qi_physics::qi_release_to_zone`（fauna-v1 已有路径）

**前置依赖**：
- `plan-fauna-v1` ✅ — HybridBeast 基础 stats + `item.beast.core_mutant` drop + `BeastKind::HybridBeast`
- `plan-npc-ai-v1` ✅ — big-brain Scorer/Action 框架
- `plan-rat-v1` ✅ — `negative_pressure_avoidance` 已留 HybridBeast 延伸口（Rat 逃跑触发器）
- `plan-qi-physics-v1` P1 ✅ — ledger API 冻结（灵压狂暴守恒依赖）

**反向被依赖**：
- `plan-anqi-v2` ✅ active — 异变兽骨载体（`anqi.carrier.mutant_bone`，来自 HybridBeast 的骨骼 drop，本 plan 完善行为后掉落率调参）
- `plan-poi-novice-v1` ✅ — 异变兽巢 POI（v1 zombie 占位，本 plan 完成后替换真实 HybridBeast）
- `plan-world-ecology-events-v1` ✅ — 大迁徙事件（HybridBeast 参与格局，本 plan 行为完善后大迁徙更有意义）

---

## 接入面 Checklist

- **进料**：`BeastKind::HybridBeast`（fauna/components.rs，HP=400，攻击范围 3）+ `BeastKind::all_wild()` 列表（待融合候选：Rat/Spider/Pig 等）+ zone `spirit_qi < HUNGER_THRESHOLD`（融合触发条件：低灵气 = 饥饿）+ `Cultivation.qi_current`（各野兽真元状态）
- **出料**：`HybridBeastFormationEvent { component_entities: Vec<Entity>, zone: ZoneId, fused_at: u64 }` + `HybridBeastRageState { hp_pct: f32, rage_absorption_rate: f32 }` component + `CoreAbsorptionHallucinationEvent { player, duration_ticks: u32 }` + VFX `bong:vfx/hybrid_formation` + VFX `bong:vfx/hybrid_rage`
- **共享类型**：复用 `BeastKind::HybridBeast` / `QiTransfer` / big-brain Scorer/Action；新增 `HybridBeastFormationEvent` / `HybridBeastRageState` / `CoreAbsorptionHallucinationEvent`
- **跨仓库契约**：server `bong:vfx/hybrid_formation` + `bong:vfx/hybrid_rage`（client VFX）；client `CoreAbsorptionHallucinationEvent` 触发 HUD 扭曲效果；agent NpcDigest 包含 HybridBeast 实体（无新字段，现有 realm/position 字段足够）
- **worldview 锚点**：§七:725 缝合兽 + §十 极稀资源（兽核）
- **qi_physics 锚点**：融合守恒 `QiTransfer(FusionMerge)` / 灵压狂暴 `regen_from_zone` / 死亡 `qi_release_to_zone`

---

## §0 设计轴心

- **融合触发**：同 zone 内 `N ≥ FUSION_MIN_BEASTS`（推 3）只野兽（BeastKind ∈ wild_list）在连续 `FUSION_HUNGER_TICKS`（推 600 tick = 30s）内处于低灵气区（`zone.spirit_qi < HUNGER_THRESHOLD`，推 0.05）→ emit `HybridBeastFormationEvent`，组件野兽消亡，生成新的 HybridBeast entity
- **灵压狂暴**（核心机制）：`HybridBeastRageState` component 动态更新 qi 吸收率：
  ```
  BASE_HYBRID_ABSORPTION_RATE = 0.002  (per tick, 正常状态)
  RAGE_MULTIPLIER = 3.0                (HP=0 时是正常的 4 倍)
  absorption_rate = BASE * (1 + RAGE_MULTIPLIER * (1 - hp_pct))
  ```
  全程走 `regen_from_zone` + `QiTransfer` 守恒
- **不速战后果**：zone.spirit_qi 被快速吸干后跌破 0 → 进入负灵域（zone physics 规则，不是本 plan 新定义）→ 玩家 HP/qi 受负压损耗（plan-qi-physics-v1 规则）→ "利用环境压强耗死你"物理实现
- **兽核幻觉**：玩家吸收 `item.beast.core_mutant` → server emit `CoreAbsorptionHallucinationEvent { duration_ticks: 200 }`（10s）→ client 激活 HUD 幻觉层（视野扭曲 + 伪造 HP/qi 读数随机偏移 + 音效噪声）

---

## 阶段总览

| 阶段 | 状态 | 主要交付物 | 验收标准 |
|------|------|-----------|---------|
| **P0** | ⬜ | 融合触发数据模型 + 融合参数决策门 | `HybridBeastFormationEvent` 数据模型 PR + ≥ 8 单测 green |
| **P1** | ⬜ | 融合 system（野兽→HybridBeast） + qi 守恒 | 3 只野兽低灵气融合 → HybridBeast 生成，qi 守恒 |
| **P2** | ⬜ | 灵压狂暴 system + zone 灵气吸干效果 | HP 越低吸收率越高；zone spirit_qi 正确下降 |
| **P3** | ⬜ | 兽核幻觉 HUD + VFX + client 整合 | 吸收兽核后 client HUD 幻觉效果正确触发 |

---

## P0 — 融合触发数据模型

- [ ] `HybridBeastFormationEvent { component_entities: Vec<Entity>, zone: ZoneId, fused_at: u64, qi_merged: f64 }` event（`server/src/fauna/hybrid_beast.rs`）
- [ ] `FUSION_MIN_BEASTS: usize = 3`（融合最低野兽数量，P0 决策门）
- [ ] `FUSION_HUNGER_TICKS: u64 = 600`（连续低灵气时间，P0 决策门）
- [ ] `HUNGER_THRESHOLD: f64 = 0.05`（低灵气判定，P0 决策门）
- [ ] `HybridBeastRageState { hp_pct: f32, rage_absorption_rate: f32 }` component
- [ ] `RAGE_MULTIPLIER: f32`（P0 决策门，推 3.0；即 HP=0 时 4× BASE_RATE）
- [ ] 融合前 qi 加和：`sum(beast.qi_current)` → 新 HybridBeast 初始 `qi_current`（不超 `qi_max = 400 × qi_density_factor`）
- [ ] ≥ 8 单测（融合条件满足触发 / 未满足不触发 / 融合 qi 守恒：sum(beast_qi) == hybrid_qi + ledger_QiTransfer / HybridBeastFormationEvent 序列化）

**P0 验收**：数据模型 PR 合并 + 8 单测 green

---

## P1 — 融合 system

- [ ] `hybrid_beast_formation_system`（FixedUpdate，`server/src/fauna/hybrid_beast.rs`）：
  - 按 zone 聚合 wild beast 列表，统计低灵气区内连续饥饿 tick
  - 满足条件 → emit `HybridBeastFormationEvent`
  - 组件野兽：`qi_physics::qi_release_to_zone(qi_current * FUSION_RELEASE_RATIO, ...)` + despawn（残余 qi 归还 zone，不全部转移防止守恒异常）
  - 新 HybridBeast spawn：`qi_current = sum(beasts_qi) * FUSION_RETAIN_RATIO`（推 0.8，20% 在融合过程中逸散到 zone）
  - 每次融合 emit `QiTransfer(reason: FusionMerge)`（N+1 条：N 个 from beast_i + 1 个逸散到 zone）
- [ ] 融合 VFX（`bong:vfx/hybrid_formation`）：
  - `BongRibbonParticle`：count=24，颜色 `#A07058`（HybridBeast 颜色）从各组件兽位置汇聚到中心，lifetime 20 tick，spawn_mode continuous（20 tick），速度 2.0m/s 向中心
  - 音效 recipe：`entity.generic.hurt`（×3 叠加），pitch 0.5/0.6/0.7，volume 1.0/0.8/0.6，delay 0/4/8
- [ ] rat 逃跑联动：`negative_pressure_avoidance` 延伸口（rat-v1 P1 §3 已预留）—— HybridBeast spawn 后向周围 24 格 Rat emit phase change → Transitioning（群逃跑，模拟"鼠群在缝合兽进入视野前四分之一炷香便知道了"）
- [ ] ≥ 12 单测（融合 spawn 正确 / qi 守恒：merge + release QiTransfer 完整 / Rat 逃跑触发 / 融合 VFX event 正确 emit）

**P1 验收**：测试 zone（spirit_qi=0.05，3 只 Rat hunger=600 tick）→ HybridBeast spawn → qi 守恒验证 → 周围 Rat 逃跑

---

## P2 — 灵压狂暴 system

- [ ] `hybrid_beast_rage_system`（FixedUpdate 2Hz，`server/src/fauna/hybrid_beast.rs`）：
  - 读取 HybridBeast HP → 更新 `HybridBeastRageState.hp_pct` = `current_hp / max_hp`
  - 计算 `rage_absorption_rate = BASE_HYBRID_ABSORPTION_RATE * (1.0 + RAGE_MULTIPLIER * (1.0 - hp_pct))`
  - 调用 `qi_physics::regen_from_zone(zone.spirit_qi, rage_absorption_rate, 1.0, qi_room)` + emit `QiTransfer(reason: CultivationRegen)`
- [ ] VFX 狂暴（`bong:vfx/hybrid_rage`，HP < 50% 时持续）：
  - `BongLineParticle`：count=8，颜色 `#FF4010`（血红），从 HybridBeast body 向外辐射，lifetime 12 tick，continuous，速度 1.5m/s 径向
  - HP < 25% 时颜色加深 `#FF0000`，count=16
  - 音效：持续低频嗡鸣 `block.deepslate.hit`，volume=(1.0-hp_pct)*0.8，pitch 0.4，loop interval 10 tick
- [ ] zone spirit_qi 被吸到 -0.1 以下时：server emit `ZoneEnteringNegativePressure` event（已有事件，检查是否存在）
- [ ] ≥ 12 单测（HP=100% 时 BASE_RATE / HP=50% 时 rate=BASE×2.5 / HP=25% 时 rate=BASE×3.25 / HP=0% 时 rate=BASE×4 / zone spirit_qi 正确下降 / QiTransfer 完整）

**P2 验收**：HybridBeast HP 从 100% 打到 10%，全程 zone spirit_qi 变化量 == ledger 累计 QiTransfer（守恒验证）

---

## P3 — 兽核幻觉 HUD

- [ ] `CoreAbsorptionHallucinationEvent { player_id: EntityId, duration_ticks: u32 }` event（`server/src/fauna/hybrid_beast.rs`）
- [ ] 兽核吸收接口（`server/src/cultivation/breakthrough.rs` 或 item use handler）：
  - 玩家使用 `item.beast.core_mutant` → 正常突破逻辑 + emit `CoreAbsorptionHallucinationEvent { duration_ticks: 200 }`
- [ ] client 幻觉 HUD（`client/src/hud/hallucination_layer.java`）：
  - 收到 `bong:core_absorption_hallucination` CustomPayload → 激活幻觉层（持续 200 tick = 10s）
  - 效果：视野轻微旋转（max ±3° yaw，sin wave，period 40 tick）+ 边缘彩色像差（`#80F040` 绿色边缘晕染）+ HP/qi bar 显示值随机 ±20% 偏移（每 10 tick 重新随机，不影响实际值）
  - fade in: 10 tick，fade out: 20 tick（最后 20 tick 退出）
  - 音效：`ambient.cave.cave1`（变速），pitch 0.5→1.2 渐变，volume 0.4
- [ ] narration（scope: player，style: perception）：
  - "兽核在经脉中炸开——你感到世界扭曲，影像重叠，真元猛地冲向意识深处。"（吸收瞬间）
  - "幻觉渐散。你感到境界在颤抖的边缘稳住了。"（幻觉结束）
- [ ] ≥ 8 单测（幻觉 event 正确 emit / 幻觉 duration 到期后 client 收到取消 payload / HP/qi 实际值不被幻觉改变 / narration scope=player）

**P3 验收**：e2e 手测——击杀 HybridBeast → 兽核掉落 → 玩家吸收 → 10s 幻觉 HUD 效果（含视野旋转 + HP bar 偏移）→ 幻觉结束 → 境界推进正常

---

## §8 开放问题（P0 决策门收口）

1. **融合所需野兽数量 N**：N=3 vs N=5（N 越大越稀有，但 zone 内 3 只野兽聚集已不常见，是否需要降到 2？）
2. **RAGE_MULTIPLIER**：3.0 意味 HP=0 时吸收率 4×（约 0.008/tick），zone spirit_qi 会在几秒内从 0.5 吸到 0；是否太激进？推 1.5（HP=0 时 2.5×）
3. **融合野兽种类**：是否限制只有特定 BeastKind 可融合（Rat+Rat / Rat+Spider / 任意组合）？worldview 说"猪的身体、狼的头颅、蜘蛛的腿"暗示跨种融合更正确
4. **融合可见**：融合过程 VFX 是否向玩家可见（玩家能看到野兽融合则可以打断 → 策略；不可见则发现时已是 HybridBeast）
5. **幻觉 HUD 强度**：10s 幻觉是否影响游戏性（玩家吸收兽核通常在安全地点 or 战斗中）？是否需要根据境界差（兽核阶级 vs 玩家境界）调整幻觉强度
6. **负灵域触发整合**：灵压狂暴把 zone 吸成负灵域时，是否触发 plan-zone-environment-v1 ✅ 的 `EnvironmentEffect::NegativePressure` 视觉层（可见的负压环境效果）