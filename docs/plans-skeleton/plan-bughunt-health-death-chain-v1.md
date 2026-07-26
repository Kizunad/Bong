# plan-bughunt-health-death-chain-v1 — 血条/死亡链系统性收口：0血僵尸、severity 双量纲、流血螺旋、死态重连逃逸、状态机杂症

> **一句话**：血量写入分散在 30+ 处而死亡判定靠各写入点自觉发 `DeathEvent`，已确认 8 条致死路径漏发（产生"0 血永久僵尸"：不死、不回血、后续攻击也杀不死）；叠加伤口 severity 双量纲让绷带/疗伤丹全体失效、流血无衰减锁死回血、死态/运数不落盘让重连逃脱死亡裁决——本 plan 收口审计确认的这批正确性缺陷。**范围诚实声明**：Alive 状态残血/伤口的重连恢复（"低血退登=满血"）是否纳入由 §10#3 拍板；若决议不落盘，则该项作为已知遗留登记在 §9 并指向后续 plan，本 plan 不声称覆盖它。
>
> 来源：2026-07-26 血条系统全链路审计（4 路只读 Explore 分头映射伤害源/死亡状态机/治疗回复/客户端显示 + 主循环逐行亲验，memory `project_health_system_audit_20260726`）。所有 file:line 均为亲验锚点。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | 0血僵尸收口：唯一致死结算边界（字段私有化）+ 8 条漏发路径迁移 + 守卫 | ⬜ |
| P1 | severity 双量纲统一：绷带/疗伤丹对实战伤口失效 + 腿/头阈值恒真 | ⬜ |
| P2 | 流血闭环：自然止血衰减 + Bleeding 状态同步（含调度契约） | ⬜ |
| P3 | 死态重连收口：Lifecycle 死态/deadline/运数/DeathRegistry 持久化回读（Alive 伤情范围按 §10#3 决议） | ⬜ |
| P4 | 状态机杂症清扫：weakened 全链路接线/stabilized 零代价/血炼旁路/医道窗口/重试风暴/双时钟/硬编码 0.05 | ⬜ |
| P5 | 治疗侧记账与 client 防御：过量治疗/retain 误删/npc_heal 弃 target/entries 无上限/client NaN 冻结 | ⬜ |

---

## §1 接入面（docs/CLAUDE.md §二 checklist）

- **进料**：
  - `combat::events::DeathEvent` + `emit_death_event_if_lethal`（events.rs:232-261；P0 后者收编为结算核心内部实现，不再作为公开旁路）
  - `combat::lifecycle::death_arbiter_tick / near_death_tick / handle_revival_action_intents / auto_confirm_revival_decisions`（既有死亡链，只消费不重造）
  - `alchemy::pill::{apply_wound_heal, is_severed_like, wound_grade_delta}`（P1/P5 修正对象）
  - `persistence` sqlite：`death_registry` 表已建（persistence/mod.rs:1197）但**只写不读**；`persist_near_death_transition` 已在写死态（P3 实施时核对写侧字段集，缺 deadline 字段则同步补写侧）
- **出料**：
  - 全部致死路径 → `DeathEvent` → 既有 `death_arbiter_tick` 消费（不新开死亡路径）
  - 血量/伤口变化 → 既有 `combat_hud_state` / `wounds_snapshot`（`Changed<Wounds>` 触发，修复后自动生效，无需动 emit 层）
  - P0 唯一结算入口是 [[plan-satiety-hydration-v1]] P1 饥渴掉血（已声明走 DeathEvent）的现成工具
