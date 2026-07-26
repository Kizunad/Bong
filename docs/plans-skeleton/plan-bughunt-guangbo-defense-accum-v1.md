# BugHunt: 广播体操 defense_profile 每 tick 无限累加至 85% 全类型减伤

## Bug 摘要

**严重度：critical（潜伏态——当前被 `plan-bughunt-r10-findings-v1` P1 的熟练度断裂掩盖，r10 P1 修复即引爆；dev 命令今天就可复现）**

`body.guangbo_ticao`（广播体操）的四肢防御加成走的是**每 tick 向 `DerivedAttrs.defense_profile` 做 `+=` 累加**，而 `defense_profile` 的唯一重建/重置点是 `Changed<PlayerInventory>` 触发的护甲重扫。玩家只要不换装备，加成就每 tick 叠一层，直到撞上 `ARMOR_MITIGATION_CAP = 0.85`——设计上限是 **+0.5%** 四肢防御（`LIMB_DEFENSE_BONUS_MAX = 0.005`），实际会漂到 **85% 全伤型减伤**，裸体站桩即可达成，偏差 170 倍。

关键机制链：

1. `apply_guangbo_ticao_bonuses`（`server/src/combat/body_conditioning.rs:157`）在 `body_conditioning.rs:169-181` 对 4 肢 × 5 伤型共 20 个 entry 执行 `.and_modify(|v| *v = (*v + limb_def).min(ARMOR_MITIGATION_CAP))`（关键行 `body_conditioning.rs:176`）——**有 cap 无 reset**。
2. 驱动系统 `body_conditioning_aggregate`（`body_conditioning.rs:183`）注册于 `server/src/combat/mod.rs:320-323`，`in_set(CombatSystemSet::Physics).after(status::attribute_aggregate_tick)`，**无 run condition、无 Changed 过滤，每 tick 全量执行**。
3. `defense_profile` 全仓唯一重建点 `sync_armor_to_derived_attrs`（`server/src/combat/armor_sync.rs:84-92`）带 `Changed<PlayerInventory>` filter（`armor_sync.rs:85`）——不换装永不重置。
4. **对照组证明这是遗漏而非设计**：同函数 `body_conditioning.rs:166-167` 的 `move_speed_multiplier` / `jump_height_multiplier` 也是每 tick 乘入，但它们不漂——因为 `attribute_aggregate_tick`（`server/src/combat/status.rs:163`）在 `status.rs:171-175` 每 tick 把 `attack_power/defense_power/move_speed_multiplier/jump_height_multiplier/qi_max_multiplier` 显式重置为 1.0。**`defense_profile` 恰恰不在这份重置清单里。**
5. 消费端 `apply_armor_mitigation`（`server/src/combat/resolve.rs:133` 读 `derived.defense_profile`）直接吃这张表 → 四肢命中裸体吃满 85% 减伤成立。

饱和速度：`limb_def = LIMB_DEFENSE_BONUS_MAX × prof`，prof=1.0 时约 170 tick（<9 秒）饱和；prof=0.01（练一次）约 17000 tick（~14 分钟）同样饱和到 0.85。

## 可达性（诚实声明）

- **当前生产路径不可达**：`plan-bughunt-r10-findings-v1` P1 确认 `GuangboTicaoPracticeEvent` 生产端为零 → 熟练度恒 0 → `limb_def = 0` 走不进 `body_conditioning.rs:170` 的 `if limb_def > 0.0`。本 bug 处于**潜伏态**。
- **dev 命令立即可达**：`/technique add body.guangbo_ticao` + `/technique proficiency body.guangbo_ticao 1.0` 后站桩 ~9 秒即可复现 85% 减伤。
- **落地顺序硬约束**：本 plan **必须先于或随同 r10 P1（熟练度闭环接通）落地**——先修 r10 P1 会把潜伏 critical 直接推上生产。

## 实际游玩体验影响（r10 P1 接通后）

- 任何练过一次广播体操的玩家，站着不动十几分钟后四肢获得 85% 全伤型（Cut/Blunt/Pierce/Burn/Concussion）减伤，等效于满配护甲 cap，**裸体免费拿到本应由整套护甲 + 丹药叠出来的极限值**。
- 广播体操是入门级锻体功法（scroll 习得），却提供了越过所有装备 progression 的防御——护甲、锻体、境界的防御成长曲线全部作废。
- PvP 中先手方只要提前挂机蓄叠，四肢部位近乎免疫，命中部位随机性会让实际 TTK 剧烈失真。

