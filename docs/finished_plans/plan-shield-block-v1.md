# plan-shield-block-v1 — 手持盾牌格挡机制

> 一句话：让 wooden_shield / bone_shield 从「合成出来装不上的纯孤岛」变成可装备 off_hand、按住右键持续举盾的格挡机制——原版式正面减伤，但持续消耗体力、盾有耐久、格挡给 shield_block skill exp；全程不涉真元（纯凡人级物理防御）。
>
> 来源：僵尸物品审计「盾不可装备(2)」类；调查 workflow 2026-06-10（7 维摸底 + opus 抽查 5/5 证据属实）；PR #470 Pi review 3 条措辞修正已并入（见各阶段注）。

**依赖**：无硬依赖（独立于套包 4-plan 族）。OffHand 校验接入面与套包族、consumable-effects 交叉，见 §7。

| 阶段 | 主题 | 状态 | 验收 |
|------|------|------|------|
| P0 | 装备打通（消灭孤岛根因） | ⬜ | YYYY-MM-DD |
| P1 | 持续状态协议 + 右键输入层 | ⬜ | YYYY-MM-DD |
| P2 | 减伤 + 体力 drain + 正面判定 | ⬜ | YYYY-MM-DD |
| P3 | 耐久 + 破盾 | ⬜ | YYYY-MM-DD |
| P4 | skill exp + 视听四件套差异化 | ⬜ | YYYY-MM-DD |

---

## 接入面（防孤岛 checklist）

- **进料**：
  - `inventory` 装备流程：`EquipSlotV1::OffHand` 校验分支（`server/src/inventory/mod.rs:3799-3821`，现硬编码只放行 Treasure，否则要求 `weapon_spec.weapon_kind` 为 Dagger/Fist——盾无 weapon_spec 故走 `ok_or_else` 报错，**这是僵尸根因**）
  - 物品模板：`server/assets/items/workbench_materials.toml:655-675`（wooden_shield/bone_shield，现 `category="armor"` 无任何 spec）
  - category 解析：`server/src/inventory/mod.rs:1955-1971` `parse_item_category`（现 `"armor"|"armour"`→Armor，无 shield 分支）
  - 槽校验入口（**核心抓手，PR #470 review 修正**）：`server/src/inventory/mod.rs:3799` 的 `EquipSlotV1::OffHand` match arm。装备目标槽 `EquipSlotV1::OffHand` 由 caller（client move/equip 请求）显式携带（见 `mod.rs:6683` 测试范例 `slot: EquipSlotV1::OffHand`），**不经 item_id 反推**。盾装 off_hand 只需在此 arm 加 `ItemCategory::Shield` 放行分支即可
  - **`equip_slot_for_item_id`（`server/src/armor/mundane.rs:245`）不在盾的路径上**：其签名为 `fn equip_slot_for_item_id(item_id: &str) -> Option<EquipSlotV1>`，函数体走 `parse_mundane_armor_item_id`（按 `armor_<material>_<slot>` 后缀匹配），**拿不到 `ItemCategory`，无法对 `Shield` match**；且唯一 caller 在 `mod.rs:3888` 的 armor 槽分支（Head/Chest/Legs/Feet 求 `expected_slot`），OffHand 校验在 3799 是独立 arm 根本不调它。故本 plan **不改 `equip_slot_for_item_id`**（grep 已确认 OffHand 不经此函数）
  - 体力：`server/src/combat/lifecycle.rs:207-240` `stamina_tick` 状态机（`COMBAT_DRAIN_PER_SEC=5.0`、`JOG=2.0`、`SPRINT=10.0`，`lifecycle.rs:75-77`，是持续 drain 先例；**穷尽 match 无 `_` 兜底**，新增 StaminaState 变体不改此函数即编译报错——设计安全网）
  - 朝向：`server/src/combat/jiemai.rs:190-213` `jiemai_fov_check`（facing.dot(to_attacker) vs 随境界变化阈值——**只参照结构不调用**，盾用固定 dot）
  - 减伤管线：`server/src/combat/resolve.rs:895-908` SwordParrying 分支（`active_status_magnitude(...,SwordParrying)`→clamp(0,0.95)→削 severity/bleeding/contam，附 0.15 反伤；盾分支并列插入，**无反伤**）
- **出料**：
  - `server/src/combat/resolve.rs:895` 之后插 ShieldBlocking 减伤分支（独立变量，不与 sword_parry 共享）
  - `ItemInstance.durability`（`server/src/inventory/mod.rs:350` `pub durability: f64` 0..=1 normalized）经 `set_item_instance_durability`（`mod.rs:2593`，返回 `Option<ItemInstance>` 供 caller 检测归零）扣耐久
  - `KnownTechniques` proficiency：`record_shield_block_success`（参照 `server/src/combat/sword_basics.rs:234` `record_sword_parry_success`，签名 `(world: &mut World, defender: Entity)`，**须在 resolve `commands.add(move |world|...)` 延迟闭包内调用**，照搬 `resolve.rs:942-950` SwordParry 接线点——主体伤害结算无现成 `&mut World`；**emit+consume 同 PR 真接 callsite**）
  - 破盾事件 → client toast（参照 `server/src/combat/weapon.rs:187-195` `WeaponBroken { entity, instance_id, template_id }` 先例，新建 `ShieldBroken`）
