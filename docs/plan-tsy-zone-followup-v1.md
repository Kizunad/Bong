# TSY Zone P0 收尾 · plan-tsy-zone-followup-v1

> 补 `plan-tsy-zone-v1.md` 落地后剩下的 2 个非 manual-QA gap：集成测 + Server→Redis 桥。
> 交叉引用：`plan-tsy-zone-v1.md §1.4 / §5.2 / §6`、`docs/finished_plans/...`（merged via PR #49 = `bd349286`）

---

## §-1 现状（zone-v1 落地后）

| 层 | 能力 | 状态 |
|----|------|------|
| Zone struct + helpers | `is_tsy / tsy_depth / tsy_family_id / is_tsy_entry` + `register_runtime_zone` | ✅ `server/src/world/zone.rs` |
| TsyPresence / RiftPortal / DimensionAnchor | Components | ✅ `server/src/world/tsy.rs` |
| 负压 tick | `tsy_drain_tick` + 公式 + `DeathEvent("tsy_drain")` | ✅ `server/src/world/tsy_drain.rs` |
| Entry/Exit portal system | 含 escape margin（codex P1 修复） | ✅ `server/src/world/tsy_portal.rs` |
| Entry filter | `apply_entry_filter` + `strip_name` | ✅ `server/src/world/tsy_filter.rs` |
| `!tsy-spawn` 调试命令 | 含同 tick 去重（codex P2 修复） | ✅ `server/src/world/tsy_dev_command.rs` |
| Schema TypeBox + JSON Schema artifact | `TsyEnterEventV1` / `TsyExitEventV1` / `TsyDimensionAnchorV1` / `TsyFilteredItemV1` | ✅ `agent/packages/schema/src/tsy.ts` + `generated/` |
| 模块级 unit test | 30+ 用例（drain 公式表 / filter / portal / dev-command） | ✅ |
| `scripts/smoke-tsy-zone.sh` | server cargo test + schema vitest + check | ✅ |

**zone-v1 没做的（本 plan 范围）**：

1. `server/tests/tsy_zone_integration.rs`（zone-v1 §5.2）— Valence test harness 端到端：portal 触发 → 传送 → drain → 出关
2. Server → Redis 桥 — `TsyEnterEmit / TsyExitEmit` Bevy event → `bong:world_state` 频道 JSON publish（用 §1.4 已生成的 schema）

**本 plan 不做（明确 out-of-scope）**：

- Manual QA A–E（`plan-tsy-zone-v1 §6`）— 真人 WSLg 跑 client 验证入场/分层/出关/边界/断线，需要人工
- Agent 侧消费 `tsy_enter / tsy_exit` 写 narration — 归 `plan-tiandao-...` / agent plan
- POI 持久化、worldgen 自动产 portal — 归 `plan-tsy-worldgen-v1`
- `qi_drained_total` 累计 — 归 `plan-tsy-loot-v1`
- `/tsy-spawn` 与 `!tsy-spawn` 命名差（zone-v1 §6 用 `/`，落地用 `!` 跟仓库 dev-command 习惯）— 不改

---

## §1 集成测试（zone-v1 §5.2）

### 1.1 位置

`server/tests/tsy_zone_integration.rs`（新建，作为 `cargo test` 一部分）

### 1.2 覆盖

最少 4 个 `#[test]`，每个起一个 mock App（**不需要真 Valence 网络层**——`world::dimension::mark_test_layer_as_overworld` 已有 test helper，可参考）：

- [ ] **A.entry_full_path**
  - Spawn Entry portal 在 (0,64,0)；spawn 玩家在 (0.5, 64, 0) Overworld、带 `PlayerInventory`（含 1 个 spirit_quality=0.7 物品）
  - `app.update()` 一次
  - 验证：`TsyEnterEmit` 发出、`TsyPresence` 已 attach、`DimensionTransferRequest(Tsy, shallow_center)` 发出、inventory 那个物品 spirit_quality=0 + 改名
- [ ] **B.drain_after_entry**
  - 接 A 的 state，把玩家位置 set 到 TSY shallow zone center 内、`CurrentDimension::Tsy`、`PlayerState { spirit_qi: 50, spirit_qi_max: 50 }`
  - 跑 N tick（N = 10 或够覆盖 1 个 drain 周期）
  - 验证：`PlayerState.spirit_qi` 已下降 ≥ `compute_drain_per_tick * N` 的 0.9 倍（容差）
- [ ] **C.drain_to_zero_emits_death_event**
  - B 的延伸：把 `spirit_qi` set 为接近 0 的小值，跑直到归零
  - 验证：`Events<DeathEvent>` 收到一条 `cause = "tsy_drain"`
