# plan-defense-hardening-v1（骨架）

> **骨架（草案）**。一句话主题：防御体系结构性加固——给自由叠乘的减伤链加**跨源全局 cap**、把 **17 处绕过结算管线直写 `health_current` 的旁路**收编进统一伤害入口（附 GameMode 门）、堵**攻击侧真元守恒缺口**（qi_invest 蒸发 + NpcMelee 凭空铸造）、补**防御失败反馈闭环**（格挡失败/被崩/吞截脉全零反馈 + `ParrySuccessEvent` 假反馈）、以及**护甲客户端数据同源化**（straw 材质缺注册 → 新手甲隐形、iron 12/280↔8/200 漂移、单向 CrossCheck）。
>
> 来源：2026-07-26 防御系统全链路审计，全部锚点经两轮独立 read-only Explore agent 对抗核验修正（含四处对审计初稿的证伪/订正，见各段"核验订正"标注）。

## 阶段总览

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | 跨源全局减伤 cap（剑格×盾格×护甲×丹药叠乘收口） | ⬜ |
| P1 | 统一旁路伤害入口 `ExternalDamageRequest` + GameMode 门（17 处直写收编） | ⬜ |
| P2 | 攻击侧真元守恒（qi_invest 归还 zone + NpcMelee 铸造收敛） | ⬜ |
| P3 | 防御失败反馈闭环（`DefenseKind` 失败变体 + 假反馈矫正 + 吞截脉提示，含 A/V） | ⬜ |
| P4 | 护甲客户端数据同源化（straw 注册 / iron 漂移 / CrossCheck 双向化 / 孤儿条目清理） | ⬜ |
| P5 | 数值旋钮与死参数收口 + 饱和回归 | ⬜ |

## 接入面（docs/CLAUDE.md §二）

- **进料**：`combat::resolve` 的 `AttackIntent`/`Wound`/`DerivedAttrs` 结算链（`resolve_attack_intents` @ `server/src/combat/resolve.rs:288`）；`combat::events::DefenseKind`（`server/src/combat/events.rs:90-95`）；`zhenfa`/`cultivation::tribulation`/`alchemy`/`npc` 等 17 处现存直写调用点（P1 表）；`armor::mundane::MundaneArmorMaterial`（`server/src/armor/mundane.rs:17-28`）。
- **出料**：`ExternalDamageRequest` 事件 + 消费 system 供全部旁路源调用；`QiTransfer(ReleaseToZone)` 回灌 zone（P2）；`combat_event` payload 新 `DefenseKind` 失败变体给 client HUD/VFX/SFX（P3）；`ArmorTintRegistry` 修正后的 tooltip/tint 数据（P4）。
- **共享类型 / event**：复用并**扩展**（不另造）`DefenseKind`；复用 `death_hooks::release_qi_amount_to_zone`（`server/src/cultivation/death_hooks.rs:278-338`）；复用 `is_damageable` GameMode 守卫（`resolve.rs:281-286`）；复用 `ARMOR_MITIGATION_CAP`（`server/src/combat/armor.rs:25`）并在同文件新增 `GLOBAL_MITIGATION_CAP`。
- **跨仓库契约**：server `DefenseKind::{ShieldGuardBroken, ShieldBlockOffAngle}`（暂名）→ `proto/bong/envelope.proto` `combat_event` → `agent/packages/schema/src/server-data.ts` 镜像 + samples → client `combat_event` handler；P4 为 client↔server 数值对拍（`ArmorTintRegistry.java` ↔ `mundane.rs`），无 wire 改动。
- **worldview 锚点**：worldview.md §五「防御三流」L428-L432——「防御的本质是**如何处理已经打到肉体上的物理冲击与真元污染**」；截脉=极限弹反有代价、盾=凡人层最低门槛。全局 cap 与失败反馈都在强化"防御有代价、有失败态"这一正典基调。守恒部分对应 §二/§十 守恒律。
- **qi_physics 锚点**：P2 只调用既有 `qi_physics::ledger::QiTransfer`、`death_hooks::release_qi_amount_to_zone`、`qi_physics::ledger::assert_conservation`；**不新增任何物理常数**（`GLOBAL_MITIGATION_CAP` 是战斗减伤域常数不是真元物理，落 `combat/armor.rs` 与 `ARMOR_MITIGATION_CAP` 同址）。

