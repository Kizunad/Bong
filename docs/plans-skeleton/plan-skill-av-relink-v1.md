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
4. `server/src/network/quickslot_config_emit.rs:92` 的 `icon_texture: String::new()` 占位一并接真值（从 `technique_definition(id)` 取）。

## P1 — 孤儿动画接线

**现状证据**：以下动画 JSON 存在于 `assets/bong/player_animation/` 且在 `BongAnimations.java:93-99` 定义了常量，但全仓无任何 `play_anim` 发射（client 探查 2026-07-17 证真）。

**交付物**（每条 = server 侧一个 emit 接线 + client 侧无需改动）：

| 孤儿动画 | 事件源（实施前逐条核实存在性，见 §8 #2/#3） | 接线点 |
|---|---|---|
| `stance_{woliu,dugu,dugu_poison,baomai,zhenfa,zhenmai,tuike}` ×7 | 流派架势切换（`stance_switch` audio recipe 已存在，找到其触发事件同点发 PlayAnim） | `server/src/network/vfx_animation_trigger.rs` |
| `rune_draw` | zhenfa 施放（已有粒子 `ZhenfaActionVfxPlayer` + 音效，独缺动画） | zhenfa cast 事件的 emit system |
| `forge_hammer` | 锻造敲击（`ForgeHammerStrikePlayer` 粒子已接的同一事件） | forge 敲击 emit |
| `alchemy_stir` | 炼丹熬煮（`AlchemyBrewVaporPlayer` 同源事件） | alchemy 熬煮 emit |
| `dodge_back` / `dodge_roll` | 受击位移/闪避事件（与 `movement.dash` 的 `dash_forward` 区分场景） | movement/combat emit |
| `fist_punch_left` | 空手连击左右交替（现只用 `fist_punch_right`） | 攻击动画选择逻辑 |

**处置原则**：找不到真实事件源的孤儿（`sword_ride`/`levitate`/`bow_salute`/`stealth_crouch`/`enlightenment_pose`/`cultivate_stand`）**不硬造事件**——为接而接等于制造反向孤岛。逐个在 plan 内记录「事件源缺失，等对应 gameplay 落地」的 report-only 结论。

## P2 — 真缺图标补齐

- r9 追加的 dugu runtime 5 张走 `/gen-image item` 批量生成（`scripts/images/gen.py`）。**消费方澄清**：这 5 张**不是** `TECHNIQUE_DEFINITIONS` 条目，消费链是 `dugu_v2` runtime visual payload（server 下发招式提示图标路径 → client `HudTextureProbe::exists` 探测后由事件 UI 加载），因此不落入 P3 的 technique 快照覆盖。处置：命名仍按 P0 约定收编——生成为 `bong-client:textures/gui/items/skill_scroll_dugu_{eclipse,penetrate,reverse,self_cure,shroud}.png`，同步把 `dugu_v2` payload 中旧引用路径（`bong:textures/gui/skill/dugu_*.png`）重链到新命名（不给旧命名空间留新增量）；并在本批为这 5 条 runtime 引用路径**单独加存在性 pin 测试**（server 侧断言 payload 引用路径 == 磁盘真实资产），保证生成后有真实加载路径、不再漂移。生成后程序化全量扫透明度揪假透明（--transparent ~10% 白底失败率）。
- P0 映射逐条核对后若仍有缺口（如 `morph_yixing` 磁盘完全无文件），一并入本批生成，同样按 `skill_scroll_*` 约定命名。
- 图标资产变更同步 `resourcepack.rs` + committed manifest 的 sha1/size（否则 Build resource pack CI 红）。

## P3 — 防回归测试

**图标链（server 发射 → 快照 → client 资源，三级都锁）**：

- **发射契约测试**（server 侧）：遍历全部 technique，断言 `skillbar_config_emit` 与 `quickslot_config_emit` 发出的 `icon_texture` **严格等于** `technique_definition(id).icon_texture` 且非空；覆盖未知 technique id、非技能槽位（Item 型槽）的错误/兜底分支——quickslot 接真值后 `String::new()` 不再合法。
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

## 测试声明

- server：图标发射契约（skillbar+quickslot，含未知 id/非技能槽分支）+ 快照单向同步 + P1 各接线 emit 单测（happy path / 前置不满足 / 重复触发幂等 / 实体状态转换）+ 连击交替序列（cargo test）；
- client：快照消费的资产存在性 + 映射约束（空路径/坏命名空间/重复映射）扫描 + anim_id 参数化 registry pin + `PlayAnim` 经 `VfxEventRouter`→`ClientAnimationBridge` 路由契约（含未知 id 失败分支）+ 既有 `QuickBarHudPlannerTest` 回归（gradlew test）；
- e2e：`bash scripts/smoke-test-e2e.sh` 绿；图标资产变更后 Build resource pack CI 绿（sha1 同步）。

## §10 实施工作流

- 单 plan 多 PR 序列化（**测试随对应实现同批交付，不预置无实现对象的测试**）：PR-1 = P0 图标重链 + P3 图标链测试（发射契约/快照同步/存在性/映射约束）；PR-2 = P1 孤儿动画接线（§8 #2/#3 收口后）+ P3 动画链测试（emit pin/registry pin/路由契约/连击序列）；PR-3 = P2 `/gen-image` 图标补齐 + runtime 引用 pin + resourcepack sha1 同步 + 存在性 allowlist 清零。
- 每 PR 独立实施 subagent（context 隔离）；CodeRabbit / `/review` 等待走 ScheduleWakeup 1200s 协议，修完意见重等 re-review。
- **单次 consume-plan 全自动到 merge**：用户提交 `/consume-plan` 后全自动走完实施→review→merge→归档至 `docs/finished_plans/`，无需人工值守；P2 生成的图标 PNG 附 PR body 供人工抽查。
