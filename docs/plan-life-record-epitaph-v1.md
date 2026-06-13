# Bong · plan-life-record-epitaph-v1 · active

**一生记录·遗念碑刻**——玩家在「最终死亡」（寿元耗尽/运数到头，无法再生）后，服务端在其最后一处灵龛坐标自动 stamp 一块「遗念碑」structure（刻有生平摘要），世界上任何修士路过时均可俯身查阅，获得死者的位置遗念（「此处东 200 格有地下洞穴」）或战斗摘要，也作为 Tiandao agent 叙事素材（"此地曾有一名通灵修士陨落"）。

## 目标

- 实装 worldview §十二「一生记录的用途」：最终死亡 → 遗念碑置入世界，记录玩家生平关键事件摘要（最高境界 / 击杀数 / 采集路线 / 最终遗念）
- 工程目标：复用 `LifeRecord`（`server/src/cultivation/life_record.rs`，1012 行，完整实装），仅新增世界置入逻辑（`EpitaphBlock` + `EpitaphRegistry`）+ client 交互 UI
- 验收：运数耗尽死亡 → 最后灵龛坐标产生遗念碑 → 其他玩家查阅 → 显示死者生平摘要 → agent 可读取 epitaph 列表 e2e 闭环

**来源**：`worldview.md §十二:1108 一生记录的用途`（"亡者博物馆 + 记录的用途"：坐标 tip、击杀/死亡比、顿悟记录、最终境界、最终遗念）+ `worldview.md §十二:1153 不可篡改`（死后生平卷"永久不可篡改"，碑刻只读）+ `agent/packages/tiandao/src/skills/era.md §新一世开场叙事`（era.md 已约定 agent 可引用亡者博物馆条目）

**前置条件**：
- `plan-death-lifecycle-v1` ✅ — `PlayerDeathEvent` + 玩家最终死亡事件链路（`is_final_death` 已标注）
- `plan-multi-life-v1` ⏳ active (~8%) — 多生多世系统（`FinalDeathEvent`、`LifespanCapTable`、`per-life luck_pool` 是本 plan 的上游触发器）
- `plan-persistence-v1` ✅ — SQLite 持久化层（`LifeRecord` 数据已落 DB）
- `plan-cultivation-v1` ✅ — `LifeRecord`（完整实装：1012 行，含 encode/decode/test）
- `plan-worldgen-v3` ✅ — `StructureStamper` 或等效 structure placement API（遗念碑是小型 NBT structure）
- `plan-niche-defense-v1` ✅ — 灵龛坐标数据（死者最后灵龛坐标作为碑刻放置锚点）

**交叉引用**：`plan-multi-life-v1` ⏳（`FinalDeathEvent` 触发器，**此 plan 的 P0 强依赖，须等 multi-life-v1 P0 landing**）· `plan-agent-v2` ✅（agent `world_state.epitaphs` 列表，era.md 已约定可引用但未实装数据字段）· `plan-daozhan-v1` ⬜（道伥的 origin_realm 可与死者遗念碑绑定——同一玩家最终死亡 → 可化为高级道伥 + 留碑刻，双效并存）

**worldview 锚点**：
- **§十二:1108 一生记录**：记录内容（高点 / 击杀 / 顿悟 / 寿元使用 / 最终遗念）；"从未来修士视角看前人的判断"
- **§十二:1141 记录的用途**：坐标 tip（死前 300 格内遗念）/ 经济数据（击杀/死亡比）/ 路线（哪条路常走）/ 最终境界
- **§十二:1148 不可篡改**：死后碑文只读，玩家不能修改他人或自己的死后碑刻
- **§十二:1153 遗念**：最终死亡时可自选一条「遗念」（位置提示 / 仇人名 / 最后顿悟内容）刻入碑文，其他玩家可读取

**qi_physics 锚点**：碑刻本身不含真元，无 qi_physics 依赖。

---

## 接入面 Checklist