## 去重边界（显式声明，不在本 plan 范围）

| 主题 | 归属 | 本 plan 的关系 |
|------|------|----------------|
| 破盾后 ShieldBlock/ShieldBlocking 状态泄漏（`resolve.rs:1262` `unwrap_or("wooden_shield")` 兜底继续 50% 减伤 + 扣体力至 Exhausted + 反受硬直） | `docs/plans-skeleton/plan-bughunt-r10-findings-v1.md` **P0 critical**（已立案未实施） | 不重复修；P3 的"被崩盾反馈"消费其修复后的干净状态机 |
| 广播体操 defense_profile 每 tick 累加至 85% | `docs/plans-skeleton/plan-bughunt-guangbo-defense-accum-v1.md`（同批立案） | P0 全局 cap 是其纵深防御层，不替代该修复 |
| NPC 穿甲减伤恒 0（`armor_sync.rs:84-87` 要求 `PlayerInventory`，NPC 不挂；`npc/equipment.rs:66` `armor_profile_id` 写而不读） | `docs/plans-skeleton/plan-npc-combat-gear-v2.md`（§8.1 已拍板 B 路线 = NPC 挂真 `PlayerInventory`，P2 去"实体=玩家"假设） | 不另起炉灶；P1 收编 NPC 侧旁路时不触碰其 B 路线 |
| 护甲 OBJ 渲染实装（`ArmorFeatureRenderer.java:41` `OBJ_RENDER_READY=false` 全程 early-return） | `docs/plans-skeleton/plan-module-wiring-gaps-v2.md` T13（视觉资产，真机 3 轮打磨） | P4 只修数据表，不碰渲染开关 |
| 濒死自救/他救（`[====]` 双条 HUD、挣扎/渡真元 intent） | `docs/plans-skeleton/plan-neardeath-ux-v1.md`（P0-P5 已划） | 防御终局体验归它；本 plan 不涉濒死态 |
| `combat_event` payload 上下文扩展（hit-stop / parry pushback / kill slowmo） | `docs/plans-skeleton/plan-combat-event-juice-runtime-bridge-gap-v1.md` | P3 新变体只加枚举+最小字段，juice 上下文扩展归它 |
| `shield_broken`/`shield_block_hit` SFX/VFX 网络线程落地问题 | `docs/plans-skeleton/plan-bughunt-shield-feedback-network-thread-ui-v1.md` | P3 新反馈落地时**必须**沿用其"切主线程"结论，不新增网络线程直调 |
| `ARMOR_MITIGATION_CAP` 三层 clamp 语义 | `docs/finished_plans/plan-layered-equip-v1.md`（护甲域 cap 的 owner） | P0 的全局 cap 叠在其**外层**，不改护甲域语义；plan 落地时回标该 plan「外层另有 GLOBAL_MITIGATION_CAP，见 plan-defense-hardening-v1 P0」 |
| NPC 日常生活侧真元铸造 | `docs/plan-bughunt-npc-daily-life-qi-mint-v1.md`（active） | P2 只处理**战斗结算侧** `NpcMelee` 豁免；实施前 grep 该 plan 确认边界不重叠 |
| 远端玩家护甲 wire 字段（`EquippedInventorySnapshot` 无 owner 维度，`proto/bong/envelope.proto:709-741` 是玩家私有 payload → 远端玩家护甲数据在 wire 上不存在） | **无人认领**，协议级改动 | 本 plan 不做，登记 §8 #6 建议独立 plan |

## P0 — 跨源全局减伤 cap

**现状（全部实核）**：四层减伤系数各自 clamp、自由叠乘，无任何全局收口：

