# BugHunt: Wounds 不持久化——战斗中断线重连满血清创 + NearDeath 秒退免费逃脱

## Bug 摘要

**严重度：high（战斗诚实性 + 死亡后果链双破坏；opus 对抗验证衍生发现，主循环亲验锚点）**

`Wounds`（血量 + 伤口条目，`server/src/combat/components.rs:89-103`）从落地起就没有任何持久化路径：`attach_combat_bundle_to_joined_clients`（`server/src/combat/mod.rs:91-117`，filter `(Added<Client>, Without<Wounds>)`）对每次加入（含重连）的玩家无条件 `insert(Wounds::default())`——即 `health_current = health_max = DEFAULT_HEALTH_MAX`、`entries` 清空；而 `despawn_disconnected_clients` / `flush_connected_players_on_shutdown`（`server/src/player/mod.rs`）从不读取或落盘 `Wounds`。在 `origin/main` `662609339` 上实测：`grep Wounds\|health_current` 于 `server/src/player/state.rs`、`server/src/persistence/mod.rs`、`server/src/player/mod.rs` 全部 0 命中。

由此产生两条独立可利用链：

1. **战斗白嫖满血**：任何战斗（PvP / NPC 交战 / 妖兽围攻）中受重伤的玩家，断线重连即满血 + 全部伤口条目消失——combat logout 是零成本的完全治疗，绕过所有治疗资源（丹药 / yidao 疗伤 / 自然恢复）的经济与时间成本。
2. **NearDeath 秒退免费逃脱**：被打倒进入 `NearDeath`（30s 稳定窗）的瞬间断线再重连——即便在途的 Lifecycle 持久化修复（见「与在途 PR 的关系」）恢复了 `NearDeath` 状态机，重连同 tick 插入的满血 `Wounds::default()` 会让 `near_death_tick` 的 stabilized 分支（`server/src/combat/lifecycle.rs:766-777`：`health_current > health_max * NEAR_DEATH_HEALTH_FRACTION` 严格大于判定 → 判 `Alive` + 清 `near_death_deadline_tick`）立即通过——濒死后果（稳定窗赌命、复活决策、Tribulation 永久终结风险）被 100% 免费逃脱。

## 实际游玩体验影响

- PvP：优势方把对手打到丝血/打倒，对手拔线 10 秒回来满血站起，战斗结果作废；死亡后果链（`plan-death-lifecycle-v1` 的 NearDeath → 复活决策 → Fortune/Tribulation）对会拔线的玩家形同虚设。
- PvE：妖兽/守卫把玩家打残的全部产出（伤口、濒死压力）一次重连归零，威胁谱系的压迫感失效。
- 经济：疗伤丹药、yidao 疗伤流派、卧棺休养的价值被"免费重连治疗"替代性摧毁。

## 证据定位

- `server/src/combat/components.rs:89-103`：`Wounds { entries, health_current, health_max }`，`Default` 实现 = 满血空伤口。
- `server/src/combat/mod.rs:91`：`type JoinedClientsWithoutCombatBundleFilter = (Added<Client>, Without<Wounds>)`——重连玩家必然 `Without<Wounds>`（实体是新 spawn 的），必然走 default 注入。
- `server/src/combat/mod.rs:93-117`：`attach_combat_bundle_to_joined_clients` 无条件 `insert(Wounds::default(), ...)`，无任何持久化读取。
- `server/src/combat/lifecycle.rs:766-777`：`near_death_tick` stabilized 分支——`NearDeath` + 血量**严格高于** `health_max * NEAR_DEATH_HEALTH_FRACTION` → 立即 `Alive` + 清 deadline。满血 default 恒满足该条件。
- `server/src/combat/lifecycle.rs:778-860`：deadline 到期分支——玩家路径 `determine_revival_decision` 有解 → `await_revival_decision(decision, now + REVIVAL_CONFIRM_WINDOW_TICKS)` + insert `DeathCinematic` + `emit_death_screen`；无解 → `terminate_lifecycle("natural_end")`。这是 P1 状态转换测试必须锁定的两个到期出口。
- 持久化零覆盖（`origin/main` `662609339` 实测）：`server/src/player/state.rs`（`PlayerState` / `LoadedPlayerSlices` 及全部 slice load/save 函数）、`server/src/persistence/mod.rs`（全部迁移与表）、`server/src/player/mod.rs`（disconnect/shutdown 双 flush 路径）中 `Wounds` / `health_current` 均 0 命中。
- 既有 S2C 血量链（bot 验收的数据来源，均为现成生产代码，本 plan 零改协议）：`server/src/network/wounds_snapshot_emit.rs:21` `emit_wounds_snapshot_payloads` → `ServerDataPayloadV1::WoundsSnapshot(WoundsSnapshotV1)`（schema：`server/src/schema/combat_hud.rs:33`，含 per-wound part/kind/severity/state 字段）；血量比例走 `server/src/network/combat_hud_state_emit.rs:56`（`(health_current / health_max).clamp(0.0, 1.0)` 进 combat HUD state payload）。
- NPC 侧对照：`server/src/combat/mod.rs:138-146` `attach_combat_bundle_to_joined_npcs` 同样注入 default——但 NPC 无重连概念，属设计内，不在本 plan 范围。