- **共享类型 / event**：复用 `DeathEvent`/`Wounds`/`Wound`/`Lifecycle`/`StatusEffects`/`ApplyStatusEffectIntent`；**不新造任何死亡/复活 event**（dev revive 豁免走标记组件，见 §7#8，不开独立事件）。新增仅限：`combat::damage` 结算模块（P0）、`StatusEffectKind::Weakened`（P4，§10#5 决议后，全链路交付见 §7#1）、`load_death_registry`（P3）。
- **跨仓库契约**：server 为主，两处例外必须显式验收：① P4 `Weakened` wire id——server `network/status_snapshot_emit.rs` 映射加行 + **wire id pin 测试** + client 侧通用状态渲染路径锚点核验（label 由 server 下发，client 走通用渲染；实施时给出 client 消费 file:line，并补未知 id 降级测试——"客户端免费可见"必须被证明而非断言）；② P5 client `CombatHudStateHandler` NaN 防御是 client Java 改动，走 `cd client && ./gradlew test` 门禁。agent 不参与。
- **worldview 锚点**：§十二 死亡、重生与一生记录（死亡必须有代价——死态重连逃逸/零代价免死直接违背正典）；§四 伤残与经脉（伤口/流血语义）。不改 worldview，纯落地既有语义。
- **qi_physics 锚点**：**不新增 qi_physics 常数、不新增真元流动路径**（收窄措辞：P2 的流血衰减是生命/伤情 gameplay 参数，非真元物理——命名常量放 `combat/components.rs` 常量区，声明单位 per-sec 与 tick 换算，配 tick-rate 无关 pin 测试，不入 qi_physics）。僵尸收口是守恒的正向修复——0 血僵尸实体永不 Terminated，其携带的 `qi_current` 永久滞留、不走 `on_player_terminated → release_terminated_qi_to_zone` / NPC 侧 `release_dormant_qi_to_zone` 归还 zone；收口后释放链恢复可达。

## §2 审计事实底座（实施前先读，全部亲验）

死亡判定是**事件驱动**，无中央 0 血扫描：血量写入方必须自己发 `DeathEvent`。三个系统性放大器让漏发变成永久僵尸：

1. `wound_bleed_tick` 跳过 `health_current <= 0.0`（combat/lifecycle.rs:182）——流血不补刀；
2. `can_health_regen` 拒绝 `health_current <= 0.0`（combat/lifecycle.rs:248）——永不回血；
3. 主 resolver 死亡判定要求 `was_alive`（combat/resolve.rs:811 捕获、:2092 检查）——**后续任何攻击也无法再触发死亡**。

`enter_near_death` 模块 helper（combat/lifecycle.rs:2122-2142）会把血钳到 `min(current, 5%)` 并清空 StatusEffects；`Lifecycle::enter_near_death` 组件方法（combat/components.rs:268-277）只早退 `NearDeath`，**不挡 AwaitingRevival/Terminated**。玩家 join 无条件插 `Wounds::default()` + `Lifecycle::default()`（combat/mod.rs:116-133）。

## §3 P0 — 0血僵尸收口：唯一致死结算边界 ⬜

**设计原则（review r1 收口）**：正确性靠**类型边界**保证，不靠文本扫描和调用方自觉。全仓只允许存在**一个** canonical 结算核心。

**交付物**：

1. **字段私有化**：`Wounds.health_current` 改私有（`pub(crate)` 或 private + 受控 API：`health()` 读、`settle_damage(...)` 扣、`heal(...)` 加、`#[cfg(test)] set_health_for_test`）——绕行直写在编译期不可表达。全仓 30+ 写入点在本阶段一次性迁移（含合法白名单：lifecycle 钳血/revive/debug/spawn 初始化，各自走对应受控方法）。
2. **唯一结算核心** `server/src/combat/damage.rs`：私有核心函数持有完整语义（clamp、`was_alive` 捕获、致死发 `DeathEvent`）；对外仅两个薄适配——直通版（供已持有 `&mut Wounds` + `EventWriter<DeathEvent>` + GameMode 查询结果的 system，签名显式收 `is_damageable` 前置结果，杜绝"拿不到实体上下文却声称做守卫"的含糊）与 deferred 版（供 `commands.add` 闭包，内部自查 GameMode）。**两适配共享同一核心，测试锁两者语义 bit 级一致**。`emit_death_event_if_lethal` 迁入 damage 模块收窄为私有实现，woliu（combat/woliu_v2/skills.rs:692-711）、dugu（combat/dugu_v2/skills.rs:615-630）两个既有用户同批迁移到新入口。
3. 八条漏发路径全部迁移到唯一入口（cause 字符串建议）：

