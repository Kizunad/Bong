# BugHunt Skeleton Plan - Dugu v2 五招缺 official technique 定义

一句话：`dugu.eclipse/self_cure/penetrate/shroud/reverse` 已注册到 `SkillRegistry` 并被 finished plan 当作毒蛊 v2 完整招式包，但没有进入 `TECHNIQUE_IDS/TECHNIQUE_DEFINITIONS`，导致正式学习、技能快照、技能栏绑定与玩家技能栏施放路径不可达。

状态：Skeleton。仅记录 bug 与修复计划骨架，不包含实际修复。

## 接入面

- 进料：毒蛊 v2 五招 `combat::dugu_v2::skills` resolver、`SkillRegistry` 注册、finished `plan-dugu-v2` 的五招完整包契约。
- 出料：玩家 `KnownTechniques`、技能快照 `TechniquesSnapshotV1`、技能栏绑定 / 施放 C2S、NPC technique 分配与选技。
- 共享类型 / event：`TechniqueDefinition`、`KnownTechniques`、`SkillBarBindings`、`SkillRegistry`、`EclipseNeedleEvent`、`PenetrateChainEvent`、`ReverseTriggeredEvent`。
- 跨仓库契约：server official technique 定义 -> server_data techniques snapshot -> client 功法面板 / 技能栏绑定；残卷 `technique_scroll.skill_id` -> server 学习入口。
- qi_physics 锚点：本 plan 不改变 Dugu v2 真元消耗、目标扣减或 zone 回流公式。修复时若只补 official technique 定义与入口测试，不得重写 `combat::dugu_v2` 既有 `release_cast_cost_to_zone`、`penetrate_zone_credit_tick`、`reverse_victim_qi_zone_credit_tick` 等守恒路径。

## 客户端 A/V/SFX/HUD/icon 验收约束

Skill / combat plan 不允许只接 server resolver 或 schema enum；五招成为 official technique 后，玩家必须能从动画、粒子、音效、HUD 与技能栏图标上区分每招。现有 `combat::dugu_v2::skills::visual_for()` 已声明的逐招 ID 必须被 official definition、schema snapshot 与 client skillbar/HUD 消费链保持一致：

| skill_id | animation | particle / VFX | SFX | HUD 反馈 | SkillBar icon |
|---|---|---|---|---|---|
| `dugu.eclipse` | `bong:dugu_needle_throw` | `bong:dugu_taint_pulse` | `dugu_needle_hiss` | `蚀针` | `bong:textures/gui/skill/dugu_eclipse.png` |
| `dugu.self_cure` | `bong:dugu_self_cure_pose` | `bong:dugu_dark_green_mist` | `dugu_self_cure_drink` | `自蕴` | `bong:textures/gui/skill/dugu_self_cure.png` |
| `dugu.penetrate` | `bong:dugu_needle_throw` | `bong:dugu_taint_pulse` | `dugu_needle_hiss` | `侵染` | `bong:textures/gui/skill/dugu_penetrate.png` |
| `dugu.shroud` | `bong:dugu_shroud_activate` | `bong:dugu_dark_green_mist` | `dugu_self_cure_drink` | `神识遮蔽` | `bong:textures/gui/skill/dugu_shroud.png` |
| `dugu.reverse` | `bong:dugu_pointing_curse` | `bong:dugu_reverse_burst` | `dugu_curse_cackle` | `倒蚀` | `bong:textures/gui/skill/dugu_reverse.png` |

截至本 plan 审查，仓库中已有三张 Dugu v2 particle PNG，但未找到五张 SkillBar icon PNG。Codex 执行实现时不能跑 `/gen-image`；若接通 official technique 后这些 icon 仍不存在，必须完成 server/schema/client 查图接线和占位引用，并在对应 TODO 标注 `[BLOCKED: 需 /gen-image 生成 dugu_eclipse.png, dugu_self_cure.png, dugu_penetrate.png, dugu_shroud.png, dugu_reverse.png]`，不能用手绘临时图糊弄验收。

## 实际游玩体验影响

1. 玩家无法通过正式残卷 / 导师 / 学习入口学到毒蛊 v2 五招：`learn_technique_if_allowed()` 对缺少 definition 的 skill_id 直接返回 `InvalidScroll`。
2. 即使测试或旧存档把 `dugu.eclipse` 等 ID 塞进 `KnownTechniques`，server 下发技能快照时会按 `TECHNIQUE_DEFINITIONS` 过滤，客户端功法面板看不到这些招式的名称、描述、消耗、冷却和范围。
3. 玩家无法把五招绑定到 1-9 技能栏：`skill_bar_bind` 先查 `technique_definition(skill_id)`，未知 skill 被拒绝。
4. 即使某个外部路径绕过绑定并留下 `SkillBarBindings`，`skill_bar_cast` 仍先查 definition，未知 skill 会在进入 Dugu v2 resolver 前被丢弃。
5. 这让已投入的 Dugu v2 守恒修复、HUD/S2C、VFX/SFX、agent narration 只剩测试或事件链可达，普通玩家无法体验“蚀针 / 自蕴 / 侵染 / 神识遮蔽 / 倒蚀”五招。

