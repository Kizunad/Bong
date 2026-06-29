# plan-bughunt-r5-findings-v1（active）

> **Active（已从 skeleton 升级，待逐项消费）**。一句话主题：代码库自检 bug-hunt **round5**（fresh origin/main worktree ROOT，换角度：worldgen runtime · death/respawn/棺链路 · 经脉 declare 系统审计 · client VFX/动画注册完整性 · faction/声望）确认的 **7 个新真 bug**——含 **1 critical（复活/新建角色后不清 CoffinComponent → 玩家被 `pin_coffin_players` 每 tick 永久传送回棺内）**。已对 r1-r4 去重，全部 real-on-main。

> 立项动机：round5 用 fresh origin/main worktree 为 ROOT，5 全新角度 finder → 怀疑者对抗 → opus 逐条 Read/Grep 复核，8 候选 → 7 REAL / 1 NOT_REAL（dismiss：raster decoration_palette 无上界——manifest 受信 worldgen 产物，仅理论 OOM）。本轮两条主线：**VFX emit-orphan 整簇**（skull_fiend/supply_coffin/hybrid_beast 共 8 个粒子 id client 零注册，延续 r3 VoidPath 主题）+ **棺/派系跨会话状态机漏分支**。

## 阶段总览（按主题分组，逐项独立可修）

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 🔴 复活/新建角色不清 CoffinComponent → 永久钉棺 | plan_skeleton | ✅ 2026-06-29 |
| P1 | worldgen NBT 结构锚点偏移（structures.rs 漏 centering） | fix_pr | ✅ 2026-06-29 |
| P2 | anqi.charge_carrier 漏运行时经脉门 + declare | fix_pr | ✅ 2026-06-29 |
| P3 | VFX emit-orphan 整簇（8 粒子 id client 未注册） | plan_skeleton | ✅ 2026-06-29 |
| P4 | 派系 Resign/Expel/Betray 重连自动复籍 | fix_pr | ✅ 2026-06-29 |

## P0 — 🔴 复活/新建角色不清 CoffinComponent（critical）

- **#2 critical（plan_skeleton）**：`server/src/combat/lifecycle.rs` `revive_lifecycle`（1350-1465）在 `position.set(spawn_anchor)`（1454-1461）后 send `PlayerRevived`，但**从不 `remove::<CoffinComponent>` 也不调 `CoffinRegistry::clear_player`**（整文件零 coffin 引用）；`reset_for_new_character`（1743-1748）移除 Casting/PendingInsightOffer/TribulationState/OverloadedMarker 但**缺 CoffinComponent**。而 `coffin/mod.rs:1058-1062` `pin_coffin_players` **无条件**对所有持 `CoffinComponent` 的 Client 每 tick `position.set(coffin_player_position)`，无生命周期门控。可行性：延寿棺 `lifespan.rs:438-443` `coffin_lifespan_multiplier` 只放缓 years_lived 不停止 → 在棺中等寿命是常规用法，寿尽死亡→复活/新建角色即触发；dev `/kill self`、被钉住时被击杀亦可触发。后果：**玩家被永久钉在棺内**，`CoffinRegistry.occupied_by` 永不清空致该棺对外永久"已占用"。修：镜像 `handle_coffin_leave_requests`（665-684）的完整离棺序列（clear_player + remove CoffinComponent + persist_in_coffin(None) + CoffinStateChanged + 恢复 invisible），revive 与 reset 两入口都加，跨 combat↔coffin 边界。**critical，需跨模块状态机收口。**

## P1 — worldgen NBT 结构锚点偏移

- **#0 major（fix_pr）**：`server/src/world/terrain/structures.rs:2530` `stamp_structure` 缺 P7 centering——`nbt_registry.rs:291-335` 的 stamp 把 template-local `[0,0,0]`（角）锚在 `surface_pos.x/z`（块位 = base_origin + rotated_offset，offset 从 `[0,0,0]` 起算）；structures.rs:2530 直接 `surface_pos = BlockPos::new(origin_x, base_y-1, origin_z)`，而所有 `rasterize_*`（foundation 2134、column 2172、altar 1925/1971）用 `-radius..=radius` 围绕 `instance.origin_x/z` **对称（中心锚）**。`flora.rs:686-713` 对同一 stamp 协议**显式加了 `half_x/half_z + rotation.apply` centering shift**（`world_x - centre_dx`）并注释说明 `[0,0,0]` 是 corner；structures.rs **完全无** half_x/centre_dx（grep 仅 2529 注释**错称** "origin_x/z is the structure centre"）→ 多块授权 NBT 模板整体偏向 +x/+z 约 footprint/2，与它替换的程序几何锚点不一致（**呼应 worldgen-v4 P6 锚点遗留：flora 侧已修，structures 侧漏**）。单块模板测试（2724-2763）掩盖此偏移。修：照搬 flora.rs 的 rotated 半幅 offset。**局部明确。**

## P2 — anqi.charge_carrier 漏运行时经脉门

