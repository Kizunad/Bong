# Bong · plan-lingtian-npc-v1 · 骨架

**NPC 散修种田**（灵田 v2 扩展）。在 plan-lingtian-v1 全部 P0–P5 落地后，启动 NPC 散修自主开荒-种植-收获循环。NPC 与玩家共享 zone 灵气账本（worldview §十 零和），共同推动 §5.1 密度阈值；散修种田给世界添加"非玩家驱动"的灵气竞争与社交节点。

**世界观锚点**：
- `worldview.md §十一` 玩家默认敌对——**散修非雇工/非合作 NPC**，他们也偷玩家灵田、玩家也可偷他们的
- `worldview.md §十` SPIRIT_QI_TOTAL = 100 灵气零和（NPC 种田同样从 zone 抽吸）
- `worldview.md §七` 灵物密度阈值（多个 NPC 在同 zone 种田 → 一同推高密度 → 触发天道注视 / 道伥刷新）
- `worldview.md §八.1` 注视规则——NPC 散修被道伥锁定时也会被攻击（不为玩家专属）

**library 锚点**：`peoples-0006 战斗流派源流`（散修生态）· 待补 `peoples-XXXX 散修生计录`（散修种田动机文本，写作 plan 启动后补）

**交叉引用**：
- `plan-lingtian-v1.md`（plot/seed/session/replenish/pressure 全套依赖）
- `plan-npc-ai-v1.md`（big-brain Scorer / Action 扩展，新增 `LingtianFarmingScorer` / `TillAction` / `PlantAction` / `HarvestAction` / `ReplenishAction`）
- `plan-cultivation-v1.md`（NPC 散修必须有 cultivation 组件才有种田动机——种植积 herbalism XP）
- `plan-skill-v1.md`（NPC herbalism 技艺等级影响成功率）
- `plan-death-lifecycle-v1.md`（散修老死 → 灵田无主 → 玩家可接收）

---

## §0 设计轴心

- [ ] 散修 = **独立 archetype**，不是雇工——他们有自己的灵田、收获自用、被玩家偷会反击
- [ ] 共用 `LingtianPlot` 组件 + 共用 `ZoneQiAccount` 灵气账本（不另立"NPC 灵田表"）
- [ ] NPC 种田走 **同套 session 流程**（TillSession/PlantingSession/HarvestSession/ReplenishSession），由 brain 触发 intent，不绕过 ECS
- [ ] **不做** NPC 互相买卖作物 / 雇佣关系（违反 §十一玩家敌对的延伸"散修也敌对"）
- [ ] **不做** "看到 NPC 种田就触发剧情" —— 散修是 ambient，不是 quest 节点

---

## §1 NPC 种田动机（big-brain Scorer 设计）

| Scorer | 触发条件 | 优先级 |
|---|---|---|
| **PlotEmptyAndHasSeedScorer** | 自有 plot 空 + 背包有种子 | 0.6 |
| **CropRipeScorer** | 自有 plot crop ripe | 0.8 |
| **PlotQiLowScorer** | 自有 plot plot_qi < 0.3 + 有补灵材料 | 0.5 |
| **PlotBarrenScorer** | 自有 plot harvest_count ≥ 5 | 0.4 |
| **NoPlotScorer** | 还没 plot + zone 灵气 > 阈值 + 有锄头 | 0.3 |
| **StealPlotScorer**（高阶）| 邻近无主 / 玩家 plot + crop ripe | 0.2（避免一上来就偷） |

战斗 / 逃跑 Scorer 优先级仍高于种田（与 plan-npc-ai-v1 现有 picker 对接）。

---

## §2 散修 archetype 数据

