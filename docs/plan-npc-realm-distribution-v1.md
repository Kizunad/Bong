# plan-npc-realm-distribution-v1 — NPC 境界真实分布：修"realm 被吞" + seeder 加权抽样

> **升 active 2026-07-03**（骨架成文 2026-07-03，同日收口 §8 全部开放问题后升 active）。一句话：先修掉"spawn 链路把 realm 参数丢弃、全员落回醒灵"的 choke point bug（修完派系首领/TSY/hydrate 的既有境界逻辑**自动全部生效**），再给自然种群 seeder 按 zone 灵气档 + 确定性哈希做境界分布，让末法散修世界有真实的强弱长尾。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | choke point 修复（realm 写进 Cultivation） | ⬜ |
| P1 | 种群 seeder 境界分布（zone 加权 + 确定性） | ⬜ |
| P2 | 境界-功法-视觉单一来源一致性收口 | ⬜ |
| P3 | 感知面（视觉档 / narration / 存量迁移） | ⬜ |

> 全部阶段仍标 ⬜——promote 只做"骨架→active"的收口升级（§8 开放问题全部拍板），不代表已实施。P0 第一个 PR 开工时再翻 ⏳。

## 接入面（docs/CLAUDE.md §二）

- **进料**：`npc_runtime_bundle_with_age`（`npc/lifecycle.rs:596-619`，现恒 `Cultivation::default()`=醒灵，且 `:608 LifespanComponent::for_realm(Cultivation::default().realm)` 同源醒灵化，二者必须一并修）**及其 2-arg 姐妹函数 `npc_runtime_bundle`（`lifecycle.rs:592`，转调 `_with_age(entity, archetype, 0.0)`，同样恒醒灵，2026-07-03 第二轮博弈复核补充——P0 必须同时给两个函数加 `realm: Realm` 形参，遗漏后者会让走此包装的全部身份 realm 站点继续被吞）**
  - `npc_runtime_bundle_with_age` 侧 spawn 调用点：`rogue.rs:236-291`（`spawn_rogue_npc_at`，insert 调用点 `:290`）/ `disciple.rs:94-160`（`spawn_disciple_npc_at`，insert 调用点 `:159`）/ `commoner.rs:51-87`（insert 调用点 `:86`）/ `rogue.rs:298-343`（`spawn_scattered_cultivator_at`，已持 `realm: Realm` 形参 `:306`，insert 调用点 `:340`）——四处的 `realm` 参数现全部只喂视觉/功法，最终 insert 调用点均不透传给 `Cultivation`
  - `npc_runtime_bundle`（2-arg）侧调用点：全仓 `grep -rn "npc_runtime_bundle(" server/src --include=*.rs`（排除 `_with_age`/其定义行）现命中 **28 处**（含测试）——**完整性方法论**：不锁定"三处/四处"这类会随博弈轮次递增的静态计数，P0 实施与验收各跑一次上述 grep（配对 `npc_runtime_bundle_with_age(` 的等价 grep，当前 7 处）逐条核对新增 `realm` 实参是否透传真实身份境界。已核实**必须透传真实身份 realm（非默认）**的站点：
    - `disciple.rs:233`（`spawn_relic_guard_npc_at`，`guard_realm = Realm::Spirit` 定义于 `:217`）——GuardianRelic 守护者
    - `world/tsy_lifecycle.rs:837`（`spawn_daoxiang_from_corpse`，`corpse.origin_realm` 已在 `:831` 喂给 `DaoZhangBehaviorBlackboard`，须同源喂进 bundle，不可只喂 blackboard 不喂 `Cultivation`）——尸体激活的道伥，worldview §七:739「道伥保留生前本能」的境界前提
    - `tsy_hostile.rs:795`（`spawn_tsy_daoxiang_at`，`daoxiang_realm = Realm::Induce` 定义于 `:778`）——TSY 道乡
    - `tsy_hostile.rs:939`（`spawn_tsy_zhinian_at`，`zhinian_realm = Realm::Condense` 定义于 `:922`）——TSY 执念
    其余约 20 处调用点（`fauna`/`Beast` 类通用生物、`combat/resolve.rs`/`brain/mod.rs` 测试 fixture、`scenario.rs`/`spawn_pillar.rs`/`spawn_rat.rs`/`spawn_whale.rs`/`heiwushi.rs`/`spawn_spider.rs`/`hybrid_beast.rs`/`dandao/boss_spawn.rs` 等）无身份 realm 信号，P0 实施时在各调用处显式写字面量 `Realm::Awaken`（每处自己写清楚，不是让 wrapper 内部默认兜底）——干净代码无兼容层，不留隐式 Awaken 吞没
  - **活体种群入口（独立于 dormant 快照，P0/P1 均须覆盖）**：`seed_initial_rogue_population_on_startup`（`rogue.rs:353`）是与 `dormant_rogue_seed_snapshot`（下条）**完全独立**的第二条种群生产线——它在 `mod.rs:199` 注册于 `Update`（生产系统，非测试专属），`RoguePopulationSeedConfig::default().target_count` 默认 `20`（`rogue.rs:62`，可用 `BONG_ROGUE_SEED_COUNT` 覆盖但生产默认非 0，必跑），逐 tick 调用 `spawn_scattered_cultivator_at`（调用点 `rogue.rs:517-527`）产出**真实 entity**、把 `Realm::Awaken` 硬编在调用处（`rogue.rs:525`）。此路径**不产出 dormant snapshot**、**不经过** `dormant_rogue_seed_snapshot`/hydrate 往返——P0 R2 pin 测试（hydrate round-trip）和 P1 的 `classify_zones_by_qi` 分布表若只接到 `dormant_rogue_seed_snapshot`，这条活体路径会永久锁死醒灵，直接推翻 P1"末法长尾分布"目标。P0/P1 均须显式把此路径纳入交付物（见下方 P0/P1 正文）
