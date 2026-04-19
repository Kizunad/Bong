# Bong · plan-persistence-v1

**存档 / 持久化专项**。统一 server / agent 两侧的落盘规范，让玩家数据、世界状态、agent 推演都能跨 server 重启。

**交叉引用**：`plan-death-lifecycle-v1.md §7`（LifeRecord / Lifespan 契约）· `plan-tribulation-v1.md §6`（TribulationState / AscensionQuotaStore）· `plan-npc-ai-v1.md §6`（FactionStore / NpcDigest / NpcRegistry）· `plan-alchemy-v1.md`（recipe JSON）· `worldgen/`（raster 只读）。

---

## §-1 现状（已实装，本 plan 不重做）

| 层 | 已有 | 备注 |
|---|---|---|
| **PlayerState JSON** | `data/players/offline:{username}.json` · 自动 60s + 登出强存 | realm / spirit_qi / karma / experience / inventory_score |
| **zones.json 读取** | 静态配置，无写回 | — |
| **Schema TypeBox** | `agent/packages/schema`，33 samples 覆盖 IPC | 导出 JSON Schema → Rust serde |
| **Agent WorldModel** | Redis hash `bong:tiandao:state` · `loadWorldModelState()` / `saveWorldModelState()` | **无 AOF/RDB 配置**；Redis 挂 = 历史全丢 |
| **Raster mmap** | `memmap2` 只读 16 层语义字段 | 运行时不写 |

**尚未实装**（本 plan 目标）：
- LifeRecord 落盘（当前纯内存）
- NPC / Faction / Zone runtime 状态（灵气值、Reputation、Lifespan、DeathRegistry）
- TribulationState / AscensionQuota / QuestLog
- Agent WorldModel 的权威存储（脱离 Redis 单点）
- 崩溃恢复 / WAL / 版本迁移
- 玩家背包的完整序列化

---

## §0 设计轴心

- [ ] **单一权威 = server**：玩家/世界/NPC 状态由 server 落盘；agent 只做"推演快照"读写
- [ ] **SQLite 为主 + JSON 只读配置 + Redis 仅作缓存**（详见 §2）
- [ ] **事件驱动 + 节流快照**：语义事件（突破/死亡/渡劫/夺舍）立即 commit；高频状态（位置/HP/真元）按节流写（见 §3）
- [ ] **ACID 兜底**：单 SQLite 文件 WAL 模式，单机单进程写，天然避免并发冲突
- [ ] **版本字段强制**：所有持久化结构带 `schema_version`，无则拒加载
- [ ] **可恢复 ≠ 零丢失**：崩溃最多丢 N 秒实时操作（N = 快照间隔），但语义事件不丢

## §1 存档范围

| 数据类型 | 后端 | 写入策略 | 负责 plan |
|---------|------|---------|----------|
| PlayerState | SQLite `players` 表 | 节流 60s + 登出立即 | （已有，待迁移） |
| PlayerInventory | SQLite `inventories` 表 | 节流 60s + 物品变动立即 | — |
| LifeRecord（完整生平卷） | SQLite `life_records` + `life_events` 表 | 语义事件 append | plan-death §7 |
| DeathRegistry | SQLite `death_registry` | 死亡立即 | plan-death §7 |
| LifespanEvent | SQLite `lifespan_events` append-only | 每事件立即 | plan-death §4c |
| NPC state（active） | SQLite `npc_state` | 节流 60s + spawn/death 立即 | plan-npc-ai §6 |
| NpcDigest（远方） | SQLite `npc_digests` | agent 推演时更新 | plan-npc-ai §6 |
| FactionState | SQLite `factions` + `reputation` + `membership` | 事件立即 | plan-npc-ai §4 |
| ZoneRuntime（灵气值 / 状态）| SQLite `zones_runtime` | 节流 5min + 域崩立即 | — |
| TribulationState（进行中）| SQLite `tribulations_active` | 阶段切换立即 | plan-tribulation §6 |
| AscensionQuota | SQLite `ascension_quota` 单行 | 名额变动立即 | plan-tribulation §3 |
| QuestLog | SQLite `quest_log` + `quest_progress` | 接/交/失败立即 | plan-quest-v1 |
| Agent WorldModel | SQLite `agent_state` · Redis hash 镜像 | 推演周期 + 关键决策 | plan-agent-v2 |
| QuickUseSlotStore | SQLite `player_ui_prefs` | 玩家改配置立即 | plan-HUD |
| zones.json / recipes | 文件只读 | — | 静态配置 |
| raster（地形） | mmap 只读 | — | worldgen |
| 亡者博物馆快照 | SQLite `deceased_snapshots` + library-web 静态导出 | 角色终结立即 | plan-death §5 |
| Relationship 稀疏图 | SQLite `relationships` | 建立/解除立即 | plan-social §3 |
| ExposureLog | SQLite `exposures` append-only | 暴露事件立即 | plan-social §1 |
| Renown | SQLite `renown` | 事件立即 | plan-social §4 |
| SpiritNiche | SQLite `spirit_niches` | 放置/揭露立即 | plan-social §2 |