| 路径 | file:line | cause |
|---|---|---|
| 暗器弹道命中（完整远程武器链） | combat/carrier.rs:1078 | `anqi:{attacker_id}` |
| 击退碰撞伤害（另补 :630/:685 两处调用点缺失的 GameMode 守卫） | npc/movement.rs:934 | `collision:{inflictor}` |
| 剑格反伤打攻击者（deferred command） | combat/resolve.rs:1186-1192 | `parry_reflect:{defender_id}` |
| 绝壁法则反噬自伤（deferred command） | combat/resolve.rs:905 → combat/zhenmai_v2.rs:910 | `juebi_backfire` |
| 兽夹 snap | zhenfa/mod.rs:3825 | `beast_trap` |
| 拆阵反噬（硬编码 -6.0，同文件另两条阵法路径 :3566/:3794 都发了唯独这条没发） | zhenfa/mod.rs:4161 | `zhenfa_backlash` |
| 服丹异种真元排斥（-10.0，兼缺 GameMode 守卫） | network/client_request_handler.rs:15908 | `foreign_qi_rejection` |
| 骷髅妖撞墙自伤（精英怪自撞成不可击杀木桩） | npc/skull_fiend.rs:826 | `wall_impact` |

4. **守卫降级为辅助线**（review r1 D#3 采纳）：类型边界是主防线；`test_coverage_guards.rs` 的源码扫描守卫仅作第二道自测——明列覆盖的写法模式（直接赋值/复合赋值/整体 `Wounds` 替换/`set_health_for_test` 泄漏到非 test），并为守卫本身配正反样例测试（构造应命中/不应命中的代码样本，证明不漏检不误报）。plan 文本不再称其为"编译期可核验契约"。
5. 测试：每条路径一个致死单测（血打到 0 → 必发 DeathEvent → 状态机进 NearDeath/Terminated）；僵尸不可再入回归（既有 0 血实体再受击不重复发 DeathEvent——`was_alive` 语义保持）；直通/deferred 两适配对同输入产出一致 pin；woliu/dugu 迁移后行为不回归（既有测试全绿）。
6. bot 场景：`scripts/bot/scenarios/` 新增暗器致死观察场景（远程投掷打死 NPC → 断言死亡链事件可观察 + NPC despawn），锁"远程武器致死必走死亡链"。

纯 server 逻辑，视听豁免（docs/CLAUDE.md §四视听条款）；死亡表现复用既有 `npc_death_smoke`/`npc_death_qi_burst`（combat/lifecycle.rs:1690-1697）——收口后这些既有 VFX 自然恢复触发。

## §4 P1 — severity 双量纲统一 ⬜

**现状**（亲验）：resolve 产出伤口 `severity: damage`（HP 量纲，且 `damage.max(1.0)` 地板，resolve.rs:809/:1160）；而 alchemy 侧当 0..1 比例用——`is_severed_like = severity >= 0.85`（alchemy/pill.rs:717-719）把一切未被格挡/护甲大幅削减的实战伤口都判成"断肢"跳过 → **绷带/夹板/活血丹/续骨膏/NPC 治疗对实战伤口全部静默 no-op**。连带 `LEG_SLOWED_SEVERITY_THRESHOLD=0.3` / `HEAD_STUN_SEVERITY_THRESHOLD=0.5`（components.rs:25-28）恒真：任何腿/头命中必减速/眩晕。`/wound add` 却 clamp 0..1（combat/debug.rs:59），zhenfa/botany/movement 生产 0.02~0.25——两套约定并存于同一字段。

**交付物**（方向按 §10#1 决议）：

1. `Wound.severity` 单一量纲声明写进 components.rs 字段文档（唯一真相源）；
2. `is_severed_like` / `wound_grade_delta` / `LEG_SLOWED_*` / `HEAD_STUN_*` / debug clamp / zhenfa·botany·movement 生产点全部对齐同一量纲，阈值改引命名常量；
3. 回归测试：徒手/持械打一刀 → 用 `bandage` → severity 下降、`bleeding_per_sec` 等比下降、HP 回升（当前是 no-op，测试先红后绿）；续骨膏/活血丹/`npc_heal_basic` 同理各一条；腿伤减速/头伤眩晕**不再恒触发**（轻击不触发、重击触发，boundary 各一条）。

## §5 P2 — 流血闭环 ⬜

**现状**（亲验）：伤口无任何自然愈合/衰减系统（combat/decay.rs 是真元距离衰减，与伤口无关）；五种伤型 bleed_mul 无一为 0（resolve.rs:2344-2377）；`has_active_bleeding` 锁死回血（lifecycle.rs:250）→ 中一刀不吃药必然流血至死。resolve 还给流血伤口挂 `duration_ticks: u64::MAX` 的 Bleeding 状态（resolve.rs:1681-1690），全库无人移除 → HUD"流血"标记永久悬挂。活血丹副作用推的 Bleeding 状态（pill.rs:568-572）零实际伤害，纯 HUD 幻觉。

