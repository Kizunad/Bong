# BugHunt: 矿脉耗尽日志半写后重启复活

## Bug 摘要

`ExhaustedMineralsLog::flush` 直接用 `fs::write(&self.file_path, json)` 覆盖 `data/minerals/exhausted.json`。`std::fs::write` 走 `File::create + write_all`，打开成功后会截断既有文件；如果进程在写入中途崩溃、被 kill，或底层 I/O 在 truncate 后失败，最终路径可能留下空文件或半写 JSON。

启动期 `hydrated_from_path` 对解析失败只 warn 并返回空 log。随后 `spawn_mineral_anchor_nodes` 用这份空 log 计算 exhausted skip 集合，已耗尽矿脉位置会被重新物化。

边界：这不是 #972 dormant Redis dirty ACK 问题，也不是 `docs/plans-skeleton/plan-bughunt-r10-findings-v1.md` 已记录的“耗尽 entry 还在 30 秒节流窗口内、尚未落盘就关服/崩溃”。本缺陷是“已经存在的有效 exhausted log 被下一次 flush 直接覆盖写坏”。

## 实际游玩体验影响

玩家已经挖穿的矿脉，本应由 `data/minerals/exhausted.json` 记录为耗尽并在重启后跳过。若一次刷盘半写导致 JSON 损坏，下一次开服会把整份耗尽记录当成空状态。

结果是旧矿脉在重启后复活，永久耗尽矿物或尚未到 respawn 时间的矿物都可能提前出现。玩家会看到矿洞资源回滚，矿物有限性和矿区经济被破坏。

## 证据定位

- `server/src/mineral/persistence.rs:154-173`：`flush()` 序列化后直接 `fs::write(&self.file_path, json)` 写最终路径，没有 tmp 文件、rename、备份或恢复旧文件。
- `server/src/mineral/persistence.rs:228-254`：`hydrated_from_path` 对 corrupt/parse failure 只 warn，然后保留空 `entries`。
- `server/src/mineral/persistence.rs:415-421`：现有测试明确把坏 JSON 锁定为“空 log”。
- `server/src/mineral/mod.rs:69-77`：启动注册时从默认 `data/minerals/exhausted.json` hydrate，并把结果插入 ECS resource。
- `server/src/mineral/mod.rs:87-90`：启动阶段随后运行 `spawn_mineral_anchor_nodes`。
- `server/src/mineral/anchors.rs:95-110`：锚点物化只靠 `exhausted.entries()` 生成 skip 集合；log 为空时已耗尽位置不会被跳过。
- `server/src/mineral/anchors.rs:123-130`：化石矿脉同样使用这份 exhausted skip 集合。
- `server/src/mineral/break_handler.rs:401-410`：矿块剩余单位归零后发送 `MineralExhaustedEvent` 并 despawn/index remove。
- `server/src/mineral/persistence.rs:189-217`：生产路径每 600 tick 自动 flush dirty exhausted log，因此不需要手工破坏文件，正常运行中的刷盘即可进入风险窗口。
- 对照 `server/src/craft/unlock.rs:175-203`、`server/src/spiritwood/persistence.rs:150-193`：同仓其它 JSON 持久化已采用 tmp 写入 + rename，注释明确是为了避免写入中断留下截断 JSON。
- 对照 `server/src/craft/unlock.rs:1131-1165`、`server/src/spiritwood/persistence.rs:486-511`：同仓已有“失败写不得触碰 final path”的原子写安全测试。

## 触发路径

1. 服务器已有一份有效 `data/minerals/exhausted.json`，记录至少一个已耗尽矿脉。
2. 玩家继续挖穿另一处矿脉，`handle_block_break_for_mineral` 发送 `MineralExhaustedEvent`，`record_exhausted_minerals` 将新 entry 记入内存并标 dirty。
3. 600 tick 节流窗口到期，`ExhaustedMineralsLog::flush` 开始直接覆盖最终 `exhausted.json`。
4. 进程在 `File::create` 成功截断后、`write_all` 完整写完前崩溃/被 kill，或底层写入失败；最终文件留下空文件或半写 JSON。
5. 下次开服 `hydrated_from_path` 解析失败，warn 后启动空 log。
6. `spawn_mineral_anchor_nodes` 看到 empty exhausted set，重新物化原本已耗尽的锚点/化石矿脉。

