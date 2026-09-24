# plan-bughunt-r9-skill-icons-missing-v1

> 来源：从 `docs/plans-skeleton/plan-bughunt-r9-findings-v1.md` 拆出 r9 P3。本文只记录技能图标 PNG 缺失的第一性原理复核、低风险代码侧防回归和后续资产清单；不移动 round9 findings 聚合 skeleton。

## 结论

- 当前基线：`origin/main` = `d96491fb`（2026-07-07 本地已取回引用）。
- 技能栏生产链路：`SkillBarBindings` → `server/src/network/skillbar_config_emit.rs` → `known_techniques::technique_definition(skill_id)` → `SkillBarEntryV1::Skill.icon_texture` → `bong:server_data` → client `SkillBarConfigHandler`/`QuickBarHudPlanner`。
- 当前 `server/src/cultivation/known_techniques.rs` 注册 48 个技能定义，43 个 distinct 图标路径；磁盘存在 15 个，缺失 28 个。
- runtime 辅助链路（`known_techniques` + `dugu_v2`/`woliu_v2`/`tuike_v2` visual payload 中的技能图标路径）合计 53 个 distinct 图标路径；磁盘存在 20 个，缺失 33 个。
- r9 skeleton 中的 “tuike 3 缺失” 在当前 `origin/main` 已不成立：server 现在下发 `bong-client:textures/gui/items/skill_scroll_tuike_{don,shed,transfer_taint}.png`，这 3 个文件均已存在。
- r9 skeleton 中“绑定后直接渲染 missing_texture”的危害在当前 `origin/main` 已被客户端生产路径缓解：`BongHudOrchestrator` 传 `HudTextureProbe::exists`，`LoadoutIconLayer.resolveExistingSkillTexture` 只返回真实存在的贴图，否则 `QuickBarHudPlanner` 走文字标签兜底。

## 当前存在的技能栏图标

- `bong-client:textures/gui/items/skill_scroll_tuike_don.png`
- `bong-client:textures/gui/items/skill_scroll_tuike_shed.png`
- `bong-client:textures/gui/items/skill_scroll_tuike_transfer_taint.png`
- `bong-client:textures/gui/skill/zhenmai_harden.png`
- `bong-client:textures/gui/skill/zhenmai_multipoint.png`
- `bong-client:textures/gui/skill/zhenmai_neutralize.png`
- `bong-client:textures/gui/skill/zhenmai_parry.png`
- `bong-client:textures/gui/skill/zhenmai_sever_chain.png`
- `bong:textures/gui/skill/body_guangbo_ticao.png`
- `bong:textures/gui/skill/woliu_burst.png`
- `bong:textures/gui/skill/woliu_heart.png`
- `bong:textures/gui/skill/woliu_hold.png`
- `bong:textures/gui/skill/woliu_mouth.png`
- `bong:textures/gui/skill/woliu_pull.png`
- `bong:textures/gui/skill/woliu_vortex.png`

## BLOCKED: 需 /gen-image 生成清单

这些是 `TECHNIQUE_DEFINITIONS` 直接下发到技能栏的缺失 PNG。Codex 不能跑 `/gen-image`，也不能手绘占位图，因此本轮不生成假资源。

- `client/src/main/resources/assets/bong/textures/gui/skill/anqi_armor_pierce.png`
- `client/src/main/resources/assets/bong/textures/gui/skill/anqi_charge_carrier.png`
- `client/src/main/resources/assets/bong/textures/gui/skill/anqi_echo_fractal.png`
- `client/src/main/resources/assets/bong/textures/gui/skill/anqi_multi_shot.png`
- `client/src/main/resources/assets/bong/textures/gui/skill/anqi_single_snipe.png`
- `client/src/main/resources/assets/bong/textures/gui/skill/anqi_soul_inject.png`
- `client/src/main/resources/assets/bong/textures/gui/skill/beng_quan.png`
- `client/src/main/resources/assets/bong/textures/gui/skill/dugu_infuse_poison.png`
- `client/src/main/resources/assets/bong/textures/gui/skill/dugu_shoot_needle.png`
- `client/src/main/resources/assets/bong/textures/gui/skill/full_power_charge.png`
- `client/src/main/resources/assets/bong/textures/gui/skill/full_power_release.png`
- `client/src/main/resources/assets/bong/textures/gui/skill/movement_dash.png`
- `client/src/main/resources/assets/bong/textures/gui/skill/ni_mai_hu_ti.png`
- `client/src/main/resources/assets/bong/textures/gui/skill/npc_buff_defense.png`
- `client/src/main/resources/assets/bong/textures/gui/skill/npc_buff_speed.png`
- `client/src/main/resources/assets/bong/textures/gui/skill/npc_heal_basic.png`
- `client/src/main/resources/assets/bong/textures/gui/skill/shield_block.png`
- `client/src/main/resources/assets/bong/textures/gui/skill/sword_cleave.png`
- `client/src/main/resources/assets/bong/textures/gui/skill/sword_condense_edge.png`
- `client/src/main/resources/assets/bong/textures/gui/skill/sword_heaven_gate.png`
- `client/src/main/resources/assets/bong/textures/gui/skill/sword_infuse.png`
- `client/src/main/resources/assets/bong/textures/gui/skill/sword_manifest.png`
- `client/src/main/resources/assets/bong/textures/gui/skill/sword_parry.png`
- `client/src/main/resources/assets/bong/textures/gui/skill/sword_qi_slash.png`
- `client/src/main/resources/assets/bong/textures/gui/skill/sword_resonance.png`
- `client/src/main/resources/assets/bong/textures/gui/skill/sword_thrust.png`
- `client/src/main/resources/assets/bong/textures/gui/skill/tie_shan_kao.png`
- `client/src/main/resources/assets/bong/textures/gui/skill/xue_beng_bu.png`