**交付物**（衰减形态按 §10#2 决议）：

1. **止血衰减**：`wound_bleed_tick` 内联衰减（不新开 system，避免新调度面）：每次流血结算后 `bleeding_per_sec` 按命名常量衰减（常量放 components.rs 常量区，单位 per-sec、tick 换算显式，tick-rate 无关 pin）——丹药/绷带仍是加速手段而非唯一手段，拆掉"一刀必死螺旋"；
2. **Bleeding 状态同步（调度契约显式，review r1 收口）**：新 system `bleeding_status_sync`，注册 `Update` + `CombatSystemSet` 内 `.after(resolve_attack_intents).after(wound_bleed_tick).after(status_effect_tick)`，且先于 `network` 侧 `status_snapshot_emit` 所在 set（同 tick 快照必须反映同步后终态；现有 set 拓扑锚点：combat/mod.rs:278-315）。语义：`Wounds` 为唯一真相源——`has_active_bleeding == true` 则确保 Bleeding 状态在（挂载/续期），`false` 则摘除；resolve.rs:1681-1690 的 u64::MAX upsert **删除**，挂载职责全部移交同步 system；
3. 活血丹副作用语义收口：Bleeding 状态改为对应真实微量 `bleeding_per_sec` 伤口（经同步 system 自然产生状态），消灭"HUD 显示流血但不掉血"的幻觉分支；
4. 测试（四类状态转换 + 调度，review r1 收口）：无伤→中刀同 tick 挂载 Bleeding；持续流血保持；衰减归零/治疗归零 → 摘除且 `can_health_regen` 恢复 true；同 tick 新增+归零时快照见终态；同步 system 重复运行幂等；衰减曲线 pin（引常量）；活血丹副作用真实掉血 pin。

无新增玩家交互，HUD 复用既有 wounds_snapshot 红点 + status"流血"标记——无新增视听资产。

## §6 P3 — 死态重连收口 ⬜

**现状**（亲验）：join 无条件 `Wounds::default()` + `Lifecycle::default()`（combat/mod.rs:116-133，仅回填 character_id/spawn_anchor）；`death_registry` 表只写不读（persistence/mod.rs 只有 upsert_death_registry:7180）。后果：NearDeath/AwaitingRevival 断线重连=完全逃脱死亡（修为惩罚/掉落/死亡屏全跳过）；`fortune_remaining` 每登录重置 3、death_number 重置 → "前 3 死保底"变永久保底，劫数期实际不可达。唯一既有兜底是 join 转世门（cultivation/mod.rs:756-790，仅 Terminated 末条触发）。

**交付物**：

1. **死亡计数单一权威（review r1 收口）**：`DeathRegistry.death_count` 为 canonical；`Lifecycle.death_count` 降级为运行时派生值——加载时从 registry 对齐，运行期两处自增点收敛为 registry 先行、lifecycle 跟随（同一 system 内顺序写，不跨 tick）。加载遇不一致按 `max` 修复并 `tracing::warn`。测试：不一致数据加载对齐 pin、单侧缺失 fallback pin、写失败不回退计数 pin。（顺带收掉审计 S8 三处自增点漂移。）
2. `persistence`：新增 `load_death_registry`（回读 death_count/last_death_tick/last_death_zone）+ Lifecycle 死态回读：`state` / `fortune_remaining` / `weakened_until_tick` / **`near_death_deadline_tick` / `revival_decision_deadline_tick`（绝对 tick 持久化，review r1 收口——离线期间服务器 tick 照走，窗口不因下线暂停；写侧 `persist_near_death_transition` 若缺这两列则同步补写侧+迁移）**。deadline 缺失兜底 `now + 对应窗口` **仅限旧 schema 行一次性迁移**，恢复后立即回写持久化，杜绝重连刷窗口；
3. join 时 NearDeath/AwaitingRevival 恢复：重进对应状态；AwaitingRevival 恢复时**重发死亡屏 payload**（顺带修"死亡屏只在进入 tick 发一次、丢包即黑屏"）；`weakened_until_tick` 未过期 → 重挂剩余时长的 `Weakened` 状态（依赖 §7#1 的状态定义，闭环"持久字段↔运行时状态"双向同步）；
4. **Alive 伤情范围**：按 §10#3 决议执行。选项 A（血量+伤口落盘）则本阶段加 `wounds` 持久化 schema + 版本迁移 + Alive 残血/流血/无伤三类重连测试；选项 B（不落盘）则**本 plan 标题范围即不含 Alive 洗白**，在 §9 登记遗留 + 指向后续 plan，且 P3 完成声明里明确写"Alive 残血重连恢复满血为已知未修"；
5. 测试：三态重连恢复 pin ×3（NearDeath 续原窗口 / AwaitingRevival 重见死亡屏且 deadline 不重置 / Terminated 走既有转世门不回归）；**窗口将尽时反复重连不获得新窗口、已过期重连立即推进状态机**；fortune 不因重连重置；death_number 跨会话连续（劫数期可达性回归）；损坏/缺失持久化值的降级分支。

