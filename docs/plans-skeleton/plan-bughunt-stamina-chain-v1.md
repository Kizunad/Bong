# plan-bughunt-stamina-chain-v1（骨架）

> **骨架（2026-07-26）**。一句话主题：体力（Stamina）全链路审计（组件 → stamina_tick → 12+ 扣费点 → emit → client HUD）确认的 **3 个高危**（ShieldBlocking 态被战斗结算覆写 / recover_per_sec 双写打架 / change-detection 被击穿成 5Hz 伪心跳且重连补水恰靠它兜底）+ 中危 6 项 + 死代码/契约债簇。

> 立项动机：与同日血条审计同系列的系统性链路审计。3 路只读调查（server 核心 / emit 契约 / client HUD）+ 主循环亲读决定性代码行复核全部高危。已对 bughunt r1-r10 与 bughunt-20260726 ×20 批次（#1280）去重：唯一邻近项 `plan-bughunt-r10-findings-v1` P0 是**破盾（耐久归零）路径**的状态清理缺失，本 plan P0 是**正常格挡受击/出手路径**的 StaminaState 覆写，点位与修法均不同，不重复；`plan-satiety-hydration-v1` 计划向 `sync_stamina_regen_from_realm` 注入恢复乘数，落地前**依赖本 plan P1 先收敛双写**（否则乘数写进战场必然被抹）。

## 阶段总览（按根因分组，逐项独立可修）

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 🔴 ShieldBlocking 态被 resolve/攻击分支无条件覆写（盾耗/叙事/放盾复位全乱） | fix_pr | ⬜ |
| P1 | 🔴 recover_per_sec 双写打架（境界回速 2~6/s 被 5.0 兜底互搏，读序不定） | fix_pr | ⬜ |
| P2 | 🔴 stamina_tick 无条件 DerefMut 击穿 Changed 门禁（5Hz 全量推送）+ 缺显式 join resync（**必须捆绑修**） | fix_pr | ⬜ |
| P3 | 防御方 `DEBUG_ATTACK_STAMINA_COST`(12.0) 主路径扣费拍板 + Walking/Jogging/Sprinting 三态死代码 | decision + fix_pr | ⬜ |
| P4 | movement 通道 stamina 四字段死负载 + 低体力阈值四套口径（0.30/0.25/0.15/绝对10.0）统一 | fix_pr | ⬜ |
| P5 | 契约债：combat_hud_state 零 schema 覆盖 + 死字段簇（last_drain_tick / ShieldSpec.stamina_drain_per_s / stamina_cost_mult）| fix_pr | ⬜ |

## P0 — 🔴 ShieldBlocking 态一被打/一出手就跌出（高危，已亲验）

- **覆写点 ①（受击）**：`server/src/combat/resolve.rs:1062-1069` 对被击方无条件 `stamina.state = Combat/Exhausted`——"被打"正是举盾核心场景，每次格挡命中都把状态打出 ShieldBlocking。与 `combat/lifecycle.rs:160-166` 目标分支注释明写的「举盾态与精疲状态不被战斗事件覆盖」保护**直接矛盾**（该保护只防 `sync_combat_state_from_events`，防不到 resolve）。
- **覆写点 ②（出手）**：`combat/lifecycle.rs:146-156` 攻击者分支无条件覆写（目标分支 `:158-167` 有保护，攻击者分支没有）——举盾中出手同样跌出。
- **跌出后无人修复**：全仓 `state = StaminaState::ShieldBlocking` 唯一生产赋值点是 `combat/shield_block.rs:325`（新举盾路径）；幂等重举盾分支（`:293-310`）只刷新 status 后 `continue`，**不重设 state**。
- **连锁后果**：drain 从熟练度盾耗 2~3/s（`ShieldDrainOverride`，`shield_block.rs:112`）跳成 `COMBAT_DRAIN_PER_SEC` 5/s（`lifecycle.rs:289`）；低体力叙事静音（`shield_block.rs:525` 只认 ShieldBlocking）；放盾不复位（`shield_block.rs:370-374` guard `state == ShieldBlocking` 不成立）→ 状态卡 Combat 直到战斗窗口过期由 `combat_state_tick`（`lifecycle.rs:338-350`）收尾。期间 `ShieldBlock` 组件仍在、减伤照常——不是自洽的「受击打断」设计，是保护漏装。
- 修法草案：resolve 被击方与 lifecycle 攻击者分支补齐与 `lifecycle.rs:161-166` 相同的 `Exhausted | ShieldBlocking` 保护；或收敛为单一状态仲裁函数。饱和测试：格挡中受击 N 次 state 恒 ShieldBlocking、格挡中出手、放盾回 Idle、Exhausted 强制放盾回归。

## P1 — 🔴 recover_per_sec 双写打架（高危，已亲验）

