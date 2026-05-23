# Bong · plan-sword-path-v3 · 骨架

接续 sword-path-v2 明确 deferred 的三段——黑武士 BOSS AI 完整 ECS、全量 VFX/音效资产联调、剑意化形完整实体追踪、化虚天门剑四阶段时序。**plan-sword-path-v2 在本 plan 归档前不归档，继续 active。**

**来源**：`docs/plan-sword-path-v2.md` §遗留 / 后续（明确 deferred 范围）

**世界观锚点**：`worldview.md` §四（战力分层：飞剑型攻击 / 器修载体）· §六.1（剑修 → 锋锐色，染色加速）

**依赖 plan（前置必须完成）**：
- `plan-sword-path-v2` — 需达到「P0-P2 全 ✅ + 测试全绿」才允许启动本 plan

---

## 接入面 Checklist

- **进料**：`server/src/combat/sword_path/` ✅（v1+v2 全部模块） / `big-brain` Utility AI ✅ / `plan-vfx-v1` VfxEventRouter ✅ / `plan-audio-world-v1` PlaySoundRecipeRequest ✅ / `plan-entity-model-v1` BongEntityModelKind ✅（黑武士 fauna 模型已注册）/ `plan-npc-ai-v1` big-brain Scorer/Action 框架 ✅
- **出料**：`heiwushi` spawn + 行为树；`VfxEventRequest` 触发链路（server → client VfxPlayer）；`CombatAttackSourceV1` schema v2 新变体（需 agent 端 enum 同步）
- **共享类型**：新增 `HeiwushiState` Component / `SwordIntentEntity`（追踪实体）；复用 `AttackIntent` / `SwordBondComponent` / `CombatEvent`
- **跨仓库契约**：

| 层 | 新增 symbol |
|----|------------|
| server | `combat/sword_path/heiwushi.rs` — HeiwushiState + 6 Actions + 5 Scorers |
| server | `combat/sword_path/sword_intent_entity.rs` — SwordIntentEntity spawn + track |
| server | `combat/sword_path/heaven_gate_full.rs` — 4 阶段时序 |
| client | 9 个 Java VfxPlayer 类 + PlayerAnimator JSON x6 |
| agent | `CombatAttackSourceV1` v2 enum 变体反序列化 + narration 模板 |
| schema | proto `CombatAttackSourceV1` 新增专属 sword_path 变体 |

- **worldview 锚点**：§四「飞剑是末法残土稀有的远程手段之一」 / §六.2「剑修真元呈银白细丝，锋锐色 → 攻击穿透+，破护体真气效率高」

---

## 阶段总览

| 阶段 | 内容 | 状态 | 验收 |
|------|------|------|------|
| **P0** | 黑武士 BOSS AI：HeiwushiState + 6 Actions + 5 Scorers + spawn + 掉落 | ⬜ | 20 TPS 压测 10 只黑武士同屏 < 5ms/tick；掉落表测试 |
| **P1** | 剑意化形完整版：SwordIntentEntity 实体追踪 5s + 5 次伤害 | ⬜ | 单测：追踪 + 伤害序列 + 超时消失 |
| **P2** | 化虚天门剑全时序：蓄力60t → 临界60t 区域警告 → AoE60-80t → aftermath | ⬜ | 单测：4 阶段时间窗边界；client 收到 zone_warning VfxEventRequest |
| **P3** | VFX 资产包：13 粒子贴图 + 17 audio_recipe + 6 PlayerAnimator + 9 VfxPlayer + 联调 | ⬜ | server VfxEventRequest → client 端到端联调 9 个 VfxPlayer 全触发 |
| **P4** | 测试收尾：v1 遗留 9 单测 + e2e 集成 + InspectScreen 扩展 + schema v2 变体 | ⬜ | 全局 `cargo test` ≥ 5020 pass；schema 双端 sample roundtrip |

---

## §1 P0：黑武士 BOSS AI

### 6 Actions（行为）

| Action | 描述 | 触发条件 |
|---|---|---|
| `HeiwushiPatrol` | 在生成半径内巡逻，感知范围 16 格 | 无玩家在感知范围 |
| `HeiwushiChase` | 追踪目标（A* pathfinding） | 玩家在 16-32 格 |
| `HeiwushiMeleeSlash` | 近战重斩（剑路特攻）| 玩家在 3 格内，蓄力 0.5s |
| `HeiwushiSwordIntent` | 释放剑意化形追踪实体 | 玩家在 4-20 格，冷却 8s |
| `HeiwushiRetreat` | 受创撤退，距离 > 5 格 | HP < 30%，玩家贴近 |
| `HeiwushiDeath` | 死亡动画 + 掉落表 | HP = 0 |