## 触发路径

1. 玩家 A 在任意战斗中被打到残血（或被打倒进入 NearDeath 稳定窗）。
2. A 正常断开连接（关客户端 / 拔线，无需任何工具或 dev 命令）。
3. A 重连：实体重新 spawn，`attach_combat_bundle_to_joined_clients` 注入满血 `Wounds::default()`。
4. 残血场景 → A 满血归来；NearDeath 场景 → 下一 tick `near_death_tick` stabilized 分支判 `Alive`、清 deadline，濒死后果链整体跳过。

## 反方审查记录

- 来源：bughunt 20260726-r1 wave-1 `player-lifecycle-relog-death-consequence-wipe`（critical，在途修复中）的 opus 对抗验证明确指出：Lifecycle 持久化修复**只堵住了决策窗内的逃逸**，而「复原 NearDeath 后同 tick 插入的 `Wounds::default()`（满血）让 stabilized 分支立刻判活并清 deadline」——NearDeath 阶段秒退仍是免费逃单，且根因（Wounds 不持久化）超出该 PR 的最小修复边界，应独立立案。
- 主循环亲验：上列全部代码锚点在 `origin/main` `662609339` 实地读码 + grep 确认；持久化零覆盖为三文件全文 grep 结论，非抽样。
- 质疑「是否为刻意设计」：`docs/finished_plans/plan-death-lifecycle-v1.md` 的 NearDeath/复活决策链设计明确以「后果不可白嫖」为目标；若重连可满血，整条链路的设计意图自我矛盾——判定为遗漏而非设计。
- 去重核对（2026-07-26，基于 `662609339`）：`docs/plans-skeleton/` 87+20 个 bughunt skeleton 中无任何 Wounds / 血量持久化 / combat-logout 主题；`docs/finished_plans/plan-death-lifecycle-v1.md` 未实现任何血量持久化；in-flight 分支 `bughunt-20260726-r1-0-player-lifecycle-relog-death-consequence-wipe` 明确将 Wounds 持久化排除在 scope 外。
- 首轮 `/review`（4×gpt-5.6-sol，41 findings）裁定初版 Fix Plan 的「坏行/缺行回退满血 default」会在 Lifecycle 已恢复 NearDeath 时重开本 plan 要堵的漏洞（blocker），并指出 character_id 无持久化载体、双 slice 非原子、立即重连时序、health_max 双权威矛盾、deadline 到期测试缺失、migration 生产注册无验收、bot S2C 链未定位共 8 类缺口——本版已按 §核心恢复契约 / §一致性与时序 / 各阶段测试计划逐项收口。

## 与在途 PR 的关系（实施顺序约束）

- **前置**：`bughunt-20260726-r1-0-player-lifecycle-relog-death-consequence-wipe`（Lifecycle 状态机持久化：`player_lifecycle` 单 JSON-blob 表 + `combat_clock_tick_at_save` 锚点列 + wall-clock deadline 折算 + character_id 守卫 + 60s autosave + 重连重发死亡屏，opus verify 已 ship）。**本 skeleton 必须在该 PR merge 后实施**：
  1. 复用其表模式、envelope 内 character_id 判定与 join/flush/autosave 三处接线点；两个 slice 的 character_id 计算必须共用同一份函数，不许各写一份。
  2. NearDeath 秒退链的完整闭环 = 该 PR（状态机恢复）+ 本 plan（血量恢复 + 联合恢复契约）两块拼齐。
- 若该 PR 最终被 close，本 skeleton 升级为同时承接 Lifecycle + Wounds 双持久化（范围扩大需重新评估）。

