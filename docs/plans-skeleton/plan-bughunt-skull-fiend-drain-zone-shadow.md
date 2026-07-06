# plan-bughunt-skull-fiend-drain-zone-shadow（骨架）

> **BugHunt skeleton**。一句话主题：骨煞冲撞命中后扣目标 `Cultivation.qi_current`，但只把真元写进会被 `zone.spirit_qi` 覆盖的 `WorldQiAccount zone:<name>` 镜像账户，不写真实玩法字段 `ZoneRegistry.zone.spirit_qi`，导致环境灵气体感不变，后续重同步还可能抹掉这笔账。

## Bug 摘要

- **核心 bug**：`server/src/npc/skull_fiend.rs::drain_target_qi_to_zone_ledger` 在骨煞命中玩家或 NPC 后，先通过 `credit_skull_fiend_drain` 给 `QiAccountId::zone(zone_name)` 增加 `WorldQiAccount` balance，再扣目标 `cultivation.qi_current`。但该路径没有拿到 `ResMut<ZoneRegistry>`，也没有修改 `zone.spirit_qi`。
- **守恒问题**：仓内已有注释明确 `zone:<name>` 是会被 `zone.spirit_qi * QI_ZONE_UNIT_CAPACITY` 整体覆写的镜像账户。把消耗真元只 credit 到 `zone:<name>`、却不写回 `zone.spirit_qi`，等价于把真元放进一个下次重同步可能被清零/顶替的影子余额。
- **非重复性**：不重复 #975/#989/#1000/#1013/#1020/#1026/#1037/#1043。最接近 #1043，但 #1043 是 NPC 技能 overflow/no-zone 只发事件、不进真实余额；本题是骨煞正常命中、正常解析 zone 后只写 zone ledger 镜像，不写玩法字段。

## 对实际游玩体验的影响

玩家在坍缩渊或通过 `spawn_npc skull_fiend` 遭遇骨煞时，冲撞会真实扣掉玩家或 NPC 的当前真元，玩家看到的是“被骨煞抽空/抽掉真元”。按世界观和 `SkullFiendDrain` 注释，这些真元应逸散回目标所在区域，使局部环境灵气略微回暖或至少被真实承接。

当前实现下，修炼、NPC 决策、生态刷新、区域环境桥等仍读未变化的 `zone.spirit_qi`。所以玩家体感上只承受损失，所在区域不会因为被抽出的真元而变得更可修炼、更高灵压或更接近回暖；多人战斗中骨煞反复撞击越多，越像把玩家/NPC 真元挪进一个玩法不可见的影子账本。后续 dormant/LOD/heartbeat 一类系统按字段权威重同步 zone ledger 时，还可能把这笔 `zone:<name>` balance 覆盖掉，变成真实守恒漂移。

## 证据定位

- `server/src/npc/skull_fiend.rs:667-716`：`drain_target_qi_to_zone_ledger` 解析 zone 和 actor id 后调用 `credit_skull_fiend_drain`，成功才执行 `cultivation.qi_current -= drain`；函数参数只有 `Option<&ZoneRegistry>` 和 `Option<&mut WorldQiAccount>`，没有可变 zone 字段写回能力。
- `server/src/npc/skull_fiend.rs:749-769`：`credit_skull_fiend_drain` 只 `set_balance(zone_account, zone_balance + amount)` 并 `push_transfer_audit`，没有 `qi_release_to_zone`、没有 `zone.spirit_qi` 更新、没有 overflow/cap 处理。
- `server/src/npc/skull_fiend.rs:1123-1147`：现有回归只断言目标 `qi_current` 降到 0、`WorldQiAccount zone:spawn` 余额为 3、存在 `SkullFiendDrain` 审计；没有断言 `ZoneRegistry.spirit_qi` 上升。
- `server/src/qi_physics/ledger.rs:481-489`：`pending_inflow_account` 注释明确 `zone:<name>` 会被 `apply_dormant_regen_with_multiplier` 等系统按 `zone.spirit_qi * QI_ZONE_UNIT_CAPACITY` 整体覆写，credit 进去会被静默清零/顶替。
- `server/src/cultivation/meridian_open.rs:299-303` 与 `server/src/cultivation/breakthrough.rs:608-612`：同类“只注水 `zone:<name>`、不写回 `zone.spirit_qi`”已被注释称为记账蒸发 bug，并迁到独立待分配池。
- `server/src/world/zone.rs:319-324`：`find_zone_mut_by_pos` 注释明确 qi 入账场景需要直接修改 `zone.spirit_qi`。
- `server/src/npc/npc_skill.rs:80-97`：NPC 技能 accepted 分支使用 `qi_release_to_zone` 并写回 `zone.spirit_qi`，可作为“玩法字段为真实环境落点”的对照。

## 触发路径

1. 服务器生成或命令生成骨煞，目标玩家/NPC 带有 `Cultivation { qi_current > 0 }`、`LifeRecord` 和可解析的 `ZoneRegistry` zone。
2. 骨煞进入 `SkullFiendState::Charging` 并命中目标，`tick_skull_fiend_charge` 调用 `drain_target_qi_to_zone_ledger`。
3. `target_qi_drain_amount` 取 `min(target.qi_current, skull.config.qi_drain)`，默认最多 5 点。
4. `credit_skull_fiend_drain` 把这 5 点写进 `WorldQiAccount zone:<当前 zone>`，然后目标 `cultivation.qi_current -= drain`。
5. `ZoneRegistry.zone.spirit_qi` 不变。玩家后续在原地修炼、NPC 选址、区域桥推送都读不到这笔回灌；下一次 zone ledger 镜像重同步还可能覆盖 `zone:<name>` 余额。