- 种群 seeder `dormant_rogue_seed_snapshot`（`npc/dormant/mod.rs:1274-1285`，:1285 恒 `Cultivation::default()`，其注册入口 `seed_initial_dormant_population_on_startup` 在 `dormant/mod.rs:1111`）；zone 灵气分档 `classify_zones_by_qi`（**真定义 `npc/spawn/rogue.rs:99`**，`dormant/mod.rs:1145-1148` 只是调用点，非定义）；确定性哈希 `deterministic_hash(char_id, salt)`（**真定义 `npc/dormant/mod.rs:1400`**；`seed_rogue_faction :1228` 是用例之一 salt=0，`GROUP_SALT` 具名常量在 `:1245`，用于 `:1252` 的分组哈希）
- **出料**：正确的 `Cultivation.realm` → 下游**零改动自动生效**：战力 `compute_combat_power`（`combat_power.rs:22-46`，realm_ordinal×20；**其内 `default_cultivation` 测试专用 qi_max 表 `combat_power.rs:61-65`（10/30/60/120/200/400）是 test-only fixture，非正典，P0 严禁复用/参照**）、离屏战争结算（`dormant/combat.rs:57`）、死亡遗物门槛（`combat.rs:245`，固元起）、AI 威胁评估（`brain/threat.rs:134-249`，realm_delta 权重 0.4）、全部招式境界门控、寿元 `LifespanComponent::for_realm`
- **共享类型**：`Realm` 枚举 / `Cultivation`（`cultivation/components.rs:15-22,386-398`）不动；既有身份境界逻辑 `leader_realm_for`（`faction.rs:1123-1130`，Qingyun→Solidify 固元 / Cangyuan→Spirit 通灵 / NorthWaste→**Awaken 醒灵，非高境**——三档不是清一色高境界）、TSY 硬编 realm（`tsy_hostile.rs:778`「道乡默认 Induce」/`:922`）复用不重造，但见下方 P0 R1 pin 测试注意事项（TSY realm 目前是局部变量喂 technique，未必已过 bundle）
- **跨仓库契约**：无 wire 变更（NPC cultivation 已在 world_state 快照内）；agent 侧天道推演读到的散修境界将首次真实
- **worldview 锚点**：§三:61 六境界正典；§三:195-203 进入境界 qi_max 表（醒灵10/引气40/凝脉150/固元540/通灵2100/化虚10700，膨胀 1074 倍）；§一 末法时代（高境界稀有 → 分布必须长尾）；§七:739 智能 NPC 散修；§937 境界感知门控（固元+ 才看得到对方大致段位，观察者侧非广播）
- **qi_physics 锚点**：分布本身不动真元；NPC qi_max 随 realm 由既有系统派生（组合 `breakthrough.rs:91 qi_max_multiplier` + `meridian_open.rs:42 MERIDIAN_CAPACITY_ON_OPEN=10.0` + `components.rs:37-45 Realm::required_meridians`，具体组合见 §8.1 #2；不新增守恒常数）；`plan-zone-qi-economy-v1` 的 NPC 让灵地板与预算需按新境界结构重估——见 §8.1 #4，跨 plan 联动不在本 plan 写公式

