# Skeleton 未决事项登记

本目录下 plan 在展开/落地过程中累积的**延后处理**事项。每条带 plan 锚点 + 上下文，后续回来解决时直接对号入座。

---

## plan-alchemy-v1

### 依赖外部 plan 尚未立项

- [ ] **plan-botany-v1**（植物/采集）未立 — alchemy §3 的材料全是 placeholder ID (`kai_mai_cao` / `ling_shui` / `xue_cao` / `shou_gu` / `huo_jing` / `bai_cao`)。botany 落地后需替换为真实 item registry，并定义"灵气 > 0.3 才生长"接入。
- [ ] **plan-forge-v1**（炼器）未立 — 待决：炉体是否抽象共用？若共用则 `AlchemyFurnace` 要提前考虑 `CraftingKind` 枚举。
- [ ] **plan-inventory-v1** 缺：
  - 丹方残卷 item 定义（1×2，携带 `recipe_id`）
  - 所有材料 placeholder item 定义 + 图标
  - pill item 的"操作磨损"（worldview §七）未实现

### 测试 JSON 的占位

- [ ] `server/assets/alchemy/recipes/*.json` 中的三份示例（kai_mai / hui_yuan / du_ming）**仅为测试**，不进生产。生产配方等 botany + 真实平衡调参后重出。
- [ ] `side_effect_pool` 里的 tag（`minor_qi_regen_boost` / `rare_insight_flash` / `qi_cap_perm_minus_1` 等）目前只是字符串，没接真实 debuff/buff 系统 — 等 StatusEffect 系统统一后映射。

### 未落地的开放设计

- [ ] 丹方残卷**损坏**（只能学到残缺版）— plan §1.4 提过，未定数据结构
- [ ] **续命丹**（plan-death-lifecycle §4c）— alchemy 有能力承载，但具体代价曲线未定
- [ ] 品阶 / 铭文 / 开光（plan §7 TODO）— 全部 v2+
- [ ] AutoProfile 自动化炼丹（傀儡绑炉读曲线）— plan §1.3 预留口，未实装
- [ ] 丹心识别（玩家逆向配方）— worldview §九 "情报换命"钩子

---

## plan-death-lifecycle-v1

- [ ] §4a 寿元系统刚加入（1 real hour = 1 in-game year），需验证：与 51.5h 化虚基线、与亡者博物馆时间戳、与 agent 长线叙事节奏是否协调
- [ ] §4c 续命路径（续命丹/夺舍/坍缩渊换寿）全部未实装，只写方向
- [ ] "风烛" buff 具体数值未定
- [ ] 老死的"善终 vs 横死"生平卷分类字段未落

---

## plan-tribulation-v1

- [ ] 渡虚劫全服广播的截胡机制：其他玩家赶路需 10-20 分钟（worldview §十三），预兆窗口具体给多长未定
- [ ] 域崩触发阈值（灵气值 × 持续抽吸时长）未量化

---

## plan-zhenfa-v1

- [ ] 欺天阵的"假劫气权重"如何进入天道密度计算管线 — 依赖天道 agent 推演层的接口
- [ ] 阵法持久化方案（存档量级）未评估

---

## plan-social-v1

- [x] ~~匿名稀疏关系图的数据结构（邻接表？事件流衍生？）未定~~ ✅ plan-social-v1 §3.1 定为 `HashMap<CharId, Vec<Relationship>>` 稀疏图
- [x] ~~"声名"由行为累积 — 累积公式未写~~ ✅ plan-social-v1 §4 双轴 fame/notoriety + top 5 显示

### 灵龛防御模式（新 plan 承接）

- [ ] **灵龛应支持"防御模式"**（来源：2026-04-16 社交 plan 决策），但不归 plan-social-v1 写
  - 基础灵龛（plan-social §2）只防"被发现前"的主动攻击（NPC）+ 方块不破（玩家）
  - 防御模式设想：
    - 可消耗额外"阵石 / 禁制"升级灵龛为**主动防御**
    - 入侵者进入 5 格触发反击（雷击 / 真元放射 / 幻象陷阱 等）
    - 可能与 plan-zhenfa 整合（阵法 block 叠加到灵龛）
    - 平衡考虑：防御模式本身会暴露"这里有灵龛"（靠伤害/声光），让"被发现"和"被防御"成为取舍
  - **承接 plan**：`plan-niche-defense-v1`（待立项）或并入 `plan-zhenfa-v1`
  - 本项**不在 plan-social-v1 内实装**，仅留 hook：`SpiritNiche { defense_mode: Option<DefenseModeId> }` 字段预留

---

## plan-lingtian-v1

- [ ] 灵田 tick 抽取 `zone.spirit_qi` 的具体速率未给
- [ ] 与天道密度阈值（plan-zhenfa 也用）的共享接口未定

---

## plan-alchemy-v1 SVG 草图

- [ ] `docs/svg/alchemy-furnace.svg` 的 Tarkov 背包缩略（每格 57×52 示意）与实际 CELL_SIZE=28 不一致，只是为了草图可读。真实渲染按 `GridSlotComponent.CELL_SIZE` 走。
- [ ] 投料槽的拖拽可用性 — DragState 当前只支持"背包↔装备/经脉/身体部位/丢弃"四类目标，需扩展 `FurnaceSlot` target

