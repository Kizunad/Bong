# plan-bughunt-baomai-v3-aoe-dimension-gate-v1

## §0 摘要

**来源 Issue：#1368。** 撼山 AOE 的 `targets_in_radius` 只查询 `Position` 并按欧氏距离筛选，没有读取 caster 与目标的 `CurrentDimension`。异维实体只要坐标重合就会收到 `AttackIntent` 与眩晕；后续 resolver 的维度字段用于结算/回灌，不能撤销已经发出的命中。本 skeleton 不改生产代码。

接入面：进料是 caster `Position`/`CurrentDimension`、目标 `Position`/`CurrentDimension`、`AttackIntent` 与 `ApplyStatusEffectIntent`；出料是只作用于同维目标的伤害/眩晕事件。复用 `CurrentDimension`、`DimensionKind` 和既有 AOE helper；agent/client 契约不变。worldview §二/§十三的跨维世界边界是本空间门禁依据，qi_physics 不新增物理常数。

## §1 游玩影响

- Overworld 与 Tsy（或其他维度）中位于相同坐标的玩家会被 Void 撼山跨维命中，受到高额伤害和眩晕。
- 跨维战斗破坏传送与安全边界，且命中事件已发出后再依赖 defender_dim 处理死亡回灌无法补救。
- 生产中存在 `apply_dimension_transfers`，所以异维同坐标不是不可达测试构造。

## §2 复现路径

1. 将 caster 放在 `CurrentDimension::Overworld`，目标放在 `CurrentDimension::Tsy`，让两者 `Position` 相同或在 AOE 半径内。
2. 施放 `BaomaiSkillId::MountainShake` 并通过正常境界、经脉、qi 与冷却门。
3. `cast_mountain_shake` 调 `targets_in_radius(world, caster, position, radius)`，返回目标实体。
4. 目标收到 `AttackIntent` 和 `Stunned`，尽管维度不同；对照 `woliu_v2::collect_targets_in_radius` 的显式维度过滤。

## §3 根因证据

- `server/src/combat/baomai_v3/skills.rs:342-407` 的 `cast_mountain_shake` 只把 caster 位置传入 `targets_in_radius`，没有传入 caster dimension。
- `server/src/combat/baomai_v3/skills.rs:1021-1032` 的 query 类型是 `(Entity, &Position)`，筛选条件只有排除 caster 与距离比较；没有 `Option<&CurrentDimension>` 或 dimension equality。
- 同函数返回的每个 target 随即用于 `AttackIntent` 与 `ApplyStatusEffectIntent`，所以这是命中选择的根因，而非后续事件桥接显示问题。
- 对照 `server/src/combat/woliu_v2/skills.rs:603-640` 的 `collect_targets_in_radius` 同时读取 `Option<&CurrentDimension>` 并过滤 `current_dimension != dimension`；证明仓库已有应复用的边界模式。

## §4 非重复比对

- `docs/finished_plans/plan-baomai-v1.md`/v3 技能底盘覆盖 AOE 玩法与 resolver，但没有为本 helper 增加跨维 gate。
- `plan-bughunt-woliu-v2-placeholder-erosion-gate-v1` 的境界 gate 是技能解锁问题，不是空间筛选；`woliu_v2` 只作为本问题的对照实现。
- `docs/finished_plans/plan-bughunt-health-death-chain-v1.md` 的 defender dimension 处理属于命中后的死亡链，不会修复已发出的跨维 AttackIntent。

## §5 修复计划骨架

### P0：AOE 同维筛选

- 在 `targets_in_radius` 读取 caster 的 `CurrentDimension`（缺失时按现有默认策略明确处理），query 目标的 `Option<&CurrentDimension>`，仅在维度相等时返回实体；不要用坐标推断维度。
- 让 `AttackIntent`、眩晕和 `BaomaiSkillEvent.targets_hit` 共用同一过滤后的集合，避免伤害与反馈数量不一致。

### P1：回归契约

- 同维近距目标命中、异维同坐标目标不命中、缺少目标 dimension 的兼容策略各保留代表 case。
- 多目标 AOE 断言异维实体既无攻击事件也无状态效果；攻击者自己的实体仍被排除。
- 不改变既有 radius、qi cost、cooldown 与 `AttackIntent` payload。

## §6 验证计划

实现后运行 server 栈 fmt、clippy、cargo test，重点覆盖 `baomai_v3::skills` 与 dimension 测试。本 skeleton 阶段不编译。

## §7 跨仓契约与可核验锚点

- **Server：** AOE 入口是 `baomai_v3::skills::cast_mountain_shake`，选择器是 `targets_in_radius`，输出集合同时喂给 `AttackIntent`、`ApplyStatusEffectIntent` 与 `BaomaiSkillEvent.targets_hit`；对照实现为 `woliu_v2::skills::collect_targets_in_radius` 的 `CurrentDimension` 过滤。
- **Qi：** 维度筛选不新增物理路径；保留 `cast_mountain_shake` 现有 `spend_qi`/`emit_spent_qi_release`，其中 zone 回灌继续使用 `qi_release_to_zone`、`QiTransferReason::ReleaseToZone`、`QI_ZONE_UNIT_CAPACITY` 与 `QI_EPSILON`。若测试建立总量快照，使用 `assert_conservation` 和 `DEFAULT_SPIRIT_QI_TOTAL`（现有测试 fixture 的 `SPIRIT_QI_TOTAL` 来自 `schema::common`），不得因加 gate 改写 qi 账本。
- **Agent：无变更。** 证据是 `AttackIntent`、状态效果与维度组件均在 server combat resolver 内部，未修改 Redis/schema。
- **Client：无变更。** 证据是 AOE 命中事件 payload 和 HUD/VFX wire 不变，异维目标只在 server 选择阶段被排除。