## 背景调研结论（2026-07-03）

两层 bug 叠加：

1. **realm 参数在 spawn 链路被丢弃**（更深层）：rogue/disciple/commoner 的 spawn 函数都收 `realm` 参数，但只用于 `select_npc_visual_profile` / `npc_meridian_system_for_realm` / `assign_npc_techniques`，最后 insert 的 `npc_runtime_bundle_with_age` 恒用 `Cultivation::default()` 覆盖（`rogue.rs:290` / `disciple.rs:159` / `commoner.rs:86`）。于是**既有境界逻辑全部沦为装饰**：派系首领（青云猎户→固元、苍原商队→通灵）、TSY 道乡引气/执念凝脉、hydrate 快照境界，实际全被吞成醒灵
2. **种群 seeder 写死默认**：genesis 初始人口全走 `Cultivation::default()`（`dormant/mod.rs:1285`），无任何分布逻辑
3. 全仓唯一真正非醒灵的 NPC = 垂死大能（化虚，`fauna/dying_elder.rs:391-394`，直接构造 `Cultivation` 字面量未经 bundle）
4. **一致性隐患**：NPC 按"意图境界"分到凝脉门槛功法，Cultivation 却是醒灵——战力/威胁/遗物判定与其功法自相矛盾

## P0 choke point 修复 ⬜

- `npc_runtime_bundle_with_age`（`lifecycle.rs:596`）加 `realm: Realm` 入参：`Cultivation` 不再恒 `::default()`，改为 `realm` + `qi_max_for_realm(realm)`（新函数，见 §8.1 #2）算出的 `qi_max`/`qi_current`；同一处 `:608 LifespanComponent::for_realm(...)` 同步吃 `realm` 而非 `Cultivation::default().realm`；四处 spawn 调用点透传各自已持有的 `realm` 参数——`rogue.rs:290`（`spawn_rogue_npc_at`）/ `disciple.rs:159` / `commoner.rs:86` / `rogue.rs:340`（`spawn_scattered_cultivator_at`，函数签名 `:298` 已持 `realm` 形参 `:306`，本条只需把该形参透传进 bundle 调用，函数签名不用改）
- **`npc_runtime_bundle`（2-arg 姐妹函数，`lifecycle.rs:592`）同步加 `realm: Realm` 入参**（转调 `_with_age(entity, archetype, realm, 0.0)`，禁止签名不改、内部继续悄悄塞 `Realm::Awaken`）——**优先方案：给 wrapper 加 realm 形参全透传**，不做"编译期强逼但默认吞没"的折中。全仓 28 个调用点（grep 方法论见接入面）逐一改传：
  - 身份站点透传已持有的局部变量：`disciple.rs:233` 传 `guard_realm`（`:217` 定义）/ `world/tsy_lifecycle.rs:837` 传 `corpse.origin_realm.unwrap_or(Realm::Awaken)`（`spawn_daoxiang_from_corpse` 入参已保证走到此行时 `origin_realm` 通常为 `Some`，见调用侧 `:619`/`:737` 的 `None` 分支已提前 despawn 不会到达此处，但函数签名允许 `Option`，兜底值仅为类型安全非业务默认）/ `tsy_hostile.rs:795` 传 `daoxiang_realm`（`:778` 定义）/ `tsy_hostile.rs:939` 传 `zhinian_realm`（`:922` 定义）
  - 其余约 20 处非身份调用点在调用处显式写字面量 `Realm::Awaken`（逐处显式，非 wrapper 内建默认）
