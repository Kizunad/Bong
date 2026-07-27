# plan-bughunt-player-slice-load-failure-clears-v1 — 玩家持久化 slice 加载失败被默认值覆盖写回：SkillSet 同款丢档 + 连接早退全 slice 降级

> **一句话**：`load_player_slices` 对每个 slice 的读取失败都静默兜底默认值，随后各 flush/autosave 系统把默认值**写回 DB**——一次 sqlite busy/损坏行/连接打不开就永久抹掉玩家真实存档。KnownTechniques 一份已由 PR #1288 修复（`LoadedKnownTechniques` 三态 + `KnownTechniquesLoadFailed` 写保护标记），但 **SkillSet 同款漏洞仍敞着**，且连接早退路径会让 state/position/inventory/skill_set/ui_prefs **全体同时降级**。
>
> **不变量口径（本 plan 的验收总纲）**：对每个**已证实存在覆盖链**的持久化 slice，LoadFailed 会话中该 slice 的数据**不得触达任何持久化出口或导出出口**（即时 flush / 5s autosave / 60s autosave / 断线落盘 / 关服落盘 / player bundle 导出，下称「六类出口」；单个 slice 实际不经过的出口在矩阵中显式标「不适用」）；且**任何依赖失败 slice 落盘的跨 slice 消耗操作必须在消耗发生前被拒绝**（不允许"扣了库存、学的东西不落盘"的部分提交）。覆盖链未证实的 slice（当前：lifespan、craft_session）按 §4 调查项处理——证实即纳入同款门禁，证伪则记录「已核验排除」，**不预先计入漏洞清单**。
>
> 来源：2026-07-26 technique 系统 C1 修复（PR #1288 `bugfix/technique-load-guard`）的无上下文 opus validator 移交发现；本骨架全部 file:line 锚点已在 origin/main `662609339` 逐一亲验。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | SkillSet 写保护（全出口门禁）+ 跨 slice 消耗前置拒绝（卷轴白耗自 P0 即封死，不可拆分交付） | ⬜ |
| P1 | 连接早退收口 + 已证实 slice（state/position/inventory/ui_prefs）全出口门禁 + lifespan/craft_session 可达性裁决 + 统一 bundle 契约 | ⬜ |
| P2 | LoadFailed 会话语义：玩家可观测提示（文案绑定粒度决议）+ 恢复路径 | ⬜ |
| P3 | 回归闭环：slice × 出口全矩阵 e2e + bot 场景 + 编译期穷尽 wiring guard | ⬜ |

---

## §1 接入面（docs/CLAUDE.md §二 checklist）

- **进料**：
  - `player::state::load_player_slices`（`server/src/player/state.rs:422`）及各 `load_player_*_from_sqlite`；`load_player_state`（state.rs:385-392 Err 兜底 `PlayerState::default()`）
  - `open_player_connection` 失败早退（state.rs:427-451，全 slice 一次性降级默认）
  - join 装配消费点：`server/src/player/mod.rs:215`（`load_player_slices` 唯一生产调用）
- **出料**（写保护标记消费端 = 六类出口，逐一挂门）：
  - 即时 flush：`flush_changed_player_skills`（mod.rs:804-816，Changed 含 join 时 Added，无任何门禁——P0 主目标）、`flush_changed_player_inventories`（mod.rs:780-802）
  - 定时 autosave：`autosave_player_core_slices`（mod.rs:622，5s 含 position）、`autosave_player_slow_and_ui_slices`（mod.rs:648，60s）、`autosave_player_lifespan_slices`（mod.rs:746）
  - 断线落盘：`despawn_disconnected_clients` 链；关服落盘：`flush_connected_players_on_shutdown`（mod.rs:484）
  - 导出：player bundle 导出口（对齐 #1288 的 `export_player_bundle` 拒绝语义）——**通用消费端，所有 LoadFailed 标记都必须被它消费**（统一契约见 §4）
  - P2 玩家提示走既有 server_data 通知链——skeleton 不预写 payload symbol，但把「emit 系统落点 + 注册路径 + client 消费点 + bot 断言锚」四件套列为**升 active 硬门槛**（§8#3）