边界：不主张 `combat::dugu_v2` resolver 或事件桥不存在；问题是 official technique 层漏接，导致玩家正式学习 / 快照 / 绑定 / 技能栏施放不可达。

## 复现路径

### 残卷 / 学习路径

1. 新增或加载一个 `technique_scroll.skill_id = "dugu.eclipse"` 的物品模板。
2. 观察 item registry 解析在 `parse_technique_scroll_spec()` 阶段拒绝 unknown technique，合法残卷无法进入生产 registry。
3. 或直接调用 `learn_technique_if_allowed(..., "dugu.eclipse", ...)`，返回 `InvalidScroll`，不会写入 `KnownTechniques`。

### 技能栏路径

1. 通过测试夹具或旧存档构造 `KnownTechniques { id: "dugu.eclipse", active: true }`。
2. 玩家上线触发 `TechniquesSnapshotV1`。
3. 观察 snapshot 过滤掉该 entry，客户端没有 Dugu v2 招式可展示。
4. 手动发送 `skill_bar_bind` 绑定 `dugu.eclipse`，server 因 unknown skill 拒绝。
5. 即使强行构造绑定后发送 `skill_bar_cast`，server 仍在 `technique_definition()` 处拒绝，不会调用 Dugu v2 resolver。

### NPC 路径

1. 生成 Rogue / Disciple / GuardianRelic 等会分配功法的 NPC。
2. `assign_npc_techniques()` 只从 `TECHNIQUE_DEFINITIONS` 收集候选，Dugu v2 五招不会进入 NPC `KnownTechniques`。
3. 若测试手动塞入未知 Dugu v2 entry，`select_technique()` 会因 `technique_definition(&entry.id)` 失败跳过。

## 根因证据

- `server/src/combat/dugu_v2/skills.rs:39-56`：定义并注册 `dugu.eclipse`、`dugu.self_cure`、`dugu.penetrate`、`dugu.shroud`、`dugu.reverse` 五招。
- `server/src/cultivation/skill_registry.rs:98-115`：生产 `init_registry()` 调用 `crate::combat::dugu_v2::register_skills`，说明 resolver 已进入全局 registry。
- `server/src/cultivation/known_techniques.rs:39-88`：`TECHNIQUE_IDS` 只包含 `dugu.shoot_needle`、`dugu.infuse_poison`，没有 Dugu v2 五招。
- `server/src/cultivation/known_techniques.rs:604-633`：Dugu official technique definitions 只覆盖 v1 两招。
- `server/src/cultivation/known_techniques.rs:1014-1018`：`technique_definition()` 只查 `TECHNIQUE_DEFINITIONS`。
- `server/src/cultivation/technique_scroll.rs:77-110`：学习入口对缺少 definition 的 skill_id 返回 `InvalidScroll`。
- `server/src/inventory/mod.rs:2723-2750`：物品模板中的 `technique_scroll.skill_id` 若不在 `technique_definition()`，item registry 解析失败，无法合法做 Dugu v2 残卷。
- `server/src/network/techniques_snapshot_emit.rs:40-72`：技能快照按 `TECHNIQUE_DEFINITIONS` `filter_map`，未知 `KnownTechnique` 不会下发给客户端。
- `server/src/network/client_request_handler.rs:10887-10906`：技能栏绑定先查 `technique_definition(skill_id)`，未知 skill 直接拒绝。
- `server/src/network/client_request_handler.rs:10122-10140`：技能栏施放先查 `technique_definition(&skill_id)` 和 `KnownTechniques`，未知 skill 在 resolver 前被丢弃。
- `server/src/npc/technique.rs:179-186`：NPC 功法分配只从 `TECHNIQUE_DEFINITIONS` 收集候选。
- `server/src/npc/technique.rs:399-405`：NPC 选技时 unknown entry 直接跳过；`server/src/npc/technique.rs:928-938` 才查 `SkillRegistry`。
- `docs/finished_plans/plan-dugu-v2.md:286-298`：finished plan 明确声明五招 SkillRegistry 注册与通用 cast 经脉门。
- `docs/finished_plans/plan-dugu-v2.md:441-442`：归档 evidence 把 P0/P1 server 五招、HUD/AV/事件链列为已落地。

## 去重记录

