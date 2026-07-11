# plan-bughunt-active-events-restart-loss-v1

> BugHunt persistence r10。仅记录真实 bug 与修复计划，不做实际修复。

## 一句话

`ActiveEventsResource.active_events` 中仍在进行的长时世界事件没有持久化；关服/重启会丢掉事件队列、`elapsed_ticks`、`duration_ticks` 与运行态，导致兽潮、域崩倒计时和天灾生命周期被硬切断。

## 范围

- 包含：`server/src/world/events.rs` 的 `ActiveEvent` / `ActiveEventsResource` 长时事件队列，以及 `beast_tide`、进行中的 `realm_collapse`、`thunder_tribulation`、`heavenly_fire`、`pressure_invert`、`all_wither`、`poison_miasma`、`meridian_seal`、`daoxiang_wave` 等通过 `ActiveEvent` tick 推进的事件。
- 不包含：已完成域崩的 `collapsed` overlay、`WorldHeartbeat.active_pseudo_veins` 伪灵脉持久化、独立 `tribulations_active` 渡劫持久化、静态 `zones.json active_events` 标签、即时 `karma_backlash` roll。

## 证据

1. `server/src/world/events.rs:101-114`：`ActiveEvent` 持有 `event_name`、`zone_name`、`elapsed_ticks`、`duration_ticks`、`target_player`、`thunder`、`beast_tide`、`collapse`、`calamity_state` 等运行态，但不是 persistence record。
2. `server/src/world/events.rs:297-337`：`spawn_event` 由命令创建 `ActiveEvent`，初始 `elapsed_ticks = 0`。
3. `server/src/world/events.rs:838-927`：enqueue 时只把事件名写入 `zone.active_events`，随后 `self.active_events.push(event)` 写入内存队列。
4. `server/src/world/events.rs:964-990`、`998-1520`：tick 只推进内存队列；过期清理也依赖队列仍在。
5. `server/src/world/events.rs:1620-1633`：启动注册直接 `insert_resource(ActiveEventsResource::default())`，没有 hydrate。
6. `server/src/persistence/mod.rs:475-479`：`ZoneRuntimeRecord` 只保存 `zone_id/spirit_qi/danger_level`，没有 `active_events`、倒计时或事件运行态。
7. `server/src/persistence/mod.rs:661-685`、`854-875`：persistence 注册了 zone runtime 的周期/关服 flush，但没有 `ActiveEventsResource` 的 flush/hydrate 系统。
8. `server/src/persistence/mod.rs:3095-3138`、`5635-5665`：zone runtime snapshot 写读列仍只有 `zone_id/spirit_qi/danger_level`。

## 复现路径

1. 触发一个长时世界事件，例如 `spawn_event` 的 `beast_tide`，或让低灵气监控进入 `realm_collapse` 撤离倒计时。
2. 在事件未过期时正常关服或重启 server。
3. 新进程启动后 `ActiveEventsResource` 为空；现有 persistence bootstrap 不恢复该队列。
4. 原事件不再继续 tick，也不会发后续提醒、VFX/SFX、落雷/灼烧/道伥波次、域崩最终结算或过期清理。

## 实际游玩体验影响

玩家看到的长时世界事件会被重启硬切断：兽潮可能凭空消失，或只留下没有生命周期管理的怪；域崩倒计时停止，原本应横死/塌陷的区域继续存在；天灾预兆、HUD 倒计时、VFX/SFX、后续落雷/灼烧/道伥波次中断。体感不是单纯 UI 误差，而是世界事件没有跨服连续性，甚至会让重启成为规避高危事件的手段。

## 去重

- 不重复 #1052、#1058、#1064、#1078、#1084、#1090：这些分别是化虚动作冷却、散真元珠、长期状态效果、配方解锁 flush、物资棺冷却、天道注意力。
- 不重复 #969-#1098 其他主题；相近的 #1082 是灵蝗潮路径灵气守恒，#1098 是伪灵脉 agent 叙事链路，均不是世界事件调度器运行态持久化。
- 不重复 `docs/plan-bughunt-ao-worldgen-state-pseudo-vein-restart-loss-v1`：该 plan 针对 heartbeat 伪灵脉动态 zone/生命周期；本 plan 针对 `ActiveEventsResource.active_events` 的通用长时事件队列。
- 不重复 finished `plan-tribulation-v1`：已完成域崩 overlay 能恢复永久 collapsed 标记，但不能恢复进行中的倒计时、入侵击杀、提醒和最终结算；独立渡劫有自己的 `tribulations_active` 持久化，不作为本 plan 证据。

## 修复方向

- 新增 versioned `active_world_events` persistence record，至少保存 `event_name`、`zone_name`、`elapsed_ticks`、`duration_ticks`、`intensity`、`target_player`、事件子状态中可恢复且必须恢复的字段。
- 在 `PersistenceSettings` bootstrap 后 hydrate `ActiveEventsResource`；只恢复仍未过期、zone 仍存在、事件类型受支持的记录。
- 在周期 snapshot 与 `AppExit` 上 flush 队列；写入时过滤即时事件与不可恢复 transient pending 队列。
- 明确事件级策略：`karma_backlash` 不入表；已完成 `realm_collapse` 仍以 overlay 为权威；伪灵脉和渡劫继续走各自专用持久化。
- 恢复时同步 `zone.active_events` 标签，避免队列与 zone snapshot 分叉。

## 验收

- server 单测：构造 `beast_tide` 运行到一半，flush 后新 `ActiveEventsResource` hydrate，断言 `elapsed_ticks/duration_ticks/zone_name/event_name` 保留，继续 tick 后正常过期清理 `zone.active_events`。
- server 单测：构造进行中的 `realm_collapse`，重启恢复后仍进入撤离提醒窗口，并在倒计时结束触发 `ZoneCollapsedEvent` 与 collapsed overlay。
- server 单测：`karma_backlash` 不被持久化，已完成 `realm_collapse` 只依赖 overlay 恢复。
- e2e/smoke：触发长时世界事件，中途重启 server，玩家重连后仍看到正确剩余倒计时和后续结算。

## 对抗结论

- 第一轮找洞：确认 `ActiveEventsResource` 队列只在内存，重启后长时世界事件会丢失。
- 第一轮反驳：未发现新增候选，但列出的重复项不覆盖本主题。
- 第二轮反驳：`ACCEPT`，要求收窄为“仍在进行的长时/有运行时副作用事件缺少恢复”，并排除伪灵脉、渡劫、已完成 overlay 与静态 zone 标签。
- 第二轮补强：`ACCEPT`，确认 `beast_tide`、进行中的 `realm_collapse` 与 calamity 系列受影响，`karma_backlash` 与已完成域崩不受影响。