- **#3 major（fix_pr）**：`server/src/combat/carrier.rs:252-300` `resolve_anqi_charge_skill` **只查 cooldown(262)+qi(275)，无 `blocked_meridian`**。玩家路径：`ChargeCarrier` 是独立 `ClientRequestV1` 分支（`client_request_handler.rs:1913-1924`）直发 `ChargeCarrierIntent`，**不走** `handle_skill_bar_cast`，且 `validate_skill_config_before_cast`（7402-7427）只校 SkillConfig schema 零经脉检查。对照 5 个兄弟暗器技走 `resolve_anqi_skill`，其 431 行有 `blocked_meridian(MeridianSeveredPermanent)` 运行时门；`anqi_v2.rs:345-354` declare 了 single_snipe/multi_shot/soul_inject/armor_pierce/echo_fractal **唯独漏 charge_carrier**；`known_techniques.rs:684-688` 标 charge_carrier `required_meridians:[Lung,0.01]` 但该字段仅被 NPC 选技门 + HUD 快照消费，**无玩家 cast-time 强制** → 已学会封骨后被断肺经者仍可施放，绕过 worldview §四（与 r1 `plan-skill-cast-meridian-gate-v1` 同主题，别招）。修：`anqi_v2.rs` 补 `.declare(charge_carrier, [Lung])` + carrier.rs 补 `blocked_meridian`。**局部明确，可并入 r1 经脉门 plan。**

## P3 — VFX emit-orphan 整簇（client 未注册）

> 三处 server emit `SpawnParticle` 但 client `VfxBootstrap.java` 零注册，`BongVfxParticleBridge.lookupPlayer`（47-53）非 botany 事件无兜底 → `bridgeMiss` 静默丢。延续 r3 VoidPath 五招主题，本轮新增 3 簇共 8 id。

- **#4 major（plan_skeleton）**：`server/src/npc/skull_fiend.rs:43-46` 定义 `bong:skull_fiend_locking/trail/impact/stunned` 4 id，经 `emit_skull_fiend_vfx`（701）发 → client 全目录 grep 零注册 → 锁定追踪/尾迹/撞击/眩晕战斗视觉全失。
- **#5 major（plan_skeleton）**：`server/src/supply_coffin/lifecycle.rs:195-196` emit `bong:supply_coffin_break`、`refresh.rs:127-128` emit `bong:supply_coffin_emerge` 2 id → client 零注册 → 供棺出现/破棺核心寻宝视觉缺失。
- **#6 major（plan_skeleton）**：`server/src/fauna/hybrid_beast.rs:476-477` emit `bong:vfx/hybrid_formation`(count=24)、800/814 emit `bong:vfx/hybrid_rage`(count=8/16) 2 id → client 零注册 → 融合仪式汇聚 + 低血量狂暴粒子全失。
- 修：client 各自增 VfxPlayer 注册（按 server count/color 设计差异化粒子，遵 [[feedback_skill_av_diff]]）。**统一收口为 VFX roadmap（连同 r3 VoidPath + r2 RenewCompleted 音效）。**

## P4 — 派系状态机

- **#7 major（fix_pr）**：`server/src/social/mod.rs` 派系 `Resign/Expel/Betray` 终止后**重连自动复籍**。`Resign`（1441-1444）`remove::<FactionMembership>` 后随即 `persist_social_faction_membership`（1471-1482），而 `next_membership.faction = event.faction`（**仍为原派系**，loyalty-20）；`Expel/Betray`（1446-1455）同样 remove 后 persist（faction 仍原派系 + invite_block）。persist 是 UPSERT（`ON CONFLICT DO UPDATE faction=excluded.faction`，2724+）**保活该行**；全仓 grep **无任何 `DELETE FROM social_faction_memberships`**。重连 `load_social_faction_membership`（2401-2449）**无 active-membership guard**，有行即返 Some，faction 经 `from_str_name` 映回原派系（非 Neutral）；`attach_social_bundle_to_joined_clients`（286-288）无条件 insert 该组件 → 三种终止跨会话均不完整：Resign 零门禁干净复籍，Expel/Betray 的 invite_block 仅门控 AcceptInvite 不门控重连重挂。现有测试（4183+）只验同会话组件移除 + DB loyalty=-10，未覆盖重连。修：终止分支把 `next_membership.faction` 置 `Neutral`（保留 betrayal_count/permanently_refused 供 invite-block），或 load 端加 guard。**局部明确。**

## §N 开放问题

