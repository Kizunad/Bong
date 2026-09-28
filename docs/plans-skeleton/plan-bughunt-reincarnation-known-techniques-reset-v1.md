# plan-bughunt-reincarnation-known-techniques-reset-v1

> 来源 Issue：#1929。转世路径清空了 cultivation、经脉和技能栏，却保留旧实体的 `KnownTechniques`。

## §0 摘要

转世分支在 `cultivation/mod.rs` 中创建默认 `Cultivation`、`MeridianSystem` 和 `SkillSet`，但没有插入 `KnownTechniques::default()`。玩家登录时旧功法已由 `attach_player_state_to_joined_clients` 挂到实体；转世保存的 slices 又不会删除旧功法行，因此“新角色”仍保留前世功法和熟练度。另一个手动新生入口 `reset_for_new_character` 已显式清空，两个入口行为不一致。

## §1 实际游玩体验影响

- 永久死亡后重连即可保留全部前世功法，死亡重置失去意义。
- 玩家通过按钮开启新生会丢功法，而通过转世门反而保留，产生明显的规则漏洞。
- 旧功法还会继续参与冷却、熟练度和持久化，影响新角色后续成长。

## §2 复现路径

1. 给角色写入至少一条 `KnownTechniques` 和熟练度，令 `LifeRecord` 进入 `Terminated`。
2. 重连触发 `attach_player_state_to_joined_clients`，再进入 `if let Some(spec) = reincarnation` 分支。
3. 观察分支只插入默认 `SkillSet`，没有覆盖 `KnownTechniques`；持久化 `player_known_techniques` 行仍在。
4. 对照 `combat/lifecycle.rs:1952` 的 `reset_for_new_character`，确认另一条新生入口会清空。

## §3 根因证据

- `server/src/cultivation/mod.rs:1169-1200` 的转世分支插入 fresh inventory、`SkillSet::default()` 和位置，但没有 `KnownTechniques::default()`。
- `server/src/player/mod.rs:234-279`（当前 attach 链）会先把持久化 KnownTechniques 挂到实体，故遗漏不是“组件尚未创建”。
- `server/src/combat/lifecycle.rs:1952` 明确插入 `KnownTechniques::default()`，是同仓正确入口。
- `save_player_slices` 对 `player_known_techniques` 使用保留式写入，转世 cultivation bundle 保存不会自动删除旧功法。

## §4 非重复比对

- `docs/finished_plans/plan-bughunt-reincarnation-clean-slate-v1.md` 覆盖寿元、库存或经脉重置，但没有证明 `KnownTechniques` 在转世分支被清空。
- 这不是 client UI 的功法列表过滤问题；服务端实体和持久化表都已带入旧数据。
- 也不是 agent 叙事状态；新生规则在 server persistence/ECS 内完成。

### 立项检查记录（2026-09-28）

- `docs/worldview.md`：检索“转世、新生、功法、前世”，核对 §十二 L1058-L1060 与 §三 L65-L77。
- `docs/finished_plans/`：检索 `KnownTechniques`、`reset_for_new_character`、`reincarnation`；命中 cultivation/lifecycle 文档，但没有转世门的功法清理证据。
- active plan：检索 `KnownTechniques`、`save_player_slices`、`PlayerCharacterRotationGateway`；持久化/轮换计划未覆盖该转世分支的旧功法行。
- `docs/plans-skeleton/`：检索 `KnownTechniques`、`reincarnation`、`SkillSet::default`；已有功法增长/接线 skeleton 不涉及新生清单，未发现同主题 skeleton。
- `docs/plans-skeleton/reminder.md`：已查阅并检索 `KnownTechniques`、`reincarnation`、`SkillSet::default`，未发现同主题延后事项。

## 接入面与跨仓契约

- **Inputs**：`reincarnation` spec、`LifeRecord`、持久化 player slices、`KnownTechniques`、`SkillSet`、默认 loadout。
- **Outputs**：转世实体与持久化切片都没有前世功法；新角色仍使用现有空技能栏和 cultivation snapshot。
- **共享类型 / 事件**：复用 `KnownTechniques`、`SkillSet`、`save_player_slices`、`CultivationBundleTutorialHandoff`；不新增 agent/client payload。
- **server 契约符号**：`attach_cultivation_to_joined_clients`、`reincarnation` 分支、`KnownTechniques::default`、`reset_for_new_character`、`save_player_slices`。
- **agent**：无变更。agent 不持久化功法组件，也不决定转世清单。
- **client**：无变更。客户端继续消费 server 下发的功法/技能列表；修复只改变新角色初始数据。
- **worldview 锚点**：`docs/worldview.md §十二 L1058-L1060`（夺舍/转世的代价与身份）；`§三 L65-L77`（新生境界起点）。
- **qi_physics**：本 issue 不改变真元，不调用 ledger；若转世同时触发 qi 清零/迁移，必须单独用现有 `Cultivation` 事务和 `SPIRIT_QI_TOTAL` 守恒断言验证，不能用清空 KnownTechniques 代替真元结算。

## §5 修复骨架

### P0 新角色清单一致

- 转世分支显式插入 `KnownTechniques::default()`，并在持久化切片层删除或替换旧功法记录；保证 ECS 与数据库同时为空。
- 复用 `reset_for_new_character` 的清单语义，避免只清内存组件而在下次重连恢复旧行。

### P1 回归与迁移

- 覆盖有/无旧功法、转世重连、手动新生、旧存档迁移和重复触发保护。
- 断言新角色功法、熟练度、技能栏和持久化 slices 均为默认状态，既有 agent/client payload 形状不变。
