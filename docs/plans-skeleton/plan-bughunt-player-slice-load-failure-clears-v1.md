# plan-bughunt-player-slice-load-failure-clears-v1 — 玩家持久化 slice 加载失败被默认值覆盖写回：SkillSet 同款丢档 + 连接早退全 slice 降级

> **一句话**：`load_player_slices` 对每个 slice 的读取失败都静默兜底默认值，随后各 flush/autosave 系统把默认值**写回 DB**——一次 sqlite busy/损坏行/连接打不开就永久抹掉玩家真实存档。KnownTechniques 一份已由 PR #1288 修复（`LoadedKnownTechniques` 三态 + `KnownTechniquesLoadFailed` 写保护标记），但 **SkillSet 同款漏洞仍敞着**，且连接早退路径会让 state/position/inventory/skill_set/ui_prefs **全体同时降级**。本 plan 把 #1288 的写保护范式推广到全部 slice。
>
> 来源：2026-07-26 technique 系统 C1 修复（PR #1288 `bugfix/technique-load-guard`）的无上下文 opus validator 移交发现；本骨架全部 file:line 锚点已在 origin/main `662609339` 逐一亲验。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | SkillSet 写保护：复制 #1288 三态+标记范式，堵死 flush/shutdown/导出三条落盘路径 | ⬜ |
| P1 | 连接早退收口 + 逐 slice 盘点：state/position/inventory/lifespan/ui_prefs 写保护决议与落地 | ⬜ |
| P2 | LoadFailed 会话语义：玩家可观测提示 + 消耗类操作前置拒绝（卷轴白耗收口）+ 恢复路径 | ⬜ |
| P3 | 回归闭环：DB 注错 e2e + 断线重连/优雅关服全路径 + bot 场景 | ⬜ |

---

## §1 接入面（docs/CLAUDE.md §二 checklist）

- **进料**：
  - `player::state::load_player_slices`（`server/src/player/state.rs:422`）及各 `load_player_*_from_sqlite`；`load_player_state`（state.rs:385-392 Err 兜底 `PlayerState::default()`）
  - `open_player_connection` 失败早退（state.rs:427-451，全 slice 一次性降级默认）
  - join 装配消费点：`server/src/player/mod.rs:215`（`load_player_slices` 唯一生产调用）
- **出料**（写保护标记要挂的门）：
  - `flush_changed_player_skills`（mod.rs:804-816，Changed 含 join 时 Added，无任何门禁——P0 主目标）
  - `flush_changed_player_inventories`（mod.rs:780-802）、`autosave_player_core_slices`（mod.rs:622，5s 含 position）、`autosave_player_slow_and_ui_slices`（mod.rs:648，60s）、`autosave_player_lifespan_slices`（mod.rs:746）
  - `flush_connected_players_on_shutdown`（mod.rs:484）与断线落盘（`despawn_disconnected_clients` 链）
  - P2 玩家提示走既有 server_data 通知链（具体 payload symbol 升 active 时定，不新开通道）
- **共享类型 / event**：**复用 PR #1288 范式**——`LoadedKnownTechniques` 三态（NewPlayer / Loaded / LoadFailed）+ `KnownTechniquesLoadFailed` 标记组件 + `Without<KnownTechniquesLoadFailed>` 查询过滤 + `export_player_bundle` 拒绝导出。本 plan 为各 slice 落同构组件（如 `SkillSetLoadFailed`），不重造 event，不动 #1288 已修的 KnownTechniques 路径。
- **跨仓库契约**：纯 server（persistence 基建）；仅 P2 提示若走 HUD 需列 client 消费 symbol，升 active 时补。agent 不参与。
- **worldview 锚点**：§十二 死亡、重生与一生记录——存档即修士一生的记录，只能被正典机制（死亡/劫）改写，不允许被基建故障静默清写。纯 server 基建 plan，无视听规格要求。
- **qi_physics 锚点**：不新增常数、不新增真元流动路径。但 P1 必须核对一条守恒旁支：`PlayerState` 被 default 覆盖写回时，其 cultivation 侧 `qi_current` 快照被清零=真元凭空湮灭（吞真元红旗的持久化变体）——实施时用 `qi_physics::ledger::assert_conservation` 口径确认写保护后无此路径，不引入新的释放/回收逻辑。

