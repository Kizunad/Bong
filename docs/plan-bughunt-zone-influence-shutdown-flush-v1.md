# plan-bughunt-zone-influence-shutdown-flush-v1

## Bug 摘要

`zone_influence` 只在 `Update` 阶段按 300 秒节流快照落盘，关服 `Last + AppExit` 只强制刷 `zone_runtime`，没有强制刷 `ZoneInfluenceMap`。玩家在最近一次快照后获得/失去区域影响力、霸主或公开状态，服务器若在下一个 5 分钟快照前正常关服，重启会从 SQLite 旧快照 hydrate，导致领地影响力和霸主状态回滚。

## 对实际游玩体验的影响

玩家刚通过驻留、修炼、战斗或 PvP 击杀夺下某个区域的霸主后，如果服主很快重启，重进游戏会看到这次领地成果消失：区域霸主可能回到旧人或变成无人，`public_known` 传播状态回退，相关传闻、NPC 态度、交易信誉、区域加成和天道注意力都按旧状态继续。玩家视角是“刚打下来的地盘被重启吞了”，不是单纯后台计数误差。

## 证据定位

- `server/src/persistence/mod.rs:661-685`：`register` 初始化 `ZoneInfluenceSnapshotState`，把 `persist_zone_influence_system` 挂在 `Update`，但 `Last` 只挂 `persist_zone_runtime_on_shutdown_system`。
- `server/src/persistence/mod.rs:83`：`ZONE_RUNTIME_SNAPSHOT_INTERVAL_SECS = 5 * 60`，`zone_influence` 复用同一 300 秒节流窗口。
- `server/src/persistence/mod.rs:879-905`：`persist_zone_influence_system` 在 `last_snapshot_wall > 0` 且未满 300 秒时直接 return；即使 AppExit 当帧先跑 Update，也不会强制保存。
- `server/src/persistence/mod.rs:3277-3320`：已有 `persist_zone_influence_snapshot` 可完整写入 value、source_breakdown、dominant、established_tick、public_known。
- `server/src/persistence/mod.rs:777-789`、`3412-3435`：启动时从 SQLite hydrate `ZoneInfluenceMap`；未落盘内存态会被旧快照覆盖。
- `server/src/world/territory.rs:464-552`：`territory_tick` 直接修改玩家影响力、`source_breakdown`、`dominant`。
- `server/src/world/territory.rs:643-718`：PvP death 路径即时修改击杀者/被杀者影响力并重算霸主，不等待 60 秒 tick。

## 触发路径

1. 服务器启动后，`bootstrap_persistence_system` 从 `zone_influence` hydrate 到 `ZoneInfluenceMap`。
2. `persist_zone_influence_system` 第一次成功快照，写入 `last_snapshot_wall`。
3. 300 秒窗口内，玩家通过驻留/修炼/战斗/PvP 改变 `ZoneInfluenceMap`，例如击杀旧霸主后 `dominant` 变为新玩家。
4. 服主执行正常关服或重启，`Last` 阶段只强制刷 `zone_runtime`，不刷 `zone_influence`。
5. 下次启动从 SQLite 旧 `zone_influence` hydrate，最近一次窗口内的影响力与霸主状态丢失。

## 反方审查记录

- 第一轮反方结论：通过。未找到即时 persist、AppExit 强制刷盘或开放修复 PR；`last_snapshot_wall=0` 只覆盖首次快照，后续 300 秒窗口仍存在。
- 第二轮反方结论：通过。`docs/plans-skeleton/plan-bughunt-r10-findings-v1.md` 曾在 P2 #2 记录同类 finding，但这是不可消费骨架，PR #579 只是 merged skeleton，不是 active plan 或开放修复 PR；本 plan 可作为独立 active bughunt plan 推进。
- 严重性复核：维持 minor/medium 合理，因窗口限定在关服前最多 300 秒，但玩家可直接感知领地成果、NPC 态度、传闻和加成回退。

## Skeleton Fix Plan

1. 在 `server/src/persistence/mod.rs` 新增 `persist_zone_influence_on_shutdown_system`。
2. 将该 system 挂到 `Last`，与 `persist_zone_runtime_on_shutdown_system` 同级监听 `AppExit`。
3. 关服路径直接调用 `persist_zone_influence_snapshot(&settings, &influence_map)`，绕过 300 秒节流。
4. 保持现有周期性 `persist_zone_influence_system` 不变，避免扩大运行时写库频率。
5. 不改 territory/social/rumor/perk 逻辑；本修复只补持久化生命周期缺口。

## 验收测试计划

- 新增 server 单测：先持久化旧 `ZoneInfluenceMap`，模拟周期快照已发生，再在 300 秒窗口内修改 `dominant/public_known/source_breakdown`，发送 `AppExit` 并跑 `Last`，断言 SQLite 读回最新状态。
- 新增负向回归断言：没有 `AppExit` 时仍遵守 300 秒节流，不把周期系统改成每 tick 写库。
- 跑 `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`。
- 可选 e2e：两名玩家在同一区域完成 PvP 夺主，立即关服重启，验证重连后霸主、传闻状态和相关加成不回滚。

## 风险

- 关服时新增一次 SQLite 写入，风险低；数据量随玩家区域影响记录增长，需要沿用已有事务写法。
- `persist_zone_influence_snapshot` 当前只 upsert，不删除已从内存移除的旧记录；本 plan 不扩大范围，若未来支持清空某玩家区域影响，需要另立数据清理策略。
- 旧 r10 skeleton 已提到该问题，本 plan 应避免与矿脉关服刷盘盲区混在同一修复中，保持边界单一。