## §2 存储后端

### §2.1 主存储：SQLite（WAL 模式）

- [ ] `rusqlite` + `r2d2` 连接池
- [ ] 单文件 `data/bong.db` + `bong.db-wal` + `bong.db-shm`
- [ ] WAL 模式：并发读 + 单写；崩溃时 WAL 自动回放
- [ ] 每张表带 `schema_version INTEGER NOT NULL`
- [ ] 语义事件表全部 append-only（life_events / lifespan_events / death_registry / quest_log）
- [ ] 状态表允许 update（players / npc_state / zones_runtime）
- [ ] 业务层不直接写 SQL，走 `*Repo` trait（抽象层）

### §2.2 配置只读：JSON / TOML

- [ ] `data/config/zones.json` / `data/config/recipes/*.json` / `data/config/factions_init.json`
- [ ] 启动时加载到内存，运行时不写回
- [ ] git 跟踪，版本跟代码走

### §2.3 缓存 / IPC：Redis

- [ ] **不做权威存储**——Redis 只作为 pub/sub + agent 短期缓存
- [ ] Agent WorldModel 在 Redis hash 镜像，**权威在 SQLite**（Agent 启动时先读 SQLite 再同步 Redis）
- [ ] 启用 **AOF everysec**，降低 Redis 崩溃数据丢失窗口；但 SQLite 才是 source of truth
- [ ] Redis 重启后 agent 自动从 SQLite 重放

### §2.4 大文件：mmap / 文件树

- [ ] Raster：保留现有 mmap 只读
- [ ] library-web 静态站：写 `library-web/public/deceased/{char_id}.json`（亡者博物馆）
- [ ] 玩家截图 / agent 长文本 narration 归档：`data/archive/` 按日分目录

### §2.5 序列化

- [ ] SQLite 行内字段：TEXT (JSON 字符串) for 复杂嵌套，Integer/Real for 数值
- [ ] **通道传输**：保留 JSON（与 TypeBox schema 对齐），不引入 bincode
- [ ] 档案导出格式：JSON（可读 > 紧凑）

## §3 写入策略

### §3.1 事件驱动（立即 commit）

**语义事件，丢一条 = bug**：
- 突破 / 死亡 / 老死 / 夺舍 / 渡虚劫起/结算 / 化虚 / NPC 死亡 / 派系战争宣告 / 任务接/交/失败 / 炼丹成品出炉

```rust
trait EventJournal {
    fn append<E: SerializeEvent>(&self, event: E) -> Result<EventId>;
}
```

- [ ] 单事务内完成 "append event + update state"（原子）
- [ ] 失败 → 上层拒绝该行为（炼丹失败 → 不消耗材料）

### §3.2 节流快照（高频状态）

**高频变动，丢 60s 可接受**：
- 玩家位置 / HP / 真元 / stamina / 经脉进度
- NPC 位置 / blackboard / 行为状态
- Zone 灵气值

- [ ] `ThrottledWriter { last_write: Tick, min_interval: Tick }`
- [ ] 默认 60s (玩家) / 60s (活跃 NPC) / 300s (Zone 灵气)
- [ ] **登出 / server shutdown 强制 flush 所有节流 writer**

### §3.3 WAL / 事务

- [ ] SQLite 原生 WAL + 事务即可满足
- [ ] 业务层不做二次 WAL（KISS）
- [ ] 多表联动（如夺舍：写双方生平卷 + 玩家 state 变更）→ 单事务包裹

## §4 崩溃恢复