- 写点 A：`movement/mod.rs:179-186` `sync_stamina_regen_from_realm`（每帧、`With<Client>`）按境界写 2.0~6.0（`:808-816`，醒灵/引气 2.0 → 化虚 6.0）。
- 写点 B：`combat/status.rs:355-361` `combat_pill_stamina_status_tick`（每 4 tick、**无 filter**）无丹药 buff 时强制写回 `DEFAULT_STAMINA_RECOVER_FOR_STATUS` 5.0（`status.rs:149`）。
- 两写点各有 epsilon 守卫，但守卫只防同值重写——**写不同值时每帧互相改写**。写点 B 有 `.before(stamina_tick)` 约束（`combat/mod.rs:340-342`），写点 A 游离于 `CombatSystemSet` 之外（`movement/mod.rs:153`），与 B 和 `stamina_tick` 均无序 → `stamina_tick` 读到境界值还是 5.0 取决于调度器仲裁。低境界玩家可能白拿 5.0，化虚玩家可能被削回 5.0。
- 附带：NPC 被 `With<Client>` 排除在境界同步外，回速恒 5.0（`npc/lifecycle.rs:647`）——同一 resolve 路径下攻防双方两套恢复标准，是否给 NPC 分级一并拍板。
- 修法草案：收敛为单一 owner（推荐：status tick 只在「有 buff→无 buff」转换时回写 baseline，baseline 从境界函数取而非常数；或 realm 同步移进 CombatSystemSet 并显式排序）。pin 测试锁 6 境界回速。

## P2 — 🔴 5Hz 伪心跳 + 缺显式 join resync（高危，已亲验，必须捆绑修）

- 击穿点：`combat/lifecycle.rs:282-283` `stamina_tick` 每次对每个实体无条件可变解引用 `max`/`recover_per_sec`（无 if 守卫，对比同文件写点 B 的 epsilon 风格）→ 每 4 tick（200ms，`components.rs:20`）把所有 `Stamina` 标 Changed。
- 后果：`movement_state` emit（`movement/mod.rs:522-530` 含 `Changed<Stamina>`）与 `combat_hud_state` emit（`network/combat_hud_state_emit.rs:31-39`）的 change-detection 事实退化为 **5Hz 定时全量推送**，体力满格不动也推。
- **捆绑关系**：体力三条下发通道均无显式 join/重连 resync 系统（对比 `network/mod.rs:978-988` 的 inventory/dropped_loot join hydration），客户端断线清空（`CombatHudBootstrap.java:97-125`）后**全靠 200ms 伪心跳补首帧**。单独给 `:282-283` 加守卫会让「重连后体力 HUD 长期空白」显形——修 P2 必须同 PR 补显式 join 首帧快照 + 断线重连 e2e。
- 顺带（client 侧同窗口竞态）：`DerivedAttrsHandler.java:48-53` 经 `CombatHudState.create()`（硬编码 `active=true`，`CombatHudState.java:39-52`）会把清空态 `EMPTY`（hp/qi/stamina 全 1.0，`:13`）复活成「active + 全满」渲染，窗口 ≤200ms；join resync 落地时一并断言首帧顺序或让 create 继承 active。

## P3 — 防御方 12.0 扣费拍板 + 三态死代码（中）

- **拍板项**：被击方无条件扣 `DEBUG_ATTACK_STAMINA_COST(12.0) * decay`（`resolve.rs:154` 定义、`:1062` 主路径消费），近身满额为攻方 `ATTACK_STAMINA_COST` 3.0（`components.rs:13`）的 4 倍；常量带 `DEBUG_` 前缀却跑生产，且被测试 pin 死（`resolve.rs:4894-4897`）。需拍板：是设计（受击掉体力）就改名+入设计文档；是调试遗留就下调/移除。
- **三态死代码**：全仓生产代码零赋值 `Walking/Jogging/Sprinting`（仅测试 `world/furniture.rs:608,639` 等）→ `SPRINT_DRAIN_PER_SEC` 10.0 / `JOG_DRAIN_PER_SEC` 2.0（`lifecycle.rs:74-75`）不可达；`Stamina::normalized()` 的 Sprinting 归一分支（`components.rs:159-161`）死路。连带 `world/furniture.rs:242` 床/蒲团灵韵的「须 Idle」门形同虚设——全速跑动照样吃 buff。拍板：接移动状态机（冲刺/疾跑真实进入 Sprinting/Jogging）或删态清常量、furniture 门改按真实移动速度判定。
- 附带同域小项：剑基础 `spend_stamina` 无余额预检（`sword_basics.rs:810-822`，1 点体力可打出 8 耗 Cleave），对照 `sword_path/skill_register.rs:846-853` 有预检并注释自认是 review 修复——统一为有预检。

## P4 — movement 死负载 + 阈值口径统一（中）