- 新增 `qi_max_for_realm(realm: Realm) -> f64`，落 `cultivation/breakthrough.rs`（紧挨 `qi_max_multiplier:91`），组合既有原语（`qi_max_multiplier` × `MERIDIAN_CAPACITY_ON_OPEN=10.0` × `Realm::required_meridians()`，具体组合公式见 §8.1 #2）；输出必须对拍 worldview §三:195-203 六个数值（10/40/150/540/2100/10700）
- 修完自动生效面 pin 测试：
  - 派系首领 spawn 后 `Cultivation.realm == leader_realm_for(faction)`（三档全覆盖：Qingyun→Solidify、Cangyuan→Spirit、NorthWaste→Awaken）
  - **R1**（2026-07-03 第二轮博弈复核核实：四站点均实测走 **2-arg `npc_runtime_bundle`**，非 `_with_age`——上一版误记为 `_with_age`，本轮已订正）：
    - TSY 道乡 `Cultivation.realm == Realm::Induce`（`tsy_hostile.rs:795`，`daoxiang_realm` 定义于 `:778`）
    - TSY 执念 `Cultivation.realm == Realm::Condense`（`tsy_hostile.rs:939`，`zhinian_realm` 定义于 `:922`）
    - GuardianRelic 守护者 `Cultivation.realm == Realm::Spirit`（`disciple.rs:233`，`guard_realm` 定义于 `:217`，经 `spawn_relic_guard_npc_at`）
    - 尸体激活道伥 `Cultivation.realm == corpse.origin_realm`（`world/tsy_lifecycle.rs:837`，经 `spawn_daoxiang_from_corpse`；`origin_realm` 已在 `:831` 喂 `DaoZhangBehaviorBlackboard`，本测试断言同一份值也落进 `Cultivation`，覆盖至少一个非默认 `Some(Realm::Solidify)` 等取值，不用 `None` 分支——`None` 分支在 `:737-743`/`:619-629` 已提前 despawn 干尸，不会 spawn 道伥）
    四处均须显式把各自局部变量传进 2-arg `npc_runtime_bundle` 调用（不能假设"改完 `_with_age` 自动生效"，因为它们根本不走 `_with_age`），测试直接断言 `Cultivation.realm`，不是断言功法门槛
  - **R2**：hydrate 往返 `snapshot.cultivation.realm` 不丢——此测试成立的前提是 seed 阶段（P1）已把非默认 `realm` 写进快照；在 P0 阶段（seeder 仍产出 `Cultivation::default()`）该 pin 测试恒真无意义，**必须等 P1 落地后才补齐真实断言**（P0 阶段先写 hydrate round-trip 骨架 + `Realm::Solidify` 等非默认值的手工构造快照做覆盖，不依赖 seeder 产出）
  - dev 命令显式 realm 直达组件（`/realm set` 路径不受影响）
- 回归锁：`dying_elder`（`fauna/dying_elder.rs:391-394`）化虚路径直接构造 `Cultivation` 字面量、不经 `npc_runtime_bundle_with_age`，不受本阶段 bundle 签名变更影响，加专属回归测试锁 `Realm::Void` 不被误改

## P1 种群 seeder 境界分布 ⬜

