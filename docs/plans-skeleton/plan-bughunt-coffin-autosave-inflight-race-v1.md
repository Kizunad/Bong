# BugHunt: 延寿棺 in_coffin 事件驱动写入与 60 秒周期 autosave 排序竞态

## Bug 摘要

**严重度：low（skeptic 由 medium 调整为 low）**

`server/src/coffin/mod.rs::handle_coffin_enter_requests` 在处理 `CoffinEnterRequest` 时，先通过 `commands.entity(event.player).insert(CoffinComponent{...})` 做延迟生效的结构性插入，随后在**同一 tick、同一系统调用内**同步执行 `persist_in_coffin(..., Some(coffin.grade))`，把 `in_coffin=1` / `coffin_grade` 立刻写进 sqlite。与此同时，`server/src/player/mod.rs::autosave_player_lifespan_slices` 每 60 秒（`LIFESPAN_SLICE_FLUSH_INTERVAL_TICKS = 1200 tick`）对所有在线玩家跑一次周期性快照，直接从 ECS `Query<Option<&CoffinComponent>>` 读取当前组件并调用同一个 `save_player_lifespan_slice_with_coffin` 落盘。`coffin::register` 与 `player::register` 各自独立注册系统，彼此之间没有任何 `.after`/`.before` 排序声明，也没有共享数据访问冲突，Bevy 调度器因此不会在两者之间强制插入 `apply_deferred` 同步点。若某玩家的进棺事件恰好落在 `timer.ticks` 是 1200 倍数的那个 tick，`autosave_player_lifespan_slices` 读到的 `Option<&CoffinComponent>` 可能仍是"进棺前"的 stale `None`（Commands 插入尚未在调度 sync point 生效），随后调用 `save_player_lifespan_slice_with_coffin(..., None)` 会把 `in_coffin` 显式覆写为 `false`，直接盖掉刚刚写入的 `in_coffin=1`（SQLite 无跨系统事务，写入以后执行者为准）。

## 实际游玩体验影响

ECS 内存态本身不受影响——下一 tick `CoffinComponent` 已经生效，60 秒后下一次周期性 autosave 会自我纠正回 `in_coffin=1`。但如果服务器在这条错误写入发生之后、下一次周期性 autosave 之前发生**非优雅关闭**（进程崩溃/断电，走不到 `flush_connected_players_on_shutdown` 的优雅落盘路径），DB 就会永久停留在 `in_coffin=false`。玩家重启重连后，`attach_player_state_to_joined_clients` 读到 `persisted.in_coffin=false`，不会触发把该玩家重新钉回棺材的 reclaim 逻辑——玩家悄悄失去"正在棺中安眠"状态；同时该棺材在 `CoffinRegistry` 重建后可能被判定为未占用，被另一玩家占用造成同一具延寿棺被两名玩家同时认领的脏状态。触发条件是"进/出棺事件恰好落在 1200-tick 边界（约 1/1200 概率）+ 此后 60 秒内服务器非优雅重启"两个小概率事件叠加，因此虽然是真实存在、无需任何 dev 指令即可触发的竞态，但持久损坏窗口很窄，长期运行的生产服务器上才有实际概率复现。

## 证据定位