## 核心恢复契约（fail-closed，本 plan 的第一交付物）

重连恢复必须是 **Lifecycle × Wounds 联合裁决**，在任何战斗系统 tick 之前完成（与 lifecycle slice 同一接线点 `attach_combat_bundle_to_joined_clients`，其 `.after(attach_cultivation_to_joined_clients)` 顺序已由前置 PR 建立）。裁决表（每行一个确定结果，无二选一）：

**裁决的第一维是 Wounds slice 是否有效，而不是 Lifecycle 状态**——只要 wounds envelope 有效，无论 Lifecycle 是什么状态都恢复持久化血量（这是本 plan headline #1「残血重连不得满血」的正面实现）；`Wounds::default()` 只出现在「wounds 无效 **且** Lifecycle 允许满血」这一格。`LifecycleState` 的真实变体是 `Alive / NearDeath / AwaitingRevival / Terminated`（`server/src/combat/components.rs:195-200`，无 `Dead`），下表按全状态空间穷举：

| Wounds slice 状态 | Lifecycle 恢复结果 | 注入的 Wounds | 附加动作 |
|---|---|---|---|
| **有效**（envelope 校验 + 代次匹配 + character_id 匹配 + 数值卫生可修复） | 任意（Alive / NearDeath / AwaitingRevival / Terminated / 缺行） | **持久化值**（经数值卫生表） | — |
| 无效（缺行 / 坏 JSON / schema_version 不支持 / character_id 不匹配 / 代次不匹配 / 数值卫生不可修复） | Lifecycle 缺行 或 `Alive` | `Wounds::default()`（满血） | 非缺行原因一律 `warn!`；此格是唯一的满血入口 |
| 无效（同上） | `NearDeath` / `AwaitingRevival` / `Terminated` | **fail-closed 安全血量**：`entries` 空、`health_max` = 运行时权威值、`health_current = health_max * NEAR_DEATH_HEALTH_FRACTION`（恰在阈值上；stabilized 分支是严格 `>` 判定，**不会**判活） | `warn!` 带失败原因枚举；**deadline 与 Lifecycle 状态原样保留**，死亡后果链照常推进 |

- 第一行同时覆盖两条 headline：Alive 残血玩家重连拿回残血（不满血）；NearDeath 玩家重连拿回濒死血量（不判活）。
- 第二、三行覆盖审查指出的全部撕裂场景：升级后已有 lifecycle 行但尚无 wounds 行、单表写入失败、坏 JSON、转世不匹配、代次不一致——任何一种都不得产出「非 Alive + 满血」。
- 第二行（Alive + 无效 → 满血）是可接受的：Alive 且无可信血量数据时无法区分"首登"与"数据损坏"，且此格不参与濒死后果链；损坏原因写 warn 供运维追查。
- fail-closed 血量选 `阈值本身` 而非 0 或任意小值：确定、可审计、不改变 NearDeath 既有推进节奏（不提前终结也不判活）。

### 数值卫生表（唯一权威 + 逐项确定结果）

**`health_max` 的唯一权威 = 运行时**：恢复时不还原持久化的 `health_max`，一律取运行时初始化值（现状全仓唯一来源 `DEFAULT_HEALTH_MAX`，`server/src/combat/components.rs:95-103`；若实施时已出现修改 `health_max` 的系统，以该系统的重算入口为准并在 Finish Evidence 记录）。envelope 中仍存 `health_max` 字段但**仅作诊断**，加载永不采用。据此逐项固定（对象均为 envelope 内的 `health_current`）：

| 持久化值 | 结果 |
|---|---|
| 有限且 `0 <= v <= runtime_max` | 原样恢复 |
| 有限且 `v > runtime_max` | clamp 到 `runtime_max`，`warn!` |
| 负数 / NaN / ±Inf | 判「数值卫生不可修复」→ 走联合裁决表对应行（Alive → default；非 Alive → fail-closed 安全血量），`warn!` |
| entries 内单条字段非法 | 丢弃该条 entry，其余保留，`warn!`（entry 不参与 stabilized 判定，宽松处理不构成利用面） |

## 一致性与时序（三条契约，均为强制，不可互相替代）

