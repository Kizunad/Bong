# plan-bughunt-stamina-chain-v1（骨架）

> **骨架（2026-07-26）**。一句话主题：体力（Stamina）全链路审计（组件 → stamina_tick → 12+ 扣费点 → emit → client HUD）确认的 **3 个高危**（ShieldBlocking 态被战斗结算覆写 / recover_per_sec 双写打架 / change-detection 被击穿成 5Hz 伪心跳且重连补水恰靠它兜底）+ 中危与附带项，**每项发现均有「阶段 → 处置 → 验收」闭环映射**（见 §处置总表）。

> 立项动机：与同日血条审计同系列的系统性链路审计。3 路只读调查（server 核心 / emit 契约 / client HUD）+ 主循环亲读决定性代码行复核全部高危。已对 bughunt r1-r10 与 bughunt-20260726 ×20 批次（#1280）去重：唯一邻近项 `plan-bughunt-r10-findings-v1` P0 是**破盾（耐久归零）路径**的状态清理缺失，本 plan P0 是**正常格挡受击/出手路径**的 StaminaState 覆写，点位与修法均不同，不重复；`plan-satiety-hydration-v1` 计划向体力恢复注入乘数，落地前**依赖本 plan P1 先收敛单写者**（乘数作为 P1 聚合函数的一个输入位）。

## 阶段总览（按根因分组，逐项独立可修）

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 🔴 ShieldBlocking 态被 resolve/攻击分支无条件覆写（按 §P0 状态仲裁表修复） | fix_pr | ⬜ |
| P1 | 🔴 recover_per_sec 双写打架 → 收敛为唯一聚合写入者（含 NPC baseline 拍板） | decision + fix_pr | ⬜ |
| P2 | 🔴 5Hz 伪心跳 + join 首帧权威契约（三通道枚举 + 顺序无关化，**捆绑修**） | fix_pr | ⬜ |
| P3 | 防御方 12.0 扣费拍板 + 三态死代码拍板 + 剑技余额预检统一（扣费守恒） | decision + fix_pr | ⬜ |
| P4 | 低体力阈值语义逐项拍板 + movement 死负载字段去留拍板 → 按决议落地 | decision + fix_pr | ⬜ |
| P5 | 契约债：combat_hud_state schema 四件套 + 死字段/药效写入/持久化逐项处置 | decision + fix_pr | ⬜ |

## 处置总表（发现项 → 阶段 → 处置 → 验收锚点）

主项（3 高危 + 3 中危）分别是 P0-P5 的主题本身；下表收录全部**附带发现**，保证没有「落盘后从验收里消失」的项：

| 发现 | 阶段 | 处置 | 验收锚点 |
|---|---|---|---|
| 剑基础 `spend_stamina` 无余额预检（1 点体力打出 8 耗 Cleave，`sword_basics.rs:810-822`） | P3 | **本 plan 修复**（统一到 `skill_register.rs:846-853` 预检模式） | 验收 P3-c 守恒四测试 |
| NPC 回速被 `With<Client>` 排除在境界同步外恒 5.0（`npc/lifecycle.rs:647`） | P1 | **拍板**（NPC baseline 进聚合函数还是保持常数） | 验收 P1-d 决策测试 |
| 组件缺失 fallback 方向相反（速度 `unwrap_or(100.0)` `movement/mod.rs:237` vs payload `unwrap_or((0.0,1.0))` `:590-592`） | P4 | **本 plan 修复**（统一缺失契约，方向随 P4 字段拍板确定） | 验收 P4-c |
| HuGuSan 直写 `stamina.max`（`client_request_handler.rs:16284-16290`）4 tick 内被 `status.rs:383-385` 不同公式覆盖 | P5 | **本 plan 修复**（删除直写，max 归一走 status 聚合链） | 验收 P5-c |
| `StaminaRecovBoost` magnitude `<1.0/>=1.0` 双语义分流（`status.rs:367-397`） | P5 | **本 plan 修复**（拆两个 StatusEffectKind 或加显式字段，弃数值分流） | 验收 P5-d |
| 体力不持久化 = 重连免费回满（`combat/mod.rs:116-133` 一律 default） | P5 | **拍板**（持久化 or 有意重置，随血条审计持久化方案联动决议） | 验收 P5-e 决策记录 |
| DerivedAttrsHandler 把 EMPTY 复活成 active+全满（`DerivedAttrsHandler.java:48-53` + `CombatHudState.java:13,39-52`） | P2 | **本 plan 修复**（create 继承 active，顺序无关化） | 验收 P2-d 乱序 e2e |
| `last_drain_tick` 6 写 0 读、`ShieldSpec.stamina_drain_per_s` 配置死字段、`stamina_cost_mult` 零消费 | P5 | **逐项拍板**（接线 or 删除，三项各自独立 commit） | 验收 P5-f |
| dash `spend_stamina` clamp 只保下界（`movement/mod.rs:720`，与其余 6 处双向 clamp 不一致） | P3 | **本 plan 修复**（顺手统一，随 P3-c 同 PR） | 验收 P3-c |
| 明确排除：`status_snapshot` 裸 JSON 遗民改造、`ThroughputPeakHudPlanner` 层归类、幻觉 overlay 独立偏移 | — | **不在本 plan 范围**（低危 UI/管道债，留待独立 skeleton；此处仅记录不承诺） | — |

