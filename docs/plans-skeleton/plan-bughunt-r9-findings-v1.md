# plan-bughunt-r9-findings-v1（骨架）

> **骨架（草案）**。一句话主题：代码库自检 bug-hunt **round9**（fresh origin/main worktree ROOT，角度：zhenfa 阵法新代码 · client 渲染/资源包 · payload 往返 · Bevy 时序 · dev 命令泄漏）确认的 **6 个新真 bug**——含 **player-facing HUD 错（spirit_qi_max 未下发 → 固元及以上境界真元条分母恒为 100）** + 涡流全 6 招缺 atlas 条目无贴图 + 拟态灰烬蛛常驻缺贴图 + 散灵珠 ledger 僵尸账户（zhenfa 新子系统）。已对 r1-r8 去重，全部 real-on-main。

> 立项动机：round9 转资产/纹理完整性 + payload 字段缺失 + zhenfa 新代码角度。11 候选 → **6 REAL / 5 NOT_REAL**（严格裁决——dev 命令无认证 #10/#11 是 CLAUDE.md 文档化 dev-harness 设计、rift_portal RemovedComponents 被 Bevy 0.14 auto sync point 化解、tsy DeathEvent 排序仅 hygiene、alchemy 5 字段是 env-gated mock dormant，均正确 dismiss）。本轮主题（资产纹理 / HUD payload / zhenfa ledger）均 r1-r8 未碰。

## 阶段总览（按主题分组，逐项独立可修）

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 🔶 spirit_qi_max 未下发 → 中高境界真元条分母错恒 100（HUD player-facing） | fix_pr | ⬜ |
| P1 | VFX/实体纹理缺失（涡流 atlas 条目 + 拟态蛛贴图） | fix_pr | ⬜ |
| P2 | 散灵珠 ledger 僵尸账户（zhenfa 新子系统守恒漂移） | fix_pr | ⬜ |
| P3 | 技能图标 PNG 大面积缺失（38/45 + tuike 3，美术 backlog） | plan_skeleton | ⬜ |

## P0 — 🔶 spirit_qi_max 未下发（HUD player-facing）

- **#6 major（fix_pr）**：`server/src/player/state.rs` 真元条**分母错**。`ServerDataPayloadV1::PlayerState`（`server_data.rs:309-317` + wire 变体 1130-1140）**只有 spirit_qi 无 spirit_qi_max**；活跃生产者 `server_payload_with_social_and_local_pressure`（`state.rs:236-247`）只发 `spirit_qi: cultivation.qi_current` 不发 qi_max；旧 `bong:bong` 通道的 `PlayerStatePayload`（有 spirit_qi_max）已无生产引用（grep 0 命中），client 全走 `bong:server_data → ServerDataRouter → PlayerStateHandler.java:61` `readOptionalDouble(...,NaN)` → `PlayerStateViewModel.normalizeSpiritQiMax` 回退 `Math.max(100, current)`。但 qi_max 从 base 10 经 `breakthrough.rs` `qi_max_multiplier` 累乘：**固元≈150、通灵≈525、化虚≈2625**（测试用例 qi_max=500/1000）远超 100 → **中高境界玩家真元条分母被钳到 max(100,current)，显示恒满或比例严重偏高**。真元条是核心 HUD 三状态条之一（[[project_hud_qualitative_status]]）。修：PlayerState schema（V1 + Wire + proto）补 `spirit_qi_max` + emit 端发 `cultivation.qi_max`（连 sample 对拍双端）。**活跃链路缺字段，局部明确。**

## P1 — VFX / 实体纹理缺失

- **#2 major（fix_pr）**：涡流全 6 招 VFX **无贴图渲染**。链路齐全：`vortex_spiral.png` 存在、`bong/particles/vortex_spiral.json` 描述符存在、`BongParticles.java:37/128` 注册 VORTEX_SPIRAL 类型 + Factory、`VortexSpiralPlayer.java:64/108/149` setSprite、server `woliu_v2/skills.rs` 全 6 招 emit `bong:vortex_spiral`——**唯独 `client/.../assets/minecraft/atlases/particles.json` 的 sources 里没有 `bong:particle/vortex_spiral`**。本仓库 `ShieldBreakParticleAssetTest`（断言 `bong:particle/wood_debris` 必须在 atlas，注释"缺则 MC 不烘焙贴图，Player 静默渲染空白粒子"）坐实这一精确失败模式。**与饱和的"粒子类型未注册"不同——此处类型已注册，是 atlas-stitch 条目遗漏。** 修：sources 补 `{"type":"single","resource":"bong:particle/vortex_spiral"}`（+ 补 ShieldBreak 同款 atlas 断言测试覆盖 vortex_spiral）。**一行修复。**
- **#3 major（fix_pr）**：`client/.../assets/bong/textures/entity/fauna/ash_spider_disguised.png` **缺失**。可达链路：server `spawn_ash_spider_npc_at` 默认初始化 **Disguised 态**（spawn_spider.rs 测试 139）+ 经 `mob_spawn.rs` `NaturalMobKind::AshSpider` 自然生成；`spider_disguise_emit.rs` 玩家 join + periodic 下发 `bong:spider_disguise_enter`；client `BongNetworkHandler.java:629` → handleEnter 记录 disguised id；`FaunaModel.java:74` isDisguised 时返回 `ASH_SPIDER_DISGUISE_TEXTURE`。目录下只有 `ash_spider.png`，disguised 版不存在（注释 31-32"美术按此路径交付"从未交付）→ **Disguised 是灰烬蛛常态，故拟态蛛常驻显示缺失贴图棋盘，反比正常蛛更显眼，破坏拟态机制**。修：`/gen-image` 产出实体贴图（[[feedback_item_icon_gen]] 同流程）。**资产缺失。**

