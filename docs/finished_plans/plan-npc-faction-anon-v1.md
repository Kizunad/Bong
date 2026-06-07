# Bong · plan-npc-faction-anon-v1 · 骨架

把 hydrated NPC 展示线上残留的具名宗门语义（`魔修派 / 正道盟 / 掌门 / 真传弟子 / 宗门弟子`）匿名化收口，对齐已落地的 offscreen-war **reframe b** 正典决议（匿名涌现群体，无具名宗门），并把 `Disciple` archetype 重定义为末法残土合理身份（推荐「残宗余孽」）。**纯收口 plan，不改 worldview，不引入新机制。**

## 阶段总览

| 阶段 | 主题 | 状态 | 验收日期 |
|------|------|------|---------|
| P0 | server payload 匿名化：faction 退出玩家可见层 | ✅ | 2026-06-07 |
| P1 | `Disciple` archetype 重定义为「残宗余孽」（server） | ✅ | 2026-06-07 |
| P2 | client 渲染对齐 + reframe b 正则 pin + e2e | ✅ | 2026-06-07 |
| ~~P3~~ | ~~EmergentGroupId live 数据通路~~ —— **已取消**：§8.1 #1 选定方案 (a) 完全匿名 | ❌ 2026-06-05 | — |

> **共 3 PR**（P0 → P1 → P2）。§8 开放问题已于 2026-06-05 全部收口（见 §8.1），可提升为 active 后 `/consume-plan`。

> **方向依据**：本 plan 的匿名化方向**已被正典决议定调**，不是新设计——见 `docs/finished_plans/plan-offscreen-war-v1.md:327`（§10.1 #6 reframe b 已决：「P5-P7 全程匿名涌现群体 + 『{zone}一带散修』区域描述符，无具名宗门/组织化语义，未改 worldview.md」）+ `docs/finished_plans/plan-social-v1.md:5`（禁止「正道/魔道阵营、掌门/长老/内门/外门弟子」层级）。reframe b 当时只覆盖了 dormant census / war 结算线（`#361` / `#363`），**漏掉了 hydrated NPC 的头顶浮字 + Inspect 面板这条线**——本 plan 补完这半截迁移。

---

## 接入面 Checklist

- **进料**：
  - `FactionMembership`（`server/src/npc/faction.rs:418`，字段 `faction_id: FactionId` / `rank: FactionRank` / `reputation: Reputation{loyalty}`）—— 这是当前具名展示的数据源
  - `NpcMetadataBuildInput`（`server/src/network/npc_metadata.rs:209`，含 `archetype` / `cultivation` / `membership` / `wounds` …）
  - `is_hostile_pair(FactionId, FactionId)`（`faction.rs:316`，Attack↔Defend 互敌）/ `are_hostile(EmergentGroupId, EmergentGroupId)`（`faction.rs:332`，`a != b`）—— 敌我判定，**保留为 server 内部 AI 逻辑，不进玩家可见 payload**
- **出料**：
  - `NpcMetadataS2c`（`npc_metadata.rs:29`）通过 `bong:npc_metadata` CustomPayload 每 20 tick 推给 64 格内 client（`npc_metadata.rs:185` / `emit_npc_metadata_payloads`，系统注册 `network/mod.rs:683`）
  - `display_name()`（`npc_metadata.rs:343`）还被 `client_request_handler.rs:6862` 复用（NPC 互动消息）—— 改 `display_name` 一处同时覆盖 payload + 互动文本
- **共享类型 / event 复用**：
  - 复用 reframe b 的 `region_descriptor` 公式 `format!("{zone}一带散修")`（`server/src/npc/war/mod.rs:386`）—— **不另造**匿名描述符格式
  - 复用 client reframe b 匿名化模式：`FactionWarHudPlanner.java`（甲方/乙方中性词）+ 测试正则 `.*(青云|盟|宗主|门派|宗门).*` 不命中（`FactionWarHudPlannerTest.java:148-151`）
- **跨仓库契约**：
  - server：`NpcMetadataS2c`（payload struct）/ `bong:npc_metadata`（CustomPayload type ID，不变）
  - client：`NpcMetadata.java`（record）/ `NpcMetadataHandler.java`（解析）/ `NpcInspectScreen.java` / `NpcDialogueScreen.java` / `NpcNametagRenderer.java` / `NpcDialogueBubbleRenderer.java` / `NpcLodWorldRenderer.java`
  - agent：**无需改**。`bong:npc_metadata` 在 agent/schema 侧无 TypeBox 定义、无 sample（client 手工解析）；`FactionId`/`FactionRank`（`world-state.ts:13-24`）是 attack/defend/neutral 三态内部枚举，与具名展示无关
