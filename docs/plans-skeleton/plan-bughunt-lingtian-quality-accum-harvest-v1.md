# plan-bughunt-lingtian-quality-accum-harvest-v1

## §0 摘要

灵田丰沛期品质加成 `CropInstance.quality_accum` 只在生长阶段累积（丰沛期 `plot_qi/cap >= 0.9` 每 tick +0.001），收获时被整体丢弃——`apply_harvest_completion` 用 `add_item_to_player_inventory(...)` 发放作物，未传 `customize_instance` 闭包，`spirit_quality` 恒为 `template.spirit_quality_initial`；随后 `plot.crop = None` 在没有读取 `quality_accum` 的情况下直接清空，数据永久丢失。认真维持 `plot_qi` 满仓/无杂染种出的作物，与刚勉强靠区域漏吸熟成的作物，拿到手的 `spirit_quality` 完全一样。

本 plan 仅是 BugHunt Skeleton Plan，不包含实际修复。

## §1 实际游玩体验影响

- 玩家开垦灵田、种下作物、维持 `plot_qi` 接近满仓（丰沛期）直到成熟收获，投入的额外灌溉/维护努力完全不体现在收获物品质上——认真经营和敷衍种植拿到的作物一模一样。
- 破坏灵田玩法"品质经营"这一核心设计意图：两份 finished plan（`plan-lingtian-v1`、`plan-lingtian-process-v1`）都隐性把"quality_accum 会累积"当作已完成功能记录，实际代码里这条从生长到收获的链路根本不存在——是文档自报完成、代码里静默漏接的孤岛。
- 走标准 起 Harvest session → `apply_harvest_completion` 结算路径，该函数是 lingtian `Update` 系统里处理会话完成的生产代码，非测试专用，全程无需任何 dev 命令。

## §2 复现路径

1. 玩家开垦灵田、种下任意可种作物。
2. 维持 `plot_qi` 接近满仓（丰沛期，`qi_ratio>=0.9`）直到成熟，期间每 lingtian-tick `quality_accum += bonus`（`server/src/lingtian/growth.rs:116-120`，唯一写入点）。
3. 走标准 Harvest session 完成结算路径。
4. 现状预期：`apply_harvest_completion`（`server/src/lingtian/systems.rs:969-1069`）调用 `add_item_to_player_inventory(inv, item_registry, allocator, plant_id, 1, now_lingtian_tick)`（`inventory/mod.rs:1779` 定义，`customize_instance` 固定传 `None`），其 `runtime_instance_from_template` 只会把 `spirit_quality` 设成 template 里的静态 `spirit_quality_initial`；随后 `plot.crop = None`（`systems.rs:1069`）直接把携带 `quality_accum` 的 `CropInstance` 丢弃，之前读到的 plot/crop 局部变量里从未提取过这个字段。
5. 修复后预期：收获物的 `spirit_quality` 反映该植株生长期实际累积的 `quality_accum`，丰沛期维护 = 更高品质收获、杂染 = 更低品质收获对玩家可见。

## §3 根因证据

- `server/src/lingtian/plot.rs:119` `CropInstance.quality_accum` 字段定义（`:127` 初始化为 0.0），注释"生长过程累积品质修饰"。
- `server/src/lingtian/growth.rs:116-120` `quality_accum += bonus` 是全仓唯一一处写入点（丰沛期每 tick +0.001，测试 `growth.rs:207/228/261` 印证该行为）。
- `server/src/lingtian/systems.rs:1013-1025`（函数体在 969-1069 区间）`apply_harvest_completion` 用 `add_item_to_player_inventory(...)` 发放作物，未传 `customize_instance` 闭包，`spirit_quality` 恒为 `template.spirit_quality_initial`。
- `server/src/lingtian/systems.rs:1069` `plot.crop = None` 在没有读取 `quality_accum` 的情况下直接清空，数据永久丢失。
- 全仓 grep `quality_accum`：写入点只有 `growth.rs:120` 一处，读取点除测试断言与字段定义外为零——`alchemy/processed_input.rs`（`plan-lingtian-process-v1` P4 声称"加工产物作为 pill_recipe 优选投料"的落点）里也搜不到 `quality_accum`。
- 对照 finished plan 的 Finish Evidence——`plan-lingtian-v1` 只勾选了"quality_accum 会累积"这一半（生长侧），`plan-lingtian-process-v1 §0` 明文的设计公理"原作物 quality_accum [0.8,1.5] → 加工产物 quality"、`plan-alchemy-recycle-v1` 勾选的"dye_contamination 影响 quality_accum"，都只字未提"quality_accum 如何真正体现在收获/加工产物上"的具体落点，而实测代码里这条链路根本不存在。

## §4 非重复比对

