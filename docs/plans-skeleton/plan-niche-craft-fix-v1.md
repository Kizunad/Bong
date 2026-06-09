# plan-niche-craft-fix-v1 — 灵龛制作流断链修复(骨架)

> 一句话:修复 niche_base 双断链(配方产出 niche_base 但放置 handler 只认 spirit_niche_stone,整段制作流形同虚设)+ 给 niche_repair_kit 建 use 闭环。
>
> 来源:放置类 17 调查 workflow,**红旗 #9(最严重)**:`social/mod.rs:80` 配方消耗 spirit_niche_stone 产出 niche_base,而 `handle_spirit_niche_place_requests`(social/mod.rs:1466)只认 `SPIRIT_NICHE_ITEM_TEMPLATE_ID="spirit_niche_stone"`。

**依赖**:无(灵龛放置链路已实装,本 plan 是纯断链修复)。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | niche_base 接通放置(ID 断链收口) | ⬜ |
| P1 | 灵龛损伤/修补生命周期 + niche_repair_kit use 闭环 | ⬜ |

---

## 接入面(防孤岛 checklist)

- **进料**:`social/mod.rs:80`(配方)/ `:1466` handle_spirit_niche_place_requests / SpiritNiche 实体系统(plan-niche-defense-v1,finished)/ `niche_repair_kit` 模板(workbench_materials.toml,"碎石灵铁混合料,修补损坏灵龛")
- **出料**:玩家手搓 niche_base → 放置成永久复活点;灵龛受损 → repair_kit 修复
- **共享类型 / event**:复用 SpiritNiche 全套,不新造;损伤状态若 niche-defense 已有 HP 字段直接用
- **跨仓库契约**:client 放置/修补走既有 niche intent;agent 不参与
- **worldview 锚点**:§十二 死亡、重生与一生记录(灵龛=永久复活点)
- **qi_physics 锚点**:无(修复消耗为物料;若 niche-defense 已有真元维护机制则只读不改)
- **遗留资产红旗顺带修**(调查 #8):`NicheDefenseReactionVfxPlayer.java` 返回 `niche_guardian_broken` 等 SFX ID 无资产文件——P1 补 audio_recipe JSON 或改用现有 ID

---

## P0 — niche_base 接通放置

- 裁决二选一(§8 #1):A. handler 改认 niche_base(spirit_niche_stone 降为材料);B. 配方产物改回 spirit_niche_stone,删 niche_base
- 接通后 e2e:合成 → 放置 → 复活点注册 → 死亡复活于此
- 测试:断链 ID grep 全仓归一;放置/复活回归

## P1 — 灵龛损伤/修补 + repair_kit

- 灵龛可受损(niche-defense 袭击事件已有,确认损伤落字段);损坏态复活功能降级/停用
- niche_repair_kit use handler:对受损灵龛使用 → 消耗 → 恢复;完好灵龛拒绝使用
- 视听:修补 SFX `block.smithing_table.use`(pitch 1.1)+ 石屑弥合粒子(BongSpriteParticle burst 6 颗 #B8B0A0);补活 niche_guardian SFX 断链(见接入面)
- 测试:损伤→修补→复活功能恢复 e2e;完好拒用/材料不足拒用;SFX 资产存在性断言

---

## §8 开放问题(P0 决策门前需收口)

1. **断链方向**:handler 认 niche_base(A,保留制作链语义)vs 配方改产 spirit_niche_stone(B,改动最小)——倾向 A,niche_base description 本就写"永久复活点基座"
2. **损坏态行为**:复活功能完全停用 vs 降级(复活后重伤 debuff)
3. **灵龛 HP 模型**:niche-defense 现状是否已有损伤字段(实施前 grep 确认,无则本 plan 补)
