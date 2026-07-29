# plan-refactor-inventory-core-v1 — Inventory 巨石拆分 + 网格/交付事务一致性（重构轨 R10）

> 所属总纲：`plan-refactor-master-v1.md`。一句话：拆掉 20165 行的 `inventory/mod.rs`，把"给予/交付/拾取/堆叠/占格"改成统一事务 API（满包不丢物、先校验后扣、堆叠必合并、占格必同步）——物品凭空消失/孤儿物品整簇收口。

## 现状证据（2026-07-27 侦察）

- `server/src/inventory/mod.rs` 20165 行、目录 89% 代码在单文件——全仓第一大 god module。
- 交付路径各写各的：满包退款只记日志（craft-refund，active 接近完成）、满包取丹先清 session 丢产物（alchemy-takeback）、锻造产物满包丢失（forge-outcome，#1294 在飞）、满包强塞无碰撞检测叠孤儿物品（force-attach-grid-collision）。
- 一致性缺口：世界掉落拾取不合并已有堆叠（dropped-loot-pickup-stack-merge）、旋转后占格不同步 client（rotate-footprint-sync）、give 后快照拾取不到实例（bot-production-inventory-instance-visibility）、pack stow 无稳定回执（bot-inventory-pack-feedback）。
- 历史包袱：pre-#249 老存档卡旧布局（memory 已记，值得随拆分一并处理）；tarkov 套包 `owner_instance_id` 全栈已落地是既定架构，不动。

## 接入面

- **进料**：R1 session 产物交付调用、R4 gate 通过后的物品类请求、掉落物系统、R3 的持久化 slice（inventories 表）。
- **出料**：统一 `InventoryTxn` API：`deliver(items) -> Delivered | Spilled(fallback)`（满包溢出策略统一：脚下掉落/暂存箱，按 worldview 拍板）、`consume_checked`（先校验后扣）、`merge_stack`、占格变更事件（S2C 经 R6）。
- **共享类型**：`ItemCategory` 合法集不动（无 Material，材料用 Misc——历史坑）；`owner_instance_id` 架构不动。
- **worldview 锚点**：物品不凭空消失对齐末法稀缺经济（§十三物资锚点）；含真元物品的销毁/溢出走 R5 ledger。

## 阶段

- ⬜ P0 设计收口 + 吸收清单验真：mod.rs 职责普查出拆分图（grid/txn/container/corpse/shelflife 接缝）；冻结 `InventoryTxn` API 与满包溢出策略；等 craft-refund P4、#1294 相关项定基线。
- ⬜ P1 巨石拆分：按职责拆文件（行为不变，测试平移），`InventoryTxn` 骨架上线。
- ⬜ P2 交付路径统一：give/craft/alchemy/forge/loot 全部改走 `deliver`；先校验后扣全量化；满包场景全绿。
- ⬜ P3 网格/堆叠一致性：拾取合并、占格同步、pack 回执、老存档布局迁移补课。
- ⬜ P4 bot 验收 + 吸收 plan 批量归档。

## 吸收清单（短名省略 plan-bughunt- 前缀与 -v1 后缀）

skeleton：alchemy-takeback-full-inventory-loss（交付垫层部分；session teardown 归 R1）、dropped-loot-pickup-stack-merge、force-attach-grid-collision、rotate-footprint-sync、bot-inventory-pack-feedback、bot-production-inventory-instance-visibility；在飞 #1294：forge-outcome-full-inventory-loss。
**不吸收**：craft-refund-full-inventory-loss（active，P0-P3 已 ✅，独立收尾 P4）；container-filter-and-completion（feature，独立）；nested-pack-base（已 WITHDRAWN，#1275）。

## 文件所有权与边界

- 独占：`server/src/inventory/**`、各域交付调用点的替换行。
- 不碰：`InspectScreen`（R7 域）；session 生命周期（R1）；掉落物拾取的 gate 校验（R4）。
- 依赖：R3 P1（persistence 拆分先行，inventories 表接缝清晰）；与 R1 的交付接缝 API 由本轨定义、R1 消费。Wave 2 开工，P0 普查可先行。

## bot 验收场景

1. `inv_full_delivery_matrix`：满包状态下 craft 完工/取丹/锻造出炉/给予→断言产物按统一溢出策略落地，总数不丢。
2. `inv_stack_merge`：拾取同类掉落→断言合并入既有堆叠。
3. `inv_footprint_sync`：旋转/移动占格物品→断言 client 快照占格一致（P6 protobuf 深断言）。
4. `inv_give_visibility`：dev give 后立即快照→断言实例可见可拾取（修 bot 基建自身的假阳性）。

## 开放问题（pre-P0 收口）

1. 满包溢出策略正典拍板：脚下掉落（可被他人捡走，符合末法残酷）vs 个人暂存箱（体验友好）——需人工定。
2. pre-#249 老存档迁移是否并入本轨 P3（倾向并入，一次清账）。