## P0 — 🔴 ShieldBlocking 态一被打/一出手就跌出（高危，已亲验）

- **覆写点 ①（受击）**：`server/src/combat/resolve.rs:1062-1069` 对被击方无条件 `stamina.state = Combat/Exhausted`——"被打"正是举盾核心场景，每次格挡命中都把状态打出 ShieldBlocking。与 `combat/lifecycle.rs:160-166` 目标分支注释明写的「举盾态与精疲状态不被战斗事件覆盖」保护**直接矛盾**（该保护只防 `sync_combat_state_from_events`，防不到 resolve）。
- **覆写点 ②（出手）**：`combat/lifecycle.rs:146-156` 攻击者分支无条件覆写（目标分支 `:158-167` 有保护，攻击者分支没有）。
- **跌出后无人修复**：全仓 `state = StaminaState::ShieldBlocking` 唯一生产赋值点是 `combat/shield_block.rs:325`（新举盾路径）；幂等重举盾分支（`:293-310`）只刷新 status 后 `continue`，不重设 state。
- **连锁后果**：drain 从熟练度盾耗 2~3/s（`ShieldDrainOverride`，`shield_block.rs:112`）跳成 `COMBAT_DRAIN_PER_SEC` 5/s（`lifecycle.rs:289`）；低体力叙事静音（`shield_block.rs:525`）；放盾不复位（`shield_block.rs:370-374` guard 不成立）→ 状态卡 Combat 直到战斗窗口过期。期间 `ShieldBlock` 组件仍在、减伤照常——不是自洽的「受击打断」设计，是保护漏装。

### 扣费两类契约（全 plan 权威定义，P0 仲裁表与 P3-c 守恒测试共用）

体力流出只有两类，任何 spend 路径必须归入其一，**两类的余额不足语义不同且互不冲突**：

- **`try_spend`（主动消费，可拒绝）**：普攻发起 gate、剑基础四招、剑道五招、气针、暗器、dash、举盾开始。契约：产生任何招式事件/效果**之前**预检余额，`current < cost` → 拒绝且零扣费、零事件、零效果；成功路径原子扣**恰好一次**。
- **`force_drain`（被动/持续，不可拒绝）**：受击扣费（`resolve.rs:1062`）、`stamina_tick` 各状态 drain。契约：无预检、clamp 到 0（伤害/持续效果照常结算），扣至 `<= 0` 走 Exhausted 转换（见仲裁表）。

### P0 状态仲裁表（修复的唯一权威语义，fix PR 不得偏离）

保护 guard 的准确形状：**仅当扣费后 `current > 0` 时保护 ShieldBlocking/Exhausted 不被覆写成 Combat；扣费后 `current <= 0` 时照常写入 Exhausted**（即耗尽转换永不被保护阻断）。ShieldBlock/ShieldDrainOverride 组件的移除**唯一责任方**保持既有 `force_lower_shield_on_stamina_exhausted`（`shield_block.rs:455-495`，Physics set 内 `.after(stamina_tick)`，同 tick 闭合），扣费点不自行拆盾。