- **进料**：`FinalDeathEvent { player, last_niche_pos, life_record_snapshot: LifeRecordSnapshotV1 }`（multi-life-v1 P0 产出，本 plan 依赖）+ `LifeRecord` 数据（`persistence-v1` SQLite 存储，通过 `LifeRecordSnapshotV1` 序列化）+ 玩家最后选择的「遗念」（`FinalThought { kind: LocationHint | RevengeHint | InsightHint, content: String }`，在最终死亡前 60s 内玩家可提交）
- **出料**：`WorldEpitaphRegistry { entries: HashMap<EpitaphId, EpitaphEntry> }` Bevy Resource + `PendingFinalThoughtStore { HashMap<PlayerId, FinalThought> }` Bevy Resource（缓存 `FinalDeathEvent` 之前提交的遗念，生成碑刻时 consume）+ `EpitaphEntry { id, player_name, final_realm, death_tick, niche_pos, record_summary: LifeRecordSummary, final_thought: FinalThought }` + 世界坐标处的 `EpitaphBlock` structure（小型 NBT，3×1×1）+ agent `world_state.epitaphs` 字段（最近 10 条摘要）
- **共享类型**：新增 `EpitaphId`（UUID 字符串）+ `LifeRecordSummary { peak_realm, total_kills, total_deaths, signature_skill_ids, final_zone }` + `FinalThought`；复用 `LifeRecordSnapshotV1`（已存在 `server/src/schema/cultivation.rs:83`）
- **跨仓库契约**：server `bong:world_state` 新增 `epitaphs: EpitaphEntry[]`（最近 N 条）→ agent `world-model.ts` 增加 `epitaphs` 字段（era.md skill 已引用但数据未接）；client `EpitaphInspectS2c` CustomPayload（查阅请求 + 响应）+ `FinalThoughtSubmitC2s`（临终遗念提交）
- **worldview 锚点**：§十二 一生记录 + 遗念 + 不可篡改
- **qi_physics 锚点**：无

---

## 阶段总览

| 阶段 | 状态 | 主要交付物 | 验收标准 |
|------|------|-----------|---------|
| **P0** | ⬜ | `EpitaphEntry` 数据模型 + `WorldEpitaphRegistry` + `FinalDeathEvent` → 碑刻生成系统（Server 内存 + SQLite 持久化）| multi-life-v1 触发最终死亡 → EpitaphRegistry 有新条目 + ≥ 8 单测 |
| **P1** | ⬜ | `EpitaphBlock` structure 置入世界（最后灵龛坐标）+ `FinalThoughtSubmitC2s` 临终遗念提交 | 最终死亡 → 灵龛坐标产生遗念碑方块；存活玩家可查看到碑刻 |
| **P2** | ⬜ | Client 查阅 UI（`EpitaphInspectS2c`）：右键碑 → 显示生平摘要面板（最高境界/击杀数/遗念） | 查阅全链路 e2e；面板内容与 LifeRecordSummary 对齐 |
| **P3** | ⬜ | Agent 接入：`world_state.epitaphs` 字段 + era.md "新一世开场叙事"引用亡者博物馆条目 | agent narration 可引用最近一位陨落修士；era_decree 叙事有碑刻素材 |

---

## P0 — 数据模型与碑刻生成

- [ ] `server/src/cultivation/epitaph.rs`（新文件）：`EpitaphId(String)` + `LifeRecordSummary { peak_realm, total_kills, total_deaths, signature_skill_ids, final_zone: ZoneId }` + `FinalThought { kind: LocationHint | RevengeHint | InsightHint, content: String }` + `EpitaphEntry`
- [ ] `WorldEpitaphRegistry { entries: IndexMap<EpitaphId, EpitaphEntry>, max_cap: 1000 }` Bevy Resource（超出 cap 时删除最旧的，但 SQLite 永久保留）
- [ ] `EpitaphGenerationSystem`：监听 `FinalDeathEvent`（multi-life-v1 P0 产出）→ 从 `LifeRecord` 提取 `LifeRecordSummary` → 从 `PendingFinalThoughtStore`（P1）`remove(player_id)` 取出预提交遗念（无则 `FinalThought::None`）→ 创建 `EpitaphEntry` → 写 `WorldEpitaphRegistry` → 持久化 SQLite `epitaphs` 表
- [ ] ≥ 10 单测（LifeRecordSummary 字段提取正确 / EpitaphRegistry 写入 / SQLite round-trip / cap 1000 淘汰最旧）