- 已读 `docs/plan-bughunt-lingtian-plot-qi-ledger-gap-v1.md`（active plan）：处理 `plot_qi` 记账缺口，未涉及 `quality_accum`→`spirit_quality` 链路，已读该文件确认未提 `quality_accum`。
- 已读 `docs/plan-bughunt-alchemy-freshness-feed-v1.md`：炼丹投料侧忽略衰减 factor，与本问题"收获瞬间就没把生长期加成写进物品"完全不同的输入源。
- 已读 `docs/finished_plans/plan-lingtian-v1.md` 与 `docs/finished_plans/plan-lingtian-process-v1.md` 的 Finish Evidence 章节：均未记录 `quality_accum` 到 `spirit_quality` 在收获时的接线落点，与已知/已记录的显式遗留项（如 `plan-lingtian-v1.md`"⏳ BlockEntity 持久化待 plan-persistence-v1"）不同，本项在两个 finished plan 里都被隐性记为"已完成"。
- Grep 全部 `docs/plan-bughunt-*.md` 与 `docs/plans-skeleton/plan-bughunt-*.md` 未命中 `quality_accum` 关键词的既有 bughunt 报告。

## §5 修复计划骨架

### P0 收获时读取并转化 quality_accum

- 在 `apply_harvest_completion` 清空 `plot.crop` 之前先读出 `crop.quality_accum`，改用 `add_customized_item_to_player_inventory` 并在 `customize_instance` 闭包里把它转换成 granted item 的 `spirit_quality`（例如 `template.spirit_quality_initial * (1.0 + quality_accum).clamp(...)`，或按 `plan-lingtian-process-v1 §0` 设计公理的 `[0.8, 1.5]` 区间映射），使丰沛期维护 = 更高品质收获、杂染 = 更低品质收获对玩家可见。

### P1 饱和测试

- 满仓丰沛期全程种植 vs `plot_qi=0` 全靠区域漏吸种植两组，断言收获物的 `spirit_quality` 有统计上可分辨的差异（而不是恒等于 template 默认值）。
- 补充边界测试：`quality_accum=0`（从未进入丰沛期）时收获物品质应与现状一致（回归保护）。

## §6 验证计划

- `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
- 手工复现矩阵：丰沛期全程维护种植 vs 勉强达标种植，收获后对比 `spirit_quality` 数值差异。

## §7 接入面与守恒说明

- 进料：`CropInstance.quality_accum`（生长阶段累积）、`ItemTemplate.spirit_quality_initial`。
- 出料：收获物 `ItemInstance.spirit_quality`。
- 跨端契约：本问题是纯 server 内部结算逻辑，物品品质数值变化通过既有库存快照通道传给客户端，不涉及新增 schema 字段。
- qi_physics：`quality_accum` 是灵田田块内部品质修饰系数，不直接对应 `zone.spirit_qi` 扣减/归还（该部分由 `plan-bughunt-lingtian-plot-qi-ledger-gap-v1` 单独覆盖），本 finding 修复不涉及真元转移，不新增 qi 常数或 ledger 流。

## §8 对抗复核结论

- 候选证据：`quality_accum` 全仓唯一写入点在 `growth.rs:120`；`apply_harvest_completion` 调用的 `add_item_to_player_inventory` 传 `customize_instance=None`，`runtime_instance_from_template` 只从静态 template 取值；`plot.crop = None` 前从未提取 `quality_accum`；唯一可能的下游救援 `alchemy/processed_input.rs::processed_alchemy_bonus`（`plan-lingtian-process-v1` P4）只被自己单测调用，从未被生产代码调用，不能挽救这条链路；`HarvestCompleted` 事件的全部消费者（`vfx_animation_trigger.rs`/`audio_trigger.rs`/`emit_harvest_inventory_snapshots`）都只是 VFX/音效/快照，从不触及 `spirit_quality`。
- 反方质疑：是否是已知/已记录的遗留项？是否与 `plot-qi-ledger-gap`/`alchemy-freshness-feed` 重复？
- 修正/反驳：`plan-lingtian-v1.md` 对已知遗留有显式标注惯例（如"⏳ BlockEntity 持久化待 plan-persistence-v1"），而 `quality_accum` 链路在两份相关 finished plan 里都被隐性记为已完成，不是显式标注的开放问题；`plot-qi-ledger-gap` 操作对象是 `LingtianPlot.plot_qi`（记账缺口），与本 finding"生长期品质累积在收获时丢失"是不同层面的问题，已确认该 active plan 未提 `quality_accum`；`alchemy-freshness-feed` 是炼丹投料侧忽略衰减 factor，输入源完全不同。
- 反方最终裁决：通过（`is_real: true`, `reachable: true`, `severity_adjust: unchanged`，保持 medium）。可达性完全正常游玩可达（标准种植-维护-收获循环），非重复，未修复，适合开 Skeleton Plan PR。