- `server/src/coffin/mod.rs:604-630`（`handle_coffin_enter_requests`）：L620-624 `commands.entity(event.player).insert(CoffinComponent{...})`（延迟 Commands），L625-630 同一函数内同步调用 `persist_in_coffin(..., Some(coffin.grade))`。
- `server/src/coffin/mod.rs:309-353`（`coffin::register`）：系统排序仅 `.after(handle_client_request_payloads).before(emit_audio_play_payloads)`（L318-331），与 `player` 模块无任何交叉排序声明。
- `server/src/coffin/mod.rs:332-343`：唯一的 `apply_deferred` 链 `(apply_deferred, rebuild_missing_coffin_markers).chain().after(handle_coffin_place_requests).after(handle_coffin_breaks).after(handle_coffin_menu_reclaim)`——只覆盖放置/破坏/回收，**不包含** `handle_coffin_enter_requests` / `handle_coffin_leave_requests`。
- `server/src/player/mod.rs:45`：`LIFESPAN_SLICE_FLUSH_INTERVAL_TICKS: u64 = 60 * TICKS_PER_SECOND`（即 1200 tick）。
- `server/src/player/mod.rs:325-327`（`tick_player_persistence_timer`）：`timer.ticks += 1` 每 tick 无条件自增，独立于玩家动作。
- `server/src/player/mod.rs:746-778`（`autosave_player_lifespan_slices`）：L749 `Query<(&Username, &LifespanComponent, Option<&CoffinComponent>), With<Client>>` 直接读 ECS，L760-766 原样把 `coffin.map(|c| c.grade)` 传给 `save_player_lifespan_slice_with_coffin`，无"回读 DB 既有值"兜底分支。
- `server/src/player/mod.rs:99-124`（`player::register`）：`autosave_player_lifespan_slices.after(autosave_player_cultivation_bundles)`（L115）只在 player 模块内部链式排序，从未引用 `coffin` 模块任何系统。
- `server/src/player/state.rs:656-666`（`save_player_lifespan_slice`，兄弟函数）：显式传 `None` 给 `coffin_grade`，注释写明"回读 DB 既有 grade，避免悟道延寿路径把 Jade/Stone/Bronze 洗成 Mundane"——这是本该同样应用到周期快照路径、但没有被应用的纪律。
- `server/src/player/state.rs:668-684`（`save_player_lifespan_slice_with_coffin`）：恒传 `Some(grade.is_some())` 作为 `in_coffin` 显式值（L680），从不使用"回读 DB"分支。
- `server/src/player/state.rs:1738-1789`（`persist_player_lifespan_slice_in_sqlite`）：L1749 `resolve_in_coffin_for_persist(...)`。
- `server/src/player/state.rs:1791-1808`（`resolve_in_coffin_for_persist`）：`explicit=Some(value)` 时直接采用并覆盖（L1796-1798），只有 `explicit=None` 才回读 DB 既有 `in_coffin`（L1799-1807）——`save_player_lifespan_slice_with_coffin` 永远不落入回读分支。
- `server/src/main.rs:86, 100`：`player::register(&mut app)` 与 `coffin::register(&mut app)` 各自独立调用，注册顺序上也没有相互依赖声明。
- `server/src/player/state.rs:696-699`（F21 断连兜底注释）：说明现有代码已经意识到 `in_coffin` 持久化的边界情况，但未覆盖本竞态。

## 触发路径

1. 玩家使用延寿棺交互（进棺右键）产生 `CoffinEnterRequest`。
2. `handle_coffin_enter_requests` 在本 tick 内先 `commands.entity(event.player).insert(CoffinComponent{...})`（Deferred Commands，需等调度 sync point 才对其他系统的 Query 可见），紧接着同步调用 `persist_in_coffin(..., Some(coffin.grade))`，把 `in_coffin=1`/`coffin_grade` 立即写入 sqlite。
3. 若这一 tick 恰好是 `timer.ticks` 的 1200 倍数（`tick_player_persistence_timer` 每 tick 无条件递增，与玩家动作无关），`autosave_player_lifespan_slices` 会在**同一 tick** 对所有在线玩家跑一次；由于 `coffin` 与 `player` 两个模块之间没有任何排序声明、也没有数据访问冲突，Bevy 判定两系统无序/不冲突，不会强制插入 `apply_deferred` 同步点。
4. `autosave_player_lifespan_slices` 的 `Query<Option<&CoffinComponent>>` 读到的仍是"进棺前"的 stale `None`，于是调用 `save_player_lifespan_slice_with_coffin(..., None)`。
5. `save_player_lifespan_slice_with_coffin` 内部恒传 `Some(grade.is_some())=Some(false)` 给 `resolve_in_coffin_for_persist`，覆盖刚刚写入的 `in_coffin=1`（SQLite 无跨系统事务保护，写入顺序以后执行者为准）——若两个系统本 tick 的执行顺序恰好是"进棺 persist 先、autosave 后"，DB 停留在 `in_coffin=false`。
6. ECS 内存态未受影响：下一 tick `CoffinComponent` 已生效，60 秒后下一次周期性 autosave 会读到真实组件并自我纠正回 `in_coffin=1`。
7. 但若在这条错误写入之后、下一次周期性 autosave（≤60 秒）之前，服务器发生非优雅关闭（进程崩溃/断电，走不到 `flush_connected_players_on_shutdown`），DB 就永久停留在 `in_coffin=false`。
8. 重启后 `attach_player_state_to_joined_clients` 读到 `persisted.in_coffin=false`，不会调用 reclaim 逻辑把玩家重新钉回棺材；该棺材在 `CoffinRegistry` 重建后可能被判定未占用，被另一玩家占用，造成同一具棺被两名玩家同时认领的脏状态。

## 反方审查记录