| 场景（扣费点：resolve 受击 / lifecycle 出手） | 扣费后 current | state 终值 | 转换执行方 |
|---|---|---|---|
| Idle/Walking/Jogging/Combat 出手 | > 0 | Combat | lifecycle 攻击者分支（现状保留） |
| Idle/Walking/Jogging/Combat 受击 | > 0 | Combat | resolve 被击分支（现状保留） |
| ShieldBlocking 出手/受击 | > 0 | **ShieldBlocking（保持）** | 覆写点补保护（本 plan 修复） |
| ShieldBlocking 出手/受击 | <= 0 | Exhausted | 扣费点就地写入 → force_lower 同 tick 拆盾 |
| Exhausted 出手（gate `current >= 3.0` 允许时）/受击 | 任意 | **Exhausted（保持，30% 退出规则唯一出口）** | 覆写点补保护（本 plan 修复） |

- 修法：resolve 被击分支与 lifecycle 攻击者分支按上表补齐保护（与 `lifecycle.rs:161-166` 同形），不引入新的状态写入方。
- 测试矩阵（表驱动，走真实 resolve/lifecycle 调度而非抽出的 helper）：攻击者/受击者 × {Idle, Combat, ShieldBlocking, Exhausted} × 扣费后 {>0, ==0, 余额<成本} 全组合；外加：格挡中连续 N 次受击 state 恒 ShieldBlocking 且 drain 走 override、放盾回 Idle、Exhausted 强制放盾回归（对齐 r10-P0 不重叠：本表不含破盾耐久路径）、**普通非格挡攻防双方仍进 Combat 的反误伤回归**。

## P1 — 🔴 recover_per_sec 双写打架 → 唯一聚合写入者（高危，已亲验）

- 写点 A：`movement/mod.rs:179-186` `sync_stamina_regen_from_realm`（每帧、`With<Client>`）按境界写 2.0~6.0（`:808-816`，醒灵/引气 2.0 → 化虚 6.0）。
- 写点 B：`combat/status.rs:355-361` `combat_pill_stamina_status_tick`（每 4 tick、无 filter）无丹药 buff 时强制写回 5.0（`status.rs:149`）。
- 两写点各有 epsilon 守卫但写不同值 → 每帧互相改写；写点 A 游离于 `CombatSystemSet` 之外（`movement/mod.rs:153`），与 B 和 `stamina_tick` 均无序 → 读值取决于调度器仲裁。
- **修法（唯一方案，无备选）**：`recover_per_sec` 收敛为**唯一运行期写入者**——新聚合系统（工作名 `aggregate_stamina_recovery`）按 `stamina_regen_rate(realm) × pill_recov_mult × crash_mult ×（预留 satiety 乘数位）` 计算最终值并带 epsilon 守卫写入；`sync_stamina_regen_from_realm` **删除**；status 系统只维护自己的 modifier 数据（buff 增删），**不再直接写 recover_per_sec**。~~仅把写点 A 移进 CombatSystemSet 排序~~——**明确否决，不作为可选修法**。
- **调度契约（运行接线交付物）**：聚合系统注册进 `CombatSystemSet::Physics` 且 `.before(lifecycle::stamina_tick)`（对齐既有 `combat/mod.rs:340-342` 约束模式）；status modifier 增删系统 `.before(聚合系统)`——modifier 变更、聚合、消费三步在同 tick 内定序完成。
- **「唯一写入点」的 grep 验收口径**：指**运行期**写入点唯一（聚合系统一处）；初始化/重置类赋值不算运行期写入者，豁免清单显式枚举：`Stamina::default()` 构造、join bundle 插入（`combat/mod.rs:116-133`）、revive/new_character/`/reset`（`lifecycle.rs:1505-1508`、`:1819-1821`、`cmd/dev/reset.rs:202-204`）。豁免清单外新增写入点 = 验收失败。
- **P1-d 拍板项（decision）**：NPC 的 baseline——进同一聚合函数按 NPC realm 分级，还是保持常数 5.0（写明设计依据）。
- 测试（**走真实 App schedule，不测抽出的纯函数**）：6 境界最终值 pin；buff 加入当 tick 生效、buff 撤销当 tick 回境界 baseline（不是回 5.0 常数）、crash 切换当 tick 的 recover_per_sec 与实际恢复量断言；多 buff 乘数叠加；稳态（值不变时）不重复标 Changed；NPC baseline 按拍板结论 pin。