### §4.1 启动时

1. 打开 SQLite，WAL 自动回放
2. 加载 PlayerState（仅"当前在线过的角色"先加载即可，延后加载节省时间）
3. 加载 Zone runtime、NPC active state、Faction、Ascension quota、进行中 TribulationState
4. **进行中渡虚劫的恢复策略**：若 server 在劫波中途崩溃 → 判定为"天意所归"，当前波次直接视为通过（不惩罚玩家）
5. Agent WorldModel：先读 SQLite 快照 → 回填 Redis → 订阅 channel 继续推演

### §4.2 数据损坏检测

- [ ] SQLite 开 `PRAGMA integrity_check` 启动自检
- [ ] 失败 → 启动 halt，提示手动介入
- [ ] 关键表加 CHECK constraint（寿元 ≥ 0、境界 ∈ 0..=5）

### §4.3 部分失败策略

- [ ] PlayerState 损坏 + 其他完好 → 该玩家禁止登录，其他继续
- [ ] Agent 状态损坏 + server 完好 → agent 从空白 WorldModel 开始推演（少量历史丢失，不影响玩家体验）
- [ ] Zone runtime 损坏 → 回退到 zones.json 默认值 + narration 宣告"灵脉回归初始"

## §5 版本迁移

### §5.1 schema_version 字段

- [ ] 每条持久化记录必带 `schema_version: u32`
- [ ] 加载时若 version < current → 跑 migration 链；若 > current → 拒绝加载（旧 server 读新档）

### §5.2 migration 脚本

- [ ] `server/src/persistence/migrations/vN_to_vN+1.rs`
- [ ] 启动时顺序执行所有未应用的 migration
- [ ] SQLite `user_version` PRAGMA 记录当前 schema version
- [ ] migration 必须 idempotent（重跑无害）

### §5.3 向前兼容

- [x] TypeBox schema 也对齐 `v` 字段（已有）
- [x] IPC message 收到 unknown `v` → 丢弃并 warn，不 panic

## §6 备份

### §6.1 本地自动备份

- [ ] server 启动时自动快照 `data/bong.db` → `data/backups/bong-{YYYYMMDD-HHMM}.db`
- [ ] 每日午夜自动快照一份
- [ ] 保留最近 7 份，其余按日期清理

### §6.2 导出 / 导入（**admin-only**，不面向玩家）

- [ ] CLI 子命令：`bong-server export --player <name>` → JSON 包
- [ ] CLI 子命令：`bong-server import --file <path>`（仅 dev 模式）
- [ ] **无玩家可触达路径**——server 不直接暴露给玩家，CLI 仅运维/开发使用；无脱敏需求

### §6.3 亡者博物馆导出

- [ ] 终结事件触发时同时写 `library-web/public/deceased/{char_id}.json`
- [ ] 站点重新 `npm run build` 即纳入

## §7 实施节点

**Phase 0 — SQLite 基础设施**
- [ ] `persistence/` 模块：`rusqlite` + `r2d2` 连接池
- [ ] 启动时打开 + `PRAGMA integrity_check` + WAL
- [ ] migration 框架（`user_version` + 按序执行）
- [ ] UUIDv7 生成器（`uuid` crate v7 feature）作为 CharId/FactionId/QuestId/RelicId 统一来源
- [ ] 多态事件表模板（§10.2）：`EventRepo<E>` trait + JSON payload + version
- [ ] 双时间字段注入 util（`game_tick` + `wall_clock`）
- [ ] 单元测试：打开/关闭/事务回滚/WAL 恢复 / UUID 有序性

**Phase 1 — 玩家数据迁移**
- [ ] `player_core`（战斗关键，5s 节流）+ `player_slow`（位置/UI，60s 节流）+ `inventories` + `player_ui_prefs`
- [ ] 从 `data/players/*.json` 一次性迁移，为每个玩家生成 `current_char_id UUIDv7`
- [ ] 原 JSON 文件保留为 `*.json.migrated` 供回滚
- [ ] 写路径：事件驱动 HP/真元 → `player_core`；位置节流 → `player_slow`；登出 flush 全部

**Phase 2 — Life / Death / Lifespan**
- [ ] `life_records` / `life_events` / `death_registry` / `lifespan_events` 表
- [ ] 对接 plan-death §7 数据契约
- [ ] 所有语义事件 append-only
- [ ] `deceased_snapshots` + library-web 导出钩子

