# plan-bughunt-skill-milestone-worldstate-schema

## Bug 摘要

`agent/packages/schema/src/cultivation.ts` 的 `SkillMilestoneSnapshotV1.skill` 仍是三值闭集：`herbalism / alchemy / forging`。但同一个 schema 包里的 `SkillIdV1` 已经扩到六值：`herbalism / alchemy / forging / combat / mineral / cultivation`，服务端 `SkillMilestoneSnapshotV1::from_runtime` 也会把真实运行态的 `Combat / Mineral / Cultivation` 映射成 `combat / mineral / cultivation`。

结果是：真实 `bong:world_state` 一旦携带战斗、采矿或修行升级里程碑，agent schema 的 `validateWorldStateV1Contract` 和 committed generated JSON schema 会拒绝这类真实快照。这是 agent-schema 合约漂移；当前线上 Tiandao Redis 订阅路径只是 `JSON.parse(message) as WorldStateV1`，不应表述为线上 Tiandao 已直接丢弃 `world_state`。

## 实际游玩体验影响

对实际游玩体验的影响在于调试和回归链路会误判真实成长记录：玩家通过战斗、采矿、开脉/突破获得技能升级后，服务端会把这些 milestone 写入 `life_record.skill_milestones` 并随 `world_state` 发布。schema 驱动的 mock、契约测试、调试工具或未来严格校验入口会把这类真实快照判成非法，导致 Tiandao/agent 侧围绕玩家能力画像、成长历程和 query-player skill milestones 的调试上下文不可信。

这不是玩家当场可见的 HUD 断链，也不是当前 Tiandao 主订阅路径必然拒收 `world_state`；严重度应收窄为 agent-schema 契约/调试回归 bug。

## 证据定位

- `agent/packages/schema/src/cultivation.ts:6`-`12`：`SkillMilestoneSnapshotV1.skill` 只允许 `herbalism / alchemy / forging`。
- `agent/packages/schema/src/skill.ts:16`-`23`：同包 `SkillIdV1` 已允许 `combat / mineral / cultivation`。
- `agent/packages/schema/src/world-state.ts:119`-`120`：`PlayerProfile.life_record` 使用 `LifeRecordSnapshotV1`，该结构包含 `skill_milestones`。
- `agent/packages/schema/src/world-state.ts:246`-`248`、`agent/packages/schema/src/validate.ts:13`-`21`：`validateWorldStateV1Contract` 真实走 TypeBox `Value.Errors`，不是空壳。
- `agent/packages/schema/generated/world-state-v1.json:390` 附近：生成物里的 `skill` 也只展开三值闭集，说明 committed generated schema 同步保留了漂移。
- `server/src/schema/cultivation.rs:63`-`72`：服务端 schema 映射明确输出 `combat / mineral / cultivation`。
- `server/src/schema/proto_gen.rs:11086`-`11101`：服务端样例已经包含 `"采药 Lv.5, 战斗 Lv.3"` 和 `"combat"` milestone。
- `agent/packages/tiandao/src/redis-ipc.ts:347`-`350`：当前线上订阅路径只是 parse + cast，因此本 bug 不应扩大成运行时必然丢包。

## 触发路径

1. 玩家使用卧柳 v2 战斗招式，`server/src/combat/woliu_v2/skills.rs:831`-`838` 发出 `SkillXpGain { skill: SkillId::Combat }`。
2. 玩家采矿入包，`server/src/mineral/inventory_grant.rs:115`-`120` 发出 `SkillXpGain { skill: SkillId::Mineral }`。
3. 玩家开脉或突破，`server/src/cultivation/meridian_open.rs:250`-`256`、`server/src/cultivation/breakthrough.rs:809`-`816` 发出 `SkillXpGain { skill: SkillId::Cultivation }`。
4. 技能系统在升级时写入生平记录：`server/src/skill/mod.rs:36`-`41` 注册 `consume_skill_xp_gain` 后接 `record_skill_lv_up`，`server/src/skill/mod.rs:78`-`99` 把 `SkillLvUp` 写入 `LifeRecord.skill_milestones`。注意触发条件是升级 milestone，不是任意 XP 事件。
5. world_state 发布收集玩家 `LifeRecord`：`server/src/network/mod.rs:1180`-`1183` 查询 `LifeRecord`，`server/src/network/mod.rs:1287`-`1310` 转成 `LifeRecordSnapshotV1.skill_milestones`，`server/src/network/mod.rs:1658`-`1662` 填入 `PlayerProfile.life_record`。
6. agent 侧若用 `validateWorldStateV1Contract` 或 generated schema 校验该真实快照，会因 `combat / mineral / cultivation` 不在旧三值枚举内而失败。

## 反方审查记录

### Round 1：Aristotle