- 第一轮质疑：
  - 是否已有跨模块排序约束覆盖 enter/leave？——查 `coffin::register`（L309-353）只对内部系统和 `handle_client_request_payloads`/`emit_audio_play_payloads` 排序，`apply_deferred` 链只覆盖 place/breaks/reclaim，不含 enter/leave；`player::register`（L99-124）链条完全在 player 模块内部，未引用 coffin。确认无跨模块排序。
  - 是否 `Commands` 插入与 Query 读取存在数据访问冲突从而被 Bevy 自动定序？——`autosave_player_lifespan_slices` 只读 `Username`/`LifespanComponent`/`CoffinComponent`，`handle_coffin_enter_requests` 的 Query 还写 `Position`/`Flags`，但两者对 `CoffinComponent` 的访问是"一读一写(经 Commands 延迟)"，Bevy 对 Commands 引入的结构性变更不产生同步屏障，除非显式声明排序。确认竞态成立。
  - 是否与已知 plan 重复？——`gh`/文档核对：`docs/plan-bughunt-coffin-dimension-gate-v1.md` 是缺失跨维度授权检查的完全不同问题；`plan-bughunt-supply-coffin-cooldown-restart-rollback-v1.md` 是 `SupplyCoffinRegistry` 重启冷却丢失，属另一子系统。确认非重复。
- 第二轮补证：
  - 核对兄弟函数 `save_player_lifespan_slice`（L656-666）确实走"grade=None → 回读 DB"分支，注释明确写出这是为了防止悟道延寿路径洗掉档级——证明这条纪律在代码里是已知模式，只是没有被应用到 `save_player_lifespan_slice_with_coffin`/`autosave_player_lifespan_slices` 这条周期性路径上，形成不对称。
  - 核对 `resolve_in_coffin_for_persist`（L1791-1808）：`explicit=Some` 恒覆盖、`explicit=None` 才回读——坐实"两条写路径中只有一条会覆盖真值"的机制细节。
  - 让步：触发需要"进/出棺事件恰好落在 1200-tick 边界"（约 1/1200 概率）**且**随后 ≤60 秒内服务器发生非优雅关闭（崩溃/断电）两个条件同时成立才会造成可观测的持久损坏；若无非优雅关闭，60 秒周期自愈会覆盖回正确值，玩家无感知。
  - 终裁：**severity 由 medium 下调为 low**——bug 真实存在、可复现路径完整、无需 dev 指令，但持久损坏窗口窄、且有自愈机制，不构成"容易复现的中等严重度"缺陷，定为 low 优先级排队修复项。
- 主循环复核：已亲读关键行确认。

## Skeleton Fix Plan

- [ ] 在 `server/src/coffin/mod.rs` 与 `server/src/player/mod.rs` 之间为 `autosave_player_lifespan_slices` 与 `handle_coffin_enter_requests`/`handle_coffin_leave_requests` 建立显式跨模块排序约束（`.after(...)`），并确认排序本身足以让 `autosave` 观察到当 tick 内 Commands 插入生效——若 Bevy 只对系统排序不对 Commands flush 生效，需要额外在两者之间显式插入 `apply_deferred`（比照 `coffin/mod.rs:332-343` 已有的 `(apply_deferred, rebuild_missing_coffin_markers).chain()` 模式）。这需要把 `handle_coffin_enter_requests`/`handle_coffin_leave_requests` 的可见性从模块私有调整为 `pub(crate)`，评估这对 API 面的影响是否可接受。
- [ ] 或者（推荐的更根本修复）：把 `autosave_player_lifespan_slices` 对 `in_coffin` 的写入语义对齐 `save_player_lifespan_slice`（L656-666）的"None=回读 DB 既有值"惯例——只有真正处理 coffin 状态变化的事件驱动路径（`handle_coffin_enter_requests`/`handle_coffin_leave_requests`）才允许显式写 `Some(bool)`；`autosave_player_lifespan_slices` 本身不应该"重新宣称"in_coffin 的真值，因为它只是周期性快照，不是状态变更事件源。需要设计一个新的辅助函数（例如 `save_player_lifespan_slice_periodic`，`in_coffin=None` 走回读）区分"周期快照"与"事件驱动显式写"两类调用者，避免 `save_player_lifespan_slice_with_coffin` 继续被两种语义不同的调用方共用。
- [ ] 若选择"周期快照回读"路径，必须同时验证 `handle_coffin_leave_requests` 在离棺时依然会显式写 `in_coffin=false`（离棺是事件驱动路径，必须保留显式覆盖能力），不能让"回读 DB"语义变成"进棺后 in_coffin 永远读不到 false"的另一种回归。
- [ ] 本次修复只涉及 `in_coffin`/`coffin_grade` 布尔与枚举字段的持久化排序问题，**不涉及任何真元/灵气流动**——确认修复不触碰 `SPIRIT_QI_TOTAL`、`qi_physics::ledger::QiTransfer`，不新增衰减/衰变常数。
- [ ] `CoffinEnterRequest`/`CoffinLeaveRequest` 均是玩家交互产生的 C2S 请求；`handle_coffin_enter_requests` 内的 `occupied_by`/距离检查已经是 server 侧权威判定。本次修复不改变这条 C2S 权威校验链路——server gate 依旧是最终权威，客户端侧如果要加任何"棺材数据保存中"提示，也只是 UX 增强，不构成校验的一部分。
- [ ] 补充针对该竞态的最小可复现单测：直接构造两次 `persist_player_lifespan_slice_in_sqlite`/`save_player_lifespan_slice_with_coffin` 调用序列（先事件驱动 `Some(grade)`，再模拟周期性 autosave 传入 stale `None`），断言修复后最终落盘值仍为 `in_coffin=true`（未修复前应先复现出被覆盖为 `false` 的现状，作为回归红线）。
- [ ] 若走排序修复路径，额外补一个用 `bevy::app::App` 手动 `update()` 一次 tick 的集成测试：在同一 tick 内先发 `CoffinEnterRequest` 事件、把 `timer.ticks` 设为 `LIFESPAN_SLICE_FLUSH_INTERVAL_TICKS` 的倍数，`update()` 后断言 DB 最终 `in_coffin=true`。
- [ ] 评估线上是否已存在因该竞态产生的脏数据（`in_coffin=false` 但 `CoffinRegistry` 仍标记 `occupied_by=Some(player)`，或反之）——若发现需要单独的一次性数据修复脚本或人工清理说明，不在本次代码修复范围内自动处理。