1. **同事务提交**：disconnect / shutdown / autosave / 状态转换触发（下条）四条路径中，`player_lifecycle` 与 `player_wounds` 必须在**同一 SQLite transaction** 内、取自**同一 ECS 读取瞬间**的快照提交；任一写入失败整体回滚，禁止单表落盘。共享快照代次：两表 envelope 均含 `snapshot_combat_tick`（= 前置 PR 已有的 `combat_clock_tick_at_save` 同源锚点）；加载时两者不等 → 判「代次不匹配」→ 走裁决表第三行。**同事务只保证两表不撕裂，不保证新鲜度**（见下条与「风险」节）。

2. **致命转换先于快照（transition-before-save，本轮 review 新增的必修项）**：现状 `near_death_tick` 在 combat 的 Update 链（`server/src/combat/mod.rs:281-287`，`.after(death_arbiter_tick)`），`despawn_disconnected_clients` 在 player 链（`server/src/player/mod.rs:122`，`.after(flush_changed_player_inventories)`），两者**跨模块无任何相对顺序约束**。因此「同一 update 内挨致命一击 + 立刻断线」时，落盘可能是转换前的 `Alive + 残血`——重连后没有 NearDeath 状态，秒退依旧成功，headline #2 不闭环。必修两条：
   - 钉死 ordering：`despawn_disconnected_clients` 与 shutdown flush 显式 `.after(crate::combat::lifecycle::near_death_tick)`（并顺带 `.after(resolve::resolve_attack_intents)` / `.after(lifecycle::death_arbiter_tick)`，保证同 update 的伤害与状态转换都已落定）。实施时若 Bevy 0.14 的跨模块 set 依赖需要显式 `SystemSet`，一并引入并在 Finish Evidence 记录。
   - **状态转换触发即时落盘**：Lifecycle 从 `Alive` 进入任何非 Alive 状态时立即触发一次同事务 lifecycle+wounds 落盘（不等 60s autosave、不等断线）。这同时把硬崩场景的陈旧窗口从 60s 压到 sub-tick。

3. **断线提交先于同名重连加载（barrier，强制；session generation 只能补强不能替代）**：`despawn_disconnected_clients` 的**事务已提交**必须先于同 username 新实体的 join 恢复被观察到。实施时钉死系统 ordering / schedule 阶段边界形成真实 barrier；session generation 可以额外加上用来**检测**陈旧写入（检测到即拒绝该次写入），但它无法阻止新实体读到缺行后按裁决表回退，所以不构成 barrier 的替代方案。以「同一 update 内断线+重连」「下一 tick 重连」两条真实 App schedule 集成测试锁定：新实体读到的必须是刚提交的残血值，不得是缺行或陈旧行。

## Skeleton Fix Plan

- [ ] 新增 `player_wounds` sqlite 表：`username` 主键 + 单 JSON envelope 列。**envelope 结构显式版本化**，`entries` 逐字段钉死（镜像 `Wound`，`server/src/combat/components.rs:78-86`）：

  ```jsonc
  {
    "schema_version": 1,                  // u32；未列入支持集 → Err(UnsupportedVersion)
    "character_id": "…",                  // String，与 lifecycle slice 同一 canonical 计算
    "snapshot_combat_tick": 0,            // u64，= combat_clock_tick_at_save 同源锚点
    "health_current": 0.0,                // f32，唯一被恢复的血量字段
    "health_max": 0.0,                    // f32，仅诊断，加载永不采用
    "entries": [{                         // 镜像 Wound，字段名与 serde 表示随组件
      "location": "…",                    // BodyPartId 的既有 serde 表示（不新造编码）
      "kind": "…",                        // WoundKind；组件已有 #[serde(default)]，旧行缺字段合法
      "severity": 0.0,                    // f32
      "bleeding_per_sec": 0.0,            // f32
      "created_at_tick": 0,               // u64（绝对 CombatClock tick，跨重启不折算——伤口不带 deadline 语义）
      "inflicted_by": null                // Option<String>
    }]
  }
  ```

  envelope 与 wire 层 `WoundEntryV1`（`server/src/schema/combat_hud.rs`）是两套独立表示，本 plan 不合并、不互相派生。migration 版本号以实施时 `persistence/mod.rs` 最新版本递增，建表进生产 `apply_migrations` 递进链（fresh DB 同样建表）。