**Phase 2b — NPC 老死分层归档 + NpcDigest 淘汰**
- [ ] `npc_deceased_index` 表（char_id / archetype / died_at / path）
- [ ] 终结触发打包 → `data/archive/npc_deceased/{year}/{char_id}.json.zst`
- [ ] zstd 压缩（`zstd` crate）
- [ ] 查询接口：按 index 找 path，按需解压（亡者博物馆不收 NPC，主要供 agent 回溯）
- [ ] 启动时扫描孤儿文件（存在文件但 index 缺失 → warn）
- [ ] **§10.7 Digest 淘汰**：`npc_digests.last_referenced_wall` 字段 + 每周 cron 扫描 180 天未用 → 归档 `data/archive/npc_digests/`
- [ ] **§10.10 library-web 增量**：终结事件同时 `public/deceased/{char_id}.json` + 更新 `public/deceased/_index.json`；静态站点改为前端 fetch，不依赖构建

**Phase 3 — NPC / Faction**
- [ ] `npc_state` / `npc_digests` / `factions` / `reputation` / `membership`
- [ ] NPC 补员从 digest 反向加载
- [ ] Faction 关系矩阵落盘

**Phase 4 — Zone / Tribulation / Quest**
- [ ] `zones_runtime`（Zone 灵气值节流 5min）
- [ ] **`zone_overlays`（§10.9 可变性层）**：域崩 / 灵眼形成 / 遗迹显现 等 overlay 事件
- [ ] `tribulations_active`（阶段切换立即）
- [ ] `ascension_quota` 单行
- [ ] `quest_log` / `quest_progress`（若 plan-quest 已启动）

**Phase 5 — Agent WorldModel**
- [ ] `agent_state` 多表（`agent_eras` / `agent_decisions` append-only）
- [ ] Agent 启动时 SQLite → Redis 同步流程
- [ ] **单向写**（§10.8）：Agent → SQLite → publish Redis；Redis 发布失败告警不回滚
- [ ] 订阅侧 5 min reconcile 从 SQLite 对账
- [ ] Redis 挂掉重启后的自愈测试

**Phase 6 — 崩溃恢复 + 备份**
- [ ] 启动时自动快照 → `data/backups/`
- [ ] 每日午夜 cron
- [ ] 保留最近 7 份
- [ ] 渡虚劫中途崩溃的"天意所归"判定

**Phase 7 — 版本迁移演练**
- [ ] 故意造一次 schema 变更（加字段），验证 migration
- [ ] 回退测试（旧 server 读新档拒绝加载的表现）

**Phase 8 — CLI 导出 / 导入**
- [ ] `bong-server export` / `import` 子命令
- [ ] dev 模式校验

**Phase 9 — 性能回归**
- [ ] 1000 NPC + 50 玩家 同时节流写 SQLite 的开销
- [ ] 语义事件峰值（同时 10 人死亡）的事务冲突
- [ ] 必要时加 `PRAGMA synchronous=NORMAL`（默认 FULL）

## §8 已决定

- ✅ **SQLite 为主存储**（WAL 模式 + rusqlite + r2d2）
- ✅ **JSON 做配置只读**（zones / recipes / factions_init）
- ✅ **Redis 仅缓存 + IPC**，不做权威存储；agent 权威在 SQLite
- ✅ **事件驱动 + 节流快照双轨**：语义事件立即 commit，高频状态 60s 节流
- ✅ **登出 / shutdown 强制 flush 节流 writer**
- ✅ **schema_version 强制**，旧读新直接拒绝
- ✅ **migration 顺序执行 + idempotent**
- ✅ **备份**：启动 + 每日自动快照，保留 7 份
- ✅ **玩家不可自导出**（防开档复活 / 物品复制）
- ✅ **渡虚劫中途崩溃**：当前波次视为通过（天意所归，不惩罚）
- ✅ **部分损坏隔离**：一个玩家档损坏不影响其他人
- ✅ **IPC 通道继续用 JSON**（不引入 bincode，保持可读性与 TypeBox 对齐）
- ✅ **Server 不直接暴露给玩家**：玩家只通过 client / library-web 间接交互；任何"玩家可见"的导出路径不经 server CLI，SQLite 连接/文件不对外暴露
- ✅ **NPC 老死分层归档**（§9.2）：
  - SQLite `life_records` 仅保留活跃 NPC
  - 终结 NPC 打包 `data/archive/npc_deceased/{year}/{char_id}.json.zst`（zstd 压缩 5-10x）
  - SQLite 留 `npc_deceased_index` 指针（char_id / archetype / died_at / path）
  - 配套加 Phase 2b：归档任务 + 压缩管道
