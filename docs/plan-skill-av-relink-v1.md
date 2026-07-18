# plan-skill-av-relink-v1 —— 梯队一：技能图标重链 + 孤儿动画接线（零/极少新资产）

> 一句话主题：把「资产已存在、只差接线」的两类白捡收益收掉——① `known_techniques.rs` 声明的技能栏图标与磁盘资产三重错配（命名空间/目录/文件名），重链到既有 `skill_scroll_*` 资产；② 十余个已做好却全仓无任何 `play_anim` 触发的孤儿 PlayerAnimator 动画（7 个流派站桩、`rune_draw`、`forge_hammer`、`alchemy_stir` 等）接到真实事件源。
>
> 调研来源：2026-07-17 三路并行探查（server 发射链路 / client 渲染接线 / 资产覆盖矩阵），基线 `origin/main` = `062cf636`。

## 与既有 plan 的关系（防重声明）

- **`docs/plan-bughunt-r9-skill-icons-missing-v1.md`（active）**：r9 证真了「声明 43+ distinct 图标路径、磁盘只有 15、缺 28」，其解法是 28 张全走 `/gen-image` 生成（当时 harness 跑不了 gen-image 故 BLOCKED）。本 plan 采取不同解法：**28 张缺失图标全部能映射到磁盘既有的 `bong-client:textures/gui/items/skill_scroll_*.png`（39 张）**，重链声明路径即可，零新资产。本 plan P0 落地后 r9 的 BLOCKED 清单自然闭合；r9 plan 的归档由其 owner 流程收口，本 plan 不动它（一个 PR 只动一个 plan）。r9 追加的 dugu runtime 5 缺口（磁盘确实无资产）归本 plan P2 用 `/gen-image` 补。
- **`docs/plan-bughunt-woliu-voidpath-missing-animations-v1.md`（active）**：涡流虚蚀 5 招动画缺失归该 plan，本 plan 不碰。
- **`docs/plans-skeleton/plan-module-wiring-gaps-v2.md`**：T13 是 client 渲染技术方案类决策，与本 plan 的动画事件接线无交集；已核对无重复主题。
- **`docs/plans-skeleton/reminder.md`**：`WorkbenchConstants.java:15` SFX/VFX stub 归 plan-workbench-place-runtime-v1，不在本 plan 范围。

## 接入面 checklist

- **进料**：`server/src/cultivation/known_techniques.rs:140` `TECHNIQUE_DEFINITIONS`（49 条 `icon_texture` 字段）；`client/src/main/resources/assets/bong/player_animation/*.json` 既有孤儿动画资产；各子系统既有事件源（zhenfa 施放、forge 敲击、alchemy 熬煮、movement 闪避）。
- **出料**：技能栏图标经 `server/src/network/skillbar_config_emit.rs` → `SkillBarEntryV1::Skill.icon_texture` → client `SkillBarConfigHandler` / `LoadoutIconLayer.resolveExistingSkillTexture`；动画经 `VfxEventPayloadV1::PlayAnim` → client `VfxEventRouter` → `ClientAnimationBridge` → `AnimationLayerManager`。
- **共享类型/event**：全部复用既有 `VfxEventPayloadV1::PlayAnim`（`server/src/schema/vfx_event.rs`）与 `SkillBarEntryV1`，**不新增任何 schema/payload/event**。
- **跨仓库契约**：server `known_techniques.rs` / `vfx_animation_trigger.rs` ↔ client `SkillIconIds` / `BongAnimationRegistry`；agent 层不参与（纯表现层不过 Redis IPC）。
- **worldview 锚点**：worldview.md §四 招式物理可见性（旁观者须能从姿态/视觉读招）；HUD 沉浸式极简约定（图标缺失退化成文字标签违背该约定）。
- **qi_physics 锚点**：不涉及——纯表现层，不触碰任何真元流动。

## 阶段总览

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | 技能栏图标重链（28 张映射到既有 skill_scroll 资产）+ 命名约定统一 | ⬜ |
| P1 | 孤儿动画接线（有真实事件源的批次） | ⬜ |
| P2 | 真缺图标 `/gen-image` 补齐（dugu runtime 5 + 复核后仍缺者） | ⬜ |
| P3 | 防回归：资产存在性扫描测试 + 接线 pin 测试 | ⬜ |

