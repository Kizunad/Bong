# BugHunt Skeleton Plan — 武器修复绕过工作站/材料门禁

一句话：`RepairWeaponIntent` 已作为生产 UI/C2S 入口暴露，但 server 端未校验工作站、距离、维度、材料或 session，直接把玩家自己库存内的 weapon instance 耐久修满。

状态：Skeleton。仅记录 bug 与修复计划骨架，不包含实际修复。

## 接入面

- 进料：client 装备页武器右键菜单 `修复` → `RepairScreen` → `repair_weapon_intent`。
- 出料：server 修改 `PlayerInventory` 中目标 `ItemInstance.durability`，随后发 inventory snapshot。
- 共享类型 / event：现有 `ClientRequestV1::RepairWeaponIntent`、`InventoryDurabilityUpdate`、`PlayerInventory`、`ItemRegistry`。
- 跨仓库契约：client `ClientRequestProtocol.encodeRepairWeapon` / server `schema::client_request::RepairWeaponIntent` / agent schema `RepairWeaponIntentRequestV1`。
- worldview 锚点：worldview.md §四 L413 凡器磨损；worldview.md §四 L557 武器/工具是手持装备；worldview.md §十六 L1535 保养容器与耐久经济。
- qi_physics 锚点：本 plan 不改变真元/灵气流动。若最终修复选择“修复消耗真元/骨币/带灵质材料”，必须走 `qi_physics::ledger::QiTransfer`，不得在 repair handler 内自写真元扣减或凭空生成。

## 实际游玩体验影响

1. 玩家装备一把低耐久武器后，在装备页右键武器点“修复”，无论是否站在锻炉/修理台旁，都会把 server 权威耐久改成 1.0。
2. `RepairScreen` 上“投入精钢锁”和“投入丹药”两个按钮都会走同一个 `sendRepairWeapon(instance_id, station_pos)`；材料类型没有进入协议，也没有被 server 消耗。
3. 恶意或脚本客户端可直接发送 `repair_weapon_intent`，只要 `instance_id` 属于该玩家 `PlayerInventory` 内的 weapon instance，即使物品在容器/热栏而非装备槽，也会被修满。
4. 这会绕过 `plan-forge-v1` 的工作站/材料经济，让武器耐久失去实际成本；与 `plan-weapon-v1` “RepairWeaponIntent 由 forge 工作站触发”的契约相反。

边界：不主张能修他人物品、地面掉落物或非武器；`fully_repair_weapon_instance` 会拒绝没有 `weapon_spec` 的 template。

## 复现路径

### 普通 UI 路径

1. 准备一把 `weapon_spec` 武器，并让其 `durability < 1.0`。
2. 打开 Inspect 装备页，把该武器放入手持装备槽。
3. 右键装备槽武器，选择“修复”。
4. 在 `RepairScreen` 点击任一材料按钮。
5. 观察 server 下发的新 inventory snapshot：同一 `instance_id` 的 `durability` 变为 1.0，且没有工作站存在性、距离、维度或材料消耗前置。

### C2S trust-boundary 路径

1. 让玩家库存容器或 hotbar 中存在一把低耐久 weapon instance。
2. 发送 `{"type":"repair_weapon_intent","v":1,"instance_id":<该武器实例>,"station_pos":[0,64,0]}`。
3. 观察该 weapon instance 被修满；`station_pos` 只进入日志，不参与校验。

## 根因证据