## P2 — 🔴 5Hz 伪心跳 + join 首帧权威契约（高危，已亲验，必须捆绑修）

- 击穿点：`combat/lifecycle.rs:282-283` `stamina_tick` 每次对每实体无条件可变解引用 `max`/`recover_per_sec` → 每 4 tick（200ms，`components.rs:20`）把所有 `Stamina` 标 Changed，下游 `Changed<Stamina>` 门禁（`movement/mod.rs:522-530`、`network/combat_hud_state_emit.rs:31-39`）退化为 5Hz 全量推送。
- **捆绑关系**：体力下发无显式 join/重连 resync（对比 `network/mod.rs:978-988` 的 inventory join hydration），客户端断线清空（`CombatHudBootstrap.java:97-125`）后全靠 200ms 伪心跳补首帧。单独加守卫会让「重连后体力 HUD 长期空白」显形——同 PR 必须补显式 join 首帧。

### P2 通道枚举（全部三条，producer → payload → consumer）

| 通道 | server producer | payload/schema | client consumer | 携带的体力信息 |
|---|---|---|---|---|
| `movement_state` | `movement/mod.rs:533-566`（filter `:522-530`） | proto `envelope.proto:3166-3178`（`stamina_current/max/low_stamina/stamina_cost_active`，绝对值） | `MovementStateHandler.java` → `MovementStateStore` | 数值（当前零渲染消费，去留由 P4 拍板） |
| `combat_hud_state` | `combat_hud_state_emit.rs:41-89`（filter `:31-39`） | proto `envelope.proto:760-765`（`stamina_percent` 0..1） | `CombatHudStateHandler.java` → `CombatHudStateStore` → `StaminaBarHudPlanner`/`MiniBodyHudPlanner` | **唯一渲染数值源** |
| `status_snapshot` | `status_snapshot_emit.rs:15-67`（`Changed<StatusEffects>`，裸 JSON） | 无 proto/schema（遗民，改造不在本 plan） | `StatusEffectStore` → `ExhaustedGreyOverlay` 等 | 仅状态名（Exhausted/StaminaCrash 等），无数值 |

### P2 join 首帧权威契约

- **触发源（显式，不隐含在 `Added<T>` 上）**：新增单一 join resync 系统，消费与 `network/mod.rs:978-988` 既有 join hydration（inventory/dropped_loot 的 emit_join_* 系列）**同一个 join 检测源**——该源在 valence 连接模型下对首连与重连一视同仁（每次连接都是新 client 实体），fix PR 须在 plan promotion 时把具体事件/组件 symbol 钉进本节。该系统**一次性显式推送全部三条通道**的权威首帧，均不经 `Changed` filter。
- **三通道首帧各自的断言**（不只 combat_hud_state）：① `combat_hud_state` 全量帧（`hp/qi/stamina_percent + derived`，权威数值源）；② `movement_state` 全量帧（既有 `Added<MovementState>`（`movement/mod.rs:170-177`）已覆盖，e2e 仍须显式断言收到）；③ `status_snapshot` 当前 status 全集（并入 join resync 显式触发，不依赖 `Changed<StatusEffects>` 碰巧命中）。
- **顺序无关化（而非锁顺序）**：client 侧 `CombatHudState.create()` 不再硬编码 `active=true`（`CombatHudState.java:39-52`），改为**继承当前 snapshot 的 active**——`derived_attrs_sync` 先到/后到都不能把清空态复活成 active+全满。
- **幂等**：client store 为无条件覆盖语义，重复快照天然幂等；join resync 重复触发不产生额外可观察状态，测试锁死。
- **e2e 场景钉死**：仅客户端断线重连（server 侧玩家 ECS 数据不发生任何体力变化）→ 分别断言三条通道各收到恰好一次权威首帧、HUD 渲染真值非 EMPTY 常量；该场景在伪心跳守卫（`lifecycle.rs:282-283` epsilon 化）**开启**状态下运行，证明首帧不再寄生伪心跳。
- 伪心跳修复本体：`lifecycle.rs:282-283` 加 epsilon 守卫（对齐同文件 `status.rs:355-361` 风格），`Changed<Stamina>` 回归真变化驱动。