## 证据定位

- `server/src/combat/body_conditioning.rs:157`（`pub fn apply_guangbo_ticao_bonuses`）
- `server/src/combat/body_conditioning.rs:169-181`（累加循环；`:176` `.and_modify(|v| *v = (*v + limb_def).min(ARMOR_MITIGATION_CAP))`）
- `server/src/combat/body_conditioning.rs:183`（`pub fn body_conditioning_aggregate`，无过滤每 tick）
- `server/src/combat/mod.rs:320-323`（系统注册，`after(status::attribute_aggregate_tick)`）
- `server/src/combat/body_conditioning.rs:26`（`const LIMB_DEFENSE_BONUS_MAX: f32 = 0.005;` 设计上限 0.5%）
- `server/src/combat/armor.rs:25`（`pub const ARMOR_MITIGATION_CAP: f32 = 0.85;` 实际饱和值）
- `server/src/combat/armor_sync.rs:84-92`（唯一重建点，`Changed<PlayerInventory>` @ `:85`）
- `server/src/combat/status.rs:163` + `:171-175`（`attribute_aggregate_tick` 每 tick 重置清单——**不含 `defense_profile`**，对照组铁证）
- `server/src/combat/resolve.rs:133`（`apply_armor_mitigation` 消费 `defense_profile`）
- `server/src/combat/body_conditioning.rs:355-368`（既有 cap 测试只单次调用 `apply_guangbo_ticao_bonuses`，无多 tick App 循环——这就是它没被抓住的原因）

## 触发路径

1. 玩家习得 `body.guangbo_ticao`（`scroll_body_guangbo_ticao`）且熟练度 > 0（当前需 dev 命令 `/technique proficiency`；r10 P1 修复后自然施放即可累积）。
2. 每 tick `body_conditioning_aggregate` → `apply_guangbo_ticao_bonuses` 向 `defense_profile` 的 20 个 (肢体, 伤型) entry 各 `+= limb_def`。
3. 玩家不触发 `Changed<PlayerInventory>`（不穿脱装备）→ 无任何重置。
4. N tick 后所有肢体 entry 达到 0.85，`apply_armor_mitigation` 按 85% 削减一切四肢伤害。

## 对抗核验记录

- 2026-07-26 防御系统全链路审计发现，经独立 read-only Explore agent 逐行核验：累加逻辑、注册方式、重建触发器、重置清单缺席四点全部 CONFIRMED。
- 审计初稿称"无限累加"——核验修正为**饱和于 `ARMOR_MITIGATION_CAP = 0.85`**（`:176` 有 `.min()`），但 0.5% 设计值 vs 85% 实际值的 170 倍偏差不变。
- 核验排除"设计如此"：同函数的 move_speed/jump 加成因 `attribute_aggregate_tick` 重置清单而幂等，`defense_profile` 是清单唯一漏项；`LIMB_DEFENSE_BONUS_MAX` 命名与文件头 doc comment（`body_conditioning.rs:2`「+0.5% 四肢防御」）均自证意图。
- 与 `plan-bughunt-r10-findings-v1` P1 去重确认：r10 P1 修**熟练度增长断裂**（事件无生产者），本 plan 修**加成应用幂等性**——同模块两个独立缺陷，且存在上述落地顺序耦合。

## Skeleton Fix Plan

两条候选路线，由修复 subagent 第一性原理拍板（推荐 A）：

- **路线 A（推荐）——limb 加成改独立字段，幂等化**：
  - [ ] `DerivedAttrs` 新增 `limb_defense_bonus: f32` 之类独立字段（或 `guangbo_limb_defense`），`apply_guangbo_ticao_bonuses` 每 tick **直写**（`=` 而非 `+=`）该字段，不再碰 `defense_profile`。
  - [ ] `apply_armor_mitigation`（`resolve.rs:117-151`）消费时对四肢部位把 `defense_profile` 查表值与 `limb_defense_bonus` **相加后统一 `clamp(0.0, ARMOR_MITIGATION_CAP)`**（保持 `plan-layered-equip-v1` 写死的"resolve 侧最终唯一兜底"语义）。
  - [ ] 优点：护甲重建保持 `Changed<PlayerInventory>` 懒惰语义不动；锻体加成天然幂等；两个数据源职责分离。
