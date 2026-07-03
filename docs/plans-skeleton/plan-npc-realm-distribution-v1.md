# plan-npc-realm-distribution-v1 — NPC 境界真实分布：修"realm 被吞" + seeder 加权抽样

> **骨架**（2026-07-03）。一句话：先修掉"spawn 链路把 realm 参数丢弃、全员落回醒灵"的 choke point bug（修完派系首领/TSY/hydrate 的既有境界逻辑**自动全部生效**），再给自然种群 seeder 按 zone 灵气档 + 确定性哈希做境界分布，让末法散修世界有真实的强弱长尾。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | choke point 修复（realm 写进 Cultivation） | ⬜ |
| P1 | 种群 seeder 境界分布（zone 加权 + 确定性） | ⬜ |
| P2 | 境界-功法-视觉单一来源一致性收口 | ⬜ |
| P3 | 感知面（视觉档 / narration / 存量迁移） | ⬜ |

## 接入面（docs/CLAUDE.md §二）

- **进料**：`npc_runtime_bundle_with_age`（`npc/lifecycle.rs:596-619`，现恒 `Cultivation::default()`=醒灵）；spawn 三处 `rogue.rs:236-291` / `disciple.rs:94-160` / `commoner.rs:51-87`（realm 参数现只喂视觉/功法）；种群 seeder `dormant_rogue_seed_snapshot`（`npc/dormant/mod.rs:1274-1285`）；zone 灵气分档 `classify_zones_by_qi`（`dormant/mod.rs:1145-1148`）；确定性哈希 `deterministic_hash(char_id, salt)`（同 `seed_rogue_faction :1228` 模式）
- **出料**：正确的 `Cultivation.realm` → 下游**零改动自动生效**：战力 `compute_combat_power`（`combat_power.rs:22-46`，realm_ordinal×20）、离屏战争结算（`dormant/combat.rs:57`）、死亡遗物门槛（`combat.rs:245`，固元起）、AI 威胁评估（`brain/threat.rs:134-249`，realm_delta 权重 0.4）、全部招式境界门控、寿元 `LifespanComponent::for_realm`
- **共享类型**：`Realm` 枚举 / `Cultivation`（`cultivation/components.rs:15-22,386-398`）不动；既有身份境界逻辑 `leader_realm_for`（`faction.rs:1123-1130`）、TSY 硬编 realm（`tsy_hostile.rs:778/922`）复用不重造
- **跨仓库契约**：无 wire 变更（NPC cultivation 已在 world_state 快照内）；agent 侧天道推演读到的散修境界将首次真实
- **worldview 锚点**：§三:61 六境界正典；§一 末法时代（高境界稀有 → 分布必须长尾）；§七:739 智能 NPC 散修
- **qi_physics 锚点**：分布本身不动真元；NPC qi_max/吸收随 realm 由既有系统派生（`plan-zone-qi-economy-v1` 的 NPC 让灵地板与预算需按新境界结构重估——见 §8 #4，跨 plan 联动不在本 plan 写公式）

## 背景调研结论（2026-07-03）

两层 bug 叠加：

1. **realm 参数在 spawn 链路被丢弃**（更深层）：rogue/disciple/commoner 的 spawn 函数都收 `realm` 参数，但只用于 `select_npc_visual_profile` / `npc_meridian_system_for_realm` / `assign_npc_techniques`，最后 insert 的 `npc_runtime_bundle_with_age` 恒用 `Cultivation::default()` 覆盖（`rogue.rs:290` / `disciple.rs:159` / `commoner.rs:86`）。于是**既有境界逻辑全部沦为装饰**：派系首领（青云猎户→固元、苍原商队→通灵）、TSY 道乡引气/执念凝脉、hydrate 快照境界，实际全被吞成醒灵
2. **种群 seeder 写死默认**：genesis 初始人口全走 `Cultivation::default()`（`dormant/mod.rs:1285`），无任何分布逻辑
3. 全仓唯一真正非醒灵的 NPC = 垂死大能（化虚，`fauna/dying_elder.rs:391-392`，直接构造未经 bundle）
4. **一致性隐患**：NPC 按"意图境界"分到凝脉门槛功法，Cultivation 却是醒灵——战力/威胁/遗物判定与其功法自相矛盾

