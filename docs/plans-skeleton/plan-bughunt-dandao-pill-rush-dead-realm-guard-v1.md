# plan-bughunt-dandao-pill-rush-dead-realm-guard-v1（骨架）

> 一句话主题：删除或改正 `pill_rush` 对最低境界 `Realm::Awaken` 的恒 false 守卫，让丹道突进的境界契约只有一个真实、可测试的来源。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 对齐技能定义与六境界门槛，拍板“全境界可用”或真实最低门槛 | ⬜ |
| P1 | `resolve_pill_rush` 清除死守卫/接 canonical gate + 边界测试 | ⬜ |
| P2 | server gate | ⬜ |

## 接入面

- **进料**：`server/src/dandao/skills.rs::resolve_pill_rush`、`Realm`、丹道 qi cost 与静态经脉门。
- **出料**：cast 成功/拒绝结果沿现有 skill feedback 与 qi ledger 路径返回。
- **共享类型 / event**：只复用 `Realm` 与现有 skill registry/gate；不另造 realm enum。
- **跨仓库契约**：纯 server 门禁，不改 payload。
- **worldview 锚点**：六境界顺序固定为醒灵→引气→凝脉→固元→通灵→化虚。
- **qi_physics 锚点**：门禁通过后的 qi 消耗路径不变。

## 当前证据（origin/main @ c625d5a5）

`server/src/dandao/skills.rs:221` 仍执行 `(cultivation.realm as u8) < (Realm::Awaken as u8)`；`Awaken` 是首个合法境界，因此所有合法 `Realm` 都不可能命中该分支。缺少 `Cultivation` 时的拒绝仍有效，只有这个 realm 比较是死代码。

## 验收

1. P0 以现有技能定义/plan 决定门槛；不得由实现者凭“修仙常识”新增境界限制。
2. 若全境界可用：删除 realm 比较，并 pin 醒灵与化虚均可继续进入 qi/目标校验；若有真实门槛：使用 canonical realm gate 并覆盖门槛前、等于门槛、门槛后。
3. 缺 `Cultivation`、qi 不足、非法目标与 happy path 回归保持现有可观察结果。
4. 运行完整 server gate。

## 边界

- 不重构整个丹道 skill registry，不改变技能伤害、位移、冷却或 qi cost。
- 不与已归档的 r2 bundle 形成第二实现入口；本文件是该 finding 的唯一 owner。