- **路线 B——纳入每 tick 重置**：`attribute_aggregate_tick`（`status.rs:171-175`）重置清单补 `defense_profile` 清空 + `sync_armor_to_derived_attrs` 去掉 `Changed` 过滤每 tick 重建。改动更小但护甲矩阵每 tick 全玩家重建，且把懒重建语义改成热路径——仅当路线 A 与其他 `defense_profile` 写入方有冲突时才选。
- [ ] 无论路线：不改 `LIMB_DEFENSE_BONUS_MAX = 0.005` 数值本身（设计代价曲线不动，只修幂等性）。
- [ ] 检查 `defense_profile` 是否还有第三个写入方（核验时全仓只有 `armor_sync.rs:89` 与 `body_conditioning.rs:174` 两处生产写入，修复时复核一遍防新增）。

## 验收测试计划

**server/ cargo test（`body_conditioning` + `armor_sync` 集成）**：

- **多 tick 不增长（本 bug 的直接锁定，必须是真 App 多 tick 循环）**：构造 App 注册 `attribute_aggregate_tick` + `body_conditioning_aggregate`（+ 路线 A 时的 resolve 消费函数），KnownTechniques 挂 `body.guangbo_ticao` prof=1.0，跑 2 tick 与 200 tick 各取一次四肢有效减伤值，断言两者相等且等于 `guangbo_ticao_limb_defense(1.0)` 的单次期望值（取 const 引用组装期望，不写字面数）。失败信息写明「期望锻体加成幂等（+0.5%×prof），实际第 200 tick 漂移到 X」。
- happy path：prof=0.5 裸体四肢命中，减伤 == limb_def 期望值；躯干/头部命中不受 limb 加成影响。
- 边界：穿满甲（护甲某肢体 entry 已 0.849）+ limb 加成，总减伤 clamp 到 `ARMOR_MITIGATION_CAP`（取 const 引用断言）。
- 状态转换：换装触发 `Changed<PlayerInventory>` 重扫后，limb 加成仍正确存在（路线 A 天然通过；路线 B 需专门锁定）；`/technique remove` 卸掉功法后下一 tick 加成归零。
- 错误分支：prof=0（r10 P1 现状）时 `defense_profile`/新字段完全不被触碰。
- 回归：既有 `body_conditioning.rs:355-368` cap 单测保留；move_speed/jump 每 tick 幂等的既有行为不回归。

## 风险

- 路线 A 动 `DerivedAttrs` 结构体——并行 PR 若同时改该 struct，merge 后须防 E0062 重复字段（合并主线后重跑完整门禁）。
- `apply_armor_mitigation` 是热路径纯函数（`resolve.rs:133` 的 `?` 提前返回依赖 entry 缺失），路线 A 给四肢部位引入"无护甲 entry 也有 limb 加成"的新分支时，注意不要破坏"NPC defense_profile 恒空 → 不减伤"的现状（NPC 穿甲减伤归 `plan-npc-combat-gear-v2`，本 plan 不扩大范围）。
- 与 r10 P1 的顺序耦合是**调度约束**不是代码依赖：两者代码互不冲突，可独立 PR，但 r10 P1 先合且本 plan 未合的窗口期 = 生产可达的 critical，主干调度时注意。

## 与其他 plan 的关系

- `docs/plans-skeleton/plan-bughunt-r10-findings-v1.md` P1：同模块上游断裂（熟练度恒 0），修好即引爆本 bug——见「可达性」落地顺序硬约束。
- `docs/plans-skeleton/plan-defense-hardening-v1.md`（同批立案）：P0 全局减伤 cap 是本 bug 的纵深防御层（即使再出现类似漂移，总减伤也被全局 cap 压住），但不替代本修复。
- `docs/finished_plans/plan-layered-equip-v1.md`：`ARMOR_MITIGATION_CAP` 三层 clamp 语义的 owner，路线 A 的"相加后统一 clamp"必须保持其"resolve 侧最终唯一兜底"约定。