## P3 — 防御扣费拍板 + 三态拍板 + 扣费守恒（中）

- **P3-a 拍板**：被击方无条件扣 `DEBUG_ATTACK_STAMINA_COST(12.0) * decay`（`resolve.rs:154` 定义、`:1062` 主路径消费、`:4894-4897` 测试 pin），近身满额为攻方 `ATTACK_STAMINA_COST` 3.0 的 4 倍且带 `DEBUG_` 前缀。拍板：是设计（受击掉体力）→ 常量改名 + 数值依据入 plan；是调试遗留 → 下调/移除并更新 pin 测试（失败信息写明设计依据）。
- **P3-b 拍板**：`Walking/Jogging/Sprinting` 三态生产零赋值（仅测试 `world/furniture.rs:608,639`）→ `SPRINT/JOG_DRAIN_PER_SEC`（`lifecycle.rs:74-75`）不可达、`Stamina::normalized()` Sprinting 分支（`components.rs:159-161`）死路、`world/furniture.rs:242` 灵韵「须 Idle」门形同虚设。两个分支的前置条件不对称：
  - **接线分支的硬前置**：接真实移动状态机会新增 StaminaState 写入方，**必须先把三态并入 P0 仲裁表扩展版**（至少覆盖：移动中格挡、移动中进战斗、Exhausted 期间有移动输入、停止移动回落——每格写明终值与执行方）+ 真实调度转换 pin，才允许动代码；不建仲裁不许选此分支。
  - **删除分支**：enum 三变体 + drain 常量 + `normalized()` 分支 + 相关测试 + furniture `state != Idle` 条件全链删除（furniture 门改按真实移动速度判定），全链清单进验收。
- **P3-c 修复（扣费守恒，非拍板）**：按「扣费两类契约」（见 P0 前置节）逐路径归类并修复：剑基础 `spend_stamina`（`sword_basics.rs:810-822`）属 `try_spend` 却无预检 → 统一到 `skill_register.rs:846-853` 模式（余额不足 → 拒绝进 InRecovery，零效果/事件/扣费）；dash clamp（`movement/mod.rs:720`）顺手统一为双向。**守恒测试按两类分别断言**：`try_spend` 类——余额恰好等于成本成功且扣至 0、低于成本拒绝且无伤害/事件/扣费、成功恰好扣一次；`force_drain` 类——不足时 clamp 至 0 且伤害/持续效果照常、归零触发 Exhausted 转换（对齐 P0 仲裁表）；两类共同——`current` 永不为负。

## P4 — 阈值语义拍板 + movement 死负载字段拍板（中，decision 先行）

- **P4-a 决策交付物（先于任何代码）**：四个「低体力」相关值逐项命名业务语义并拍板归属，**不预设合一**：