- **共享类型 / event**（新增变体，连带穷尽 match 与 wire）：
  - `ItemCategory::Shield`（`server/src/inventory/mod.rs:225`）
  - `StatusEffectKind::ShieldBlocking`（`server/src/combat/events.rs:115` 之后；**不复用** SwordParrying——剑式格是「窗口」模型，盾是「持续」模型，语义不同必须并列）
  - `StaminaState::ShieldBlocking`（`server/src/combat/components.rs:85`，连带 `stamina_tick` 穷尽 match）
  - `DefenseKind::ShieldBlock`（`server/src/combat/events.rs:75`）→ `CombatDefenseKindV1::ShieldBlock`（`server/src/schema/combat_event.rs:46`）→ `map_defense_kind` 穷尽 match（`server/src/network/combat_bridge.rs:298-302`）
  - `ShieldSpec` 新 struct（**不扩 ArmorProfile**——`server/src/combat/armor.rs:60-67` `validate()` 硬检查 slot∈{Head,Chest,Legs,Feet}，复用会在加载期被拒）
- **跨仓库契约**：
  - server：`ClientRequestV1::RaiseShield` / `ClientRequestV1::LowerShield`（`server/src/schema/client_request.rs:35`）
  - client：`CombatKeybindings.java:105-130` spellVolumeKey hold 边沿范式（照搬给举盾键）、`MixinMouse.java:32-65`（现仅拦 `GLFW_MOUSE_BUTTON_LEFT`，右键 button==1 须新拦 + cancel）、`InventoryEquipRules.java:83-88`（OFF_HAND case 加 isShield）、**新增** `BongAnimations.SHIELD_RAISE = new Identifier(MOD_ID, "shield_raise")`（`BongAnimations.java:35` 旁，**不复用** `GUARD_RAISE`——见下「资产隔离红旗」）、复用 `BongAnimations.PARRY_BLOCK`（`BongAnimations.java:36`，命中动画无冲突）、`CombatJuiceEvent.Kind.SHIELD_BLOCK`（`CombatJuiceEvent.java:54`）、`WeaponBrokenHandler.java:21-43`（破盾 handler 照此结构）
  - **资产隔离红旗（PR #470 review blocker 修正）**：`guard_raise.json` 已被 `BaomaiSkillId::FullPowerCharge` 复用（`vfx_animation_trigger.rs:361` `ANIM_GUARD_RAISE = "bong:guard_raise"`，4-tick snap-up 蓄力姿态）。`guard_raise.json` 是 PlayerAnimator 加载成的**唯一 KeyframeAnimation 实例**（键 `bong:guard_raise`），`isLoop` 是该实例属性——把它翻 true 会让 FullPowerCharge 蓄力动画也无限循环（命中 docs/CLAUDE.md §四「覆盖既有触发」+ memory `feedback_playeranimator_gotchas`「单帧循环衰减到 defaultValue」）。**故本 plan 不动 guard_raise.json**：举盾走全新独立资产 `shield_raise.json`（`isLoop:true` 持续举盾姿态）+ 新 server 常数 `ANIM_SHIELD_RAISE = "bong:shield_raise"`（`vfx_animation_trigger.rs:35` 旁）+ 新 `BongAnimations.SHIELD_RAISE`；`guard_raise.json` 的 `isLoop:false` 保持不动给 FullPowerCharge
  - agent：**不参与**（纯战斗机制，无天道介入面；agent 侧不新增 schema / Redis key）
- **worldview 锚点**：`docs/worldview.md:432` §「防御三流」——「将真元外放形成护盾是不可能的……防御的本质是处理已经打到肉体上的物理冲击与真元污染」。三流（截脉/替尸/涡流，`worldview.md:434-446`）均为修士功法防御；凡木/骨盾填的是三流之外的**凡人级物理防御**空白，醒灵/引气期无功法玩家亦可用，与正典不矛盾。注意 §涡流流「盾面强行制造负灵域」（`worldview.md:445`）是修士施法作用面，**非本 plan 的物理盾**，不得混淆为「盾牌带真元」。
- **qi_physics 锚点**：**无，且必须保持无**。盾牌格挡全程不产生、不消耗、不转移真元；体力（stamina）是物理量不是真元，drain 不走 `QiTransfer` ledger，不引入任何 `*_DRAIN*` 真元常数。plan 内禁止出现真元参与的「附魔盾/灵盾」设计（那属 worldview §五 涡流流领域，不在本 plan）。

---

## P0 — 装备打通（消灭孤岛根因）

**目标**：`/give wooden_shield` 后能装入 off_hand 槽。消灭「合成出来装不上」僵尸根因。

