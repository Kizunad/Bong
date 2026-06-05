# Bong · plan-neg-domain-fauna-v1 · 负灵域特有生态

**负灵域两类特有生态实装**：看不见的真空漩涡「诡影（空兽）」——撞上直接扣真元；以及蔓延在负灵域地表的「噬灵藓」——踩上持续吸真元。两者均为负灵域的**环境生态**，不是可交战 NPC；其真元扣除全部走 `qi_physics::ledger::QiTransfer`，守恒律不破。

**世界观锚点**：
- `worldview.md §二:51` 负灵域特有生态——"吞噬真元的「噬灵藓」，以及看不见的真空漩涡「诡影（空兽）」——撞上直接扣真元"
- `worldview.md §七:751` 生态联动——"除诡影外，没有生物敢靠近负灵域"（诡影是负灵域唯一活体存在）
- `docs/library/peoples/异变图谱·残卷.json` — "诡影空兽（无形扣真元**疑为环境**）"（馆内研究者注：视为环境现象而非真兽）

**前置依赖**：
- `plan-qi-physics-v1` ✅ — `qi_physics::ledger::QiTransfer`（守恒律基础）
- `plan-qi-physics-patch-v1` ✅ — `SIPHON_FACTOR` + `siphon_amount` 已在 `cultivation/negative_zone.rs`（zone 级抽吸已实装）
- `plan-world-ecology-events-v1` ✅ — 游离风暴/负灵域触发系统已实装（本 plan 在其产生的负灵域内生成诡影/噬灵藓）
- `plan-fauna-v1` ✅ — `FaunaKind` enum 扩展入口（诡影挂为环境型 fauna variant）
- `plan-botany-v1` ✅ — `PlantRegistry` 扩展入口（噬灵藓注册为特殊地块类植物）

**反向被依赖**：
- `plan-neg-domain-escape-v1` ⬜（本 plan P0 完成后：诡影在负灵域真正致命，骨架中的逃遁路径才有游戏价值）
- `plan-sou-da-che-v1` ⬜ active（§3 "负灵域边缘噬灵藓蔓延"视觉警告依赖本 plan 实装）
- `plan-model-asset-v1` ⬜（`gui_ying` / `qi_eating_moss` 模型待资产对接，本 plan 先用占位方块/粒子）

---

## 接入面 Checklist

- **进料**：`qi_physics::ledger`（QiTransfer 写入）/ `ZoneRegistry`（zone spirit_qi 读取，判定负灵域阈值 < 0）/ `cultivation::negative_zone::siphon_amount`（复用或参考公式）/ `FaunaKind` enum（诡影 variant 注册）/ `botany::PlantRegistry`（噬灵藓 plant spec 注册）
- **出料**：`GhostEntity`（诡影，server-only 移动实体，不发 MC entity packet）+ `ShiLingXian` block（噬灵藓方块，负灵域地表）+ 两者的 `QiTransfer` 事件流 + Narration hint（诡影触碰 / 噬灵藓效果）
- **共享类型**：新增 `FaunaKind::Ghost`（诡影，server-only 环境型）；复用 `PlantSpec`（噬灵藓注册）；无新 IPC schema（纯 server-side 环境危害）
- **跨仓库契约**：
  - server：新文件 `server/src/fauna/ghost.rs`（诡影漂移 + 接触检测）+ `server/src/botany/shiling_xian.rs`（噬灵藓生长 + 踩踏事件）；`cultivation/negative_zone.rs` 保持不动（zone 级抽吸已够，本 plan 叠加实体级抽吸）
  - agent：无新 schema；narration hints 复用 `world_state` 中已有的 zone qi 字段感知负灵域
  - client：诡影本 plan P0 无渲染（server-only），P2 可选加 subtle 粒子扰动（需 VFX plan 配合）；噬灵藓 P1 用 vanilla 苔藓材质占位，P2 待 model-asset plan 资产
- **worldview 锚点**：§二 负灵域特有生态 + §七 负压灭杀/生态联动
- **qi_physics 锚点**：
  - 诡影接触扣真元：`QiTransfer { from: player, to: zone_neg, amount: ghost_siphon_pulse, reason: GhostContact }`（真元归还负灵域 zone，守恒）
  - 噬灵藓踩踏扣真元：`QiTransfer { from: player, to: zone_neg, amount: moss_drain_per_tick, reason: ShiLingXianDrain }`（同上）
  - **不自造物理公式**：接触量 = `|zone_qi| × qi_max × GHOST_CONTACT_FACTOR`（参照 `siphon_amount` 模式）

