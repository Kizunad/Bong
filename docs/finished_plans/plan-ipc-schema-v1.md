# Bong · plan-ipc-schema-v1 · 模板

**横切基础专项**。集中管理所有 Redis channel + TypeBox schema + 双端对齐规范。

**落位**：`agent/packages/schema/` source of truth → JSON Schema export → Rust serde。

**交叉引用**：CLAUDE.md「Server ↔ Agent IPC」· 各 plan 自述 channel。

---

## §0 设计轴心

- [x] 单一真相源（TypeBox in TS）
- [x] 双端强校验（samples/*.json）
- [x] channel 命名规范（`bong:<domain>` / `bong:<domain>/<event>`，双端 `CHANNELS` / `CH_*` 常量 + frozen 测试）
- [~] 版本化策略 — **非范围**，见 §2

## §1 Channel 总表

| Channel | 方向 | 节流 | Payload | 来源 plan |
|---|---|---|---|---|
| `bong:world_state` | server → agent | | | server |
| `bong:agent_cmd` | agent → server | | | agent |
| `bong:player_chat` | client ↔ server | | | client |
| `bong:combat/*` | | | | combat / HUD |
| `bong:cultivation/*` | | | | cultivation |
| ... | | | | |

## §2 Schema 版本化（非范围 — 显式不做）

Bong 部署模型为 server / client mod / agent 三层**同 git revision 一起跑**（玩家装的 mod 必须匹配服务器版本），不存在"老 client 连新 server"的 cross-version 兼容场景。常规 IPC schemaVersion / migration 设计的核心价值（跨版本兼容）在 Bong 上下文里不存在，因此本节显式不做：

- [~] 向后兼容策略 — 不需要。所有协议变更同 commit 修 TypeBox + Rust serde + client handler，三方一起 deploy。
- [~] schema hash / version 字段 — 不加。每条 payload 不带 `schemaVersion`，由 channel 名 + 双端 frozen 测试 + samples roundtrip 共同锁住契约。
- [~] migration 流程 — 不需要。破坏性变更走"同 PR 改三端 + frozen 测试 bump"流程；server 重启时 Redis 残留老 payload 的毫秒级窗口由订阅方 `try_deserialize → log + skip` 兜底（已是默认行为）。

注：持久化层（SQLite 存档）的版本迁移属于 `plan-persistence-v1` 范围，与本 plan 无关。

## §3 命名规范

- [x] channel 前缀约定（`bong:<domain>/<event>`，已落于 botany/skill；agent_* / *_event / world_state 走单段命名）
- [ ] Intent / State / Event 后缀
- [ ] Store 命名

## §4 测试与校验

- [x] samples/*.json 双端 round-trip 测试
- [ ] 新增 channel checklist

## §5 实施节点

- [x] P1 · 盘点现有 channel → 补档（实际双端 28 个 channel，超出 P1 时设想的 18）
- [~] P2 · 版本字段加入 — 非范围，见 §2
- [x] P3 · CI schema 校验

## §6 开放问题

- [ ]

---

## 进度日志

- 2026-04-25：现状盘点 — 已有 18 个 channel 双端落地（`agent/packages/schema/src/channels.ts` ↔ `server/src/schema/channels.rs`，含 `REDIS_V1_CHANNELS` 与 frozen 测试），TypeBox source-of-truth + samples/*.json round-trip + CI 校验闭环；剩 P2 版本化（schemaVersion / migration）未启动。
- 2026-04-28：实地核验 — `REDIS_V1_CHANNELS` 28 项、`CH_*` 常量约 28 个、`samples/*.json` 106 份、`generated/*.json` 156 份、`tests/schema.test.ts` ~85 个 it() round-trip case，远超文档自报的"18 channel"。结合 Bong 三层同版本部署模型，§2 schemaVersion / migration 无实际收益，标为非范围；plan 主体目标（双端 source-of-truth + frozen 测试 + roundtrip + CI）已达成，迁入 finished_plans/。

---

## Finish Evidence

### 落地清单

- **agent 侧** `agent/packages/schema/`
  - `src/channels.ts`：`CHANNELS` + `REDIS_V1_CHANNELS`（28 项）
  - `src/*.ts`：30+ TypeBox payload 定义文件（server-data / client-request / inventory / forge / cultivation / tsy / tsy-hostile / weapon 等）
  - `generated/*.json`：156 份 JSON Schema 导出
  - `samples/*.json`：106 份正反例样本（含 `*.invalid-*.sample.json` 反例）
  - `tests/schema.test.ts` + 各专项 test：~85 个 it() roundtrip case
- **server 侧** `server/src/schema/`
  - `channels.rs`：`CH_*` 常量 + `redis_v1_channel_constants_remain_frozen` frozen 测试
  - `server_data.rs` / `client_request.rs` / `inventory.rs` / `forge_bridge.rs` / `combat_hud.rs` / `tsy.rs` / `tsy_hostile.rs` 等 serde struct 双端镜像
- **client 侧** `client/src/main/java/com/bong/client/network/`：handler 路由消费上述 channel

### 关键 commit

- `8d22922f feat(schema): 补齐武器装备推送契约`
- `3fe36222 plan-tsy-hostile-v1: 定义敌对 NPC IPC schema`
- channel frozen 测试历次 bump（随各 plan PR）

### 测试结果

- `cd agent/packages/schema && npm test`：roundtrip 全绿（~85 case）
- `cargo test -p bong-server schema::`：双端 frozen + serde roundtrip 通过
- CI schema check 已纳入 PR 必跑

### 跨仓库核验

| 端 | 命中 |
|---|---|
| agent | `agent/packages/schema/src/channels.ts` `REDIS_V1_CHANNELS` 28 项 |
| server | `server/src/schema/channels.rs` `CH_*` + frozen test |
| client | `client/src/main/java/com/bong/client/network/` 各 handler 注册 |

### 遗留 / 后续

- **§1 Channel 总表**：表格 placeholder 未填具体 28 项；以代码 (`channels.ts` / `channels.rs`) 为单一事实源，文档表格不再补回填，新增 channel 时直接在代码侧 + frozen 测试落地。
- **§3 Intent / State / Event 后缀 + Store 命名规范**：未定形式化命名规则；当前 ad-hoc 命名工作良好，不阻塞，留作后续如有大批量重命名需求再启 v2。
- **§4 新增 channel checklist**：未写文档化清单；当前流程靠"加 channel → 加 frozen 测试 → 加 sample → CI 跑过"自然形成约束，已够用。
- **§2 版本化**：显式不做（见 §2）；如未来 Bong 进入"插件市场 / 玩家自带 mod" 模式，需要重新评估，届时启 plan-ipc-schema-v2。
