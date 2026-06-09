# plan-shield-block-v1 — 手持盾牌格挡机制(骨架)

> 一句话:让 wooden_shield / bone_shield 从"合成出来装不上的纯孤岛"变成可装备 off_hand、按住右键持续举盾的格挡机制——原版式减伤,但持续消耗体力、盾有耐久、格挡给 skill exp。
>
> 来源:僵尸物品审计「盾不可装备(2)」类;调查 workflow 2026-06-10(7 维摸底 + opus 抽查 5/5 证据属实)。

**依赖**:无硬依赖(独立于套包 4-plan 族,但 OffHand 校验接入面交叉,见 §7)。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | 装备打通(消灭孤岛根因) | ⬜ |
| P1 | 持续状态协议 + 右键输入层 | ⬜ |
| P2 | 减伤 + 体力 drain + 正面判定 | ⬜ |
| P3 | 耐久 + 破盾 | ⬜ |
| P4 | skill exp + 视听四件套 | ⬜ |

---

## 接入面(防孤岛 checklist)

- **进料**:
  - `inventory` 装备流程:`EquipSlotV1::OffHand` 校验(`server/src/inventory/mod.rs:3799-3824`,现硬编码只放行 Treasure/Dagger/Fist)
  - 物品模板:`workbench_materials.toml:654-674`(wooden_shield/bone_shield,现 `category="armor"` 无 weapon_spec)
  - 体力:`combat/lifecycle.rs:217` `stamina_tick` 状态机(`StaminaState::Combat=-5/s` 是持续 drain 先例;**穷尽 match 无 `_` 兜底**,新变体不改不编译)
  - 朝向:`combat/jiemai.rs:190-211` `jiemai_fov_check` 的 facing dot 计算(只参照不复用——其阈值随境界变化)
- **出料**:
  - `combat/resolve.rs:895` 减伤管线插 ShieldBlocking 分支(SwordParrying 之后、护甲之前)
  - `ItemInstance.durability`(`inventory/mod.rs:2593` `set_item_instance_durability`)扣耐久
  - `KnownTechniques.proficiency` 加 shield 熟练度(参照 `sword_basics.rs:234` `record_sword_parry_success`)
  - 破盾事件(参照 WeaponBroken)→ client toast
- **共享类型 / event**:
  - 扩展 `ItemCategory`(新增 `Shield` 变体)、`StatusEffectKind`(`combat/events.rs:115` 加 `ShieldBlocking`)、`DefenseKind`(`events.rs:75` 加 `ShieldBlock`,连带 `schema/proto_convert.rs` + `network/combat_bridge.rs` wire 映射)
  - **不复用** SwordParrying 的 StatusEffectKind——剑式格挡是"窗口"模型,盾是"持续"模型,语义不同必须并列
- **跨仓库契约**:
  - server:`ClientRequestV1::RaiseShield / LowerShield`(`schema/client_request.rs:35`)
  - client:`CombatKeybindings.java:112`(R 键 hold 范式)、`MixinMouse.java:32`(现只拦左键,须扩右键)、`InventoryEquipRules.java:83`(OFF_HAND 校验)、`BongAnimations.java:35-36`(GUARD_RAISE/PARRY_BLOCK,**现为零调用死代码**)
  - agent:不参与(纯战斗机制,无天道介入面)
- **worldview 锚点**:§五:432「末法时代真元外放护盾不可能……防御的本质是处理已经打到肉体上的物理冲击」——凡木/骨盾是纯物理防御,填"防御三流"(截脉/替尸/涡流)之外的**凡人级物理防御**空白,醒灵/引气期玩家无功法也能用。
- **qi_physics 锚点**:**无,且必须保持无**。盾牌格挡全程不产生、不消耗、不转移真元;体力(stamina)是物理量不是真元,drain 不走 `QiTransfer` ledger。plan 内禁止出现任何真元参与的"附魔盾/灵盾"设计(那是 worldview §五 涡流流的领域)。

---