## P0 — 技能栏图标重链

**现状证据**：`known_techniques.rs` 绝大多数 `icon_texture` 写 `bong:textures/gui/skill/<name>.png`，该路径磁盘只有 15 张；而 `bong-client:textures/gui/items/skill_scroll_*.png` 已有 39 张覆盖同批招式。client `SkillIconIds.java` 本就统一按 `skill_scroll_<safeId>.png` 约定解析，`LoadoutIconLayer` 走 `HudTextureProbe::exists` 兜底成文字标签——所以当前技能栏大面积无图标。

**交付物**：

1. `known_techniques.rs` 中以下 28 条 `icon_texture` 重链到磁盘既有资产（映射表，实施时逐条核对文件存在）：
   - `sword.{cleave,thrust,parry,infuse}` → `bong-client:textures/gui/items/skill_scroll_sword_{cleave,thrust,parry,infuse}.png`
   - `sword_path.{condense_edge,qi_slash,resonance,manifest,heaven_gate}` → `skill_scroll_sword_path_*.png`
   - `anqi.{charge_carrier,single_snipe,multi_shot,soul_inject,armor_pierce,echo_fractal}` → `skill_scroll_anqi_*.png`
   - `burst_meridian.{beng_quan,tie_shan_kao,xue_beng_bu,ni_mai_hu_ti}` → `skill_scroll_burst_meridian_*.png`
   - `baomai.{full_power_charge,full_power_release}` → `skill_scroll_baomai_full_power_*.png`
   - `dugu.{shoot_needle,infuse_poison}` → `skill_scroll_dugu_*.png`
   - `npc.{heal_basic,buff_speed,buff_defense}` → `skill_scroll_npc_*.png`
   - `movement.dash` → `skill_scroll_movement_dash.png`；`shield_block` → `skill_scroll_shield_block.png`
2. woliu 进阶 5 招（`vacuum_palm`/`vortex_shield`/`vacuum_lock`/`vortex_resonance`/`turbulence_burst`）当前借用基础 5 招图标，重链到各自的 `skill_scroll_woliu_*.png`（磁盘已有）。
3. 命名约定写进 `known_techniques.rs` 模块注释：技能栏图标单一真相源 = `bong-client:textures/gui/items/skill_scroll_<safe_id>.png`（`safe_id` = 技能 id 的 `.`→`_`，与 `SkillIconIds.java` 既有约定一致）；`bong:textures/gui/skill/` 目录保留给非 technique 的 HUD 特化图（woliu 虚蚀、guangbo）。
4. **修订 2026-07-17：原文前提经实施核实不成立**——本条原文为「`server/src/network/quickslot_config_emit.rs:92` 的 `icon_texture: String::new()` 占位一并接真值（从 `technique_definition(id)` 取）」。实施核实：quickslot 是**纯 Item 槽**（`QuickSlotBindings` 为 `[Option<u64>; 9]`，只绑 instance_id→template_id，**无 Skill 变体**），`technique_definition(template_id)` 恒 `None`，「接真值」不可实施；且 client 对 Item 槽空串按 itemId 走 `ItemIconRegistry` 富解析（tools/ 子目录映射、armor tint、存在性探测、broken_artifact 兜底），server 回填非空 naive 模板路径会使 client 落入裸 `texture()` 分支绕过富解析，造成工具/护甲类图标回归。修正后契约：**Item 型槽（quickslot 全部槽位 + skillbar Item 槽）`icon_texture` 恒空串 = 显式契约**，空串即「由 client 按 itemId 走 `ItemIconRegistry` 富解析」的信号。依据锚点：`server/src/network/quickslot_config_emit.rs` 模块注释；client `QuickBarHudPlanner`（Item 槽空串 → `itemTexture(itemId)` 富解析分支，非空 → 裸 `texture()` 分支）。

## P1 — 孤儿动画接线

**现状证据**：以下动画 JSON 存在于 `assets/bong/player_animation/` 且在 `BongAnimations.java:93-99` 定义了常量，但全仓无任何 `play_anim` 发射（client 探查 2026-07-17 证真）。