## §7 P4 — 状态机杂症清扫 ⬜

逐项小修，每项独立可核验（全部亲验锚点）：

1. **weakened 全链路接线**（review r1 收口为端到端交付）：现状 `weakened_until_tick` 全库零消费者（唯一"读"是过期清理 lifecycle.rs:760-763）。交付链条：`StatusEffectKind::Weakened` 定义（减益数值见 §10#5）→ `attribute_aggregate_tick` 聚合消费 → revive 时挂载（时长=weakened_until_tick 剩余）→ 到期由既有 status 生命周期摘除且**同步清 `weakened_until_tick` 字段**（双向同步，杜绝字段/状态漂移）→ `status_snapshot_emit` wire id 映射 + **wire id pin 测试** + client 通用状态渲染路径锚点核验 + 未知 id 降级测试 → 重连重建（§6#3）。状态转换测试：复活挂载 / 到期单次摘除 / 濒死清状态后再复活重挂 / 重连按剩余时长重建；
2. **stabilized 零代价免死**：濒死 30s 内血被抬过 5% 即回 Alive（lifecycle.rs:770-777），不降境界/不扣运数/不设 weakened——对比正规复活降一级境界，激励倒挂。按 §10#4 决议补代价（推荐挂 Weakened，与 [[plan-neardeath-ux-v1]] 挣扎/救援复用该分支协同）；
3. **血炼旁路 arbiter**：baomai_v3/skills.rs:468-471 直调组件方法进濒死，跳过持久化/传记/死亡登记/DeathInsight，且组件方法不挡 AwaitingRevival（死亡屏期间施血炼可把状态拍回 NearDeath、death_count 通胀）。修：`Lifecycle::enter_near_death` 补 AwaitingRevival/Terminated 早退（components.rs:268）+ 血炼改走 death_arbiter 链（发既有事件，不直调）；
4. **医道救援窗口错位**：`dying_window_ticks = lerp(60,90)s`（qi_physics/healing.rs:126）恒被 `NEAR_DEATH_WINDOW_TICKS = 30s` 截断——熟练度梯度完全不可观察。**方案按 §10#7 决议**（review r1 收口：单纯 clamp 不修复梯度，必须选一种保留单调可观察差异的口径），交付含低/中/高熟练度三档 boundary pin，证明有效救援收益严格可区分；
5. **AwaitingRevival 流血/充能跳过表缺项**：wound_bleed_tick 跳过表 `{NearDeath, Terminated}` 漏 AwaitingRevival（lifecycle.rs:188-194，对照 can_health_regen 三态全列可证是遗漏）；carrier.rs:445-451 充能门同漏。补齐；
6. **auto_confirm 重试风暴 + 掷骰种子**：持久化失败时 AwaitingRevival 保持 + deadline 已过 → `auto_confirm_revival_decisions`（lifecycle.rs:1102-1123）每 tick 重发 intent，且 `roll_rebirth` 以 clock.tick 为种子每 tick 重掷。修：重发间隔退避（每 20 tick）；种子改为**确定性组合哈希 `hash(character_id, death_number, revival_decision_deadline_tick, "rebirth-roll-v1")`**（review r1 收口：单用 deadline 会让同 tick 决策的多玩家共享随机流）。测试：同一决策重试骰点恒定 + 不同角色/不同 death_number 即使 deadline 相同也不共享结果 + intent 频率 ≤1/20tick；
7. **`/kill self` 时钟异类**：唯一用 `CultivationClock` 填 `DeathEvent.at_tick` 的发送方（cmd/dev/kill.rs:44，其余全用 CombatClock），靠 `at_tick.max(clock.tick)` 兜住但会拉长濒死窗。统一 CombatClock；
8. **`/revive self` 暗带降境惩罚**：发 `PlayerRevived` → `on_player_revived → apply_revive_penalty` 境界降一级+真元清零（cmd/dev/revive.rs:47 + cultivation/death_hooks.rs:110），dev 命令语义应是纯回血复活。修（review r1 收口，**不新造 event**，与 §1 边界一致）：dev 命令在发 `PlayerRevived` 前插一次性标记组件 `DevReviveMarker`，`on_player_revived` 检出即跳过惩罚并移除标记；顺带清 `Wounds.entries`。测试：普通复活仍受罚 / dev revive 不受罚不清真元 / 生产路径无法伪造标记（标记仅 dev 命令模块可构造，`pub(crate)` 收窄）；
9. **硬编码 0.05 两处**：resolve.rs:2080（切磋保底）、tribulation.rs:4095（渡劫善后）改引 `NEAR_DEATH_HEALTH_FRACTION`（components.rs:22）；
10. **同函数双时间源**：handle_revival_action_intents 内 Terminate 分支用 `intent.issued_at_tick`（lifecycle.rs:1034）而 Reincarnate 用 `clock.tick`（:955），统一 clock.tick。