结论：通过第一轮反方审查，但必须降级表述为“schema 合约漂移 / 调试校验会拒”，不能写成“Tiandao 运行时必然丢弃 world_state”。

反方补充确认：真实发布链路、三类真实 XP 来源、`record_skill_lv_up` 写入 `LifeRecord`、`validateWorldStateV1Contract` 的 TypeBox 校验均成立；开放 PR 未覆盖该问题。保留意见是 `agent/packages/tiandao/src/redis-ipc.ts:347` 线上不调用 validator。

### Round 2：Aristotle

结论：通过，但只能作为低风险 agent-schema 契约漂移通过；不能写成玩家当场可见玩法断链。

反方要求收窄：

- 可以说真实 server 可发布 `combat / mineral / cultivation` skill milestone，schema/debug/mock/契约检查会拒绝真实快照。
- 不要说线上 Tiandao 当前会拒收或丢弃 `world_state`。
- 不要说 Tiandao 玩家能力画像线上必然错误；当前多处 query/world-model 逻辑按 string 透传，主风险是 schema 驱动工具和类型契约不可信。
- fix plan 应只改 schema、测试和生成物，不需要服务端改动。

## Skeleton Fix Plan

- [ ] 在 `agent/packages/schema/src/cultivation.ts` 中让 `SkillMilestoneSnapshotV1.skill` 复用或严格对齐 `SkillIdV1`，覆盖 `combat / mineral / cultivation`。
- [ ] 补 `validateWorldStateV1Contract` 样例测试：构造 `players[].life_record.skill_milestones` 分别包含 `combat`、`mineral`、`cultivation` 的 world_state，应从失败转为通过。
- [ ] 检查 `server-data.ts` 里复用 `SkillMilestoneSnapshotV1` 的路径，确保同一修复同时覆盖 server-data schema。
- [ ] 重建 `@bong/schema` dist/generated schema，确认 `agent/packages/schema/generated/world-state-v1.json` 和 `server-data-v1.json` 的 milestone skill enum 与 `SkillIdV1` 一致。
- [ ] 确认 Tiandao query-player / query-player-skill-milestones 不需要代码迁移；若 TypeScript 类型因复用 `SkillIdV1` 收窄而暴露旧假设，再补最小适配。

## 验收测试计划

- 在 `agent/packages/schema` 跑 schema 测试，新增/更新测试覆盖三类新增 milestone skill：
  - `combat` world_state 校验通过。
  - `mineral` world_state 校验通过。
  - `cultivation` world_state 校验通过。
  - 未知 skill 仍应校验失败，证明闭集没有被放宽成任意字符串。
- 在 `agent/` 跑 `npm run build`，确保 dist/generated 与源码 schema 一致。
- 如修复触及 Tiandao 类型消费，再在 `agent/packages/tiandao` 跑 `npm test`。

## 风险

- 低风险：服务端已经在输出六类 skill，本修复是 agent schema 追平服务端事实。
- 主要风险是 schema 复用路径可能改变 generated JSON 的结构形态，需要确认下游只依赖语义枚举而非手写比较 JSON 结构。
- 若把 `SkillMilestoneSnapshotV1.skill` 直接改成 `SkillIdV1`，需避免引入循环 import；若存在循环风险，可先抽公共 skill id 原子或用本地测试保护。

## 验证结论（2026-07-26 整理审计追认）

commit c108aa047（2026-07-07「对齐技能里程碑 schema 枚举」）修复了本 bug：`agent/packages/schema/src/cultivation.ts:9` 的 `SkillMilestoneSnapshotV1.skill` 已改为复用 `SkillIdV1` 六值 union（`herbalism / alchemy / forging / combat / mineral / cultivation`），`generated/world-state-v1.json:390-416` 同步重新生成，schema 与服务端真实运行态输出一致。

## Finish Evidence

- **落地清单**：`agent/packages/schema/src/cultivation.ts`（skill 字段复用 `SkillIdV1`）、`agent/packages/schema/generated/world-state-v1.json`（同步生成物）
- **关键 commit**：c108aa047（2026-07-07，「对齐技能里程碑 schema 枚举」）
- **测试结果**：`agent/packages/schema` 的 `schema.test.ts:3584`、`schema.test.ts:3608` 正反 pin 测试覆盖 combat/mineral/cultivation 通过与未知 skill 拒绝；2026-07-26 审计为只读核验（Read+grep+git log 对拍 origin/main），未重跑测试套件
- **跨仓库核验**：agent `SkillMilestoneSnapshotV1`/`SkillIdV1`（schema 侧收敛）；server `server/src/schema/cultivation.rs:63-72` 输出的 combat/mineral/cultivation 已被 schema 接纳，无需服务端改动
- **遗留 / 后续**：无