**交付物**（每条 = server 侧一个 emit 接线 + client 侧无需改动）：

| 孤儿动画 | 事件源（2026-07-17 实施逐条核实后的定案） | 接线点（实施落地） |
|---|---|---|
| `stance_{woliu,zhenmai}` ×2 | ✅ 已接：`TechniqueLearnedEvent`（激活功法时刻——`learn_technique_if_allowed` 习得即写 `active:true`）。核实结论：全仓无「流派架势切换」gameplay 事件，`stance_switch` audio recipe 唯一发射点是 `SkillXpGain` 经验反馈（`audio_trigger.rs`）、与架势无关，按 §8 #2 决议改接激活功法时刻。**liveness 澄清（2026-07-18 对抗审查修正）**：该事件的生产发射路仅两条——卷轴习得（`client_request_handler`）与首击领悟（`first_hit_dash`，仅授 `movement.dash` 无架势映射）；`technique_mentor::mentor_teaches_technique` 是**无生产调用方的休眠 helper**（仅测试调用，`social` 模块只写 technique_hint 关系元数据不传功），「导师传授」不是活路。现有内容卷轴仅授 `woliu.*`×11 + `zhenmai.parry`，故生产可达 = woliu / zhenmai 两族 | `vfx_animation_trigger::emit_technique_learned_stance_triggers`（technique_id 前缀映射，无映射前缀不发） |
| `stance_{dugu,dugu_poison,baomai,tuike}` ×4 | ⛔ report-only（**2026-07-18 review 返工降级**：首版曾作「接口先锁定的潜伏接线」计入 P1 交付，4 位 reviewer 一致判定违反 P1「有真实事件源的批次」验收边界——直接构造 `TechniqueLearnedEvent` 的 emit pin 只能证明映射函数可执行，不能证明生产接线可达）：现有内容无任何生产路径能产生这 4 族的 `TechniqueLearnedEvent`（卷轴只授 `woliu.*`/`zhenmai.parry`，mentor 无生产调用方），按「不硬造事件」断链原则降级，映射分支/`P1_WIRED_ANIM_IDS`/共享清单均不预置。等对应流派习得内容（卷轴/导师传功生产化）落地时连映射 + 清单 + 测试一起接入 | —— |
| `stance_zhenfa` | ⛔ report-only：事件源缺失——`TECHNIQUE_DEFINITIONS` 无 `zhenfa.*` 条目，`TechniqueLearnedEvent` 驱动不到；流派入门实为 `ArrayMastery.add_cast`（落阵时刻），但该时刻已归 `rune_draw`，同点双动画打架。等 zhenfa 功法条目或独立架势事件落地 | —— |
| `rune_draw` | ✅ 已接：zhenfa 落阵成功（`handle_zhenfa_place_requests` registry.insert Ok 分支，覆盖全部 ZhenfaKind——deploy 事件仅覆盖 4 kind + 组网成型，普通陷阱无事件，走 adapter 会漏，故内联） | `server/src/zhenfa/mod.rs` 落阵成功分支内联 emit |
| `forge_hammer` | ✅ 已接：`TemperingHit`（J/K/L 淬炼按键，与 `handle_tempering_hits` 的 FORGE_HAMMER_STRIKE 粒子同源；镜像 `ForgeStep::Tempering` 步骤门） | `vfx_animation_trigger::emit_forge_tempering_animation_triggers` |
| `alchemy_stir` | ✅ 已接：炼丹干预请求（`handle_alchemy_intervention`，与 ALCHEMY_BREW_VAPOR/OVERHEAT 粒子同点；干预无 bevy 事件可订阅，故内联） | `server/src/network/client_request_handler.rs` 干预生效分支内联 emit |
| `enlightenment_pose` | ✅ 已接（超出原 report-only 预期——逐个证实发现真实事件源）：`InsightChosen` 经 `apply_insight_chosen` 三重校验（pending 对齐/choice 合法/arbiter 配额）通过后 `apply_choice` 生效时刻；校验前 emit 会在 stale/无效/被拒抉择上误播，故置于校验通过分支 | `server/src/cultivation/insight_flow.rs` `apply_insight_chosen` 内联 emit |
| `dodge_back` / `dodge_roll` | ⛔ report-only：事件源缺失——`MovementAction` 仅 `None\|Dashing` 两变体、请求映射仅 Dash、dash 方向恒取面朝向（无后撤变体）；`combat/` 无任何 dodge/evade 机制；dash iframe 语义也属 `dash_forward` 已接线的同一动作。等后撤/翻滚 gameplay 落地 | —— |
| `fist_punch_left` | ✅ 已接：空手连击左右交替（`AttackIntent` 空手恒 Blunt→fist 分支；right 起手、right→left 交替、超时复位 right、持械不参与交替恒 right、按 Entity 键玩家隔离） | `vfx_animation_trigger::emit_attack_animation_triggers` 交替态改造 |