- 剑格 `(block_ratio * defender_off_arm_block_multiplier).clamp(0.0, 0.95)` @ `resolve.rs:1171`，`wound.severity *= 1.0 - block_ratio` @ `:1174`
- 盾格 `clamp(0.0, 0.95)` @ `resolve.rs:1266`，`wound.severity *= 1.0 - ratio` @ `:1279`
- 护甲 `ARMOR_MITIGATION_CAP = 0.85` @ `armor.rs:25`，施加 `resolve.rs:138`
- 丹药部位抗性 `1.0 - effect.magnitude.clamp(0.0, 0.95)` @ `server/src/combat/status.rs:303`
- 理论残余 = 0.05 × 0.05 × 0.15 × 0.05 ≈ **1.875e-5**（十万分之二伤害，事实免疫）
- `.max(1.0)` 保底只在减伤**之前**（`resolve.rs:809`），减伤后 `wound.severity` 全程 `*=` 无 floor（`resolve.rs:142/1174/1279/1584`）
- `resolve.rs:1296-1303` 注释自认"同帧双激活极罕见，不施加互斥"——但 ShieldBlocking 是持续 status 不是稀有窗口，且有 pin 测试（`resolve.rs:10883-10963`）锁住"两者独立减伤"语义
- **核验订正**：截脉不在叠乘链里——它**不削 `wound.severity`**，只削污染 `emitted_contam_delta`（`resolve.rs:1123-1128`）+ 给防守方加 Concussion 自伤（`:1131-1140`，注释自证 `:1442-1443`）。审计初稿把截脉算进叠乘是错的。

**交付物**：

- `server/src/combat/armor.rs` 新增 `pub const GLOBAL_MITIGATION_CAP: f32`（数值 §8 #1 收口，建议 0.92）
- `resolve_attack_intents` 在进入减伤链前快照 `severity_before_mitigation`，护甲+丹药之后、写 HP（`resolve.rs:1675`）之前施加 `wound.severity = wound.severity.max(severity_before_mitigation * (1.0 - GLOBAL_MITIGATION_CAP))`——combined cap 本身就是保底，无需第二常数
- 更新 `resolve.rs:1296-1303` 注释 + `10883-10963` pin 测试语义（"独立减伤但受全局 cap 收口"）
- 测试：满配叠乘场景（SwordParrying + ShieldBlocking + 满甲 + BodyPartResist 丹）断言实际减伤 ≤ `GLOBAL_MITIGATION_CAP`（const 引用）；单层减伤场景断言 cap 不干扰（低于 cap 时行为不变）；BOSS `defense_power` 特权路径回归（§8 #1 拍板后）

## P1 — 统一旁路伤害入口 + GameMode 门

**现状（全部实核）**：`server/src/` 生产代码扣血 18 处，**17 处绕过 `resolve_attack_intents` 管线**直写 `health_current`，其中 **13 处完全不查 GameMode**——创造模式玩家会被结界灼烧、阵法反噬、天劫、炸炉、剑格反伤照常扣血：

| # | file:line | symbol | GameMode |
|---|-----------|--------|----------|
| 0 | `resolve.rs:1675` | 管线正典写入 | ✅ `is_damageable` @ `:281-286` |
| 1 | `resolve.rs:1190` | 剑格 15% 反伤（`reflected_damage` @ `:1178`，`commands.add` 直写攻方 Wounds） | ❌ |
| 2 | `zhenfa/mod.rs:3516` | `apply_shrine_ward_pressure`（结界灼烧） | ❌ |
| 3 | `zhenfa/mod.rs:3695` | `apply_trigger_snapshots` | ❌ |
| 4 | `zhenfa/mod.rs:3825` | `apply_beast_trap_snap` | ❌ |
| 5 | `zhenfa/mod.rs:4161` | `apply_backlash`（阵法反噬，硬编 `- 6.0`） | ❌ |
| 6 | `cultivation/tribulation.rs:1454` | `tribulation_aoe_system` | ❌ |
| 7 | `cultivation/tribulation.rs:1695` | `apply_juebi_phase_damage` | ❌ |
| 8 | `combat/woliu_v2/skills.rs:650` | `apply_turbulence_burst_target_effects`（**核验订正**：审计初稿写 `woliu ~:459`，实际 `woliu.rs` 全文件无 `health_current`，真实位置在此） | ❌ |
| 9 | `combat/dugu_v2/skills.rs:600` | `fn apply_damage`（流派私有，定义 `:584`） | ❌ |
| 10 | `combat/baomai_v3/skills.rs:456` | 血炼自伤 `hp_burned` | ❌ |
| 11 | `combat/zhenmai_v2.rs:910` | `apply_self_damage`（wrapper `apply_self_damage_to_entity` @ `:914` 有门 @ `:919-923`） | ⚠️ 仅 wrapper |
| 12 | `alchemy/mod.rs:363` | `apply_alchemy_explode_outcomes`（炸炉） | ❌ |
| 13 | `npc/movement.rs:934` | `apply_collision_wound`（`queue_collision_wound` @ `:950` 有门 @ `:960-964`，但 `:630`/`:685` 两分支直调不过门） | ⚠️ 部分 |
| 14 | `npc/skull_fiend.rs:826` | `apply_wall_self_damage` | ❌ |
| 15 | `combat/carrier.rs:1078` | `projectile_tick_system` | ✅ @ `:967` |
| 16 | `combat/lifecycle.rs:207` | `wound_bleed_tick` | ✅ @ `:185` |
| 17 | `network/client_request_handler.rs:15910` | `handle_alchemy_take_pill` 异种真元排斥 | ❌ |

