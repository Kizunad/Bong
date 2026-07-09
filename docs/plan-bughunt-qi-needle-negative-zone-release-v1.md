# BugHunt: 气针过期回灌抹掉负灵域缺口

## 摘要

`server/src/combat/needle.rs` 的气针过期释放路径在计算落点 zone 当前真元余额时使用了 `zone.spirit_qi.max(0.0) * QI_ZONE_UNIT_CAPACITY`。当气针在 Overworld 负灵域内过期时，负的 `spirit_qi` 会先被抹成 0，再把残余真元释放回 zone，导致负灵域赤字被凭空补齐。

这违反 `docs/CLAUDE.md §四 L59` 的守恒红线：真元/灵气流动必须走 `QiTransfer` 且不能凭空产生；也违反 `docs/worldview.md §二 L18-L20` 的全服灵气总量守恒与压强法则。负灵域本身是正典玩法状态，见 `docs/worldview.md §二 L44-L46`。

## 证据

- 气针过期系统会在超过最大飞行 tick 后释放针容器里的完整 `qi_payload`，然后 despawn。
  - `server/src/combat/needle.rs:147-159`
- 释放 helper 先用 Overworld 落点查询 zone，然后在 `server/src/combat/needle.rs:311` 把负灵气截断：
  - `let zone_current = zone.spirit_qi.max(0.0) * QI_ZONE_UNIT_CAPACITY;`
- `qi_release_to_zone` 本身支持负 `zone_current`，并有负值测试锁定：负灵域接收 0.4 raw qi 时应从 `-0.6` 变 `-0.2`，而不是先归零。
  - `server/src/qi_physics/release.rs:142`
- Overworld 真实 zone 中已有负灵域：
  - `server/zones.json:116-129` `baolongwang_cavern_deep`，`spirit_qi = -0.729232`
  - `server/zones.json:465-478` `wangyintai`，`spirit_qi = -0.15544`
- 现有气针测试覆盖了默认 spawn zone、出界 no-zone、总释放量和零 payload，但没有覆盖 `spirit_qi < 0`：
  - `server/src/combat/needle.rs:507`
  - `server/src/combat/needle.rs:557`
  - `server/src/combat/needle.rs:632`

## 守恒账

以默认气针 `qi_payload = 1.0`、落点 zone `spirit_qi = -0.5` 为例：

- 正确账：zone raw qi 从 `-25.0` 接收 `+1.0`，结果应为 `-24.0`，即 `spirit_qi = -0.48`。
- 当前账：`-0.5.max(0.0)` 变成 `0.0`，zone raw qi 从 `0.0` 接收 `+1.0`，结果写回 `spirit_qi = 0.02`。
- 漏账：一次射空气针凭空补齐 `25.0` raw qi 的负缺口，并额外加入 `1.0` payload；`QiTransfer.amount` 只记录 `1.0`，无法解释 zone 从 `-25.0` 到 `+1.0` 的跃迁。

## 实际游玩体验影响

玩家在负灵域内使用毒蛊气针，射空或让气针飞到最大距离过期时，落点负灵域会被异常净化：原本应维持倒吸压力的区域会被抬到接近正灵气状态。玩家可以通过反复射空低成本气针，把危险负灵域逐步“刷白”，削弱负灵域的生存压力和区域风险。

## 去重

- 不重复 #1050：craft qi_cost 固定落到 `zone:spawn`。
- 不重复 #1056：NPC 日程/休息/QiSpring/Far LOD 凭空恢复真元。
- 不重复 #1076：灵田 `plot_qi` 未进 `WorldQiAccount`。
- 不重复 #1082：灵蝗潮推进扣 zone qi 未入账。
- 不重复 #1089：垂死大能 rift drain 未落账。
- 与 #975 `dormant 负灵域死亡释放抹掉负缺口` 是同类红旗，但本候选是 `combat/needle.rs` 气针过期容器释放路径，现有气针测试未覆盖，属于未修 sibling finding。

## 修复建议

1. `release_needle_qi_to_zone` 使用 `zone.spirit_qi * QI_ZONE_UNIT_CAPACITY` 作为 `zone_current`，不要 `.max(0.0)`。
2. 新增气针过期负灵域回归测试：
   - 构造覆盖落点的 Overworld zone，`spirit_qi = -0.5`。
   - spawn `QiNeedle { qi_payload: QI_NEEDLE_QI_COST, ... }` 并推进到过期。
   - 断言 zone 从 `-0.5` 变为 `-0.5 + QI_NEEDLE_QI_COST / QI_ZONE_UNIT_CAPACITY`。
   - 断言 `QiTransfer(from=container:qi_needle:..., to=zone:<zone>, amount=QI_NEEDLE_QI_COST, reason=ReleaseToZone)` 存在。
3. 验收命令限 server 栈：
   - `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test combat::needle`

## 对抗结论

- 第 1 轮对抗确认：路径真实可达，`qi_release_to_zone` 支持负余额，现有气针测试无负灵域覆盖；与 #975 同类但非重复。
- 第 2 轮反方确认：`ZoneRegistry::find_zone(DimensionKind::Overworld, pos)` 不过滤负 `spirit_qi`，真实 zones 已有 Overworld 负灵域，候选成立。