1. #2 棺收口：revive/reset 两入口都补离棺序列 vs 在 `pin_coffin_players` 加 alive/lifecycle 门控（治本）——是否需 CoffinComponent 与 death_lifecycle 显式互斥不变量。
2. #0 structures.rs centering：直接照搬 flora.rs 半幅 offset；是否抽 `nbt_registry` 公共 centering helper 让 flora/structures 共用（消除"一处修一处漏"）。
3. #3 charge_carrier：并入 r1 `plan-skill-cast-meridian-gate-v1`（同经脉门主题）还是独立——建议并入，且顺带系统审计**所有** anqi/暗器路径的 declare 完整性。
4. #4/#5/#6 VFX 簇：是否与 r3 VoidPath 五招 + r2 RenewCompleted 音效合并成一个"视听补全 roadmap" plan（统一 client VfxPlayer 授权 + 粒子差异化设计），还是逐簇 fix_pr。
5. #7 派系：faction 置 Neutral vs load guard——前者更彻底（DB 行语义即"无派系记忆"），但需确认 betrayal_count/permanently_refused 不依赖 faction 字段。

## 审计来源

bug-hunt round5（workflow，5 全新角度 finder + 怀疑者对抗 + opus 裁决，8 候选）。**ROOT = fresh origin/main worktree**（方法论修正后第三轮）。已对 r1-r4 去重。**report-only**：critical 棺钉死优先；#0/#3/#7 局部明确可直接 fix_pr，#2/#4/#5/#6 需跨模块状态机/VFX 客户端授权设计。**本轮主线**：VFX emit-orphan 整簇（client 未注册粒子，与 r3 同类系统性缺口）+ 棺/派系跨会话状态机漏分支。**worldgen 锚点**：#0 呼应 worldgen-v4 P6 遗留（flora 已修 structures 漏，建议抽公共 centering helper）。

## Finish Evidence

> 本 plan 为 report-only findings 文档，记录 round5 自检确认的 7 个真 bug（含 1 critical）。全部已在后续独立 PR 修复并合并 origin/main；本归档 PR 仅补收尾文档（git mv + 本节，无代码变更）。核验方式：2026-06-29 代码审计。

### 落地清单（每阶段 → 真实模块）
- **P0 #2**（复活/新建角色不清 CoffinComponent → 永久钉棺，critical）：`server/src/combat/lifecycle.rs:1538` `revive_lifecycle` + `:1865` `reset_for_new_character` 均 `remove::<CoffinComponent>` + clear registry；测试 `revive_clears_coffin_component_and_registry_and_emits_state_changed`(`:4935+`)。
- **P1 #0**（worldgen NBT 结构锚点偏移）：`server/src/world/terrain/structures.rs:2547-2550` 加 `half_x/half_z/centre_dx/centre_dz` centering（对齐 flora.rs）；测试 `stamp_structure_centres_odd_footprint_on_origin`(`:3029`)。
- **P2 #3**（anqi.charge_carrier 漏运行时经脉门 + declare）：`server/src/combat/carrier.rs:290` 加 `check_meridian_dependencies` + `MeridianSeveredPermanent` 门；`anqi_v2.rs:361-363` 补 `declare(ANQI_CHARGE_CARRIER_SKILL_ID, [Lung])`；测试 `charge_carrier_cast_rejected_when_lung_severed`(`:1818`)。
- **P3 #4/#5/#6**（VFX emit-orphan 整簇，8 粒子 id client 零注册）：`client/.../NpcParticleVfxPlayer.java` 8 Kind 枚举 + `VfxBootstrap.java:128-143` 注册（skull_fiend×4 / hybrid×2 / supply_coffin×2）。
- **P4 #7**（派系 Resign/Expel/Betray 重连自动复籍）：`server/src/social/mod.rs:1546`(Resign) + `:1553`(Expel/Betray) `next_membership.faction = FactionId::Neutral`；测试组 faction=Neutral 验证(`:4880+`)。

### 关键 commit
- `dbfdad2ba` (2026-06-17) #603 — coffin clear on revive/terminate/new_char paths (r5-P0)
- `b743ce823` (2026-06-17) #598 — centre stamp_structure footprint on scatter point (r5-P1)
- `9f1fb5d3c` (2026-06-17) #594 — anqi.charge_carrier 补经脉门 + declare (r5-P2)
- `68b0dffd9` (2026-06-18) #614 — 接入 NPC 粒子事件注册（skull_fiend/hybrid/supply_coffin 8 id）(r5-P3)
- `e5d51ac36` (2026-06-17) #592 — 派系除籍后置 Neutral 防重连复籍 (r5-P4)

### 测试结果
本 PR 纯文档，无代码变更。各阶段 pin 测试见落地清单，落地代码于 #592/#594/#598/#603/#614 合并时 CI 全绿。

### 跨仓库核验（2026-06-29）
- **server**：`remove::<CoffinComponent>`(lifecycle 两路径) / `centre_dx`(structures) / `check_meridian_dependencies`(carrier) / `next_membership.faction = FactionId::Neutral`(social) 均命中。
- **client**：`NpcParticleVfxPlayer` 8 Kind + `VfxBootstrap.java:128-143` SKULL_FIEND_LOCKING/TRAIL/IMPACT/STUNNED、HYBRID_FORMATION/RAGE、SUPPLY_COFFIN_EMERGE/BREAK 注册命中。
- **agent**：无改动。

### 遗留 / 后续
无。7 项发现全部修复并测试锁定。