背景约束：`resolve_attack_intents` 已 12842 行、**16/16 SystemParam 用满**（注释自证 `resolve.rs:311-323`，第 16 位还是 16 元 tuple bucket）——新减伤层/新伤害源在管线内已无处安放，旁路是被单体逼出来的结构性产物。

**交付物**：

- 新模块 `server/src/combat/damage_entry.rs`：`ExternalDamageRequest` 事件（字段含 target、amount/wound 描述、`DamageChannel`/source 标签、`mitigation_policy`）+ 消费 system `apply_external_damage_requests`（独立 system，天然绕开 16 参上限；形态 §8 #3 收口）
- 消费 system 内统一过 `is_damageable` GameMode 门 + 统一 emit 审计/事件流
- **第一阶段语义 = 收编不改数值**：各源现有伤害量、是否吃甲全部保持现行为（`mitigation_policy: None` 起步），只收口写入路径 + 补 GameMode 门——把"旁路吃不吃减伤链"留给 §8 #3 逐源拍板，避免一次 PR 同时改结构与平衡
- 17 处调用点逐个改为 emit `ExternalDamageRequest`（#11/#13 的既有 wrapper 门保留语义合并进入口）
- 测试：每个旁路源一条 creative-mode 免伤 case + 一条数值不变回归 case（收编前后同输入同伤害）；`damage_entry` 模块自身 happy/边界/无效 target/重复请求全覆盖

## P2 — 攻击侧真元守恒

**现状（全部实核，两处独立缺口）**：

1. **攻方 qi_invest 蒸发**：`resolve.rs:641-644` 扣 `attacker_cultivation.qi_current -= qi_invest` 后只把 `qi_invest * ATTACK_QI_THROUGHPUT_FACTOR` 记进经脉 throughput（`:645-652`），**无任何 `release_qi_amount_to_zone`/`QiTransfer`**——招式释放只扣攻方不写入环境，CLAUDE.md 守恒律红旗原文命中。反差铁证：同文件截脉消耗走了 `release_qi_amount_to_zone`（`resolve.rs:1108`，reason `"jiemai_parry"`，守恒测试 `jiemai_parry_emits_qi_transfer_for_conservation` @ `:6443`）；`body_conditioning.rs:143`（`"guangbo_ticao"`）、`tribulation.rs:1435`（`"tribulation_wave_aoe"`）同类回灌都做了——**唯独攻击这条最大流量路径没做**。
2. **NpcMelee 凭空铸造**：`fn source_uses_prepaid_qi` @ `resolve.rs:2149`，`AttackSource::NpcMelee` 在白名单 @ `:2176`（注释自认 `:2169-2175`）。白名单其余 8 个 source（BurstMeridian/FullPower/SwordCleave/SwordThrust/5×SwordPath/QiNeedle）是 cast 阶段真预扣过的，**只有 NpcMelee 是纯豁免无预扣**：反作弊门跳过（`:468-470`）+ 扣费跳过（`:641-643`）但 `hit_qi`（`:712`）与伤害（`:786-803`）按 `qi_invest` 全额结算——NPC `qi_current=0`（`npc/lifecycle.rs:622`）打出全额真元伤害且真元账不动。
3. **核验订正（NPC 防御对称性）**：审计初稿"NPC 截脉依赖 client 通道 + PlayerInventory 永不触发"**不成立**——`resolve.rs:1071-1073` 分支对称；NPC 有独立 DefenseIntent 发射点（`npc/brain/actions_combat.rs:509`）；`jiemai_prep_window` 是 `Option<&PlayerInventory>` 优雅退化（`jiemai.rs:55/65-70`）。**真因 = NPC `qi_current: 0.0` 撞两道真元门**（`actions_combat.rs:483-486` + `resolve.rs:1091-1093`）→ NPC 截脉恒失败。与 #2 是同一根因（NPC 无真元账户）的攻防两面：**攻击侧给了豁免、防御侧没给**。