- **worldview 锚点**：
  - **§一:4**「这里没有宗门收你」—— plan 核心宣言
  - **§十一:922-930** 匿名系统「修士之间默认不显示名字」
  - **§十一:1355**「玩家之间是默认敌对的陌生人」「合作有时比对抗更理性」
  - **§七:731-740** 散修利己 / **§十:864-872** 灵气零和（涌现群体的张力来源）
  - **Disciple 重定义依据**：worldview.md:1377 / 1462「失落宗门遗迹」「弟子佩物」—— 末法前宗门崩解后的遗民，当下是散修身份
- **qi_physics 锚点**：**无**。本 plan 不触碰真元 / 灵气流动，纯展示层 + 身份语义。敌我判定走已有 `is_hostile_pair` / `are_hostile`，不新增物理常数。

---

## P0 — server payload 匿名化：faction 退出玩家可见层 ⬜

**目标**：让 `NpcMetadataS2c` 与 NPC 互动文本**不再出现任何具名宗门字样**。faction 从"玩家可见标签"降级为"纯 server 内部 AI 敌我逻辑"。

**交付物（可核验）**：

- `server/src/network/npc_metadata.rs`：
  - `display_name()`（`:343`）：删除 `membership.is_some()` 时拼 `{faction_name}·{faction_rank}` 的分支；统一回退到匿名身份 `{archetype_label}·{realm}`（如「残宗余孽·凝脉」）。**此改动同时覆盖 `client_request_handler.rs:6862` 互动文本。**
  - `build_npc_metadata()`（`:223`）：`faction_name` / `faction_rank` 字段（`:241-242`）恒置 `None`（玩家不再看到 NPC 所属派系）。
  - 删除（或标记 `#[allow(dead_code)]` 仅测试用）`faction_name()`（`:395`）/ `faction_rank_label()`（`:403`）两个**产出具名字符串**的函数——它们只被上述两处调用。
  - `reputation_to_player`（`:240`）**保留**——这是 worldview 允许的"NPC 对我的态度"（§十一 NPC 反应分级），不是具名派系，玩家凭它感知敌我。
- 敌我逻辑不动：`is_hostile_pair` / `are_hostile` / `FactionId` enum / `FactionMembership` component 全部保留（NPC 间 big-brain DuelTarget AI 仍需要）。

**测试声明（饱和化）**：

- `npc_metadata` 模块新增 pin 测试：构造一个挂 `FactionMembership { faction_id: Attack, rank: Leader }` 的 NPC → 断言 `build_npc_metadata` 产出的 `NpcMetadataS2c` 序列化 JSON **不含** 正则 `魔修派|正道盟|中立盟|掌门|真传弟子|客卿`，且 `faction_name == None`、`faction_rank == None`。失败信息写明「期望 NPC payload 不暴露具名派系（reframe b 对齐），实际 display_name=...」。
- 三 `FactionId` 变体 × 三 `FactionRank` 变体逐一覆盖：任意组合的 `display_name` 都回退到 `{archetype}·{realm}` 形态，不含派系字样。
- 互动文本 pin：`client_request_handler.rs` 的 `npc_engagement_target_for` 产出的 `display_name` 同样不含具名派系（共用 `display_name()`，一条断言即可）。

**视听规格**（玩家可感知，必须写明）：

- **头顶 nameplate**（client 本地按 `displayName` 渲染，见 P2）：近距（<20m）原本显示 `[魔修派·真传弟子]` → 改为 `[残宗余孽·凝脉]`（即 `[{archetype_label}·{realm}]`）。本阶段只保证 server 不再发具名 `display_name`；client 渲染对齐在 P2。
- **NPC 互动消息**：原 `§7[NPC] 魔修派·真传弟子：…` → `§7[NPC] 残宗余孽·凝脉：…`。

---

## P1 — `Disciple` archetype 重定义为「残宗余孽」（server） ⬜

**目标**：消除「末法没有宗门，却有『宗门弟子』NPC」的世界观矛盾。**保留 Rust enum 名 `NpcArchetype::Disciple`**（避免牵动 spawn/skin/trade/equipment 大量已落地代码的大重构），只重定义其**展示语义 + 叙事物品**。