- 新增 `ItemCategory::Shield` 变体（`server/src/inventory/mod.rs:225`，带 doc 注释「plan-shield-block-v1 P0」）
- `parse_item_category`（`server/src/inventory/mod.rs:1955-1971`）加 `"shield"` → `ItemCategory::Shield` 分支
- TOML 两盾 category 从 `armor` 改 `shield`（`workbench_materials.toml:655-675`），并加 `shield_spec`（见 P3 字段定义；P0 先放占位，使加载链通过）
- `inventory/mod.rs:3799` OffHand 校验：在 Treasure 放行之后、weapon_spec 校验之前，加 `ItemCategory::Shield` 放行分支（同样校验 two_hand 占用拒绝）。这是盾装 off_hand 的唯一校验入口——目标槽由 caller 显式传 `EquipSlotV1::OffHand`，**不改 `equip_slot_for_item_id`**（见 §接入面：该函数拿不到 category 且不在 OffHand 路径上）
- client `InventoryEquipRules.java:83-88` OFF_HAND case：加 `isShield(category)` 条件与 server 对齐
- 配方 id 归位：`workbench.weapon.wooden_shield` / `workbench.weapon.bone_shield` → `workbench.shield.wooden_shield` / `workbench.shield.bone_shield`（`server/src/craft/workbench_recipes.rs:789,800`，消除 weapon 命名空间 vs shield category 打架）
- **break 兼容说明**：本阶段改动会破坏 `plan-workbench-recipes-v1`（`docs/finished_plans/`，category=armor + recipe id weapon.*）的历史记录；recipe id 变更对存档无影响（配方按 id 查表，旧 id 无引用）

**测试声明（饱和化）**：
- ItemCategory pin 测试：`Shield` 变体 serde 正反对拍（happy）；`parse_item_category("shield")` / `("Shield")` / `("")`（错误分支）专属 case
- equip happy：`Shield` 物品装入 off_hand 成功
- equip 边界：two_hand 槽占用时拒绝装 off_hand 盾
- equip 错误：非盾非 treasure 非 dagger 物品装 off_hand 仍按原逻辑拒绝（回归保护）
- 路由回归：`equip_slot_for_item_id("iron_chestplate")` → 原 armor slot 不变；`equip_slot_for_item_id("wooden_shield")` 仍返回 `None`（盾不经此函数，OffHand 走 3799 arm，断言不误改该函数）
- client `InventoryEquipRules` 单测：isShield off_hand 放行 / 非盾 off_hand 拒绝
- e2e：`/give wooden_shield` → 装 off_hand → 服务端 equipped 含该槽

## P1 — 持续状态协议 + 右键输入层

**目标**：按住右键持续举盾、松开放下，server 收 raise/lower 增删持续状态。

- schema：`ClientRequestV1::RaiseShield`、`ClientRequestV1::LowerShield`（`server/src/schema/client_request.rs:35`，两条独立 variant；`agent/packages/schema/samples/` 加正反 sample 对拍）
- client 举盾键：照搬 `CombatKeybindings.java:105-130` spellVolumeKey 的 `isPressed()` 轮询 vs `heldLastTick` 边沿检测范式——按下边沿发 RaiseShield、松开边沿发 LowerShield
- client `MixinMouse.java:32-65` `onMouseButton` 扩 `GLFW_MOUSE_BUTTON_RIGHT`（button==1）：**仅当 main_hand/off_hand 持盾**时拦截右键 hold 并 cancel 原版右键派发（防双触发）；右键路由仲裁规则见 §8 #5 决议
- server `client_request_handler` 消费 RaiseShield/LowerShield：
  - RaiseShield → 校验 off_hand 实装盾 → 插入 `StatusEffectKind::ShieldBlocking`（magnitude 暂存 block_ratio，P2 用）+ 独立 `ShieldBlock` component（**不走** `combat/weapon.rs` Weapon component 路径——盾无 weapon_spec）
  - LowerShield → 移除状态 + component
- 强制清理：玩家死亡（接 death lifecycle）/ 断线时强制移除 ShieldBlocking 状态 + component（防残留）

**视听规格（举盾姿态，玩家可感知）**：
- 动画 `SHIELD_RAISE`（**新建** server 常数 `ANIM_SHIELD_RAISE = "bong:shield_raise"`，`server/src/network/vfx_animation_trigger.rs:35` `ANIM_GUARD_RAISE` 旁）：server 在插入 ShieldBlocking 时通过 `emit_play_for_entity` 触发，**isLoop 持续播放**。**禁止复用 `ANIM_GUARD_RAISE`**——`guard_raise.json` 已被 `baomai_anim_for_skill`（`vfx_animation_trigger.rs:361`）绑给 `BaomaiSkillId::FullPowerCharge`（4-tick snap-up 蓄力姿态），且二者共用同一 KeyframeAnimation 实例，改其 `isLoop` 会破坏爆脉蓄力（见 §接入面「资产隔离红旗」）。举盾用独立 ID 后两机制完全解耦，无 priority 竞争——`SHIELD_RAISE` 与 `FullPowerCharge` 的 `GUARD_RAISE` 是两条独立动画轨，玩家同时举盾 + 蓄力时各播各的（仍由 P1 专测断言互不串台）
- 动画文件 **新建** `client/src/main/resources/assets/bong/player_animation/shield_raise.json`（`isLoop:true` 持续举盾姿态，姿态参照 guard_raise.json 双臂 pitch 抬起持举盾但独立成文件）；**`guard_raise.json` 保持 `endTick:4, isLoop:false` 不动**（FullPowerCharge 消费方），不在本 plan 触碰
- 松开过渡：LowerShield 后播放反向放下，新建短过渡（endTick≈4，双臂 pitch 回 0，easeOutQuad）；或直接停 SHIELD_RAISE 让 PlayerAnimator 衰减到 defaultValue（**注意：单帧循环衰减到 defaultValue 是已知坑**，须验证放下视觉到位）

