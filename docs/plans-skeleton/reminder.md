# Skeleton 未决事项登记

本目录下 plan 在展开/落地过程中累积的**延后处理**事项。每条带 plan 锚点 + 上下文，后续回来解决时直接对号入座。

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
> **已转为独立骨架（2026-04-28，本次全量整理）**：
> - `plan-inventory-v2`（背包 v2：grid 自动寻位 + stacking 合并）
> - `plan-death-lifecycle-v2`（死亡生命后续：善终/横死 + 风烛 buff + 寿元交叉验证 + deathInsight tool）
> - `plan-alchemy-effects-v1`（炼丹副效系统：side_effect_pool 接真实效果 + 丹心识别 + AutoProfile + 品阶方向）
> - `plan-combat-anticheat-v1`（战斗反作弊层：AntiCheatCounter + bong:anticheat 推送）
>
> **已核查为已实装，直接删除（2026-04-28）**：
> - plan-death-lifecycle §4c 续命路径：`PillExtensionContract` / `CollapseCoreExtensionContract` / `EnlightenmentExtensionContract` 均已实装（`lifespan.rs` 94–154）
> - plan-lingtian `ZONE_LEAK_RATIO`：常量已实装（`growth.rs:29`，值 0.2）
> - plan-lingtian `zhenfa_jvling` 钩子：已实装并已填值（`environment.rs` compute_plot_qi_cap +1.0）
> - plan-alchemy 丹方残卷损坏：`FlawedFallback` 已实装（`recipe.rs` 122–248）
> - plan-alchemy SVG 草图 CELL_SIZE 不一致：草图仅为可读性处理，不影响实际渲染
> - plan-tribulation 域崩触发阈值：`plan-tribulation-v1 §4.1` 已量化（spirit_qi < 0.1，持续 1 小时）
> - plan-tribulation 欺天阵接口：已在 `plan-tribulation-v1 §5` + `plan-zhenfa-v1 §9` 双端标注延后
>
> **已归并到现有 plan（2026-04-28）**：
> - plan-tribulation 半步化虚 buff + 截胡预兆窗口 → 补入 `plan-tribulation-v1 §9` 开放问题
> - plan-zhenfa 持久化方案 + 欺天阵权重注入 → 补入 `plan-zhenfa-v1 §10` 开放问题
> - plan-combat deathInsight tool → 归入 `plan-death-lifecycle-v2 §4`
> - 采药工具系统 → 由 `plan-botany-v2`（active）承接