## §2 审计事实底座（全部亲验于 origin/main 662609339）

`load_player_slices` 逐 slice 失败兜底与写回路径盘点（**加载失败 ≠ 无行**——无行是真新玩家，default 合法；有行但读失败被 default 覆盖才是丢档）：

| slice | Err 兜底 | 写回路径（覆盖发生点） | 现状防护 |
|-------|----------|------------------------|----------|
| PlayerState | `PlayerState::default()`（state.rs:385-392） | autosave core 5s / slow+ui 60s / shutdown | ❌ |
| position/dimension | spawn 默认（state.rs:460-470） | autosave core（position 回 spawn 被固化） | ❌（低危，§5#3 拍板） |
| inventory | `None`（state.rs:472-482）→ join 不插组件（mod.rs:261-263） | `(Added<Client>, Without<PlayerInventory>)` 补发默认背包（`server/src/inventory/mod.rs:121`）→ `flush_changed_player_inventories` 覆盖旧行 | ❌（补发→覆盖链路 P1 先实证再修） |
| craft_session | `None` + `tracing::error!` 「refusing to invent」（state.rs:483-493） | None 不插组件、无自动重建 | 半防护先例（不臆造；旧行是否残留待 P1 盘点） |
| lifespan | `None`（state.rs:494-512） | autosave lifespan（None→不插入，是否有补发系统 P1 盘点） | ❌ |
| skill_set | `SkillSet::default()`（state.rs:513-523）→ join 无条件 insert（mod.rs:286） | `flush_changed_player_skills`（Changed 含 Added）+ shutdown | ❌ **P0** |
| known_techniques | default（state.rs:524-534） | flush（mod.rs:818）+ shutdown（mod.rs:614） | ✅ PR #1288 |
| ui_prefs | default（state.rs:535-545） | autosave slow+ui（技能栏/快捷栏绑定全清） | ❌ |

- **连接早退放大器**：`open_player_connection` 失败（state.rs:427-451）不逐 slice 报错，直接 return 全默认 `LoadedPlayerSlices`——一次 DB busy 同时命中上表所有行。
- **SkillSet 丢档面**：`skills` + `consumed_scrolls` 双清零——已学技能全灭之外，卷轴消耗查重表同时蒸发（restored_skill 判定见 mod.rs:218-219）。
- 触发场景与 #1288 相同：重连瞬间 DB busy/锁竞争一次即触发；或未来给 slice struct 加无 `#[serde(default)]` 字段后读旧档必炸。

## §3 P0 — SkillSet 写保护（复制 #1288 范式）

- `server/src/player/state.rs`：`load_player_skill_set_from_sqlite` 返回三态（区分「无行=新玩家」/「有行读取成功」/「读取失败」，命名对齐 `LoadedKnownTechniques` 风格，如 `enum LoadedSkillSet`）；连接早退路径同样映射为 LoadFailed。
- 新标记组件 `SkillSetLoadFailed`（落点跟随 SkillSet 定义处），join 装配（mod.rs:286 附近）在 LoadFailed 时插入。
- 三条落盘路径挂门：`flush_changed_player_skills` 查询加 `Without<SkillSetLoadFailed>`；`flush_connected_players_on_shutdown` 与断线路径按 `Has<SkillSetLoadFailed>` 跳过 skill slice；player bundle 导出口（对齐 #1288 的 `export_player_bundle` 拒绝导出语义）。
- 饱和测试（对齐 #1288 已落的测试形态）：sqlite 注错 / 损坏 JSON 行 / 缺行新玩家正常 flush / 成功加载正常 flush / LoadFailed 后 flush 被阻且 DB 原文不被覆盖 / 失败会话断线+关服不落盘。断言带修复线索。

## §4 P1 — 连接早退收口 + 逐 slice 盘点落地