- 已避开 server-gameplay 既有项：#1048 灵木满包吞产物、#1055 锻炉远程落砧、#1060 物资棺跨维会话、#1065 武器修复绕过工作站、#1073 制作台跨维打开、#1088 延寿棺跨维裸坐标、#1095 炼丹投料鲜度、#1101 阵法布置空间门禁、#1106 夺舍空间门禁、#1114 垂死大能给丹门禁、#1123 暗器投掷失败丢封元。
- 已避开 qi 类 #1107/#1122；本 bug 不涉及新增或修正真元账本流。
- #693 / #698 只修 Dugu v2 施法成本和目标侧真元守恒；没有补 official technique 定义。
- #843 只修 Dugu v2 粒子 / AV 落地；没有补学习、快照、绑定或施放入口。
- #997 / #1002 是 Dugu v2 HUD / AV 下游体验问题，假设五招能被玩家释放；本 plan 是上游 official technique reachability 缺口。
- `docs/plans-skeleton/plan-bughunt-dugu-penetrate-av-mismatch.md`、`docs/plans-skeleton/plan-bughunt-dugu-v2-hud-skill-hint.md`、`docs/plans-skeleton/plan-bughunt-dugu-v2-hud-disconnect-bleed-v1.md` 均在 skeleton 目录，按仓库规则不可视为 active/finished 覆盖；且主题分别是 AV、HUD hint、HUD session reset，不是 official technique 定义缺失。
- `gh pr list` 搜索 `Dugu v2 uncastable / dugu_v2 skillbar / dugu.eclipse definition / TECHNIQUE_DEFINITIONS dugu / 毒蛊 v2 技能栏 / 毒蛊 v2 学习 / 蚀针 技能栏` 无同题结果。

## 修复计划骨架

- [ ] P0：确认 Dugu v2 五招是否应作为玩家 official technique。若是，补齐 `TECHNIQUE_IDS` 与 `TECHNIQUE_DEFINITIONS` 五条定义；若不是，必须显式标记为 internal/NPC-only 并清理 finished plan 与下游 HUD/AV 假设。
- [ ] P1：为五招定义 display name、grade、description、required realm、required meridians、qi/stamina cost、cast/cooldown、range、icon、category，确保与 `combat::dugu_v2::physics::skill_spec()`、肝经依赖和 finished plan 语义一致。
- [ ] P2：接通学习来源。补残卷 / 导师 / 掉落或其他权威来源，并保证 `parse_technique_scroll_spec()`、`read_combat_technique_scroll()`、`learn_technique_if_allowed()` 可学习这些 ID。
- [ ] P3：补技能栏回归。`skill_bar_bind` 和 `skill_bar_cast` 对已学 Dugu v2 五招进入对应 resolver；未学、inactive、境界不足、经脉不满足、真元不足、冷却中仍按现有拒绝语义返回。
- [ ] P4：补 NPC / bot 覆盖。若 NPC 应可使用毒蛊 v2，确认 `assign_npc_techniques()` 能按 realm/meridian 选择；若只玩家可用，补明确过滤和测试。
- [ ] P5：补 client 可观察性。`TechniquesSnapshotV1` 下发五招 definition，客户端功法面板、技能栏 icon、HUD hint 与既有 Dugu v2 AV/HUD 链路一致；逐招验收表见上方 A/V/SFX/HUD/icon 约束。

## 验证计划

- server 单测：`technique_definition("dugu.eclipse")`、`dugu.self_cure`、`dugu.penetrate`、`dugu.shroud`、`dugu.reverse` 全部存在，且 `TECHNIQUE_IDS`、`TECHNIQUE_DEFINITIONS`、`KnownTechniques::dev_default()` 同步。
- server 单测：五招残卷模板可解析；`learn_technique_if_allowed()` happy path 写入 `KnownTechniques`，境界 / 经脉 / 重复学习 / unknown ID 负例保持红线。
- server 单测：`send_techniques_snapshot_to_client()` 不过滤 Dugu v2 已学 entry，payload 含 display name、cooldown、range、category。
- server 单测：`skill_bar_bind` 对已学 Dugu v2 五招成功，对未学 / inactive 拒绝；`skill_bar_cast` happy path 进入 resolver 并产生对应 event。
- server 单测：NPC 路径按产品决议 pin 住：可用则能分配 / 选中 / lookup；不可用则有明确排除测试，不再靠缺 definition 偶然不可达。
- client / 资源回归：五招 animation、particle/VFX、SFX、HUD hint 与 SkillBar icon 均逐招可辨；若缺 icon PNG，按上方规则标 `[BLOCKED: 需 /gen-image]` 而不是宣称完成。
- 回归：跑 `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`；涉及 client snapshot/icon 时再跑 `cd client && ./gradlew test build`。

## 对抗复核

两轮 adversarial subagent 复核已完成。

- Round 1 反方最强反驳：仓库允许 `SkillRegistry` 注册项不一定都是玩家 official technique；注册 resolver 但不进 `TECHNIQUE_DEFINITIONS` 可能代表 internal/NPC/skeleton/未来入口。
- 主回应：本候选不主张 resolver 完全不可达，只主张玩家正式学习、快照、绑定、技能栏施放不可达；finished plan 与 #693/#843 等后续修复均把 Dugu v2 当常规完整招式包维护。
- Round 2 反方继续审查：未找到生产 NPC/AI/剧情入口能绕过 `TECHNIQUE_DEFINITIONS` 主动施放 Dugu v2 五招；已有 Dugu skeleton 都是 HUD/AV 下游问题，不覆盖 official technique reachability。
- 最终裁决：可提交，建议 major。结论收窄为“Dugu v2 五招 official technique 接入缺失，导致正式学习、技能快照、技能栏绑定与玩家技能栏施放路径不可达”。