- `dormant_rogue_seed_snapshot`（`dormant/mod.rs:1285`）替换 `Cultivation::default()`：按 `deterministic_hash(char_id, "realm")` 抽样，权重按 zone 灵气档（`classify_zones_by_qi` 现成 resource/background 二分）查分布表
- **活体种群入口同源接入（2026-07-03 博弈复核补充，P1 必做非 P0 附带项）**：`seed_initial_rogue_population_on_startup`（`rogue.rs:353`，`mod.rs:199` 注册于 `Update`，生产系统必跑）里调用 `spawn_scattered_cultivator_at` 时硬编的 `Realm::Awaken`（调用点 `rogue.rs:517-527`，硬编在 `:525`）必须换成与 `dormant_rogue_seed_snapshot` **同源同规则**的抽样：同一份 §8.1 #1 分布表 + 同一个 `deterministic_hash` 派生函数（seed 可用该函数已在作用域内的 `zone_spirit_qi`/`global_index`/`zone_name` 组合出 char_id 等价输入，不新增第二套抽样逻辑）。此路径产出真实 entity、不进 dormant 快照，若遗漏则该活体种群永久锁死醒灵，直接推翻本阶段目标——不能假设"改完 `dormant_rogue_seed_snapshot` 活体路径会自动继承"
- 分布表（数值 §8 #1 收口，末法长尾基调）：background zone 约 醒灵 55 / 引气 30 / 凝脉 12 / 固元 3 / 通灵 0；resource zone 整体上移一档、通灵 ≤1%；**化虚不自然刷**（正典稀有，仅垂死大能类稀有实体）
- 确定性要求专属测试：同 seed 两次 genesis 境界逐 NPC 一致；分布直方图区间 pin
- **活体种群产出实体 pin 测试（专属，不可用 dormant hydrate 往返代替）**：起 `App` 跑 `seed_initial_rogue_population_on_startup` 到 `progress.done`，直接 query 产出实体的 `Cultivation.realm` 分布，断言 ① 不再恒为 `Realm::Awaken` ② 分布落在 §8.1 #1 分布表容差区间内 ③ 同 seed 两次跑该 system 逐实体 realm 一致（确定性）
- 与 `plan-ambient-threat-v1` 的物种池合账（archetype 池选择不受本 plan 影响，只是 realm 抽样叠加在其之上）
- **cross-plan TODO（不阻塞本 plan，§8.1 #4）**：realm 分布落地后，高境界 NPC 变多 → 吸灵路径（`dormant/mod.rs:1435` 起 `apply_dormant_regen_with_multiplier`，2026-07-03 第二轮博弈复核订正——上一版误记函数名为反方向的 `release_dormant_qi_to_zone`（`:1601`，是败者死亡把残余真元回灌 zone，方向相反；真正的吸灵/regen 函数是 `apply_dormant_regen_with_multiplier`，`room = qi_max - qi_current` 见 `:1454`）的 room 需求上移，需要 `plan-zone-qi-economy-v1`（owns `QI_NPC_ABSORB_FLOOR=0.3` 与 equilibrium/inflow 数值）在本 plan 落地后用 `account:npc:*` dump 重新标定——本 plan 只改变输入（更多高境界 NPC），不改吸灵地板/流量公式。**已登记**（2026-07-03 博弈复核落实，非仅口头承诺）：`docs/plans-skeleton/reminder.md`「plan-npc-realm-distribution-v1 → plan-zone-qi-economy-v1」条目

## P2 一致性收口（单一来源） ⬜

- 功法 / 经脉 / 视觉 profile / trade inventory 全部从**最终写入的** `Cultivation.realm` 派生（消灭"意图 realm ≠ 组件 realm"双源）
- audit 测试：任意 spawn 路径出来的 NPC，`assign_npc_techniques` 结果里每条 technique 的 `required_realm ≤ cultivation.realm`；视觉档位与 realm 映射 pin

## P3 感知面 ⬜