| 现值 | 位置 | 现语义 | 待拍板 |
|---|---|---|---|
| 0.30（比例） | `movement/mod.rs:48` wire `low_stamina` | 协议标志（当前零消费者） | 随 P4-b 字段去留连动 |
| 0.25（比例） | `StaminaBarHudPlanner.java:39` | 横条视觉告警（变红） | 独立保留 or 与 0.15 分级统一命名 |
| 0.15（比例） | `MiniBodyHudPlanner.java:58` | 竖条临界闪烁 | 同上（允许有意分级，需具名常量 + 各自 pin） |
| 10.0（绝对值） | `movement/mod.rs:47,836-840` | 速度惩罚（玩法规则；乘子误名 `EXHAUSTED_SPEED_MULTIPLIER`） | 保留绝对值 or 改比例——**注意改比例会随 stamina_max 改变减速边界，须写明依据**；乘子改名必做 |
- **P4-b 决策交付物**：movement 通道 `stamina_current/max/low_stamina/stamina_cost_active` 四字段（server `movement/mod.rs:589-607` 计算编码，client `MovementStateHandler.java:50-52` 入库后零消费）**接消费者 or 全链删除**二选一拍板后才进 fix：删 → proto/schema/sample/client 全链连动 + 反向 sample 锁定；留 → 指定唯一消费者与渲染行为 + server→client 契约测试。两个方向不得同时留在验收里。
- **P4-c 修复**：组件缺失 fallback 统一（`movement/mod.rs:237` vs `:590-592` 方向相反），统一后的方向按 P4-b 决议确定，补缺失组件契约测试。

## P5 — 契约债与死字段逐项处置（中低）

- **P5-a**：combat_hud_state schema 四件套——TypeBox 定义 + generated JSON schema + 正反 sample（含 `stamina_percent` 0 / 1 / 越界负样本）+ Rust roundtrip（对齐 movement_state 的 `server_data.rs:5963` 模式）。它是唯一渲染数值源却零 schema 覆盖，且不在 `plan-bughunt-server-data-s2c-schema-union-drift-v1` §6 清单内。
- **P5-b**：死字段三项逐项拍板（接线 or 删除，各自独立 commit）：`last_drain_tick`（6 写 0 读：`lifecycle.rs:150`、`movement/mod.rs:721`、`sword_basics.rs:821`、`carrier.rs:815`、`resolve.rs:1064`、`skill_register.rs:938`）；`ShieldSpec.stamina_drain_per_s`（`inventory/mod.rs:221-231` 注释自认 P4 承诺未兑现——接进 `shield_block_profile` or 删配置字段）；`technique_proficiency.stamina_cost_mult`（`technique_proficiency.rs:102`）。
- **P5-c 修复**：HuGuSan 删除对 `stamina.max` 的直写（`client_request_handler.rs:16284-16290`），max 计算归一到 status 聚合链单一口径（producer：服药 push StatusEffect → 聚合写入者按 status 计算 max）。测试**同时证明不回跳与药效未丢**：服药后 max 达聚合口径预期值、跨多个 status tick 保持、与 `StaminaCrash` 叠加口径唯一、效果过期回 baseline 100——单锁「不回跳」会让「把药效整个删掉」也通过，不允许。
- **P5-d 修复**：`StaminaRecovBoost` magnitude 双语义拆解（`status.rs:367-397`）。**方案收窄为唯一方向：不改 wire 状态名**——`status_snapshot` 是裸 JSON、client 侧 `ExhaustedGreyOverlay` 等硬依赖 id 字符串（`status_snapshot_emit.rs:83` 兜底生成），拆新 StatusEffectKind = 跨端 wire 迁移，风险/成本不成比例，**否决**；改为 `ActiveStatusEffect` 加显式语义字段（如 `mode: MaxBoost | RecovMult`）在 server 端分流，wire 名与 client 消费零变更。`alchemy/pill.rs:685-707` 配方连动；正反 pin 测试锁两种效果不再靠数值大小分流。
- **P5-e 拍板**：体力持久化 or 有意重连重置（`combat/mod.rs:116-133`），与血条审计持久化方案联动决议并记录。**两个分支都不豁免测试**：若持久化 → 往返测试 + 不出现未声明的体力增生；若有意重置 → pin 测试把重置钉成显式契约（重连后 `current == max`、`state == Idle`、无残留 stamina modifier），防未来引入持久化时行为静默漂移。
- **P5-f**：验收 = P5-b 三项每项都有「接线 PR」或「删除 commit」着落，不留纯写不读。

## 两轮反方裁决（高危三项）

