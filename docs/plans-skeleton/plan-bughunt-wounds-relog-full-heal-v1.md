# BugHunt: Wounds 不持久化——战斗中断线重连满血清创 + NearDeath 秒退免费逃脱

## Bug 摘要

**严重度：high（战斗诚实性 + 死亡后果链双破坏；opus 对抗验证衍生发现，主循环亲验锚点）**

`Wounds`（血量 + 伤口条目，`server/src/combat/components.rs:89-103`）从落地起就没有任何持久化路径：`attach_combat_bundle_to_joined_clients`（`server/src/combat/mod.rs:91-117`，filter `(Added<Client>, Without<Wounds>)`）对每次加入（含重连）的玩家无条件 `insert(Wounds::default())`——即 `health_current = health_max = DEFAULT_HEALTH_MAX`、`entries` 清空；而 `despawn_disconnected_clients` / `flush_connected_players_on_shutdown`（`server/src/player/mod.rs`）从不读取或落盘 `Wounds`。在 `origin/main` `662609339` 上实测：`grep Wounds\|health_current` 于 `server/src/player/state.rs`、`server/src/persistence/mod.rs`、`server/src/player/mod.rs` 全部 0 命中。

由此产生两条独立可利用链：

1. **战斗白嫖满血**：任何战斗（PvP / NPC 交战 / 妖兽围攻）中受重伤的玩家，断线重连即满血 + 全部伤口条目消失——combat logout 是零成本的完全治疗，绕过所有治疗资源（丹药 / yidao 疗伤 / 自然恢复）的经济与时间成本。
2. **NearDeath 秒退免费逃脱**：被打倒进入 `NearDeath`（30s 稳定窗）的瞬间断线再重连——即便在途的 Lifecycle 持久化修复（见「与在途 PR 的关系」）恢复了 `NearDeath` 状态机，重连同 tick 插入的满血 `Wounds::default()` 会让 `near_death_tick` 的 stabilized 分支（`server/src/combat/lifecycle.rs:766-777`：`health_current > health_max * NEAR_DEATH_HEALTH_FRACTION` → 直接判 `Alive` + 清 `near_death_deadline_tick`）立即通过——濒死后果（稳定窗赌命、复活决策、Tribulation 永久终结风险）被 100% 免费逃脱。「刚被打倒瞬间秒退」是最早、最自然的逃逸时机。

## 实际游玩体验影响

- PvP：优势方把对手打到丝血/打倒，对手拔线 10 秒回来满血站起，战斗结果作废；死亡后果链（`plan-death-lifecycle-v1` 的 NearDeath → 复活决策 → Fortune/Tribulation）对会拔线的玩家形同虚设。
- PvE：妖兽/守卫把玩家打残的全部产出（伤口、濒死压力）一次重连归零，威胁谱系的压迫感失效（末法基调要求所有生物有威胁——重连白嫖直接取消威胁的后果面）。
- 经济：疗伤丹药、yidao 疗伤流派、卧棺休养的价值被"免费重连治疗"替代性摧毁。

## 证据定位

- `server/src/combat/components.rs:89-103`：`Wounds { entries, health_current, health_max }`，`Default` 实现 = 满血空伤口。
- `server/src/combat/mod.rs:91`：`type JoinedClientsWithoutCombatBundleFilter = (Added<Client>, Without<Wounds>)`——重连玩家必然 `Without<Wounds>`（实体是新 spawn 的），必然走 default 注入。
- `server/src/combat/mod.rs:93-117`：`attach_combat_bundle_to_joined_clients` 无条件 `insert(Wounds::default(), ...)`，无任何持久化读取。
- `server/src/combat/lifecycle.rs:766-777`：`near_death_tick` stabilized 分支——`NearDeath` + 血量高于阈值 → 立即 `Alive` + 清 deadline。满血 default 恒满足该条件。
- 持久化零覆盖（`origin/main` `662609339` 实测）：`server/src/player/state.rs`（`PlayerState` / `LoadedPlayerSlices` 及全部 slice load/save 函数）、`server/src/persistence/mod.rs`（全部迁移与表）、`server/src/player/mod.rs`（disconnect/shutdown 双 flush 路径）中 `Wounds` / `health_current` 均 0 命中。
- NPC 侧对照：`server/src/combat/mod.rs:138-146` `attach_combat_bundle_to_joined_npcs` 同样注入 default——但 NPC 无重连概念，属设计内，不在本 plan 范围。