- 视觉：`select_npc_visual_profile` 已按 realm 分档（现成，验证接的是修复后 realm）
- narration 示例（zone / perception）：「集市角落那个补锅匠收锤时指缝漏了一缕凝而不散的白气——凝脉，藏得很深」「北荒来的独行客靴底不沾尘，固元境的横行是写在步子里的」
- ~~高境界 NPC 气息粒子~~：**不做**（§8.1 #5 已拍板）。境界感知走既有 `spiritual_sense/scanner.rs:11` 通道 + worldview §937 观察者侧门控（固元+ 才能读到对方大致段位）+ narration，不加常驻广播式粒子——广播式气息违反"藏拙"基调（§439/§554 伪灵皮伪装机制的存在本身说明气息是可以刻意隐藏的，常驻粒子会让伪装机制失去意义）
- 存量迁移：一次性确定性重 roll（§8.1 #3 已拍板，见下方决议，含 marker 幂等实现要求）

## §8 开放问题（升 active / P0 决策门前收口）

> 全部 5 条已在 §8.1 收口（2026-07-03）：#1/#3 用户授权代拍，#2/#4/#5 经 Explore agent 实地核验代码现状后收口。**原表留作历史回溯，实施以 §8.1 为准**，不带任何开放问题进 P0。

1. **分布表数值**：background/resource 两档的具体百分比；通灵是否开放自然涌现；与在线人口上限（数百 dormant）相乘后的各档绝对数量预估
2. **qi_max 派生口径**：worldview §三:171 真元池容量曲线（境界×经脉复利）在 NPC 侧的简化实现用哪个既有函数——严禁本 plan 手写曲线
3. **存量存档迁移**：重 roll 全体 dormant（简单但打破连续性）vs 只对新 seed 生效（旧人口永远醒灵）vs 一次性迁移脚本
4. **跨 plan 预算联动**：高境界 NPC 吸灵更凶，`plan-zone-qi-economy-v1` 的 NPC 让灵地板 0.3 与 equilibrium 数值是否要按境界结构重估
5. **气息粒子是否做**：末法"藏拙"基调下高境界该更难辨认还是更好认——正典倾向前者，粒子可能反直觉，需拍板

## §8.1 决议（2026-07-03，用户授权代拍）

### #1 分布表数值

**决议**（末法长尾基调；P1 实施基线，实测后允许微调但长尾形状不变）：

| 境界 | background zone | resource zone |
|------|----------------|---------------|
| 醒灵 | 55% | 35% |
| 引气 | 30% | 35% |
| 凝脉 | 12% | 20% |
| 固元 | 3% | 8% |
| 通灵 | 0% | 2% |
| 化虚 | 0%（不自然刷） | 0%（不自然刷） |

按当前 genesis 种群量级（数百 dormant），预期全服固元 10-20 人、通灵 ≤5 人——遗物门槛（固元起）与派系首领（固元/通灵）在此分布下不再是全服孤例。身份逻辑（`leader_realm_for`、TSY 硬编）优先级高于抽样：有显式 realm 的路径不走分布表。

### #3 存量存档迁移

**决议**：**一次性确定性重 roll**（拒绝"只对新 seed 生效"——genesis 人口只 seed 一次，不迁移则世界永远全员醒灵；bug 产物无保留价值）：
1. 启动 hydrate 时检测迁移标记（`data/npc/realm_migration_v1.marker` 或快照文件 version 字段），无标记 → 对全体 dormant snapshot 按 §8.1 #1 分布重 roll（`deterministic_hash(char_id, "realm")`，与 P1 seeder 同源同 salt → 跨重启稳定）
2. 有显式身份 realm 者（派系首领/TSY）直接写身份值不抽样
3. 重 roll 后立即触发 P2 一致性派生（功法/经脉/视觉按新 realm 重派），消灭存量"凝脉功法+醒灵组件"矛盾体
4. 写标记防重复迁移；标记本身进 flush/hydrate 测试