- [ ] **D.exit_round_trip**
  - 玩家在 TSY shallow center 内 + 持 `TsyPresence(return_to=Overworld(2.5,65,0))`
  - 把玩家位置移到对应 family `_shallow` Exit portal trigger_radius 内
  - `app.update()`
  - 验证：`TsyExitEmit` 发出、`DimensionTransferRequest(Overworld, return_to.pos)` 发出、`TsyPresence` 已 remove

### 1.3 工程约束

- 不依赖 `valence::testing::ScenarioSingleClient`（那个起整个 client 太重）。直接 `App::new()` + `add_event::<...>` + `add_systems(Update, ...)` + 关键 component 手动 spawn——已经在各 `tsy_*.rs` 模块测试里验证过这种最小 harness 可行
- 不依赖 Redis（§2 的 bridge 测试归 §2）
- 跑测命令：`cd server && cargo test --bin bong-server --test tsy_zone_integration`（cargo 自动 pickup `tests/*.rs`）

---

## §2 Server → Redis 桥

### 2.1 位置

新建 `server/src/network/tsy_event_bridge.rs`（参照已有 `vfx_event_emit.rs` / `combat_bridge.rs` 的 pattern——读 Bevy Event + 写 Redis outbound）

### 2.2 流程

```
TsyEnterEmit (Bevy Event)
  → tsy_event_bridge_system (Update)
  → 转 TsyEnterEventV1 wire payload（按 schema/src/tsy.ts shape）
  → 通过 RedisBridgeResource.tx_outbound 推到 bong:world_state（或专属 bong:tsy_event 频道，决策见 §2.4）
```

同理 `TsyExitEmit → TsyExitEventV1`。

### 2.3 实现要点

- **player_id 解析**：`TsyEnterEmit.player_entity` 是 Bevy Entity；wire schema 要 string `player_id`。需从 `Username` component 取 + `canonical_player_id(...)`（仿 `chat_collector.rs:160` 用法）
- **return_to.dimension 字符串化**：`DimensionKind::Overworld` → `"minecraft:overworld"`、`DimensionKind::Tsy` → `"bong:tsy"`。给 `DimensionKind` 加 `pub fn ident_str(&self) -> &'static str`（或 `Display` impl），双端字面量统一
- **filtered_items**：从 `TsyEnterEmit.filtered: Vec<FilteredItem>` 直接映射（field 名一致）
- **tick**：用 `CombatClock.tick`（或 plan §1.4 schema 里写的 server tick）
- **qi_drained_total**：P0 emit 时填 0（schema 接受）；累计归 loot plan

### 2.4 Redis channel 决策

候选：

- A. 复用 `bong:world_state`（当前 `agent_world_model_envelope` 用的频道）— 简单但 envelope 类型变多，agent 侧 dispatch 要扩
- B. 新增 `bong:tsy_event` 专属频道 — 隔离干净，agent 侧加单独 subscriber

**默认选 B**，与 `combat_bridge` 走 `bong:combat_event` 是同样思路。在 `agent/packages/schema/src/channels.ts` 加常量 `CH_TSY_EVENT = "bong:tsy_event"`。

### 2.5 测试

- `server/src/network/tsy_event_bridge.rs` 内置 `#[cfg(test)] mod tests`：
  - emit 一个 `TsyEnterEmit` → bridge 跑一遍 → `crossbeam_channel::Receiver<RedisOutbound>` 拿到一条 payload，反序列化回 `TsyEnterEventV1` 校验通过
  - 同理 `TsyExitEmit`

---

## §3 文件清单

新建：

- `server/tests/tsy_zone_integration.rs`
- `server/src/network/tsy_event_bridge.rs`

修改：

- `server/src/network/mod.rs` — register `tsy_event_bridge`
- `server/src/world/dimension.rs` — `DimensionKind::ident_str(&self)` helper
- `agent/packages/schema/src/channels.ts` — `CH_TSY_EVENT` 常量

---

## §4 验收

- `cd server && cargo test --bin bong-server --test tsy_zone_integration` 全绿（4 case）
- `cd server && cargo test --bin bong-server` 全绿（含 §2 bridge unit test）
- `cd agent/packages/schema && npm test && npm run check` 全绿（channels.ts 加常量后 generated artifact 可能要 regen）
- `bash scripts/smoke-tsy-zone.sh` 全绿（不变 / 略扩）

---

## §5 进度日志

- 2026-04-26 创建（zone-v1 P0 PR #49 已 merge `bd349286`）；本 plan 待 `/consume-plan tsy-zone-followup-v1` 实施