## 追加 runtime visual 缺口

这些不属于 `TECHNIQUE_DEFINITIONS` 技能栏 distinct 缺口，但 server runtime visual payload 仍引用它们。应与同批 `/gen-image` 合并生成，避免技能事件 UI 或后续复用再撞 missing asset。

- `client/src/main/resources/assets/bong/textures/gui/skill/dugu_eclipse.png`
- `client/src/main/resources/assets/bong/textures/gui/skill/dugu_penetrate.png`
- `client/src/main/resources/assets/bong/textures/gui/skill/dugu_reverse.png`
- `client/src/main/resources/assets/bong/textures/gui/skill/dugu_self_cure.png`
- `client/src/main/resources/assets/bong/textures/gui/skill/dugu_shroud.png`

## 本轮非资产改动

- 增加 `QuickBarHudPlannerTest.skillSlotWithMissingConfiguredIconFallsBackToTextWhenProbeIsProvided`：锁定生产 overload 带贴图存在性谓词时，缺失的 server 图标不会生成 texture 命令，而是走短文字标签兜底。

## 复核命令

```bash
grep -ho "icon_texture: \"[^\"]*\"" server/src/cultivation/known_techniques.rs | sed 's/^.*icon_texture: "//; s/"$//' | sort -u
grep -Rho "bong[-a-z]*:textures/gui/skill/[^\"]*\.png\|bong[-a-z]*:textures/gui/items/skill_scroll_[^\"]*\.png" server/src --exclude-dir=target | sort -u
find client/src/main/resources/assets/bong/textures/gui/skill client/src/main/resources/assets/bong-client/textures/gui/skill client/src/main/resources/assets/bong-client/textures/gui/items -type f -name '*.png' | sort
```

## 验证结论（2026-07-26 整理审计追认）

本 plan 记录的技能图标缺失问题并未在本 plan 名下单独实施，而是被后续的 `plan-skill-av-relink-v1` 取代并实际交付：该 plan 通过 PR #1220（commit `9d2e29d08`，2026-07-18）把技能栏图标重链至 `skill_scroll` 单一真相源，一次性重链 33 个 `icon_texture`；PR #1222（commit `001bbe7d8`）补齐资产并完成归档，`docs/finished_plans/plan-skill-av-relink-v1.md` 已存在。2026-07-26 复核 `server/src/cultivation/known_techniques.rs`，49 个 `icon_texture` 路径实测 0 缺失，本 plan 记录的缺口已被完全消解。

## Finish Evidence

- **落地清单**：本 plan 自身未落地代码；实际交付落在 `plan-skill-av-relink-v1`（`docs/finished_plans/plan-skill-av-relink-v1.md`），涉及 `server/src/cultivation/known_techniques.rs`（icon_texture 单一真相源重链）+ client `assets/bong*/textures/gui/skill/` 图标资产
- **关键 commit**：`9d2e29d08`（2026-07-18，PR #1220，技能栏图标重链至 skill_scroll 单一真相源 + 图标链防回归测试）、`001bbe7d8`（PR #1222，补资产+归档）
- **测试结果**：实测 `known_techniques.rs` 49 个 icon_texture 路径 0 缺失；2026-07-26 审计为只读核验（Read+grep+git log 对拍 origin/main），未重跑测试套件
- **跨仓库核验**：server `known_techniques::TECHNIQUE_DEFINITIONS.icon_texture` ↔ client `assets/bong*/textures/gui/skill/*.png` 资产路径全量对齐（由 plan-skill-av-relink-v1 落地）
- **遗留 / 后续**：无（已被 plan-skill-av-relink-v1 完全覆盖）