**测试声明（饱和化）**：
- schema sample 对拍：RaiseShield / LowerShield 正反 sample（双端 TypeBox↔serde）
- 状态转换：无盾态→RaiseShield→ShieldBlocking 插入；ShieldBlocking→LowerShield→移除；ShieldBlocking→死亡→强制移除；ShieldBlocking→断线→强制移除
- 错误分支：off_hand 无盾时 RaiseShield 被拒（不插状态）；重复 RaiseShield 幂等（不叠加）
- 动画隔离：玩家同时举盾 + FullPowerCharge 蓄力时，server 分别 emit `bong:shield_raise`（举盾）与 `bong:guard_raise`（蓄力）两条独立动画轨，互不串台（断言两 ID 不共用，guard_raise.json `isLoop` 仍为 false 未被改动）
- e2e：按住右键 → server 收 RaiseShield → 状态插入 + `bong:shield_raise` 发送；松开 → LowerShield → 移除

## P2 — 减伤 + 体力 drain + 正面判定

**目标**：正面持盾受击减伤、背面无效；举盾持续 drain 体力、归零强制放盾。

- `resolve.rs:895` 之后插 ShieldBlocking 减伤分支（**独立于 SwordParrying，无反伤**）：
  - `let shield_block_ratio = active_status_magnitude(..., StatusEffectKind::ShieldBlocking)` → clamp(0,0.95)
  - 须先过 `shield_fov_check`（正面判定）才生效，否则 ratio 视作 0
  - 削减 `wound.severity` / `bleeding_per_sec` / `emitted_contam_delta`，记 `shield_block_success` + `shield_block_ratio`（不写 reflected_damage）
  - 挡下伤害量传给 P3 耐久结算
- `shield_fov_check`（新函数，`server/src/combat/jiemai.rs` 同模块或 `combat/shield.rs` 新模块）：参照 `jiemai_fov_check:190-213` 结构，`facing.dot(to_attacker)` 与**固定阈值** `SHIELD_FOV_DOT`（±120° → dot≥-0.5，见 §8 #4 决议）比较；**不调用 jiemai_fov_check**（其阈值随境界变化，盾不应有境界加成）
- `StaminaState::ShieldBlocking` 变体（`components.rs:85`）+ `stamina_tick`（`lifecycle.rs:207-240` 穷尽 match）加分支：`SHIELD_DRAIN_PER_SEC: f32 = 3.0`（`lifecycle.rs:75` 旁，量级对齐 COMBAT=5.0/JOG=2.0；**非真元常数，是体力 drain，不触 qi_physics**）
- 体力归零 → **强制放盾**：server 端移除 ShieldBlocking 状态 + component + S2C LowerShield 通知 client 收姿态 + 施加短暂 `ParryRecovery`（复用 `events.rs:115` 附近，破势硬直语义）
- `DefenseKind::ShieldBlock`（`events.rs:75`）+ `CombatDefenseKindV1::ShieldBlock`（`combat_event.rs:46`）+ `map_defense_kind` 穷尽 match arm（`combat_bridge.rs:298`）；格挡成功 emit DefenseIntent/DefenseKind 经 `emit_defense_animation_triggers`（`vfx_animation_trigger.rs:97`）触发 PARRY_BLOCK——**兼容现有 DefenseIntent 触发路径，不覆盖**

**视听规格（格挡命中/体力，玩家可感知）**：
- HUD：体力 drain 直接反映在现有左下角体力竖条（参 memory `project_hud_qualitative_status`）——**无新增 layer**；强制放盾瞬间体力竖条触底闪一次（复用现有 store 数据，不加常驻元素）
- 命中动画 `PARRY_BLOCK`：见 P4 差异化（短 recoil）

**测试声明（饱和化）**：
- happy：正面（dot≥-0.5）持盾受击 severity/bleeding/contam 按 ratio 削减
- 边界：dot 恰在阈值（off-by-one：dot=-0.5 命中 / dot=-0.51 不减伤）
- 背面：背面受击（dot<-0.5）ratio 视作 0，不减伤
- 无反伤断言：ShieldBlock 不产生 reflected_damage（与 SwordParry 区分）
- drain：满体力持续举盾可维持时长 = max_stamina / 3.0（数值断言）；体力归零自动放盾 + ParryRecovery 施加
- 状态转换：StaminaState 各变体进入 ShieldBlocking / 退出（穷尽 match 编译保证）
- wire 双端 sample：CombatDefenseKindV1::ShieldBlock 正反对拍 + map_defense_kind 全 variant 覆盖