- `server/src/network/client_request_handler.rs:1855-1868`：`RepairWeaponIntent` arm 直接调用 `handle_repair_weapon`。
- `server/src/network/client_request_handler.rs:11767-11817`：`handle_repair_weapon` 只取 `PlayerInventory` 并调用 `fully_repair_weapon_instance`；没有 `Position`、`CurrentDimension`、forge station query、材料消耗或后续 event/system 二次校验。
- `server/src/inventory/mod.rs:3873-3893`：`fully_repair_weapon_instance` 只校验 template 有 `weapon_spec`，随后 `set_item_instance_durability(..., 1.0)`。
- `server/src/inventory/mod.rs:3974-3997` 与 `server/src/inventory/mod.rs:5587-5613`：按 `instance_id` 搜索 `containers/equipped/hotbar`，所以伪造 C2S 可覆盖玩家自己库存内的 weapon instance。
- `client/src/main/java/com/bong/client/inventory/InspectScreen.java:2118-2185`：普通 UI 仅装备页右键装备槽进入武器菜单；背包 grid 分支不直接开武器修复菜单。
- `client/src/main/java/com/bong/client/inventory/InspectScreen.java:4145-4201`：武器菜单含“修复”，`openRepairScreen` 把 `station_pos` 填成玩家当前位置，而非真实工作站坐标。
- `client/src/main/java/com/bong/client/combat/screen/RepairScreen.java:51-57`：两个材料按钮传入的 `material` 在 `weaponInstanceId > 0` 路径被丢弃，只发 `sendRepairWeapon`。
- `agent/packages/schema/src/client-request.ts:340-349`：`RepairWeaponIntentRequestV1` 只有 `instance_id` 与 `station_pos`，没有材料、数量、station entity 或 session id。
- `docs/finished_plans/plan-weapon-v1.md:424`：历史契约写明 `RepairWeaponIntent` 触发为 `plan-forge-v1 工作站`。
- `docs/finished_plans/plan-weapon-v1.md:492`：耐久修复需 `plan-forge-v1` 工作站。
- `docs/finished_plans/plan-forge-v1.md:370`：完整修复系统仍是 TODO；当前问题不是 TODO 未做，而是未完成入口已经改生产状态。

## 去重记录

- 已避开既有 BugHunt 主题：#1048 灵木满包吞产物、#1055 ForgeStationPlace 坐标门禁、#1060 物资棺跨维 session gate。
- 本地 grep `docs/plan-*.md docs/plans-skeleton docs/finished_plans` 对 `RepairWeaponIntent/repair_weapon_intent/武器修复/修复武器/免费修/远程修` 只命中 forge/weapon 历史 TODO 与契约，没有同题 `plan-bughunt-*`。
- `gh pr list --state all --limit 320` 按 `RepairWeapon|repair_weapon|武器.*修|修复.*武器|远程.*修|免费.*修|forge.*repair|repair` 过滤无输出。

## 修复计划骨架

- [ ] P0：先封口。`handle_repair_weapon` 在没有权威 repair station/session 的情况下必须拒绝，不再直接满修；拒绝后发 inventory snapshot / toast，保证 UI 不假绿。
- [ ] P1：定义权威修复入口。二选一：A) 复用 forge station entity/session；B) 新增明确的 repair station component。无论选哪条，都必须由 server 校验 station 存在、同维度、距离、玩家状态。
- [ ] P2：补材料契约。扩 `RepairWeaponIntent` 或新增 repair session submit，使材料 template/count 由 server 从库存验证并消耗；客户端按钮不得只传本地标签。
- [ ] P3：耐久边界。明确耐久 0、装备中、容器中、hotbar 中、断线/死亡中、武器已不存在等状态的拒绝/允许规则。
- [ ] P4：反馈与可观察性。拒绝原因进入 server_data 或 chat/toast，Bot e2e 可黑盒观察；成功路径下发 inventory snapshot 与材料减少。

## 验证计划

- server 单测：`repair_weapon_intent` 无 station/session 时拒绝且不改 durability。
- server 单测：站点同维度且距离内 + 材料足够时成功，武器耐久变 1.0，材料减少，revision 增加。
- server 单测：站点不存在、跨维、超距、材料不足、非武器 instance、他人/不存在 instance 全部拒绝且状态不变。
- schema pin：若协议扩字段，更新 Rust / TS / proto / samples，对缺字段、坏字段、未知 station/session 做反向样例。
- client 回归：装备页修复按钮在 server 拒绝时显示明确反馈，不关闭成假成功；材料按钮发送可被 server 验证的信息。
- Bot e2e：用 dev 命令造低耐久武器，发送坏 `repair_weapon_intent` 断言耐久不变/收到拒绝；再走合法站点路径断言耐久与材料变化。

## 对抗复核

两轮对抗已完成。

- 反方第一轮质疑：这可能只是 `plan-forge-v1` 已知 TODO；“背包菜单直接修复”表述过宽；“任意武器”需收窄；需要证明没有后续校验路径。
- 主 agent 修正：把 bug 定义为“未完成入口已暴露并修改生产状态”；UI 可达仅主张装备页已装备武器；背包/热栏/容器内武器只作为伪造 C2S trust-boundary 风险；补充 schema、client、server helper 与去重证据。
- 反方最终裁决：通过。该候选足以开只含 Skeleton Plan 的 PR，核心是已暴露的生产 UI/C2S 入口绕过工作站、距离、材料与 session 校验，直接把玩家自己库存内的 weapon instance 权威耐久改为 1.0。