**交付物**：

- `resolve.rs:641-644` 扣费后调 `death_hooks::release_qi_amount_to_zone(attacker, qi_invest, ..., "attack_qi_invest")`（复用 `:1108` 截脉先例的完整传参形态，缺 Position/Zone 时自动走 overflow 兜底，不新写查 zone 逻辑）
- NpcMelee 收敛：按 §8 #4 拍板的模式落地（推荐：NPC 攻击真元流入被追踪账户/沉降槽，口径可被 `assert_conservation` 断言；不解决 NPC 真元账户体系本身——那是 `plan-npc-realm-distribution-v1`/`plan-zone-qi-economy-v1` 的域）
- NPC 截脉门（防御侧对称）只登记 §8 #5，本 plan 不实施——依赖 NPC 真元预算结构
- 测试：镜像 `jiemai_parry_emits_qi_transfer_for_conservation` 写 `attack_qi_invest_emits_qi_transfer_for_conservation`；NpcMelee 场景守恒断言用 `qi_physics::ledger::assert_conservation`；`source_uses_prepaid_qi` 白名单逐变体 pin 测试（8 个真预扣 + NpcMelee 新语义）

## P3 — 防御失败反馈闭环（server + schema + client，A/V 内联）

**现状（全部实核）**：

- `DefenseKind`（`server/src/combat/events.rs:90-95`）只有 `JieMai / SwordParry / ShieldBlock` 三个**成功**变体——格挡失败在 wire 上不存在，三种失败（方向不对 FOV 外被击 / 被崩盾 / 破盾）屏幕表现与"没举盾"完全一样
- `ParrySuccessEvent` 在**施法瞬间**就 emit（`zhenmai_v2.rs:533`，位于施法入口 `fn resolve_parry` @ `:505`，注释自认"弹反成功瞬态闪现"）——真实格挡成功标志 `jiemai_success = true`（`resolve.rs:1150`）却**不** emit 它，假反馈与真结算完全脱节
- 举盾期间玩家自己的截脉 DefenseIntent 被静默吞（`resolve.rs:263-266` 无差别 `continue`，注释 `:260-262` 说明本意只是拦盾格自身的动画 intent）——玩家举盾按截脉：完全失效且无提示

**交付物**：

- `DefenseKind` 扩失败变体（暂名 `ShieldBlockOffAngle` / `ShieldGuardBroken`，命名实施时定）：server emit 点分别在盾格 FOV 检查失败分支与 `force_lower_shield_on_stamina_exhausted`（`shield_block.rs:471`）；proto `combat_event` → schema `server-data.ts` + samples 正反对拍 → client handler，走 `plan-bughunt-shield-feedback-network-thread-ui-v1` 的主线程落地结论
- `ParrySuccessEvent` 语义矫正（§8 #7 拍板：挪 emit 到 `resolve.rs:1150` 真实成功点 vs 拆 stance/success 两事件；推荐前者）
- 举盾吞截脉：`resolve.rs:263-266` 分支区分 intent 来源——盾格自身动画 intent 照旧吞，玩家主动截脉输入改为 emit player-scope 事件流提示（不开截脉窗口的现行为保留，只补反馈）
- **A/V 规格（内联，实施精度）**：