> **命名待 §8 #2 收口**：推荐「残宗余孽」（worldview.md:1462 失落宗门遗民，当下散修身份）。备选：「残派散修」「守墟人」。

**交付物（可核验）**：

- **纯 label 改（不动逻辑）**：
  - `server/src/network/npc_metadata.rs:364`：`NpcArchetype::Disciple => "宗门弟子"` → `=> "残宗余孽"`
- **叙事物品语义重审（动到逻辑，需判断）**：
  - `server/src/npc/loot.rs:83-87`：loot key `item.disciple.sect_token`（宗门信物）/ `item.disciple.sect_scroll`（宗门残卷）—— 「残宗余孽」掉落"失落宗门遗物"叙事上**仍成立**（遗民持有先宗遗物），可保留 item key，仅在 §8 #2 确认 item 显示名是否对齐新身份。
  - `server/src/npc/dormant/combat.rs:241-244`：`should_leave_relic` 的 `Disciple | GuardianRelic` 必留遗物分支 —— 「残宗余孽」死后留先宗遗物，叙事成立，**保留**。
  - `server/src/npc/spawn/disciple.rs:78` 文档注释「Spawn a Disciple (宗门弟子)」→ 更新为「残宗余孽」。
- **skin / 视觉不变**：`select_npc_visual_profile` / `disciple_skin_tier`（`skin/npc_skin_selector.rs:120/168`）的 `DiscipleLow/Mid/High` skin pool key 保留（只是 Rust 内部名，玩家不可见）——视觉外观不在本 plan 范围。

**测试声明**：

- `archetype_label(NpcArchetype::Disciple)` 返回值更新后，pin 测试断言 `== "残宗余孽"` 且不含「宗门」「弟子」字样（与 P0 正则共用）。
- loot / should_leave_relic 行为测试不变（语义重定义不改掉落概率与遗物规则）——保证"只换叙事，不动数值"。

**视听规格**：

- archetype 中文展示：全链路（nameplate / inspect / dialogue）「宗门弟子」→「残宗余孽」。具体 client 改点见 P2。

---

## P2 — client 渲染对齐 + reframe b 正则 pin + e2e ⬜

**目标**：client 侧所有 NPC 身份展示点与 P0/P1 对齐，并用 reframe b 风格正则断言锁死"不回潮"。

**交付物（可核验）**：

- `client/src/main/java/com/bong/client/npc/NpcInspectScreen.java:104-110` + `:183-187`（`describe()`）：
  - 当前渲染 `"派系: " + factionName + " / " + factionRank` 或 `"派系: 无"`。
  - **§8.1 #3 已定**：「派系」行改为「态度」行，按 `reputation_to_player`（-100..100，clamp）分级：`< -33` →「态度: 敌意」（红）、`-33..33` →「态度: 中立」（灰）、`> 33` →「态度: 亲善」（绿）。`describe()`（`:183-187`）同步改。
- `client/.../NpcDialogueScreen.java:198`：`case "disciple" -> "宗门弟子"` → `-> "残宗余孽"`。
- `client/.../NpcNametagRenderer.java:111`：archetypeIcon `"disciple" -> "宗"` → 改单字图标（如「余」或「残」，§8 #2 定）。
- `client/.../NpcLodWorldRenderer.java:79`：LOD 标识 `"disciple" -> "宗"` → 同上单字。
- `client/.../NpcDialogueBubbleRenderer.java:84`：气泡色 `"disciple" -> 0x6B3FA0`（紫）—— §8 #2 确认是否保留紫色身份色（建议保留，紫色不含宗门语义）。
- `NpcNametagRenderer.java:47`：近距渲染 `[displayName]` —— P0 已保证 server 发的 `displayName` 干净，client 无需额外遮蔽（确认即可）。

**测试声明（client，reframe b 模式复刻）**：

- `NpcScreenDescribeTest` / `NpcInspectScreen` 测试：断言 inspect describe 输出 **不命中** 正则 `.*(魔修|正道|宗门|掌门|真传弟子|内门|外门).*`（仿 `FactionWarHudPlannerTest.java:148-151`），失败信息「期望 NPC inspect 不含具名宗门字样（reframe b），实际: …」。
- `archetypeLabel("disciple")` 断言 `== "残宗余孽"`（更新现有 `NpcScreenDescribeTest.java:108`）。
- **e2e（端到端，不可省）**：server spawn 一个挂 `FactionMembership{Attack, Leader}` 的 disciple NPC → emit `bong:npc_metadata` → client 解析 → 断言 `NpcMetadataStore` 里该 NPC 的 `factionName == null` 且 `displayName` 匹配 `残宗余孽·.*`、不含具名派系。覆盖 server→payload→client 全链路。

