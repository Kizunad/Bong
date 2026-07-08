# plan-bughunt-spirit-eye-runtime-persistence-v1

> Skeleton Plan（BugHunt H7 / persistence 第七轮）。仅记录真实 bug 与修复计划，不消费、不归档。

## Bug 摘要

`SpiritEyeRegistry` 的灵眼运行态只存在内存里，server 重启会用 `SpiritEyeRegistry::from_zones(&ZoneRegistry::load(), startup_salt())` 重新生成资源，导致玩家已发现名单、迁移压力、上次迁移 tick、迁移后的非血谷坐标全部回到初始态。

本 bug **不主张**“血谷灵眼初始坐标重启漂移”是问题；`plan-spirit-eye-v1` 已把血谷重启不稳定作为设计选项。这里的问题是：已发现/已使用/已迁移的运行态没有任何持久化或 hydrate。

## 实际游玩体验影响

玩家花时间用神识找到一口灵眼后，重启服务器会丢失私有发现状态：HUD 私有标记消失，坐标笔记/交易资格失效，死亡遗念拿不到“已知灵眼坐标”。多人长期服中，玩家反复用同一灵眼突破累积的 `usage_pressure` 也会在重启后归零，原本用于防止灵眼永久固定化的迁移压力和迁移结果被回滚。

## 证据定位

- `server/src/world/spirit_eye.rs:43-51`：`SpiritEye` 运行态字段包括 `discovered_by`、`usage_pressure`、`last_migrate_tick`。
- `server/src/world/spirit_eye.rs:147-158`：`from_zones` 初始化每口灵眼时把 `discovered_by` 设为空、`usage_pressure=0.0`、`last_migrate_tick=0`。
- `server/src/world/spirit_eye.rs:195`：发现灵眼时只向内存 `discovered_by` push。
- `server/src/world/spirit_eye.rs:216-219`：突破使用时只在内存补发现者并累加 `usage_pressure`。
- `server/src/world/spirit_eye.rs:263-266`：迁移时只在内存改 `pos`、清 `discovered_by`、清压力、写 `last_migrate_tick`。
- `server/src/world/spirit_eye.rs:350-352`：注册时每次从 `ZoneRegistry::load()` + `startup_salt()` 新建 registry 并直接插入 resource，没有读取已保存状态。
- `server/src/world/spirit_eye.rs:281-305`：死亡遗念和坐标笔记资格都从 `discovered_by` 查询。
- 全仓搜索未发现 `SpiritEyeRegistry`/`spirit_eye` 对应的 persist、hydrate、save、load 路径。

## 触发路径

1. 玩家靠近或高境界感知灵眼，`spirit_eye_discovery_tick` 调 `registry.discover()`，`discovered_by` 写入内存。
2. 玩家在灵眼处突破，`record_breakthrough_use_by_id()` 累加 `usage_pressure`；压力达到阈值或 72h 周期触发 `tick_migration()`，灵眼迁移并清空发现名单。
3. server 重启。
4. `register()` 重新 `from_zones()`，上述内存态全部丢失：发现名单为空、压力归零、迁移 tick 归零、非血谷迁移后坐标回到候选初始位置。

## 反方审查记录

- Round 1：反方结论 `REAL`。最强反驳是“血谷灵眼重启漂移是设计允许”以及“固元突破会写入生平记录”；裁决：收窄 bug，不把血谷初始漂移纳入；生平记录只能证明曾经突破，不能恢复 `discovered_by`、压力、迁移 tick 或坐标。
- Round 2：反方结论 `REAL`。确认非血谷初始坐标本身稳定，但迁移后坐标会在重启后回滚；Redis/tiandao 只是事件缓冲，不是 server hydrate 源；`known_spirit_eyes` 单点不够强，但私有 HUD、坐标笔记交易和迁移压力共同构成明确游玩影响。

## Skeleton Fix Plan

- [ ] 给 `SpiritEyeRegistry` 增加 versioned runtime snapshot：每口灵眼至少保存 `eye_id`、`dimension`、`pos`、`discovered_by`、`usage_pressure`、`last_migrate_tick`、`zone_name`、`blood_valley`、`qi_concentration`。
- [ ] 启动 hydrate：先按当前 zones 构建候选和初始灵眼，再按 `eye_id` 合并已保存运行态；缺失或无效条目走保守 fallback，不阻塞启动。
- [ ] 变更落盘：发现、突破使用、迁移后标 dirty，并按现有 persistence 风格节流 flush；server shutdown/AppExit 必须强制 flush。
- [ ] 明确血谷规则：保留“未被 runtime snapshot 覆盖的血谷初始选择可随重启变化”；一旦有运行态 snapshot，就以 snapshot 为准，避免已迁移/已发现状态回滚。
- [ ] 将 DeathInsight、坐标笔记、私有 marker 都继续读同一个 authoritative registry，避免并行缓存。

## 验收测试计划

- [ ] `SpiritEyeRegistry` 单测：discover 后 snapshot/hydrate，`discovered_by` 保留，`private_marker_entries` 与 `known_spirit_eyes_for` 可继续命中。
- [ ] 突破使用单测：`usage_pressure` 与 `last_migrate_tick` snapshot/hydrate 后保留。
- [ ] 迁移单测：非血谷灵眼迁移后 snapshot/hydrate，`pos` 不回到 `from_zones` 初始候选。
- [ ] shutdown flush 单测：dirty registry 收到 `AppExit` 后落盘，重启 hydrate 不丢最后一次发现/迁移。
- [ ] 回归测试：血谷未产生 runtime snapshot 时仍允许 startup salt 影响初始候选；有 snapshot 时不得覆盖已保存运行态。

## 风险

- `eye_id` 目前包含 zone slug 和 slot，zone 配置调整可能让旧 snapshot 指向不存在候选；hydrate 需要校验维度和 zone 合法性。
- snapshot 若盲目覆盖 `qi_concentration`，可能遮蔽 zones/worldgen 平衡调整；需要决定哪些字段 authoritative，哪些字段可从新 zones 刷新。
- `discovered_by` 是私有情报资产，落盘位置和日志输出必须避免无意广播。