## P0 — 装备打通(消灭孤岛根因)

- `ItemCategory::Shield` 新变体(`inventory/mod.rs:225` + parse `mod.rs:1950`);TOML 两盾 `category` 从 `armor` 改 `shield`
- `equip_slot_for_item_id` 路由:Shield → `EquipSlotV1::OffHand`(现 `armor/mundane.rs:245-247` 只解析 mundane armor 返回 None)
- `inventory/mod.rs:3799` OffHand 校验加 Shield 放行分支;client `InventoryEquipRules.java:83` 同步 isShield 条件
- 配方 id 归位:`workbench.weapon.wooden_shield` → `workbench.shield.*`(消除 weapon 命名空间 vs armor category 打架)
- 测试:`/give wooden_shield` 后能装入 off_hand;双端 equip 单测(happy + two_hand 占用拒绝 + 非盾物品拒绝 + ItemCategory::Shield 变体专属 pin 测试)

## P1 — 持续状态协议 + 右键输入层

- `ClientRequestV1::RaiseShield` / `LowerShield`(按下/松开两条;schema sample 正反对拍)
- client 右键 hold 边沿检测(照抄 `CombatKeybindings.java:112` R 键范式);`MixinMouse.java:32` cancel 原版右键派发防双触发——**与 plan-consumable-effects-v1 协调**:两 plan 都吃右键,优先级=手持盾时右键归举盾、手持消耗品时归使用(见 §8)
- server `client_request_handler` 消费:插入/移除 `StatusEffectKind::ShieldBlocking` + 独立 `Shield` component(**不走** `combat/weapon.rs:121` Weapon component 路径——盾无 weapon_spec 该路径不插 component)
- 视听(举盾姿态):`GUARD_RAISE` 真接线(现零调用);`guard_raise.json` **必须改 `isLoop=true`**(现 `isLoop=false/endTick=4`,直接用会 4 tick 后姿态消失);松开播反向放下过渡(endTick≈4,双臂 pitch 回 0,easeOutQuad)
- 测试:e2e 按住右键 → server 收 RaiseShield → 状态插入;松开 → 移除;断线/死亡时状态强制清理

## P2 — 减伤 + 体力 drain + 正面判定

- `resolve.rs:895` 加 ShieldBlocking 分支:`block_ratio` 减伤(独立于 SwordParrying,无反伤),挡下部分进耐久结算(P3)
- `shield_fov_check` 独立实现(参照 `jiemai.rs:211` facing 计算,固定 dot 阈值,建议 ±120°;**不复用** jiemai 的境界变阈值)
- `stamina_tick`(`lifecycle.rs:217`)加 `StaminaState::ShieldBlocking` 持续 drain 分支;体力归零 → **强制放盾**(server 端移除状态 + S2C 通知 client 收姿态)+ 短暂破势硬直
- `DefenseKind::ShieldBlock` + proto_convert + combat_bridge wire 映射(双端 sample 对拍)
- 视听(格挡命中):HUD 体力条 drain 可见(现有左下角体力竖条直接反映,无新增 layer);格挡成功瞬间 client 收 DefenseKind::ShieldBlock → juice(见 P4 差异化)
- 测试:正面受击减伤 / 背面不减伤 / 体力归零自动放盾 / drain 速率边界(满体力持续举盾可维持时长)各专属用例;wire 双端 sample 对拍

## P3 — 耐久 + 破盾

