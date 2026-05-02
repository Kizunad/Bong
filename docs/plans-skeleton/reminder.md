# Skeleton 未决事项登记

本目录下 plan 在展开/落地过程中累积的**延后处理**事项。每条带 plan 锚点 + 上下文，后续回来解决时直接对号入座。

---

> **约定**：每解决一条就从这里删。新增延后事项请直接追加到对应 plan 段，保持扁平。
>
> **已转为独立骨架（2026-04-27）**：
> - `plan-alchemy-client-v1`（炼丹系统 Fabric 客户端接入）→ 已归档至 `finished_plans/`
> - `plan-niche-defense-v1`（灵龛主动防御）
> - `plan-fauna-v1`（妖兽骨系材料）→ 已归档至 `finished_plans/`
> - `plan-spiritwood-v1`（灵木材料体系）→ 已归档至 `finished_plans/`
> - `plan-spirit-eye-v1`（灵眼系统）
> - `plan-botany-agent-v1`（植物生态快照接入天道 agent）→ 已升为 active plan
>
> **已转为独立骨架（2026-05-01）**：
> - `plan-lifespan-v1`（寿元精细化 / 风烛 / 续命路径 / 老死分类）— 源自 plan-death-lifecycle-v1 §4a/§4c reminder
> - `plan-anticheat-v1`（AntiCheatCounter / CHANNEL_ANTICHEAT）— 源自 plan-combat-no_ui §1.5.6 reminder
> - `plan-alchemy-v2`（side_effect_pool 映射 / 丹方残卷 / 品阶铭文开光 / AutoProfile / 丹心识别）— 源自 plan-alchemy-v1 reminder
> - `plan-inventory-v2`（Tarkov grid placement / stacking 合并）— 源自 plan-inventory-v1 reminder
>
> **已转为独立骨架（2026-05-02）**：
> - `plan-halfstep-void-v1`（半步化虚 buff 实装 + 重渡机制）— 源自 plan-tribulation-v1 §8 buff 占位 + 重渡机制待确认
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
