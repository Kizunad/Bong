# Bong · plan-zone-atmosphere-v2 · 骨架

区域氛围视觉识别系统。当前 `RealmVisionFogController` + `RealmVisionTintRenderer` 已按境界/神识做 fog/tint——但不同 zone 之间走进去视觉上几乎一样（除了灵气条数字变化）。本 plan 给每个 zone 赋予**独立的视觉身份**：灵压雾色分层、环境粒子密度、zone 边界过渡特效、负压区视觉扭曲。让"走进血谷"和"离开灵泉湿地"是肉眼可辨的体验。

**世界观锚点**：`worldview.md §二` 灵压三态（馈赠区 / 死域 / 负灵域）视觉差异 · `§十三` 6 区域各不相同的威胁/资源/灵气 · `§十七·5 类地形季节响应`（死域恒 0、渊口寒气带冬 ×1.3 等）· `§七` 残灰方块（踩上减速+留脚印）→ 视觉上可区分死域边缘

**library 锚点**：`world-0002 末法纪略`（各区域首次描述）· `ecology-0005 异兽三形考`（不同 zone 的生态暗示）

**前置依赖**：
- `plan-vfx-v1` ✅ → 屏幕叠加层
- `plan-particle-system-v1` ✅ → 环境粒子基类
- `plan-HUD-v1` ✅ → ZoneHudRenderer（zone 信息显示）
- `plan-realm-vision` (impl) ✅ → RealmVisionFogController / FogParamsSink
- `plan-jiezeq-v1` 🆕 active → 季节 fog/sky color 覆盖
- `plan-terrain-ash-deadzone-v1` ⏳ active → 死域视觉/移动规则
- `plan-terrain-pseudo-vein-v1` ⏳ active → 伪灵脉视觉

**反向被依赖**：
- `plan-combat-gamefeel-v1` 🆕 → zone 内战斗 PVP 视野受 zone fog 影响
- `plan-breakthrough-cinematic-v1` 🆕 → 突破光柱在 zone atmosphere 中更突出

---

## 接入面 Checklist

- **进料**：`ZoneEnvironment` component（已有，plan-zone-environment-v1 ✅）/ `qi_physics::zone::ZoneQiPressure` / `RealmVisionFogController` / `RealmVisionTintRenderer` / zone 坐标/名称
- **出料**：`ZoneAtmosphereProfile`（每 zone 6 参数: fog_color / fog_density / ambient_particle / sky_tint / entry_transition_fx / sfx_ambient_loop）+ `ZoneAtmosphereRenderer`（按 profile 动态混合 fog+particles+sky）+ zone 边界过渡带（150 格渐变区域，fog/color lerp）+ 6 zone profile
- **跨仓库契约**：server 不动——纯 client 侧按 `ZoneEnvironment.zone_id` 选 profile

---

## §0 设计轴心

- [ ] **每个 zone 看起来不同**：玩家不需要看 HUD 就知道自己进了哪个区
- [ ] **灵压 ⇄ 视觉耦合**：高灵气区 = 清透/微金 fog；死域 = 灰白/远景 fade；负灵域 = 紫黑/扭曲
- [ ] **zone 边界 = 过渡带而非硬切**：150 格渐变 lerp，避免"一步天堂一步地狱"的硬边界
- [ ] **残灰方块足迹**：死域/馈赠区边缘地面方块退化为残灰方块时，视觉可见（方块 texture swap + 脚印粒子）

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|----|------|----|
| P0 | `ZoneAtmosphereProfile` 数据结构 + `ZoneAtmosphereRenderer` 骨架（fog color/density lerp + sky tint lerp）+ 初醒原 profile（淡灰蓝 fog / 低密度尘埃粒子 / 晴天 tint）+ 青云残峰 profile（青灰 fog / 山间薄雾粒子 / 微阴天 tint）+ 两 zone 之间 150 格过渡带 | ⬜ |
| P1 | 血谷 profile（暗红 fog / 铁锈味尘埃粒子 / 暗天 tint + 偶发电光粒子）+ 灵泉湿地 profile（淡绿 fog / 水雾粒子 + 萤火粒子 / 湿润天色）+ 北荒 profile（深灰 fog high density / 风沙粒子 / 灰白 dead sky）+ 幽暗地穴 profile（纯黑 fog near distance / 微光孢子粒子 / 洞壁湿反光） | ⬜ |
| P2 | 死域/负灵域特殊视觉：死域 = 全饱和度 -50% 后处理 + 远景 cut-off（150 格外纯白 void）+ 残灰方块 footprint 粒子（踩过留下灰烬脚印 30s 消散）；负灵域 = 紫黑 vignette + screen 边缘扭曲 shader（无需 Iris，DrawContext quad + noise）+ 真元被抽吸时身体周围微紫吸流粒子 | ⬜ |
| P3 | 坍缩渊（秘境）内部 atmosphere：浅/中/深层不同 fog density（浅层薄雾 / 中层浓雾 50 格 fade / 深层几乎全黑 15 格可见）+ 负压低频嗡声 ambient + 干尸/遗骸 ambient 粒子（灰尘从尸骸上升）+ 塌缩时 zone 视觉崩溃序列（fog 急速变黑 + vignette 收紧 → 全黑 + 被挤出） | ⬜ |
| P4 | 季节联动：按 `SeasonState` 动态调 zone profile（夏 = fog density ×0.8 + sky tint 微金 / 冬 = fog density ×1.3 + sky tint 灰白 / 汐转 = fog density 波动 + sky tint 间歇紫）+ 天气粒子（夏季雷暴在血谷 + 冬季飘雪在北荒） | ⬜ |
| P5 | 性能：ZoneAtmosphere profile 切换 lerp 开销 + 6 zone × 3 季节 × 多客户端 fog+particle 压测 | ⬜ |

---

## Finish Evidence（待填）

- **落地清单**：`ZoneAtmosphereProfile` ×6 / `ZoneAtmosphereRenderer` / 死域负灵域特殊视觉 / 坍缩渊 atmosphere / 季节联动
- **关键 commit**：P0-P5 各自 hash
- **遗留 / 后续**：未来新 zone 只需加法 profile（不碰 shader/管线）