| 事件 | SFX（audio_recipe JSON 层） | VFX | HUD |
|------|------|------|------|
| ShieldBlockOffAngle（举盾中侧后被击穿防） | layer1 `item.shield.block` pitch 0.55 vol 0.6 delay 0；layer2 `entity.zombie.attack_wooden_door` pitch 1.35 vol 0.45 delay_ticks 1 | `BongSpriteParticle` ×6，lifetime 8 tick，burst，受击点向盾背侧 45° 锥形散射，颜色 `#C9B37E`，复用既有 spark 贴图，新 `bong:vfx_event` ID `shield_block_off_angle` | 盾槽图标 shake 4 tick；屏缘 vignette `#B33A2E` opacity 0.25，6 tick 线性 fade-out；事件流一条「盾面未对准来向」 |
| ShieldGuardBroken（体力耗尽被崩盾 + ParryRecovery 破势） | layer1 `item.shield.break` pitch 0.7 vol 0.8 delay 0；layer2 `entity.player.attack.crit` pitch 0.5 vol 0.5 delay_ticks 2；layer3 `block.wood.break` pitch 0.6 vol 0.4 delay_ticks 3 | `BongLineParticle` ×10，lifetime 12 tick，radial burst，颜色 `#8A6A3F`→`#4A3A28` 渐变，新 `bong:vfx_event` ID `shield_guard_broken` | ParryRecovery 持续期内盾槽图标灰化 + 「破势」标签（时长与 status 同步）；体力条红闪 3 次 × 4 tick |
| 举盾吞截脉提示 | layer1 `block.note_block.bass` pitch 0.5 vol 0.4 delay 0（低哑失效音） | 无（纯 HUD 反馈） | player-scope 事件流 toast「举盾中无法运转截脉」 |

  动画：两个失败态复用既有放盾/ParryRecovery 姿态（shield-block-v1 已有资产），不新增 PlayerAnimator JSON；本阶段不涉天道 narration。
- 测试：`DefenseKind` 全变体（含新增）schema 正反 sample 对拍；三失败路径 server emit 单测；client handler 每变体一条分发 case；e2e：bot 举盾背身受击 → 断言收到 `shield_block_off_angle` payload

## P4 — 护甲客户端数据同源化（client 为主）

**现状（全部实核）**：

- **straw 材质缺注册**：server 7 种凡物甲材质（`mundane.rs:17-28` `ALL: [Self; 7]`），client `ArmorTintRegistry.java:21-28` 只注册 6 种，缺 `straw`（草编 4 件：斗笠/蓑衣/草编腿套/草鞋，`server/assets/items/armor.toml:124-166`）。机械链全核验：`ArmorTintRegistry.item()` 返回 null（`:69-72`）→ `createLeatherArmorStack` 返回 EMPTY（`:111`）→ `MixinPlayerEntityArmor.java:61` return → **穿身完全不渲染**；`materialLine()/defenseLine()/repairLine()/iconPathForItemId()`（`:88-107`）全空 → **tooltip 空白**；`ArmorProfileStore.mitigationForItemId` fallback 拿 null → `isArmor()` false（`:75-77`）。根因 commit `902b796ba`（PR #250 加 straw 甲 + 铁甲重平衡）**一行 Java 都没碰**。新手第一件甲就是隐形甲。
- **iron 数值漂移**：同一 commit 把 server 铁甲 defense 8→12 / durability 200→280（`mundane.rs:68/:79`、`armor.toml:265-298`），client `ArmorTintRegistry.java:24` 停在 **8/200** → 铁甲胸甲 tooltip 显示 `防御: +3.20`，server 真实 `+4.80`，骗人 33%。
- **CrossCheck 单向**：`ArmorProfileStoreCrossCheckTest.java:36-40` 只遍历 server JSON 目录断言 client 有对应项——抓不到 client 多余条目（`ArmorProfileStore.java:46-49` 四条孤儿：`cloth_robe`/`fake_spirit_hide`/`spirit_weave_robe`/`iron_plate_chest`，server 无对应 profile），也看不见程序生成无 JSON 的 20 件 mundane 甲（`mundane.rs:249` `register_mundane_armors`）——straw 全缺照样绿灯。
- **核验订正**：审计初稿"ArmorFeatureRenderer.java:71 只渲染本地玩家"有偏差——`:41/:73` 是 OBJ 未实装的独立 gate（归 T13）；"仅本地玩家"真正的硬门在 `MixinPlayerEntityArmor.java:40`，且远端玩家护甲**在 wire 上根本无数据**（协议缺口，见 §8 #6），不是渲染 bug。

**交付物**：