**落点**：迁移逻辑挂 `npc/dormant/mod.rs` hydrate 入口（`seed_initial_dormant_population_on_startup` 同文件）；plan P1 交付物加"迁移器 + marker 幂等测试"。

### #2 qi_max 派生口径【唯一硬阻塞，Explore 核验】

**决议**：

1. **结论**：全仓当前**不存在**任何 `qi_max_for_realm` 一类单函数——`combat_power.rs:61-65` 的 `default_cultivation` 表（10/30/60/120/200/400）是**测试专用 fixture**，既非生产代码也非正典数值，本 plan **禁止复用**。P0 必须新增薄函数，**组合既有原语**而非发明新曲线，输出对拍 worldview §三:195-203 权威表。
2. **实施方案**：新增 `pub fn qi_max_for_realm(realm: Realm) -> f64`，落 `server/src/cultivation/breakthrough.rs`（紧挨 `qi_max_multiplier` 函数，`:91` 起）。组合三个既有原语：
   - `qi_max_multiplier(realm)`（`breakthrough.rs:91`，×1/2/2.5/3/3.5/5，六境界各一档乘数）
   - `MERIDIAN_CAPACITY_ON_OPEN: f64 = 10.0`（`cultivation/meridian_open.rs:42`，每条已开经脉贡献的池容量）
   - `Realm::required_meridians()`（`cultivation/components.rs:37-45`，各境界门槛经脉数 1/3/6/12/16/20）
   纯乘链（`multiplier × MERIDIAN_CAPACITY_ON_OPEN × required_meridians`）算出 10/20/50/150/525/2625，**不等于**正典 10/40/150/540/2100/10700——正典表隐含每条经脉之外还有一个**基础加法项**（醒灵 1 条经脉 × 10 容量 = 10 吻合，但引气开始 multiplier×capacity×meridians 的纯乘积系统性偏低，说明正典曲线不是单一乘法链，而是"前置境界池 + 本境界新增经脉贡献"的**累加式**）。P0 实施时须反推出与正典 6 个数值完全吻合的组合公式（例如逐境界基于`previous()`递归的 qi_max 累加，而非从零重新相乘），**验收标准 = 6 个输出值必须与 worldview §三:195 表逐一相等**，允许函数内部实现细节由 P0 实施者决定，但结果值不可协商。**兜底（2026-07-03 第二轮博弈复核收口）**：若 P0 实施者反复验算仍反推不出与正典 6 值吻合的组合公式（本节验算的纯乘链、递推加法式 `Condense=130` 等尝试均不吻合，clean 公式大概率不存在），**直接把 worldview §三:195 权威表转写成 `match realm { ... }` 查表是可接受的兜底实现**——`qi_max_for_realm` 内部允许是纯查表，不强制是推导公式。docs/CLAUDE.md §四"禁止手写曲线/自定真元物理常数"红线约束的是**发明非正典数值**（比如自己拍一套 10/30/60/120/200/400），不是禁止"把已经写在 worldview 里的权威数值抄进代码"；查表转写 worldview §三:195 六个数值与 `combat_power.rs:61` test-only fixture（数值与正典无关、非权威来源）是两回事，不违反本 plan §8.1 #2 边界。
   `qi_current` 建议初始值 = `qi_max`（满灵假设，与 `dormant/combat.rs:269` 附近既有注释"离屏快照满灵"语义一致）。
3. **边界 / 拒绝理由**：不采用 `combat_power.rs:61` 表（test-only，数值与正典无关，若被生产代码引用等于把测试 fixture 泄漏为权威数据源）；不采用 `dormant/combat.rs:277` 的另一套硬编表（同为非生产）；不在本 plan 新增任何 `*_DECAY`/`*_ATTEN`/独立衰减常数——`qi_max_for_realm` 只是静态查表式容量函数，不涉及真元流动，不触碰 `qi_physics` 守恒红线（docs/CLAUDE.md §四）。

