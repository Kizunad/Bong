# Skeleton 未决事项登记

本目录下 plan 在展开/落地过程中累积的**延后处理**事项。每条带 plan 锚点 + 上下文，后续回来解决时直接对号入座。

---

> **约定**：每解决一条就从这里删。新增延后事项请直接追加到对应 plan 段，保持扁平。

---

## 套包系统 4-plan 族（PR #467，Pi review 2026-06-10 产出的 §8.1 收口待办）

升 active 做 §8.1 收口时一并处理（Pi 标"升 active 前做"，非阻塞 merge）：

- **`plan-nested-pack-base-v1` §8 已决项搬正文**：#1 嵌套深度=1、#2 全计入负重 已写"用户已确认"，#3 子容器持久化时机其实是测试点（移到 P3/P5 测试项）——这三条从"开放问题"移入实现正文，§8 只留真正待拍板项（#4 grid 尺寸、#5 浮窗拖拽 P0 spike 风险）。
- **`plan-container-filter-and-completion-v1` §8 已决项搬正文**：Q1 筛选粒度（已决 ItemCategory 升变体）、Q3 `accept=[]` 全收（唯一合理选项）移入 P0 正文；§8 只留 Q2 `moisture_guard` SpoilOnly **精确 rate** —— 这是唯一需 P0 前拍板的数值（否则 `inventory_snapshot_emit` 永远 Normal 兜底，保鲜对玩家隐形）。
- **`plan-container-filter-and-completion-v1` 12 容器验收表加"落地阶段"列**：标每个容器在哪 plan 哪阶段落地（如 `herb_pouch`→A-P5+B-P3，`trade_crate`→C），降实施混淆。
- **Plan A 与 Plan D §8 交叉引用**：两者皆无依赖根 plan，A 嵌套深度=1 与 D 方块表示策略（vanilla 占位 vs bong_blocks）将来可能联动（bong_blocks 容器方块被放入套包时），各加一条交叉引用。（注：原 Plan D `plan-block-placement-base-v1` 已于 2026-06-10 废弃并入 `plan-workbench-place-runtime-v1`，交叉引用以新名为准。）

## 放置类 17 消杀 plan 族（2026-06-10 调查 workflow 产出，立骨架时已知待办）

- **plan-block-lifecycle-v1 文档漂移回写**：P0(2ad2076a0)/P1(c1461f7bd)/P2(39d722956)/P3(b8bb67787)已 commit 但 plan 阶段总览全标 ⬜，P4 在 worktree 分支——该 plan 归属其 orchestrator，待其收尾时回写 ✅，新 plan 族不代改。
- **双死字段顺带激活**：`healing_rate_multiplier`（components.rs:291，SpleenKidney 写入无读取）归 plan-furniture-buff-v1 P2 激活；`qi_regen_multiplier`（HeartLung 写入，qi_regen tick 不读 DerivedAttrs）暂无归属——若 furniture P3 蒲团走 QiRegenBoost 路径则顺带接，否则单列待办。
- **WorkbenchConstants.java:15 SFX/VFX 常量 stub**：7 条 SFX + 3 组 VFX 无资产无调用，归 plan-workbench-place-runtime-v1 P2 实装或删除。
- **niche_guardian SFX 断链**（NicheDefenseReactionVfxPlayer.java 返回无资产 ID）：归 plan-niche-craft-fix-v1 P1。
- **A-P5 / B-P3 改同一 TOML 协调**：两阶段都改 `workbench_materials.toml` 同 5 个随身子包条目，依赖图（B 依赖 A）已保证顺序，实施时注意相邻段 merge conflict。

## plan-shield-block-v1（PR #470，Pi review 2026-06-10 的 3 条措辞修正，升 active 前改）

- **GUARD_RAISE / PARRY_BLOCK 不是零调用死代码**：GUARD_RAISE 被爆脉 FullPowerCharge 消费（`vfx_animation_trigger.rs:361`），PARRY_BLOCK 被 DefenseIntent 动画系统消费（`vfx_animation_trigger.rs:94-102`）。盾牌接线是**新增消费方且须兼容现有触发**，不是"随意覆盖"——plan 内三处"零调用死代码"措辞全改。
- **熟练度孤岛归类修正**：`ProficiencySource::BackfireSurvived` 是活跃变体（`technique_proficiency.rs:52`，0.015 系数），不是 dead_code；真正的第三处孤岛是 `ProficiencyScalars` struct（`technique_proficiency.rs:97-104`）。
- **路径修正**：`armor.rs:61-67` → `combat/armor.rs:60-67`。

## plan-economy-zombie-cleanup-v1（PR #472，Pi review 2026-06-10 的 5 条勘误，升 active 前改）

- **蜕壳入口定位修正**："硬编码只认 tuike_*" 的定位从 `npc/npc_skill.rs` 改为 `combat/tuike.rs:22-23`（或泛指蜕壳流模块）。
- **rat_bait 删除理由修正**：删去"鼠群系统在(spawn_tutorial.rs)"的说法，改为"仅有配方定义无消费端接入"。
- **3 处路径勘误**：`forge/station.rs`、`alchemy/furnace.rs`、client icon 路径按实际修正。
- **配方定位改注释锚点**：`workbench_recipes.rs:1183/:1295` 行号易漂移，改为 `// #87 伪装包裹` / `// #94 伪装网` 注释锚点。
- **P0 交付物细化到函数名**：如 `fn filter_tuike_skin_items` 级别。