## 触发路径

1. 玩家 A 在任意战斗中被打到残血（或被打倒进入 NearDeath 稳定窗）。
2. A 正常断开连接（关客户端 / 拔线，无需任何工具或 dev 命令）。
3. A 重连：实体重新 spawn，`attach_combat_bundle_to_joined_clients` 注入满血 `Wounds::default()`。
4. 残血场景 → A 满血归来；NearDeath 场景 → 下一 tick `near_death_tick` stabilized 分支判 `Alive`、清 deadline，濒死后果链整体跳过。

## 反方审查记录

- 来源：bughunt 20260726-r1 wave-1 `player-lifecycle-relog-death-consequence-wipe`（critical，在途修复中）的 opus 对抗验证明确指出：Lifecycle 持久化修复**只堵住了决策窗内的逃逸**（AwaitingRevival + fortune 耗尽 + Tribulation 风险），而「复原 NearDeath 后同 tick 插入的 `Wounds::default()`（满血）让 stabilized 分支立刻判活并清 deadline」——NearDeath 阶段秒退仍是免费逃单，且根因（Wounds 不持久化）超出该 PR 的最小修复边界，应独立立案。
- 主循环亲验：上列全部代码锚点在 `origin/main` `662609339` 实地读码 + grep 确认；持久化零覆盖为三文件全文 grep 结论，非抽样。
- 质疑「是否为刻意设计（重连即痊愈作为仁慈机制）」：`docs/finished_plans/plan-death-lifecycle-v1.md` 的 NearDeath/复活决策链设计明确以「后果不可白嫖」为目标（Fortune 消耗、Tribulation 永久终结风险）；若重连可满血，整条链路的设计意图自我矛盾——判定为遗漏而非设计。
- 去重核对（2026-07-26，基于 `662609339`）：`docs/plans-skeleton/` 87+20 个 bughunt skeleton 中无任何 Wounds / 血量持久化 / combat-logout 主题（`plan-bughunt-world-transport-tsy-relog-presence-v1` 是 TSY 维度 presence、`plan-bughunt-niche-guardian-cross-session-leak-v1` 是灵龛守卫会话泄漏，均不覆盖）；`docs/finished_plans/plan-death-lifecycle-v1.md` 未实现任何血量持久化；in-flight 分支 `bughunt-20260726-r1-0-player-lifecycle-relog-death-consequence-wipe` 明确将 Wounds 持久化排除在 scope 外（见其 PR 描述与 verify 记录）。

## 与在途 PR 的关系（实施顺序约束）

- **前置**：`bughunt-20260726-r1-0-player-lifecycle-relog-death-consequence-wipe`（Lifecycle 状态机持久化，含 `player_lifecycle` 单 JSON-blob 表 + `load/save_player_lifecycle_slice` + character_id 守卫 + wall-clock deadline 折算）。**本 skeleton 必须在该 PR merge 后实施**：
  1. 复用其表模式与 join/flush 接线点（`attach_combat_bundle_to_joined_clients` / disconnect / shutdown 三处已被该 PR 打开），Wounds slice 按同构模式并列落地，避免两套持久化风格。
  2. NearDeath 秒退链的完整闭环 = 该 PR（状态机恢复）+ 本 plan（血量恢复）两块拼齐；单独任何一块都堵不死。
- 若该 PR 最终未 merge（被 close），本 skeleton 升级为同时承接 Lifecycle + Wounds 双持久化（范围扩大需重新评估）。

## Skeleton Fix Plan

