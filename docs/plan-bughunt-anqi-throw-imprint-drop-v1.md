# BugHunt Skeleton Plan - 暗器投掷失败丢失封元 imprint

一句话：`ThrowCarrierIntent` 旧投掷路径在零方向或体力不足失败前先移除 `CarrierStore` imprint，导致手中 charged item 保留但服务端封元权威状态丢失。

状态：Skeleton。仅记录 bug 与修复计划骨架，不包含实际修复。

## 接入面

- 进料：暴露的 `throw_carrier` C2S / `ThrowCarrierIntent`，用于暗器 anqi-v1 持骨投掷路径。
- 出料：成功路径生成 `QiProjectile` + `AnqiProjectileFlight`，并清空手持装备槽；失败路径应保持玩家状态不变。
- 共享状态：`PlayerInventory`、`CarrierStore::imprints_by_instance`、`Stamina`、`CarrierStateV1`。
- 边界：暗器 v2 技能栏释放走 `resolve_anqi_skill`，不是本 plan 主张的必现入口；但 v2 的 hand-slot 载体校验也依赖同一个 `CarrierStore` imprint，因此会受坏状态影响。
- qi_physics 锚点：本 plan 不直接改变全服真元常量或 ledger 公式。修复时不得把封存真元凭空销毁；若需要释放或迁移，必须复用既有 qi_physics / ledger 路径。

## 实际游玩体验影响

1. 玩家持有已充能暗器载体后，若 server 收到零向量 `throw_carrier` 请求，投掷不会生成 projectile，手中 charged item 仍存在，但 `CarrierStore` 中对应 imprint 已被删除。
2. 玩家体力低于 `ANQI_THROW_STAMINA_COST` 时也会进入同类失败路径：投掷失败、物品仍在、imprint 丢失。
3. 后续 `carrier_state` 只能从 `CarrierStore` 读 charged 状态，因此 HUD 会从 charged 退回 Idle，玩家看到手里还有 charged item，但服务端不再承认其封元状态。
4. 自然衰减只遍历现有 imprint，丢失后的 charged item 不再走载体衰减处理；旧投掷路径也无法再次取出 `qi_amount` 生成 projectile。
5. 若后续走暗器 v2 hand-slot 检查，`has_loaded_carrier` 要求 charged item 与 `CarrierStore` imprint 同时存在，坏状态会被判定为未装载载体。

边界：不直接断言常规技能栏暗器 v2 必现，也不直接断言全服真元总量必然破坏。更准确的影响是封存真元对应的运行态记录变成不可达，无法再通过投射物命中、射空、自然衰减等路径正常结算，存在吞账风险。

## 复现路径

### 零向量请求

1. 让玩家手持一个已完成充能的暗器载体，使 `CarrierStore::imprints_by_instance` 存在该 `item.instance_id`。
2. 发送 `ThrowCarrierIntent`，`slot` 指向该手持槽，`dir_unit` 为零向量或归一化后长度为 0。
3. 观察投掷 system 不生成 `QiProjectile`，装备槽仍保留 charged item。
4. 再观察 `CarrierStore::imprints_by_instance` 已不含该 `instance_id`；HUD 状态由 charged 变 Idle。

### 体力不足请求

1. 同样准备已充能载体，并把玩家 `Stamina.current` 降到低于 `ANQI_THROW_STAMINA_COST`。
2. 发送合法方向的 `ThrowCarrierIntent`。
3. 观察投掷失败且 charged item 留在手上，但 imprint 被提前移除。
4. 再次尝试旧投掷或暗器 v2 hand-slot 载体校验，server 找不到封元记录。

## 根因证据