## 反方审查记录

- 第一轮反方：候选成立。确认开放 PR 中 #971 是矿脉锚点坐标漂移，#876 是矿脉采集移动打断，#972 是 dormant Redis 写失败 dirty ACK，均不覆盖本问题；r10 skeleton 只覆盖“未及时 flush”，不覆盖“final-path overwrite 写坏已有日志”。
- 第一轮反方还确认：没有找到矿脉 exhausted log 的 tmp+rename、备份、fsync、启动阻断或从旧日志恢复的保护；启动 anchor/fossil 物化确实依赖该 log。
- 第二轮反方：通过。进一步确认 `std::fs::write` 不是“写失败保留旧文件”的接口，打开成功后会截断既有文件；同仓 craft/spiritwood 已把同类风险作为持久化契约测试，说明该风险达到 BugHunt 真实 bug 门槛。
- 第二轮边界建议：fix plan 只聚焦 `ExhaustedMineralsLog::flush` 的 final-path 直接覆盖，避免混入 r10 的 Last/AppExit 强刷缺口，也不触碰 #972 的 Redis ACK 语义。

## Skeleton Fix Plan

- 将 `ExhaustedMineralsLog::flush` 改为同目录临时文件写入：
  - 先 `fs::write(tmp_path, json)`。
  - 成功后 `fs::rename(tmp_path, self.file_path)`。
  - rename 成功后才 `dirty = false`、`flush_clock = 0`。
- 失败路径保持现有语义：
  - 返回 `Err(...)`。
  - 保持 `dirty = true`，允许后续 tick 重试。
  - 不触碰最终 `exhausted.json` 的旧内容。
- 保留 corrupt 文件启动不阻断策略，但将其视为外部/历史损坏 fallback；正常 flush 不应再制造 corrupt final file。
- 不在本计划中处理 r10 已记录的 AppExit/Last shutdown 强刷问题；若未来合并修复，可作为矿脉持久化的另一个独立补丁。

## 验收测试计划

- 在 `server/src/mineral/persistence.rs` 增加与 craft/spiritwood 同款原子写测试：
  - 先 flush 一份有效 exhausted log。
  - 读取并保存原始 final path 字节。
  - 在 `path.with_extension("tmp")` 位置创建目录，强制下一次 tmp 写失败。
  - 新增一条 entry 后调用 `flush()`。
  - 断言 `flush()` 返回 Err。
  - 断言 final path 字节与原始内容完全一致。
  - 断言 log 仍为 dirty。
- 增加 roundtrip 测试确认成功 flush 后 tmp path 不残留，`load_exhausted_log` 能读回所有 entries。
- 增加启动物化回归测试：
  - 一份有效 exhausted log 能继续让 `spawn_mineral_anchor_nodes` 跳过已耗尽位置。
  - 一次失败 flush 后重启 hydrate 仍能读旧 final file，不会把已耗尽矿脉复活。
- 按 server 栈运行：
  - `cd server && cargo fmt --check`
  - `cd server && cargo clippy --all-targets -- -D warnings`
  - `cd server && cargo test`

## 风险

- 使用固定 `.tmp` 路径时，若上次崩溃遗留 tmp 文件，下一次 flush 会覆盖 tmp 再 rename；这符合“final path 只有成功写完才替换”的目标。
- `fs::rename` 在同一目录内保持原子替换语义；tmp 必须与 final path 在同一目录，避免跨文件系统 rename。
- 修复不改变 corrupt final file 的启动 fallback；已有坏文件仍会 warn + 空 log。若需要从 `.bak` 或 tmp 恢复，应另立计划，避免扩大本修复范围。
- AppExit/Last 强刷仍是 r10 skeleton 的独立缺口；本计划不宣称解决“尚未落盘的新 entry”。