- **P0 反方 ①**：「resolve 的覆写会被 lifecycle 目标分支的保护恢复」。裁决：证伪——该保护（`lifecycle.rs:161-166`）只是不覆写，从不回写 ShieldBlocking；全仓唯一赋值点 `shield_block.rs:325` 在新举盾路径，幂等分支 `:293-310` 不重设。
- **P0 反方 ②**：「受击跌出举盾态是有意的打断设计」。裁决：证伪——跌出后 `ShieldBlock` 组件仍在、减伤照常、只有 drain/叙事/复位坏掉；若是打断设计不会只打断状态机的一半。
- **P0 反方 ③（review 引擎首轮提出，已采纳进仲裁表）**：「宽泛 guard 会阻断耗尽转换、误伤普通 Combat 转换」。裁决：成立——故仲裁表明确 guard 只在 `current > 0` 时生效、耗尽写入永不被阻断，测试矩阵含普通攻防反误伤回归。
- **P1 反方**：「epsilon 守卫 + `.before(stamina_tick)` 已保证确定性」。裁决：证伪——守卫防重写不防互搏；写点 A 无 set 约束，读值跨构建不定。排序类修法已被明确否决（见 P1）。
- **P2 反方**：「5Hz 全量是刻意的心跳设计」。裁决：存疑但按 bug 处理——若是设计不会挂在 `Changed<Stamina>` filter 后面，且同文件写点 B 的 epsilon 风格自证「不动不标脏」是本仓惯例；因重连补水寄生其上，修复捆绑 join resync 成一个阶段。

## 验收口径

- **P0**：状态仲裁表全场景表驱动测试绿（攻击者/受击者 × 4 态 × 3 余额边界，走真实调度）；格挡中连续受击 state 恒 ShieldBlocking 且 drain 恒走 override（2~3/s）；扣至 0 同 tick 进 Exhausted 并由 force_lower 拆盾；普通非格挡攻防双方仍进 Combat（反误伤回归）；放盾回 Idle；低体力叙事在格挡受击中正常触发。
- **P1**：`recover_per_sec` 运行期写入点全仓唯一（grep 可验，初始化/重置豁免清单外零新增写入点）；聚合系统按调度契约注册且顺序断言在测试内；6 境界 pin；真实 schedule 下 buff 加入/撤销/crash 切换当 tick 的值与恢复量正确、撤销回境界 baseline；稳态不标 Changed；NPC baseline 拍板结论 + 对应测试。
- **P2**：满体力静止玩家的 `Stamina` 不再每 4 tick 标 Changed；「仅客户端断线重连、server 数据不变」e2e 在伪心跳守卫开启下**三条通道各收到恰好一次权威首帧**且 HUD 渲染真值；join resync 重复触发幂等；`derived_attrs` 先到/后到两种排列均不出现 active+全满假帧。
- **P3**：P3-a 拍板结论落档（常量改名或删除 + 测试随决议更新且失败信息写明依据）；P3-b 按分支落地——接线分支须先交付 P0 仲裁表扩展版 + 交叉转换 pin（移动中格挡/进战斗/Exhausted 移动输入/停止回落），删除分支须全链清单核销；furniture 门语义与注释一致；**P3-c 守恒测试按 try_spend / force_drain 两类分别绿**（try_spend：恰好等于成本成功、不足拒绝零副作用、只扣一次；force_drain：clamp 至 0 + 效果照常 + Exhausted 转换；共同：永不为负）。
- **P4**：P4-a 阈值语义表拍板落档，共享的用权威常量、有意分级的具名并各自边界 pin（等于/略高/略低 × 不同 stamina_max）；P4-b 字段去留拍板后单方向落地（删则全链 + 反向 sample，留则唯一消费者 + 契约测试）；P4-c 缺失组件契约测试绿。
- **P5**：P5-a 四件套齐且负样本在；P5-b 三项各有着落 commit；P5-c 不回跳 + 药效未丢双向断言（预期值/跨 tick 保持/叠加口径/过期回归）；P5-d 显式字段方案 pin 且 wire 名零变更（client 零改动可验）；P5-e 两分支均有测试（持久化→往返 + 无增生；有意重置→重置契约 pin），决策记录落档。