- `server/src/combat/carrier.rs:791-798`：`throw_carrier_intents` 先从装备槽取手持 item，随后立即执行 `store.imprints_by_instance.remove(&item.instance_id)`。
- `server/src/combat/carrier.rs:801-804`：`normalized_dir(intent.dir_unit)` 之后才检查零方向；失败分支直接 `continue`，没有把 `imprint` 插回 `CarrierStore`。
- `server/src/combat/carrier.rs:805-808`：体力不足检查也发生在 `remove()` 之后；失败分支同样直接 `continue`，没有回滚。
- `server/src/combat/carrier.rs:812-835`：只有成功路径才清空手持槽并生成 `QiProjectile` / `AnqiProjectileFlight`，使用的 `qi_payload` 来自已被取出的 `imprint`。
- `server/src/combat/carrier.rs:716-744`：自然衰减只遍历 `store.imprints_by_instance`，不会扫描 charged item 重建已丢失 imprint。
- `server/src/network/carrier_state_emit.rs:86-110`：HUD charged 状态完全来自 `CarrierStore` 中现有 imprint；缺失后会落入 Idle。
- `server/src/combat/anqi_v2.rs:850-867`：暗器 v2 hand-slot 载体检查要求 held item 是 charged template，且 `store.imprints_by_instance.get(&item.instance_id)` 存在并匹配 skill。

## 去重记录

- 已按要求先执行 `gh pr list --state all --limit 600 --json number,title,headRefName,url`，避开 #969-#1116 与 server-gameplay 已列主题。
- 范围内暗器相关主题如 #970 暗器充能完成天道叙事断链、#976 暗器 HUD 跨 session lastTick、#1116 暗器 HUD agent schema 漂移，均不是 `ThrowCarrierIntent` 失败路径提前移除 imprint 未回滚。
- 本地 grep `ThrowCarrierIntent / throw_carrier / imprints_by_instance / CarrierStore / 暗器 / 封元` 未发现同根因 `plan-bughunt-*`。
- 已排除放置容器 session lifecycle 与炼丹炉 scope gate 等真实但重复候选。

## 修复计划骨架

- [ ] P0：重排 `throw_carrier_intents` 的失败校验，先校验方向与体力等所有前置条件，再从 `CarrierStore` 移除 imprint。
- [ ] P0：若未来仍需先取出 imprint，必须在每个 early `continue` 分支把 `(item.instance_id, imprint)` 原样插回。
- [ ] P1：体力扣减、装备槽清空、inventory revision bump、projectile spawn 与 imprint 删除应保持同一成功事务语义；任一步失败不得留下 charged item / Store 不一致。
- [ ] P1：补充明确拒绝反馈，使零向量或体力不足不会表现为“投掷无响应但状态暗坏”。
- [ ] P2：梳理 `throw_carrier` 旧路径与暗器 v2 hand-slot 载体检查的契约，避免同一 charged item 有 item template 与 Store imprint 两套权威状态漂移。

## 验证计划

- server 单测：零向量 `ThrowCarrierIntent` 后不生成 projectile、不扣体力、不清装备槽、`CarrierStore::imprints_by_instance` 保留原 imprint。
- server 单测：体力不足 `ThrowCarrierIntent` 后不生成 projectile、不清装备槽、`CarrierStore::imprints_by_instance` 保留原 imprint。
- server 单测：成功投掷仍删除 imprint、清空手持槽、扣体力、生成携带原 `qi_amount` 的 projectile。
- server 单测：失败后 `carrier_state_emit` 仍报告 charged，而不是 Idle。
- server 单测：失败后 `anqi_v2::has_loaded_carrier` 对同一 held charged item 仍可识别为 loaded。
- 守恒回归：若测试涉及封存真元数值，断言失败路径前后 inventory / carrier / projectile / zone 口径不减少，不写死 `SPIRIT_QI_TOTAL` 字面量。

## 对抗复核

两轮对抗已完成。

- 反方第一轮结论：候选成立，但影响口径必须收窄为 `throw_carrier` C2S / `ThrowCarrierIntent` 旧投掷路径；暗器 v2 技能栏释放走 `resolve_anqi_skill`，不能声称常规技能栏必现。
- 主 agent 修正：把实际影响改为“暴露的旧投掷路径可制造 charged item 仍在但 Store imprint 丢失的坏状态”；真元部分改为“运行态记录不可达、存在吞账风险”，不直接断言全服总量必然不守恒。
- 反方第二轮裁决：收窄后接受。没有发现自动恢复或自动清理路径；#969-#1116 未覆盖同一根因；建议按零向量/体力不足失败路径提前 remove imprint 未回滚开 Skeleton Plan。