---

## §0 设计轴心

- [ ] **诡影是"环境陷阱"不是 NPC**：不可见、不可交战、不掉落。存在理由：让负灵域不是"可无限苟着"的安全角落——即使没被 zone 级抽吸死，还有随机漂移的 contact 扣量
- [ ] **噬灵藓是地形警告 + 惩罚**：视觉上有暗紫色发光提示（满足 worldview §二 视觉辨识要求），踩上即扣真元（惩罚鲁莽穿越）；可被玩家铲除但会随负灵域环境再生
- [ ] **两者叠加但不相互屏蔽**：玩家在负灵域可能同时承受 zone 级抽吸（negative_zone.rs）+ 诡影接触脉冲 + 噬灵藓持续扣量。三层叠加正是"除诡影外无生物敢靠近"的物理解释
- [ ] **守恒律强约束**：所有扣量必须走 ledger，真元从玩家账户流向 zone 账户，zone spirit_qi 变化已由 zone 系统处理
- [ ] **不做可攻击版诡影**：library 注释"疑为环境"是世界观对诡影的正式定性，实装就按环境处理，不做 AI 树

---

## §1 阶段总览

| 阶段 | 内容 | 状态 | 验收 |
|------|------|------|------|
| **P0** | 诡影：`GhostEntity` 漂移 + 接触检测 + QiTransfer；负灵域内随机生成 | ⬜ | server test：玩家进入 ghost 半径 → QiTransfer 触发；ghost 在 spirit_qi < 0 的 zone 内生成 |
| **P1** | 噬灵藓：`ShiLingXian` 方块注册 + 踩踏持续扣真元 + 负灵域生长逻辑 | ⬜ | server test：踩噬灵藓 → FoodBonus-style Drain Component 挂载 → QiTransfer per tick；负灵域 zone 内地表方块生长 |
| **P2** | 生态联动：诡影密度随 zone 负压强度缩放；噬灵藓蔓延速率随 spirit_qi 负值正比 | ⬜ | server test：spirit_qi = -0.2 vs -1.0 区域诡影数量差异；噬灵藓在 spirit_qi 趋近 0 时停止蔓延 |
| **P3** | Narration hints + 视觉占位（诡影粒子扰动/噬灵藓暗紫占位材质） | ⬜ | narration：首次进入负灵域且遭遇诡影时输出提示；噬灵藓踩踏有短暂视觉反馈 |

---

## §2 诡影（空兽）设计

### GhostEntity

```rust
// server/src/fauna/ghost.rs
pub struct GhostEntity {
    pub position: Vec3,            // 当前位置（server-only，不发 MC entity packet）
    pub drift_velocity: Vec3,      // 漂移速度（随机方向，每 N tick 重新采样）
    pub siphon_radius: f64,        // 接触半径（建议 2.0 格）
    pub zone_name: String,         // 所在 zone（负灵域判定）
}
```

**生成条件**：`ZoneRegistry` 中 `zone.spirit_qi < NEG_DOMAIN_THRESHOLD`（-0.2）时，按负压强度决定诡影密度（`|spirit_qi| × GHOST_DENSITY_FACTOR`）

**漂移行为**：Bevy 系统 `ghost_drift_system` 每 tick 更新位置；每 3s 重新随机方向；不穿越 zone 边界

**接触检测**：`ghost_contact_system` 遍历玩家位置 vs 诡影位置，距离 < `siphon_radius` → 触发接触

**接触效果**：

```text
pulse_amount = |zone_qi| × qi_max × GHOST_CONTACT_FACTOR
QiTransfer { from: player, to: zone, amount: pulse_amount, reason: GhostContact }
```

- 每次接触 cooldown 1s（防止一帧被多个诡影叠爆）
- 接触后 narration hint（随机，低频，一次会话内不重复）

**Worldview 约束**：
- 不可见（不发 SpawnEntity packet）
- 不可攻击（无 HP / Damageable Component）
- 境界越高 qi_max 越大 → pulse_amount 越大（正确体现"高手反而更危险"）

---

## §3 噬灵藓（ShiLingXian）设计

### 方块注册