## P0 choke point 修复 ⬜

- `npc_runtime_bundle_with_age`（`lifecycle.rs:596`）加 `realm: Realm` 入参并写进 `Cultivation`（qi_max 随 realm 由既有容量曲线派生，不得手写常数）；三处 spawn 调用点透传
- 修完自动生效面 pin 测试：派系首领 spawn 后 `Cultivation.realm == leader_realm_for(faction)`；TSY 道乡 == Induce、执念 == Condense；hydrate 往返 `snapshot.cultivation.realm` 不丢；dev 命令显式 realm 直达组件
- 回归锁：`dying_elder` 化虚路径不被 bundle 化改动误伤

## P1 种群 seeder 境界分布 ⬜

- `dormant_rogue_seed_snapshot`（`dormant/mod.rs:1285`）替换 `Cultivation::default()`：按 `deterministic_hash(char_id, "realm")` 抽样，权重按 zone 灵气档（`classify_zones_by_qi` 现成 resource/background 二分）查分布表
- 分布表（数值 §8 #1 收口，末法长尾基调）：background zone 约 醒灵 55 / 引气 30 / 凝脉 12 / 固元 3 / 通灵 0；resource zone 整体上移一档、通灵 ≤1%；**化虚不自然刷**（正典稀有，仅垂死大能类稀有实体）
- 确定性要求专属测试：同 seed 两次 genesis 境界逐 NPC 一致；分布直方图区间 pin
- 与 `plan-ambient-threat-v1` 的物种池 / `plan-zone-qi-economy-v1` 的 NPC 吸灵预算合账（§8 #4）

## P2 一致性收口（单一来源） ⬜

- 功法 / 经脉 / 视觉 profile / trade inventory 全部从**最终写入的** `Cultivation.realm` 派生（消灭"意图 realm ≠ 组件 realm"双源）
- audit 测试：任意 spawn 路径出来的 NPC，`assign_npc_techniques` 结果里每条 technique 的 `required_realm ≤ cultivation.realm`；视觉档位与 realm 映射 pin

## P3 感知面 ⬜

- 视觉：`select_npc_visual_profile` 已按 realm 分档（现成，验证接的是修复后 realm）
- narration 示例（zone / perception）：「集市角落那个补锅匠收锤时指缝漏了一缕凝而不散的白气——凝脉，藏得很深」「北荒来的独行客靴底不沾尘，固元境的横行是写在步子里的」
- 高境界 NPC 气息粒子（可选，§8 #5）：固元+ 常驻 `BongRibbonParticle` 极淡雾带 ×1、lifetime 40t 循环、颜色随 qi_color 染色、半径 0.4m——玩家"眼力"辨境界的物理线索
- 存量迁移：已 seed 的 dormant 人口全是醒灵——重 roll 还是自然留存（§8 #3）

## §8 开放问题（升 active / P0 决策门前收口）

1. **分布表数值**：background/resource 两档的具体百分比；通灵是否开放自然涌现；与在线人口上限（数百 dormant）相乘后的各档绝对数量预估
2. **qi_max 派生口径**：worldview §三:171 真元池容量曲线（境界×经脉复利）在 NPC 侧的简化实现用哪个既有函数——严禁本 plan 手写曲线
3. **存量存档迁移**：重 roll 全体 dormant（简单但打破连续性）vs 只对新 seed 生效（旧人口永远醒灵）vs 一次性迁移脚本
4. **跨 plan 预算联动**：高境界 NPC 吸灵更凶，`plan-zone-qi-economy-v1` 的 NPC 让灵地板 0.3 与 equilibrium 数值是否要按境界结构重估
5. **气息粒子是否做**：末法"藏拙"基调下高境界该更难辨认还是更好认——正典倾向前者，粒子可能反直觉，需拍板
