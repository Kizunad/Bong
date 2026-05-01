# Bong · plan-spawn-tutorial-v1 · 骨架

把 §L 30min 钩子表的环境线索注入 spawn_plain worldgen profile。**完全不显式提示**(O.13)——靠 POI 布置和 NPC 行为自然引导玩家完成"看灵气 → 打坐 → 打通第一条经脉 → 突破"。

**世界观锚点**：`worldview.md §十三.初醒原`(灵气 0.3 + 北边馈赠区) · `§三.醒灵/引气` line 100-110

**library 锚点**：`world-0002 末法纪略` · `world-0001 天道口述残编`(narration 风格基准)

**交叉引用**：`plan-worldgen-v3.1` ✅ · `plan-fauna-v1` ⬜(噬元鼠) · `plan-narrative-v1` ⏳(教学 narration 分支) · `plan-gameplay-journey-v1` §L/P0

---

## 接入面 Checklist

- **进料**：`worldgen/scripts/terrain_gen/profiles/spawn_plain.py` ✅ + 30min 钩子表(§L)
- **出料**：注入后的 spawn_plain 包含: 半埋石棺 + 一次性龛石 + 教学小灵泉 ×2 + 友善散修 + 开脉丹宝箱 + 噬元鼠群路径
- **共享类型**：复用 worldgen blueprint POI 体系 + `inventory::initial_grant`
- **跨仓库契约**：worldgen export → server runtime POI 加载 → tiandao narration 分支
- **worldview 锚点**：§十三 初醒原(灵气浓度 0.3 + 北边 200-500 格 0.5+ 馈赠区)

---

## §0 设计轴心

- [ ] **沉默引导**(O.13)：所有教学不通过 UI/弹窗/任务面板,只靠环境布置
- [ ] **路径暗示**：灵气条变色 + 散修走向馈赠区 + tiandao 偶发台词("风从东北来,那里的空气稍微厚一点") → 玩家自己悟
- [ ] **可被打破**：玩家完全不按教学走也能玩——只是失去 30min 内的最佳路径

---

## §1 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ⬜ | 半埋石棺出生点 + 一次性龛石 + 灰白残灰地面 | 玩家从棺旁醒来,5 格内可拾龛石 |
| **P1** ⬜ | 教学小灵泉 ×2(灵气 0.5+,半径 5 格) + 开脉丹小宝箱 | 灵泉肉眼可见(色温更暖 + 草叶绿色) |
| **P2** ⬜ | 友善散修 NPC(`Rogue` archetype ✅) + tiandao narration 分支 | 1 名散修在 spawn 200 格内,会主动 narration |
| **P3** ⬜ | 噬元鼠群路径(玩家走向灵泉时 80% 遭遇) | 鼠扣真元不扣 HP,死亡的玩家在灵龛重生 |

---

## §2 spawn_plain POI 注入清单

| POI | 位置(相对 spawn) | 内容 |
|---|---|---|
| 半埋石棺 | (0, 64, 0) | 自定义 BlockEntity,出生点视觉锚 |
| 龛石 item | spawn 身边 | inventory::initial_grant 一次性 |
| 教学小灵泉 #1 | (50, 65, 100) 灵气 0.5,半径 5 | 草叶绿色 + 色温暖 |
| 教学小灵泉 #2 | (-30, 64, -80) 灵气 0.5,半径 5 | 备选路径 |
| 开脉丹小宝箱 | 灵泉 #1 旁 5 格内 | 1 颗开脉丹 |
| 友善散修 NPC | spawn 200 格内随机 | `Rogue` archetype + 教学 narration trigger |
| 噬元鼠群 ×2-3 | spawn ↔ 灵泉路径上 | 扣真元不扣 HP |

---

## §3 30min 钩子触发链(对齐 plan-gameplay-journey-v1 §L)

```
0:00  玩家在棺旁醒来
0:00  tiandao 第一句 narration("你又醒了..." 重生 / "你醒了..." 新角色)
5:00  玩家移动 200 格触发灵气条变色
10:00 玩家右键长按打坐(无 UI 提示,客户端 input 监听)
15:00 第一条正经打通(自动选最近邻接的)
20:00 噬元鼠群遭遇(扣真元) — 真元 < 30 触发逃跑视觉
25:00 玩家走到灵泉(灵气 0.5+) 准备突破
30:00 醒灵 → 引气突破成功 + 世界变色
```

---

## §4 数据契约

- [ ] `worldgen/scripts/terrain_gen/profiles/spawn_plain.py` POI 布置扩展
- [ ] `server/src/world/spawn_tutorial.rs` runtime POI 加载 + 一次性教学逻辑
- [ ] `server/src/inventory/initial_grant.rs` 龛石 + 凡铁刀给玩家
- [ ] `agent/.../skills/era.md` 重生 vs 新角色第一句台词分支(plan-narrative-v1 协调)
- [ ] 5 句 narration 风格基准台词写入 plan-narrative-v1(详 §L)

---

## §5 开放问题

- [ ] 龛石放出生点身边是否太"露馅"? 是否要藏在残灰下让玩家挖?
- [ ] 友善散修是否会被玩家杀掉(若杀掉则失去引导)? 是否给 NPC 不死状态 1h?
- [ ] 30min 路径玩家偏离怎么办(玩家南边走 1000 格)? 天道是否再 nudge 一次?
- [ ] 多人联机时,所有人都需要走 30min 教学吗? 还是第二个加入的玩家直接进 P1?

## §6 进度日志

- 2026-05-01：骨架创建。plan-gameplay-journey-v1 §L 钩子表 + P0 决策派生。
