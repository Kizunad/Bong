# plan-bughunt-lingtian-default-zone-shadow-v1（骨架）

> **骨架（草案）**。一句话主题：`server/src/lingtian` 已经把 `plot.zone`、zone-aware weather/profile、`ZoneQiAccount` 多 zone 键空间铺出来，但核心消费链仍把生长 / 补灵 / 偷灵 / 压力 / recent-event / ledger 回流硬绑到 `DEFAULT_ZONE`，导致**非默认区灵田“看起来分区，实际全串默认区”**。

## 结论

- **核心 bug**：多 zone 灵田 sidepath 只有“写入端”支持真实 zone，“消费端”仍统一读写 `DEFAULT_ZONE`；结果是远端灵田不会吃自己的 zone qi / 天气 / 压力，而是偷偷改默认区账本。
- **这个 bug 对实际游玩体验的影响**：玩家把灵田建在 `spawn` 以外的湿地/血谷/北荒时，表面上 plot 已经被标成当地 zone，天气系统也会为该 zone 掷出事件，但作物生长、补灵成本、偷灵回流、压力升档、道伥风险仍按默认区结算。体感上会出现“我在异地种田，为什么影响的是出生区、而本地天气/灵压又像没接上”的强割裂；多人服里还会把不同 zone 的农场互相串账，误伤别人的默认区灵气与压力。

## 复现路径

1. 在有 `ZoneRegistry` 的世界里放置一块非默认区灵田；`auto_set_plot_zone` 会把 plot 补成真实 zone，而不是空串或 `default`（`server/src/lingtian/systems.rs:4373-4443` 测试已钉住，`plot.zone` 字段定义在 `server/src/lingtian/plot.rs:17-20,56-58`）。
2. 让 `weather_generator_system_zone_aware` 运行到 day 边界；它会逐 zone 掷天气并把结果写进 `ActiveWeather.by_zone[zone]`，不是单一默认区（`server/src/lingtian/weather.rs:530-570`）。
3. 对这个非默认区 plot 执行补灵 / 偷灵 / 生长 tick / 压力累计。
4. 观察消费链：补灵检查与扣款读写 `zone_qi.get(DEFAULT_ZONE)` / `get_mut(DEFAULT_ZONE)`（`server/src/lingtian/systems.rs:520-526,1266-1336`）；偷灵回流与 ledger 目标写 `QiAccountId::zone(DEFAULT_ZONE)`（`server/src/lingtian/systems.rs:1139-1147,1203-1209`）；压力系统只算 `DEFAULT_ZONE` 且天气也只读 `aw.current(DEFAULT_ZONE)`（`server/src/lingtian/systems.rs:1533-1629`）；生长 tick 永远拿 `zone_qi.get_mut(DEFAULT_ZONE)`（`server/src/lingtian/systems.rs:1634-1671`）。
5. 结论：远端 plot 的真实 `plot.zone` 和该 zone 的天气 profile 只停留在“被写出来”，并未进入核心结算。

## 根因链路

1. `LingtianPlot` 已有 `zone: String`，并且 `auto_set_plot_zone` 会在 registry 可用后回填真实 zone。
2. `weather_generator_system_zone_aware` 已按 `ZoneRegistry` 全量 zone 掷天气，`ActiveWeather` 也按 zone 存储。
3. 但灵田主循环仍保留单 zone MVP 假设：
   - `handle_start_replenish` 预检 `Zone` 来源时直接查 `DEFAULT_ZONE`。
   - `apply_replenish_completion` 扣款/overflow 回流仍写 `DEFAULT_ZONE`。
   - `apply_drain_qi_completion` 与 `emit_drain_qi_transfers` 仍把散逸真元记回 `DEFAULT_ZONE`。
   - `record_replenish_to_pressure`、`compute_zone_pressure_system`、`advance_plot_one_lingtian_tick_in_zone` 都忽略 `plot.zone`。
   - `record_dye_contamination_warning_recent_events` 连 recent-event 的 `zone` 字段也固定成 `DEFAULT_ZONE`（`server/src/lingtian/systems.rs:1404-1413`）。
4. 结果是“zone-aware 写入端”与“single-zone 消费端”并存，形成影子默认区。

