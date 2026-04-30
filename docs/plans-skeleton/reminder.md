# Skeleton 未决事项登记

本目录下 plan 在展开/落地过程中累积的**延后处理**事项。每条带 plan 锚点 + 上下文，后续回来解决时直接对号入座。

---

## plan-tribulation-v1

- [ ] 渡虚劫全服广播的截胡机制：其他玩家赶路需 10-20 分钟（worldview §十三），预兆期 60s 只是锁定起点——截胡者在劫波期（波间冷却 15s × 3-5 波）期间抵达并截胡，设计上截胡是小概率情境事件。若上线后截胡率极低（玩家反馈太难赶到），考虑延长预兆期或加"天象广播提前量"
- [ ] 域崩触发阈值（灵气值 × 持续抽吸时长）未量化——等 plan-lingtian + plan-zhenfa P1 灵气消耗数据上线后再定基线
- [ ] **半步化虚 buff 强度未定**（§3）：当前 +10% 真元上限 / +200 年寿元是占位。待 Phase 1-3 上线后看"卡在半步化虚"的玩家比例再定（buff 过强 → 玩家故意卡；buff 过弱 → 服务器稀疏时没人渡；名额释放后升级机制也待定）
- [ ] **欺天阵接口延后到 plan-zhenfa**（§5 / Phase 5）：阵法系统尚未立项，tribulation Phase 5 的"欺天阵接口"先不实装，只保留定向天罚的隐性层（劫气标记 / 概率操控 / 灵物密度热图）

---

## plan-lingtian-v1

- [ ] 灵田 tick 抽取 `zone.spirit_qi` 的具体速率（ZONE_LEAK_RATIO）已落但未做过生产环境平衡测试——部署后需采集真实数据再调；首次调参建议降低 10-20%
- [ ] 与天道密度阈值（plan-zhenfa 也用）的共享接口：`zhenfa_jvling` 钩子已预留在 `environment.rs`，等 plan-zhenfa P0 落地后填值

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
>
> **已转为独立骨架（2026-04-30）**：
> - `plan-longevity-v1`（续命体系：续命丹/夺舍/坍缩渊换寿 + 风烛 buff + 善终/横死 + 寿元时钟）—— 覆盖原 plan-death-lifecycle-v1 §4a/§4c 未实装项
> - `plan-alchemy-advanced-v1`（高级炼丹：副作用接入/残卷/品阶/铭文开光/丹心识别/AutoProfile）—— 覆盖原 plan-alchemy-v1 v2+ 延后项
> - `plan-anticheat-v1`（战斗防作弊子系统：AntiCheatCounter + CHANNEL_ANTICHEAT）—— 覆盖原 plan-combat-no_ui §1.5.6 未实装项
> - `plan-inventory-grid-v1`（背包格位放置与堆叠：BFS free-slot + stacking merge）—— 覆盖原 plan-inventory-v1 已知缺口
>
> **已确认实装，直接删除**：
> - ~~plan-combat-no_ui 遗念 agent deathInsight tool~~：已实装（`server/src/combat/lifecycle.rs` 全路径建构 `DeathInsightRequestV1`，`agent/packages/tiandao/src/death-insight-runtime.ts` 订阅消费）
> - ~~plan-zhenfa-v1 欺天阵假劫气权重 + 持久化~~：已追加到 `docs/plan-zhenfa-v1.md §10` 开放问题