## P3 — 耐久 + 破盾

**目标**：格挡按挡下伤害扣耐久，归零破盾、物品销毁、client toast。

- 新 struct `ShieldSpec`（`inventory/mod.rs`，并列 weapon_spec/container_spec 加载分支 `mod.rs:1669+`，带 `validate()` 参照 `armor.rs` 风格）：
  - TOML 字段 `shield_spec = { block_ratio = <f64>, durability_max = <f64 次满伤格挡>, stamina_drain_per_s = <f32> }`
  - **不扩 ArmorProfile**（`combat/armor.rs:60-67` validate 硬拒非四槽）
  - 两盾数值见 §8 #1/#3 决议（木 block_ratio 0.5 / durability_max 40；骨 0.65 / 80）
- 格挡命中（P2 的 `shield_block_success`）按挡下伤害折算扣 `ItemInstance.durability`，经 `set_item_instance_durability`（`mod.rs:2593`）写回；检测返回归零
- 归零 → emit `ShieldBroken { entity, instance_id, template_id }` event（新 struct，参照 `combat/weapon.rs:187` WeaponBroken，**近义重名红旗回避：用 ShieldBroken 不复用 WeaponBroken**，盾非 weapon）+ 从 inventory 移除物品（盾销毁）
- combat_bridge 序列化 ShieldBroken 发往 client（参照 WeaponBroken 序列化路径）
- client：新建 `ShieldBrokenHandler.java`（参照 `WeaponBrokenHandler.java:21-43` 结构，`handledWithEventAlert` + ToastSpec + VisualEffectState）
- client off_hand 耐久条：**收口决策（见 §8 #3）**——盾非 Weapon 不走 `WeaponEquippedStore`/`EquippedWeapon`，新建独立 `EquippedShieldStore` + off_hand 槽耐久渲染（参照 `WeaponHotbarHudPlanner.java:93-94` 渲染逻辑但独立 store，**避免语义污染 WeaponEquippedStore**）

**视听规格（破盾，玩家可感知）**：
- 粒子（按材质差异）：
  - 木盾破碎 — `ShieldWoodShatterPlayer`（**新建**，基类 `BongSpriteParticle`，复用/新增 wood debris 贴图 `bong:particle/wood_debris`）：burst 12 颗，lifetime 10t，radial 放射状速度，颜色 `#8B6F47`（木褐）
  - 骨盾破碎 — 复用 `FaunaBoneShatterPlayer`（`client/.../particle/FaunaBoneShatterPlayer.java:8`，基类 `BongLineParticle` burst radial）：传颜色 `#E8DCC8`（骨白），count 12，duration 10t
  - vfx_event ID：`bong:vfx_event` `shield_break_wood` / `shield_break_bone`
- 音效 audio_recipe（破盾，新建 `shield_break.json`，参 `sword_shatter.json` 已用 `minecraft:item.shield.break`）：
  - layer1：`entity.zombie.break_wooden_door`，pitch 1.2，volume 0.8，delay_ticks 0
  - layer2：`entity.item.break`，pitch 1.0，volume 0.7，delay_ticks 2
- HUD toast：`ShieldBrokenHandler` 发 ToastSpec「盾已碎裂」，颜色 `0xFFC04040`（参 WeaponBrokenHandler「武器损坏：」配色），时长默认 toast 时长，边缘闪烁 VisualEffectState

**测试声明（饱和化）**：
- ShieldSpec 加载：木/骨盾 shield_spec 字段正确解析；缺字段 / 非法 block_ratio 被 validate 拒绝（错误分支）
- 耐久递减曲线：连续 N 次满伤格挡后 durability 单调下降到预期值（边界：第 39 次未破 / 第 40 次归零破盾，木盾）
- 破盾事件：durability 归零 → emit ShieldBroken（一次，不重复）+ inventory 物品移除
- 不破：未归零时不 emit、物品保留
- wire：ShieldBroken 双端 sample 对拍
- e2e：连续格挡至破盾 → client 收 toast「盾已碎裂」 + 物品从 off_hand 消失

## P4 — skill exp + 视听四件套差异化

**目标**：格挡成功给 shield_block 熟练度；木/骨盾命中视听按材质差异化。

