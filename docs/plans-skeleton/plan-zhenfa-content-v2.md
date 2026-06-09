# plan-zhenfa-content-v2 — 凡阶阵法内容:聚灵阵/散灵珠/阵旗组网(骨架)

> 一句话:放置类僵尸物品「阵法」消杀——gather_array_base 接通 Lingju 空分支、qi_scatter_bead 走守恒散逸、array_flag_basic + array_eye_basic 实装**组网阵**(旗圈边界 + 眼激活,用户拍板候选 A,2026-06-10)。
>
> 来源:放置类 17 调查 workflow(opus 抽查 7/7 属实);承接 finished `plan-zhenfa-content-v1`。

**依赖**:plan-block-lifecycle-v1 P4 合入(放置管线);qi_physics 扩展先行(P0)。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | qi_physics 扩展 + ID 统一裁决落地 | ⬜ |
| P1 | gather_array_base 接通 Lingju(聚灵真生效) | ⬜ |
| P2 | qi_scatter_bead 守恒散逸 | ⬜ |
| P3 | 阵旗组网:旗圈边界 + 阵眼激活 | ⬜ |

---

## 接入面(防孤岛 checklist)

- **进料**:
  - zhenfa 系统:`zhenfa/mod.rs:309` ZhenfaRegistry / `:1213` anchor spawn / `:1582` tick_zhenfa_registry(proximity)/ `:1711` **`ZhenfaKind::Lingju => {}` 空分支**(调查坐实:LingArrayDeployEvent 只 Redis 广播,server 侧聚灵从未生效)
  - lingtian:`environment.rs:37` `PlotEnvironment.zhenfa_jvling`(**从未被写 true**)+ `:117` compute_plot_qi_cap(+1.0 cap 已处理此 flag)
  - qi_physics:`release.rs:12-47` `qi_release_to_zone(amount, from, zone, zone_current, zone_cap) -> Result<ZoneReleaseOutcome>`(含 `transfer: Option<QiTransfer>`)/ `excretion.rs:6-32` `qi_excretion(initial, ContainerKind, elapsed_secs, EnvField)`(`ContainerKind::EmbeddedTrap` 已存在 carrier.rs:60)/ `ledger.rs:140-202` QiTransferReason 18 变体
  - client:`MixinClientPlayerInteractionManagerAlchemy.java:138` `bong$zhenfaKindForItem` 物品→ZhenfaKind 映射
- **出料**:聚灵阵真效果(plot qi cap 扩张)、散灵珠 zone 注入(干扰追踪)、组网阵范围 tag(警戒/小幅聚灵)
- **共享类型 / event**:全部复用 `ZhenfaKind` 扩变体 + ZhenfaPlace C2S(client_request.rs:257),**不另开放置路径**;ledger 复用 `QiTransferReason::ReleaseToZone`(语义吻合,不新增变体,降低改动面)
- **跨仓库契约**:server ZhenfaKind 扩枚举 ↔ client zhenfaKindForItem 映射 ↔ schema sample 对拍;LingArrayDeployEvent 继续广播 agent
- **worldview 锚点**:§五.3 地师/阵法流(环境改造者);§二 灵压环境(聚灵=局部浓度操作);worldview.md:417「无人上套真元几小时随载体朽坏白白流失」(散灵珠持续逸散)
- **qi_physics 锚点(强约束)**:
  - 聚灵=**容量扩张非真元搬运**(参 HalfStepBuff audit-only 模式):不调 `WorldQiAccount::transfer`,只写 `zhenfa_jvling=true` + emit 审计 event
  - 散灵珠=**守恒搬运**:`qi_release_to_zone(bead_qi, QiAccountId::container("qi_scatter:{owner}:{id}"), zone, ...)` → **必须 emit `outcome.transfer`**(前车之鉴:emit-only 无 consumer = 吞真元红旗,abstract_combat 烂账)
  - 预埋未触发的衰减走 `qi_excretion(_, ContainerKind::EmbeddedTrap, _, _)`,**禁止自写衰减常数**

---

## P0 — ID 统一裁决 + qi_physics 扩展