```text
item_id: "shi_ling_xian"
PlantSpec {
    habitat: Tainted,             // 负灵域（spirit_qi < 0）
    growth_mode: SpreadByCrawl,   // 从已有方块向相邻地面蔓延
    spread_rate: |spirit_qi|,     // 负压越强蔓延越快
    max_spread_radius: 16 blocks,
    decay_when: spirit_qi >= 0,   // 正灵域自然枯萎
}
```

**踩踏效果**（`ShiLingXianDrainTag` Component）：
- 玩家站在噬灵藓上时挂载 `ShiLingXianDrainTag { drain_per_tick: ... }`
- `drain_per_tick = qi_max × MOSS_DRAIN_FACTOR`（比 siphon_amount 小，属于补充而非主要威胁）
- 离开方块后 Component 移除

**生长逻辑**：
- 初始：游离风暴/伪灵脉消散后 + 坍缩渊边缘地带随机生成首批种子方块
- 扩散：`spread_ticked_system` 按 spread_rate 向未被占据的相邻固体表面扩展
- 消退：zone spirit_qi ≥ 0 时停止扩散，存量方块 60 秒内自动消亡

---

## §4 数据契约

- [ ] `server/src/fauna/ghost.rs`：`GhostEntity` struct + `ghost_spawn_system` + `ghost_drift_system` + `ghost_contact_system`
- [ ] `server/src/botany/shiling_xian.rs`：`ShiLingXianDrainTag` Component + `moss_drain_system` + `moss_spread_system`
- [ ] `PlantRegistry` 新增 `shi_ling_xian`（P1 前置于 botany）
- [ ] `FaunaKind::Ghost` variant（server-only；不参与 MC entity lifecycle）
- [ ] `QiTransfer` reason variant：`GhostContact` / `ShiLingXianDrain`
- [ ] 两个 narration hint 模板（`ghost_contact_hint` / `moss_drain_hint`）

---

## §5 平衡参数

| 参数 | 建议初值 | 设计理由 |
|---|---|---|
| `GHOST_CONTACT_FACTOR` | 0.05 | 单次接触扣 5%×\|zone_qi\|×qi_max；zone=-0.5 时醒灵扣 0.25，固元扣 7.5 |
| `GHOST_DENSITY_FACTOR` | 0.5 | spirit_qi=-1.0 时每 chunk 约 0.5 个诡影（低密度，随机恐惧感） |
| `GHOST_SIPHON_RADIUS` | 2.0 | 贴脸才触发，不会远程扣 |
| `MOSS_DRAIN_FACTOR` | 0.002 | 每 tick 扣 0.2%×qi_max；比 zone siphon 弱，属辅助惩罚 |
| `NEG_DOMAIN_THRESHOLD` | -0.2 | 本方案阈值；实施时与 `negative_zone.rs` 实际判定核对对齐（勿假设已一致）|

> **qi_physics 归口（docs/CLAUDE.md 孤岛红旗）**：`GHOST_CONTACT_FACTOR` / `MOSS_DRAIN_FACTOR` 属真元抽取/衰减率，实施时**必须先扩 `qi_physics::constants` 再 import**，不在 fauna 模块内硬编；本表只声明建议初值，底层公式与常数归 `qi_physics` 唯一实现。`GHOST_DENSITY_FACTOR` / `GHOST_SIPHON_RADIUS` 为生成/几何参数，不属真元物理，留本模块即可。

---

## §6 开放问题

- [ ] 诡影是否对 NPC 生效（目前仅对玩家）？——倾向：仅对玩家（NPC 不进负灵域，§七 行为是逃跑）
- [ ] 多诡影接触 cooldown 共享还是独立？——倾向：共享 cooldown，防止同时被三个诡影叠爆
- [ ] 诡影是否跨 zone 边界漂移？——倾向：不跨，保持在负灵域内；漂移边界 = zone AABB
- [ ] 噬灵藓是否可合成/作为药材？——倾向：否（它不在 botany 植物分类里，是负面地块）
- [ ] 诡影密度是否需要上限（防止大量 server-only entity 性能问题）？——建议：每 zone 最多 10 个诡影

## §7 进度日志

- 2026-05-31：骨架创建。worldview §二/§七 审计发现负灵域特有生态无实装 plan。前置 qi-physics / negative_zone.rs 已就绪，本 plan 在其基础上叠加实体级危害。