- shield technique 注册 `KnownTechniques`：technique_id 用 `shield_block` 前缀（**命名歧义防御，§8 #7**——避开 `woliu_vortex_shield`）
- `record_shield_block_success`（新函数，参照 `sword_basics.rs:234` `record_sword_parry_success` 签名 `(world: &mut World, defender: Entity)`）：读 KnownTechniques → 更新 proficiency。**接线模式硬约束**：因签名需 `&mut World` 而 P2 的 resolve ShieldBlocking 分支处在伤害结算主体（无现成 `&mut World`），**必须在 resolve 的 `commands.add(move |world: &mut World|{...})` 延迟闭包内调用**，照搬 `resolve.rs:942-950` SwordParry 接线点（先 `let defender = target_entity;` 再 `commands.add(move |world| record_shield_block_success(world, defender))`）；**禁止在拿不到 World 的地方调用导致编译失败或退化成 emit-only 孤岛**。**emit 与 consume 同 PR 真接 P2 resolve callsite**（前车之鉴：`GuangboTicaoPracticeEvent` emit 侧断链 `body_conditioning.rs:45,77`、`generic_proficiency_scalars` `#[allow(dead_code)]` 孤岛 `technique_proficiency.rs:97-115`——**禁止声称「复用」孤岛**）
- 熟练度缩放：**内联到新函数** `shield_block_profile(proficiency) -> (block_ratio, drain_per_s)`（仿 `sword_profile` `sword_basics.rs:134`，**不依赖** `generic_proficiency_scalars` 孤岛）：
  - block_ratio 随熟练度小幅上浮（基础 + 上浮，上限见 §8 #6：木 0.5→0.6 上限 / 骨 0.65→0.72 上限）
  - stamina drain 随熟练度小幅下降（3.0→2.0 下限，防满熟练度无限举盾——§8 #6）
- **招式依赖经脉**：shield_block 是凡人级物理防御**不依赖任何经脉**——经 `cultivation::meridian::severed::SkillMeridianDependencies::declare("shield_block", vec![])`（空 vec），明示无经脉依赖，避免被通用 `check_meridian_dependencies` 漏判（与 qi_physics 同级强约束，必走 declare）

**视听规格（格挡命中四件套差异化，玩家可感知，用户硬约束：各招差异化 animation+粒子+SFX+HUD）**：
- 粒子（命中迸溅，区别于 P3 破碎）：
  - 木盾 — `ShieldWoodShatterPlayer`（复用 P3，小规模参数）：burst 6 颗，lifetime 8t，沿受击法线方向，颜色 `#8B6F47`，vfx_event `shield_block_wood`
  - 骨盾 — `FaunaBoneShatterPlayer`（复用）：burst 6 颗，骨白 `#E8DCC8`，lifetime 8t，vfx_event `shield_block_bone`
- 音效 audio_recipe（命中，新建 `shield_block.json`）：
  - 木盾：layer1 `item.shield.block`，pitch 0.9，volume 0.9，delay 0
  - 骨盾：layer1 `item.shield.block`，pitch 1.3，volume 0.8，delay 0；layer2 `entity.skeleton.hurt`，pitch 1.0，volume 0.3，delay_ticks 1
  - （按材质选 recipe：木盾走 `shield_block.json`，骨盾走 `shield_block_bone.json`，由 template_id 路由）
- 命中动画 `PARRY_BLOCK`（`server/src/network/vfx_animation_trigger.rs:34` `ANIM_PARRY_BLOCK`，`parry_block.json` endTick:16 isLoop:false，短 recoil 语义）：格挡命中触发，**新增消费方且兼容现有 DefenseIntent 触发路径**（`emit_defense_animation_triggers:97` 已对所有 DefenseIntent 触发，ShieldBlock 的 DefenseKind 走同路径）
- juice：新 `CombatJuiceEvent.Kind.SHIELD_BLOCK`（`client/.../combat/juice/CombatJuiceEvent.java:54`，**不复用 PARRY**——其音效硬编码剑击声）
- HUD：格挡成功事件流一条（复用现有事件流基础设施 `handledWithEventAlert`，**非新 layer**；按 HUD 极简 + conditional 原则，未持盾时无任何盾 UI）
- narration（scope=player，style=perception，简短第一人称体感，非天道叙事）：
  - 「盾面一震，那一下被卸开了大半。」
  - 「骨盾发出一声脆响，裂纹爬上盾沿。」（接近破盾时）
  - 「臂膀发酸，撑不了几下了。」（体力低时）

**测试声明（饱和化）**：
- happy：格挡成功 → proficiency 单调上涨
- emit-consume 接线：record_shield_block_success 在 resolve 的 `commands.add` 延迟闭包内被真实调用（参照 `resolve.rs:942-950`），跑完一帧后断言 defender 的 proficiency 真变化（覆盖该 callsite，**不是 emit-only 孤岛**）
- 缩放：`shield_block_profile(0.0)` 基础值 / `(1.0)` 上限值 / 中间插值；drain 下限 clamp（边界）
- 熟练度收益封顶：满熟练度 block_ratio 不超上限、drain 不低于下限
- 视听差异化：木盾命中 vfx_event=shield_block_wood + audio recipe pitch 0.9；骨盾 = shield_block_bone + pitch 1.3 + skeleton.hurt 第二层——**每盾独立断言**（材质路由正确）
- HUD conditional：未持盾时无盾 UI 元素渲染（对齐 HUD conditional 原则）
- meridian：`SkillMeridianDependencies` 含 shield_block 空依赖条目（断言注册存在）

---

## §7 与其他 plan 的交叉