**视听规格**：

- **nameplate 近距（<20m）**：`[残宗余孽·凝脉]`，白色 owo 默认。
- **nameplate 中距（20-40m）**：单字图标（§8 #2 定，如「余」），距离衰减。
- **inspect 面板**："派系"行移除或改"态度"行（§8 #3），颜色沿用 `COLOR_INFO = 0xD0D0D0`。
- **对话气泡**：`残宗余孽` 紫色 `0x6B3FA0`（建议保留）。

---

## ~~P3~~ — EmergentGroupId live 数据通路 ❌ 已取消（§8.1 #1 选方案 a）

§8.1 #1 选定**方案 (a) 完全匿名**：NPC 既不显示派系、也不显示「{zone}一带散修」涌现描述符，直接回退 `{archetype_label}·{realm}`。因此无需为 live NPC 补 EmergentGroupId 数据通路（hydration/dehydration 同步 + payload `region_descriptor` 字段），**本阶段整段取消**，plan 收敛为 3 PR。

> 原 P3 设计（dehydrate 当前在 `hydrate/mod.rs:297` 写死 `emergent_group=None`，需补 roundtrip + payload 字段）保留于本 commit 的 git 历史；若将来翻案改走方案 (b)，从此处恢复。

---

## 与既有 faction 骨架的关系（防孤岛说明）

`docs/plans-skeleton/` 下已有 `plan-faction-expansion-v1`、`plan-faction-wars-v1`、`plan-social-v2` 三份骨架，方向是**具名散修势力**（青云残峰猎人会式 + 领袖 NPC + 派系战争）。

- **本 plan 不并入这三份**：方向相反。`faction-expansion` 骨架重新解读 worldview（「玩家可挂靠具名散修势力」），但该解读与已**落地**的 reframe b §10.1 #6 决议（`plan-offscreen-war-v1.md:327`，"无具名宗门/组织化语义，未改 worldview"）**直接冲突，已被正典架空**。
- **建议**（交人工，本 plan 不擅自改他人骨架）：将 `faction-expansion-v1` / `faction-wars-v1` / `social-v2` 三份骨架标记为「⚠️ 待重审：方向与 reframe b 冲突」，或降级/废弃。若确要推进具名势力，须先人工修订 worldview.md + 推翻 reframe b 决议（属于 §8 #1 的「反向」选项，本 plan 范围外）。

---

## §8 开放问题（P0 决策门前需收口）

| # | 问题 | 推荐 | 影响 |
|---|------|------|------|
| 1 | NPC 身份显示方案：**(a) 完全匿名**（回退 `{archetype}·{realm}`，faction 退出 payload）vs **(b) 涌现描述符**（显示「{zone}一带散修」，需做 P3 数据通路） | **(a)** —— 最对齐 worldview §十一「默认不显示名字」，且免去 P3 复杂度，plan 收敛 3 PR | 决定 P3 是否启用、plan 总 PR 数 |
| 2 | `Disciple` 重定义后的中文身份名 + nameplate 单字图标 + 气泡色 | 「残宗余孽」/ 单字「余」/ 保留紫 `0x6B3FA0` | P1 label、P2 client 多处 |
| 3 | inspect 面板"派系"行：**移除** vs 改"态度"行（展示 `reputation_to_player`） | 改"态度"行（敌意/中立/亲善），保留信息量 | P2 `NpcInspectScreen` |
| 4 | 玩家侧 `FactionMembershipSnapshotV1.faction`（`agent/social.ts:61`，具名字符串，被天道 `query-player.ts:291` 暴露给 LLM）是否纳入本 plan 匿名化 | **不纳入**——属玩家 faction 挂靠（social-v1 §5），是 social-v2 范畴；本 plan 只管 **NPC 展示线**。标记为边界 | 划定 plan scope，避免越界 |

> **全部已在 §8.1 收口（2026-06-05，用户拍板）。§8 原表保留以备追溯，实施时以 §8.1 决议为准。**

---

## §8.1 决议（pre-P0 收口，2026-06-05）

### #1 NPC 身份显示方案 → 方案 (a) 完全匿名