- 盾 durability_max 来源:新 `ShieldSpec`(TOML 字段 `shield_spec = { block_ratio, durability_max, stamina_drain_per_s }`)——**不扩 ArmorProfile**(`armor.rs:61-67` `validate()` 硬编码 head/chest/legs/feet 四槽,直接复用会在加载期被拒)
- 格挡命中按挡下伤害扣 `ItemInstance.durability`(`set_item_instance_durability`,`inventory/mod.rs:2593`);归零 → 破盾事件(参照 WeaponBroken)+ 物品销毁
- client:off_hand 槽耐久条显示 + 破盾 toast
- 视听(破盾):木盾=碎木片粒子(BongSpriteParticle,复用 wood debris 贴图,burst 12 颗,lifetime 10t,放射状,#8B6F47);骨盾=骨粉粒子(burst 12 颗,#E8DCC8);SFX audio_recipe:`entity.zombie.break_wooden_door`(pitch 1.2, vol 0.8)+ `entity.item.break`(delay 2t);HUD toast「盾已碎裂」
- 测试:连续格挡耐久递减曲线 / 归零触发破盾事件 + 物品移除 / client toast e2e

## P4 — skill exp + 视听四件套差异化

- shield technique_id 注册 `KnownTechniques`;`record_shield_block_success` 参照 `sword_basics.rs:234`,**emit 与 consume 同 PR 真接 resolve callsite**(前车之鉴:GuangboTicaoPracticeEvent emit 侧断链;另 `generic_proficiency_scalars`/`practice_session_tick`/`BackfireSurvived` 三处是 `#[allow(dead_code)]` 孤岛,**禁止声称"复用"**,要用先接活)
- 熟练度影响:block_ratio 随熟练度小幅上浮 + stamina drain 小幅下降(数值见 §8)
- 视听差异化(用户硬约束:各招差异化 animation+粒子+SFX+HUD):
  - 格挡命中:木盾=木屑迸溅(BongSpriteParticle burst 6 颗,lifetime 8t,沿受击法线,#8B6F47)+ SFX `item.shield.block`(pitch 0.9);骨盾=骨白火花(burst 6 颗,#E8DCC8)+ SFX `item.shield.block`(pitch 1.3)+ `entity.skeleton.hurt`(vol 0.3, delay 1t)
  - 命中瞬间动画:`PARRY_BLOCK` 真接线(短 recoil,endTick 6)
  - juice:新 `CombatJuiceEvent.Kind.SHIELD_BLOCK`(**不复用** PARRY——其音效硬编码剑击声)
  - HUD:格挡成功事件流一条(复用现有事件流,非新 layer;按 HUD 极简原则不加常驻元素)
- 测试:格挡成功 proficiency 上涨单测;木/骨盾粒子/SFX 按材质差异每盾独立断言;HUD 未持盾时无任何盾 UI(对齐 HUD conditional 原则)

---

## §7 与其他 plan 的交叉

- **套包 4-plan 族**(`plans-skeleton/plan-nested-pack-base-v1` 等):盾占 off_hand 与套包 loadout/装备校验共用 `inventory/mod.rs:3799` 一带;先 merge 者定基线,后实施者 rebase,**不各改一套 OffHand 逻辑**
- **plan-consumable-effects-v1**(active):右键输入层冲突——手持消耗品 vs 手持盾的右键路由须统一仲裁(按 main_hand 物品类型分发),P1 实施前对齐
- **SwordParrying**(已落地):盾格挡是其"持续版"姊妹机制,复用先例不复用类型

## §8 开放问题(P0 决策门前需收口)

1. **block_ratio 数值**:木盾/骨盾各挡多少?(建议木 0.5 / 骨 0.65,参照 SwordParrying `block_ratio.clamp(0,0.95)` 框架)
2. **stamina drain 速率**:举盾静置 vs 格挡命中追加消耗?(参照 Combat=-5/s;建议举盾 -3/s + 命中按伤害折体力)
3. **durability_max**:木/骨各多少次有效格挡?(建议木 ~40 / 骨 ~80 次满伤格挡)
4. **FOV 阈值**:±120°(dot≥-0.5)是否合适,还是收紧到 ±90°?
5. **右键路由仲裁**:与 consumable-effects 的右键分发优先级,谁实施谁先立规则?
6. **熟练度收益曲线**:block_ratio 上浮上限 / drain 下降下限(防止满熟练度无限举盾)
7. **命名**:grep 歧义防御——本 plan 全部 symbol 用 `shield_block` 前缀(避开 `woliu_vortex_shield` 技能护盾)