- **套包 4-plan 族**（`docs/plan-nested-pack-base-v1` 等，2026-06-10 已升 active）：盾占 off_hand 与套包 loadout/装备校验共用 `inventory/mod.rs:3799` 一带；先 merge 者定基线，后实施者 rebase，**不各改一套 OffHand 逻辑**
- **plan-consumable-effects-v1**（active）：右键输入层冲突——手持消耗品 vs 手持盾的右键路由须统一仲裁（按 main_hand/off_hand 物品类型分发）。consumable-effects 当前无右键拦截代码（消耗品走 QuickSlot/`cast_emit.rs:565` 非原版右键），**本 plan P1 先写 MixinMouse 右键仲裁**，规则见 §8 #5
- **SwordParrying**（已落地，`plan-sword-basics-v1`）：盾格挡是其「持续版」姊妹机制，**复用先例不复用类型**（StatusEffectKind/DefenseKind 各自独立变体）
- **plan-woliu-v3**（finished）：`vortex_shield` 技能护盾——本 plan 全 symbol 用 `shield_block` 前缀避开命名冲突（§8 #7）

## §8 开放问题（已在 §8.1 收口；原表保留以备追溯，实施以 §8.1 为准）

1. **block_ratio 数值**：木盾/骨盾各挡多少？
2. **stamina drain 速率**：举盾静置 vs 命中追加？
3. **durability_max**：木/骨各多少次有效格挡？
4. **FOV 阈值**：±120°（dot≥-0.5）还是收紧 ±90°？
5. **右键路由仲裁**：与 consumable-effects 谁先立规则？
6. **熟练度收益曲线**：block_ratio 上浮上限 / drain 下降下限？
7. **命名**：本 plan 全 symbol 用 `shield_block` 前缀避开 `woliu_vortex_shield`。

## §8.1 决议（pre-P0 收口，基于调研 workflow 2026-06-10 + 实地 grep 证据）

### #1 block_ratio 数值
**决议**：① 木盾 0.5 / 骨盾 0.65（骨盾轻而韧但脆——靠耐久平衡而非减伤短板）。② 写入 TOML `shield_spec.block_ratio`，resolve 取值后 `clamp(0.0, 0.95)`。③ 拒绝 >0.7 基础值——避免凡人盾压过修士功法防御（worldview §五 三流应更强）。
**落点**：`workbench_materials.toml:655-675`（shield_spec）/ `server/src/combat/resolve.rs:895`（clamp 框架同 SwordParrying:899）/ plan P2/P3。

### #2 stamina drain 速率
**决议**：① 举盾静置持续 `-3.0/s`（量级对齐 COMBAT=5.0/JOG=2.0）。② 命中不额外折体力（避免与减伤双重惩罚使盾鸡肋）——drain 仅来自持举。③ 拒绝命中追加消耗。
**落点**：`server/src/combat/lifecycle.rs:75-77`（SHIELD_DRAIN_PER_SEC 常数旁）/ `lifecycle.rs:207-240`（stamina_tick 分支）/ plan P2。

### #3 durability_max
**决议**：① 木盾 40 次满伤格挡 / 骨盾 80 次（骨盾减伤高+耐久高，但 base_weight 轻 2.5 vs 木 3.0，定位「贵但好」，靠合成成本平衡）。② 按挡下伤害折算扣减（满伤=1 次），非固定每次 -1。③ off_hand 耐久条独立 `EquippedShieldStore`，**不复用 WeaponEquippedStore**（盾非 Weapon，复用即语义污染）。
**落点**：`workbench_materials.toml`（shield_spec.durability_max）/ `server/src/inventory/mod.rs:2593`（set_item_instance_durability）/ `client/.../hud/`（新 EquippedShieldStore）/ plan P3。

### #4 FOV 阈值
**决议**：① 固定 `SHIELD_FOV_DOT = -0.5`（±120°）——比 ±90° 宽容，符合「举盾护住正面扇区」直觉，但背后偷袭仍穿。② 固定值不随境界变化（凡人盾无境界加成，区别于 jiemai_fov_dot_threshold）。③ 拒绝复用 jiemai_fov_check。
**落点**：`server/src/combat/jiemai.rs:190-213`（参照结构）/ 新 `shield_fov_check` / plan P2。

### #5 右键路由仲裁
**决议**：① 本 plan P1 先写 MixinMouse 右键拦截与仲裁（consumable-effects 当前无右键代码）。② 仲裁规则：`off_hand=shield 且按住右键 → 举盾（cancel 原版右键）`；`main_hand=consumable → 归使用（不拦，让 consumable plan 路由）`；`否则透传原版右键`。③ 仲裁写在 MixinMouse 一处，consumable-effects 后续 rebase 接入同一仲裁器，不各写一套。
**落点**：`client/.../mixin/MixinMouse.java:32-65`（现仅拦左键）/ plan P1 / §7 consumable-effects 交叉。