---

## plan-tribulation-v1

- [ ] **半步化虚 buff 强度未定**（§3）：当前写的 +10% 真元上限 / +200 年寿元是占位。待 Phase 1-3 上线后看"卡在半步化虚"的玩家比例再定：
  - buff 过强 → 玩家故意撞名额上限赚免费 buff
  - buff 过弱 → 服务器稀疏时没人想渡
  - 需同步考虑：名额释放后"升级"机制（半步→化虚是否自动 / 要不要再渡一次）

- [ ] **欺天阵实装延后到 plan-zhenfa**（§5 / Phase 5）：阵法系统尚未立项，tribulation Phase 5 的"欺天阵接口"**先不实装**，只保留定向天罚的隐性层（劫气标记 / 概率操控 / 灵物密度热图）。待 plan-zhenfa 落地后：
  - 定义欺天阵 block + 材料 + 布阵流程
  - 实装 10 min 静态衰减曲线（具体形状：匀速 vs 前高后低）
  - 与 `KarmaWeight` 对接（吸引天道注意力到假坐标）
  - 期间玩家对策仅靠"搬家 / 分仓 / 降消耗"

---

## plan-botany-v1 → 天道 agent 钩子（待 agent 侧接入）

- [ ] **订阅 `bong:botany/ecology` channel**（plan §7 生态可视化 · server 每 600 tick / ~30s 发布 `BotanyEcologySnapshotV1`）。
  - payload 结构：`{ v: 1, tick, zones: [{ zone, spirit_qi, plant_counts[{ kind, count }], variant_counts[{ variant: none|thunder|tainted, count }] }] }`
  - agent 用途：
    - 全局灵气重分配决策（plan-worldview §七 天道回收）—— 哪些 zone 植物密度过高/灵气透支
    - 天道观测稀有变种分布（Thunder / Tainted）—— 用于 narrative 事件埋点
    - 调试用：agent 掉线时靠 channel 追数据
  - Rust side：`crate::schema::botany::BotanyEcologySnapshotV1`，`RedisOutbound::BotanyEcology`
  - TS side：`@bong/schema` `BotanyEcologySnapshotV1` + `CHANNELS.BOTANY_ECOLOGY`

---

## plan-inventory-v1（并行开发中，botany 接入的已知缺口）

- [ ] **塔科夫 grid placement 未实装**（`server/src/inventory/mod.rs:376 add_item_to_player_inventory`）：目前每次 harvest drop 直接 `main_pack.items.push({ row:0, col:0, instance })`，不做空 slot 搜索也不做冲突检测。单种植物多株 / 多植物同时入包 row-col 全部冲撞在 (0,0)，客户端塔科夫格位渲染会堆叠显示异常。
- [ ] **stacking 未实装**：`add_item_to_player_inventory(..., stack_count)` 被 botany 调用时传 1，但如果后续传 >1，当前实现仅创建一个 `ItemInstance.stack_count = N` 而不与既有实例合并；也不校验 stack_count 与物品 `category` 的堆叠上限。
- botany 这边的调用点（`server/src/botany/harvest.rs::complete_harvest_for_player`）已按"每次给 1 株 = 1 instance"写；等 inventory plan 把 placement + stacking 接上，botany 侧不需要改——`ItemRegistry` + `add_item_to_player_inventory` 的合约保持即可。

---

## 通用 / 跨 plan

- [ ] 所有 plan 的"开放问题"节尚未做过一次 review pass — 可能有早期假设已被后续决策推翻
- [ ] **灵眼结构未实装**：worldview §十 明确灵眼是凝脉→固元突破必需 + 血谷"灵眼(不固定)"，但 server/worldgen 都没有这个实体/方块/坐标登记。botany 稀有灵草曾打算锚在灵眼旁，改用其他锚点后可等灵眼系统立项再回补。
- [ ] **采药工具系统未立**（botany §1.3）：目前右键即开小 session，后续加"采药刀 / 灵铲"影响品质/安全度，需要独立 item + 工具栏位接入
- [ ] **alchemy 配方 placeholder 材料名未改正典**：`docs/plan-alchemy-v1.md §3.2` 的 JSON 仍用 `kai_mai_cao / ling_shui / xue_cao / shou_gu / huo_jing / bai_cao`，按 botany §6 hook 需替换为 `ci_she_hao / ning_mai_cao / chi_sui_cao / hui_yuan_zhi` 等正典名（来自 `docs/library/ecology/末法药材十七种.json` + `辛草试毒录.json`），同时对齐辛度 → 丹毒色（Mellow/Sharp/Violent）
- [ ] **forge 配方材料未进 library**：`docs/plan-forge-v1.md §3.2` 用的 `xuan_iron / qing_steel / yi_beast_bone / ling_wood` 非正典；金属/骨需单独立 `plan-mineral-v1` / `plan-fauna-v1`，或补写入 `docs/library/ecology/` 或新开"矿物录"

---

> **约定**：每解决一条就从这里删。新增延后事项请直接追加到对应 plan 段，保持扁平。
