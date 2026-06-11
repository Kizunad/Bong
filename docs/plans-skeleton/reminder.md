# Skeleton 未决事项登记

本目录下 plan 在展开/落地过程中累积的**延后处理**事项。每条带 plan 锚点 + 上下文，后续回来解决时直接对号入座。

---

> **约定**：每解决一条就从这里删。新增延后事项请直接追加到对应 plan 段，保持扁平。

---

## 套包系统 4-plan 族（PR #467，Pi review 2026-06-10 产出的 §8.1 收口待办）

升 active 做 §8.1 收口时一并处理（Pi 标"升 active 前做"，非阻塞 merge）：

> 2026-06-10 填充 workflow 已解决并删除：§8 已决项搬正文（nested-pack + container-filter）、12 容器验收表"落地阶段"列、Plan A↔D 交叉引用。

- **`plan-nested-pack-base-v1` §8 #6 持有式套包 ContainerSpec 表示（升 active 前 §8.1 必收口，2026-06-10 Pi review）**：`parse_container_spec`（`mod.rs:1532`）强制 `equip_slot ∈ {back_pack/waist_pouch/chest_satchel}` 必填，持有式套包（放普通格双击开）填 `back_pack` 会触发 `rebuild_containers_from_equipment`（`mod.rs:3192`）与持有路径撞车（P3 research 风险 4）。§8.1 须实地拍定 held-only spec 表示（候选：`ContainerSpec.held_only: bool` / 独立 `PackSpec` / `rebuild` 黑名单排除）+ registry load 不 panic 的最小字段集。**下游 [[plan-container-filter-and-completion-v1]] P3 给同 5 子包补 `accept` 时依赖此 spec 定型**——两 plan 须用同一容器 spec 表示，不可各填各的 equip_slot。
- **`plan-nested-pack-base-v1` §8.1 #7 + 下游 wire 破坏（2026-06-10 Pi review，已在根 plan 定案）**：S2C 套包 open **复用 `LootContainerOpenV1`**（标 `#[serde(deny_unknown_fields)]` `server_data.rs:610`），**全 plan 族不存在 `PackContainerOpenV1` struct**（仅 `PackContainerOpen/Move/Close` 三个 C2S enum 变体）。下游 [[plan-container-filter-and-completion-v1]] P4 给 `LootContainerOpenV1` 加 `accept_filter` 是 **wire 破坏**，实施时必须连同 `agent/packages/schema/samples/*.json` 双端 sample + `.proto` 一起改，不可只加 Rust 字段。下游 L37/125/144 引用已与根 plan 对齐，无悬空 symbol——2026-06-10 升 active 时已核对两 plan 一致（filter plan 接入面明确复用 `LootContainerOpenV1`+`PackItem` 变体、无 `PackContainerOpenV1` struct）；wire 破坏约束（samples + .proto 同改）留给实施时遵守。

## 放置类 17 消杀 plan 族（2026-06-10，调查 workflow 产出，立骨架时已知待办）