- [ ] `server/src/player/state.rs` 新增 `load_player_wounds_slice` / `save_player_wounds_slice`；load 返回 `Result<Option<WoundsEnvelope>>`，失败分支带原因枚举（`MissingRow / MalformedJson / UnsupportedVersion / CharacterIdMismatch / GenerationMismatch / NumericUnrecoverable`）供裁决表消费；save 走同事务接口。
- [ ] `attach_combat_bundle_to_joined_clients`（`server/src/combat/mod.rs:93-117`）：实现裁决表全部三行 × 全部 Lifecycle 状态。
- [ ] `despawn_disconnected_clients` / `flush_connected_players_on_shutdown` / 前置 PR 的 60s autosave / 新增的转换触发落盘：query 加 `Option<&Wounds>`，与 lifecycle slice 同事务、同快照落盘。
- [ ] 实现「一致性与时序」三条契约：同事务+代次、transition-before-save（ordering + 转换触发即时落盘）、save-commit-before-load barrier。
- [ ] NPC 路径（`attach_combat_bundle_to_joined_npcs`）保持现状；本 plan 不持久化 `StatusEffects` / `ParryRecovery`（范围红线，各自独立验真）。
- [ ] 饱和测试（下节）。

## 验收测试计划

全部在 `server/` 用 `cargo test`：

- **envelope 契约（state.rs 单测 + golden fixture）**：
  - roundtrip：满血空伤口 / 残血多伤口 / `health_current == 0` 三态；重复保存覆盖不堆行；缺行返回 `Ok(None)`。
  - **冻结历史 fixture**：提交手写的 `schema_version = 1` JSON fixture 文件（非当前 serializer 现产），断言字段名、必选字段、entry 形状可读——防两端同改的自 roundtrip 假绿；`schema_version` 未知 → 返回带原因的 Err（进联合裁决表 fail-closed 行）。
  - 数值卫生表逐行 pin：`v > runtime_max` → clamp；负数 / NaN / ±Inf → 不可修复错误；非法 entry 单条丢弃。每行独立断言确切值，无「clamp/拒绝」开放分支。
- **migration 生产注册（persistence 集成测试）**：从实施前最新已发布 schema 版本构造 DB → 走生产 `apply_migrations` → 断言版本号递增、`player_wounds` 表与 username 唯一约束存在、重复启动幂等 → 经真实 load/save API 完成一次残血写入与回读。
- **裁决表集成（combat 测试，全状态空间 × 有效/无效两维穷举）**：
  - **有效 wounds 行**（裁决表第一行）× 全部 4 个 `LifecycleState` + Lifecycle 缺行 = 5 条用例，一律断言恢复持久化残血值、**绝不注入 default**：
    - `Alive` + 有效残血 → 原样恢复（headline #1：残血重连不满血）。
    - `NearDeath` + 有效濒死血量（未到期 deadline）→ 下一 tick 不 stabilized、deadline 保持——随后**推进 clock 越过 deadline**，断言进入 `lifecycle.rs:778-860` 的到期出口（`determine_revival_decision` 有解 → `AwaitingRevival` + `DeathCinematic` insert + death screen emit；构造无解场景 → `terminate_lifecycle("natural_end")`），死亡后果链完整闭环，不允许只锁瞬时状态。
    - `AwaitingRevival` + 有效残血 → 血量恢复、决策窗与 `revival_decision_deadline_tick` 不被血量恢复干扰（配合前置 PR 的重连重发死亡屏断言）。
    - `Terminated` + 有效残血 → 血量恢复不复活该角色（`Terminated` 不因血量变化回到 Alive）。
    - Lifecycle 缺行 + 有效残血 → 血量恢复，Lifecycle 走前置 PR 的 default。
  - **无效 wounds**（第二/三行）× 5 种失败原因（缺行 / 坏 JSON / `schema_version` 不支持 / character_id 不匹配 / 代次不匹配）：
    - Lifecycle 缺行或 `Alive` → default 满血（唯一满血入口），非缺行原因断言 warn。
    - `NearDeath` / `AwaitingRevival` / `Terminated` 各自 → fail-closed 安全血量、**不判活**、deadline 与状态原样保留、warn 带原因。NearDeath × 5 原因逐条独立用例（首轮 review blocker 的直接回归锁），另两个状态各至少覆盖缺行 + 坏 JSON。
  - **离线期间 deadline 已到期** + 有效濒死血量 → 重连后（lifecycle slice 的 wall-clock 折算判定已过期）下一 tick 直接走到期分支进入复活决策/终结，不停留 NearDeath。
  - **正向对照**：NearDeath + 合法治疗把血量拉到严格高于阈值 → stabilized 判活 + 清 deadline（锁住阈值语义没被本 plan 改坏）。
  - 转世（character_id 轮换）+ Alive → 旧 wounds 丢弃注入 default。