- `load_player_slices` 早退路径重构：整体 `LoadedPlayerSlices` 携带 per-slice load 结果（或统一 `PlayerSlicesLoadFailed` 粗粒度标记，取舍升 active 时定：粗粒度实现小但会把「单 slice 失败」升级为全会话只读）。
- **inventory 链路实证 + 修复**：写复现证明「load Err→None→补发默认背包→flush 覆盖旧行」整链可达（`server/src/inventory/mod.rs:121` 的 `JoinedClientsWithoutInventoryFilter` 是关键结点），然后以 `InventoryLoadFailed` 阻断补发或阻断 flush（两点至少断一点，倾向断在补发——不给玩家一个"看起来空了"的假背包）。与老库存 #249 布局迁移问题（memory `project_player_inventory_persist_migration_gap`）划界：那是 schema 迁移缺失，本项只管失败兜底写保护。
- state/ui_prefs/lifespan 写保护按 §2 表逐行落地；position 按 §5#3 决议。
- 每 slice 一组「失败注入→写回被阻」测试，覆盖 5s/60s autosave 与 shutdown 三类调度。

## §5 P2 — LoadFailed 会话语义（开放问题决议后实施）

#1288 已选口径：正常游玩但该 slice 不落盘。本阶段收口其副作用：

1. **卷轴白耗**（#1288 移交的次生观察）：LoadFailed 会话内 `consume 卷轴→learn` 的学习结果不落盘，但卷轴消耗走 inventory 正常 flush→白耗。决议方向（升 active 前拍板）：a) LoadFailed 时前置拒绝消耗类习得操作并提示（推荐，最小惊讶）；b) 事后补偿。同类互动（技能升级消耗、ui_prefs 绑定编辑）一并盘点。
2. **玩家可观测提示**：登录时一次性告知「存档暂不可用，本次修行不落盘」（走既有通知链，文案遵循末法语体，禁词表 worldview.md §三 L63）。
3. **恢复路径**：LoadFailed 是否支持会话内重试加载（如 5min 重试一次，成功则解除标记并以 DB 为准合并/覆盖内存态——合并语义复杂，倾向只提示重连）。

## §6 P3 — 回归闭环

- e2e：DB 注错（连接层+单 slice 层各一）→ join → 游玩写操作 → 断线/关服 → 重启核对 DB 原文未被覆盖。
- bot 场景（CI bot e2e 硬约定，memory `feedback_bot_client_e2e`）：`scripts/bot/scenarios/` 增加 LoadFailed 会话登录+提示可见+重连恢复场景。
- 全 slice 写保护 wiring guard 测试：枚举 `LoadedPlayerSlices` 字段与写保护标记的映射表，新增 slice 忘配写保护时撞红。

## §7 边界划定（不在本 plan 范围）

- **#1282 `plan-bughunt-wounds-relog-full-heal-v1`**：Wounds 组件从未持久化（加持久化路径）；本 plan 只管「已有持久化的 slice 加载失败时的写保护」，不新增持久化面。
- **#1284 `plan-bughunt-health-death-chain-v1` P3**：死态/运数/DeathRegistry 的持久化回读补全——同样是「加读路径」，非失败兜底。
- **PR #1288**：KnownTechniques 已修，本 plan 引用其范式、不重改其路径。
- **`plan-bughunt-identity-persist-key-mismatch-v1`**（skeleton）：身份写读主键漂移，与 load 失败无关。
- **`plan-bughunt-coffin-autosave-inflight-race-v1`**（skeleton）：写侧竞态，非读侧兜底。
- 老库存 #249 布局迁移：schema 演进问题，仅在 P1 inventory 项引用划界。

## §8 开放问题（升 active 前收口）

1. P1 粒度：per-slice 标记 vs 全会话 `PlayerSlicesLoadFailed` 粗标记（连接早退场景两者等价，单 slice 失败场景前者体验更好、实现面更大）。
2. 卷轴白耗口径（§5#1 a/b）。
3. position 回 spawn 是否纳入写保护（丢的是位置非资产，玩家损失有限；但与「存档不可被故障改写」原则一致性上仍建议纳入，代价是 position 也要三态）。
4. craft_session / lifespan 的 None 双语义（「无会话」vs「读失败」）是否值得拆分，或接受现状半防护。
