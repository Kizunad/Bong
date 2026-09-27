# plan-bughunt-woliu-v2-placeholder-erosion-gate-v1

## §0 摘要

**来源 Issue：#1866。** `origin/main` 的真实调用链是 `register_skills` 注册 `AmbientVortex`/`VortexEcho`，再由 `cast_ambient_vortex`/`cast_vortex_echo` 统一进入 `resolve_woliu_v2_skill`；两招确实走通用 resolve。它们在 `erosion_placeholder_spec` 中返回 0 qi、0 cooldown、0 duration，而 generic 入口没有调用已有的 `realm_unlocks_skill`，成功后还无条件 `apply_skill_erosion`，所以低境界可以每 tick 免费刷虚蚀，持久化的 `VoidErosion` 最终把角色推到不可追踪阶段。本 skeleton 不改生产代码。

接入面：进料是 `WoliuSkillId`、`Realm`、`skill_spec`、`realm_unlocks_skill` 和 `VoidErosion`；出料应是专属技能 handler 的真实成本/冷却/状态，而不是通用 placeholder 的伪 cast。共享 `CastRejectReason`、`SkillBarBindings`、`add_erosion_capped` 与既有虚蚀 constants；agent/client 无新增 wire。worldview §三/§十的境界门和天道可见性是外部契约，qi_physics 负责任何实际真元成本。

## §1 游玩影响

- 醒灵/引气角色可以绕过虚蚀招式应有的境界门，用零成本零冷却输入迅速堆积 `VoidErosion`。
- `VoidErosion` 持久化并影响 `tiandao_detection_modifier`；刷到 VoidEroded 后会永久进入天道盲区，还可能在突破后跳过预期的逐阶段玩法。
- 这不是只影响 UI 的 placeholder，因为通用成功路径实际修改 ECS erosion 状态。

## §2 复现路径

1. 低境界角色调用 `cast_ambient_vortex` 或 `cast_vortex_echo`；两者当前都直接调用 `resolve_woliu_v2_skill`。
2. `skill_spec` 经 `erosion_placeholder_spec` 返回 `startup_qi=0`、`cooldown_ticks=0`、`duration_ticks=0`；`resolve_woliu_v2_skill` 不检查 `realm_unlocks_skill`，成本检查通过。
3. 通用路径写入状态、设置当前 tick 的 cooldown，并在 `server/src/combat/woliu_v2/skills.rs:475` 调 `apply_skill_erosion`。
4. 重复调用并推进 `VoidErosion`，验证每次增加量和不存在施法冷却/境界拒绝。

## §3 根因证据

- `server/src/combat/woliu_v2/skills.rs:311-481` 的 `resolve_woliu_v2_skill` 只检查 cooldown、位置、Cultivation、qi、经脉和 target，没有 `realm_unlocks_skill(cultivation.realm, skill)` 门。
- `server/src/combat/woliu_v2/skills.rs:1878-1895` 为 `AmbientVortex`/`VortexEcho` 返回 `(0.0, 0, 0, ...)`；通用 `total_qi_cost` 与 cooldown 因而永远放行。
- `server/src/combat/woliu_v2/skills.rs:475-481` 对所有 skill（包含 placeholder）无条件调用 `apply_skill_erosion`；该函数写 `VoidErosion`，不是只生成展示事件。
- `server/src/combat/woliu_v2/erosion.rs:410-427` 已定义每个虚蚀招式的境界解锁表，但生产 resolve 路径没有调用；相关单测只验证函数本身，不能替代运行时 gate。
- `server/src/combat/woliu_v2/erosion.rs:439-448` 的 `add_erosion_capped` 将累计值持久化到 `VoidErosion` 并按境界 cap stage，不能抵消“无限免费输入”的累计污染。
- `server/src/combat/woliu_v2/skills.rs:1876-1910` 的 `erosion_placeholder_spec` 注释声称“不走通用 resolve”，但 `cast_ambient_vortex`/`cast_vortex_echo` 的实际调用链与之相反；该注释是过时契约，修复必须统一为“确实经过 generic resolve”后再决定加 gate 或改专属 handler。

## §4 非重复比对

- `plan-bughunt-woliu-v2-state-replacement-v1` 处理 `TurbulenceField`/`VortexV2State` 覆盖，不处理 skill unlock 或 erosion placeholder。
- `docs/finished_plans/plan-woliu-path-v1.md` 提供虚蚀招式和 constants，但当前 generic resolve 接线未兑现其境界表；该 plan 不是本缺陷的修复证据。
- `plan-bughunt-combat-qi-max-shrink-ledger-v1` 处理余额 clamp，不处理零成本 cast 造成的状态增长。

## §5 修复计划骨架

### P0：专属 handler 与境界门

- 在当前 generic resolve 入口先调用 `realm_unlocks_skill(cultivation.realm, skill)`，未解锁立即返回明确的 realm/skill rejection；对 Heart 等非虚蚀技能保持原门禁。
- `AmbientVortex`/`VortexEcho` 若设计为 toggle/passive，再从这个已确认的 generic 入口路由到各自专属状态机；若保留 placeholder 仅作展示，必须保证 display API 不触发 `apply_skill_erosion` 等 gameplay side effect。不能再以旧注释为依据声称它们不走 generic resolve。
- 明确每次 erosion 的真实来源、冷却和 qi/zone 账本；任何新 cost 只复用 `qi_physics` constants，不在本 plan 自定义公式。

### P1：回归契约

- 各境界对每个虚蚀技能至少一正一负代表 case；负例断言没有 qi、erosion、cooldown 或 VFX 副作用。
- Ambient toggle、VortexEcho passive 和真实 VoidVortex 等路径分别断言其成本/冷却/erosion 只有一次提交；重复输入不能免费刷。
- 持久化/重连后 `VoidErosion.stage` 与合法施法次数一致，不能因旧 placeholder 输入跳阶。

## §6 验证计划

实现后运行 server 栈 fmt、clippy、cargo test，重点覆盖 `woliu_v2::skills`、`erosion` 及持久化测试。本 skeleton 阶段不编译。

## §7 跨仓契约与可核验锚点

- **Server：** 可核验链是 `woliu_v2::skills::register_skills` → `cast_ambient_vortex`/`cast_vortex_echo` → `resolve_woliu_v2_skill` → `skill_spec`/`erosion_placeholder_spec` → `apply_skill_erosion`；境界表为 `erosion::realm_unlocks_skill`，持久化累积为 `erosion::add_erosion_capped` 写入 `VoidErosion`。回归测试必须命中这条生产链，而不是只测 `realm_unlocks_skill` 单函数。
- **Qi：** 当前 generic resolve 的真实成本/zone 回灌沿 `QiTransfer`/`QiTransferReason::Channeling`、`qi_release_to_zone`、`QI_ZONE_UNIT_CAPACITY` 与 `QI_EPSILON`；若修复给 placeholder 增加成本，真实账户必须调用 `ledger.transfer(QiTransfer { from, to, amount, reason })`，不得用 `apply_skill_erosion` 或事件伪造扣款。守恒测试使用 `assert_conservation` 和 `DEFAULT_SPIRIT_QI_TOTAL`（现有 fixture 的 `SPIRIT_QI_TOTAL` 来自 `schema::common`）。
- **Agent：无变更。** 证据是 `VoidErosion`、境界门和 cast resolver 都是 server 内部 ECS/系统；没有新增 Redis/schema 字段。
- **Client：无变更。** 证据是现有虚蚀技能注册、视觉资源和 skill wire id 不变；拒绝发生在 server generic resolve 的 gameplay gate。