## §8 P5 — 治疗侧记账与 client 防御 ⬜

1. **过量治疗记账**：`apply_wound_heal` 回血按 `delta × changed`（每条伤口满额计），而非实际 severity 削减总量（pill.rs:429-443，`.max(0.0)` 截断后仍满计）——N 条微伤 + 一张绷带可虚增回血。改为累计实际削减量；
2. **retain 误删**：`entries.retain(|w| severity >= 0.05)`（pill.rs:441）连未被本次治疗触及的微伤口一并清除。改为仅清本次触及且归零者（阈值引常量）；
3. **npc_heal_basic 弃 target**：签名收 `_target` 却恒治 caster（npc/npc_skill.rs:308/334-337），治疗型 NPC 无法救人。接线 target（或改名 self_heal 并留 TODO 移交 NPC AI plan——取决于现有 AI 是否已按"救队友"选招，实施时核）；
4. **Wounds.entries 无上限**：30+ 生产点单调增长，每秒全量遍历+快照全量下发。加合并/上限策略（同部位同类型合并，或 cap 后丢最旧非重伤，常量声明）；
5. **续命/急救不对称**：`apply_life_extension` 血拉 50% 但伤口+流血原封不动（yidao.rs:956-957，对比急救 :901 清 bleeding）→ 救回来大概率再次流血倒地。对齐清 bleeding；顺带补 :956 上界 clamp；
6. **client NaN 整包冻结**（client Java 改动，非 server）：`CombatHudStateHandler` 任一字段非法即整包 noOp，qi/体力条一起卡死在陈旧快照（client CombatHudStateHandler.java:23-28）。改逐字段降级 + 日志；server 侧 `combat_hud_state_emit.rs:56` 对 NaN 补 `is_finite` 防线（clamp 不拦 NaN）。

测试：治疗记账 pin（削减量=回血量）、retain 白名单 pin、entries cap/合并 pin、续命清 bleeding pin、client handler 单字段坏值不冻结其余（`cd client && ./gradlew test` 门禁）。

## §9 边界（不做/移交/遗留，防重复劳动）

- **濒死权威 wire 契约 + client HP<0.12 阈值推断替换 + `[====]` 自救他救**：归 [[plan-neardeath-ux-v1]]（已有骨架，P0 即权威 NearDeathState 契约）。审计发现的"濒死时 hp_percent=0 → 红屏反而不显示"与"坚持代价 0.5 真元/秒 是 client 虚构文案"两项，在该 plan 切权威驱动后自然消失——本 plan 不动 client 濒死推断，避免双改冲突；
- **Alive 残血/伤口重连洗白**：若 §10#3 决议选 B（不落盘），此项为**本 plan 显式遗留**——P2 流血衰减落地后"重登止血"的刚需消失，但"残血重登回满"仍在；后续以 `plan-wounds-persistence-v1` 之类独立骨架承接（立骨架为选 B 时的 P3 交付物之一）；
- **饥渴掉血**：归 [[plan-satiety-hydration-v1]] P1（已声明走 DeathEvent；本 plan P0 唯一入口是其现成工具）；
- **摔落伤害不存在**：设计缺口非 bug，不在本 plan（如立项走 feature plan）；
- **voluntary_retire 物品蒸发**（善终不掉落=净销毁）：注释自认有意，物品守恒语义如需改走独立决策；
- **玩家 health_max 恒 100 无境界缩放**：设计现状不动。