```rust
// server/src/npc/scattered_cultivator.rs（新文件）
#[derive(Component)]
pub struct ScatteredCultivator {
    pub home_plot: Option<Entity>,         // 自有 LingtianPlot
    pub seed_inventory: HashMap<PlantId, u32>,
    pub bone_coins: u32,
    pub herbalism_level: u8,               // 影响 till/plant 成功率 + auto 模式解锁
    pub farming_temperament: FarmingTemperament,  // Diligent/Lazy/Aggressive
}

pub enum FarmingTemperament {
    Diligent,    // ratio 7:3 种田:战斗
    Lazy,        // ratio 3:7
    Aggressive,  // 偏 StealPlotScorer 权重
}
```

- [ ] spawn 时初始化 home_plot 周围 1-2 个 LingtianPlot（plan-worldgen 提供合适地块）
- [ ] 死亡 → home_plot.owner = None（plot 进入"无主"状态，可被玩家/其他 NPC 接收）

---

## §3 共享灵气账本与密度阈值

- [ ] NPC 散修种田同样调 `ZoneQiAccount::deposit/withdraw`，与玩家共享同一个 zone qi
- [ ] `compute_zone_pressure` 公式不变 —— NPC 的 plot 也算入 demand
- [ ] **关键**：当 NPC 多到一定密度（zone 内 NPC plot 数 ≥ N），即使无玩家也会触发 `ZonePressureCrossed{ level: High }` → 道伥 spawn
- [ ] 道伥同时攻击 NPC 散修 + 玩家（worldview §八.1，无玩家专属性）

---

## §4 数据契约

- [ ] `server/src/npc/scattered_cultivator.rs` —— `ScatteredCultivator` 组件 + `FarmingTemperament` enum
- [ ] `server/src/npc/farming_brain.rs` —— 6 个 Scorer + 4 个 Action（Till/Plant/Harvest/Replenish）
- [ ] `server/src/npc/spawn.rs` 扩展 —— `spawn_scattered_cultivator_at(BlockPos)`
- [ ] `server/src/lingtian/session.rs` 复用 —— Session struct 加 `actor: Entity`（已有 `player: Entity` 改名 / 加 alias）
- [ ] schema 扩展 —— `LingtianSessionDataV1` payload 区分 actor=Player / actor=NPC（HUD 是否显示）
- [ ] `assets/npc/scattered_cultivator.toml` —— 默认 loadout（hoe_iron + 各种子若干）

---

## §5 实施节点

- [ ] **P0**：`ScatteredCultivator` 组件 + spawn 路径 + 单测覆盖 spawn 后 home_plot 正确挂接
- [ ] **P1**：6 Scorer + 4 Action 接 big-brain Thinker；NPC 能完成"till → plant → 等成熟 → harvest"完整循环（无补灵）
- [ ] **P2**：补灵 Scorer + ReplenishAction（NPC 自动消耗 bone_coins / lingshui 补 plot_qi）
- [ ] **P3**：StealPlotScorer + 与玩家偷田对称（NPC 偷玩家 → 玩家 LifeRecord 记 `PlotHarvestedByOther{ by_npc: true }`）
- [ ] **P4**：FarmingTemperament 三档 + 平衡测试（多 NPC 同 zone → 触发 §5.1 密度阈值）

---

## §6 开放问题

- [ ] NPC 散修死后 plot 持续多久无主？是否随时间被天地噬散（worldview §十二）？
- [ ] NPC 种田收获的作物去向？囤积自用 / 慢慢消耗 / 死后散落？
- [ ] NPC 偷玩家田的 detection / 反应——玩家在场 vs 离线时行为差异？
- [ ] 散修 archetype 是否再分支（凡人散修 / 引气散修 / 凝脉散修），不同境界种田能力差异？
- [ ] 与 plan-tribulation 的接触——若散修推进到固元期，是否会因密度阈值反噬全 zone？

---

## §7 进度日志

- 2026-04-27：骨架创建。前置依赖 `plan-lingtian-v1` 全部 ✅（已核验）；`plan-npc-ai-v1` 仍 ⏳（仅僵尸 archetype），本 plan 启动需先解决 npc-ai 多 archetype P0。