**处置原则**：找不到真实事件源的孤儿**不硬造事件**——为接而接等于制造反向孤岛。2026-07-17 逐个核实结论（`enlightenment_pose` 原列此清单，证实有真实事件源 `InsightChosen`，已升级接线、见上表）：

- `sword_ride`：⛔ report-only——server 无御剑飞行系统（`levitat|sword_flight|riding` 全仓零命中），movement 仅 dash 一种主动位移。事件源缺失，等御剑 gameplay 落地。
- `levitate`：⛔ report-only——无悬浮状态机（同上检索零命中，cultivation/movement 均无悬浮/滞空状态）。事件源缺失，等对应 gameplay 落地。
- `bow_salute`：⛔ report-only——`social/` 无行礼/greet 交互事件（`salute|bow|greet` 于 social/ 零命中）。事件源缺失，等社交礼仪交互落地。
- `stealth_crouch`：⛔ report-only——server 无玩家潜行状态（无 sneak C2S、无潜行状态机组件）；vanilla 潜行姿态 client 本地已渲染，server 强推同名动画反与本地姿态叠加冲突。事件源缺失，等潜行 gameplay 落地。
- `cultivate_stand`：⛔ report-only——无站桩修炼状态机（修炼是被动 tick 累积，`is_recently_practicing` 是回看式记账非状态进入事件；`CultivationSessionPracticeEvent` 按分钟记账且语义为打坐，接它会每分钟重播且与 `meditate_sit` 语义冲突）。事件源缺失，等站桩/打坐区分的修炼状态机落地。

## P2 — 真缺图标补齐

- r9 追加的 dugu runtime 5 张走 `/gen-image item` 批量生成（`scripts/images/gen.py`）。**消费方澄清**：这 5 张**不是** `TECHNIQUE_DEFINITIONS` 条目，消费链是 `dugu_v2` runtime visual payload（server 下发招式提示图标路径 → client `HudTextureProbe::exists` 探测后由事件 UI 加载），因此不落入 P3 的 technique 快照覆盖。处置：命名仍按 P0 约定收编——生成为 `bong-client:textures/gui/items/skill_scroll_dugu_{eclipse,penetrate,reverse,self_cure,shroud}.png`，同步把 `dugu_v2` payload 中旧引用路径（`bong:textures/gui/skill/dugu_*.png`）重链到新命名（不给旧命名空间留新增量）；并在本批为这 5 条 runtime 引用路径**单独加存在性 pin 测试**（server 侧断言 payload 引用路径 == 磁盘真实资产），保证生成后有真实加载路径、不再漂移。生成后程序化全量扫透明度揪假透明（--transparent ~10% 白底失败率）。
- P0 映射逐条核对后若仍有缺口（如 `morph_yixing` 磁盘完全无文件），一并入本批生成，同样按 `skill_scroll_*` 约定命名。
- 图标资产变更同步 `resourcepack.rs` + committed manifest 的 sha1/size（否则 Build resource pack CI 红）。

## P3 — 防回归测试

**图标链（server 发射 → 快照 → client 资源，三级都锁）**：

