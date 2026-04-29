# Skeleton 未决事项登记

本目录下 plan 在展开/落地过程中累积的**延后处理**事项。每条带 plan 锚点 + 上下文，后续回来解决时直接对号入座。

---

## plan-death-lifecycle-v1

- [ ] §4a 寿元系统（1 real hour = 1 in-game year）需验证：与 51.5h 化虚基线、与亡者博物馆时间戳、与 agent 长线叙事节奏是否协调
- [ ] §4c 续命路径（续命丹/夺舍/坍缩渊换寿）全部未实装，只写方向
- [ ] "风烛" buff 具体数值未定
- [ ] 老死的"善终 vs 横死"生平卷分类字段未落

---

## plan-tribulation-v1

- [ ] 渡虚劫全服广播的截胡机制：其他玩家赶路需 10-20 分钟（worldview §十三），预兆窗口具体给多长未定
- [ ] 域崩触发阈值（灵气值 × 持续抽吸时长）未量化
- [ ] **半步化虚 buff 强度未定**（§3）：当前 +10% 真元上限 / +200 年寿元是占位。待 Phase 1-3 上线后看"卡在半步化虚"的玩家比例再定（buff 过强 → 玩家故意卡；buff 过弱 → 服务器稀疏时没人渡；名额释放后升级机制也待定）
- [ ] **欺天阵接口延后到 plan-zhenfa**（§5 / Phase 5）：阵法系统尚未立项，tribulation Phase 5 的"欺天阵接口"先不实装，只保留定向天罚的隐性层（劫气标记 / 概率操控 / 灵物密度热图）

---

## plan-zhenfa-v1

- [ ] 欺天阵的"假劫气权重"如何进入天道密度计算管线 — 依赖天道 agent 推演层接口
- [ ] 阵法持久化方案（存档量级）未评估

---

## plan-lingtian-v1

- [ ] 灵田 tick 抽取 `zone.spirit_qi` 的具体速率（ZONE_LEAK_RATIO）已落但未做过生产环境平衡测试
- [ ] 与天道密度阈值（plan-zhenfa 也用）的共享接口：`zhenfa_jvling` 钩子已预留在 `environment.rs`，等 plan-zhenfa P0 落地后填值

---

## plan-combat-no_ui

- [ ] `AntiCheatCounter` component + CHANNEL_ANTICHEAT 推送（plan §1.5.6）：当前 reach/cooldown/qi_invest 三道 clamp 分散在 `resolve.rs`，无统一违规计数 + 上报通道
- [ ] 遗念 agent `deathInsight` tool：跨到修炼 plan + agent-v2 scope，等 death-lifecycle 立项时再对齐

---

## plan-alchemy-v1（server-only，未转入新 plan 的后续项）

### 测试 JSON 的占位

- [ ] `server/assets/alchemy/recipes/*.json` 中的三份示例（kai_mai / hui_yuan / du_ming）仅为测试，不进生产。正式配方名称正典化已转入 `plan-alchemy-client-v1 §7 P5`
- [ ] `side_effect_pool` 里的 tag（`minor_qi_regen_boost` / `rare_insight_flash` / `qi_cap_perm_minus_1` 等）目前只是字符串，没接真实 debuff/buff 系统 — 等 StatusEffect 系统统一后映射

### 未落地开放设计

- [ ] 丹方残卷**损坏**（只能学到残缺版）— plan §1.4 提过，未定数据结构
- [ ] 品阶 / 铭文 / 开光（plan §7 TODO）— 全部 v2+
- [ ] AutoProfile 自动化炼丹（傀儡绑炉读曲线）— plan §1.3 预留口，未实装
- [ ] 丹心识别（玩家逆向配方）— worldview §九 "情报换命"钩子

---

## plan-inventory-v1（已知缺口）

- [ ] **塔科夫 grid placement 未实装**（`server/src/inventory/mod.rs add_item_to_player_inventory`）：目前 harvest drop 直接 `push({ row:0, col:0 })`，多株同时入包 row-col 冲撞，客户端渲染会堆叠异常
- [ ] **stacking 未实装**：`add_item_to_player_inventory(..., stack_count)` 不与既有实例合并，也不校验 stack_count 上限

---

## plan-alchemy-v1 SVG 草图

- [ ] `docs/svg/alchemy-furnace.svg` 的 Tarkov 背包缩略（每格 57×52）与实际 CELL_SIZE=28 不一致，只是草图可读性处理。真实渲染按 `GridSlotComponent.CELL_SIZE` 走

---

## 通用 / 跨 plan

- [ ] 所有 plan 的"开放问题"节尚未做过一次 review pass — 可能有早期假设已被后续决策推翻
- [x] ~~**采药工具系统未立**~~：已由 `plan-tools-v1`（骨架，2026-04-29 立）覆盖——7 件凡器（采药刀 / 刨锄 / 草镰 / 冰甲手套 / 骨骸钳 / 钝气夹 / 刮刀）；命名避用"灵\*"词头

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