- **共享类型 / event**：**复用 PR #1288 范式**——`LoadedKnownTechniques` 三态（NewPlayer / Loaded / LoadFailed）+ `KnownTechniquesLoadFailed` 标记组件 + `Without<KnownTechniquesLoadFailed>` 查询过滤 + 导出拒绝。本 plan 为各 slice 落同构组件（如 `SkillSetLoadFailed`），不重造 event，不动 #1288 已修的 KnownTechniques 路径。
- **跨仓库契约**：纯 server（persistence 基建）；P2 提示的 client 消费 symbol 按 §8#3 硬门槛在升 active 时补齐。agent 不参与。
- **worldview 锚点**：§十二 死亡、重生与一生记录——存档即修士一生的记录，只能被正典机制（死亡/劫）改写，不允许被基建故障静默清写。纯 server 基建 plan，无视听规格要求。
- **qi_physics 锚点**：不新增常数、不新增真元流动路径。但 P1 必须核对一条守恒旁支：`PlayerState` 被 default 覆盖写回时，其 cultivation 侧 `qi_current` 快照被清零=真元凭空湮灭（吞真元红旗的持久化变体）——实施时用 `qi_physics::ledger::assert_conservation` 口径确认写保护后无此路径，不引入新的释放/回收逻辑。

## §2 审计事实底座（全部亲验于 origin/main 662609339）

**加载失败 ≠ 无行**——无行是真新玩家，default 合法；有行但读失败被 default 覆盖才是丢档。盘点分两档：

### §2.1 已证实覆盖链（Err 兜底产出组件/值 → join 装配 → 写回路径可达）

| slice | Err 兜底 | 写回路径（覆盖发生点） | 现状防护 |
|-------|----------|------------------------|----------|
| PlayerState | `PlayerState::default()`（state.rs:385-392） | autosave core 5s / slow+ui 60s / 断线 / 关服 | ❌ |
| position/dimension | spawn 默认（state.rs:460-470），join 直接 `position.set`（mod.rs:223） | autosave core（position 回 spawn 被固化） | ❌（**纳入保护**，见 §4，不设"损失有限"豁免） |
| inventory | `None`（state.rs:472-482）→ join 不插组件（mod.rs:261-263）→ `(Added<Client>, Without<PlayerInventory>)` 补发默认背包（`server/src/inventory/mod.rs:121`） | `flush_changed_player_inventories` / 断线 / 关服 覆盖旧行（补发→覆盖整链为 P1 首个任务，先复现实证再落门禁） | ❌ |
| skill_set | `SkillSet::default()`（state.rs:513-523）→ join 无条件 insert（mod.rs:286） | `flush_changed_player_skills`（Changed 含 Added）+ 断线 + 关服 + 导出 | ❌ **P0** |
| known_techniques | default（state.rs:524-534） | flush（mod.rs:818）+ shutdown（mod.rs:614） | ✅ PR #1288 |
| ui_prefs | default（state.rs:535-545） | autosave slow+ui（技能栏/快捷栏绑定全清） | ❌ |

### §2.2 待验证风险（Err 兜底为 None、join 不插组件，**当前未找到默认组件生产者，写回链未证实——不计入漏洞清单**）

| slice | Err 兜底 | 现状 | 处置 |
|-------|----------|------|------|
| craft_session | `None` + `tracing::error!`「refusing to invent」（state.rs:483-493） | 无已证实的自动重建路径；「不臆造」是本 plan 引用的半防护先例 | P1 调查项：证实覆盖链才纳入门禁，证伪则记录「已核验排除」 |
| lifespan | `None`（state.rs:494-512） | join 仅 `Some` 才 insert（mod.rs:267-269）；autosave 查询对无组件实体不写回；是否存在补发系统未盘点 | 同上，P1 调查项 |

- **连接早退放大器**：`open_player_connection` 失败（state.rs:427-451）不逐 slice 报错，直接 return 全默认 `LoadedPlayerSlices`——一次 DB busy 同时命中 §2.1 所有行。
- **SkillSet 丢档面**：`skills` + `consumed_scrolls` 双清零——已学技能全灭之外，卷轴消耗查重表同时蒸发（restored_skill 判定见 mod.rs:218-219）。
- 触发场景与 #1288 相同：重连瞬间 DB busy/锁竞争一次即触发；或未来给 slice struct 加无 `#[serde(default)]` 字段后读旧档必炸。

## §3 P0 — SkillSet 写保护 + 跨 slice 消耗前置拒绝（一个不可拆分的交付物）

> **P0 发布前置**：SkillSet 全出口门禁与跨 slice 消耗前置拒绝必须同时完成。**只做前者会把已知的卷轴白耗留在可发布阶段**（扣库存成功、学习结果不落盘的部分提交），不允许拆开交付。