- **plan-block-lifecycle-v1 文档漂移回写**：P0(2ad2076a0)/P1(c1461f7bd)/P2(39d722956)/P3(b8bb67787)已 commit 但 plan 阶段总览全标 ⬜，P4 在 worktree 分支——该 plan 归属其 orchestrator，待其收尾时回写 ✅，新 plan 族不代改。
- **双死字段顺带激活**：`healing_rate_multiplier`（components.rs:291，SpleenKidney 写入无读取）归 plan-furniture-buff-v1 P2 激活；`qi_regen_multiplier`（HeartLung 写入，qi_regen tick 不读 DerivedAttrs）暂无归属——若 furniture P3 蒲团走 QiRegenBoost 路径则顺带接，否则单列待办。
- **WorkbenchConstants.java:15 SFX/VFX 常量 stub**：7 条 SFX + 3 组 VFX 无资产无调用，归 plan-workbench-place-runtime-v1 P2 实装或删除。
- **niche_guardian SFX 断链**（NicheDefenseReactionVfxPlayer.java 返回无资产 ID）：归 plan-niche-craft-fix-v1 P1。
- **A-P5 / B-P3 改同一 TOML 协调**：两阶段都改 `workbench_materials.toml` 同 5 个随身子包条目，依赖图（B 依赖 A）已保证顺序，实施时注意相邻段 merge conflict。
- **`practice_session_tick` 接活留专门 plan（plan-furniture-buff-v1 §8.1 #6 剥离，2026-06-10）**：`cultivation/practice_session.rs:69` `practice_session_tick` + `:85` `check_practice_session_exit` 均 `#[allow(dead_code)]`，mod.rs:66 仅 `pub mod` 无 ECS 注册，调用全在自身测试内。接活需 ① 新写供给 qi/zone/proficiency 的包装 system（非一行注册）② 处理其 `practice_session.rs:78` `*current_qi -= cost` 的 qi 流——当前无 zone credit、不走 `qi_physics::ledger`，是守恒律红旗（修炼消耗须有 zone 等额变化）。furniture-buff 蒲团已用守恒安全的 `CultivationAcceleration` 兜住 +20% 修炼速度，打坐累积系统不在其 scope。将来另立 plan 接活时须补 ledger 归还路径 + 守恒断言。
- **`BongEntityModelKind` raw_id 占号协调**（plan-placeable-container-blocks-v1 Pi review 2026-06-10）：`plan-workbench-place-runtime-v1` §8 #3 占 165（`WORKBENCH`），`plan-placeable-container-blocks-v1` 顺延占 166/167/168（`TRADE_CRATE`/`HERB_CRATE_PLACED`/`DEAD_DROP_BOX`，紧随 `Baolongwang=164`）。两 plan 升 active 时核对 165 实际落地避免撞号。
- **plan-placeable-container-blocks-v1 升 active 前 §8.1 收口硬约束**（Pi review 2026-06-10）：① 核实 `plan-workbench-place-runtime-v1` §8 #3 entity 表示确实落地为「纯 entity + bbmodel」（非 bong_blocks）——否则本 plan 交互/渲染层需重写，停下交人工；② P1 通用容器 open 路径**非复用 coffin**（coffin handler 受 `SupplyCoffinRegistry.active` gate + 硬编 source_kind 拒非棺 entity），须新增泛化 C2S/独立 open system + `external_kind_to_source_kind` 映射；③ P3 毒气雷无现成「无 attacker AoE」原语，参照 `zhenfa::BlastTrap`（owner 当 attacker）范式新建，复用现有 StatusEffect 变体不新增 Poison；④ P0 破坏链路须含 open-session 强制关闭（新增 `LootContainerCloseReasonV1::ContainerDestroyed`，不复用 `CoffinDestroyed`）。
- **moisture_base（防潮架）接 shelflife 保鲜留 follow-up（plan-furniture-buff-v1 §8.1 #3 降级，2026-06-10）**：shelflife 是 item-in-container 模型（`container_storage_multiplier` 纯函数无世界坐标/范围），"moisture_base 范围内的容器"需**世界放置容器 entity** 作命中对象，归 `plan-placeable-container-blocks-v1`（仍 skeleton 未 active/merge）。该依赖 merge 后另立/并入：触发 enter_container/放置 → `FurnitureRegistry.kinds_in_range` 判容器 entity 是否在 MoistureBase 范围 → 改容器实例 `ContainerFreshnessBehavior` 为 `Freeze`（非直写 storage_multiplier；Stepwise→1.0 防归零）→ 移出/破坏还原 + `exit_container` 维护 frozen 字段。furniture-buff 中 moisture_base 仅落地为家具方块 + 进 FurnitureRegistry，无保鲜行为。
- **§808 转移税另立 plan 待办（2026-06-10 甩锅消解后遗留）**：worldview §九:808「inventory 操作扣灵气纯度 1-5%」转移税现无 plan 认领——套包族两 plan 均已显式划出 scope（shelflife 无一次性扣减接口、扣 spirit_quality 须走 qi_physics ledger 守恒、与筛选/保鲜不同源；nested-pack L39 甩锅措辞已删）。另立 plan 时需先扩 qi_physics 定义扣减率常数 + ledger 归还路径，不可 plan 内自拍 1-5% 数值。

## plan-economy-zombie-cleanup-v1 遗留

> plan-shield-block-v1 的 3 条措辞修正（PR #470 Pi review）与 plan-economy-zombie-cleanup-v1 的 5 条勘误（PR #472 Pi review）均已于 2026-06-10 填充 workflow 并入对应 plan（plan 头部有声明），已按约定删除。

- **camouflage_net 驻地遮蔽待办**（§8.1 #5 收口决议产物）：本 plan 仅把 camouflage_net 接成 Fan 档伪皮材料，未做差异化「驻地遮蔽」效果（需放置形态，依赖 `plan-workbench-place-runtime-v1` 的 `block_item_to_state` 或新 ECS Component）。将来做驻地遮蔽时另立 plan 或并入放置类族，届时可从 Fan 档升级。