- ✅ **CLI export/import 为 admin-only**（无玩家可触达路径，无脱敏需求）
- ✅ **异地备份 / Agent 历史保留**：不在本 plan 范围，由运维 / plan-agent-v2 承接

## §9 剩余开放问题

_（无未决项，所有设计问题均已收口）_

**已排除范围**（非本 plan 职责）：
- 异地备份 / 对象存储同步 → 靠外部运维工具（restic / borg / rsync），本 plan 不做
- Agent 推演历史保留时长 → 交由 `plan-agent-v2` 决定；本 plan 只提供表存储，容量策略由上层定
- 导出脱敏 → server 不面向玩家，CLI 是 admin-only，无脱敏需求

## §10 前瞻规划（数据设计的未来风险）

_review §1-§3 后识别的潜在演化风险；每条给出**现在就该做的约束**，避免 v2 搬家。_

### §10.1 ID 命名空间统一

**风险**：现在 PlayerState 用 `username` 字符串，plan-death/plan-npc 用 `CharId`。当玩家死透重开新角色，或 NPC 生平卷与玩家生平卷同表查询时，两套 ID 会打架。

- [ ] **立刻约定**：`CharId = UUIDv7`（时间有序 + 全局唯一），所有生平相关表以 CharId 为主键
- [ ] PlayerState 追加 `current_char_id UUID` 外键 + 保留 `username` 仅作显示名
- [ ] NPC 出生即分配 CharId，与玩家共用 ID 空间（不做 player_id/npc_id 二元区分）
- [ ] 预留其他命名空间：`FactionId = UUIDv7`、`QuestId = UUIDv7`、`ItemId = 字符串枚举`、`RelicId = UUIDv7`

### §10.2 事件表的多态存储

**风险**：`life_events` 初版可能列举一堆字段（突破用字段 A，死亡用字段 B），新增事件类型要 migration。

- [ ] **初版即采用多态列**：
  ```sql
  CREATE TABLE life_events (
    event_id INTEGER PRIMARY KEY,
    char_id BLOB NOT NULL,           -- UUIDv7
    event_type TEXT NOT NULL,        -- 'breakthrough' | 'death' | 'duoshe' | ...
    payload TEXT NOT NULL,           -- JSON，结构由 event_type 决定
    payload_version INTEGER NOT NULL,
    game_tick INTEGER NOT NULL,
    wall_clock INTEGER NOT NULL,     -- UTC unix seconds
    schema_version INTEGER NOT NULL
  );
  CREATE INDEX idx_life_events_char ON life_events(char_id, game_tick);
  ```
- [ ] 同一 `event_type` 的 payload 演化走 `payload_version` 字段 + 代码侧 match
- [ ] 适用于：`life_events` / `lifespan_events` / `faction_events` / `quest_history`

### §10.3 时间双轨

**风险**：`Tick` 会随 server 重启归零或跳变，单用 tick 无法按真实时间查询（"三天前发生了什么"）。

- [ ] 所有事件表必带 `game_tick` + `wall_clock`（UTC 秒）双字段
- [ ] 快照表（player_state / npc_state / zones_runtime）加 `last_updated_wall INTEGER`
- [ ] 服务器重启时 `game_tick` 从 SQLite 恢复（不归零），但需要 `server_run_id` 字段追踪重启次数（诊断用）

### §10.4 schema_version 分层

**风险**：当前文案混用"每表 schema_version"和"PRAGMA user_version"。两者职责不同。

- [ ] **库级 `PRAGMA user_version`**：migration 顺序控制（加表 / 删表 / 结构变更）
- [ ] **行级 `schema_version INTEGER`**：单行 payload 结构版本，用于 JSON payload 演化（如 §10.2）
- [ ] 两者独立推进：结构变更推 user_version、payload 演化推 schema_version

### §10.5 高频状态节流的分级