- `server/src/player/state.rs`：`load_player_skill_set_from_sqlite` 返回三态（「无行=新玩家」/「读取成功」/「读取失败」，命名对齐 `LoadedKnownTechniques` 风格，如 `enum LoadedSkillSet`）；连接早退路径同样映射为 LoadFailed。
- 新标记组件 `SkillSetLoadFailed`，join 装配（mod.rs:286 附近）在 LoadFailed 时插入。
- **全出口门禁**：`flush_changed_player_skills` 加 `Without<SkillSetLoadFailed>`；断线与关服路径按 `Has<SkillSetLoadFailed>` 跳过 skill slice；player bundle 导出口拒绝（对齐 #1288 语义）。SkillSet 不经过 5s/60s autosave（亲验 mod.rs:622/648 查询不含 SkillSet），矩阵记「不适用」而非留空。
- **跨 slice 消耗前置拒绝**：`SkillSetLoadFailed`（含既有 `KnownTechniquesLoadFailed`）存在时，卷轴学习/技能升级等「消耗健康 slice、写入失败 slice」的操作在**任何库存扣减发生之前**返回确定性拒绝（reject reason 走既有 cast/interact 拒绝链，不新造 event）。「操作 → 依赖 slice」映射收敛为单一函数，禁止散点 if。
- 饱和测试：
  - 加载三态 × 行为：sqlite 注错 / 损坏 JSON 行 / 缺行新玩家 / 成功加载，各自 flush 行为正确；
  - LoadFailed 后逐一触发全部适用出口（即时 flush、断线、关服、导出），断言 **DB 原文逐字不变**；
  - 导出拒绝测试**在 P0 就交付**，不推给 P1；
  - 跨 slice 原子性：LoadFailed 时学习请求被拒且 inventory、SkillSet、consumed_scrolls 的内存与 DB 均不变；Loaded 状态同一操作扣减与习得同时成功；重连成功加载后同一操作恢复可用。

## §4 P1 — 连接早退收口 + 已证实 slice 全出口门禁 + 待验证 slice 裁决

- `load_player_slices` 早退路径重构：`LoadedPlayerSlices` 携带 per-slice load 结果（或统一 `PlayerSlicesLoadFailed` 粗标记，粒度取舍见 §8#1——**无论选哪种，§3 的跨 slice 消耗拒绝不变量与全出口门禁自 P0 起恒成立**，粒度只影响未失败 slice 是否连坐冻结）。
- **inventory**：先写复现实证「load Err→None→补发默认背包（inventory/mod.rs:121）→flush 覆盖旧行」整链，然后 `InventoryLoadFailed` **无条件守住全部持久化+导出出口**（即时 flush / 断线 / 关服 / bundle 导出）；阻断默认背包补发**同时做**，但只作为会话体验措施（不给玩家假空背包），**不计入写保护验收**——写侧门禁是不变量，不因补发被阻而豁免。测试含「LoadFailed 后由其他路径人为插入/修改 `PlayerInventory`，逐一触发各出口，DB 原文不变」。与老库存 #249 布局迁移问题（memory `project_player_inventory_persist_migration_gap`）划界：那是 schema 迁移缺失，本项只管失败兜底写保护。
- **position/dimension：正式纳入**三态加载 + 写侧门禁（autosave core / 断线 / 关服），不设"损失有限"豁免——风险大小不改变「存档不可被故障改写」的不变量条件。
- **state / ui_prefs**：按 §2.1 表落全出口门禁。
- **lifespan / craft_session 裁决**：给出「读失败→默认组件生产者→writer 覆盖旧行」完整 file:line 链 + 复现测试；证实 → 纳入同款三态+门禁；证伪 → 在 plan 内记录「已核验排除 + 证据」，**不为不存在的覆盖链落无消费者的标记组件**（防功能蔓延）。
- **统一 bundle 契约**：任一参与导出的持久化 slice 为 LoadFailed → **拒绝整包导出**并返回可辨识错误（报告具体失败 slice）；不做部分导出，不把内存默认值伪装成成功数据。实施时先盘点导出口实际包含的 slice 集合，每种 LoadFailed 标记配导出拒绝 pin 测试。
- **测试 = slice × 出口显式矩阵**：行 = §2.1 全部 slice（+裁决后纳入项），列 = 六类出口（不适用的格子显式标注理由），每格断言 LoadFailed 拒绝且 DB 原文逐字不变、NewPlayer/Loaded 正常写入；另加损坏行 / 连接失败 / 重连恢复后标记消失且写入恢复 三组横切用例。