### 5 Scorers（评分）

- `PlayerProximityScorer` — 距离权重（近 → 高分近战；远 → 高分剑意）
- `HealthRatioScorer` — 血量决策（低血选退）
- `SwordIntentCooldownScorer` — 技能冷却状态
- `ZoneQiDensityScorer` — 区域灵气（灵气 > 0.8 时黑武士攻击欲强）
- `TiandaoAttentionScorer` — 若目标 TiandaoAttention 高 → 黑武士主动回避（worldview §八 天道盲目看灵气聚集，高注意力玩家区域黑武士也躲）

### 成长周期

- 刷新时境界锁定（固元级 boss）
- 击杀后重刷间隔：72 in-game hours（plan-tsy-lifecycle-v1 的 zone 刷新节奏参考）
- 掉落：`scroll_sword_path`（残卷）+ `bone_coin_pack_small` + 概率 `black_sword_fragment`

---

## §2 P1：剑意化形完整实体

来自 v2 §P1.6 的 AttackIntent 占位，升级为真实追踪实体：

- `SwordIntentEntity` — server ECS Entity，携带 `Target(Entity)` + `LifetimeTicks(u64)` + `HitCount(u8)` + `DamagePerHit(f64)`
- 追踪 system：每 tick 向目标方向移动（最大速度 0.5 格/tick），5s（100 tick）超时销毁
- 命中：接触目标 3 格内 → 造成伤害 + `HitCount -= 1`；5 次后销毁
- VFX：每次命中触发 `VfxEventRequest { id: "sword_intent_hit" }` → client 渲染

---

## §3 P2：化虚天门剑全时序

来自 v2 §P2.1 的跳过设计，四阶段完整实装：

```
蓄力阶段  [0, 60 tick)   — server 发 VfxEventRequest{id:"heaven_gate_charge"}
临界阶段  [60, 120 tick) — server 发 zone_warning 给周围玩家（QI 颤动提示）
AoE 阶段  [120, 140 tick) — AoE 伤害结算，半径 8 格，qi_physics::QiTransfer 记账
Aftermath [140+]          — emit TechniqueAftermathEvent + VfxEventRequest{id:"heaven_gate_blast"}
```

**守恒要求**：AoE 真元消耗走 `qi_physics::ledger::QiTransfer { from: caster, to: Zone }`，不允许凭空消失（worldview §二 守恒律）

---

## §4 P3：VFX 资产包

来自 v2 §遗留，需实际创建的文件：

| 类型 | 数量 | 说明 |
|---|---|---|
| 粒子贴图 PNG | 13 | `client/src/main/resources/assets/bong/textures/particle/sword_*.png` |
| audio_recipe JSON | 17 | `server/src/audio/recipes/sword_*.json` |
| PlayerAnimator JSON | 6 | `server/src/animation/sword_*.json`（heiwushi.idle/walk/slash/death + sword_intent + heaven_gate_charge）|
| Java VfxPlayer 类 | 9 | 覆盖 sword path 全技能视效 |

**开发规范**：参照 docs/CLAUDE.md §六.1 三轮自我打磨，每个 VfxPlayer 类必须经过 round 1/3 → round 2/3 → round 3/3 + `<PROMISE>` 担保。

---

## §5 P4：测试收尾

### v1 遗留单测（来自 v2 §P5.1）

- `bond.rs` 5 个 TODO 测试
- `techniques.rs` 3 个 TODO 测试
- `tiandao_blind.rs` 1 个 TODO 测试

### 新增集成测试

- 黑武士 AI：20 TPS 压测（10 只同屏） + 掉落 roundtrip
- 剑意化形：追踪实体 5s 超时 + 5 次命中
- 化虚天门剑：4 阶段时序边界
- schema v2：`CombatAttackSourceV1` 新变体双端 sample 对照

### InspectScreen 扩展

- 灵剑信息：bond grade + technique 熟练度 + 剑意状态（v2 遗留）
- `SwordBondHudStateStore` client handler 接入

---

## §6 开放问题（P0 决策门前收口）

1. **黑武士成长周期**：固元级 boss 击杀后 72h 刷新是否合理？需对比 tsy-lifecycle-v1 的区域刷新节奏
2. **剑意化形追踪速度**：0.5 格/tick（10 格/s）在 20 TPS 下是否会造成客户端插值问题？参考 plan-sword-path-v2 §参数化意图同步
3. **VFX 联调优先级**：P3 VFX 资产包体积大（13 PNG + 17 JSON），是否应先建骨架类然后逐步填充资产？
4. **schema v2 backward compat**：CombatAttackSourceV1 加新变体后，旧 agent 版本 enum 反序列化是否 panic？需确认 protobuf 的 unknown enum value 处理策略