**决议**：
1. NPC **不显示**任何派系信息，也**不显示**「{zone}一带散修」涌现描述符——直接回退 `{archetype_label}·{realm}`（如「残宗余孽·凝脉」）。最贴 worldview §十一:922「默认不显示名字」。
2. `build_npc_metadata` 中 `faction_name` / `faction_rank` 恒置 `None`；`display_name()` 删除 `membership.is_some()` 的具名拼接分支。
3. 拒绝方案 (b)：**P3 整段取消**，不补 EmergentGroupId live 数据通路。`reputation_to_player` 保留为唯一的玩家可感知敌我信号。

**落点**：`server/src/network/npc_metadata.rs:241-243`（faction 字段 + display_name 赋值）/ `server/src/network/npc_metadata.rs:343`（`display_name()` 函数）/ plan §P0 / plan §P3（取消）

### #2 Disciple 重定义命名（用户"随便" → 采用推荐）

**决议**：
1. 展示身份名「**残宗余孽**」（worldview.md:1462 失落宗门遗民依据）。
2. nameplate 中距 / LOD 单字图标「**余**」（原「宗」）。
3. 对话气泡色**保留紫** `0x6B3FA0`（紫色不含宗门语义，无需改）。
4. Rust enum 名 `NpcArchetype::Disciple` **不 rename**（避免大重构），仅改展示语义。

**落点**：`server/src/network/npc_metadata.rs:364`（archetype_label）/ `client/.../NpcDialogueScreen.java:198`（中文 label）/ `client/.../NpcNametagRenderer.java:111`（单字「宗」→「余」）/ `client/.../NpcLodWorldRenderer.java:79`（同）/ `client/.../NpcDialogueBubbleRenderer.java:84`（紫色保留，不改）/ plan §P1 §P2

### #3 inspect 面板「派系」行 → 改「态度」行

**决议**：
1. 删「派系: X / Y」行，改为「态度」行，按 `reputation_to_player`（-100..100，clamp）分级。
2. 阈值：`< -33` →「态度: 敌意」（红）、`-33..33` →「态度: 中立」（灰）、`> 33` →「态度: 亲善」（绿）。
3. `build()`（`:104-110`）与 `describe()`（`:183-187`）同步改，测试断言两路一致。

**落点**：`client/src/main/java/com/bong/client/npc/NpcInspectScreen.java:104-110` + `:183-187` / plan §P2

### #4 玩家侧 FactionMembershipSnapshot → 范围外（用户"可以"）

**决议**：
1. 玩家侧 `FactionMembershipSnapshotV1.faction`（具名字符串，被天道 `query-player.ts:291` 暴露给 LLM）**不纳入本 plan**。
2. 它属玩家 faction 挂靠（social-v1 §5），归 `plan-social-v2` 范畴；本 plan 只管 **NPC 展示线**。
3. 仅在接入面标记为已知边界，不动 `agent/` 任何代码。

**落点**：`agent/packages/schema/src/social.ts:61`（标记 out-of-scope）/ `agent/packages/tiandao/src/tools/query-player.ts:291`（标记 out-of-scope）/ plan 接入面「跨仓库契约」节

---

## §10 实施工作流（轻量，scope 3-4 PR）

- **PR 序列**（依赖顺序，前一个 merge 后开下一个，共 3 PR）：P0（server payload）→ P1（server archetype 重定义）→ P2（client + e2e）。P0/P1 均纯 server 逻辑可独立 review；P2 依赖前两者落地的契约。
- **纯逻辑代码**：按常规 atomic commit + 测试全绿，不适用建筑类 3 轮 `<PROMISE>`（本 plan 无 NBT / layout / 视觉资产产出）。
- **每 PR 独立 subagent 实施**（`subagent_type: "claude"` + `model: "opus"` + prompt 末尾 `ultrathink`），主线只收 result + 等 CR/Pi review（`ScheduleWakeup` 1200s/回合，≤3 回合，对齐 `docs/CLAUDE.md` §6.4/§6.5）。
- **CR + Pi + 内置 opus 对峙自检**：每 PR push 前跑对峙自检；外部 bot @ 触发按需。
- **单次 consume-plan 全自动到 merge**：用户提交 `/consume-plan` 后即可下班，醒来看本 plan 是否已迁入 `docs/finished_plans/`。

---

## Finish Evidence

### 落地清单

