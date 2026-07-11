# plan-bughunt-baolongwang-bossdrain-zone-shadow

> BugHunt worker：server-qi r10
> 主题：暴龙王 BossDrain 只落 WorldQiAccount zone 镜像，不写回 ZoneRegistry.spirit_qi，导致玩家被抽走的真元在真实环境层不可见，并可能被后续 field-authority 重同步抹掉。

## 结论

`server/src/dandao/boss_spawn.rs:337-411` 的 `baolongwang_qi_drain_aura_system` 在暴怒/崩溃阶段会按距离持续扣玩家 `Cultivation.qi_current`，然后把同等 `actual_drain` 加到 `WorldQiAccount` 的 `zone:baolongwang_cavern_deep` 账户并 `push_transfer_audit(QiTransferReason::BossDrain)`。

问题是该系统没有 `ResMut<ZoneRegistry>`，也没有同步 `ZoneRegistry.spirit_qi`。而仓库当前的 zone qi 权威范式是 field-authority：`zone:<name>` ledger balance 是 `ZoneRegistry.spirit_qi * QI_ZONE_UNIT_CAPACITY` 的镜像，不是长期权威余额。`server/src/world/heartbeat.rs:2288-2292` 的 `zone_qi_inflow_tick` 会用 `zone.spirit_qi.max(0.0) * QI_ZONE_UNIT_CAPACITY` 覆写同名 zone ledger balance；`server/src/world/zone.rs:321` 也明确说明 BossDrain 这类 qi 入账场景需要直接修改 `zone.spirit_qi`。

因此当前实现只让测试看到“玩家减少量 == ledger zone 增加量”，但真实游玩层读到的环境灵压没有变化；一旦后续系统按字段权威重同步，BossDrain 刚加进 zone 镜像账的增量还会被陈旧 `zone.spirit_qi` 覆盖。

## 实际游玩体验影响

玩家在暴龙王光环范围内会稳定掉真元，战斗压力是真实的；但这些被抽走的真元不会稳定回到暴龙王巢穴环境。玩家看到的是“自己被吸干了”，可区域灵压、负灵域强度、后续依赖 `ZoneRegistry.spirit_qi` 的感知/风险/回血/生态判断不会按这笔吸取变化，长期 Boss 战等同于把玩家真元蒸发掉，破坏 `docs/worldview.md §二 L30-L46` 灵压环境与 `docs/worldview.md §十 L870-L879`“灵气零和、修炼消耗就是别人少掉”的守恒体验。

## 证据

- `server/src/dandao/boss_spawn.rs:382-408`：先 `cultivation.qi_current -= actual_drain`，再 `account.set_balance(zone_id, zone_balance + actual_drain)`；没有读取或写入 `ZoneRegistry`。
- `server/src/dandao/boss_spawn.rs:930-998`：现有回归只断言玩家减少量等于 `WorldQiAccount` zone 账户增加量，并检查 `BossDrain` 审计记录；没有断言 `ZoneRegistry.spirit_qi` 同步，也没有推进 heartbeat 重同步链。
- `server/src/qi_physics/ledger.rs:463-467`：`push_transfer_audit` 的语义是“余额已在外部正确更新，此处仅留轨迹”；BossDrain 不能把“只改 ledger 镜像”当成最终入账。
- `server/src/world/heartbeat.rs:2288-2292`：回流系统会把 zone ledger 镜像重设为 `zone.spirit_qi.max(0.0) * QI_ZONE_UNIT_CAPACITY`。
- `server/src/world/zone.rs:321`：注释点名 BossDrain 后续等 qi 入账场景需要直接修改 `zone.spirit_qi`。
- `docs/plans-skeleton/plan-bughunt-skull-fiend-drain-zone-shadow.md:44-53`：骨煞 plan 已把 BossDrain 列为同类旧范式审计对象，但明确“不在本 plan 范围内混修”。

## 去重说明

不是 #1050 craft qi_cost 固定落 `zone:spawn`；不是 #1056 NPC 日程/休息/QiSpring/Far LOD 凭空恢复真元；不是 #1076 灵田 `plot_qi`；不是 #1082 灵蝗潮推进扣 zone qi；不是 #1089 垂死大能 rift drain；不是 #1096 气针过期抹掉负灵域缺口。

最接近的是 #1046“骨煞抽真元落入 zone 镜像账”。本发现与 #1046 是同型 field-authority bug，但对象不同：#1046 修 `SkullFiendDrain`，这里是丹道暴龙王 `BossDrain`。骨煞 skeleton 还把 BossDrain 列为后续拆分审计对象，因此本 plan 是未覆盖的独立候选。

## 修复要求

- [ ] `baolongwang_qi_drain_aura_system` 接入 `ResMut<ZoneRegistry>`，按 `BOSS_HOME_ZONE` 找到真实 zone，并将 `actual_drain / QI_ZONE_UNIT_CAPACITY` 写回 `zone.spirit_qi`，与 ledger zone 镜像保持一致。
- [ ] 保留玩家真元在 ECS `Cultivation.qi_current`、审计用 `push_transfer_audit` 的既有语义；不要把玩家余额镜像进 `WorldQiAccount` 后调用 `transfer()`。
- [ ] 处理 zone 缺失或 ledger 缺失时的原子性：不能先扣玩家再入账失败；失败时应跳过本 tick 吸取或保证玩家扣减与环境入账同成同败。
- [ ] 补回归测试：暴龙王光环 tick 后断言玩家减少量、zone ledger 增量、`ZoneRegistry.spirit_qi` 增量三者按 `QI_ZONE_UNIT_CAPACITY` 对齐。
- [ ] 补 heartbeat 覆盖链回归：BossDrain 后推进一次会触发 zone 镜像重同步的系统，断言刚入账的真元不会被陈旧 `zone.spirit_qi` 抹掉。

## 对抗复核

- 第 1 轮 Socrates：判定成立；指出 BossDrain 只写 `WorldQiAccount zone:<BOSS_HOME_ZONE>`，没有写字段，和 #1043/#1046 骨煞是同型但不同对象。
- 第 2 轮 Beauvoir：判定不允许；强调 `zone:<name>` 是 `ZoneRegistry.spirit_qi` 镜像，不是长期权威余额，现有测试只放过了 ledger 增量，未覆盖字段权威链。
