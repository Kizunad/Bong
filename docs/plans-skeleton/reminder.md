# Skeleton 未决事项登记

本目录下 plan 在展开/落地过程中累积的**延后处理**事项。每条带 plan 锚点 + 上下文，后续回来解决时直接对号入座。

---

## plan-tribulation-v1（active，开放设计问题）

- [ ] 渡虚劫全服广播的截胡机制：worldview §十三规定其他玩家赶路需 10-20 分钟；plan 当前定 60s 预兆期 + 30s 锁定期，是否需要把"远点玩家也能赶到"的窗口扩到 minute 级？需对照 worldview 复核
- [ ] 域崩触发阈值（灵气值 × 持续抽吸时长）未量化——plan §1 表格里仍是占位 X，待 Phase 1 上线后用实测灵气曲线定值
- [ ] **半步化虚 buff 强度未定**（§3）：当前 +10% 真元上限 / +200 年寿元是占位。待 Phase 1-3 上线后看"卡在半步化虚"的玩家比例再定（buff 过强 → 玩家故意卡；buff 过弱 → 服务器稀疏时没人渡；名额释放后升级机制也待定）

> 欺天阵接口已交 plan-zhenfa-v1（skeleton §3/§9 P3）跟踪，本段不重复登记。

---

## plan-lingtian-v1（active，待生产平衡）

- [ ] `ZONE_LEAK_RATIO` 默认 0.2 已落地（`server/src/lingtian/growth.rs:29`），但**未做过生产环境平衡测试**——多人同 zone 多 plot 长时间运行后区域灵气是否枯竭过快？
- [ ] `PlotEnvironment.zhenfa_jvling` 钩子已预留在 `environment.rs`，等 plan-zhenfa-v1 P0 落地后由阵法系统填实数

---

## 已转独立骨架（2026-04-29）

reminder 中以下事项已沉淀为正式 plan skeleton，不再在本文件登记：

- **plan-shouyuan-v1**：坍缩渊换寿（§4c 续命第二条）+ 善终/横死 DeathType 字段。续命丹（commit `3ad73f90`）/ 夺舍 / 风烛 buff 已实装，不在新 plan 范围
- **plan-alchemy-v2**：side_effect_pool → StatusEffect 映射 / 丹方残卷损坏 DamagedRecipe / 品阶 PillTier / 铭文 / 开光 / AutoProfile 曲线库 / 丹心识别 / 测试配方 JSON 正典化
- **plan-inventory-v2**：`add_item_to_player_inventory` grid placement（`mod.rs:867` 硬编 row:0,col:0 修复）+ stacking 合并（`max_stack` 字段 + 同 template 合并逻辑）
- **plan-combat-anticheat-v1**：`AntiCheatCounter` ECS component + `CHANNEL_ANTICHEAT` Redis 推送 + reach/cooldown/qi_invest/defense 四道 clamp 接入

---

## 已转独立骨架（2026-04-27）

- `plan-alchemy-client-v1`（炼丹系统 Fabric 客户端接入）
- `plan-niche-defense-v1`（灵龛主动防御）
- `plan-fauna-v1`（妖兽骨系材料）
- `plan-spiritwood-v1`（灵木材料体系）
- `plan-spirit-eye-v1`（灵眼系统）
- `plan-botany-agent-v1`（植物生态快照接入天道 agent）

---

## 已落实，从 reminder 出列

- [x] 采药工具系统：已由 `plan-tools-v1`（骨架，2026-04-29）覆盖（7 件凡器，命名避用"灵\*"词头）
- [x] death-lifecycle §4a 寿元系统时间（1 real hour = 1 in-game year）：lifespan.rs / persistence lifespan_events 表 / WindCandle / NaturalDeath schema 已落地，与 51.5h 化虚基线、亡者博物馆时间戳兼容
- [x] death-lifecycle "风烛" buff 数值：`wind_candle_halves_qi_regen` 测试（`server/src/cultivation/tick.rs:207`）确认 qi_regen × 0.5 已落实
- [x] 续命丹（life_extension_pill）：`server/src/cultivation/lifespan.rs::apply_lifespan_extension` + inventory item template 已实装（commit `3ad73f90`）
- [x] 夺舍闭环：`DuoSheRequest` / `DuoSheEventV1` / `BiographyEntry::DuoShePerformed` / `PossessedBy` 已实装（commit `3ad73f90`）
- [x] 遗念 agent `deathInsight` tool：`agent/packages/tiandao/src/death-insight-runtime.ts` 已实装并在 plan-combat-no_ui Finish Evidence 验收通过
- [x] zhenfa 欺天阵假劫气权重 / 阵法持久化：已在 `plan-zhenfa-v1` skeleton §8/§10 跟踪，无需在 reminder 重复
- [x] alchemy SVG 草图（57×52 vs 28 不一致）：草图可读性处理，真实渲染按 `GridSlotComponent.CELL_SIZE` 走，无须修复

---

> **约定**：每解决一条就从这里删（移到"已落实"段或直接删除）。新增延后事项请直接追加到对应 plan 段，保持扁平。
