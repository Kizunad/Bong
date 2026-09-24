# plan-bughunt-shelflife-clock-restart-freshness-v1

## §0 摘要

`Freshness.created_at_tick` 是持久化的绝对 tick，但驱动 `now_tick` 的所有资源（`GameplayTick`/`CombatClock`/`ShelflifeSweepTick`）都是纯内存 `Default` Resource，重启归零。玩家已持有的所有可衰减物品（灵木杆 `ling_mu_gun`、熟肉/灵果/陈酒等食物、矿物、异兽肉血）在服务器重启后会被 `effective_dt_ticks` 的 `saturating_sub` 钳到 0，冻结成永久全鲜状态，绕过 `Spoiled`/`CriticalBlock` 拒食门禁、Age 陈化峰值窗口、骨市材料衰减等全部 shelflife 机制，且这部分衰减永不补偿。

本 plan 仅是 BugHunt Skeleton Plan，不包含实际修复。

## §1 实际游玩体验影响

- 服务器每次重启（`dev-reload.sh` 常规开发迭代、崩溃恢复、日常维护重启，均是本仓约定里的常规操作，绝非 dev-only）都会把玩家当时已持有的所有带 `Freshness` 的物品冻结成"刚刚采到/刚刚打到"的满鲜状态。
- 冻结期长度等于旧进程运行了多久（服务器已运行越久，新进程重启后这个冻结期越长，长期运行的服务器可能达数小时到数天）。
- 在冻结期内，本该判定为 Spoiled/CriticalBlock 而拒绝食用的腐败食物会被判定为新鲜可食；灵木杆等稀缺材料的品质窗口被无限延长。
- 即便追上旧进程的绝对 tick 值之后，物品也永久性地"少衰减了"相当于旧进程 `created_at_tick` 那么多 tick，这部分衰减永不补偿——是系统性的、可重复的经济失衡而非一次性 bug。

## §2 复现路径

1. 玩家正常采集一次灵木（spiritwood 巨树采集）拿到 `ling_mu_gun`，或吃过熟肉/陈酒/灵果、打过异兽拿到生肉生血——这些物品带 `Freshness` 进入玩家存档（`server/src/inventory/mod.rs:2178-2197` `runtime_instance_from_template` 用 `current_tick` 写 `created_at_tick`）。
2. 玩家存档落盘（`server/src/player/state.rs` 的 `save_player_inventory_slice`），`Freshness.created_at_tick` 作为绝对值随 `ItemInstance` 一起持久化。
3. 服务器重启（任意常规重启，例如 `dev-reload.sh` 循环、崩溃恢复）。
4. 现状预期：`GameplayTick`/`CombatClock`/`ShelflifeSweepTick` 全部从 0 重新计数（无任何 hydrate 逻辑），`effective_dt_ticks`（`server/src/shelflife/compute.rs:247-256`）用 `now_tick.saturating_sub(created_at_tick)` 计算，`now_tick` 远小于旧进程遗留的 `created_at_tick`，`saturating_sub` 直接钳到 0，物品读数为"刚刚创建"的满鲜状态。
5. 修复后预期：跨重启物品的新鲜度计算不应因 tick 计数器归零而倒退到 0，衰减进度应可跨进程正确延续。

## §3 根因证据

- `server/src/shelflife/compute.rs:247-256` `effective_dt_ticks`：`now_tick.saturating_sub(freshness.created_at_tick)`，`saturating_sub` 保证 `now_tick < created_at_tick` 时直接钳到 0，`apply_formula`/`apply_exponential_qi_physics` 均把 dt=0 当"返回 initial"（即 100% 新鲜）处理。
- `server/src/shelflife/types.rs:178-194` `Freshness.created_at_tick: u64`，随 `ItemInstance` 一起 `Serialize`/`Deserialize`，是权威持久层字段。
- `server/src/inventory/mod.rs:2178-2197` `runtime_instance_from_template` 用 `current_tick` 写 `created_at_tick`——其注释已承认"避免服务器运行一段时间后发出的食物立刻被当已陈化"这个方向的坑，但未解决反方向（重启后 `now_tick` 归零导致的倒退钳零）。
- `server/src/player/gameplay.rs:137-148,159-167` `GameplayTick` 是纯内存 `#[derive(Default)] Resource`，仅在 `register()` 里 `insert_resource(GameplayTick::default())`，全仓无任何 hydrate/persist 调用；`server/src/spiritwood/mod.rs:236-251,576-605` `complete_spiritwood_sessions` 用 `gameplay_tick.current_tick()` 给稀缺材料 `ling_mu_gun` 盖 `Freshness` 戳。
- `CombatClock`（`server/src/combat/mod.rs` 相关定义，`cultivation/tick.rs` 内递增）与 `ShelflifeSweepTick`（`server/src/shelflife/sweep.rs`，200-tick 周期 sweep）同样只在插件注册时 `::default()`，全仓 grep 无一处 hydrate。
- `server/src/player/state.rs` 的 `save_player_inventory_slice`/load 路径：`PlayerInventory`（连同每个 `ItemInstance.freshness`）落盘持久化并原样载回，是权威持久层，证实两侧时间基准不对齐——持久层用绝对 tick，运行时基准每次重启归零。

## §4 非重复比对