## §10 开放问题（转 active 前按 docs/CLAUDE.md §五 收口成 §10.1 决议）

1. **severity 统一量纲方向**（P1 前置）：A. 统一 HP 量纲——阈值侧全部换算（`is_severed_like ≥ 0.85` → `≥ SEVERED_THRESHOLD_HP`，`wound_grade_delta` 0.25/档 → N HP/档，腿/头阈值同步），生产点少改；B. 统一 0..1 归一——resolve 侧 `severity: damage / health_max`，消费侧不动但 resolve/快照/浮字全要过一遍。**推荐 A**：resolve 是最大生产者且 wire `wounds_snapshot.severity` 已按其量纲下发，B 会牵动 client 显示校准。
2. **流血衰减形态**（P2 前置）：衰减曲线（推荐线性归零，速率常量初值定标为"重伤约 60-90s 自止"，实施时按 P1 决议后的 severity 量纲换算）vs 结痂阈值（severity 低于 X 才开始衰减）vs 仅解锁回血不衰减。**推荐线性衰减**——最简单可 pin，丹药/绷带仍有加速价值。
3. **Alive 伤情重连范围**（P3 前置，**直接决定本 plan 完成声明的范围**）：A. 血量+伤口落盘（版本化 schema+迁移+三类重连测试，彻底堵"退登洗白"，成本：schema/迁移/低血踢出时序）；B. 不落盘（只堵死态逃逸+运数刷新，Alive 洗白登记遗留+立后续骨架）。**推荐 B**——P2 修完后重登止血刚需已消失，Alive 洗白危害降为"省一次治疗"，而 A 的 schema 迁移面配不上 v1 的收口节奏；选 B 则标题范围按 §9 诚实声明。
4. **stabilized 免死代价**（P4#2 前置）：挂 Weakened 减益（**推荐**——濒死被救是 neardeath-ux 的核心玩法，降境/扣运数会废掉救援体验）vs 扣半点运数 vs 维持零代价。
5. **Weakened 减益数值**（P4#1 前置）：攻 ×0.7 / 防 ×0.7 / 移速 ×0.85（建议初值，经 `attribute_aggregate_tick` 聚合）；是否叠加其他 debuff 的 clamp 边界。
6. **战斗中回血门**（P2 修完后暴露）：流血解锁后 0.5HP/s 站桩回血是否加 `in_combat` 门（读 `CombatState.in_combat_until_tick`）。**推荐加**——符合硬核基调，且 15s 窗口常量现成。
7. **医道熟练度窗口口径**（P4#4 前置，review r1 新增）：A. 窗口重映射进濒死窗内——`dying_window = lerp(0.5, 1.0) × NEAR_DEATH_WINDOW_TICKS`（低熟练度只能在倒地前 15s 内救，高熟练度全窗可救；梯度单调可观察，状态机零改动，**推荐**）；B. 医者在场延长濒死 deadline（触碰核心状态机与 neardeath-ux 骨架抢地盘，不推荐）；C. 删除窗口加成、熟练度收益并入 hp_restore/qi_cost 既有梯度（最省但砍玩法维度）。

## §11 实施备注

- scope 预估 3-4 PR（P0 / P1+P2 / P3+P4 / P5），转 active 时按 docs/CLAUDE.md §六 补 §12 实施工作流章节；也可按 BugFix 工作流整 plan 单 subagent 消费——由启动会话拍板。**门禁按栈**：P0-P4 为 server 侧（cargo fmt/clippy/test）；P5 含 client Java 改动，必须加跑 `cd client && ./gradlew test build`（不得声称"全 server 侧"）；
- P0 的字段私有化 + 唯一结算边界是后续所有 P 的正确性底座，必须第一个落；P3 的 deadline 持久化依赖写侧字段核对（`persist_near_death_transition` 现有列集），实施首日先核 schema；
- 每个触及 gameplay 行为的 PR 配 bot 场景（AGENTS.md 硬约定）；P0 暗器致死场景为最小集；
- 全程不改 `docs/worldview.md`、不新增 qi_physics 常数/真元流动路径；涉及 `Lifecycle`/`Wounds` 序列化字段的改动同步核 `persist_near_death_transition` 读写对称。
