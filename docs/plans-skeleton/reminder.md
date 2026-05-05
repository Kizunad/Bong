# Skeleton 未决事项登记

本目录下 plan 在展开/落地过程中累积的**延后处理**事项。每条带 plan 锚点 + 上下文，后续回来解决时直接对号入座。

---

## plan-tribulation-v1

- [ ] **半步化虚 buff 强度**：当前 +10% 真元上限 / +200 年寿元是占位。Phase 1-3 已上线，可观察"卡在半步化虚"的玩家比例后调整；名额空出时可重渡的升级机制也待确认（已在 plan-tribulation-v1 §8 标注）

---

> **约定**：每解决一条就从这里删。新增延后事项请直接追加到对应 plan 段，保持扁平。
>
> **已转为独立骨架（2026-04-27）**：
> - `plan-alchemy-client-v1`（炼丹系统 Fabric 客户端接入）
> - `plan-niche-defense-v1`（灵龛主动防御）
> - `plan-fauna-v1`（妖兽骨系材料）
> - `plan-spiritwood-v1`（灵木材料体系）
> - `plan-spirit-eye-v1`（灵眼系统）
> - `plan-botany-agent-v1`（植物生态快照接入天道 agent）
>
> **已转为独立骨架（2026-05-01）**：
> - `plan-lifespan-v1`（寿元精细化 / 风烛 / 续命路径 / 老死分类）— 源自 plan-death-lifecycle-v1 §4a/§4c reminder
> - `plan-anticheat-v1`（AntiCheatCounter / CHANNEL_ANTICHEAT）— 源自 plan-combat-no_ui §1.5.6 reminder
> - `plan-alchemy-v2`（side_effect_pool 映射 / 丹方残卷 / 品阶铭文开光 / AutoProfile / 丹心识别）— 源自 plan-alchemy-v1 reminder
> - `plan-inventory-v2`（Tarkov grid placement / stacking 合并）— 源自 plan-inventory-v1 reminder
>
> **已转为独立骨架（2026-05-05）—— qi_physics 底盘**：
> - `plan-qi-physics-v1`（修仙物理底盘：守恒律 + 压强法则 + 唯一物理实现入口）— **关键路径**。源自 plan-economy-v1 §1.5 衰变曲线裁决无解，上钻发现 worldview §二「真元极易挥发」是 9+ plan 同源现象（骨币/食材/距离/异体排斥/吸力/节律/末法残土/灵田漏液/搜刮磨损），各 plan 拍数才是问题根源。本 plan 立公理 + 算子 + 全局账本 WorldQiAccount，P1 完成 = 底盘 API 冻结
> - `plan-qi-physics-patch-v1`（qi-physics 迁移收口）— 承接 qi-physics-v1 P1 后的迁移工作；P0 红线 3 PR（combat/decay 0.06 vs 正典 0.03 翻倍 / tsy_drain×dead_zone 协调 / WorldQiAccount 合账）；P1 shelflife / P2 战斗+守恒释放 / P3 新机制（坍缩渊 redistribute / 7 流派异体排斥 ρ / 时代衰减 / 阈值灾劫）
>
> **已转为独立骨架（2026-05-05）—— 流派功法**：
> - `plan-woliu-v2`（涡流功法五招完整包：持涡 / 瞬涡 / 涡口 / 涡引 / 涡心）— 承接 plan-woliu-v1 ✅ finished。引入**搅拌器物理**（99% 紊流甩出 + 1% 入池 × 经脉流量 cap）+ **紊流场**（动态漩涡非可吸收浓度，是涡流流派专属 EnvField 边界，其他玩家在场内不可修炼 / 战斗精度 ×0.5 / shelflife ×3）+ 反噬阶梯 4 级（微感 / MICRO_TEAR / TORN / SEVERED）+ **无境界 gate 只有威力门坎**（worldview §五:537 流派由组合涌现）+ 化虚双场模型（致命场 ≤10 格 + 影响场 zone 量级）+ 化虚被动场可关。前置依赖 plan-qi-physics-v1 P1 + plan-qi-physics-patch-v1 P0/P2 完成。反向被依赖：plan-style-balance-v1（W/β/K_drain 矩阵）/ plan-color-v1（缜密色加成 hook）/ plan-tribulation-v1（化虚绝壁劫触发链）/ plan-zhenfa-v1（紊流场 vs 阵法冲突仲裁，留 zhenfa vN+1）。化虚涡心叙事意象 = worldview §四:380「化虚老怪走过新人来不及看清袍角」物理依据（整个山谷瞬间进入紊流死区）。骨架 §5 七个开放问题待 P0 决策门收口（化虚被动场默认开关 / 紊流场对 caster 自身影响 / 紊流 vs 阵法仲裁 / 99/1 比例 telemetry / 干尸涡引 / 被动场 qi 消耗 / 防御招精度衰减）
>
> **依赖链关键路径（plan-economy / plan-style-balance / 等等都在等）**：
> ```
> plan-qi-physics-v1 P0 红线决议 → P1 算子 ship
>   → plan-qi-physics-patch-v1 P0/P1/P2/P3 逐 PR 迁移
>     → plan-economy-v1 / plan-style-balance-v1 / 其他 ~9 个 plan 解阻
> ```
>
> **同步动作（2026-05-05）**：
> - `docs/CLAUDE.md §二 接入面 / §四 红旗` 各加 qi_physics 锚点条目，约束新 plan 不再自己拍真元常数
> - `plan-economy-v1` §1.5 三选一裁决整体废弃；§0 持有=贬值补地点制约推导；§4 收口 2 条原悬而未决
> - `plan-style-balance-v1` 现状对齐：7 流派 plan 全 finished（`docs/finished_plans/`）；P1 telemetry 已在 PR #129 顺手实装混元色维度，但 spec 5 维未对齐
>
> **已核实可删除（2026-05-01）**：
> - plan-tribulation-v1：预兆窗口 60s ✅（已在 plan §2.1 定义）；域崩阈值 spirit_qi<0.1 持续 1h ✅（已在 plan §4.1 定义）；欺天阵接口 → 已归 plan-zhenfa-v1 tracking
> - plan-zhenfa-v1 两条开放问题 → 已在 active plan §10 tracking
> - plan-lingtian-v1 两条 → 已在 active plan tracking
> - plan-combat-no_ui 遗念 deathInsight → 已实装（`server/src/schema/death_insight.rs` / `combat/lifecycle.rs`）
> - plan-alchemy-v1 测试 JSON 占位 → 仅提示注释，不需要 plan tracking
> - plan-alchemy-v1 SVG 草图尺寸差异 → 仅草图，不影响实装
> - 通用 "开放问题节 review pass" → meta-task，太宽泛；直接推进各 plan
> - plan-tools-v1 采药工具系统 → ✅ 已完成（2026-04-29 立项，已有骨架）