- **死负载**：server 5Hz 计算并编码 `stamina_current/stamina_max/low_stamina/stamina_cost_active`（`movement/mod.rs:589-607`、`schema/movement.rs:29-35`），client 严格解析入库（`MovementStateHandler.java:50-52`）后**零消费者**（唯一渲染源是 `combat_hud_state.stamina_percent`）。按「不写兼容层」偏好：接消费者或从 payload 直接删字段（proto/schema/sample 连动改）。
- **四套低体力口径**：wire 标志 0.30（`movement/mod.rs:48`，没人读）/ 横条变红 0.25（`StaminaBarHudPlanner.java:39`）/ 竖条闪烁 0.15（`MiniBodyHudPlanner.java:58`）/ 速度惩罚绝对值 10.0（`movement/mod.rs:47,836-840`，乘子却叫 `EXHAUSTED_SPEED_MULTIPLIER` 而与 Exhausted 态无关）。统一单一 source（建议服务端算好单一 low 语义下发）。
- 附带：组件缺失 fallback 方向相反——速度算子当满体力（`movement/mod.rs:237` `unwrap_or(100.0)` 永不减速），payload 当空体力（`:590-592` `unwrap_or((0.0, 1.0))` 且 `low_stamina=true`）。

## P5 — 契约债与死字段簇（中低）

- **combat_hud_state 零 schema 覆盖**：唯一驱动体力渲染的通道，无 TypeBox、无 generated JSON schema、无 sample（对照 movement_state 四件套齐全 + roundtrip，`server_data.rs:5963`）；且不在 `plan-bughunt-server-data-s2c-schema-union-drift-v1` §6 补齐清单内。补：TypeBox 定义 + generated + 正反 sample + Rust roundtrip。
- **死字段簇**：`last_drain_tick` 6 处写 0 处读（`lifecycle.rs:150`、`movement/mod.rs:721`、`sword_basics.rs:821`、`carrier.rs:815`、`resolve.rs:1064`、`skill_register.rs:938`；恢复延迟从未实现，写入语义还不统一）；`ShieldSpec.stamina_drain_per_s` 8 盾模板配齐但运行时被 `shield_block_profile` 硬编码覆盖（`inventory/mod.rs:221-231` 注释自认 P4 承诺未兑现）；`technique_proficiency.stamina_cost_mult` 有算有测零消费（`technique_proficiency.rs:102`）。逐一拍板：接线或删除。
- 附带小项：HuGuSan 直写 `stamina.max`（`client_request_handler.rs:16284-16290`）会在 4 tick 内被 `status.rs:383-385` 用不同公式重算覆盖；`StaminaRecovBoost` 靠 magnitude `<1.0 / >=1.0` 分流 max/回速两种语义（`status.rs:367-397`，脆弱）；体力不持久化 = 重连免费回满（`combat/mod.rs:116-133` 一律 `Stamina::default()`），与血条审计「重连洗白」同类，是否入库随血条方案一并拍板。

## 两轮反方裁决（高危三项）

- **P0 反方 ①**：「resolve 的覆写会被 lifecycle 目标分支的保护恢复」。裁决：证伪——该保护（`lifecycle.rs:161-166`）只是不覆写，从不回写 ShieldBlocking；全仓唯一赋值点 `shield_block.rs:325` 在新举盾路径，幂等分支 `:293-310` 不重设。
- **P0 反方 ②**：「受击跌出举盾态是有意的打断设计」。裁决：证伪——跌出后 `ShieldBlock` 组件仍在、减伤照常、只有 drain/叙事/复位坏掉，且目标分支注释明确声明举盾态不被战斗事件覆盖；若是打断设计不会只打断状态机的一半。
- **P1 反方**：「epsilon 守卫 + `.before(stamina_tick)` 已保证确定性」。裁决：证伪——守卫防重写不防互搏；写点 A 无任何 set 约束，A 相对 B 与 stamina_tick 的先后由调度器决定，同一二进制内固定但跨构建不定，境界差异不可依赖。
- **P2 反方**：「5Hz 全量是刻意的心跳设计」。裁决：存疑但按 bug 处理——若是设计不会挂在 `Changed<Stamina>` filter 后面（filter 形同虚设），且同文件写点 B 用了 epsilon 守卫风格自证「不动不标脏」是本仓惯例；但因重连补水寄生其上，修复必须捆绑 join resync，故单列为一个阶段而非顺手改。

## 验收口径

- P0：格挡中连续受击/出手，`stamina.state` 恒为 ShieldBlocking，drain 恒走 override（2~3/s）；放盾必回 Idle；低体力叙事在格挡受击中正常触发；Exhausted 强制放盾回归不破。
- P1：单一 owner 后，6 境界回速 pin 测试全绿；有/无丹药 buff 切换不再把境界值抹成 5.0；satiety plan 的恢复乘数注入点有明确挂载位。
- P2：满体力静止玩家的 `Stamina` 不再每 4 tick 标 Changed（emit 频率回归真变化驱动）；断线重连后 200ms 内收到显式首帧体力快照（e2e 覆盖）；重连瞬间 HUD 不出现「active + 全满」假帧。
- P3：12.0 常量拍板落地（改名入档或调值），测试随拍板更新且失败信息写明设计依据；三态要么被真实移动赋值要么连常量一起删；furniture 灵韵门语义与注释一致。
- P4：movement payload 的 stamina 字段要么有真实消费者要么全链（proto/schema/sample/client）删除；全链路只剩一套低体力阈值语义。
- P5：combat_hud_state 四件套（TypeBox/generated/正反 sample/roundtrip）齐；死字段逐项有「接线 PR」或「删除 commit」着落，不留纯写不读。