- `ArmorTintRegistry.java` 补 `straw` 条目（defense 1.5 / durability 40 对齐 `mundane.rs:63/:75`；tint 建议 `#C9B26B` 枯黄草编，实施时与既有 6 tint 肉眼区分度校验）+ iron 修为 12/280
- `ArmorTintRegistryTest.java:23-24/:37` 断言同步 6→7 材质、24→28 件、"7 套凡物甲 7 个可区分 tint"
- CrossCheck 双向化：新增反向遍历（client `BY_ITEM_ID` + TintRegistry 派生表每条断言 server 侧存在 JSON profile 或 mundane 注册）；四条孤儿条目先 grep `server/assets/items/` 确认物品是否存在——不存在则删条目，存在则补 server profile（预期是删）
- straw 4 件 icon 资产核验：已存在则接线即可；缺失标 `[BLOCKED: 需 /gen-image item 生成 armor_straw_{helmet,chestplate,leggings,boots}]`；**若新增任何 client 纹理资产，必须同步 `resourcepack.rs` + committed manifest 的 sha1/size**
- 测试：straw 4 件 `createLeatherArmorStack` 非 EMPTY + tooltip 三行非空；iron tooltip 数值对拍 `mundane.rs` const；CrossCheck 双向各一条专属失败注入 case（临时多加/删条目断言撞红）

## P5 — 数值旋钮与死参数收口 + 饱和回归

**现状（全部实核）**：

- 五伤型 `damage_mul` 全 1.0（`fn wound_kind_profile` @ `resolve.rs:2344`；Cut `:2347`/Blunt `:2353`/Pierce `:2359`/Burn `:2365`/Concussion `:2371`），且 **qi 分支根本不乘**（消费点 `:791` 仅物理分支；`:796-802` qi 分支无接线）——伤型差异化对物理是空转旋钮、对真元连形式接线都没有
- dash dodge/iframe 占位：`dash_proficiency.rs:29/:36` 的 `iframe_success: bool` 唯一调用点硬编 `false`（`movement/mod.rs:427`）→ `:39` `iframe_bonus 0.005` 是永不可达死代码；全 server 无任何 dodge/iframe 实现
- 木盾削真元污染：`resolve.rs:1281` `emitted_contam_delta *= 1.0 - ratio`，且盾格分支在 `!is_physical_hit` 之外——凡木盾还直接削真元伤害本体。**核验订正**：这不是守恒/口径 bug——`plan-shield-block-v1.md:103/:115` 明确要求削 contam，worldview §五 L432 正典背书（防御处理"物理冲击**与真元污染**"）；"盾不碰真元"实指不需经脉（`shield_block.rs:153-158` declare 空 vec）+ 耗体力（`:334-336`）。真正可议的只有**数值**：凡人盾对修士级污染削 50-60% 是否过高

**交付物**：

- qi 分支接上 `wound_profile.damage_mul` 乘法（全 1.0 时行为不变，先接线后调值）
- `damage_mul` 数值激活或显式注释"预留"（§8 #9 拍板）
- `iframe_success` 参数处置（§8 #10 拍板，推荐删除——不写兼容层，将来 dodge plan 再引入）
- 木盾污染削减数值复核（§8 #8）
- 饱和回归包：P0 满配 cap 场景、P1 全部 17 源 GameMode case、P2 守恒断言、P3 `DefenseKind` 全变体 pin、P4 CrossCheck 双向——汇总跑通后本 plan 才算收口

## §8 开放问题（P0 决策门前需收口）

