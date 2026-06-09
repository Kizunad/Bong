# plan-workbench-place-runtime-v1 — 工作台放置 runtime + 通用放置物底盘(骨架)

> 一句话:补上 `workbench.rs` 三个 stub system(place/interact/break),让"工作台合得出放不下"闭环,并抽象 `PlaceableBlockKind` 底盘供容器类放置物(plan-placeable-container-blocks)复用。
>
> **本骨架并入并替代原 `plan-block-placement-base-v1`**(2026-06-10 废弃):原骨架与 plan-block-lifecycle-v1 高度重叠(都讲方块放置底层),lifecycle P0-P4 已落地通用 block_place 三端协议,剩余的"工作台/带交互放置物"runtime 归本 plan。

**依赖**:plan-block-lifecycle-v1 P4 合入 main。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | workbench.rs 三 stub 实装(place/interact/break) | ⬜ |
| P1 | PlaceableBlockKind 抽象(带交互世界实体的放置底盘) | ⬜ |
| P2 | client 交互闭环 + Workbench bbmodel 接入 | ⬜ |

---

## 接入面(防孤岛 checklist)

- **进料**:
  - `craft/workbench.rs:55-61` 三 stub(NOTE 写"将在 PR-3 实装",PR-3 已 merge 仍未补——**所有容器放置阻塞的根因**,调查红旗 #5)
  - lifecycle 已落地:`world/block_place.rs`(BlockPlaceRequest handler)+ `bong_blocks.rs`(place/remove)+ `block_break.rs`(DiggingEvent)
  - `is_within_workbench_range`(workbench.rs:47)+ 工作台配方系统(plan-workbench-recipes-v1,finished)
  - Workbench bbmodel(`local_models/Workbench.bbmodel`,PR #468 已重做)
- **出料**:
  - 放置后的工作台 world entity(带交互距离判定)→ 解锁"靠近才能合成"真实约束
  - `PlaceableBlockKind` enum + 通用 handle_place/interact/break 派发 → [[plan-placeable-container-blocks-v1]] 直接进料
- **共享类型 / event**:复用 lifecycle 的 `BlockPlaceRequest`;**不另造 WorkbenchPlace 协议**(原 base 骨架的 WorkbenchPlace C2S 方案废弃,统一走 BlockPlaceRequest + kind 派发)
- **跨仓库契约**:client interactBlock 放置分支(lifecycle P4)+ 交互 C2S IntentHandler(禁止 vanilla entity hack);agent 不参与
- **worldview 锚点**:工作台=凡人制造业入口(worldview §九 经济);"凡物不设数量限制"决议(workbench.rs:10)沿用
- **qi_physics 锚点**:无。

---

## P0 — workbench.rs 三 stub 实装

- `handle_workbench_place` / `handle_workbench_interact` / `handle_workbench_break` 从注释变真 system(注册 ECS schedule)
- 放置消耗物品 → spawn 工作台 entity(Position + 标记 component);破坏 → 返还物品/掉落
- 测试:place→interact(范围内合成可用/范围外拒)→break 全链 e2e;重启持久化

## P1 — PlaceableBlockKind 抽象

- enum 变体:Workbench / StorageCrate / DeadDrop / ...(后续 plan 扩)
- 通用派发:BlockPlaceRequest 按物品 template→kind 路由到各自 handler;通用 break 回收
- 测试:每变体专属 pin 测试;未知 kind 拒绝分支

## P2 — client 交互闭环 + bbmodel

- 工作台实体渲染挂 Workbench.bbmodel(geo 导出走既有 entity 模型管线);交互按 IntentHandler 模式
- 视听:放置 SFX `block.wood.place`(pitch 0.9)+ 落座尘土粒子(BongSpriteParticle burst 4 颗 #8B7355);破坏 SFX `block.wood.break` + 碎屑;**接活 WorkbenchConstants.java:15 的 7 条 SFX/3 组 VFX 常量 stub**(调查红旗 #7:常量无资产无调用,实装或删除)
- 测试:client e2e 放置→渲染→交互→破坏;SFX/VFX 触发断言

---

## §8 开放问题(P0 决策门前需收口)

1. **工作台 entity 表示**:bong_blocks 方块 + 伴生 entity vs 纯 entity(参 lifecycle 已落地机制选齐一种)
2. **"靠近才能合成"启用时机**:P0 落地即强制(现有玩家流程变化)vs 配置开关过渡
3. **原 base 骨架 §8 的 vanilla 占位决议**:容器类沿用"vanilla 占位+bbmodel 换皮"还是直接 bong_blocks(跟 lifecycle 实际落地方式对齐)