## §5 P2 — LoadFailed 会话语义（提示 + 恢复）

> 跨 slice 消耗拒绝已在 P0 落地；本阶段只收口玩家可观测性与恢复。开工前置 = §8#3 四件套齐。

1. **玩家可观测提示**：登录时一次性告知，**文案与 §8#1 粒度决议绑定**——粗粒度（全会话冻结）才允许「本次修行不落盘」类总括文案；per-slice 模式文案必须准确指明失败范围（如「功法修行记录暂不可保存，其余照常」），禁止总括误述。文案遵循末法语体（命名禁词见 worldview.md §三 L63）。提示只发一次、重连恢复后不再提示，两条都进 bot 断言。
2. **恢复路径**：LoadFailed 是否支持会话内定时重试加载（成功→解除标记；内存态与 DB 的合并语义复杂，倾向只提示重连），§8#2 拍板。

## §6 P3 — 回归闭环

- e2e：DB 注错（连接层 + 单 slice 层各若干）→ join → 游玩写操作 → 断线/关服 → 重启核对 DB 原文未被覆盖；覆盖清单直接引用 §4 的 slice × 出口矩阵；卷轴场景断言「拒绝发生在库存扣减前」。
- bot 场景（CI bot e2e 硬约定，memory `feedback_bot_client_e2e`）：`scripts/bot/scenarios/` 增加 LoadFailed 会话登录 + 提示可见（按 §8#3 钉死的断言锚）+ 重连恢复场景。
- **编译期穷尽 wiring guard**（**不用手工映射表**——手工表会与生产代码同步漏改，无法兑现"新增 slice 忘配写保护时撞红"）：测试对 `LoadedPlayerSlices` 做**不带 `..` 的结构解构 / 穷举 match**，新增字段直接编译失败，逼迫作者为新 slice 显式登记写保护策略（保护 / 已核验排除，二选一，登记处即 §4 矩阵）。

## §7 边界划定（不在本 plan 范围）

- **#1282 `plan-bughunt-wounds-relog-full-heal-v1`**：Wounds 组件从未持久化（加持久化路径）；本 plan 只管「已有持久化的 slice 加载失败时的写保护」，不新增持久化面。
- **#1284 `plan-bughunt-health-death-chain-v1` P3**：死态/运数/DeathRegistry 的持久化回读补全——同样是「加读路径」，非失败兜底。
- **PR #1288**：KnownTechniques 已修，本 plan 引用其范式、不重改其路径（P0 的跨 slice 消耗拒绝会消费其 `KnownTechniquesLoadFailed` 标记，属新增消费者，不改其产生与门禁逻辑）。
- **`plan-bughunt-identity-persist-key-mismatch-v1`**（skeleton）：身份写读主键漂移，与 load 失败无关。
- **`plan-bughunt-coffin-autosave-inflight-race-v1`**（skeleton）：写侧竞态，非读侧兜底。
- 老库存 #249 布局迁移：schema 演进问题，仅在 §4 inventory 项引用划界。

## §8 开放问题（升 active 前收口）

1. P1 粒度：per-slice 标记 vs 全会话 `PlayerSlicesLoadFailed` 粗标记。连接早退场景两者等价；单 slice 失败场景 per-slice 体验更好、实现面更大。**约束**：无论选哪种，P0 的跨 slice 消耗前置拒绝与全出口门禁不变量恒成立，且 §5#1 文案必须与所选粒度一致。
2. 恢复路径口径（§5#2）：会话内重试合并 vs 只提示重连（倾向后者，合并语义有覆盖新进度风险）。
3. **提示契约硬门槛**：升 active 前必须补齐四件套——server emit 系统落点、payload/枚举注册路径、client 消费与展示组件、bot 可断言的文本/事件锚（写进 P2 交付物）。skeleton 阶段不预写 symbol（通知链具体形态属 consume 阶段设计收口），但缺四件套不得开工 P2。
4. lifespan / craft_session 若裁决为「已核验排除」，其 Err 路径是否仍要把 warn 升级为可观测告警（防止未来新增补发系统时无声引入覆盖链）——低成本防回归，倾向做。