1. **`GLOBAL_MITIGATION_CAP` 数值与 BOSS 特权**：建议 0.92（残余 8%，仍强于单层护甲 cap 但不免疫）。`docs/finished_plans/plan-dandao-path-v1.md:630` 明写 BOSS `defense_power` 特权"可突破 ARMOR_MITIGATION_CAP"——全局 cap 是否给 BOSS 留通道（cap 只管 severity 乘法链、不管 defense_power 折减，则天然不冲突，需实施时验证）。
2. **剑格+盾格互斥 vs 全局 cap 兜底**：推荐后者——保留 `resolve.rs:1296-1303` "独立减伤"语义，靠 P0 cap 收口总量；互斥会改玩家可感知的技能组合行为，代价更大。
3. **统一入口形态与逐源减伤策略**：Event + 独立 consumer system（推荐，绕开 16 参上限）vs 直接函数；`mitigation_policy` 起步全 `None`（保持现数值），哪些源后续应吃甲/吃 cap（zhenfa 灼烧？tribulation 天劫按正典应无视凡甲？）逐源列表拍板。
4. **NpcMelee 守恒模式**：推荐被追踪沉降槽/预算账户（参照 `WorldQiBudget::apply_era_decay` 的 `era_decay_accum` 模式与 BossDrain 先例），使 `assert_conservation` 口径可闭合；备选：等 NPC 真元账户体系（`plan-npc-realm-distribution-v1`）落地后走真扣费。
5. **NPC 截脉真元门**：NPC `qi_current` 恒 0 撞 `actions_combat.rs:483-486` + `resolve.rs:1091-1093` 双门 → NPC 截脉恒失败。给 NPC 战斗内真元预算属跨 plan 数值协调（owner：`plan-zone-qi-economy-v1`），本 plan 只登记不实施。
6. **远端玩家护甲 wire 字段**：`EquippedInventorySnapshot`（`envelope.proto:709-741`）无 owner/entity_id 维度，远端玩家护甲渲染在协议层就不可能。等价于新增全玩家装备广播 S2C——建议独立 plan（与 `plan-npc-combat-gear-v2` 的 NPC 装备广播可能共形），本 plan 不做。
7. **`ParrySuccessEvent` 语义矫正路线**：挪 emit 到 `resolve.rs:1150` 真实成功点（推荐，事件名保真）vs 现事件改名 `ParryStanceEvent` + 新增真实成功事件（wire 兼容面更大，不推荐——本仓约定不写兼容层）。
8. **木盾污染削减数值**：50-60% 对修士级污染是否过高；若调值只动 `shield_block.rs:101-115` profile 表，正典语义（能削）不动。
9. **`damage_mul` 激活策略**：数值 owner 是谁（combat 域内自决 vs 等武器/流派 plan 统一定伤型曲线）；先接线保 1.0 的第一步无争议。
10. **`iframe_success` 死参数处置**：推荐直接删除（含 `:39` 死分支），dodge/iframe 作为 feature 单独立 plan 时再设计完整接口；保留死参数违反本仓"不留兼容层/死代码"约定。

> 转 active 前按 docs/CLAUDE.md §五收口成 §8.1 决议（Explore agent 并行核查 + 文件:行号双锚点）。

## §10 实施工作流

### §10.1 PR 拆分（依赖顺序，前一个 merge 后开下一个）

| PR | 范围 | 栈 |
|----|------|----|
| PR-1 | P0 全局 cap（+ pin 测试更新） | server |
| PR-2 | P2 攻击侧守恒（独立小 PR，便于守恒 review 聚焦） | server |
| PR-3 | P1 damage_entry 收编 17 旁路（最大 PR，可按 zhenfa/tribulation/combat 流派/其余 分 2-3 个子 commit 序列） | server |
| PR-4 | P3 失败反馈（server events + proto/schema + client A/V） | server+schema+client |
| PR-5 | P4 护甲数据同源化 | client（+可能的 icon 资产） |
| PR-6 | P5 旋钮收口 + 饱和回归汇总 | server 为主 |

PR-1/PR-2 无相互依赖可并行；PR-3 依赖 PR-1（收编后的源将来是否过 cap 的语义以 cap 先落地为前提）；PR-4/PR-5 相互独立；PR-6 收尾。

### §10.2 实施配置

- 每 PR 独立 subagent 实施（`subagent_type: "claude"`，实施模型按当期 Workflow 模型分配惯例；主线只接收 result 摘要），主线负责 merge 与等待
- 每 PR push 前跑内置对峙自检 workflow；review gate 按根 CLAUDE.md「PR review gate」节（`/review` 评论触发 + CodeRabbit，`ScheduleWakeup` 等待，禁止 sleep loop）
- schema 改动（PR-4）：TypeBox src 改后 `cd agent && npm run build -w @bong/schema`，samples 正反对拍与 proto 同 PR 落
- client 侧（PR-4/PR-5）若动纹理资产必须同步 `resourcepack.rs` manifest sha1
- 无建筑/layout/复杂视觉资产 TODO，不适用 3 轮 `<PROMISE>` 流程（P3 粒子为参数化复用贴图）

### §10.3 单次 consume-plan 全自动到 merge

用户提交 `/consume-plan defense-hardening-v1`（§8 收口转 active 后）即可离开：设计收口 → PR-1..PR-6 序列化实施 → 每 PR 等 review/e2e → merge → 归档 Finish Evidence 全自动，仅严重设计分歧或反复修不过才交人工。