## P2 — 散灵珠 ledger 僵尸账户（zhenfa 新子系统）

- **#1 minor（fix_pr）**：`server/src/zhenfa/mod.rs:2718` 散灵珠 ledger 账户**耗尽后从不 remove_balance**。`tick_scatter_bead_excretion` 两条 depleted 路径（mod.rs:2657、2713）+ `handle_scatter_bead_trigger_requests` 成功路径，最终都只 `burials.beads.remove(&id)` **从不调 `ledger.remove_balance`**。全 server 仅 `lingtian/qi_account.rs:78` 调用过 remove_balance（zone 移除清账户）——**证明清理模式存在且别处在用，zhenfa 散灵珠路径漏掉**。耗尽珠在 `WorldQiAccount.balances`（BTreeMap）留 ≤QI_EPSILON(自然耗尽) 或 0(主动触发) 的僵尸账户永不回收 → `total()`/`summarize_world_qi` 的 ledger_qi 求和它们、`build_qi_ledger_hash_fields` 每账户输出 telemetry 行。真实危害：长跑时 BTreeMap 无界增长 + telemetry 膨胀，守恒漂移在 `assert_conservation` 容差边缘累积后可能越界。修：depleted/移除路径加 `ledger.remove_balance(&source)` 对齐 lingtian 先例。**zhenfa 新代码（plan-zhenfa-content-v2），局部明确。**

## P3 — 技能图标 PNG 大面积缺失（美术 backlog）

- **#4 minor（plan_skeleton）**：server 注册 **45 个** distinct `bong:textures/gui/skill/*.png` 路径，磁盘只有 7 个（body_guangbo_ticao + woliu_burst/heart/hold/mouth/pull/vortex），**38 个缺失**。`LoadoutIconLayer.java:19` 只在 iconTexture==null||isBlank() 返回空，对**非空但文件不存在**的路径仍照常 `HudRenderCommand.texture()` 无 fallback → 渲染 missing_texture（怀疑者"优雅 fallback"辩护被证伪）。但仅当玩家 `skill_bar_bind` 绑定该招到栏位后才显示（SkillBarBindings::default 全空），`QuickBarHudPlanner.java:222` 注释自承"缺 PNG 时 MC 显示 missing_texture"——属已知美术 backlog（[[feedback_item_icon_gen]] gen-image 流程）。
- **#5 minor（plan_skeleton）**：`client/.../assets/bong-client/textures/gui/skill/` 只有 5 个 zhenmai 图标，`tuike_don.png`/`tuike_shed.png`/`tuike_transfer_taint.png` 缺失；server `known_techniques.rs:646/661/676` + `skillbar_config_emit.rs:72` 以 `bong-client:` 命名空间下发，client `LoadoutIconLayer.java:22` 同路径渲染（同 #4 无文件 fallback）。退气流派图标绑定后显示 missing_texture。与 #4 同类，仅命名空间/流派不同。修：并入 #4 批量 `/gen-image` 产出 41 个技能图标（38 + 3）。**美术资产 backlog，统一 gen-image。**

## §N 开放问题

1. #6 spirit_qi_max：仅补 PlayerState（活跃通道）vs 顺带审计其他 ServerData 变体有无同类"server 算了但没下发"字段（体力/污染/境界进度条分母等核心 HUD 字段）。
2. #2 涡流 atlas：是否顺带审计**所有** bong:particle/* 描述符都在 particles.json atlas（写个测试枚举 bong/particles/*.json 比对 atlas sources，根治 atlas-stitch 遗漏类）。
3. #3 ash_spider_disguised：gen-image 占位 vs 正式美术——拟态蛛需与正常蛛有视觉区分但又"伪装成环境"，美术 brief 待定。
4. #4/#5 技能图标：批量 gen-image 41 个（建议合并一个"技能图标资产补全" plan，按流派分批，遵 [[feedback_item_icon_gen]]）；是否同时加"server 注册的 skill icon 路径必须有对应文件"的资产存在性测试，防再漏。

## 审计来源

bug-hunt round9（workflow，5 全新角度 finder + 怀疑者对抗 + opus 逐条全树复核，11 候选）。**ROOT = fresh origin/main worktree**（方法论修正后第七轮）。已对 r1-r8 去重。**report-only**：#6 spirit_qi_max 是 player-facing HUD 错（中高境界真元条恒满），优先；#2/#3 资产/atlas 缺失 + #1 zhenfa ledger 局部明确可 fix_pr；#4/#5 技能图标是美术 backlog，建议统一 gen-image plan。**本轮转向**：资产纹理/HUD payload/zhenfa 新代码均 r1-r8 未碰，证明换全新角度仍有真问题；同时 5 条 NOT_REAL（含 dev 命令无认证=文档化设计）显示严格裁决持续有效。