---

## P1 — 世界置入 + 临终遗念提交

- [ ] `EpitaphBlock` NBT structure（`worldgen/structures/epitaph_stone.nbt`）：3×1×1 雕刻石 + 中心块带 `custom_data: { epitaph_id }` 的 CustomBlockEntity；视觉：石碑竖立，表面雕刻纹理（无法用原版 NBT tag 写汉字，由 client 读 epitaph_id 后查询内容渲染）
- [ ] `EpitaphPlacementSystem`：在 `FinalDeathEvent.last_niche_pos` 附近 5 格半径内寻找有效放置地点（避免悬空 / 水中）→ 调用 `StructureStamper::stamp_at(epitaph_stone, pos)`
- [ ] `PendingFinalThoughtStore { HashMap<PlayerId, FinalThought> }` Bevy Resource：**遗念在 `FinalDeathEvent` 之前提交，此时 `EpitaphEntry` 尚未创建**，故先缓存到此 store；P0 的 `EpitaphGenerationSystem` 在收到 `FinalDeathEvent` 创建 `EpitaphEntry` 时 `remove(player_id)` 取出 pending thought 合并入碑刻，无 pending 则写 `FinalThought::None`
- [ ] `FinalThoughtSubmitC2s` CustomPayload：玩家在"濒死倒计时"（multi-life-v1 最终死亡前 60s）内可发送遗念；server 写入 `PendingFinalThoughtStore`（**非** `EpitaphEntry`，后者尚不存在）；窗口外提交拒绝；多次提交覆盖（取最后一次）；玩家下线/死亡取消时清理 pending
- [ ] ≥ 8 单测（structure 放置坐标不悬空 / 窗口内提交进 PendingStore / FinalDeathEvent 时 consume pending 合并入 EpitaphEntry / 无 pending → FinalThought::None / 重复提交覆盖 / 提交后下线清理 / 碑刻位于正确 zone）

---

## P2 — 客户端查阅 UI

- [ ] `EpitaphInspectC2s { epitaph_id }` + `EpitaphInspectS2c { entry: EpitaphEntry, summary: LifeRecordSummary }` CustomPayload
- [ ] client 触发：右键 `EpitaphBlock` custom block entity → 发 `EpitaphInspectC2s` → 收 `EpitaphInspectS2c` → 打开查阅面板
- [ ] 查阅面板（OwoUI Component）：`EpitaphPanel { name, realm, kills/deaths, signature_skills, final_zone_name, final_thought, death_tick_display }`；面板只读，无交互按钮；风格：古风石碑排版，深灰背景，竖向文字
- [ ] ≥ 8 单测（C2s / S2c 格式校验 / 面板字段映射 / 右键非碑刻方块不触发）

---

## P3 — Agent 接入与叙事

- [ ] `world_state.epitaphs: EpitaphEntryV1[]`（最近 10 条，按 `death_tick` 倒序）加入 IPC schema `WorldStateV1`（TypeBox 定义 + JSON sample）
- [ ] `server/src/schema/world_state.rs` 同步 `EpitaphEntryV1` struct
- [ ] agent `world-model.ts` 解析 `epitaphs` 字段；era.md "新一世开场叙事"可从 `epitaphs[0]` 取前世修士最高境界做叙事素材（已有文字约定，本阶段接数据）
- [ ] narration 模板 2 条（含入 era.md 注释）：例"血谷昨有通灵修士陨落，碑刻犹存，路人可自取遗念" / "无名者之碑，刻着三字——莫往北"（遗念 LocationHint 示例）
- [ ] ≥ 6 双端 schema 校验单测（EpitaphEntryV1 正反 sample）

---

## §8 开放问题（P0 决策门收口）