## 验收测试计划

- **server cargo test（happy path）**：正常进棺（无 tick 边界巧合）走 `handle_coffin_enter_requests` → `persist_in_coffin`，断言 DB `in_coffin=1`/`coffin_grade` 与内存 `CoffinComponent.grade` 一致；随后触发一次正常时序的 `autosave_player_lifespan_slices`（ECS 已看到 `CoffinComponent`），断言 DB 值不变（无回归）。
- **server cargo test（边界：tick 边界巧合）**：构造 `timer.ticks` 恰好为 `LIFESPAN_SLICE_FLUSH_INTERVAL_TICKS` 整数倍、且同一 tick 有 `CoffinEnterRequest` 待处理的场景，用 `bevy::app::App::update()` 跑一次，断言修复后的执行顺序/语义使得该 tick 结束后 DB `in_coffin=true`（旧代码应在同一测试下先复现为 `false` 以证明测试确实锁住了目标行为）。
- **server cargo test（错误分支：未进棺玩家）**：`coffin=None`（玩家从未进棺）时 `autosave_player_lifespan_slices` 仍应保留把 `in_coffin` 写为 `false` 的能力（不能因为本次修复变成"从不清空 in_coffin"）；断言修复后此路径行为不变。
- **server cargo test（状态转换：离棺撞周期 autosave）**：对称构造离棺事件（`CoffinLeaveRequest`）与周期性 autosave 落在同一 tick，断言最终 DB `in_coffin=false`（离棺显式写入不应被"周期快照回读语义"意外吞掉，需专门覆盖 A→A 与 A→B 两类状态转换）。
- **server cargo test（回读语义不受影响）**：对 `resolve_in_coffin_for_persist`/`resolve_coffin_grade_for_persist` 补一条 `explicit=None` 场景专属 case（悟道延寿等无棺上下文保存路径），断言其仍然回读 DB 既有值，不因本次修复被误改。
- **可选 e2e/联调**：结合 `bash scripts/smoke-test-e2e.sh` 或专用 bot 场景，模拟服务器非优雅重启（`kill -9`）后校验棺材占用状态（`CoffinRegistry.occupied_by`）与玩家 `in_coffin` 持久化字段的一致性，覆盖"崩溃发生在错误写入之后、下一次自愈之前"这一现实触发条件。

## 风险

- 修复范围必须严格限定在 `in_coffin`/`coffin_grade` 持久化写入的排序/语义问题上，禁止顺手把 `CoffinComponent`/持久化结构扩展成跨维度模型——那是完全不同的 bug（见 `docs/plan-bughunt-coffin-dimension-gate-v1.md`），不要在本 plan 里合并处理。
- 若选择"跨模块显式排序"路径，需要确认 `coffin` 与 `player` 模块之间目前没有循环依赖顺序问题（`main.rs` 中 `player::register` 先于 `coffin::register` 调用），并评估为排序需要而放宽私有函数可见性对模块边界的影响。
- 若选择"周期快照回读"路径，必须仔细设计避免破坏 `handle_coffin_leave_requests` 的显式清零能力，也要确认 F21 断连兜底路径（`state.rs:696-699`，直接把某用户名 `in_coffin` 清 0）与新语义不冲突。
- 该竞态触发窗口本身很窄（约 1/1200 概率的 tick 巧合 + 随后 ≤60 秒内非优雅关闭），修复优先级定为 low；但长期运行、玩家规模较大的生产服务器上仍有真实概率累积出脏数据（同棺双占/延寿状态丢失），不应无限期搁置。