- 已读 `docs/plan-bughunt-mineral-respawn-tick-restart-drift-v1.md`：只处理矿脉再生绝对 tick 漂移，明确声明"不改矿物掉落"，不覆盖 shelflife/`Freshness`。
- 已读 `docs/plan-bughunt-alchemy-freshness-feed-v1.md`：炼丹投料侧写死 `quality_factor=1.0`，与本问题"时钟本身错"完全不同根因，不重复。
- 已读 `docs/plan-bughunt-r6-findings-v1.md` #4：冻结容器 enter/exit 接线缺失（`frozen_since_tick` 恒为 `None`），是"冻结加速衰减未生效"的角度，与本 finding"`now_tick` 倒退导致衰减倒退到 0"是不同故障模式，虽然都引用了 `compute.rs:247` 附近代码但触发条件和影响截然不同。
- 已读 `docs/plans-skeleton/plan-bughunt-voidaction-cooldown-runtime-tick-restart-v1.md` 与 `docs/plan-bughunt-realm-taint-restart-amnesia-v1.md`：与本 finding 同属"tick 时钟重启归零"这一recurring 架构模式的另外两个已被接受的独立实例，各自作用于不同子系统（voidaction 冷却 / realm taint），均明确不涉及 shelflife。
- Grep `ShelflifeSweepTick`/`created_at_tick`/`shelflife` 未命中任何专门覆盖"重启后时钟归零"这一失败模式的既有 skeleton/active/finished plan。
- `docs/finished_plans` 中的 `spiritwood-shutdown-flush` 只管 harvested log 落盘窗口，不涉及 `Freshness` 计算。

## §5 修复计划骨架

### P0 跨重启单调时钟基准

- 把 `Freshness` 的"现在"基准迁移到跨重启单调的口径：二选一——
  1. 持久化并在启动时 hydrate 相关 tick 资源（`GameplayTick`/`CombatClock`/`ShelflifeSweepTick` 三选一统一或各自补 hydrate，仿照 mineral::persistence 的 flush/hydrate 模式）。
  2. 把 `Freshness` 从"绝对 `created_at_tick` vs 易失 `now_tick`"改造成"持久化的已耗用 `elapsed_ticks` 计数器，每次在线 tick 自增"（与 `plan-lingtian-process-v1` 里 `ProcessingSession`"session_id + 已完成 ticks 持久化"的既有模式一致），从根本上消除跨进程绝对值比较。
- 从根本上消除新进程 `now_tick` 小于旧进程遗留 `created_at_tick` 的可能性。

### P1 回归测试

- 补回归测试：模拟"`created_at_tick`=大值"的物品在新 `App`（tick 从 0 起）里立即计算 `effective_dt`，断言不为 0（或按新语义断言等价的"不倒退"）。
- 补测试覆盖 `Spoiled`/`CriticalBlock`/Age 峰值窗口在跨重启场景下的正确判定（不应被冻结绕过）。

## §6 验证计划

- `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
- 手工复现矩阵：模拟重启前后（tick 归零）计算同一物品的 `effective_dt_ticks`，断言修复后不出现"倒退到满鲜"。

## §7 接入面与守恒说明

- 进料：`Freshness`/`ItemInstance`（持久化字段）、`GameplayTick`/`CombatClock`/`ShelflifeSweepTick`（运行时资源）、玩家存档（`server/src/player/state.rs`）。
- 出料：`effective_dt_ticks` 计算结果，驱动 `Spoiled`/`CriticalBlock`/Age 峰值判定。
- 跨端契约：本问题是纯 server 内部时钟基准问题，不涉及 client/agent 契约变更。
- qi_physics：shelflife 衰减机制与真元/灵气转移无关，不涉及 `qi_physics::ledger`，本 finding 不新增衰变常数或公式（本身修复方向是"时钟基准对齐"而非"改衰减公式"）。

## §8 对抗复核结论

- 候选证据：`Freshness.created_at_tick` 跟随 `PlayerInventory` 完整持久化落盘并原样载回；`effective_dt_ticks` 的 `saturating_sub` 在 `now_tick < created_at_tick` 时钳到 0；`GameplayTick`/`CombatClock`/`ShelflifeSweepTick` 三个 now_tick 生产者全部确认无 hydrate/持久化逻辑，纯内存 `::default()` 重启归零；灵木杆/熟食/生肉血等均通过正常游玩路径带 `Freshness` 进入存档。
- 反方质疑：是否只是"冻结容器角度"的重复 finding？是否与矿脉/void action/realm taint 三个已接受的"tick 重启归零"案例重复？
- 修正/反驳：`docs/plan-bughunt-r6-findings-v1.md` #4 虽引用同一行 `compute.rs:247`，但角度是"冻结加速衰减的 `frozen_since_tick` 接线缺失导致冻结从不生效"，与本 finding"`now_tick` 本身在重启后倒退导致衰减整体倒退到 0"是不同触发条件、不同影响范围的独立缺陷；已确认的三个"tick 重启归零"接受案例（矿脉再生 / void action 冷却 / realm taint）各自作用于不同子系统且明确不覆盖 shelflife。
- 反方最终裁决：通过（`is_real: true`, `reachable: true`, `severity_adjust: unchanged`，保持 high）。可达性完全正常游玩可达（采集/进食/战利品 + 任意常规重启），是本仓已验证三次的"tick 时钟重启归零"架构模式应用在 shelflife 子系统的新实例，非重复，未修复，适合开 Skeleton Plan PR。