- **P0 — server payload 匿名化**（`server/src/network/npc_metadata.rs`）：
  - `build_npc_metadata()` 恒置 `faction_name = None` / `faction_rank = None`，faction 退出玩家可见 payload。
  - `display_name()` 删除 `membership.is_some()` 具名拼接分支，统一回退 `{archetype_label}·{realm}`（如「残宗余孽·凝脉」），同步覆盖 `client_request_handler.rs` NPC 互动文本。
  - `reputation_to_player`（-100..100）保留——worldview §十一 允许的「NPC 对我的态度」，非具名派系。
  - `is_hostile_pair` / `are_hostile` / `FactionId` / `FactionMembership` 全保留为 server 内部 big-brain 敌我逻辑，不进 payload。
- **P1 — `Disciple` 重定义「残宗余孽」**（`server/src/network/npc_metadata.rs` archetype_label + `server/src/npc/spawn/disciple.rs` 文档注释）：`NpcArchetype::Disciple` 展示名「宗门弟子」→「残宗余孽」（worldview.md:1462 失落宗门遗民依据）。loot/必留遗物分支叙事上仍成立，保留 item key。
- **P2 — client 渲染对齐 + reframe b 正则 pin**（`client/src/main/java/com/bong/client/npc/`）：
  - `NpcInspectScreen.java` `build()` + `describe()`：「派系」行 →「态度」行，按 `reputation_to_player` 分级（敌意 <-33 / 中立 / 亲善 >33）。
  - `NpcDialogueScreen.java` / `NpcNametagRenderer.java` / `NpcLodWorldRenderer.java` disciple 展示对齐「残宗余孽」/ 单字图标。

### 关键 commit

- `0fe08376e` · 2026-06-06 · `fix(npc): 匿名化 NPC faction 可见 payload`（P0）。
- `ba235d1d3` · 2026-06-06 · `fix(npc): 重定义 Disciple 展示身份为残宗余孽`（P1）。
- `d0aa808bc` · 2026-06-06 · `fix(client): 对齐 NPC 匿名身份展示`（P2）。
- （以上 3 commit 2026-06-07 rebase 至 origin/main 干净，hash 重写为当前值。）

### 测试结果

- server：`cd server && cargo test npc_metadata` → **12 passed; 0 failed**，含 reframe b pin 用例：
  - `disciple_archetype_label_is_ruined_sect_remnant`（P1 身份名 == 残宗余孽，不含「宗门」「弟子」）
  - `npc_metadata_hides_faction_fields_for_all_faction_rank_pairs`（P0 全 FactionRank 组合 faction 字段恒 None — 状态饱和）
  - `npc_metadata_reputation_is_player_specific`（reputation_to_player 保留且按玩家区分）
  - `realm_label_matches_worldview_canon`
- client：`cd client && ./gradlew test --tests "com.bong.client.npc.*"` → **BUILD SUCCESSFUL**（`NpcMetadataHandlerTest` / `NpcNametagRendererTest` / `NpcScreenDescribeTest` / `NpcLodWorldRendererTest` 共 81 个 @Test 全绿），含 reframe b 正则不命中 pin + e2e（server `FactionMembership{Attack,Leader}` disciple → payload → client 解析断言 `factionName == null` 且 `displayName` 匹配「残宗余孽·.*」）。

### 跨仓库核验

- **server**：命中 `build_npc_metadata` / `display_name` / `archetype_label` / `reputation_to_player` / `NpcArchetype::Disciple` / `is_hostile_pair` / `are_hostile`（保留）。
- **client**：命中 `NpcInspectScreen` / `NpcDialogueScreen` / `NpcNametagRenderer` / `NpcLodWorldRenderer`；CustomPayload `bong:npc_metadata` type ID 不变。
- **agent**：**无需改**——`bong:npc_metadata` 在 agent/schema 侧无 TypeBox/sample（client 手工解析），`FactionId`/`FactionRank` 为内部三态枚举，与具名展示无关。

### 遗留 / 后续

- P3（`EmergentGroupId` live 数据通路）已于 2026-06-05 §8.1 #1 取消（选定方案 a 完全匿名），不在本 plan 范围。
- 骨架 `faction-expansion-v1` / `faction-wars-v1` / `social-v2` 方向（玩家可挂靠具名散修势力）与已落地 reframe b §10.1 #6 决议直接冲突，**交人工**决定是否标记「⚠️ 待重审」或废弃——本 plan 不擅自改他人骨架（见 §关联）。