### #6 熟练度收益曲线
**决议**：① block_ratio 上浮上限：木 0.5→0.6、骨 0.65→0.72（封顶，仍 <0.95 硬上限与修士防御差距）。② drain 下降下限：3.0→2.0/s（不归零，防满熟练度无限举盾）。③ 缩放内联 `shield_block_profile(proficiency)`，线性插值 clamp 到上下限，**不依赖 generic_proficiency_scalars 孤岛**。
**落点**：`server/src/combat/sword_basics.rs:134`（sword_profile 范本）/ 新 `shield_block_profile` / plan P4。

### #7 命名
**决议**：① 本 plan 全部 symbol（technique_id / event / status / fn）用 `shield_block` 前缀。② DefenseKind/StatusEffectKind 变体用 `ShieldBlock`/`ShieldBlocking`。③ 避开 `woliu_vortex_shield`（finished）的 `vortex_shield` 命名空间。
**落点**：全 plan 各阶段 symbol 命名 / `docs/finished_plans/plan-woliu-v3.md`（命名隔离依据）。

---

## §10 实施工作流（scope = 5 PR，consume-plan 按此执行）

### §10.1 视觉资产多轮打磨 + PROMISE 担保
本 plan 含**动画文件 + 粒子贴图**视觉资产，按 docs/CLAUDE.md §6.1 强制 3 轮：
- **新建** `shield_raise.json`（isLoop:true 举盾姿态，P1）：Round1 新建文件（**不改 guard_raise.json**，它属 FullPowerCharge） → Round2 用 `client/tools/render_animation.py` headless 渲染验证举盾姿态（参 memory `reference_animation_render_tool`，注意 torso/legs 不共祖、单帧循环衰减坑） → Round3 与 spec 一致性 → 终轮 commit 写 `<PROMISE>` 块
- `bong:particle/wood_debris` 贴图（P3/P4，若新建）：走 `/gen-image particle`（memory `feedback_item_icon_gen` + `feedback_gen_image_transparency_failures` 全量扫透明度）；3 轮打磨
- **纯逻辑代码 PR（P0/P2）不适用本节**，atomic commit + 测试全绿即可

### §10.2 PR 拆分点（依赖顺序，前一个 merge 后开下一个）
1. **PR-1（P0）装备打通**：ItemCategory::Shield + parse + 路由 + OffHand 校验 + TOML category + recipe id 归位 + client InventoryEquipRules（纯逻辑，独立成 PR）
2. **PR-2（P1）持续状态 + 右键输入**：ClientRequestV1 schema + MixinMouse 右键仲裁 + 举盾键 + client_request_handler + 新 ANIM_SHIELD_RAISE/BongAnimations.SHIELD_RAISE 接线 + 新建 shield_raise.json（含动画 3 轮，**不动 guard_raise.json**）
3. **PR-3（P2）减伤 + drain + FOV**：resolve ShieldBlocking 分支 + shield_fov_check + StaminaState + DefenseKind wire（纯逻辑）
4. **PR-4（P3）耐久 + 破盾**：ShieldSpec + 扣耐久 + ShieldBroken event + ShieldBrokenHandler + EquippedShieldStore + 破盾视听（含粒子 3 轮）
5. **PR-5（P4）skill exp + 命中视听差异化**：record_shield_block_success + shield_block_profile + meridian declare + CombatJuiceEvent.SHIELD_BLOCK + 命中四件套（含粒子复用/3 轮）

### §10.3 subagent 配置（context 隔离）
每个 PR 起独立 subagent，主线只接 result：
```
Agent(subagent_type: "claude", model: "opus",
      prompt: "...本 PR 范围 + 必读 §10.1 多轮 + 测试要求...\n\nultrathink")
```
subagent 只负责实施 + 提 PR，不等 review；等待逻辑归主线。

### §10.4 CR / Pi 等待协议
- `gh pr checks <PR>`：pass→merge；pending→`ScheduleWakeup delaySeconds=1200` 等下回合；fail→按 commands/consume-plan.md 严重性桶处理
- 最多 3 回合（60 min）卡死才停交人工；修完 review **必须重等 CR re-review**
- 等 CodeRabbit + Pi agent (github-actions) 两 bot 都确认无阻塞，Pi 写 ✅ Approve 才合（memory `feedback_wait_coderabbit_approve`）
- 每 PR 各走完整等待协议，前一个未收敛不开下一个

### §10.5 PR 提交前对峙自检（P3+ 替换 Verify 阶段）
每 PR push 前跑 opus 主导 + 多 sonnet 并行对立观点对峙自检（辩方/控方/玩家视角 → opus 逐点裁决），重点核：emit-consume 真接线（record_shield_block_success 非孤岛）、qi_physics 零侵入（无真元常数）、视听材质差异化到位（memory `feedback_consume_presubmit_debate` + `feedback_system_loop_priority`）。

### §10.6 单次 consume-plan 全自动到 merge
用户提交 `/consume-plan shield-block-v1` 后即可下班，醒来看本 plan 是否在 `docs/finished_plans/`。全部 P ✅ + Finish Evidence 写完后由 consume-plan 在 PR 末尾 commit 内 `git mv` 归档。