- **发射契约测试**（server 侧）：遍历全部 technique，断言 `skillbar_config_emit` 发出的 Skill 槽 `icon_texture` **严格等于** `technique_definition(id).icon_texture` 且非空；Item 型槽（skillbar Item 槽 + quickslot 全部槽位）`icon_texture` **恒空串 pin**（P0 #4 修订后的显式契约：空串 = client 按 itemId 走 `ItemIconRegistry` 富解析，server 回填路径判红）；覆盖未知 technique id、解析不到的 Item instance、无 bindings/inventory 等错误/兜底分支。
- **快照同步测试**（server 侧）：icon 路径快照由 `TECHNIQUE_DEFINITIONS` **单向生成**（checked-in 快照 + server 测试断言快照与定义的 `skill_id → icon_texture` 集合完全一致、无多无少，或构建期直接导出——机制与 `plan-skill-anim-fidelity-v1` §8 #4 的 cast_ticks 快照拍同一方案），快照不可手改，不形成第二真相源。
- **资产存在性扫描测试**（client 侧）：消费同一份快照，断言每条 icon 路径在 classpath 资源里真实存在——新增招式漏配图标立刻撞红。**缺资产 allowlist 棘轮**（与 `plan-skill-anim-fidelity-v1` P0 对拍测试同款机制）：PR-1 合入时已知缺失、被推迟到 P2 生成的条目进 allowlist（当前全量核查后仅 `morph.yixing` 一条——它是 `TECHNIQUE_DEFINITIONS` 条目但全仓无图标资产），PR-3 生成后清零；allowlist 只允许缩小，新增条目必须在 PR body 显式说明理由。
- **映射约束测试**：空串/坏命名空间（非 `bong:`/`bong-client:` 前缀）判红；两招指向同一文件视为重复映射判红，显式声明共用的白名单除外（woliu 借用图标在 P0 后应清零）。

**动画链（server 发射 → client 加载 → 路由消费，由同一份清单驱动双端）**：

- **共享 anim_id 清单**：P1 全部接线的 anim_id 落成一份清单（与接线表同源），server / client 测试都从它驱动，防两端各自漏项。
- **接线 pin 测试**（server 侧）：P1 每条新 emit 各配单测，饱和覆盖：事件触发发出携正确 `anim_id` 的 `PlayAnim`（happy path）、事件前置不满足时不发（错误分支）、同一事件重复触发的幂等语义、实体死亡/离线等状态转换下不发。
- **client 消费闭环测试**：清单逐项断言 `BongAnimationRegistry.contains(anim_id)`（参数化资源 pin）；至少一条端到端路由契约测试——构造 `PlayAnim` payload 经真实 `VfxEventRouter` 入口走到 `ClientAnimationBridge`，断言可解析到 `AnimationLayerManager`（非 bridge miss）；未知 anim_id 的失败分支（bridge miss 记录、不崩溃）。
- **连击交替序列测试**（`fist_punch_left` 接线专属）：连续空手攻击产生 right→left→right 交替；连击超时/中断后从规定初始侧复位；持武器分支不参与交替；玩家间状态隔离。

**兜底行为保留**：r9 已加的 `QuickBarHudPlannerTest` 文字兜底测试不动（icon 全配齐后兜底仍是合法防线）。

## §8 开放问题（P0 决策门前需收口）

1. **图标真相源方向**：推荐「全部统一到 `bong-client:.../items/skill_scroll_*`」（39 张现成、`SkillIconIds` 已按此约定、零资产搬运）；备选是反向把 39 张复制/重命名到 `bong:textures/gui/skill/`（r9 声明路径）。二选一后写死约定。
2. **`stance_*` 触发源核实**：「流派架势切换」的 server 事件是否真实存在（`stance_switch` audio recipe 的发射点在哪）；若只有音效 recipe 而无 gameplay 事件，则改接「`/technique active` 激活功法」时刻，或降级 report-only。
3. **`dodge_back`/`dodge_roll` 场景边界**：与 `movement.dash` 现有 `dash_forward` 的分工（后撤 vs 翻滚 vs 冲刺），需读 movement/combat 代码定边界，防止一个事件双动画打架。

> §8 原表保留作决策背景；#1 已在 §8.1 收口、实施以 §8.1 为准，#2/#3 归 PR-2（P1 实施）设计收口时决议。

### §8.1 决议（2026-07-17）

#### #1 图标真相源方向 —— 已收口：统一 `bong-client:textures/gui/items/skill_scroll_<safe_id>.png`