- **原子性与时序（player/mod.rs + 真实 App schedule 集成）**：
  - 同事务：**disconnect / shutdown / autosave / 转换触发四条路径各一条回滚用例**——注入单表写失败（只读连接 / 表锁 / 中途 Err 注入）断言两表均未提交、DB 保持前一状态。
  - **transition-before-save**：同一 update 内构造「致命一击 → Lifecycle 转 NearDeath → 客户端断线」，断言落盘的 lifecycle 状态是 `NearDeath` 且 wounds 是濒死血量（不是转换前的 `Alive` + 残血）；配套断言 ordering 生效（`despawn_disconnected_clients` 在 `near_death_tick` 之后运行）。
  - **转换触发即时落盘**：Lifecycle 进入非 Alive 的同 tick 断言 DB 已有对应两行（不等 60s autosave、不等断线）。
  - **barrier**：「同 update 断线+重连」与「下一 tick 重连」两条调度测试，新实体恢复到刚提交的残血值；另一条断言 session generation 检测到陈旧写入时拒绝该次写入（补强机制单独可测，但不作为 barrier 替代）。
- **bot 场景（`scripts/bot/scenarios/`）**：bot 被打至残血 → 断线 → 重连 → 从真实 S2C 断言血量未回满。数据链（零协议改动）：server `emit_wounds_snapshot_payloads`（`server/src/network/wounds_snapshot_emit.rs:21`，`ServerDataPayloadV1::WoundsSnapshot`）+ combat HUD state 的血量比例（`server/src/network/combat_hud_state_emit.rs:56`）；bot 侧按既有 proto_min payload 解码范式消费，若 bot 解码器尚未覆盖这两个 payloadCase，则把 bot 侧解码支持纳入本 plan P2 范围（bot 属 `scripts/`，client/agent/schema 仍零改）。断言用比例阈值并容忍重连期间自然恢复 tick 浮动。
- 完整门禁：`cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`；bot 场景 + `bash scripts/smoke-test-e2e.sh`（headless 设 `BONG_SKIP_SKIN_PREFETCH=1`）。

## 风险

- **离线期间自然愈合语义**：默认「原样恢复、不做离线愈合折算」——离线不是安全屋（符合末法基调）。若后续设计要求离线缓慢愈合，走 `player_lifespan.offline_pause_wall` 的 wall-clock 折算范式另立扩展。
- **硬崩陈旧窗口（本轮 review 纠正了上一版的错误结论）**：同事务 + 同代次只保证两表**不撕裂**，**不保证新鲜**。硬崩（非 AppExit）后两表可能同时停留在同一份陈旧快照上——例如「`Alive` + 满血」这一致但过期的组合会被裁决表第一行正常恢复，玩家因此白嫖。上一版声称「NearDeath 场景不受硬崩影响」是错的：fail-closed 只挡撕裂，挡不住一致的陈旧。缓解＝「一致性与时序」§2 的**状态转换触发即时落盘**，把濒死场景的陈旧窗口从 60s autosave 周期压到 sub-tick（进入非 Alive 的同 tick 就已提交）；普通残血（Alive 状态内血量变化）仍受 autosave 周期约束，硬崩时可白嫖至多一个 autosave 周期的伤害——这是本 plan 明确接受的边界，若后续要收紧需另立「血量变化脏标记高频落盘」扩展，不在本 plan 抢跑。
- **与治疗系统的交互**：yidao / 丹药 / 卧棺写的是运行态 `Wounds`，本 plan 只加持久化边界，不改治疗公式；`health_max` 运行时权威规则保证治疗系统对上限的合法修改不被持久化旧值覆盖。
- **不触碰 qi_physics**：血量/伤口不是真元，无守恒律接口；若实施中发现伤口条目携带真元字段（当前无），停下重评。

## Finish Evidence

> Skeleton 阶段留空；BugFix 完成后填写。

### 落地清单

- P0（envelope 表 + 联合裁决恢复 + 同事务落盘 + 时序守卫）：
- P1（饱和回归：裁决表逐行 / 数值卫生逐行 / golden fixture / migration 注册 / deadline 双出口）：
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
- 离线自然愈合（wall-clock 折算）若立项，扩展 envelope 而非另起一套。