1. **依赖 multi-life-v1 P0 timing**：本 plan P0 依赖 `FinalDeathEvent`，但 multi-life-v1 仍 active ~8%。是否先以 `PlayerDeathEvent { is_multilife_terminal: true }` stub 提前开始 P0？需 multi-life-v1 开发者确认
2. **碑刻数量上限**：每个玩家最终死亡置 1 块碑；但玩家可多周目（每周目一块）——活跃服务器可能遍地是碑。建议每玩家最多保留最新 3 块碑，老碑在世界中消失（但 SQLite 永久存档）
3. **碑刻位置选择**：最后灵龛坐标 vs 实际死亡坐标。灵龛坐标更有意义（玩家"经营"的据点），死亡坐标更有戏剧性（战场/险地）。建议：以死亡坐标为主；若死亡在坍缩渊（负灵域/另一维度），则 fallback 到最后灵龛坐标
4. **遗念内容格式**：`FinalThought::LocationHint` 的 `content` 字段是坐标字符串还是语义描述？明确坐标（"X:-2400, Z:800"）vs 隐语（"血谷东侧 200 格的地下"）——建议 LocationHint 存坐标但 client 以隐语渲染，不直接暴露数字
5. **碑刻不可破坏**：设计意图是"永久遗物"，但 Minecraft 原版任何方块都可以被破坏。是否给 EpitaphBlock 加 `unbreakable` flag？建议是；但需确认 Valence 如何实现方块不可破坏（自定义 CustomBlock 或 Bedrock block 类型）

全部已在 §8.1 收口。原表保留以备追溯，**实施时以 §8.1 决议为准**。

## §8.1 决议（pre-P0 收口，2026-06-13）

### #1 真实终死亡信号（P0 核心接入面）

**决议**：
1. P0 使用真实存在的 `PlayerTerminated` event，而非 plan 文档虚构的 `FinalDeathEvent`。
2. `PlayerTerminated` 由 `combat/lifecycle.rs:1532` 和 `:1572` 在不可再生路径 emit；`death_hooks.rs:54` 声明该 struct。`EpitaphGenerationSystem` 监听 `EventReader<PlayerTerminated>`，通过 `Without<NpcMarker>` query filter 排除 NPC entity，只为玩家生成碑刻。
3. plan §P0 中所有 `FinalDeathEvent` 引用均已由 `PlayerTerminated` 替代——P0 已实装，信号命名已收口，后续阶段不再沿用 plan 原虚构名。

**落点**：`server/src/cultivation/death_hooks.rs:54`（struct 声明）·`server/src/combat/lifecycle.rs:1532,1572`（emit 点）·`server/src/cultivation/epitaph.rs:262-279`（EpitaphGenerationSystem 监听 + Without<NpcMarker>）

### #2 碑刻数量上限（P1）

**决议**：属 P1 世界置入范围，P0 不实装。`WorldEpitaphRegistry` 内存上限已实装为 1000 条（`registry_cap_exactly_1000_does_not_evict` 测试覆盖），SQLite 永久存档不受上限影响；每玩家展示限制待 P1 设计。

**落点**：`server/src/cultivation/epitaph.rs`（P0 已落，P1 扩展）

### #3 碑刻位置选择（P1）

**决议**：属 P1 `EpitaphPlacementSystem` 范围，P0 不实装。死亡坐标优先、坍缩渊 fallback 灵龛坐标的逻辑待 P1 立项时落地。

**落点**：plan §P1（P1 实施时补落点）

### #4 遗念内容格式（P1）

**决议**：属 P1 临终遗念提交范围。`FinalThought` struct 已在 P0 定义（`LocationHint/RevengeHint/InsightHint/None`），格式细节（坐标字符串 vs 语义描述）待 P1 实装时决议。

**落点**：`server/src/cultivation/epitaph.rs`（FinalThought struct，P1 扩展格式）

### #5 碑刻不可破坏（P1）

**决议**：属 P1 `EpitaphBlock` 世界置入范围，P0 无 block 实体，不实装。Valence 实现方式待 P1 调研。

**落点**：plan §P1（P1 实施时补落点）