**风险**：HP / 真元 不能和"位置"一样 60s 节流——断线重连可能丢掉最后 59s 的战斗伤害。

- [ ] **战斗关键字段**（HP / 真元 / stamina / active_wounds）：事件驱动（被打即写）+ 强节流 5s；登出/断线强制 flush
- [ ] **位置字段**：60s 节流即可
- [ ] **经脉进度 / 境界进度**：90s 节流（变动慢）
- [ ] 在 `players` 表拆成 `player_core`（高频战斗）+ `player_slow`（位置/UI）两表，各自节流参数

### §10.6 Archetype / 事件类型开放枚举

**风险**：`archetype` 若定义为 SQLite CHECK 约束里的固定枚举，加 archetype 要 migration。

- [ ] `archetype TEXT NOT NULL`（不加 CHECK），合法值在 Rust 侧枚举
- [ ] 加 `archetype_registry` 表（char_id, archetype, since_tick），允许 NPC 中途"转职"（如凡人被夺舍后变散修留痕）
- [ ] 同理 `event_type` / `faction_doctrine` / `quest_kind` 全部开放字符串

### §10.7 远方 NPC Digest 淘汰

**风险**：`npc_digests` 随 agent 推演累积，1 年后可能几万行，大部分 NPC 再也不被引用。

- [ ] `NpcDigest` 加 `last_referenced_wall INTEGER`
- [ ] 定期（每周）扫描：`last_referenced_wall < now - 180d` 的 digest → 归档到 `data/archive/npc_digests/` 并从表删
- [ ] Agent 若突然需要引用老 NPC，从归档反向加载（冷数据）

### §10.8 Agent ↔ SQLite ↔ Redis 三方一致性

**风险**：Agent 写 SQLite 成功 + Redis 失败 → Redis 脏，反之亦然。

- [ ] **写路径单向**：Agent → SQLite（权威）→ 触发器 publish Redis → 其他订阅者消费
- [ ] Agent 读：启动时全量拉 SQLite，运行时只订阅 Redis 增量
- [ ] Redis 发布失败 → 不回滚 SQLite（以 SQLite 为准），但告警
- [ ] 订阅侧周期性 reconcile（每 5 min 拉 SQLite diff），自愈

### §10.9 Raster 可变性预留

**风险**：worldview §十二 / §四 说域崩会永久改变区域；raster 当前只读。

- [ ] **不在 raster 上改**——raster 保持只读基线
- [ ] 新增 `zone_overlays` 表：`{ zone_id, overlay_kind, payload JSON, since_wall }`
  - `overlay_kind`: `collapsed` / `qi_eye_formed` / `ruins_discovered`
  - client 加载 zone 时 = raster 基线 + overlay 叠加
- [ ] Raster 永远只读，所有运行时变化走 overlay

### §10.10 library-web 静态导出的增量

**风险**：每个角色终结都触发 `npm run build` 显然不现实。

- [ ] 终结事件仅写 `library-web/public/deceased/{char_id}.json` 单文件
- [ ] library-web 实现前端 fetch：索引页读 `public/deceased/_index.json`（角色列表），详情页按需 fetch 单文件
- [ ] `_index.json` 由 server 维护（终结事件同时 append 一条元信息）
- [ ] 本质上 server 直接写入 library-web 的 `public/` 目录，**不再依赖构建过程**

---

## §11 已决定（§10 派生）

- ✅ `CharId` / `FactionId` / `QuestId` / `RelicId` = UUIDv7；`ItemId` = 字符串枚举
- ✅ 事件表采用多态 payload + payload_version
- ✅ 所有事件表必带 `game_tick` + `wall_clock` 双时间
- ✅ 库级 `user_version` 控结构、行级 `schema_version` 控 payload
- ✅ `players` 表拆 `player_core`（战斗关键 5s 节流）+ `player_slow`（位置 60s）
- ✅ Archetype / event_type / doctrine 全部开放字符串（无 CHECK 枚举）
- ✅ `npc_digests` 加 `last_referenced_wall` + 180 天冷归档
- ✅ Agent ↔ SQLite ↔ Redis 单向写：SQLite 权威 → Redis publish
- ✅ Raster 只读，运行时变化走 `zone_overlays` 表
- ✅ library-web 静态站不依赖构建，server 直写 `public/deceased/`