## 影响面

- `server/src/lingtian/systems.rs`：补灵、偷灵、压力、生长、事件埋点。
- `server/src/lingtian/weather.rs`：非默认区天气事件会被正常生成，但对 plot 结算基本无效，形成 dead-on-read。
- `server/src/lingtian/qi_account.rs`：账本本身支持多 zone，但上层调用方没有把真实 zone 传进去。
- 相关 client/UI 链路：`LingtianActionScreen` 以玩家准星坐标触发真实地块交互，但玩家从 HUD/动作结果感知到的后果却来自默认区，难以从前端发现串账根因。

## 修复建议

1. 以 plot 实际 zone 作为单一真相源：`StartReplenish`、`DrainQi`、`growth_tick`、`pressure`、recent-event、ledger 统一先解析目标 plot，再取 `plot.zone`。
2. `compute_zone_pressure_system` 改为按 zone 分桶 plot，而不是把全量 plots 一把塞进 `DEFAULT_ZONE`。
3. `advance_plot_one_lingtian_tick_in_zone` 改为 `zone_qi.get_mut(plot.zone_or_fallback())`；天气也改读 `aw.current(plot.zone)`。
4. 为非默认区补 3 类 pin 测试：`replenish/drain` 记账落到真实 zone、非默认区天气实际影响 growth/pressure、recent-event 的 `zone` 与 plot.zone 对齐。

## 反方裁决

### Round 1

- **反方论点**：这只是 v1 留下的“单 zone MVP”，不是 bug。
- **驳回理由**：如果全链都还是单 zone，那 `plot.zone` / `auto_set_plot_zone` / `weather_generator_system_zone_aware` / `ZoneWeatherProfileRegistry` 不会已经接入生产路径。现在的问题不是“没做多 zone”，而是“写入端已经做了，消费端还没迁完”，属于半接线导致的真实错结算。

### Round 2

- **反方论点**：也许只有天气是多 zone，qi/pressure 故意共用默认区，属于设计。
- **驳回理由**：代码注释与类型面都不支持这个说法。`ZoneQiAccount` 明确以 zone 名为 key；`LingtianPlot` 注释写的是“与所在 zone 的 spirit_qi 双向流动”；天气、压力、账本、recent-event 却分属不同 zone 口径，只会制造互相矛盾的世界状态，而不是形成自洽设计。

## 取证与退化说明

- 本轮未改源码，只做搜索、静态取证、skeleton 记录。
- 按要求做了两轮反方裁决。
- 当前会话无法再开 subagent，采用**人工反方退化处理**；已把反方论点与驳回理由显式写入本 skeleton。
- 测试侧尝试：
  - `server`: 已发起 `cargo test lingtian::`，但在本轮取证结束前仍处于长编译阶段，未拿到完成结果。
  - `client`: 定向 `gradlew test` 受沙箱限制失败，先后撞到只读 `~/.gradle` 与 daemon `java.net.SocketException: Operation not permitted`，因此本轮 client 证据以源码链路为主。

## 验证结论（2026-07-26 整理审计追认）

灵田多区结算串默认区已由 2f2314199（2026-07-06，「修复灵田多区结算串默认区」，491 行 diff）修复：`server/src/lingtian/systems.rs:78-85` 新增 `plot_zone_key()` 统一 real-zone 解析，`start_replenish`/`replenish_completion`/`drain_qi`/`dye_contamination`/`zone_pressure`/`one_tick` 全部改用 `plot.zone` 而非硬编 `DEFAULT_ZONE`，消费端与写入端的 zone 口径不再分裂。

## Finish Evidence

- **落地清单**：`server/src/lingtian/systems.rs`（`plot_zone_key()` + 六条消费链改用 `plot.zone`）
- **关键 commit**：2f2314199（2026-07-06，「修复灵田多区结算串默认区」，491 行 diff）
- **测试结果**：证据未列出具体测试名；2026-07-26 审计为只读核验（Read+grep+git log 对拍 origin/main），未重跑测试套件
- **跨仓库核验**：仅 server 侧命中（`lingtian::systems::plot_zone_key`），属纯服务端多区结算修复，无 client/agent 契约面
- **遗留 / 后续**：无