**决议**：
1. 采用推荐路线：technique 图标单一真相源 = `bong-client:textures/gui/items/skill_scroll_<safe_id>.png`（`safe_id` = 技能 id 的 `.`/`:`/`/` → `_`），39 张现成资产零搬运；client 对 `bong-client:` 前缀**原生支持、零改动**——`Identifier.tryParse` 接受任意合法命名空间，资源查找对 `bong-client` 与 `bong` 一视同仁。
2. 例外清单（既有专属图不重链，逐条锁进 P3 例外映射表）：woliu 基础六式 + `body.guangbo_ticao` 留 `bong:textures/gui/skill/`；zhenmai 五式留 `bong-client:textures/gui/skill/`；`morph.yixing` 全仓无资产、现值悬空，归 P2 `/gen-image` 生成后按约定收编（client 侧 allowlist 棘轮同步记录）。
3. 拒绝备选路线（把 39 张复制/重命名到 `bong:textures/gui/skill/`）：徒增资产搬运与双份文件漂移风险，且与 `SkillIconIds` 既有 client 端解析约定相逆。

**落点**：`server/src/cultivation/known_techniques.rs` 模块注释（约定 + 例外清单正文）；client `HudTextureProbe.java`（`Identifier.tryParse` 对 `bong-client:` 原生解析）、`LoadoutIconLayer.java` `resolveExistingSkillTexture`（服务端下发路径优先 + `skill_scroll_<safe_id>` 候选兜底）、`SkillIconIds.java`（`scrollTexturePath` 同一约定的 client 端拼法）。

#### #2 `stance_*` 触发源核实 —— PR-2（P1 实施）设计收口时决议，本 PR 不预判

**决议**：显式记录为 PR-2 前置收口项。「流派架势切换」server 事件是否真实存在（`stance_switch` audio recipe 的发射点）须在 P1 实施前按 §8 原文列出的选项核实定案（真实事件 / 改接 `/technique active` 时刻 / 降级 report-only），PR-1（P0+P3 图标链）不预判、不接线。

#### #3 `dodge_back`/`dodge_roll` 场景边界 —— PR-2（P1 实施）设计收口时决议，本 PR 不预判

**决议**：同 #2，显式记录为 PR-2 前置收口项。与 `movement.dash` 现有 `dash_forward` 的分工（后撤 vs 翻滚 vs 冲刺）须在 P1 实施前读 movement/combat 代码定边界后定案，PR-1 不预判。

## 测试声明

- server：图标发射契约（skillbar+quickslot，含未知 id/非技能槽分支）+ 快照单向同步 + P1 各接线 emit 单测（happy path / 前置不满足 / 重复触发幂等 / 实体状态转换）+ 连击交替序列（cargo test）；
- client：快照消费的资产存在性 + 映射约束（空路径/坏命名空间/重复映射）扫描 + anim_id 参数化 registry pin + `PlayAnim` 经 `VfxEventRouter`→`ClientAnimationBridge` 路由契约（含未知 id 失败分支）+ 既有 `QuickBarHudPlannerTest` 回归（gradlew test）；
- e2e：`bash scripts/smoke-test-e2e.sh` 绿；图标资产变更后 Build resource pack CI 绿（sha1 同步）。

## §10 实施工作流

- 单 plan 多 PR 序列化（**测试随对应实现同批交付，不预置无实现对象的测试**）：PR-1 = P0 图标重链 + P3 图标链测试（发射契约/快照同步/存在性/映射约束）；PR-2 = P1 孤儿动画接线（§8 #2/#3 收口后）+ P3 动画链测试（emit pin/registry pin/路由契约/连击序列）；PR-3 = P2 `/gen-image` 图标补齐 + runtime 引用 pin + resourcepack sha1 同步 + 存在性 allowlist 清零。
- 每 PR 独立实施 subagent（context 隔离）；CodeRabbit / `/review` 等待走 ScheduleWakeup 1200s 协议，修完意见重等 re-review。
- **单次 consume-plan 全自动到 merge**：用户提交 `/consume-plan` 后全自动走完实施→review→merge→归档至 `docs/finished_plans/`，无需人工值守；P2 生成的图标 PNG 附 PR body 供人工抽查。