- [ ] 新增 `player_wounds` sqlite 表（单 JSON-blob 列镜像整个 `Wounds` 组件，keyed by username，与 `player_lifecycle` / `player_known_techniques` 同模式）+ 对应 migration（版本号以实施时 `persistence/mod.rs` 最新版本递增）。
- [ ] `server/src/player/state.rs` 新增 `load_player_wounds_slice` / `save_player_wounds_slice`（serde roundtrip 整个 `Wounds`；反序列化失败必须 `warn!` 后回退 default，不静默吞——对齐 lifecycle 返工中同类修正）。
- [ ] `attach_combat_bundle_to_joined_clients`（`server/src/combat/mod.rs:93-117`）：加载持久化 Wounds slice，`character_id` 与当前世匹配时复用（转世/新角色回退 default）——守卫模式与 lifecycle slice 完全一致，两个 slice 的 character_id 判定必须共用同一份计算，不许各写一份。
- [ ] `despawn_disconnected_clients` / `flush_connected_players_on_shutdown`（`server/src/player/mod.rs`）：query 加 `Option<&Wounds>`，断线与关服双路径落盘；若 lifecycle 返工补了 autosave 节奏，Wounds 并入同一 autosave 批次。
- [ ] 数值卫生：restore 时 clamp `health_current` 到 `[0, health_max]`、拒绝 NaN/负数（坏行回退 default + warn，防持久化层被污染后把 NaN 血量注回战斗系统）。
- [ ] NPC 路径（`attach_combat_bundle_to_joined_npcs`）保持现状不动；本 plan 不顺手持久化 `StatusEffects` / `ParryRecovery` / 其他战斗态（范围红线——它们是否需要持久化各自独立验真）。
- [ ] 饱和测试（见验收测试计划）。

## 验收测试计划

全部在 `server/` 用 `cargo test`：

- **slice roundtrip（state.rs 单测）**：满血空伤口 / 残血多伤口条目 / `health_current == 0`（濒死血量）三态往返；缺行返回 None 不报错；坏 JSON 回退 default + 不 panic；重复保存覆盖不堆行；NaN/负数/超 max 血量 restore 时被 clamp/拒绝。
- **join 集成（combat 测试）**：
  - 持久化残血玩家重连 → `Wounds.health_current` 精确等于落盘值，伤口条目逐条恢复。
  - **核心回归锁**：持久化「NearDeath + 濒死血量」的玩家重连（配合 lifecycle slice 同时恢复 NearDeath）→ 下一 tick `near_death_tick` **不得**走 stabilized 分支判活；deadline 保持。这是本 plan 的 headline 断言，失败信息必须写明「重连不得白嫖濒死状态」。
  - character_id 轮换（转世后）重连 → 旧 Wounds 丢弃、注入 default。
  - 无持久化行的首登玩家 → default，行为与现状一致。
- **flush 集成（player/mod.rs 测试）**：断线路径与关服路径分别落盘残血状态后回读 DB 断言。
- **bot 场景（`scripts/bot/scenarios/`）**：bot 被打至残血 → 断线 → 重连 → 从真实 S2C 状态断言血量不回满（阈值断言，容忍重连期间的自然恢复 tick 浮动）。
- 完整门禁：`cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`；bot 场景 + `bash scripts/smoke-test-e2e.sh`（headless 设 `BONG_SKIP_SKIN_PREFETCH=1`）。

## 风险

- **离线期间自然愈合语义**：本 plan 默认「原样恢复、不做离线愈合折算」——离线不是安全屋，重连回来还是那身伤（符合末法基调，也最简单）。若后续设计要求离线缓慢愈合，走 `player_lifespan.offline_pause_wall` 的 wall-clock 折算范式另立扩展，不在本 plan 抢跑。
- **autosave 缺失窗口**：硬崩（非 AppExit）时最后一次落盘可能陈旧——回读的血量可能高于崩溃前（等效小幅白嫖）。可接受的渐进边界：与 lifecycle slice 的 autosave 节奏对齐即可，不为 Wounds 单独发明高频落盘。
- **与治疗系统的交互**：yidao 疗伤 / 丹药 / 卧棺恢复写的是运行态 `Wounds`，本 plan 只加持久化边界，不改任何治疗公式；restore 的 clamp 不得吞掉治疗系统合法写入的血量上限变化（`health_max` 一并持久化并以运行时重算为准校验）。
- **不触碰 qi_physics**：血量/伤口不是真元，无守恒律接口；若实施中发现伤口条目携带真元字段（当前无），停下重评。

## Finish Evidence

> Skeleton 阶段留空；BugFix 完成后填写。

### 落地清单

- P0（持久化 + 重连恢复）：
- P1（饱和回归）：
- P2（bot 场景 + 门禁）：

### 关键 commit

- 待填写

### 测试结果

- 待填写

### 跨仓库核验

- server：
- bot：
- client：零改
- agent/schema：零改

### 遗留 / 后续

- `StatusEffects` / `ParryRecovery` 等其他战斗运行态的持久化需求各自独立验真。
- 离线自然愈合（wall-clock 折算）若立项，扩展本表结构而非另起一套。