## 反方审查记录

### Round 1

- **反方论点**：也许 `WorldQiAccount zone:<name>` 会有系统反向同步到 `ZoneRegistry.spirit_qi`；也许 `SkullFiendDrain` 只需要 ledger 承接。
- **裁决**：反驳失败。未找到 ledger 到 zone field 的反向同步；相反，LOD、dormant、heartbeat 等范式都是先以 `zone.spirit_qi` 覆写 ledger 镜像，再 transfer，说明字段才是玩法权威。

### Round 2

- **反方论点**：如果同时写 `zone.spirit_qi` 和 `zone:<name>` ledger，会让 `summarize_world_qi` 的 `zone_qi + ledger_qi` 双计；BossDrain、ArtifactMaintenance、ArtifactEvolution 也有只写 ledger 的历史范式；本题可能应并入 #1043。
- **裁决**：反驳失败，但修复风险成立。双计问题只说明修复不能天真双加，不说明当前实现正确；Boss/Artifact 更像未迁移旧范式，不能豁免骨煞；#1043 处理的是 NPC 技能 overflow/no-zone，本题是正常 zone 命中后的字段权威缺口。

## Skeleton Fix Plan

1. **重定 `SkullFiendDrain` 落点语义**：明确目标真元被抽出后，accepted 部分必须落到真实环境字段 `zone.spirit_qi`，并按 `QI_ZONE_UNIT_CAPACITY` 换算；不能只长期堆在 `zone:<name>` 镜像账户。
2. **复用统一释放 helper**：优先让骨煞路径使用 `qi_release_to_zone(amount, from, zone_account, zone_current, QI_ZONE_UNIT_CAPACITY)`，对齐 `npc_skill.rs`、`death_hooks.rs`、毒蛊回流等写法。
3. **处理满仓/overflow**：如果目标 zone 已满，overflow 必须进入真实 overflow/pending 账户或其他被 `WorldQiAccount` 持久追踪的账户，不能只发 `QiTransfer` event，也不能被丢弃。
4. **避免 summarize 双计误修**：修复时要定义清楚 `zone.spirit_qi` 与 `WorldQiAccount zone:<name>` 的权威关系。若 accepted 部分写了 field，ledger 侧应作为同步镜像/真实 transfer 的短生命周期账本，而不是额外永久加一份同量余额；测试应锁住字段、账本、总量口径各自语义。
5. **扩展旧范式审计**：本 plan 只要求修骨煞，但实现时建议顺手搜索 `BossDrain` / `ArtifactMaintenance` / `ArtifactEvolution` 是否仍有同类“只写 zone ledger、不写 field”的历史债，必要时拆新 bughunt，不在本 plan 范围内混修。

## 验收测试计划

- `server/src/npc/skull_fiend.rs` 新增回归：命中 `qi_current=3` 目标后，目标真元减少 3，命中 zone 的 `spirit_qi` 按 `3.0 / QI_ZONE_UNIT_CAPACITY` 上升，且审计记录为 `SkullFiendDrain`。
- 新增满仓回归：zone 接近/达到 cap 时，accepted 写回 field，overflow 进入真实可观测账户；不得只留 event。
- 新增镜像覆盖回归：骨煞命中后再跑一次会 `set_balance(zone_account, zone.spirit_qi * QI_ZONE_UNIT_CAPACITY)` 的 dormant/LOD 类同步，不得让本次抽取真元从守恒口径消失。
- 新增非主世界/TSY 维度回归：`CurrentDimension` 传入的目标 zone 必须正确解析，不能硬编码 Overworld。
- 跑 server gate：`cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`。

## 风险

- **双计风险**：同时增加 `zone.spirit_qi` 和永久 ledger zone balance 会让 telemetry 暂时看起来多一份同源真元；修复必须先定 field-authority 与 ledger mirror 的边界。
- **旧范式扩散风险**：Boss/Artifact 路径存在相似写法，修骨煞时不要把所有旧债塞进同一 PR；先让骨煞闭环，再单独立项。
- **负灵域风险**：不要用 `zone.spirit_qi.max(0.0)` 作为释放基线，否则会重复 #975 类负缺口抹平问题。负值 zone 应使用 signed field 基线，再按 helper 结果写回。
- **overflow 风险**：满仓区域不能把未接收真元退回目标、静默丢弃或只发审计事件，必须进入真实账户。

## 本轮取证说明

- 已先查开放 PR，排除 #975/#989/#1000/#1013/#1020/#1026/#1037/#1043 重复。
- 已按 gameplay/qi 要求读取 `docs/CLAUDE.md §四` 与 `qi_physics` ledger/release/constants 相关代码。
- 已做两轮反方 subagent 对抗，结论均为反驳失败。
- 本 PR 只新增本 skeleton plan，不修改实际代码、配置、依赖或资源。
