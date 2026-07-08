# Bong · plan-bughunt-supply-coffin-cooldown-restart-rollback-v1

## 摘要

BugHunt persistence r07 发现：物资棺碎裂后的冷却债务只存在 `SupplyCoffinRegistry.cooldowns` 内存队列里，服务器重启后 `register()` 重新创建空 registry，`supply_coffin_refresh_tick` 又把无 cooldown 的档位视为 initial fill。结果是玩家开完棺、等 timeout 碎裂进入 30min / 2h / 6h 冷却后，只要服务器维护重启，冷却就被清空并重新刷满同档物资棺。

这不是“未开 active 棺必须持久化”的问题：`refresh.rs` 注释确实允许重启 initial fill，未开棺作为环境刷新物可接受。真正破坏的是“已消费刷新名额后进入真实时间冷却”的经济约束。

## 实际游玩体验影响

巨剑沧海物资棺本应用真实时间冷却控制稀有材料产出：Common 30 分钟、Rare 2 小时、Precious 6 小时。当前实现下，玩家或运维在开棺碎裂后重启服务器，会让该档冷却立即消失；下一次 Update 又按空 cooldown 初始化填满上限。玩家视角是珍稀祭坛棺不再需要等 6 小时，维护/崩服/主动重启都可能变成重复刷高级器修材料的入口。

打开中的 `ExternalContainer` 会话跨重启丢失不是本 plan 的主 bug：棺内剩余 loot 本来就可能在 timeout 后碎裂销毁。修复时可选择关服即按 timeout 处理剩余物，但必须把该棺计入对应档位 cooldown，不能让重启绕过刷新债务。

## 证据

- `server/src/supply_coffin/mod.rs`：`SupplyCoffinRegistry` 只含 `active: HashMap<Entity, ActiveSupplyCoffin>`、`cooldowns: Vec<CoffinCooldown>`、`rng_state`，没有稳定 id 或持久化字段。
- `server/src/supply_coffin/mod.rs`：`register()` 每次启动直接 `SupplyCoffinRegistry::new(...)` 并 `app.insert_resource(registry)`，未从 SQLite/JSON hydrate。
- `server/src/supply_coffin/refresh.rs`：`supply_coffin_refresh_tick` 在 `active_count < max_active` 且该 grade 没有 cooldown 时进入 initial fill；文件头注释也写明服务器重启后会填满 `max_active`。
- `server/src/supply_coffin/lifecycle.rs`：timeout 碎裂走 `remove_active` + `enqueue_cooldown` + `Despawned`，没有调用任何 persistence 写入。
- `server/src/persistence/mod.rs`：启动 hydrate 覆盖 void cooldown、伪灵脉、zone runtime/overlay/influence 等，但无 `SupplyCoffinRegistry` / `CoffinCooldown` / `ExternalContainer` 分支。

## 去重

- 非 #1044「可放置实体重启丢失」：#1044 覆盖玩家放置的 `workbench_item` / `trade_crate` / `herb_crate_placed` / `dead_drop_box` 丢失；本问题是系统刷新物资棺的冷却债务丢失，玩家资产没有被扣后凭空消失。
- 非 #991「散修遗缴生命周期易失状态」：SurfaceStash 的 24h per-player 限额和 depleted/respawn 已由 #991 覆盖；本问题是 SupplyCoffin 的 per-grade real-time cooldown。
- 非 `docs/plans-skeleton/plan-bughunt-supply-coffin-cross-dimension-session-gate-v1.md`：该 skeleton 覆盖跨维 open/lifecycle/move 授权；本问题是重启后的刷新经济回滚。

## 修复要求

- [ ] P0：为物资棺新增持久化冷却债务模型，至少记录 `grade`、`broken_at_wall_secs`、schema version；可先不持久化未开 active 棺位置。
- [ ] P0：timeout / 关服处理已打开会话时，必须写入对应档位 cooldown；open 会话剩余 loot 可按 timeout/关服销毁策略处理，但不能清空冷却债务。
- [ ] P1：启动 hydrate `CoffinCooldown`，并在 `supply_coffin_refresh_tick` initial fill 前尊重未到期 cooldown。
- [ ] P1：写覆盖测试：碎裂后落盘 → 重启 hydrate → 未到期不刷新；到期后刷新并消费一条 cooldown；open 会话关服后不会让同档立即 initial fill。
- [ ] P2：补 dev 命令/日志可观测性，`/supply_coffin list` 能显示持久化冷却剩余时间，便于运维确认维护重启不会刷物资。

## 对抗结论

Round 1：反方确认 `SupplyCoffinRegistry`、`ExternalContainer` 均为纯内存，persistence bootstrap 无 hydrate；和 #1044 不重复，建议立独立 plan。

Round 2：反方将范围降级并收窄：未开 active 棺重启 initial fill 可视为设计允许，不应强求全量 active 持久化；高置信 bug 是 `CoffinCooldown` / per-grade spawn debt 重启清零导致真实时间冷却被绕过。结论：中等强度 persistence bug，独立于跨维 session gate。

## 验收

- `cd server && cargo fmt --check`
- `cd server && cargo clippy --all-targets -- -D warnings`
- `cd server && cargo test supply_coffin`
- 建议补一条 server 集成测试，模拟 timeout 写 cooldown、重建 App/hydrate 后同档不 initial fill。