- **gather_array_base ↔ zhenfa_array_lingju 双路径收口**(调查红旗 #10 守恒漏洞):gather_array_base 定位为 Lingju 凡阶版,统一走 `ZhenfaKind::Lingju`,同效果单来源
- **qi_scatter_bead ↔ scattered_qi_pearl 重名切割**(红旗 #11):前者=主动投掷散逸道具,后者=破阵被动掉落,文档+代码注释双锚定
- **array_flag_basic ↔ `ZHENFA_FLAG_ITEM_ID="array_flag"`(zhenfa/mod.rs:49)断链收口**:组网阵走新 `ZhenfaKind::NetworkArray`,旧 array_flag 保持原义
- qi_physics 若需新常数(散灵珠容量、组网阵聚灵幅度)在 `qi_physics/constants.rs` 加,本 plan 只声明参数
- 测试:三组 ID 的 grep 全仓唯一性;ZhenfaKind 新变体 pin 测试

## P1 — gather_array_base 接通 Lingju

- client `bong$zhenfaKindForItem` 加 `gather_array_base → Lingju`
- Lingju tick 实装(zhenfa/mod.rs:1711 空分支):覆盖范围内 plot `zhenfa_jvling = true`(environment.rs 已有消费),阵移除时回 false
- 视听:激活时青绿光柱粒子(BongLineParticle 8 根 radial,lifetime 20t,#7FD8A8,continuous 低频)+ SFX `block.amethyst_block.chime`(pitch 0.8,vol 0.6)+ narration(zone,perception)「此地灵气似有汇聚之势」
- 测试:范围内 plot qi cap +1.0 生效/移除回退/范围外不影响;LingArrayDeployEvent 回归

## P2 — qi_scatter_bead 守恒散逸

- use handler(投掷/埋设):`qi_release_to_zone` 调用链(见 qi_physics 锚点)+ emit QiTransfer;追踪干扰效果挂 zone tag(追踪系统读)
- 预埋未触发:`qi_excretion(EmbeddedTrap)` 持续逸散,归零自毁
- 视听:破裂白雾喷散(BongSpriteParticle burst 14 颗,radial,lifetime 16t,#E8F0EE)+ SFX `block.glass.break`(pitch 1.4)+`entity.breeze.idle_air`(delay 3t,vol 0.4)
- 测试:ledger 守恒断言(zone 增=珠减)/clamp 到 zone 浓度边界/逸散曲线/重复使用拒绝

## P3 — 阵旗组网(旗圈边界 + 眼激活)

- 玩法:阵旗(凡,spirit_quality=0)放 3-4 角圈定凸多边形边界 → 阵眼(spirit_quality=0.5)放圈内激活 → 组网成阵
- 组网判定:新 `NetworkArrayRegistry`(旗位置集合 + 眼 entity);眼放置时扫描半径内旗 ≥3 且围成面积 ≤ 上限 → 成阵
- 凡阶效果(弱):警戒(圈内 NPC/玩家进入 → owner 收 HUD 事件流提示,复用 WarningTrap 节流 WARD_ALERT_THROTTLE_TICKS)+ 小幅聚灵(走 P1 同款 zhenfa_jvling 机制,幅度减半,常数归 qi_physics)
- 拆阵:任一旗/眼被破坏 → 全阵失效 + owner 提示
- 视听:成阵瞬间旗间青色光弦连线(BongRibbonParticle 沿边界,lifetime 30t,#96D6EC)+ 各旗顶火星(burst 3 颗);SFX `block.beacon.activate`(pitch 1.3,vol 0.5);破阵 SFX `block.beacon.deactivate`;HUD 事件流「阵成/阵破」
- 测试:3 旗成阵/2 旗不成/面积超限拒/旗被破全阵失效/双阵不重叠;组网几何边界 off-by-one

---

## §8 开放问题(P0 决策门前需收口)

1. **组网几何**:旗数下限(3 还是 4)、最大围合面积、眼-旗最大距离(建议 3 旗起、16×16 上限)
2. **散灵珠"干扰追踪"的消费方**:当前追踪/嗅探系统在哪(NPC 感知? tsy 搜刮?)——若无消费方,效果先收敛为"zone 浓度扰动 + 留 tag",消费留给追踪 plan(防 emit-only 孤岛)
3. **聚灵幅度常数**:Lingju 满阵 +1.0 cap 已定;组网阵凡阶建议 +0.5,需 qi_physics 侧确认 cap 叠加规则(双阵覆盖同 plot 取 max 还是相加——守恒视角应取 max)
4. **FormationCore bbmodel 复用**(红旗 #12):阵眼实体直接挂现有 FormationCore(EntityKind 154)还是新模型?旗需新 bbmodel(小件,挂幡造型)