**落点**：`server/src/cultivation/breakthrough.rs:91`（紧邻 `qi_max_multiplier`，新函数插入点）/ plan §P0（`npc_runtime_bundle_with_age` 与 `npc_runtime_bundle`（2-arg，转调前者）两处调用点均消费此函数）。

### #4 跨 plan 预算联动【不阻塞，登记 cross-plan TODO】

**决议**：

1. **结论**：本 plan 是**生产者**（改变"多少高境界 NPC 存在"这个输入），`plan-zone-qi-economy-v1` 是**消费者**（拥有让灵地板 `QI_NPC_ABSORB_FLOOR=0.3`、equilibrium、inflow 数值的唯一实现权）。本 plan **不写任何 zone-qi 公式**——触碰即撞 docs/CLAUDE.md §四"自定真元物理常数/公式"红旗。
2. **实施方案**：P1 落地后，NPC 吸灵路径（`dormant/mod.rs:1435` 起 `apply_dormant_regen_with_multiplier`——`room = qi_max - qi_current`（`:1454`）后调 `regen_from_zone` 从 zone 吸灵，消费 `MIN_ZONE_QI_TO_OPEN`/`QI_NPC_ABSORB_FLOOR` 等 zone-qi 侧常数；**不是**反方向的 `release_dormant_qi_to_zone`（`:1601`，败者死亡回灌 zone））会因为高境界 NPC 数量上升而产生更高频次/更大额的吸收请求。本 plan 登记一条 **cross-plan TODO**，**已于 2026-07-03 实际追加**到 `docs/plans-skeleton/reminder.md`（既有跨 plan 待办登记文件，非另立 plan 文件）：「realm 分布落地（本 plan P1 merge）后，用 `account:npc:*` 账本 dump 重新核算 inflow/floor 是否还匹配新的境界结构人口分布」——由 `plan-zone-qi-economy-v1` 后续 PR 消费并按约定从 reminder.md 删除该条。
3. **边界 / 拒绝理由**：不在本 plan 内调整 `QI_NPC_ABSORB_FLOOR` 或 equilibrium 数值——数值 owner 是 `plan-zone-qi-economy-v1`，跨 plan 数值协调必须由该 plan 的 PR 完成，不允许本 plan"顺手"改邻居的常数。

**落点**：`server/src/npc/dormant/mod.rs:1435`（`apply_dormant_regen_with_multiplier` 函数体内，吸灵路径现址，TODO 注释可挂在此处附近但不改动逻辑）/ plan §P1（cross-plan TODO 段落）/ 待 `plan-zone-qi-economy-v1` 消费。

### #5 气息粒子是否做【不做，Explore 核验】

**决议**：

1. **结论**：**不做**高境界常驻气息粒子。worldview §937 明确境界感知是**观察者侧**的门控能力（"固元+可看到对方大致境界段"），不是被观察者主动广播的视觉效果；§439/§554 伪灵皮/蜕壳流机制的存在本身证明气息是可以被刻意伪装、隐藏、切断的——一个常驻不可控的粒子特效会让"伪灵皮伪装气息"这条正典机制失去意义（伪装了也遮不住粒子）。
2. **实施方案**：感知线索走三条既有正典通道，全部复用不新增：`spiritual_sense/scanner.rs:11` 的主动扫描能力（观察者主动发起）、worldview §937 的相对读数门控（固元+ 观察者才能读到）、narration 文案（P3 已给 2 条示例）。P3 阶段删除"高境界 NPC 气息粒子（可选）"这一交付物，不进入任何实施 PR。
3. **边界 / 拒绝理由**：不新增 VFX 基类调用（`BongRibbonParticle` 等）、不新增 `bong:vfx_event`；若未来确有"可选氛围粒子"需求，需另立 plan 明确其与"藏拙"基调、伪装机制的取舍关系，本 plan 范围内不做。

**落点**：`server/src/cultivation/spiritual_sense/scanner.rs:11`（既有通道，无需改动）/ plan §P3（气息粒子项已删除，标注"不做"理由）。
